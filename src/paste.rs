use std::io::Read;
use std::path::Path;
use std::process::{Command, Output, Stdio};
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

trait Proc {
    fn run(
        &self,
        argv: &[&str],
        input: Option<&[u8]>,
        timeout: Option<Duration>,
    ) -> std::io::Result<Output>;
}

struct RealProc;

impl Proc for RealProc {
    fn run(
        &self,
        argv: &[&str],
        input: Option<&[u8]>,
        timeout: Option<Duration>,
    ) -> std::io::Result<Output> {
        run(argv, input, timeout)
    }
}

fn run(argv: &[&str], input: Option<&[u8]>, timeout: Option<Duration>) -> std::io::Result<Output> {
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
    current_window_with(&RealProc)
}

fn current_window_with(proc: &impl Proc) -> Option<TargetWindow> {
    let out = proc
        .run(&["hyprctl", "activewindow", "-j"], None, None)
        .ok()?;
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
    focus_window_with(&RealProc, target);
}

fn focus_window_with(proc: &impl Proc, target: &TargetWindow) {
    let lua = format!(
        "hl.dispatch(hl.dsp.focus({{ window = \"address:{}\" }}))",
        target.address
    );
    let _ = proc.run(&["hyprctl", "eval", &lua], None, None);
}

pub fn copy_text(text: &str) {
    copy_text_with(&RealProc, text, true);
}

fn copy_text_with(proc: &impl Proc, text: &str, try_gdk: bool) {
    if try_gdk && gdk_copy_text(text) {
        return;
    }
    wl_copy_with(proc, &["wl-copy"], text.as_bytes());
}

pub fn copy_image(path: &Path, mime: &str) {
    copy_image_with(&RealProc, path, mime, true);
}

fn copy_image_with(proc: &impl Proc, path: &Path, mime: &str, try_gdk: bool) {
    let Ok(data) = std::fs::read(path) else {
        return;
    };
    if try_gdk && gdk_copy_bytes(&data, mime) {
        return;
    }
    wl_copy_with(proc, &["wl-copy", "--type", mime], &data);
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

fn wl_copy_with(proc: &impl Proc, argv: &[&str], payload: &[u8]) {
    match proc.run(argv, Some(payload), Some(Duration::from_secs(1))) {
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
    send_paste_with(&RealProc, target, paste_keys);
}

fn send_paste_with(proc: &impl Proc, target: Option<&TargetWindow>, paste_keys: &str) {
    let use_shift = uses_shift_insert(paste_keys, target.map(|t| t.is_terminal()).unwrap_or(false));
    let argv: &[&str] = if use_shift {
        &["wtype", "-M", "shift", "-k", "Insert", "-m", "shift"]
    } else {
        &["wtype", "-M", "ctrl", "-k", "v", "-m", "ctrl"]
    };
    if let Ok(out) = proc.run(argv, None, None) {
        if !out.status.success() {
            log::warn!(
                "wtype paste failed: {}",
                String::from_utf8_lossy(&out.stderr)
            );
        }
    }
}

pub fn paste_now(target: Option<&TargetWindow>, paste_keys: &str) {
    paste_now_with(&RealProc, target, paste_keys, Duration::from_millis(150));
}

fn paste_now_with(
    proc: &impl Proc,
    target: Option<&TargetWindow>,
    paste_keys: &str,
    delay: Duration,
) {
    if let Some(t) = target {
        focus_window_with(proc, t);
    }
    if !delay.is_zero() {
        thread::sleep(delay);
    }
    send_paste_with(proc, target, paste_keys);
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

    struct FakeProc {
        calls: std::sync::Mutex<Vec<Vec<String>>>,
        inputs: std::sync::Mutex<Vec<Option<Vec<u8>>>>,
        stdout: Vec<u8>,
        ok: bool,
    }

    impl FakeProc {
        fn ok(stdout: &[u8]) -> Self {
            Self {
                calls: std::sync::Mutex::new(Vec::new()),
                inputs: std::sync::Mutex::new(Vec::new()),
                stdout: stdout.to_vec(),
                ok: true,
            }
        }

        fn fail() -> Self {
            Self {
                calls: std::sync::Mutex::new(Vec::new()),
                inputs: std::sync::Mutex::new(Vec::new()),
                stdout: Vec::new(),
                ok: false,
            }
        }

        fn last(&self) -> Vec<String> {
            self.calls.lock().unwrap().last().cloned().unwrap()
        }
    }

    impl Proc for FakeProc {
        fn run(
            &self,
            argv: &[&str],
            input: Option<&[u8]>,
            _timeout: Option<Duration>,
        ) -> std::io::Result<Output> {
            self.calls
                .lock()
                .unwrap()
                .push(argv.iter().map(|s| (*s).to_string()).collect());
            self.inputs.lock().unwrap().push(input.map(|b| b.to_vec()));
            use std::os::unix::process::ExitStatusExt;
            Ok(Output {
                status: std::process::ExitStatus::from_raw(if self.ok { 0 } else { 0x100 }),
                stdout: self.stdout.clone(),
                stderr: Vec::new(),
            })
        }
    }

    #[test]
    fn current_window_uses_hyprctl_json() {
        let proc = FakeProc::ok(br#"{"address":"0xabc","class":"firefox","title":"x","tags":[]}"#);
        let win = current_window_with(&proc).unwrap();
        assert_eq!(win.address, "0xabc");
        assert_eq!(proc.last()[0], "hyprctl");
        assert!(current_window_with(&FakeProc::fail()).is_none());
    }

    #[test]
    fn send_paste_picks_wtype_keys() {
        let term = window("kitty", &[]);
        let proc = FakeProc::ok(b"");
        send_paste_with(&proc, Some(&term), "auto");
        assert_eq!(
            proc.last(),
            ["wtype", "-M", "shift", "-k", "Insert", "-m", "shift"]
        );

        let proc = FakeProc::ok(b"");
        send_paste_with(&proc, Some(&window("firefox", &[])), "auto");
        assert_eq!(
            proc.last(),
            ["wtype", "-M", "ctrl", "-k", "v", "-m", "ctrl"]
        );
    }

    #[test]
    fn paste_now_focuses_then_types_without_sleep() {
        let target = window("firefox", &[]);
        let proc = FakeProc::ok(b"");
        paste_now_with(&proc, Some(&target), "ctrl-v", Duration::ZERO);
        let calls = proc.calls.lock().unwrap().clone();
        assert_eq!(calls[0][0], "hyprctl");
        assert!(calls[0][2].contains("0x1"));
        assert_eq!(calls[1][0], "wtype");
    }

    #[test]
    fn copy_falls_back_to_wl_copy() {
        let proc = FakeProc::ok(b"");
        copy_text_with(&proc, "hello", false);
        assert_eq!(proc.last(), ["wl-copy"]);
        assert_eq!(
            proc.inputs.lock().unwrap().last().cloned().flatten(),
            Some(b"hello".to_vec())
        );

        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("clip.png");
        std::fs::write(&path, b"png").unwrap();
        let proc = FakeProc::ok(b"");
        copy_image_with(&proc, &path, "image/png", false);
        assert_eq!(proc.last(), ["wl-copy", "--type", "image/png"]);
        assert_eq!(
            proc.inputs.lock().unwrap().last().cloned().flatten(),
            Some(b"png".to_vec())
        );

        let proc = FakeProc::ok(b"");
        copy_image_with(&proc, &dir.path().join("missing.png"), "image/png", false);
        assert!(proc.calls.lock().unwrap().is_empty());
    }
}
