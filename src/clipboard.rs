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
    let (mime, payload) = if label.starts_with("image") {
        let mut mime = first_image_mime(&types).unwrap_or("image/png").to_string();
        let mut payload = paste_bytes(&["wl-paste", "--type", &mime, "--no-newline"]);
        if payload.is_empty() {
            payload = paste_bytes(&["wl-paste", "--type", "image/png", "--no-newline"]);
            mime = "image/png".into();
        }
        (mime, payload)
    } else {
        (
            "text/plain".into(),
            paste_bytes(&["wl-paste", "--type", "text", "--no-newline"]),
        )
    };
    ingest(
        store,
        config,
        images_dir,
        label,
        &types,
        &mime,
        &payload,
        ignore_hash,
        None,
    )
}

fn ingest(
    store: &Store,
    config: &Config,
    images_dir: &std::path::Path,
    label: &str,
    types: &[String],
    mime: &str,
    payload: &[u8],
    ignore_hash: &Mutex<Option<(String, i64)>>,
    now: Option<i64>,
) -> Result<Option<Clip>, String> {
    if config.ignore_secrets && looks_secret(types) {
        return Ok(None);
    }
    if payload.is_empty() || payload.len() as i64 > config.max_bytes {
        return Ok(None);
    }
    if label.starts_with("image") {
        let digest = content_hash("image", mime, payload);
        if should_ignore(&digest, ignore_hash) {
            return Ok(None);
        }
        let image_path = images_dir.join(format!("{digest}.bin"));
        if !image_path.exists() {
            std::fs::write(&image_path, payload).map_err(|e| e.to_string())?;
        }
        return store
            .add(
                "image",
                mime,
                payload,
                None,
                "Image",
                Some(image_path.to_str().unwrap_or_default()),
                &config.default_keep,
                config.max_items,
                now,
            )
            .map_err(|e| e.to_string());
    }

    let text = String::from_utf8_lossy(payload).into_owned();
    if text.trim().is_empty() {
        return Ok(None);
    }
    let digest = content_hash("text", "text/plain", payload);
    if should_ignore(&digest, ignore_hash) {
        return Ok(None);
    }
    store
        .add(
            "text",
            "text/plain",
            payload,
            Some(&text),
            &make_preview(&text, 280),
            None,
            &config.default_keep,
            config.max_items,
            now,
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
    parse_mime_list(&String::from_utf8_lossy(&out.stdout))
}

fn parse_mime_list(stdout: &str) -> Vec<String> {
    stdout
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_mime_types_are_skipped() {
        assert!(looks_secret(&[
            "text/plain".into(),
            "x-kde-passwordManagerHint".into()
        ]));
        assert!(looks_secret(&["application/x-keepassxc".into()]));
        assert!(looks_secret(&["text/secret".into()]));
        assert!(looks_secret(&["x-nm-origin".into()]));
        assert!(!looks_secret(&["text/plain".into(), "text/html".into()]));
        assert!(!looks_secret(&[]));
    }

    #[test]
    fn first_image_mime_prefers_listed_image() {
        let types = vec!["text/plain".into(), "image/png".into(), "image/jpeg".into()];
        assert_eq!(first_image_mime(&types), Some("image/png"));
        assert_eq!(first_image_mime(&["text/plain".into()]), None);
    }

    #[test]
    fn ignore_hash_only_matches_live_digest() {
        let now = glib::monotonic_time();
        let slot = Mutex::new(Some(("abc".into(), now + 5_000_000)));
        assert!(should_ignore("abc", &slot));
        assert!(!should_ignore("def", &slot));
        *slot.lock().unwrap() = Some(("abc".into(), now - 1));
        assert!(!should_ignore("abc", &slot));
        *slot.lock().unwrap() = None;
        assert!(!should_ignore("abc", &slot));
    }

    #[test]
    fn parse_mime_list_drops_blank_lines() {
        assert_eq!(
            parse_mime_list("text/plain\n\n  image/png  \n"),
            vec!["text/plain".to_string(), "image/png".to_string()]
        );
        assert!(parse_mime_list("").is_empty());
    }

    fn cfg() -> Config {
        Config {
            default_keep: "1d".into(),
            max_items: 50,
            max_bytes: 100,
            ignore_secrets: true,
            paste_keys: "auto".into(),
        }
    }

    fn ingest_text(
        store: &Store,
        dir: &std::path::Path,
        config: &Config,
        types: &[&str],
        payload: &[u8],
        ignore: &Mutex<Option<(String, i64)>>,
    ) -> Option<Clip> {
        let types: Vec<String> = types.iter().map(|s| (*s).to_string()).collect();
        ingest(
            store,
            config,
            dir,
            "text",
            &types,
            "text/plain",
            payload,
            ignore,
            Some(10),
        )
        .unwrap()
    }

    #[test]
    fn ingest_skips_secrets_empty_whitespace_and_oversize() {
        let dir = tempfile::TempDir::new().unwrap();
        let store = Store::open(&dir.path().join("db")).unwrap();
        let ignore = Mutex::new(None);
        let images = dir.path().join("images");
        std::fs::create_dir_all(&images).unwrap();

        assert!(ingest_text(
            &store,
            &images,
            &cfg(),
            &["text/plain", "x-kde-passwordManagerHint"],
            b"secret",
            &ignore,
        )
        .is_none());

        let mut open = cfg();
        open.ignore_secrets = false;
        assert!(ingest_text(
            &store,
            &images,
            &open,
            &["text/plain", "x-kde-passwordManagerHint"],
            b"secret",
            &ignore,
        )
        .is_some());

        assert!(ingest_text(&store, &images, &cfg(), &["text/plain"], b"", &ignore).is_none());
        assert!(ingest_text(&store, &images, &cfg(), &["text/plain"], b"   \n", &ignore).is_none());
        let big = vec![b'a'; 101];
        assert!(ingest_text(&store, &images, &cfg(), &["text/plain"], &big, &ignore).is_none());
    }

    #[test]
    fn ingest_stores_text_and_honors_ignore_hash() {
        let dir = tempfile::TempDir::new().unwrap();
        let store = Store::open(&dir.path().join("db")).unwrap();
        let ignore = Mutex::new(None);
        let images = dir.path().join("images");
        std::fs::create_dir_all(&images).unwrap();

        let clip = ingest_text(
            &store,
            &images,
            &cfg(),
            &["text/plain"],
            b"hello world",
            &ignore,
        )
        .unwrap();
        assert_eq!(clip.kind, "text");
        assert_eq!(clip.preview, "hello world");
        assert_eq!(clip.keep_preset, "1d");

        let digest = content_hash("text", "text/plain", b"hello world");
        *ignore.lock().unwrap() = Some((digest, glib::monotonic_time() + 5_000_000));
        assert!(ingest_text(
            &store,
            &images,
            &cfg(),
            &["text/plain"],
            b"hello world",
            &ignore,
        )
        .is_none());
    }

    #[test]
    fn ingest_writes_image_once() {
        let dir = tempfile::TempDir::new().unwrap();
        let store = Store::open(&dir.path().join("db")).unwrap();
        let ignore = Mutex::new(None);
        let images = dir.path().join("images");
        std::fs::create_dir_all(&images).unwrap();
        let payload = b"png-bytes-here";
        let types = vec!["image/png".to_string()];
        let clip = ingest(
            &store,
            &cfg(),
            &images,
            "image/png",
            &types,
            "image/png",
            payload,
            &ignore,
            Some(10),
        )
        .unwrap()
        .unwrap();
        assert_eq!(clip.kind, "image");
        let path = std::path::PathBuf::from(clip.image_path.unwrap());
        assert_eq!(std::fs::read(&path).unwrap(), payload);

        std::fs::write(&path, b"stale").unwrap();
        ingest(
            &store,
            &cfg(),
            &images,
            "image/png",
            &types,
            "image/png",
            payload,
            &ignore,
            Some(11),
        )
        .unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"stale");
    }

    #[test]
    fn wl_paste_stub_on_path() {
        let _lock = crate::env_lock();
        let dir = tempfile::TempDir::new().unwrap();
        let bin = dir.path().join("wl-paste");
        std::fs::write(
            &bin,
            "#!/bin/sh\ncase \"$1\" in\n--list-types) printf 'text/plain\\nimage/png\\n' ;;\n*) printf 'hello' ;;\nesac\n",
        )
        .unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
        let old = std::env::var("PATH").unwrap_or_default();
        std::env::set_var("PATH", format!("{}:{old}", dir.path().display()));
        assert_eq!(
            list_types(),
            vec!["text/plain".to_string(), "image/png".to_string()]
        );
        assert_eq!(paste_bytes(&["wl-paste", "--type", "text"]), b"hello");
        std::env::set_var("PATH", old);
    }
}
