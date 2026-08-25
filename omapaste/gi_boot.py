"""Load gtk4-layer-shell before GTK/libwayland so the Python GI bindings work.

See https://github.com/wmww/gtk4-layer-shell/blob/main/linking.md
"""

from ctypes import CDLL, util

_lib = util.find_library("gtk4-layer-shell") or "libgtk4-layer-shell.so"
CDLL(_lib)

import gi

gi.require_version("Gtk4LayerShell", "1.0")
gi.require_version("Gtk", "4.0")
gi.require_version("Gdk", "4.0")
gi.require_version("GdkPixbuf", "2.0")
