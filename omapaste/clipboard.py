from __future__ import annotations

import logging
import subprocess
import threading
from collections.abc import Callable
from pathlib import Path

import omapaste.gi_boot  # noqa: F401  — must load before GTK

from gi.repository import GLib

from omapaste.config import Config
from omapaste.store import Clip, Store, make_preview

log = logging.getLogger("omapaste")

SECRET_HINTS = (
    "x-kde-passwordmanagerhint",
    "x-nm-origin",
    "text/secret",
    "application/x-keepassxc",
)


class ClipboardWatcher:
    """Watch the Wayland clipboard with wl-paste --watch."""

    def __init__(
        self,
        store: Store,
        config: Config,
        images_dir: Path,
        on_change: Callable[[Clip | None], None] | None = None,
    ):
        self.store = store
        self.config = config
        self.images_dir = images_dir
        self.on_change = on_change
        self._procs: list[subprocess.Popen[bytes]] = []
        self._ignore_hash: str | None = None
        self._ignore_until: int = 0
        self._ignore_all_until: int = 0
        self._stopping = False

    def start(self) -> None:
        self._watch("text", ["wl-paste", "--type", "text", "--watch", "echo"])
        self._watch("image/png", ["wl-paste", "--type", "image/png", "--watch", "echo"])

    def stop(self) -> None:
        self._stopping = True
        for proc in self._procs:
            proc.terminate()
            try:
                proc.wait(timeout=1)
            except subprocess.TimeoutExpired:
                proc.kill()
        self._procs.clear()

    def ignore_hash(self, digest: str, seconds: float = 1.5) -> None:
        now = GLib.get_monotonic_time()
        hold = int(seconds * 1_000_000)
        self._ignore_hash = digest
        self._ignore_until = now + hold
        # Skip the following capture entirely. Reading via wl-paste on the
        # GTK main thread deadlocks once this process owns the clipboard.
        self._ignore_all_until = now + hold

    def _watch(self, label: str, argv: list[str]) -> None:
        try:
            proc = subprocess.Popen(
                argv,
                stdout=subprocess.PIPE,
                stderr=subprocess.DEVNULL,
            )
        except FileNotFoundError:
            log.error("wl-paste is not installed")
            return
        if proc.stdout is None:
            return
        self._procs.append(proc)
        thread = threading.Thread(
            target=self._read_loop,
            args=(proc, label),
            name=f"omapaste-watch-{label}",
            daemon=True,
        )
        thread.start()

    def _read_loop(self, proc: subprocess.Popen[bytes], label: str) -> None:
        assert proc.stdout is not None
        for _line in proc.stdout:
            if self._stopping:
                return
            GLib.idle_add(self._capture_on_main, label)

    def _capture_on_main(self, label: str) -> bool:
        if self._stopping:
            return False
        if GLib.get_monotonic_time() < self._ignore_all_until:
            return False
        try:
            clip = self._capture(label)
        except Exception:
            log.exception("failed to capture clipboard (%s)", label)
            return False
        if clip and self.on_change:
            self.on_change(clip)
        return False

    def _capture(self, label: str) -> Clip | None:
        types = _list_types()
        if self.config.ignore_secrets and _looks_secret(types):
            log.debug("skipping secret clipboard item")
            return None

        if label.startswith("image"):
            mime = _first_image_mime(types) or "image/png"
            payload = _paste_bytes(["wl-paste", "--type", mime, "--no-newline"])
            if not payload:
                payload = _paste_bytes(["wl-paste", "--type", "image/png", "--no-newline"])
                mime = "image/png"
            if not payload or len(payload) > self.config.max_bytes:
                return None
            from omapaste.store import content_hash

            digest = content_hash("image", mime, payload)
            if self._should_ignore(digest):
                return None
            image_path = self.images_dir / f"{digest}.bin"
            if not image_path.exists():
                image_path.write_bytes(payload)
            clip = self.store.add(
                kind="image",
                mime=mime,
                payload=payload,
                text=None,
                preview="Image",
                image_path=str(image_path),
                keep_preset=self.config.default_keep,
                max_items=self.config.max_items,
            )
            return clip

        payload = _paste_bytes(["wl-paste", "--type", "text", "--no-newline"])
        if not payload or len(payload) > self.config.max_bytes:
            return None
        try:
            text = payload.decode("utf-8")
        except UnicodeDecodeError:
            text = payload.decode("utf-8", errors="replace")
        if not text.strip():
            return None
        from omapaste.store import content_hash

        digest = content_hash("text", "text/plain", payload)
        if self._should_ignore(digest):
            return None
        return self.store.add(
            kind="text",
            mime="text/plain",
            payload=payload,
            text=text,
            preview=make_preview(text),
            image_path=None,
            keep_preset=self.config.default_keep,
            max_items=self.config.max_items,
        )

    def _should_ignore(self, digest: str) -> bool:
        now = GLib.get_monotonic_time()
        return self._ignore_hash == digest and now < self._ignore_until


def _list_types() -> list[str]:
    result = subprocess.run(
        ["wl-paste", "--list-types"],
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        check=False,
    )
    if result.returncode != 0:
        return []
    return [line.strip() for line in result.stdout.decode(errors="replace").splitlines() if line.strip()]


def _looks_secret(types: list[str]) -> bool:
    lowered = [item.casefold() for item in types]
    return any(hint in item for item in lowered for hint in SECRET_HINTS)


def _first_image_mime(types: list[str]) -> str | None:
    for item in types:
        if item.startswith("image/"):
            return item
    return None


def _paste_bytes(argv: list[str]) -> bytes:
    result = subprocess.run(
        argv,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        check=False,
    )
    if result.returncode != 0:
        return b""
    return result.stdout
