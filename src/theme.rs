use std::collections::HashMap;
use std::fs;

use crate::paths::{omarchy_theme_dir, omarchy_theme_name_path};

const FALLBACK: &[(&str, &str)] = &[
    ("mode", "dark"),
    ("accent", "#7aa2f7"),
    ("selection", "#292e42"),
    ("muted", "#414868"),
    ("background", "#1a1b26"),
    ("dark_background", "#13141c"),
    ("lighter_background", "#24283b"),
    ("foreground", "#c0caf5"),
    ("dark_foreground", "#565f89"),
    ("light_foreground", "#b4bee6"),
    ("bright_foreground", "#c0caf5"),
];

#[derive(Clone, Debug)]
pub struct Theme {
    pub name: String,
    pub colors: HashMap<String, String>,
}

impl Theme {
    pub fn get(&self, key: &str) -> String {
        self.colors
            .get(key)
            .cloned()
            .or_else(|| {
                FALLBACK
                    .iter()
                    .find(|(k, _)| *k == key)
                    .map(|(_, v)| (*v).to_string())
            })
            .unwrap_or_else(|| "#ffffff".into())
    }
}

pub fn load_theme() -> Theme {
    Theme {
        name: read_name(),
        colors: read_colors(),
    }
}

fn read_name() -> String {
    fs::read_to_string(omarchy_theme_name_path())
        .ok()
        .map(|s| parse_theme_name(&s))
        .unwrap_or_else(|| "unknown".into())
}

fn parse_theme_name(raw: &str) -> String {
    let name = raw.trim();
    if name.is_empty() {
        "unknown".into()
    } else {
        name.to_string()
    }
}

fn fallback_colors() -> HashMap<String, String> {
    FALLBACK
        .iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect()
}

fn merge_colors_toml(colors: &mut HashMap<String, String>, text: &str) {
    let Ok(data) = text.parse::<toml::Table>() else {
        return;
    };
    for (key, value) in data {
        if let Some(s) = value.as_str() {
            if s.starts_with('#') || key == "mode" {
                colors.insert(key, s.to_string());
            }
        }
    }
}

fn read_colors() -> HashMap<String, String> {
    let mut colors = fallback_colors();
    let path = omarchy_theme_dir().join("colors.toml");
    let Ok(text) = fs::read_to_string(path) else {
        return colors;
    };
    merge_colors_toml(&mut colors, &text);
    colors
}

pub fn css_for(theme: &Theme) -> String {
    let bg = theme.get("background");
    let bg2 = theme.get("lighter_background");
    let fg = theme.get("bright_foreground");
    let accent = theme.get("accent");
    format!(
        r#"
window.omapaste {{
  background-color: transparent;
}}

.op-bar {{
  background-color: alpha({bg}, 0.96);
  color: {fg};
  border-radius: 0;
  border: 1px solid alpha({fg}, 0.10);
  padding: 12px 16px 12px 16px;
  min-height: 292px;
}}

.op-title {{
  font-weight: 700;
  font-size: 13px;
  letter-spacing: 0.4px;
  color: {fg};
}}

.op-icon-btn {{
  padding: 2px 6px;
  min-width: 28px;
  min-height: 28px;
  border-radius: 0;
  background-color: transparent;
  color: alpha({fg}, 0.80);
}}

.op-icon-btn:hover {{
  background-color: alpha({fg}, 0.08);
  color: {fg};
}}

.op-count, .op-hint {{
  color: alpha({fg}, 0.70);
  font-size: 11px;
}}

.op-shortcuts {{
  padding: 10px 14px;
  min-width: 200px;
}}

popover, popover.background, popover contents {{
  border-radius: 0;
}}

.op-shortcut-key {{
  font-family: "JetBrainsMono Nerd Font", "JetBrains Mono", monospace;
  font-size: 11px;
  color: alpha({fg}, 0.70);
}}

.op-shortcut-action {{
  font-size: 12px;
  color: {fg};
}}

.op-issues {{
  font-size: 11px;
  color: alpha({fg}, 0.70);
  padding-top: 8px;
}}

.op-search {{
  background-color: {bg2};
  color: {fg};
  border-radius: 0;
  min-height: 28px;
  padding: 0 8px;
  border: 1px solid alpha({fg}, 0.08);
  font-size: 13px;
}}

.op-search text {{
  min-height: 0;
  padding: 0;
}}

.op-search:focus {{
  border-color: {accent};
}}

.op-search placeholder {{
  color: alpha({fg}, 0.45);
}}

.op-card {{
  background-color: {bg2};
  color: {fg};
  border-radius: 0;
  border: 1px solid alpha({fg}, 0.08);
  padding: 0;
}}

.op-card:hover {{
  border-color: alpha({accent}, 0.55);
}}

.op-card.selected {{
  background-color: alpha({accent}, 0.20);
  border: 1px solid {accent};
}}

.op-card.selected .op-card-header {{
  background-color: alpha({fg}, 0.10);
  border-bottom-color: alpha({fg}, 0.18);
}}

.op-card.selected .op-meta,
.op-card.selected .op-chars {{
  color: alpha({fg}, 0.92);
}}

.op-card-header {{
  background-color: alpha({fg}, 0.07);
  border-bottom: 1px solid alpha({fg}, 0.12);
  padding: 10px 12px 8px 12px;
}}

.op-card-body {{
  padding: 10px 12px 6px 12px;
}}

.op-card-footer {{
  padding: 4px 12px 8px 12px;
}}

.op-kind {{
  font-weight: 600;
  font-size: 11px;
  letter-spacing: 0.3px;
  color: {fg};
  min-height: 20px;
}}

.op-preview {{
  color: {fg};
  font-family: "JetBrainsMono Nerd Font", "JetBrains Mono", monospace;
  font-size: 12px;
  padding-left: 2px;
  padding-right: 2px;
  font-feature-settings: "liga" 0, "calt" 0;
}}

.op-meta, .op-chars {{
  color: alpha({fg}, 0.70);
  font-size: 11px;
}}

.op-empty {{
  color: alpha({fg}, 0.70);
  font-size: 13px;
}}

.op-header {{
  padding: 0 4px;
  min-height: 28px;
}}
"#
    )
}

