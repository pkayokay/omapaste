from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path

from omapaste.paths import config_path
from omapaste.store import KEEP_PRESETS, DEFAULT_KEEP

try:
    import tomllib
except ModuleNotFoundError:  # pragma: no cover
    import tomli as tomllib  # type: ignore


DEFAULT_CONFIG = f"""# Omapaste — https://github.com/pkayokay/omapaste

# How long new clips are kept unless you change a clip individually.
# One of: 1h, 1d, 7d, forever
default_keep = "{DEFAULT_KEEP}"

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
"""


@dataclass(frozen=True)
class Config:
    default_keep: str = DEFAULT_KEEP
    max_items: int = 200
    max_bytes: int = 8_000_000
    ignore_secrets: bool = True
    paste_keys: str = "auto"

    def keep_seconds(self) -> int | None:
        for preset in KEEP_PRESETS:
            if preset.key == self.default_keep:
                return preset.seconds
        return KEEP_PRESETS[1].seconds  # 1d


def load_config(path: Path | None = None) -> Config:
    target = path or config_path()
    if not target.exists():
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(DEFAULT_CONFIG, encoding="utf-8")
        return Config()

    data = tomllib.loads(target.read_text(encoding="utf-8"))
    keep = str(data.get("default_keep", DEFAULT_KEEP)).strip()
    if keep not in {p.key for p in KEEP_PRESETS}:
        keep = DEFAULT_KEEP

    paste_keys = str(data.get("paste_keys", "auto")).strip()
    if paste_keys not in {"auto", "shift-insert", "ctrl-v"}:
        paste_keys = "auto"

    max_items = int(data.get("max_items", 200))
    max_bytes = int(data.get("max_bytes", 8_000_000))
    return Config(
        default_keep=keep,
        max_items=max(1, max_items),
        max_bytes=max(1024, max_bytes),
        ignore_secrets=bool(data.get("ignore_secrets", True)),
        paste_keys=paste_keys,
    )
