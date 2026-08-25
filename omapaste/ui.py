from __future__ import annotations

import logging
import time
from collections.abc import Callable
from pathlib import Path

import omapaste.gi_boot  # noqa: F401  — must load before GTK

from gi.repository import Gdk, GdkPixbuf, GLib, Gtk, Pango

try:
    from gi.repository import Gtk4LayerShell as LayerShell

    LAYER_SHELL = True
except (ImportError, ValueError):
    LayerShell = None
    LAYER_SHELL = False

from omapaste.config import Config
from omapaste.paste import TargetWindow, copy_image, copy_text, paste_now
from omapaste.store import Clip, Store, content_hash, next_keep
from omapaste.theme import Theme, css_for, load_theme

log = logging.getLogger("omapaste")

BAR_HEIGHT = 304
CARD_WIDTH = 210
CARD_HEIGHT = 236
VISIBLE_MARGIN = 14
SLIDE_PX = BAR_HEIGHT + 24
ANIM_DURATION_MS = 220


def _output_width() -> int:
    display = Gdk.Display.get_default()
    if display is None:
        return 1400
    monitors = display.get_monitors()
    if monitors.get_n_items() == 0:
        return 1400
    monitor = monitors.get_item(0)
    geometry = monitor.get_geometry()
    return max(800, int(geometry.width) - 36)


def kind_label(clip: Clip) -> str:
    if clip.kind == "image":
        return "Image"
    if clip.kind == "text":
        return "Text"
    return clip.kind.replace("_", " ").title()


def format_chars(clip: Clip) -> str:
    if clip.kind == "image":
        size = clip.byte_size
        if size < 1024:
            return f"{size} B"
        if size < 1024 * 1024:
            kb = size / 1024
            return f"{kb:.1f} KB" if kb < 10 else f"{kb:.0f} KB"
        return f"{size / (1024 * 1024):.1f} MB"
    n = len(clip.text or "")
    if n == 1:
        return "1 char"
    return f"{n:,} chars"


def format_age(ts: int, now: int | None = None) -> str:
    stamp = now if now is not None else int(time.time())
    delta = max(0, stamp - ts)
    if delta < 12:
        return "just now"
    if delta < 60:
        return f"{delta}s ago"
    if delta < 3600:
        return f"{delta // 60}m ago"
    if delta < 86400:
        return f"{delta // 3600}h ago"
    days = delta // 86400
    if days == 1:
        return "yesterday"
    if days < 7:
        return f"{days}d ago"
    return time.strftime("%b %-d", time.localtime(ts))


class ClipCard(Gtk.Box):
    def __init__(self, clip: Clip):
        super().__init__(orientation=Gtk.Orientation.VERTICAL, spacing=0)
        self.clip = clip
        self.add_css_class("op-card")
        self.set_size_request(CARD_WIDTH, CARD_HEIGHT)
        self.set_hexpand(False)
        self.set_vexpand(False)
        self.set_halign(Gtk.Align.START)
        self.set_valign(Gtk.Align.CENTER)
        self.set_overflow(Gtk.Overflow.HIDDEN)

        header = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=2)
        header.add_css_class("op-card-header")
        kind = Gtk.Label(label=kind_label(clip), xalign=0)
        kind.add_css_class("op-kind")
        age = Gtk.Label(label=format_age(clip.last_used_at), xalign=0)
        age.add_css_class("op-meta")
        header.append(kind)
        header.append(age)
        self.append(header)

        self.body = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
        self.body.add_css_class("op-card-body")
        self.body.set_vexpand(True)
        self.body.set_hexpand(False)
        body = self.body
        if clip.kind == "image" and clip.image_path:
            picture = _image_preview(clip.image_path)
            picture.set_vexpand(True)
            body.append(picture)
        else:
            label = Gtk.Label(
                label=clip.text or clip.preview or "",
                xalign=0,
                yalign=0,
                wrap=True,
                wrap_mode=Pango.WrapMode.WORD_CHAR,
                ellipsize=Pango.EllipsizeMode.END,
                lines=8,
            )
            label.add_css_class("op-preview")
            label.set_max_width_chars(28)
            label.set_width_chars(28)
            label.set_hexpand(False)
            label.set_vexpand(True)
            body.append(label)
        self.append(body)

        footer = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL)
        footer.add_css_class("op-card-footer")
        chars = Gtk.Label(label=format_chars(clip), xalign=0.5)
        chars.add_css_class("op-chars")
        chars.set_halign(Gtk.Align.CENTER)
        chars.set_hexpand(True)
        footer.append(chars)
        self.append(footer)

    def set_selected(self, selected: bool) -> None:
        if selected:
            self.add_css_class("selected")
        else:
            self.remove_css_class("selected")


