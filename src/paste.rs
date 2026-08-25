use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

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
        let deadline = Instant::now() + limit;
        loop {
            if let Some(status) = child.try_wait()? {
                let mut stdout = Vec::new();
                let mut stderr = Vec::new();
                if let Some(mut s) = child.stdout.take() {
                    let _ = s.read_to_end(&mut stdout);
                }
                if let Some(mut s) = child.stderr.take() {
                    let _ = s.read_to_end(&mut stderr);
                }
                return Ok(std::process::Output {
                    status,
                    stdout,
                    stderr,
                });
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                return child.wait_with_output();
            }
            thread::sleep(Duration::from_millis(20));
        }
    }
    child.wait_with_output()
}

pub fn current_window() -> Option<TargetWindow> {
    let out = run(&["hyprctl", "activewindow", "-j"], None, None).ok()?;
    if !out.status.success() {
        return None;
    }
    window_from_hypr_json(&out.stdout)
}

fn window_from_hypr_json(stdout: &[u8]) -> Option<TargetWindow> {
    if stdout.iter().all(|b| b.is_ascii_whitespace()) {
        return None;
    }
    let data: HyprWindow = serde_json::from_slice(stdout).ok()?;
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

pub fn uses_shift_insert(paste_keys: &str, terminal: bool) -> bool {
    paste_keys == "shift-insert" || (paste_keys == "auto" && terminal)
}

pub fn send_paste(target: Option<&TargetWindow>, paste_keys: &str) {
    let use_shift = uses_shift_insert(paste_keys, target.map(|t| t.is_terminal()).unwrap_or(false));
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

#[cfg(test)]
mod tests {
    use super::*;

    fn window(class: &str, tags: &[&str]) -> TargetWindow {
        TargetWindow {
            address: "0x1".into(),
            wm_class: class.into(),
            title: String::new(),
            tags: tags.iter().map(|s| (*s).to_string()).collect(),
        }
    }

    #[test]
    fn terminals_by_class() {
        for class in [
            "com.mitchellh.ghostty",
            "kitty",
            "Alacritty",
            "foot",
            "org.wezfurlong.wezterm",
            "rio",
        ] {
            assert!(window(class, &[]).is_terminal(), "{class}");
        }
        assert!(!window("firefox", &[]).is_terminal());
        assert!(!window("google-chrome", &[]).is_terminal());
    }

    #[test]
    fn terminals_by_hypr_tag() {
        assert!(window("something", &["terminal"]).is_terminal());
        assert!(window("something", &["terminal*"]).is_terminal());
        assert!(!window("something", &["browser"]).is_terminal());
    }

    #[test]
    fn paste_key_choice() {
        assert!(uses_shift_insert("shift-insert", false));
        assert!(uses_shift_insert("shift-insert", true));
        assert!(!uses_shift_insert("ctrl-v", true));
        assert!(!uses_shift_insert("ctrl-v", false));
        assert!(uses_shift_insert("auto", true));
        assert!(!uses_shift_insert("auto", false));
        assert!(!uses_shift_insert("laser", true));
    }

    #[test]
    fn hypr_json_window() {
        let json = br#"{"address":"0xabc","class":"kitty","title":"vim","tags":["terminal"]}"#;
        let win = window_from_hypr_json(json).unwrap();
        assert_eq!(win.address, "0xabc");
        assert_eq!(win.wm_class, "kitty");
        assert_eq!(win.title, "vim");
        assert!(win.is_terminal());

        let sparse = br#"{"address":"0x1"}"#;
        let win = window_from_hypr_json(sparse).unwrap();
        assert_eq!(win.wm_class, "");
        assert!(win.tags.is_empty());
        assert!(!win.is_terminal());
    }

    #[test]
    fn hypr_json_rejects_empty_or_invalid() {
        assert!(window_from_hypr_json(b"").is_none());
        assert!(window_from_hypr_json(b"   \n").is_none());
        assert!(window_from_hypr_json(b"not-json").is_none());
        assert!(window_from_hypr_json(br#"{"address":"","class":"kitty"}"#).is_none());
        assert!(window_from_hypr_json(br#"{"class":"kitty"}"#).is_none());
    }
}
