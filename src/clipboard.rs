use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use crate::config::Config;
use crate::store::{content_hash, make_preview, Clip, Store};

const SECRET_HINTS: &[&str] = &[
    "x-kde-passwordmanagerhint",
    "x-nm-origin",
    "text/secret",
    "application/x-keepassxc",
];

pub struct ClipboardWatcher {
    store: Rc<Store>,
    config: Config,
    images_dir: PathBuf,
    on_change: Rc<dyn Fn(Clip)>,
    ignore_hash: Rc<Mutex<Option<(String, i64)>>>,
    ignore_all_until: Rc<Mutex<i64>>,
    stopping: Arc<AtomicBool>,
}

impl ClipboardWatcher {
    pub fn new(
        store: Rc<Store>,
        config: Config,
        images_dir: PathBuf,
        on_change: Rc<dyn Fn(Clip)>,
    ) -> Self {
        Self {
            store,
            config,
            images_dir,
            on_change,
            ignore_hash: Rc::new(Mutex::new(None)),
            ignore_all_until: Rc::new(Mutex::new(0)),
            stopping: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn start(&self) {
        self.watch("text", &["wl-paste", "--type", "text", "--watch", "echo"]);
        self.watch(
            "image/png",
            &["wl-paste", "--type", "image/png", "--watch", "echo"],
        );
    }

    pub fn stop(&self) {
        self.stopping.store(true, Ordering::SeqCst);
    }

    pub fn ignore_hash(&self, digest: &str, seconds: f64) {
        let now = glib::monotonic_time();
        let hold = (seconds * 1_000_000.0) as i64;
        *self.ignore_hash.lock().unwrap() = Some((digest.to_string(), now + hold));
        *self.ignore_all_until.lock().unwrap() = now + hold;
    }

    fn watch(&self, label: &'static str, argv: &[&str]) {
        let mut cmd = Command::new(argv[0]);
        cmd.args(&argv[1..])
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let Ok(mut child) = cmd.spawn() else {
            log::error!("wl-paste is not installed");
            return;
        };
        let stdout = match child.stdout.take() {
            Some(s) => s,
            None => return,
        };
        let (tx, rx) = async_channel::unbounded::<&'static str>();
        let stopping_t = self.stopping.clone();
        thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for _line in reader.lines() {
                if stopping_t.load(Ordering::SeqCst) {
                    break;
                }
                let _ = tx.send_blocking(label);
            }
            let _ = child.wait();
        });

        let store = self.store.clone();
        let config = self.config.clone();
        let images_dir = self.images_dir.clone();
        let on_change = self.on_change.clone();
        let ignore_hash = self.ignore_hash.clone();
        let ignore_all_until = self.ignore_all_until.clone();
        let stopping = self.stopping.clone();
        glib::MainContext::default().spawn_local(async move {
            while let Ok(label) = rx.recv().await {
                if stopping.load(Ordering::SeqCst) {
                    break;
                }
                if glib::monotonic_time() < *ignore_all_until.lock().unwrap() {
                    continue;
                }
                match capture(&store, &config, &images_dir, label, &ignore_hash) {
                    Ok(Some(clip)) => on_change(clip),
                    Ok(None) => {}
                    Err(err) => log::error!("failed to capture clipboard ({label}): {err}"),
                }
            }
        });
    }
}

fn capture(
    store: &Store,
    config: &Config,
    images_dir: &std::path::Path,
    label: &str,
    ignore_hash: &Mutex<Option<(String, i64)>>,
) -> Result<Option<Clip>, String> {
    let types = list_types();
    if config.ignore_secrets && looks_secret(&types) {
        return Ok(None);
    }
    if label.starts_with("image") {
        let mut mime = first_image_mime(&types).unwrap_or("image/png").to_string();
        let mut payload = paste_bytes(&["wl-paste", "--type", &mime, "--no-newline"]);
        if payload.is_empty() {
            payload = paste_bytes(&["wl-paste", "--type", "image/png", "--no-newline"]);
            mime = "image/png".into();
        }
        if payload.is_empty() || payload.len() as i64 > config.max_bytes {
            return Ok(None);
        }
        let digest = content_hash("image", &mime, &payload);
        if should_ignore(&digest, ignore_hash) {
            return Ok(None);
        }
        let image_path = images_dir.join(format!("{digest}.bin"));
        if !image_path.exists() {
            std::fs::write(&image_path, &payload).map_err(|e| e.to_string())?;
        }
        return store
            .add(
                "image",
                &mime,
                &payload,
                None,
                "Image",
                Some(image_path.to_str().unwrap_or_default()),
                &config.default_keep,
                config.max_items,
                None,
            )
            .map_err(|e| e.to_string());
    }

    let payload = paste_bytes(&["wl-paste", "--type", "text", "--no-newline"]);
    if payload.is_empty() || payload.len() as i64 > config.max_bytes {
        return Ok(None);
    }
    let text = String::from_utf8_lossy(&payload).into_owned();
    if text.trim().is_empty() {
        return Ok(None);
    }
    let digest = content_hash("text", "text/plain", &payload);
    if should_ignore(&digest, ignore_hash) {
        return Ok(None);
    }
    store
        .add(
            "text",
            "text/plain",
            &payload,
            Some(&text),
            &make_preview(&text, 280),
            None,
            &config.default_keep,
            config.max_items,
            None,
        )
        .map_err(|e| e.to_string())
}

fn should_ignore(digest: &str, ignore_hash: &Mutex<Option<(String, i64)>>) -> bool {
    let now = glib::monotonic_time();
    match ignore_hash.lock().unwrap().as_ref() {
        Some((h, until)) => h == digest && now < *until,
        None => false,
    }
}

fn list_types() -> Vec<String> {
    let output = Command::new("wl-paste")
        .arg("--list-types")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output();
    let Ok(out) = output else {
        return Vec::new();
    };
    if !out.status.success() {
        return Vec::new();
    }
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn looks_secret(types: &[String]) -> bool {
    types.iter().any(|item| {
        let lower = item.to_lowercase();
        SECRET_HINTS.iter().any(|h| lower.contains(h))
    })
}

fn first_image_mime(types: &[String]) -> Option<&str> {
    types
        .iter()
        .map(|s| s.as_str())
        .find(|t| t.starts_with("image/"))
}

fn paste_bytes(argv: &[&str]) -> Vec<u8> {
    Command::new(argv[0])
        .args(&argv[1..])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| o.stdout)
        .unwrap_or_default()
}
