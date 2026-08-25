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
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".into())
}

fn read_colors() -> HashMap<String, String> {
    let mut colors: HashMap<String, String> = FALLBACK
        .iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect();
    let path = omarchy_theme_dir().join("colors.toml");
    let Ok(text) = fs::read_to_string(path) else {
        return colors;
    };
    let Ok(data) = text.parse::<toml::Table>() else {
        return colors;
    };
    for (key, value) in data {
        if let Some(s) = value.as_str() {
            if s.starts_with('#') || key == "mode" {
                colors.insert(key, s.to_string());
            }
        }
    }
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
  border-radius: 18px;
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
  border-radius: 8px;
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

.op-shortcut-key {{
  font-family: "JetBrainsMono Nerd Font", "JetBrains Mono", monospace;
  font-size: 11px;
  color: alpha({fg}, 0.70);
}}

.op-shortcut-action {{
  font-size: 12px;
  color: {fg};
}}

.op-search {{
  background-color: {bg2};
  color: {fg};
  border-radius: 10px;
  padding: 4px 10px;
  border: 1px solid alpha({fg}, 0.08);
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
  border-radius: 14px;
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
  padding: 8px 10px 7px 10px;
}}

.op-card-body {{
  padding: 8px 10px 4px 10px;
}}

.op-card-footer {{
  padding: 4px 10px 8px 10px;
}}

.op-kind {{
  font-weight: 600;
  font-size: 11px;
  letter-spacing: 0.3px;
  color: {fg};
}}

.op-preview {{
  color: {fg};
  font-family: "JetBrainsMono Nerd Font", "JetBrains Mono", monospace;
  font-size: 12px;
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