pub fn watch_paths() -> Vec<std::path::PathBuf> {
    vec![
        omarchy_theme_dir().join("colors.toml"),
        omarchy_theme_name_path(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn theme_with(pairs: &[(&str, &str)]) -> Theme {
        let mut colors = fallback_colors();
        for (k, v) in pairs {
            colors.insert((*k).into(), (*v).into());
        }
        Theme {
            name: "test".into(),
            colors,
        }
    }

    #[test]
    fn parse_theme_name_trims_and_defaults() {
        assert_eq!(parse_theme_name("Last Horizon\n"), "Last Horizon");
        assert_eq!(parse_theme_name("   "), "unknown");
        assert_eq!(parse_theme_name(""), "unknown");
    }

    #[test]
    fn merge_keeps_hex_and_mode_only() {
        let mut colors = fallback_colors();
        merge_colors_toml(
            &mut colors,
            r##"
accent = "#ff0000"
mode = "light"
foreground = "red"
count = 3
"##,
        );
        assert_eq!(colors.get("accent").unwrap(), "#ff0000");
        assert_eq!(colors.get("mode").unwrap(), "light");
        assert_eq!(colors.get("foreground").unwrap(), "#c0caf5");
    }

    #[test]
    fn invalid_toml_leaves_fallbacks() {
        let mut colors = fallback_colors();
        merge_colors_toml(&mut colors, "???");
        assert_eq!(colors.get("accent").unwrap(), "#7aa2f7");
    }

    #[test]
    fn get_falls_back_then_white() {
        let theme = Theme {
            name: "x".into(),
            colors: HashMap::new(),
        };
        assert_eq!(theme.get("accent"), "#7aa2f7");
        assert_eq!(theme.get("no-such-color"), "#ffffff");
    }

    #[test]
    fn css_uses_theme_colors_and_square_chrome() {
        let theme = theme_with(&[
            ("background", "#111111"),
            ("lighter_background", "#222222"),
            ("bright_foreground", "#eeeeee"),
            ("accent", "#abcdef"),
        ]);
        let css = css_for(&theme);
        assert!(css.contains("#111111"));
        assert!(css.contains("#222222"));
        assert!(css.contains("#eeeeee"));
        assert!(css.contains("#abcdef"));
        assert!(css.contains("border-radius: 0"));
        assert!(!css.contains("overflow"));
        assert!(css.contains(".op-search text"));
        assert!(css.contains("min-height: 28px"));
        assert!(!css.contains("line-height"));
        assert!(css.contains("min-height: 20px"));
        assert!(css.contains("padding: 10px 12px 6px 12px"));
        assert!(css.contains("font-feature-settings"));
    }

    #[test]
    fn watch_paths_follow_home() {
        let _lock = crate::env_lock();
        let dir = tempfile::TempDir::new().unwrap();
        let old_home = std::env::var("HOME").ok();
        std::env::set_var("HOME", dir.path());
        let paths = watch_paths();
        assert!(paths[0].ends_with("colors.toml"));
        assert!(paths[1].ends_with("theme.name"));
        assert!(paths[0].starts_with(dir.path()));
        match old_home {
            Some(h) => std::env::set_var("HOME", h),
            None => std::env::remove_var("HOME"),
        }
    }

    #[test]
    fn load_theme_reads_omarchy_files() {
        let _lock = crate::env_lock();
        let dir = tempfile::TempDir::new().unwrap();
        let old_home = std::env::var("HOME").ok();
        std::env::set_var("HOME", dir.path());
        let theme_dir = dir.path().join(".local/state/omarchy/current/theme");
        std::fs::create_dir_all(&theme_dir).unwrap();
        std::fs::write(
            dir.path().join(".local/state/omarchy/current/theme.name"),
            " Last Horizon \n",
        )
        .unwrap();
        std::fs::write(theme_dir.join("colors.toml"), "accent = \"#010203\"\n").unwrap();
        let theme = load_theme();
        assert_eq!(theme.name, "Last Horizon");
        assert_eq!(theme.get("accent"), "#010203");
        assert_eq!(theme.get("background"), "#1a1b26");
        match old_home {
            Some(h) => std::env::set_var("HOME", h),
            None => std::env::remove_var("HOME"),
        }
    }
}