def _image_preview(path: str) -> Gtk.Widget:
    try:
        pixbuf = GdkPixbuf.Pixbuf.new_from_file_at_scale(path, 190, 140, True)
        texture = Gdk.Texture.new_for_pixbuf(pixbuf)
        picture = Gtk.Picture.new_for_paintable(texture)
        picture.set_content_fit(Gtk.ContentFit.COVER)
        picture.set_can_shrink(True)
        picture.set_hexpand(False)
        picture.set_vexpand(True)
        picture.set_size_request(190, 140)
        return picture
    except Exception:
        label = Gtk.Label(label="Image", xalign=0, yalign=0)
        label.add_css_class("op-preview")
        return label


class Overlay:
    def __init__(
        self,
        application: Gtk.Application,
        store: Store,
        config: Config,
        on_copy: Callable[[str], None] | None = None,
    ):
        self.store = store
        self.config = config
        self.on_copy = on_copy
        self.theme = load_theme()
        self.target: TargetWindow | None = None
        self.clips: list[Clip] = []
        self.selected_index = 0
        self.filter_text = ""
        self.cards: list[ClipCard] = []
        self._css = Gtk.CssProvider()
        self._visible = False
        self._anim_id = 0
        self._slide = 1.0

        self.window = Gtk.Window(application=application, title="omapaste")
        self.window.set_decorated(False)
        self.window.set_resizable(False)
        self.window.add_css_class("omapaste")
        self.window.set_overflow(Gtk.Overflow.HIDDEN)
        self.window.connect("close-request", self._on_close)

        width = _output_width()
        self.window.set_default_size(width, BAR_HEIGHT)
        self.window.set_size_request(width, BAR_HEIGHT)

        self.layer_shell = bool(LAYER_SHELL and LayerShell and LayerShell.is_supported())
        if self.layer_shell:
            LayerShell.init_for_window(self.window)
            LayerShell.set_namespace(self.window, "omapaste")
            LayerShell.set_layer(self.window, LayerShell.Layer.OVERLAY)
            LayerShell.set_anchor(self.window, LayerShell.Edge.BOTTOM, True)
            LayerShell.set_anchor(self.window, LayerShell.Edge.LEFT, True)
            LayerShell.set_anchor(self.window, LayerShell.Edge.RIGHT, True)
            LayerShell.set_anchor(self.window, LayerShell.Edge.TOP, False)
            LayerShell.set_exclusive_zone(self.window, 0)
            LayerShell.set_margin(self.window, LayerShell.Edge.LEFT, 18)
            LayerShell.set_margin(self.window, LayerShell.Edge.RIGHT, 18)
            LayerShell.set_margin(self.window, LayerShell.Edge.BOTTOM, VISIBLE_MARGIN)
            LayerShell.set_keyboard_mode(self.window, LayerShell.KeyboardMode.NONE)

        self._apply_theme(self.theme)

        self.bar = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8)
        self.bar.add_css_class("op-bar")
        self.bar.set_hexpand(True)
        self.bar.set_halign(Gtk.Align.FILL)

        header = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=10)
        header.add_css_class("op-header")

        self.brand = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=8)
        history_icon = Gtk.Image.new_from_icon_name("document-open-recent-symbolic")
        history_icon.set_pixel_size(16)
        title = Gtk.Label(label="History", xalign=0)
        title.add_css_class("op-title")
        self.count_label = Gtk.Label(xalign=0)
        self.count_label.add_css_class("op-count")
        self.brand.append(history_icon)
        self.brand.append(title)
        self.brand.append(self.count_label)
        self.brand.set_halign(Gtk.Align.START)

        self.search_open_btn = Gtk.Button()
        self.search_open_btn.set_has_frame(False)
        self.search_open_btn.set_icon_name("system-search-symbolic")
        self.search_open_btn.set_tooltip_text("Search")
        self.search_open_btn.add_css_class("op-icon-btn")
        self.search_open_btn.connect("clicked", lambda *_: self._open_search())

        self.search = Gtk.Entry()
        self.search.set_placeholder_text("Search clips")
        self.search.add_css_class("op-search")
        self.search.set_hexpand(True)
        self.search.set_icon_from_icon_name(Gtk.EntryIconPosition.PRIMARY, "system-search-symbolic")
        self.search.set_icon_from_icon_name(Gtk.EntryIconPosition.SECONDARY, "edit-clear-symbolic")
        self.search.set_icon_tooltip_text(Gtk.EntryIconPosition.SECONDARY, "Close search")
        self.search.connect("changed", self._on_search)
        self.search.connect("icon-press", self._on_search_icon)

        self.hint = Gtk.Label(
            label="← → select   Enter paste   click copy   Del delete   Ctrl+K keep   Esc close",
            xalign=1,
        )
        self.hint.add_css_class("op-hint")
        self.hint.set_ellipsize(Pango.EllipsizeMode.END)
        self.hint.set_halign(Gtk.Align.END)
        header.append(self.brand)
        header.append(self.search)
        header.append(self.search_open_btn)
        header.append(self.hint)
        self._search_open = False
        self._sync_search_chrome()

        self.scroller = Gtk.ScrolledWindow()
        self.scroller.set_policy(Gtk.PolicyType.AUTOMATIC, Gtk.PolicyType.NEVER)
        self.scroller.set_hexpand(True)
        self.scroller.set_size_request(-1, CARD_HEIGHT + 8)
        self.card_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=10)
        self.card_box.set_halign(Gtk.Align.START)
        self.card_box.set_valign(Gtk.Align.CENTER)
        self.card_box.set_hexpand(False)
        self.card_box.set_homogeneous(False)
        self.scroller.set_child(self.card_box)

        self.empty = Gtk.Label(label="Copy something. It will show up here.")
        self.empty.add_css_class("op-empty")
        self.empty.set_halign(Gtk.Align.CENTER)
        self.empty.set_valign(Gtk.Align.CENTER)
        self.empty.set_size_request(-1, CARD_HEIGHT)

        self.stack = Gtk.Stack()
        self.stack.add_named(self.scroller, "clips")
        self.stack.add_named(self.empty, "empty")

        self.bar.append(header)
        self.bar.append(self.stack)
        self.bar.set_size_request(width, BAR_HEIGHT)
        self.bar.set_valign(Gtk.Align.START)
        self.bar.set_vexpand(False)
        self.bar.set_margin_top(SLIDE_PX)

        # Keep the layer-shell surface parked on-screen and slide the bar
        # inside it. Animating zwlr margins reconfigures Hyprland every frame
        # and can exclusive-grab the keyboard while the bar is still off-screen.
        stage = Gtk.Box()
        stage.set_size_request(width, BAR_HEIGHT)
        self.clipper = Gtk.Overlay()
        self.clipper.set_overflow(Gtk.Overflow.HIDDEN)
        self.clipper.set_hexpand(True)
        self.clipper.set_size_request(width, BAR_HEIGHT)
        self.clipper.set_child(stage)
        self.clipper.add_overlay(self.bar)
        self.window.set_child(self.clipper)

        for widget in (self.window, self.search):
            keys = Gtk.EventControllerKey()
            keys.set_propagation_phase(Gtk.PropagationPhase.CAPTURE)
            keys.connect("key-pressed", self._on_key)
            widget.add_controller(keys)

        scroll = Gtk.EventControllerScroll.new(
            Gtk.EventControllerScrollFlags.HORIZONTAL
            | Gtk.EventControllerScrollFlags.VERTICAL
        )
        scroll.connect("scroll", self._on_scroll)
        self.card_box.add_controller(scroll)

    def reload_theme(self) -> None:
        self._apply_theme(load_theme())

    def _apply_theme(self, theme: Theme) -> None:
        self.theme = theme
        self._css.load_from_string(
            css_for(theme)
            + f"""
.op-card {{
  min-width: {CARD_WIDTH}px;
  min-height: {CARD_HEIGHT}px;
}}
"""
        )
        display = Gdk.Display.get_default()
        if display is not None:
            Gtk.StyleContext.add_provider_for_display(
                display,
                self._css,
                Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION,
            )

    def is_open(self) -> bool:
        return self._visible

    def toggle(self, target: TargetWindow | None = None) -> None:
        if self._visible:
            self.hide()
        else:
            self.show(target)

    def show(self, target: TargetWindow | None = None) -> None:
        self.target = target
        self._close_search()
        self.selected_index = 0
        self.refresh()
        mapped = self.window.get_mapped()
        self._visible = True
        if self.layer_shell:
            LayerShell.set_keyboard_mode(self.window, LayerShell.KeyboardMode.EXCLUSIVE)
        if not mapped:
            self._set_slide(1.0)
        self.window.set_visible(True)
        self.window.present()
        self._animate_slide(0.0)
        GLib.idle_add(self.window.grab_focus)

    def hide(self) -> None:
        if not self._visible and not self.window.get_mapped():
            return
        self._visible = False
        if self.layer_shell:
            LayerShell.set_keyboard_mode(self.window, LayerShell.KeyboardMode.NONE)
            self._animate_slide(1.0, on_done=self._after_hide)
        else:
            self.window.set_visible(False)

    def _after_hide(self) -> None:
        if not self._visible:
            self.window.set_visible(False)

    def _set_slide(self, hidden: float) -> None:
        self._slide = max(0.0, min(1.0, hidden))
        self.bar.set_margin_top(int(round(SLIDE_PX * self._slide)))

    def _stop_animation(self) -> None:
        if self._anim_id:
            GLib.source_remove(self._anim_id)
            self._anim_id = 0

    def _animate_slide(self, target: float, on_done: Callable[[], None] | None = None) -> None:
        self._stop_animation()
        start = self._slide
        distance = abs(target - start)
        if distance < 0.01:
            self._set_slide(target)
            if on_done:
                on_done()
            return
        duration = max(0.08, ANIM_DURATION_MS / 1000.0 * distance)
        started = time.monotonic()

        def tick() -> bool:
            t = min(1.0, (time.monotonic() - started) / duration)
            eased = 1.0 - (1.0 - t) ** 5
            self._set_slide(start + (target - start) * eased)
            if t >= 1.0:
                self._anim_id = 0
                if on_done:
                    on_done()
                return False
            return True

        self._anim_id = GLib.timeout_add(8, tick)

    def refresh(self, keep_selection: bool = False) -> None:
        selected_id = None
        if keep_selection and 0 <= self.selected_index < len(self.clips):
            selected_id = self.clips[self.selected_index].id
        self.clips = self.store.list(self.filter_text)
        if selected_id is not None:
            for i, clip in enumerate(self.clips):
                if clip.id == selected_id:
                    self.selected_index = i
                    break
            else:
                self.selected_index = 0
        elif self.clips:
            self.selected_index = min(self.selected_index, len(self.clips) - 1)
        else:
            self.selected_index = 0
        self._rebuild_cards()
        total = self.store.count()
        shown = len(self.clips)
        if self.filter_text:
            self.count_label.set_text(f"{shown} / {total}")
        else:
            self.count_label.set_text(f"{shown} clip" + ("" if shown == 1 else "s"))

    def selected(self) -> Clip | None:
        if 0 <= self.selected_index < len(self.clips):
            return self.clips[self.selected_index]
        return None

    def copy_selected(self) -> None:
        clip = self.selected()
        if not clip:
            return
        if clip.kind == "image" and clip.image_path:
            path = Path(clip.image_path)
            if path.exists():
                payload = path.read_bytes()
                digest = content_hash("image", clip.mime, payload)
                if self.on_copy:
                    self.on_copy(digest)
                copy_image(path, clip.mime)
        elif clip.text is not None:
            digest = content_hash("text", "text/plain", clip.text.encode())
            if self.on_copy:
                self.on_copy(digest)
            copy_text(clip.text)
        self.store.touch(clip.id)

    def paste_selected(self) -> None:
        if not self.selected():
            return
        self.copy_selected()
        target = self.target
        paste_keys = self.config.paste_keys
        self.hide()

        def _paste() -> bool:
            paste_now(target, paste_keys)
            return False

        GLib.timeout_add(160, _paste)

    def delete_selected(self) -> None:
        clip = self.selected()
        if not clip:
            return
        image = self.store.delete(clip.id)
        if image:
            image.unlink(missing_ok=True)
        if self.selected_index >= max(0, len(self.clips) - 1):
            self.selected_index = max(0, self.selected_index - 1)
        self.refresh()

    def set_keep(self, clip: Clip, preset: str) -> None:
        self.store.set_keep(clip.id, preset)
        GLib.idle_add(self._refresh_keep)

    def _refresh_keep(self) -> bool:
        self.refresh(keep_selection=True)
        return False

    def cycle_keep(self) -> None:
        clip = self.selected()
        if not clip:
            return
        self.set_keep(clip, next_keep(clip.keep_preset).key)

    def _rebuild_cards(self) -> None:
        while child := self.card_box.get_first_child():
            self.card_box.remove(child)
        self.cards = []
        if not self.clips:
            self.stack.set_visible_child_name("empty")
            return
        self.stack.set_visible_child_name("clips")
        for index, clip in enumerate(self.clips):
            card = ClipCard(clip)
            click = Gtk.GestureClick()
            click.connect("pressed", self._on_card_click, index)
            card.add_controller(click)
            card.set_selected(index == self.selected_index)
            self.card_box.append(card)
            self.cards.append(card)
        GLib.idle_add(self._scroll_selected)

    def _on_card_click(self, _gesture: Gtk.GestureClick, n_press: int, _x: float, _y: float, index: int) -> None:
        self._select(index, copy=True)
        if n_press >= 2:
            self.paste_selected()

    def _select(self, index: int, copy: bool) -> None:
        if not self.clips:
            return
        self.selected_index = max(0, min(index, len(self.clips) - 1))
        for i, card in enumerate(self.cards):
            card.set_selected(i == self.selected_index)
        self._scroll_selected()
        if copy:
            self.copy_selected()

    def _scroll_selected(self) -> bool:
        if not (0 <= self.selected_index < len(self.cards)):
            return False
        card = self.cards[self.selected_index]
        alloc = card.get_allocation()
        adj = self.scroller.get_hadjustment()
        if alloc.width <= 1:
            return False
        left = alloc.x
        right = alloc.x + alloc.width
        view_left = adj.get_value()
        view_right = view_left + adj.get_page_size()
        if left < view_left:
            adj.set_value(left - 12)
        elif right > view_right:
            adj.set_value(right - adj.get_page_size() + 12)
        return False

    def _is_searching(self) -> bool:
        return self._search_open

    def _sync_search_chrome(self) -> None:
        open_ = self._search_open
        self.search.set_visible(open_)
        self.search_open_btn.set_visible(not open_)
        self.brand.set_hexpand(not open_)

    def _open_search(self, prefix: str = "") -> None:
        self._search_open = True
        self._sync_search_chrome()
        if prefix:
            self.search.set_text(self.search.get_text() + prefix)
            self.search.set_position(-1)
        self.search.grab_focus()

    def _close_search(self) -> None:
        if self.search.get_text():
            self.search.set_text("")
        elif self.filter_text:
            self.filter_text = ""
            self.refresh()
        self._search_open = False
        self._sync_search_chrome()
        self.window.grab_focus()

    def _on_search_icon(self, _entry: Gtk.Entry, position: Gtk.EntryIconPosition) -> None:
        if position == Gtk.EntryIconPosition.SECONDARY:
            self._close_search()

    def _on_search(self, entry: Gtk.Entry) -> None:
        self.filter_text = entry.get_text()
        self.selected_index = 0
        self.refresh()

    def _on_scroll(self, _controller: Gtk.EventControllerScroll, dx: float, dy: float) -> bool:
        adj = self.scroller.get_hadjustment()
        adj.set_value(adj.get_value() + (dx + dy) * 40)
        return True

    def _on_close(self, _window: Gtk.Window) -> bool:
        self.hide()
        return True

    def _on_key(self, _controller: Gtk.EventControllerKey, keyval: int, _keycode: int, state: Gdk.ModifierType) -> bool:
        ctrl = bool(state & Gdk.ModifierType.CONTROL_MASK)

        if keyval == Gdk.KEY_Escape:
            if self._is_searching():
                self._close_search()
                return True
            self.hide()
            return True
        if keyval in (Gdk.KEY_Return, Gdk.KEY_KP_Enter):
            self.paste_selected()
            return True
        if keyval == Gdk.KEY_Left:
            self._select(self.selected_index - 1, copy=True)
            return True
        if keyval == Gdk.KEY_Right:
            self._select(self.selected_index + 1, copy=True)
            return True
        if keyval == Gdk.KEY_Home:
            self._select(0, copy=True)
            return True
        if keyval == Gdk.KEY_End and self.clips:
            self._select(len(self.clips) - 1, copy=True)
            return True
        if keyval in (Gdk.KEY_Delete, Gdk.KEY_KP_Delete) or (
            keyval == Gdk.KEY_BackSpace and not self._is_searching()
        ):
            self.delete_selected()
            return True
        if ctrl and keyval in (Gdk.KEY_k, Gdk.KEY_K):
            self.cycle_keep()
            return True
        if ctrl and keyval in (Gdk.KEY_f, Gdk.KEY_F, Gdk.KEY_slash):
            self._open_search()
            return True
        if keyval == Gdk.KEY_slash and not self._is_searching():
            self._open_search()
            return True
        if ctrl and Gdk.KEY_1 <= keyval <= Gdk.KEY_9:
            self._select(keyval - Gdk.KEY_1, copy=True)
            self.paste_selected()
            return True

        # Start typing to search, like Paste.app.
        if not ctrl and not self._is_searching():
            char = chr(Gdk.keyval_to_unicode(keyval)) if Gdk.keyval_to_unicode(keyval) else ""
            if char.isprintable() and char:
                self._open_search(char)
                return True
        return False
