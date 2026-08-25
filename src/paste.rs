use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use gdk4::prelude::*;
use serde::Deserialize;

#[derive(Clone, Debug)]
pub struct TargetWindow {
    pub address: String,
    pub wm_class: String,
    pub title: String,
    pub tags: Vec<String>,
}

impl TargetWindow {
    pub fn is_terminal(&self) -> bool {
        if self
            .tags
            .iter()
            .any(|t| t.trim_end_matches('*') == "terminal")
        {
            return true;
        }
        let lowered = self.wm_class.to_lowercase();
        ["ghostty", "kitty", "alacritty", "foot", "wezterm", "rio"]
            .iter()
            .any(|n| lowered.contains(n))
    }
}

#[derive(Deserialize)]
struct HyprWindow {
    address: Option<String>,
    class: Option<String>,
    title: Option<String>,
    tags: Option<Vec<String>>,
}

fn run(
    argv: &[&str],
    input: Option<&[u8]>,
    timeout: Option<Duration>,
) -> std::io::Result<std::process::Output> {
    let mut cmd = Command::new(argv[0]);
    cmd.args(&argv[1..])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if input.is_some() {
        cmd.stdin(Stdio::piped());
    }
    let mut child = cmd.spawn()?;
    if let Some(bytes) = input {
        if let Some(mut stdin) = child.stdin.take() {
            use std::io::Write;
            let _ = stdin.write_all(bytes);
        }
    }
    if let Some(limit) = timeout {
        let start = std::time::Instant::now();
        loop {
            match child.try_wait()? {
                Some(_) => return child.wait_with_output(),
                None if start.elapsed() > limit => {
                    let _ = child.kill();
                    return child.wait_with_output();
                }
                None => thread::sleep(Duration::from_millis(20)),
            }
        }
    }
    child.wait_with_output()
}

pub fn current_window() -> Option<TargetWindow> {
    let out = run(&["hyprctl", "activewindow", "-j"], None, None).ok()?;
    if !out.status.success() || out.stdout.iter().all(|b| b.is_ascii_whitespace()) {
        return None;
    }
    let data: HyprWindow = serde_json::from_slice(&out.stdout).ok()?;
    let address = data.address.filter(|s| !s.is_empty())?;
    Some(TargetWindow {
        address,
        wm_class: data.class.unwrap_or_default(),
        title: data.title.unwrap_or_default(),
        tags: data.tags.unwrap_or_default(),
    })
}

pub fn focus_window(target: &TargetWindow) {
    let lua = format!(
        "hl.dispatch(hl.dsp.focus({{ window = \"address:{}\" }}))",
        target.address
    );
    let _ = run(&["hyprctl", "eval", &lua], None, None);
}

pub fn copy_text(text: &str) {
    if gdk_copy_text(text) {
        return;
    }
    wl_copy(&["wl-copy"], text.as_bytes());
}

pub fn copy_image(path: &Path, mime: &str) {
    let Ok(data) = std::fs::read(path) else {
        return;
    };
    if gdk_copy_bytes(&data, mime) {
        return;
    }
    wl_copy(&["wl-copy", "--type", mime], &data);
}

fn gdk_copy_text(text: &str) -> bool {
    let Some(display) = gdk4::Display::default() else {
        return false;
    };
    display.clipboard().set_text(text);
    true
}

fn gdk_copy_bytes(payload: &[u8], mime: &str) -> bool {
    let Some(display) = gdk4::Display::default() else {
        return false;
    };
    let bytes = glib::Bytes::from(payload);
    let provider = gdk4::ContentProvider::for_bytes(mime, &bytes);
    display.clipboard().set_content(Some(&provider)).is_ok()
}

fn wl_copy(argv: &[&str], payload: &[u8]) {
    match run(argv, Some(payload), Some(Duration::from_secs(1))) {
        Ok(out) if !out.status.success() => {
            log::warn!("wl-copy failed: {}", String::from_utf8_lossy(&out.stderr));
        }
        Err(err) => log::warn!("wl-copy failed: {err}"),
        _ => {}
    }
}

pub fn send_paste(target: Option<&TargetWindow>, paste_keys: &str) {
    let use_shift = paste_keys == "shift-insert"
        || (paste_keys == "auto" && target.map(|t| t.is_terminal()).unwrap_or(false));
    let argv: &[&str] = if use_shift {
        &["wtype", "-M", "shift", "-k", "Insert", "-m", "shift"]
    } else {
        &["wtype", "-M", "ctrl", "-k", "v", "-m", "ctrl"]
    };
    if let Ok(out) = run(argv, None, None) {
        if !out.status.success() {
            log::warn!(
                "wtype paste failed: {}",
                String::from_utf8_lossy(&out.stderr)
            );
        }
    }
}

pub fn paste_now(target: Option<&TargetWindow>, paste_keys: &str) {
    if let Some(t) = target {
        focus_window(t);
    }
    thread::sleep(Duration::from_millis(150));
    send_paste(target, paste_keys);
}
