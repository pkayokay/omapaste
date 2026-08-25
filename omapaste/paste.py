from __future__ import annotations

import json
import logging
import subprocess
import time
from dataclasses import dataclass
from pathlib import Path

log = logging.getLogger("omapaste")


@dataclass(frozen=True)
class TargetWindow:
    address: str
    wm_class: str
    title: str
    tags: tuple[str, ...]

    @property
    def is_terminal(self) -> bool:
        for tag in self.tags:
            if tag.rstrip("*") == "terminal":
                return True
        lowered = self.wm_class.lower()
        return any(
            name in lowered
            for name in ("ghostty", "kitty", "alacritty", "foot", "wezterm", "rio")
        )


def _run(
    argv: list[str],
    input_bytes: bytes | None = None,
    timeout: float | None = None,
) -> subprocess.CompletedProcess[bytes]:
    return subprocess.run(
        argv,
        input=input_bytes,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
        timeout=timeout,
    )


def current_window() -> TargetWindow | None:
    result = _run(["hyprctl", "activewindow", "-j"])
    if result.returncode != 0 or not result.stdout.strip():
        return None
    try:
        data = json.loads(result.stdout.decode())
    except json.JSONDecodeError:
        return None
    address = str(data.get("address") or "")
    if not address:
        return None
    tags = tuple(str(tag) for tag in data.get("tags") or [])
    return TargetWindow(
        address=address,
        wm_class=str(data.get("class") or ""),
        title=str(data.get("title") or ""),
        tags=tags,
    )


def focus_window(target: TargetWindow) -> None:
    lua = f'hl.dispatch(hl.dsp.focus({{ window = "address:{target.address}" }}))'
    result = _run(["hyprctl", "eval", lua])
    if result.returncode != 0:
        log.debug("hyprctl focus failed: %s", result.stderr.decode().strip())


def copy_text(text: str) -> None:
    if _gdk_copy_text(text):
        return
    _wl_copy(["wl-copy"], text.encode(), "text")


def copy_image(path: Path, mime: str) -> None:
    data = path.read_bytes()
    if _gdk_copy_bytes(data, mime):
        return
    _wl_copy(["wl-copy", "--type", mime], data, "image")


def _gdk_copy_text(text: str) -> bool:
    try:
        from gi.repository import Gdk
    except Exception:
        return False
    display = Gdk.Display.get_default()
    if display is None:
        return False
    try:
        display.get_clipboard().set(text)
        return True
    except Exception:
        log.exception("GTK clipboard text copy failed")
        return False


def _gdk_copy_bytes(payload: bytes, mime: str) -> bool:
    try:
        from gi.repository import Gdk, GLib
    except Exception:
        return False
    display = Gdk.Display.get_default()
    if display is None:
        return False
    try:
        provider = Gdk.ContentProvider.new_for_bytes(mime, GLib.Bytes.new(payload))
        display.get_clipboard().set_content(provider)
        return True
    except Exception:
        log.exception("GTK clipboard bytes copy failed")
        return False


def _wl_copy(argv: list[str], payload: bytes, kind: str) -> None:
    # Nested wl-copy under gtk4-layer-shell exclusive keyboard never gets a
    # seat serial and blocks the GTK main loop until killed.
    try:
        result = _run(argv, input_bytes=payload, timeout=1.0)
    except subprocess.TimeoutExpired:
        log.warning("wl-copy %s timed out", kind)
        return
    if result.returncode != 0:
        log.warning("wl-copy %s failed: %s", kind, result.stderr.decode().strip())


def send_paste(target: TargetWindow | None, paste_keys: str) -> None:
    use_shift_insert = paste_keys == "shift-insert"
    if paste_keys == "auto":
        use_shift_insert = bool(target and target.is_terminal)

    if use_shift_insert:
        argv = ["wtype", "-M", "shift", "-k", "Insert", "-m", "shift"]
    else:
        argv = ["wtype", "-M", "ctrl", "-k", "v", "-m", "ctrl"]

    result = _run(argv)
    if result.returncode != 0:
        log.warning("wtype paste failed: %s", result.stderr.decode().strip())


def paste_now(target: TargetWindow | None, paste_keys: str, delay: float = 0.15) -> None:
    if target:
        focus_window(target)
    time.sleep(delay)
    send_paste(target, paste_keys)
