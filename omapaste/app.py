from __future__ import annotations

import logging
import sys
from pathlib import Path

import omapaste.gi_boot  # noqa: F401  — must load before GTK

from gi.repository import Gio, GLib, Gtk

from omapaste import __version__
from omapaste.clipboard import ClipboardWatcher
from omapaste.config import Config, load_config
from omapaste.paste import current_window
from omapaste.paths import APP_ID, db_path, images_dir, omarchy_theme_dir, omarchy_theme_name_path
from omapaste.store import Store
from omapaste.ui import Overlay

log = logging.getLogger("omapaste")


class OmapasteApp(Gtk.Application):
    def __init__(self, command: str = "toggle"):
        super().__init__(
            application_id=APP_ID,
            flags=Gio.ApplicationFlags.HANDLES_COMMAND_LINE,
        )
        self.command = command
        self.config: Config | None = None
        self.store: Store | None = None
        self.overlay: Overlay | None = None
        self.watcher: ClipboardWatcher | None = None
        self._theme_monitors: list[Gio.FileMonitor] = []

    def do_startup(self) -> None:
        Gtk.Application.do_startup(self)
        self.hold()
        self.config = load_config()
        self.store = Store(db_path())
        removed = self.store.prune(self.config.max_items)
        _unlink(removed)
        self.overlay = Overlay(
            self,
            self.store,
            self.config,
            on_copy=self._ignore_own_copy,
        )
        self.watcher = ClipboardWatcher(
            self.store,
            self.config,
            images_dir(),
            on_change=self._on_clipboard,
        )
        self.watcher.start()
        self._watch_theme()
        GLib.timeout_add_seconds(60, self._prune)

    def do_shutdown(self) -> None:
        if self.watcher:
            self.watcher.stop()
        if self.store:
            self.store.close()
        Gtk.Application.do_shutdown(self)

    def do_command_line(self, command_line: Gio.ApplicationCommandLine) -> int:
        argv = list(command_line.get_arguments()[1:])
        command = argv[0] if argv else self.command
        self._handle(command)
        return 0

    def do_activate(self) -> None:
        pass

    def _handle(self, command: str) -> bool:
        if self.overlay is None:
            return False
        if command in {"daemon", "start"}:
            log.info("omapaste %s daemon ready", __version__)
        elif command in {"toggle", ""}:
            self._toggle()
        elif command == "show":
            self.overlay.show(current_window())
        elif command == "hide":
            self.overlay.hide()
        elif command in {"quit", "stop"}:
            self.quit()
        else:
            log.warning("unknown command: %s", command)
        return False

    def _toggle(self) -> None:
        assert self.overlay is not None
        if self.overlay.is_open():
            self.overlay.hide()
        else:
            self.overlay.show(current_window())

    def _on_clipboard(self, _clip) -> None:
        if self.overlay and self.overlay.is_open():
            self.overlay.refresh(keep_selection=True)

    def _ignore_own_copy(self, digest: str) -> None:
        if self.watcher:
            self.watcher.ignore_hash(digest)

    def _prune(self) -> bool:
        if self.store and self.config:
            removed = self.store.prune(self.config.max_items)
            _unlink(removed)
            if self.overlay and self.overlay.is_open():
                self.overlay.refresh(keep_selection=True)
        return True

    def _watch_theme(self) -> None:
        for path in (omarchy_theme_dir() / "colors.toml", omarchy_theme_name_path()):
            if not path.exists():
                continue
            gfile = Gio.File.new_for_path(str(path))
            monitor = gfile.monitor_file(Gio.FileMonitorFlags.NONE, None)
            monitor.connect("changed", self._on_theme_file)
            self._theme_monitors.append(monitor)

    def _on_theme_file(self, *_args) -> None:
        if self.overlay:
            self.overlay.reload_theme()


def _unlink(paths: list[Path]) -> None:
    for path in paths:
        try:
            path.unlink(missing_ok=True)
        except OSError:
            pass


def run(command: str) -> int:
    logging.basicConfig(
        level=logging.INFO,
        format="%(levelname)s %(name)s: %(message)s",
        stream=sys.stderr,
        force=True,
    )
    app = OmapasteApp(command=command)
    return app.run(sys.argv)
