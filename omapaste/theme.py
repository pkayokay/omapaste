from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path

from omapaste.paths import omarchy_theme_dir, omarchy_theme_name_path

try:
    import tomllib
except ModuleNotFoundError:  # pragma: no cover
    import tomli as tomllib  # type: ignore


FALLBACK = {
    "mode": "dark",
    "accent": "#7aa2f7",
    "selection": "#292e42",
    "muted": "#414868",
    "background": "#1a1b26",
    "dark_background": "#13141c",
    "lighter_background": "#24283b",
    "foreground": "#c0caf5",
    "dark_foreground": "#565f89",
    "light_foreground": "#b4bee6",
    "bright_foreground": "#c0caf5",
}


@dataclass(frozen=True)
class Theme:
    name: str
    colors: dict[str, str]

    def get(self, key: str, default: str | None = None) -> str:
        if key in self.colors:
            return self.colors[key]
        if default is not None:
            return default
        return FALLBACK.get(key, "#ffffff")


def _read_name() -> str:
    path = omarchy_theme_name_path()
    if path.exists():
        return path.read_text(encoding="utf-8").strip() or "unknown"
    return "unknown"


def _read_colors(theme_dir: Path) -> dict[str, str]:
    colors_path = theme_dir / "colors.toml"
    colors = dict(FALLBACK)
    if not colors_path.exists():
        return colors
    data = tomllib.loads(colors_path.read_text(encoding="utf-8"))
    for key, value in data.items():
        if isinstance(value, str) and value.startswith("#"):
            colors[key] = value
        elif key == "mode" and isinstance(value, str):
            colors[key] = value
    return colors


def load_theme() -> Theme:
    return Theme(name=_read_name(), colors=_read_colors(omarchy_theme_dir()))


def css_for(theme: Theme) -> str:
    bg = theme.get("background")
    bg2 = theme.get("lighter_background", theme.get("dark_background", bg))
    fg = theme.get("bright_foreground", theme.get("foreground"))
    muted = theme.get("dark_foreground", theme.get("muted"))
    accent = theme.get("accent")
    return f"""
window.omapaste {{
  background-color: transparent;
}}

.op-scrim {{
  background-color: alpha({bg}, 0.18);
}}

.op-bar {{
  background-color: alpha({bg}, 0.96);
  color: {fg};
  border-radius: 18px;
  border: 1px solid alpha({fg}, 0.10);
  padding: 12px 16px 10px 16px;
  min-height: 330px;
}}

.op-title {{
  font-weight: 700;
  font-size: 13px;
  letter-spacing: 0.4px;
  color: {fg};
}}

.op-count, .op-hint {{
  color: {muted};
  font-size: 11px;
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

.op-keep {{
  background-color: alpha({accent}, 0.16);
  color: {fg};
  border-radius: 999px;
  padding: 1px 8px;
  font-size: 11px;
  border: 1px solid alpha({accent}, 0.35);
}}

.op-keep-menu {{
  background-color: {bg};
  color: {fg};
  padding: 6px;
  border-radius: 10px;
}}

.op-empty {{
  color: {muted};
  font-size: 13px;
}}

.op-header, .op-footer {{
  padding: 0 4px;
}}
"""


def watch_paths() -> list[Path]:
    return [omarchy_theme_dir() / "colors.toml", omarchy_theme_name_path()]
