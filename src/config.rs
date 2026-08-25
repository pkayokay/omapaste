use std::fs;
use std::path::Path;

use crate::store::{keep_by_key, KeepPreset, DEFAULT_KEEP, KEEP_PRESETS};

pub const DEFAULT_CONFIG: &str = r#"# Omapaste — https://github.com/pkayokay/omapaste

# How long new clips are kept unless you change a clip individually.
# One of: 1h, 1d, 7d, forever
default_keep = "1d"

# Hard cap on stored clips. Forever clips are pruned last.
max_items = 200

# Skip clipboard items larger than this (bytes).
max_bytes = 8000000

# Skip password-manager / secret MIME types.
ignore_secrets = true

# Paste key after Enter:
#   auto           — Shift+Insert in terminals, Ctrl+V elsewhere
#   shift-insert   — always Shift+Insert (Omarchy's universal paste)
#   ctrl-v         — always Ctrl+V
paste_keys = "auto"
"#;

#[derive(Clone, Debug)]
pub struct Config {
    pub default_keep: String,
    pub max_items: i64,
    pub max_bytes: i64,
    pub ignore_secrets: bool,
    pub paste_keys: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            default_keep: DEFAULT_KEEP.into(),
            max_items: 200,
            max_bytes: 8_000_000,
            ignore_secrets: true,
            paste_keys: "auto".into(),
        }
    }
}

impl Config {
    pub fn keep_seconds(&self) -> Option<i64> {
        keep_by_key(&self.default_keep)
            .or(Some(KEEP_PRESETS[1]))
            .and_then(|p: KeepPreset| p.seconds)
    }
}

pub fn load_config(path: Option<&Path>) -> Config {
    let target = path
        .map(Path::to_path_buf)
        .unwrap_or_else(crate::paths::config_path);
    if !target.exists() {
        if let Some(parent) = target.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::write(&target, DEFAULT_CONFIG);
        return Config::default();
    }
    let Ok(text) = fs::read_to_string(&target) else {
        return Config::default();
    };
    let Ok(data) = text.parse::<toml::Table>() else {
        return Config::default();
    };
    let mut cfg = Config::default();
    if let Some(v) = data.get("default_keep").and_then(|v| v.as_str()) {
        if keep_by_key(v).is_some() {
            cfg.default_keep = v.to_string();
        }
    }
    if let Some(v) = data.get("paste_keys").and_then(|v| v.as_str()) {
        if matches!(v, "auto" | "shift-insert" | "ctrl-v") {
            cfg.paste_keys = v.to_string();
        }
    }
    if let Some(v) = data.get("max_items").and_then(|v| v.as_integer()) {
        cfg.max_items = v.max(1);
    }
    if let Some(v) = data.get("max_bytes").and_then(|v| v.as_integer()) {
        cfg.max_bytes = v.max(1024);
    }
    if let Some(v) = data.get("ignore_secrets").and_then(|v| v.as_bool()) {
        cfg.ignore_secrets = v;
    }
    cfg
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_writes_defaults() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("config.toml");
        let cfg = load_config(Some(&path));
        assert!(path.exists());
        assert_eq!(cfg.default_keep, "1d");
        assert_eq!(cfg.paste_keys, "auto");
    }

    #[test]
    fn invalid_keep_falls_back() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, "default_keep = \"nope\"\npaste_keys = \"laser\"\n").unwrap();
        let cfg = load_config(Some(&path));
        assert_eq!(cfg.default_keep, "1d");
        assert_eq!(cfg.paste_keys, "auto");
    }

    #[test]
    fn custom_values() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(
            &path,
            "default_keep = \"forever\"\nmax_items = 12\nignore_secrets = false\npaste_keys = \"shift-insert\"\n",
        )
        .unwrap();
        let cfg = load_config(Some(&path));
        assert_eq!(cfg.default_keep, "forever");
        assert_eq!(cfg.max_items, 12);
        assert!(!cfg.ignore_secrets);
        assert_eq!(cfg.paste_keys, "shift-insert");
        assert!(cfg.keep_seconds().is_none());
    }

    #[test]
    fn default_config_text_parses() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, DEFAULT_CONFIG).unwrap();
        let cfg = load_config(Some(&path));
        assert_eq!(cfg.default_keep, "1d");
        assert_eq!(cfg.max_items, 200);
        assert_eq!(cfg.max_bytes, 8_000_000);
        assert!(cfg.ignore_secrets);
        assert_eq!(cfg.paste_keys, "auto");
        assert_eq!(cfg.keep_seconds(), Some(86_400));
    }

    #[test]
    fn floors_and_paste_key_values() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(
            &path,
            "max_items = 0\nmax_bytes = 10\npaste_keys = \"ctrl-v\"\n",
        )
        .unwrap();
        let cfg = load_config(Some(&path));
        assert_eq!(cfg.max_items, 1);
        assert_eq!(cfg.max_bytes, 1024);
        assert_eq!(cfg.paste_keys, "ctrl-v");
    }

    #[test]
    fn broken_toml_uses_defaults() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, "[[[[").unwrap();
        let cfg = load_config(Some(&path));
        assert_eq!(cfg.default_keep, "1d");
        assert_eq!(cfg.paste_keys, "auto");
    }

    #[test]
    fn extra_keys_are_ignored() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, "toggle_key = \"SUPER + V\"\ndefault_keep = \"7d\"\n").unwrap();
        let cfg = load_config(Some(&path));
        assert_eq!(cfg.default_keep, "7d");
        assert_eq!(cfg.keep_seconds(), Some(86_400 * 7));
    }

    #[test]
    fn creates_parent_dirs_and_keep_seconds() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("nested/dir/config.toml");
        let cfg = load_config(Some(&path));
        assert!(path.exists());
        assert_eq!(cfg.keep_seconds(), Some(86_400));

        fs::write(&path, "default_keep = \"1h\"\nmax_items = \"12\"\n").unwrap();
        let cfg = load_config(Some(&path));
        assert_eq!(cfg.default_keep, "1h");
        assert_eq!(cfg.keep_seconds(), Some(3600));
        assert_eq!(cfg.max_items, 200);
    }
}
