use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

use chrono::{Local, TimeZone};
use gdk4::prelude::*;
use gtk4 as gtk;
use gtk4::prelude::*;
use gtk4::{gdk, glib, pango, Align, Application, Orientation, Overflow};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};

use crate::config::Config;
use crate::paste::{copy_image, copy_text, paste_now, TargetWindow};
use crate::paths::ISSUES_URL;
use crate::store::{content_hash, make_preview, next_keep, Clip, Store};
use crate::theme::{css_for, load_theme};

const BAR_HEIGHT: i32 = 356;
const CARD_WIDTH: i32 = 210;
const CARD_HEIGHT: i32 = 280;
const CARD_BORDER: i32 = 1;
const CARD_BODY_PAD_X: i32 = 12;
/// 12px JetBrains Mono is ~7px/cell; 1.25 scale hinting can be ~8px.
const PREVIEW_CELL_PX: i32 = 8;
const PREVIEW_INNER_WIDTH: i32 = CARD_WIDTH - 2 * CARD_BORDER - 2 * CARD_BODY_PAD_X;
const PREVIEW_MAX_CHARS: i32 = PREVIEW_INNER_WIDTH / PREVIEW_CELL_PX;
const PREVIEW_LINES: i32 = 7;
const VISIBLE_MARGIN: i32 = 14;
const SIDE_MARGIN: i32 = 18;
const SLIDE_PX: i32 = BAR_HEIGHT + 24;
const ANIM_DURATION: f64 = 0.220;
const CARD_DRAG_THRESHOLD_PX: i32 = 24;
const SEARCH_CARET_WIDTH: i32 = 2;
const SEARCH_CARET_GAP: i32 = 1;
const KIND_CARET_HEIGHT: i32 = 14;

const SHORTCUTS: &[(&str, &str)] = &[
    ("← →", "Select"),
    ("Enter", "Paste"),
    ("Click", "Copy"),
    ("Drag", "Drop to paste"),
    ("Del", "Delete"),
    ("Ctrl+K", "Keep"),
    ("Type", "Search"),
    ("Esc", "Close"),
];

struct UiState {
    clips: Vec<Clip>,
    selected: usize,
    filter: String,
    visible: bool,
    search_open: bool,
    slide: f64,
    anim_id: Option<glib::SourceId>,
    cards: Vec<gtk::Box>,
    target: Option<TargetWindow>,
    drag_panel_hidden: bool,
    drag_temp_path: Option<PathBuf>,
    search_blink_id: Option<glib::SourceId>,
    search_cursor: usize,
    search_anchor: usize,
    kind_edit_index: Option<usize>,
    kind_edit_text: String,
    kind_edit_anchor: usize,
    kind_edit_cursor: usize,
    kind_edit_blink_id: Option<glib::SourceId>,
    /// Set when a press finishes a rename; cleared on release so that press
    /// cannot start a card drag (which would hide the bar).
    block_card_drag: bool,
}

pub struct Overlay {
    pub window: gtk::Window,
    store: Rc<Store>,
    config: Config,
    on_copy: Rc<dyn Fn(&str)>,
    bar: gtk::Box,
    stage: gtk::Box,
    clipper: gtk::Overlay,
    brand: gtk::Box,
    search: gtk::Box,
    search_stack: gtk::Stack,
    search_label: gtk::Label,
    search_placeholder: gtk::Label,
    search_caret: gtk::Box,
    selection_rgb: RefCell<(u16, u16, u16)>,
    search_open_btn: gtk::Button,
    count_label: gtk::Label,
    scroller: gtk::ScrolledWindow,
    card_box: gtk::Box,
    stack: gtk::Stack,
    shortcuts: gtk::Popover,
    css: gtk::CssProvider,
    layer_shell: bool,
    state: RefCell<UiState>,
}

impl Overlay {
    pub fn new(
        app: &Application,
        store: Rc<Store>,
        config: Config,
        on_copy: Rc<dyn Fn(&str)>,
    ) -> Rc<Self> {
        let window = gtk::Window::builder()
            .application(app)
            .title("omapaste")
            .decorated(false)
            .resizable(false)
            .build();
        window.add_css_class("omapaste");
        window.set_overflow(Overflow::Hidden);

        let width = output_width();
        let (out_w, out_h) = output_size();
        let layer_shell = gtk4_layer_shell::is_supported();
        if layer_shell {
            window.init_layer_shell();
            window.set_namespace(Some("omapaste"));
            window.set_layer(Layer::Overlay);
            // Bottom/left/right only: height comes from our size request so the
            // surface can extend over the top bar's exclusive zone. Anchoring
            // Top as well lets the compositor inset us below that bar, and
            // clicks on it never reach our dismiss catcher.
            window.set_anchor(Edge::Top, false);
            window.set_anchor(Edge::Bottom, true);
            window.set_anchor(Edge::Left, true);
            window.set_anchor(Edge::Right, true);
            window.set_exclusive_zone(0);
            window.set_keyboard_mode(KeyboardMode::None);
            // Full monitor size so the dismiss region covers the desktop and
            // the Omarchy top bar; the clip bar still sits at the bottom.
            window.set_default_size(out_w, out_h);
            window.set_size_request(out_w, out_h);
        } else {
            window.set_default_size(width, BAR_HEIGHT);
            window.set_size_request(width, BAR_HEIGHT);
        }

        let css = gtk::CssProvider::new();
        let bar = gtk::Box::new(Orientation::Vertical, 8);
        bar.add_css_class("op-bar");
        bar.set_hexpand(true);
        bar.set_halign(Align::Fill);
        bar.set_can_focus(false);

        let header = gtk::Box::new(Orientation::Horizontal, 10);
        header.add_css_class("op-header");
        header.set_valign(Align::Center);
        header.set_vexpand(false);

        let brand = gtk::Box::new(Orientation::Horizontal, 8);
        let history_icon = gtk::Image::from_icon_name("document-open-recent-symbolic");
        history_icon.set_pixel_size(16);
        let title = gtk::Label::new(Some("History"));
        title.set_xalign(0.0);
        title.add_css_class("op-title");
        let count_label = gtk::Label::new(None);
        count_label.set_xalign(0.0);
        count_label.add_css_class("op-count");
        brand.append(&history_icon);
        brand.append(&title);
        brand.append(&count_label);
        brand.set_halign(Align::Start);
        brand.set_valign(Align::Center);
        brand.set_hexpand(false);

        let search_open_btn = gtk::Button::new();
        search_open_btn.set_has_frame(false);
        search_open_btn.set_icon_name("system-search-symbolic");
        search_open_btn.set_tooltip_text(Some("Search"));
        search_open_btn.add_css_class("op-icon-btn");
        search_open_btn.set_valign(Align::Center);
        // Keys are handled on the window; keep icon buttons mouse-only so GTK
        // does not park focus (and a focus ring) on the magnifying glass at open.
        search_open_btn.set_can_focus(false);

        let search = gtk::Box::new(Orientation::Horizontal, 6);
        search.add_css_class("op-search");
        search.set_hexpand(true);
        search.set_halign(Align::Fill);
        search.set_vexpand(false);
        search.set_valign(Align::Center);
        search.set_size_request(-1, 28);

        let search_icon = gtk::Image::new();
        search_icon.set_icon_name(Some("system-search-symbolic"));
        search_icon.add_css_class("op-search-icon");
        search_icon.set_valign(Align::Center);

        let search_field = gtk::Overlay::new();
        search_field.add_css_class("op-search-field");
        search_field.set_hexpand(true);
        search_field.set_valign(Align::Center);

        let search_label = gtk::Label::new(None);
        search_label.add_css_class("op-search-text");
        search_label.set_xalign(0.0);
        search_label.set_yalign(0.5);
        search_label.set_halign(Align::Start);
        search_label.set_valign(Align::Center);
        search_label.set_hexpand(true);
        search_label.set_ellipsize(pango::EllipsizeMode::End);
        search_field.set_child(Some(&search_label));

        let search_placeholder = gtk::Label::new(Some("Search clips"));
        search_placeholder.add_css_class("op-search-text");
        search_placeholder.add_css_class("placeholder");
        search_placeholder.set_xalign(0.0);
        search_placeholder.set_yalign(0.5);
        search_placeholder.set_halign(Align::Start);
        search_placeholder.set_valign(Align::Center);
        search_placeholder.set_hexpand(false);
        search_placeholder.set_ellipsize(pango::EllipsizeMode::End);
        search_field.add_overlay(&search_placeholder);

        let search_caret = gtk::Box::new(Orientation::Vertical, 0);
        search_caret.add_css_class("op-search-caret");
        search_caret.set_size_request(SEARCH_CARET_WIDTH, 18);
        search_caret.set_halign(Align::Start);
        search_caret.set_valign(Align::Center);
        search_caret.set_visible(false);
        search_field.add_overlay(&search_caret);

        let search_close_btn = gtk::Button::new();
        search_close_btn.set_has_frame(false);
        search_close_btn.set_icon_name("edit-clear-symbolic");
        search_close_btn.add_css_class("op-icon-btn");
        search_close_btn.set_valign(Align::Center);
        search_close_btn.set_can_focus(false);
        search_close_btn.set_tooltip_text(Some("Close search"));

        search.append(&search_icon);
        search.append(&search_field);
        search.append(&search_close_btn);

        let search_closed = gtk::Box::new(Orientation::Horizontal, 0);
        search_closed.set_hexpand(true);
        search_closed.set_halign(Align::Fill);
        search_closed.set_valign(Align::Center);
        // Fill the slot so the magnifier stays on the trailing edge
        // (before shortcuts/issues) until search opens.
        search_open_btn.set_hexpand(true);
        search_open_btn.set_halign(Align::End);
        search_closed.append(&search_open_btn);

        let search_stack = gtk::Stack::new();
        search_stack.add_css_class("op-search-slot");
        search_stack.set_hexpand(true);
        search_stack.set_halign(Align::Fill);
        search_stack.set_valign(Align::Center);
        search_stack.set_hhomogeneous(true);
        search_stack.set_vhomogeneous(true);
        search_stack.set_transition_type(gtk::StackTransitionType::None);
        search_stack.set_transition_duration(0);
        search_stack.set_size_request(-1, 28);
        search_stack.add_named(&search, Some("open"));
        search_stack.add_named(&search_closed, Some("closed"));
        search_stack.set_visible_child_name("closed");

        let shortcuts_btn = gtk::Button::new();
        shortcuts_btn.set_has_frame(false);
        shortcuts_btn.set_icon_name("input-keyboard-symbolic");
        shortcuts_btn.set_tooltip_text(Some("Shortcuts"));
        shortcuts_btn.add_css_class("op-icon-btn");
        shortcuts_btn.set_valign(Align::Center);
        shortcuts_btn.set_can_focus(false);
        let shortcuts = shortcuts_popover();
        shortcuts.set_parent(&shortcuts_btn);

        let issues_btn = gtk::Button::new();
        issues_btn.set_has_frame(false);
        issues_btn.set_icon_name("help-about-symbolic");
        issues_btn.set_tooltip_text(Some("Report an issue"));
        issues_btn.add_css_class("op-icon-btn");
        issues_btn.set_valign(Align::Center);
        issues_btn.set_can_focus(false);
        issues_btn.connect_clicked(|_| {
            let _ = std::process::Command::new("xdg-open")
                .arg(ISSUES_URL)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn();
        });

        header.append(&brand);
        header.append(&search_stack);
        header.append(&shortcuts_btn);
        header.append(&issues_btn);

        let scroller = gtk::ScrolledWindow::new();
        scroller.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Never);
        scroller.set_hexpand(true);
        scroller.set_size_request(-1, CARD_HEIGHT + 16);
        let card_box = gtk::Box::new(Orientation::Horizontal, 10);
        card_box.set_halign(Align::Start);
        card_box.set_valign(Align::Center);
        card_box.set_hexpand(false);
        card_box.set_homogeneous(false);
        scroller.set_child(Some(&card_box));

        let empty = gtk::Label::new(Some("Copy something. It will show up here."));
        empty.add_css_class("op-empty");
        empty.set_halign(Align::Center);
        empty.set_valign(Align::Center);
        empty.set_size_request(-1, CARD_HEIGHT);

        let stack = gtk::Stack::new();
        stack.add_named(&scroller, Some("clips"));
        stack.add_named(&empty, Some("empty"));

        bar.append(&header);
        bar.append(&stack);
        bar.set_valign(Align::Start);
        bar.set_vexpand(false);
        bar.set_margin_top(SLIDE_PX);

        let stage = gtk::Box::new(Orientation::Horizontal, 0);
        let clipper = gtk::Overlay::new();
        clipper.set_overflow(Overflow::Hidden);
        clipper.set_hexpand(true);
        clipper.set_vexpand(false);
        clipper.set_child(Some(&stage));
        clipper.add_overlay(&bar);

        let dismiss = if layer_shell {
            // Fullscreen dismiss under a bottom-anchored bar overlay. Overlay
            // children get pointer events in their area; the rest hits dismiss.
            clipper.set_margin_start(SIDE_MARGIN);
            clipper.set_margin_end(SIDE_MARGIN);
            clipper.set_margin_bottom(VISIBLE_MARGIN);
            clipper.set_halign(Align::Fill);
            clipper.set_valign(Align::End);
            bar.set_size_request(-1, BAR_HEIGHT);
            stage.set_size_request(-1, BAR_HEIGHT);
            clipper.set_size_request(-1, BAR_HEIGHT);

            let dismiss = gtk::Box::new(Orientation::Vertical, 0);
            dismiss.add_css_class("op-dismiss");
            dismiss.set_hexpand(true);
            dismiss.set_vexpand(true);
            dismiss.set_halign(Align::Fill);
            dismiss.set_valign(Align::Fill);
            dismiss.set_can_focus(false);

            let root = gtk::Overlay::new();
            root.set_hexpand(true);
            root.set_vexpand(true);
            root.set_size_request(out_w, out_h);
            root.set_child(Some(&dismiss));
            root.add_overlay(&clipper);
            window.set_child(Some(&root));
            Some(dismiss)
        } else {
            bar.set_size_request(width, BAR_HEIGHT);
            stage.set_size_request(width, BAR_HEIGHT);
            clipper.set_size_request(width, BAR_HEIGHT);
            window.set_child(Some(&clipper));
            None
        };

        let ov = Rc::new(Self {
            window: window.clone(),
            store,
            config,
            on_copy,
            bar,
            stage,
            clipper,
            brand,
            search: search.clone(),
            search_stack: search_stack.clone(),
            search_label: search_label.clone(),
            search_placeholder: search_placeholder.clone(),
            search_caret: search_caret.clone(),
            selection_rgb: RefCell::new(parse_hex_rgb_u16("#7aa2f7")),
            search_open_btn: search_open_btn.clone(),
            count_label,
            scroller: scroller.clone(),
            card_box: card_box.clone(),
            stack,
            shortcuts: shortcuts.clone(),
            css,
            layer_shell,
            state: RefCell::new(UiState {
                clips: Vec::new(),
                selected: 0,
                filter: String::new(),
                visible: false,
                search_open: false,
                slide: 1.0,
                anim_id: None,
                cards: Vec::new(),
                target: None,
                drag_panel_hidden: false,
                drag_temp_path: None,
                search_blink_id: None,
                search_cursor: 0,
                search_anchor: 0,
                kind_edit_index: None,
                kind_edit_text: String::new(),
                kind_edit_anchor: 0,
                kind_edit_cursor: 0,
                kind_edit_blink_id: None,
                block_card_drag: false,
            }),
        });
        ov.sync_search_chrome();
        ov.apply_theme();
        ov.watch_output();

        {
            let o = ov.clone();
            ov.window.connect_close_request(move |_| {
                o.hide_rc();
                glib::Propagation::Stop
            });
        }
        if layer_shell {
            // Transparent surfaces often shrink the input region to opaque ink only.
            // Keep the whole layer interactive so outside clicks reach us.
            {
                let win = ov.window.clone();
                ov.window.connect_map(move |_| {
                    let Some(surface) = win.surface() else {
                        return;
                    };
                    surface.set_input_region(None);
                    surface.connect_layout(move |surface, _, _| {
                        surface.set_input_region(None);
                    });
                });
            }
            // Clicks on the expanding dismiss region close the bar; clicks on the
            // bar strip never hit this widget, so they stay interactive.
            if let Some(dismiss) = dismiss {
                let o = ov.clone();
                let click = gtk::GestureClick::new();
                click.set_button(0);
                click.connect_pressed(move |_, _, _, _| {
                    if let Some(editing) = o.state.borrow().kind_edit_index {
                        o.finish_kind_edit(editing, false);
                    }
                    o.hide_rc();
                });
                dismiss.add_controller(click);
            }
        }
        {
            let o = ov.clone();
            search_open_btn.connect_clicked(move |_| o.open_search(""));
        }
        {
            let o = ov.clone();
            search_close_btn.connect_clicked(move |_| o.close_search_rc());
        }
        {
            let pop = shortcuts.clone();
            shortcuts_btn.connect_clicked(move |_| pop.popup());
        }
        {
            let keys = gtk::EventControllerKey::new();
            keys.set_propagation_phase(gtk::PropagationPhase::Capture);
            let o = ov.clone();
            keys.connect_key_pressed(move |_, key, _, state| o.on_key(key, state));
            ov.window.add_controller(keys);
        }
        {
            // Rename uses window keys (no real focus). Clicks outside the title
            // field must commit and leave edit mode — but clicks on another
            // card should only switch selection, not dismiss the bar.
            let click = gtk::GestureClick::new();
            click.set_button(0);
            click.set_propagation_phase(gtk::PropagationPhase::Capture);
            let o = ov.clone();
            click.connect_pressed(move |gesture, _, x, y| {
                let Some(editing) = o.state.borrow().kind_edit_index else {
                    return;
                };
                let Some(target) = gesture.widget() else {
                    return;
                };
                if let Some(picked) = target.pick(x, y, gtk::PickFlags::DEFAULT) {
                    if widget_or_ancestor_has_class(&picked, "op-kind-edit-field")
                        || widget_or_ancestor_has_class(&picked, "op-kind-slot")
                    {
                        return;
                    }
                    // Stay inside the bar: commit rename and let the card/button
                    // handler run (select, search, etc.).
                    if kind_edit_click_stays_in_bar(
                        widget_or_ancestor_has_class(&picked, "op-card"),
                        widget_or_ancestor_has_class(&picked, "op-bar"),
                    ) {
                        o.finish_kind_edit(editing, false);
                        o.state.borrow_mut().block_card_drag = true;
                        return;
                    }
                }
                o.finish_kind_edit(editing, false);
            });
            let o = ov.clone();
            click.connect_released(move |_, _, _, _| {
                o.state.borrow_mut().block_card_drag = false;
            });
            ov.window.add_controller(click);
        }
        let scroll = gtk::EventControllerScroll::new(
            gtk::EventControllerScrollFlags::HORIZONTAL | gtk::EventControllerScrollFlags::VERTICAL,
        );
        {
            let o = ov.clone();
            scroll.connect_scroll(move |_, dx, dy| {
                o.on_scroll(dx, dy);
                glib::Propagation::Stop
            });
        }
        card_box.add_controller(scroll);
        ov
    }

    pub fn is_open(&self) -> bool {
        self.state.borrow().visible
    }

    pub fn reload_theme(&self) {
        self.apply_theme();
    }

    fn sync_width(&self) {
        let width = output_width();
        let (out_w, out_h) = output_size();
        if self.layer_shell {
            self.window.set_default_size(out_w, out_h);
            self.window.set_size_request(out_w, out_h);
            self.bar.set_size_request(-1, BAR_HEIGHT);
            self.stage.set_size_request(-1, BAR_HEIGHT);
            self.clipper.set_size_request(-1, BAR_HEIGHT);
            if let Some(child) = self.window.child() {
                child.set_size_request(out_w, out_h);
            }
        } else {
            self.window.set_default_size(width, BAR_HEIGHT);
            self.window.set_size_request(width, BAR_HEIGHT);
            self.bar.set_size_request(width, BAR_HEIGHT);
            self.stage.set_size_request(width, BAR_HEIGHT);
            self.clipper.set_size_request(width, BAR_HEIGHT);
        }
    }

    fn watch_output(self: &Rc<Self>) {
        let Some(display) = gdk::Display::default() else {
            return;
        };
        let monitors = display.monitors();
        for i in 0..monitors.n_items() {
            if let Some(monitor) = monitors
                .item(i)
                .and_then(|o| o.downcast::<gdk::Monitor>().ok())
            {
                Self::watch_monitor(self, &monitor);
            }
        }
        let ov = Rc::clone(self);
        monitors.connect_items_changed(move |list, position, _removed, added| {
            ov.sync_width();
            for i in position..position.saturating_add(added) {
                if let Some(monitor) = list.item(i).and_then(|o| o.downcast::<gdk::Monitor>().ok())
                {
                    Self::watch_monitor(&ov, &monitor);
                }
            }
        });
    }

    fn watch_monitor(ov: &Rc<Self>, monitor: &gdk::Monitor) {
        let o = Rc::clone(ov);
        monitor.connect_geometry_notify(move |_| o.sync_width());
        let o = Rc::clone(ov);
        monitor.connect_scale_factor_notify(move |_| o.sync_width());
    }

    fn apply_theme(&self) {
        let theme = load_theme();
        *self.selection_rgb.borrow_mut() = parse_hex_rgb_u16(&theme.get("accent"));
        let extra = format!(
            "\n.op-card {{\n  min-width: {CARD_WIDTH}px;\n  min-height: {CARD_HEIGHT}px;\n}}\n"
        );
        self.css.load_from_string(&(css_for(&theme) + &extra));
        if let Some(display) = gdk::Display::default() {
            gtk::style_context_add_provider_for_display(
                &display,
                &self.css,
                gtk::STYLE_PROVIDER_PRIORITY_USER,
            );
        }
    }

    pub fn show_full(self: &Rc<Self>, target: Option<TargetWindow>) {
        self.close_search_rc();
        {
            let mut st = self.state.borrow_mut();
            st.target = target;
            st.selected = 0;
        }
        self.sync_width();
        self.refresh_rc(false);
        let mapped = self.window.is_mapped();
        self.state.borrow_mut().visible = true;
        if self.layer_shell {
            self.window.set_keyboard_mode(KeyboardMode::Exclusive);
        }
        if !mapped {
            self.set_slide(1.0);
        }
        self.window.set_visible(true);
        self.window.present();
        if self.layer_shell {
            if let Some(surface) = self.window.surface() {
                surface.set_input_region(None);
            }
            let win = self.window.clone();
            glib::idle_add_local_once(move || {
                if let Some(surface) = win.surface() {
                    surface.set_input_region(None);
                }
            });
        }
        self.animate_slide_rc(0.0, None);
        // Don't grab_focus() here: it paints a GTK focus ring around the bar.
        // Layer-shell KeyboardMode::Exclusive already delivers keys to us.
    }

    fn set_slide(&self, hidden: f64) {
        let hidden = hidden.clamp(0.0, 1.0);
        self.state.borrow_mut().slide = hidden;
        self.bar
            .set_margin_top(((SLIDE_PX as f64) * hidden).round() as i32);
    }

    fn stop_animation(&self) {
        if let Some(id) = self.state.borrow_mut().anim_id.take() {
            id.remove();
        }
    }

    pub fn refresh(&self, keep_selection: bool) {
        let st = self.state.borrow_mut();
        let selected_id = if keep_selection && st.selected < st.clips.len() {
            Some(st.clips[st.selected].id)
        } else {
            None
        };
        let filter = st.filter.clone();
        drop(st);
        let clips = self.store.list("", None).unwrap_or_default();
        let mut st = self.state.borrow_mut();
        if let Some(id) = selected_id {
            st.selected = clips.iter().position(|c| c.id == id).unwrap_or(0);
        } else if !clips.is_empty() {
            st.selected = st.selected.min(clips.len() - 1);
        } else {
            st.selected = 0;
        }
        st.clips = clips;
        let selected = st.selected;
        let clips = st.clips.clone();
        drop(st);
        self.rebuild_cards(&clips, selected);
        let total = clips.len();
        if filter.trim().is_empty() {
            self.count_label.set_text(&format!(
                "{total} clip{}",
                if total == 1 { "" } else { "s" }
            ));
        } else {
            self.apply_search_filter(false);
        }
    }

    fn rebuild_cards(&self, clips: &[Clip], selected: usize) {
        while let Some(child) = self.card_box.first_child() {
            self.card_box.remove(&child);
        }
        let mut cards = Vec::new();
        if clips.is_empty() {
            self.stack.set_visible_child_name("empty");
            self.state.borrow_mut().cards = cards;
            return;
        }
        self.stack.set_visible_child_name("clips");
        for (index, clip) in clips.iter().enumerate() {
            let card = clip_card(clip);
            if index == selected {
                card.add_css_class("selected");
            }
            self.card_box.append(&card);
            cards.push(card);
        }
        self.state.borrow_mut().cards = cards;
    }

    fn apply_search_filter(&self, reset_selection: bool) {
        let (filter, clips, cards, selected) = {
            let st = self.state.borrow();
            (
                st.filter.clone(),
                st.clips.clone(),
                st.cards.clone(),
                st.selected,
            )
        };
        let visible = visible_clip_indices(&clips, &filter);
        for (i, card) in cards.iter().enumerate() {
            card.set_visible(visible.contains(&i));
        }
        let new_selected = if visible.is_empty() {
            0
        } else if reset_selection {
            visible[0]
        } else if visible.contains(&selected) {
            selected
        } else {
            visible[0]
        };
        self.state.borrow_mut().selected = new_selected;
        for (i, card) in cards.iter().enumerate() {
            if i == new_selected {
                card.add_css_class("selected");
            } else {
                card.remove_css_class("selected");
            }
        }
        let total = clips.len();
        self.count_label.set_text(&format!(
            "{total} clip{}",
            if total == 1 { "" } else { "s" }
        ));
        if visible.is_empty() && !filter.trim().is_empty() {
            self.stack.set_visible_child_name("empty");
        } else {
            self.stack.set_visible_child_name("clips");
        }
    }

    fn set_search_query(&self, text: &str, reset_selection: bool) {
        let cursor = text.chars().count();
        self.set_search_edit(text, cursor, reset_selection);
    }

    fn set_search_edit(&self, text: &str, cursor: usize, reset_filter_selection: bool) {
        self.set_search_selection(text, cursor, cursor, reset_filter_selection);
    }

    fn set_search_selection(
        &self,
        text: &str,
        anchor: usize,
        cursor: usize,
        reset_filter_selection: bool,
    ) {
        let len = text.chars().count();
        let anchor = anchor.min(len);
        let cursor = cursor.min(len);
        {
            let mut st = self.state.borrow_mut();
            st.filter = text.to_string();
            st.search_anchor = anchor;
            st.search_cursor = cursor;
        }
        self.sync_search_display();
        self.apply_search_filter(reset_filter_selection);
    }

    fn sync_search_display(&self) {
        let (query, anchor, cursor, searching) = {
            let st = self.state.borrow();
            (
                st.filter.clone(),
                st.search_anchor,
                st.search_cursor,
                st.search_open,
            )
        };
        self.search_label.set_ellipsize(if searching {
            pango::EllipsizeMode::None
        } else {
            pango::EllipsizeMode::End
        });
        if query.is_empty() {
            self.search_label.set_text("");
            clear_label_selection_attrs(&self.search_label);
            self.search_placeholder.set_visible(true);
            self.search_caret.set_margin_start(0);
            self.search_caret.set_visible(searching);
            self.search_placeholder
                .set_margin_start(SEARCH_CARET_WIDTH + SEARCH_CARET_GAP);
        } else {
            self.search_label.set_text(&query);
            self.search_placeholder.set_visible(false);
            let (sel_start, sel_end) = kind_edit_range(anchor, cursor);
            sync_text_selection_chrome(
                &self.search_label,
                &self.search_caret,
                &query,
                sel_start,
                sel_end,
                cursor,
                searching,
                *self.selection_rgb.borrow(),
            );
        }
    }

    fn search_query(&self) -> String {
        self.state.borrow().filter.clone()
    }

    fn search_cursor(&self) -> usize {
        self.state.borrow().search_cursor
    }

    fn search_anchor(&self) -> usize {
        self.state.borrow().search_anchor
    }
}

// Full Rc-based API for interactive methods
impl Overlay {
    pub fn show_rc(self: &Rc<Self>, target: Option<TargetWindow>) {
        self.show_full(target);
    }

    pub fn hide_rc(self: &Rc<Self>) {
        let mapped = self.window.is_mapped();
        if !self.state.borrow().visible && !mapped {
            return;
        }
        self.state.borrow_mut().visible = false;
        self.shortcuts.popdown();
        if self.layer_shell {
            self.window.set_keyboard_mode(KeyboardMode::None);
            let this = Rc::clone(self);
            self.animate_slide_rc(
                1.0,
                Some(Box::new(move || {
                    if !this.state.borrow().visible {
                        this.window.set_visible(false);
                    }
                })),
            );
        } else {
            self.window.set_visible(false);
        }
    }

    fn hide_now_rc(self: &Rc<Self>) {
        let mapped = self.window.is_mapped();
        if !self.state.borrow().visible && !mapped {
            return;
        }
        self.stop_animation();
        self.state.borrow_mut().visible = false;
        self.set_slide(1.0);
        self.shortcuts.popdown();
        if self.layer_shell {
            self.window.set_keyboard_mode(KeyboardMode::None);
        }
        self.window.set_visible(false);
    }

    fn reopen_after_drag_rc(self: &Rc<Self>) {
        if self.state.borrow().visible {
            return;
        }
        self.sync_width();
        self.state.borrow_mut().visible = true;
        if self.layer_shell {
            self.window.set_keyboard_mode(KeyboardMode::Exclusive);
            if let Some(surface) = self.window.surface() {
                surface.set_input_region(None);
            }
        }
        self.set_slide(1.0);
        self.window.set_visible(true);
        self.window.present();
        self.animate_slide_rc(0.0, None);
    }

    fn finish_card_drag(self: &Rc<Self>, cancelled: bool) {
        let hidden = self.state.borrow().drag_panel_hidden;
        if let Some(path) = self.state.borrow_mut().drag_temp_path.take() {
            let _ = std::fs::remove_file(path);
        }
        self.state.borrow_mut().drag_panel_hidden = false;
        if should_reopen_after_drag(cancelled, hidden) {
            self.reopen_after_drag_rc();
        }
    }

    fn ensure_card_drag_threshold() {
        static ONCE: std::sync::Once = std::sync::Once::new();
        ONCE.call_once(|| {
            if let Some(settings) = gtk::Settings::default() {
                settings.set_gtk_dnd_drag_threshold(CARD_DRAG_THRESHOLD_PX);
            }
        });
    }

    fn animate_slide_rc(self: &Rc<Self>, target: f64, on_done: Option<Box<dyn Fn()>>) {
        self.stop_animation();
        let start = self.state.borrow().slide;
        let distance = (target - start).abs();
        if distance < 0.01 {
            self.set_slide(target);
            if let Some(cb) = on_done {
                cb();
            }
            return;
        }
        let duration = (ANIM_DURATION * distance).max(0.08);
        let started = Instant::now();
        let this = Rc::clone(self);
        let id = glib::timeout_add_local(Duration::from_millis(8), move || {
            let t = ((started.elapsed().as_secs_f64()) / duration).min(1.0);
            let eased = 1.0 - (1.0 - t).powi(5);
            this.set_slide(start + (target - start) * eased);
            if t >= 1.0 {
                this.state.borrow_mut().anim_id = None;
                if let Some(cb) = &on_done {
                    cb();
                }
                glib::ControlFlow::Break
            } else {
                glib::ControlFlow::Continue
            }
        });
        self.state.borrow_mut().anim_id = Some(id);
    }

    pub fn refresh_rc(self: &Rc<Self>, keep_selection: bool) {
        self.refresh(keep_selection);
        self.bind_card_clicks();
        self.bind_card_drags();
        self.bind_kind_label_edits();
        let this = Rc::clone(self);
        glib::idle_add_local_once(move || {
            this.scroll_selected();
        });
    }

    fn move_selection(self: &Rc<Self>, delta: i32, copy: bool) {
        let (clips, filter, selected) = {
            let st = self.state.borrow();
            (st.clips.clone(), st.filter.clone(), st.selected)
        };
        let visible = visible_clip_indices(&clips, &filter);
        if visible.is_empty() {
            return;
        }
        let pos = visible.iter().position(|&i| i == selected).unwrap_or(0);
        let next = (pos as i32 + delta).clamp(0, visible.len() as i32 - 1) as usize;
        self.select(visible[next], copy);
    }

    fn bind_kind_label_edits(self: &Rc<Self>) {
        let cards = self.state.borrow().cards.clone();
        for (index, card) in cards.into_iter().enumerate() {
            let Some(header) = find_css(card.upcast_ref(), "op-card-header")
                .and_then(|w| w.downcast::<gtk::Box>().ok())
            else {
                continue;
            };
            self.attach_header_rename_gesture(&header, index);
        }
    }

    fn attach_header_rename_gesture(self: &Rc<Self>, header: &gtk::Box, index: usize) {
        let gesture = gtk::GestureClick::new();
        gesture.set_button(0);
        gesture.set_propagation_phase(gtk::PropagationPhase::Capture);
        let this = Rc::clone(self);
        gesture.connect_pressed(move |gesture, n_press, _, _| {
            if n_press == 2 {
                gesture.set_state(gtk::EventSequenceState::Claimed);
                this.start_kind_edit(index);
            }
        });
        header.add_controller(gesture);
    }

    fn start_kind_edit(self: &Rc<Self>, index: usize) {
        let editing = self.state.borrow().kind_edit_index;
        if let Some(other) = editing {
            if other != index {
                self.finish_kind_edit(other, false);
            } else {
                return;
            }
        }
        let (card, clip) = {
            let st = self.state.borrow();
            let Some(card) = st.cards.get(index).cloned() else {
                return;
            };
            let Some(clip) = st.clips.get(index).cloned() else {
                return;
            };
            (card, clip)
        };
        if find_css(card.upcast_ref(), "op-kind-edit-field").is_none() {
            return;
        }

        let text = clip.display_label();
        let chars = text.chars().count();
        {
            let mut st = self.state.borrow_mut();
            st.kind_edit_index = Some(index);
            st.kind_edit_text = text;
            st.kind_edit_anchor = 0;
            st.kind_edit_cursor = chars;
        }
        self.sync_kind_edit_display(index);
        self.start_kind_edit_blink();
    }

    fn kind_edit_widgets(&self, index: usize) -> Option<(gtk::Label, gtk::Box)> {
        let card = self.state.borrow().cards.get(index).cloned()?;
        let field = find_css(card.upcast_ref(), "op-kind-edit-field")?;
        let label = find_css(field.upcast_ref(), "op-kind-edit-text")?
            .downcast::<gtk::Label>()
            .ok()?;
        let caret = find_css(field.upcast_ref(), "op-search-caret")?
            .downcast::<gtk::Box>()
            .ok()?;
        Some((label, caret))
    }

    fn sync_kind_edit_display(&self, index: usize) {
        let Some((label, caret)) = self.kind_edit_widgets(index) else {
            return;
        };
        let (text, anchor, cursor) = {
            let st = self.state.borrow();
            (
                st.kind_edit_text.clone(),
                st.kind_edit_anchor,
                st.kind_edit_cursor,
            )
        };
        let max_width = (PREVIEW_INNER_WIDTH - SEARCH_CARET_WIDTH).max(1);
        let (visible, scroll_start) = kind_edit_viewport(
            |slice| search_text_width(&label, slice),
            &text,
            cursor,
            max_width,
        );
        label.set_width_chars(1);
        label.set_ellipsize(pango::EllipsizeMode::None);
        label.set_text(&visible);
        let (sel_start, sel_end) = kind_edit_range(anchor, cursor);
        let vis_sel_start = sel_start
            .saturating_sub(scroll_start)
            .min(visible.chars().count());
        let vis_sel_end = sel_end
            .saturating_sub(scroll_start)
            .min(visible.chars().count());
        let cursor_in_visible = cursor.saturating_sub(scroll_start);
        sync_text_selection_chrome(
            &label,
            &caret,
            &visible,
            vis_sel_start,
            vis_sel_end,
            cursor_in_visible,
            true,
            *self.selection_rgb.borrow(),
        );
    }

    fn start_kind_edit_blink(self: &Rc<Self>) {
        self.stop_kind_edit_blink();
        let this = Rc::clone(self);
        let id = glib::timeout_add_local(Duration::from_millis(530), move || {
            if this.state.borrow().kind_edit_index.is_none() {
                this.state.borrow_mut().kind_edit_blink_id = None;
                return glib::ControlFlow::Break;
            }
            if let Some(index) = this.state.borrow().kind_edit_index {
                if let Some((_, caret)) = this.kind_edit_widgets(index) {
                    let has_selection = {
                        let st = this.state.borrow();
                        st.kind_edit_anchor != st.kind_edit_cursor
                    };
                    if has_selection {
                        caret.set_visible(false);
                    } else {
                        caret.set_visible(!caret.is_visible());
                    }
                }
            }
            glib::ControlFlow::Continue
        });
        self.state.borrow_mut().kind_edit_blink_id = Some(id);
    }

    fn stop_kind_edit_blink(&self) {
        if let Some(id) = self.state.borrow_mut().kind_edit_blink_id.take() {
            id.remove();
        }
    }

    fn clear_kind_edit_state(&self) {
        self.stop_kind_edit_blink();
        let mut st = self.state.borrow_mut();
        st.kind_edit_index = None;
        st.kind_edit_text.clear();
        st.kind_edit_anchor = 0;
        st.kind_edit_cursor = 0;
    }

    fn finish_kind_edit(self: &Rc<Self>, index: usize, cancel: bool) {
        if self.state.borrow().kind_edit_index != Some(index) {
            return;
        }
        let Some(card) = self.state.borrow().cards.get(index).cloned() else {
            self.clear_kind_edit_state();
            return;
        };

        if !cancel {
            let (text, clip_id) = {
                let st = self.state.borrow();
                (st.kind_edit_text.clone(), st.clips[index].id)
            };
            let label = text.trim();
            let value = if label.is_empty() { None } else { Some(label) };
            if let Ok(Some(updated)) = self.store.set_custom_label(clip_id, value) {
                self.state.borrow_mut().clips[index] = updated;
            }
        }

        let display = self.state.borrow().clips[index].display_label();
        if let Some(field) = find_css(card.upcast_ref(), "op-kind-edit-field")
            .and_then(|w| w.downcast::<gtk::Overlay>().ok())
        {
            show_kind_title_display(&field, &display);
        }
        self.clear_kind_edit_state();
    }

    fn on_kind_edit_key(
        self: &Rc<Self>,
        index: usize,
        key: gdk::Key,
        state: gdk::ModifierType,
    ) -> glib::Propagation {
        if key == gdk::Key::Escape {
            self.finish_kind_edit(index, true);
            return glib::Propagation::Stop;
        }
        if key == gdk::Key::Return || key == gdk::Key::KP_Enter {
            self.finish_kind_edit(index, false);
            return glib::Propagation::Stop;
        }
        if self.state.borrow().kind_edit_index != Some(index) {
            return glib::Propagation::Stop;
        }
        let (text, start, end, cursor) = {
            let st = self.state.borrow();
            let cursor = st.kind_edit_cursor;
            let (start, end) = kind_edit_range(st.kind_edit_anchor, cursor);
            (st.kind_edit_text.clone(), start, end, cursor)
        };
        let ctrl = state.contains(gdk::ModifierType::CONTROL_MASK);
        let alt = state.contains(gdk::ModifierType::ALT_MASK)
            || state.contains(gdk::ModifierType::META_MASK);

        if is_select_all_key(key, ctrl) {
            let len = text.chars().count();
            self.set_kind_edit_selection(index, text, 0, len);
            return glib::Propagation::Stop;
        }

        if key == gdk::Key::BackSpace {
            let (next, pos) = if ctrl {
                (String::new(), 0)
            } else if alt {
                entry_edit_backspace_word(&text, start, end)
            } else {
                entry_edit_backspace(&text, start, end)
            };
            self.set_kind_edit_text(index, next, pos);
            return glib::Propagation::Stop;
        }
        if ctrl && key == gdk::Key::u {
            self.set_kind_edit_text(index, String::new(), 0);
            return glib::Propagation::Stop;
        }
        if key == gdk::Key::Delete || key == gdk::Key::KP_Delete {
            let (next, pos) = entry_edit_delete(&text, start, end);
            self.set_kind_edit_text(index, next, pos);
            return glib::Propagation::Stop;
        }
        if key == gdk::Key::Left {
            let pos = if alt || ctrl {
                entry_edit_move_word(&text, cursor, false)
            } else {
                entry_edit_move(&text, start, end, cursor as i32, -1)
            };
            self.set_kind_edit_text(index, text, pos);
            return glib::Propagation::Stop;
        }
        if key == gdk::Key::Right {
            let pos = if alt || ctrl {
                entry_edit_move_word(&text, cursor, true)
            } else {
                entry_edit_move(&text, start, end, cursor as i32, 1)
            };
            self.set_kind_edit_text(index, text, pos);
            return glib::Propagation::Stop;
        }
        if key == gdk::Key::Home {
            self.set_kind_edit_text(index, text, 0);
            return glib::Propagation::Stop;
        }
        if key == gdk::Key::End {
            let pos = text.chars().count();
            self.set_kind_edit_text(index, text, pos);
            return glib::Propagation::Stop;
        }
        if ctrl {
            return glib::Propagation::Stop;
        }
        if let Some(ch) = key.to_unicode() {
            if !ch.is_control() {
                let (next, pos) = entry_edit_insert(&text, start, end, ch);
                self.set_kind_edit_text(index, next, pos);
                return glib::Propagation::Stop;
            }
        }
        glib::Propagation::Stop
    }

    fn set_kind_edit_text(self: &Rc<Self>, index: usize, text: String, cursor: usize) {
        self.set_kind_edit_selection(index, text, cursor, cursor);
    }

    fn set_kind_edit_selection(
        self: &Rc<Self>,
        index: usize,
        text: String,
        anchor: usize,
        cursor: usize,
    ) {
        let len = text.chars().count();
        {
            let mut st = self.state.borrow_mut();
            if st.kind_edit_index != Some(index) {
                return;
            }
            st.kind_edit_text = text;
            st.kind_edit_anchor = anchor.min(len);
            st.kind_edit_cursor = cursor.min(len);
        }
        self.sync_kind_edit_display(index);
        self.poke_kind_edit_caret();
    }

    fn poke_kind_edit_caret(self: &Rc<Self>) {
        if let Some(index) = self.state.borrow().kind_edit_index {
            if let Some((_, caret)) = self.kind_edit_widgets(index) {
                let has_selection = {
                    let st = self.state.borrow();
                    st.kind_edit_anchor != st.kind_edit_cursor
                };
                if has_selection {
                    caret.set_visible(false);
                } else {
                    caret.set_visible(true);
                }
            }
        }
        self.start_kind_edit_blink();
    }

    fn bind_card_clicks(self: &Rc<Self>) {
        let cards = self.state.borrow().cards.clone();
        for (index, card) in cards.into_iter().enumerate() {
            let click = gtk::GestureClick::new();
            let this = Rc::clone(self);
            click.connect_pressed(move |_, n_press, _, _| {
                if let Some(editing) = this.state.borrow().kind_edit_index {
                    this.finish_kind_edit(editing, false);
                    this.state.borrow_mut().block_card_drag = true;
                }
                this.select(index, true);
                if n_press >= 2 {
                    this.paste_selected();
                }
            });
            card.add_controller(click);
        }
    }

    fn bind_card_drags(self: &Rc<Self>) {
        Self::ensure_card_drag_threshold();
        let clips = self.state.borrow().clips.clone();
        let cards = self.state.borrow().cards.clone();
        for (index, card) in cards.into_iter().enumerate() {
            let Some(clip) = clips.get(index).cloned() else {
                continue;
            };
            let drag_source = gtk::DragSource::new();
            drag_source.set_actions(gdk::DragAction::COPY);
            drag_source.set_exclusive(true);
            drag_source.set_touch_only(false);

            let this_prep = Rc::clone(self);
            let clip_for_prepare = clip.clone();
            drag_source.connect_prepare(move |_, _, _| {
                // Drag hides the bar. Block while renaming, and for the rest of
                // a press that just committed a rename (edit state is already clear).
                let st = this_prep.state.borrow();
                if !card_drag_prepare_allowed(st.kind_edit_index, st.block_card_drag) {
                    return None;
                }
                drop(st);
                let payload = clip_drag_payload(&clip_for_prepare, |p| std::fs::read(p).ok())?;
                if let Some(ref path) = payload.drag_path {
                    this_prep.state.borrow_mut().drag_temp_path = Some(path.clone());
                }
                Some(clip_drag_provider(&payload))
            });

            let this = Rc::clone(self);
            let clip_for_begin = clip.clone();
            drag_source.connect_drag_begin(move |_, _| {
                this.select(index, false);
                this.copy_clip(&clip_for_begin);
                this.state.borrow_mut().drag_panel_hidden = true;
                // Unmap the fullscreen layer-shell surface so drops reach apps
                // below. Sliding the bar off-screen is not enough — the overlay
                // still intercepts the drag.
                this.hide_now_rc();
            });

            let this = Rc::clone(self);
            drag_source.connect_drag_cancel(move |_, _, _| {
                this.finish_card_drag(true);
                false
            });

            let this = Rc::clone(self);
            drag_source.connect_drag_end(move |_, _, _| {
                this.finish_card_drag(false);
            });

            card.add_controller(drag_source);
        }
    }

    fn selected_clip(&self) -> Option<Clip> {
        let st = self.state.borrow();
        st.clips.get(st.selected).cloned()
    }

    fn copy_selected(&self) {
        let Some(clip) = self.selected_clip() else {
            return;
        };
        self.copy_clip(&clip);
    }

    fn copy_clip(&self, clip: &Clip) {
        if clip.kind == "image" {
            if let Some(ref path) = clip.image_path {
                let p = Path::new(path);
                if p.exists() {
                    if let Ok(payload) = std::fs::read(p) {
                        let digest = content_hash("image", &clip.mime, &payload);
                        (self.on_copy)(&digest);
                    }
                    copy_image(p, &clip.mime);
                }
            }
        } else if let Some(ref text) = clip.text {
            let digest = content_hash("text", "text/plain", text.as_bytes());
            (self.on_copy)(&digest);
            copy_text(text);
        }
        let _ = self.store.touch(clip.id, None);
    }

    fn paste_selected(self: &Rc<Self>) {
        if self.selected_clip().is_none() {
            return;
        }
        self.copy_selected();
        let target = self.state.borrow().target.clone();
        let keys = self.config.paste_keys.clone();
        self.hide_rc();
        glib::timeout_add_local_once(Duration::from_millis(160), move || {
            paste_now(target.as_ref(), &keys);
        });
    }

    fn delete_selected(self: &Rc<Self>) {
        let Some(clip) = self.selected_clip() else {
            return;
        };
        if let Ok(Some(image)) = self.store.delete(clip.id) {
            let _ = std::fs::remove_file(image);
        }
        {
            let mut st = self.state.borrow_mut();
            st.selected = select_after_delete(st.selected, st.clips.len());
        }
        self.refresh_rc(false);
    }

    fn cycle_keep(self: &Rc<Self>) {
        let Some(clip) = self.selected_clip() else {
            return;
        };
        let next = next_keep(&clip.keep_preset);
        let _ = self.store.set_keep(clip.id, next.key, None);
        let this = Rc::clone(self);
        glib::idle_add_local_once(move || {
            this.refresh_rc(true);
        });
    }

    fn select(self: &Rc<Self>, index: usize, copy: bool) {
        let editing = self.state.borrow().kind_edit_index;
        if let Some(editing) = editing {
            if editing != index {
                self.finish_kind_edit(editing, false);
            }
        }
        let n = self.state.borrow().clips.len();
        let Some(index) = clamp_select(index, n) else {
            return;
        };
        self.state.borrow_mut().selected = index;
        let cards = self.state.borrow().cards.clone();
        for (i, card) in cards.iter().enumerate() {
            if i == index {
                card.add_css_class("selected");
            } else {
                card.remove_css_class("selected");
            }
        }
        self.scroll_selected();
        if copy {
            self.copy_selected();
        }
    }

    fn scroll_selected(&self) {
        let st = self.state.borrow();
        if st.selected >= st.cards.len() {
            return;
        }
        let card = st.cards[st.selected].clone();
        drop(st);
        let alloc = card.allocation();
        let adj = self.scroller.hadjustment();
        if alloc.width() <= 1 {
            return;
        }
        let left = f64::from(alloc.x());
        let right = left + f64::from(alloc.width());
        let view_left = adj.value();
        let view_right = view_left + adj.page_size();
        if left < view_left {
            adj.set_value(left - 12.0);
        } else if right > view_right {
            adj.set_value(right - adj.page_size() + 12.0);
        }
    }

    fn is_searching(&self) -> bool {
        self.state.borrow().search_open
    }

    fn sync_search_chrome(&self) {
        let open = self.state.borrow().search_open;
        self.search_stack
            .set_visible_child_name(if open { "open" } else { "closed" });
    }

    fn start_search_caret_blink(self: &Rc<Self>) {
        if let Some(id) = self.state.borrow_mut().search_blink_id.take() {
            id.remove();
        }
        self.search.add_css_class("op-search-active");
        let has_selection = {
            let st = self.state.borrow();
            let (start, end) = kind_edit_range(st.search_anchor, st.search_cursor);
            start != end
        };
        self.search_caret.set_visible(!has_selection);
        let this = Rc::clone(self);
        let id = glib::timeout_add_local(Duration::from_millis(530), move || {
            if !this.is_searching() {
                this.state.borrow_mut().search_blink_id = None;
                return glib::ControlFlow::Break;
            }
            let has_selection = {
                let st = this.state.borrow();
                let (start, end) = kind_edit_range(st.search_anchor, st.search_cursor);
                start != end
            };
            if has_selection {
                this.search_caret.set_visible(false);
            } else {
                this.search_caret
                    .set_visible(!this.search_caret.is_visible());
            }
            glib::ControlFlow::Continue
        });
        self.state.borrow_mut().search_blink_id = Some(id);
    }

    fn poke_search_caret(self: &Rc<Self>) {
        let has_selection = {
            let st = self.state.borrow();
            let (start, end) = kind_edit_range(st.search_anchor, st.search_cursor);
            start != end
        };
        self.search_caret.set_visible(!has_selection);
        if self.state.borrow().search_blink_id.is_none() {
            self.start_search_caret_blink();
        }
    }

    fn stop_search_caret_blink(&self) {
        if let Some(id) = self.state.borrow_mut().search_blink_id.take() {
            id.remove();
        }
        self.search.remove_css_class("op-search-active");
        self.search_caret.set_visible(false);
        clear_label_selection_attrs(&self.search_label);
    }

    fn open_search(self: &Rc<Self>, prefix: &str) {
        self.state.borrow_mut().search_open = true;
        self.sync_search_chrome();
        if prefix.is_empty() {
            let end = self.search_query().chars().count();
            {
                let mut st = self.state.borrow_mut();
                st.search_cursor = end;
                st.search_anchor = end;
            }
            self.sync_search_display();
            self.start_search_caret_blink();
        } else {
            self.set_search_query(prefix, true);
            self.start_search_caret_blink();
        }
    }

    fn close_search_rc(self: &Rc<Self>) {
        if !self.search_query().is_empty() {
            self.set_search_query("", true);
        }
        self.stop_search_caret_blink();
        self.state.borrow_mut().search_open = false;
        self.sync_search_chrome();
        // Avoid window.grab_focus(): it draws a GTK focus ring around the bar.
    }

    fn on_scroll(&self, dx: f64, dy: f64) {
        let adj = self.scroller.hadjustment();
        adj.set_value(adj.value() + (dx + dy) * 40.0);
    }

    fn on_key(self: &Rc<Self>, key: gdk::Key, state: gdk::ModifierType) -> glib::Propagation {
        let editing = self.state.borrow().kind_edit_index;
        if let Some(index) = editing {
            return self.on_kind_edit_key(index, key, state);
        }
        let ctrl = state.contains(gdk::ModifierType::CONTROL_MASK);
        let searching = self.is_searching();
        let intent = key_intent(key, ctrl, searching);
        if searching {
            return self.on_search_key(key, state, intent);
        }
        match intent {
            KeyIntent::Dismiss => {
                if self.shortcuts.is_visible() {
                    self.shortcuts.popdown();
                } else if self.is_searching() {
                    self.close_search_rc();
                } else {
                    self.hide_rc();
                }
                glib::Propagation::Stop
            }
            KeyIntent::Paste => {
                self.paste_selected();
                glib::Propagation::Stop
            }
            KeyIntent::Left => {
                self.move_selection(-1, true);
                glib::Propagation::Stop
            }
            KeyIntent::Right => {
                self.move_selection(1, true);
                glib::Propagation::Stop
            }
            KeyIntent::Home => {
                let (clips, filter) = {
                    let st = self.state.borrow();
                    (st.clips.clone(), st.filter.clone())
                };
                if let Some(&first) = visible_clip_indices(&clips, &filter).first() {
                    self.select(first, true);
                }
                glib::Propagation::Stop
            }
            KeyIntent::End => {
                let (clips, filter) = {
                    let st = self.state.borrow();
                    (st.clips.clone(), st.filter.clone())
                };
                if let Some(&last) = visible_clip_indices(&clips, &filter).last() {
                    self.select(last, true);
                }
                glib::Propagation::Stop
            }
            KeyIntent::Delete => {
                self.delete_selected();
                glib::Propagation::Stop
            }
            KeyIntent::CycleKeep => {
                self.cycle_keep();
                glib::Propagation::Stop
            }
            KeyIntent::OpenSearch => {
                self.open_search("");
                glib::Propagation::Stop
            }
            KeyIntent::TypeSearch(ch) => {
                self.open_search(&ch.to_string());
                glib::Propagation::Stop
            }
            KeyIntent::PasteNth(n) => {
                self.select(n, true);
                self.paste_selected();
                glib::Propagation::Stop
            }
            KeyIntent::Other => glib::Propagation::Proceed,
        }
    }

    fn on_search_key(
        self: &Rc<Self>,
        key: gdk::Key,
        state: gdk::ModifierType,
        intent: KeyIntent,
    ) -> glib::Propagation {
        let ctrl = state.contains(gdk::ModifierType::CONTROL_MASK);
        let alt = state.contains(gdk::ModifierType::ALT_MASK)
            || state.contains(gdk::ModifierType::META_MASK);
        match intent {
            KeyIntent::Dismiss => {
                self.close_search_rc();
                glib::Propagation::Stop
            }
            KeyIntent::CycleKeep if ctrl => {
                self.cycle_keep();
                glib::Propagation::Stop
            }
            KeyIntent::Paste => {
                self.paste_selected();
                glib::Propagation::Stop
            }
            KeyIntent::OpenSearch => glib::Propagation::Stop,
            _ => {
                let text = self.search_query();
                let cursor = self.search_cursor().min(text.chars().count());
                let anchor = self.search_anchor().min(text.chars().count());
                let (start, end) = kind_edit_range(anchor, cursor);

                if is_select_all_key(key, ctrl) {
                    let len = text.chars().count();
                    self.set_search_selection(&text, 0, len, false);
                    self.poke_search_caret();
                    return glib::Propagation::Stop;
                }

                if key == gdk::Key::Left {
                    let pos = if alt || ctrl {
                        entry_edit_move_word(&text, cursor, false)
                    } else {
                        entry_edit_move(&text, start, end, cursor as i32, -1)
                    };
                    self.set_search_edit(&text, pos, false);
                    self.poke_search_caret();
                    return glib::Propagation::Stop;
                }
                if key == gdk::Key::Right {
                    let pos = if alt || ctrl {
                        entry_edit_move_word(&text, cursor, true)
                    } else {
                        entry_edit_move(&text, start, end, cursor as i32, 1)
                    };
                    self.set_search_edit(&text, pos, false);
                    self.poke_search_caret();
                    return glib::Propagation::Stop;
                }
                if key == gdk::Key::Home {
                    self.set_search_edit(&text, 0, false);
                    self.poke_search_caret();
                    return glib::Propagation::Stop;
                }
                if key == gdk::Key::End {
                    self.set_search_edit(&text, text.chars().count(), false);
                    self.poke_search_caret();
                    return glib::Propagation::Stop;
                }
                if key == gdk::Key::BackSpace {
                    let (next, pos) = if ctrl {
                        (String::new(), 0)
                    } else if alt {
                        entry_edit_backspace_word(&text, start, end)
                    } else {
                        entry_edit_backspace(&text, start, end)
                    };
                    if next != text || pos != cursor {
                        self.set_search_edit(&next, pos, false);
                    }
                    self.poke_search_caret();
                    return glib::Propagation::Stop;
                }
                if ctrl && key == gdk::Key::u {
                    if !text.is_empty() {
                        self.set_search_edit("", 0, false);
                    }
                    self.poke_search_caret();
                    return glib::Propagation::Stop;
                }
                if ctrl {
                    return glib::Propagation::Proceed;
                }
                if key == gdk::Key::Delete || key == gdk::Key::KP_Delete {
                    let (next, pos) = entry_edit_delete(&text, start, end);
                    if next != text || pos != cursor {
                        self.set_search_edit(&next, pos, false);
                    }
                    self.poke_search_caret();
                    return glib::Propagation::Stop;
                }
                if let Some(ch) = key.to_unicode() {
                    if !ch.is_control() {
                        let (next, pos) = entry_edit_insert(&text, start, end, ch);
                        self.set_search_edit(&next, pos, false);
                        self.poke_search_caret();
                        return glib::Propagation::Stop;
                    }
                }
                glib::Propagation::Proceed
            }
        }
    }
}

fn search_text_width(label: &gtk::Label, text: &str) -> i32 {
    if text.is_empty() {
        return 0;
    }
    // Fresh layout so we never mutate the label's live Pango layout (and so
    // width is unconstrained by the widget allocation).
    let layout = label.create_pango_layout(Some(text));
    layout.set_width(-1);
    layout.pixel_size().0.max(0)
}

fn char_byte_index(text: &str, cursor_chars: usize) -> usize {
    if cursor_chars >= text.chars().count() {
        text.len()
    } else {
        text.char_indices()
            .nth(cursor_chars)
            .map(|(index, _)| index)
            .unwrap_or(text.len())
    }
}

fn label_caret_x(label: &gtk::Label, text: &str, cursor_chars: usize) -> i32 {
    if cursor_chars == 0 || text.is_empty() {
        return 0;
    }
    let prefix: String = text.chars().take(cursor_chars).collect();
    search_text_width(label, &prefix)
}

fn search_text_pop_word(text: &str) -> String {
    let mut chars: Vec<char> = text.chars().collect();
    while chars.last().is_some_and(|c| c.is_whitespace()) {
        chars.pop();
    }
    while chars.last().is_some_and(|c| !c.is_whitespace()) {
        chars.pop();
    }
    if chars.last().is_some_and(|c| c.is_whitespace()) {
        chars.pop();
    }
    chars.into_iter().collect()
}

fn entry_edit_backspace_word(text: &str, start: usize, end: usize) -> (String, usize) {
    if start != end {
        return entry_edit_backspace(text, start, end);
    }
    if start == 0 {
        return (text.to_string(), 0);
    }
    let prefix: String = text.chars().take(start).collect();
    let next_prefix = search_text_pop_word(&prefix);
    let suffix: String = text.chars().skip(start).collect();
    let new_pos = next_prefix.chars().count();
    (format!("{next_prefix}{suffix}"), new_pos)
}

fn entry_edit_insert(text: &str, start: usize, end: usize, ch: char) -> (String, usize) {
    let chars: Vec<char> = text.chars().collect();
    let start = start.min(chars.len());
    let end = end.min(chars.len());
    let mut next: Vec<char> = chars[..start].to_vec();
    next.push(ch);
    next.extend_from_slice(&chars[end..]);
    (next.into_iter().collect(), start + 1)
}

fn entry_edit_backspace(text: &str, start: usize, end: usize) -> (String, usize) {
    let chars: Vec<char> = text.chars().collect();
    if start != end {
        let end = end.min(chars.len());
        let mut next: Vec<char> = chars[..start.min(chars.len())].to_vec();
        next.extend_from_slice(&chars[end..]);
        (next.into_iter().collect(), start.min(chars.len()))
    } else if start == 0 {
        (text.to_string(), 0)
    } else {
        let mut next: Vec<char> = chars[..start - 1].to_vec();
        next.extend_from_slice(&chars[start..]);
        (next.into_iter().collect(), start - 1)
    }
}

fn entry_edit_delete(text: &str, start: usize, end: usize) -> (String, usize) {
    let chars: Vec<char> = text.chars().collect();
    if start != end {
        let end = end.min(chars.len());
        let mut next: Vec<char> = chars[..start.min(chars.len())].to_vec();
        next.extend_from_slice(&chars[end..]);
        (next.into_iter().collect(), start.min(chars.len()))
    } else if start >= chars.len() {
        (text.to_string(), start)
    } else {
        let mut next: Vec<char> = chars[..start].to_vec();
        next.extend_from_slice(&chars[start + 1..]);
        (next.into_iter().collect(), start)
    }
}

fn entry_edit_move(text: &str, start: usize, end: usize, cursor: i32, delta: i32) -> usize {
    let len = text.chars().count();
    if start != end {
        if delta < 0 {
            start.min(len)
        } else {
            end.min(len)
        }
    } else {
        (cursor.max(0) as usize)
            .saturating_add_signed(delta as isize)
            .min(len)
    }
}

fn entry_edit_move_word(text: &str, cursor: usize, forward: bool) -> usize {
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut i = cursor.min(len);
    if forward {
        while i < len && !chars[i].is_whitespace() {
            i += 1;
        }
        while i < len && chars[i].is_whitespace() {
            i += 1;
        }
        i
    } else {
        while i > 0 && chars[i - 1].is_whitespace() {
            i -= 1;
        }
        while i > 0 && !chars[i - 1].is_whitespace() {
            i -= 1;
        }
        i
    }
}

fn clip_matches_filter(clip: &Clip, needle: &str) -> bool {
    clip.preview.to_lowercase().contains(needle)
        || clip
            .text
            .as_deref()
            .map(|t| t.to_lowercase().contains(needle))
            .unwrap_or(false)
        || clip
            .custom_label
            .as_deref()
            .map(|l| l.to_lowercase().contains(needle))
            .unwrap_or(false)
}

fn find_css(widget: &gtk::Widget, class: &str) -> Option<gtk::Widget> {
    if widget.has_css_class(class) {
        return Some(widget.clone());
    }
    let mut child = widget.first_child();
    while let Some(c) = child {
        if let Some(found) = find_css(&c, class) {
            return Some(found);
        }
        child = c.next_sibling();
    }
    None
}

fn widget_or_ancestor_has_class(widget: &gtk::Widget, class: &str) -> bool {
    let mut current = Some(widget.clone());
    while let Some(w) = current {
        if w.has_css_class(class) {
            return true;
        }
        current = w.parent();
    }
    false
}

fn ellipsize_to_width(measure: impl Fn(&str) -> i32, text: &str, max_px: i32) -> String {
    if text.is_empty() || max_px <= 0 {
        return String::new();
    }
    if measure(text) <= max_px {
        return text.to_string();
    }
    const ELL: &str = "…";
    let ell_w = measure(ELL);
    if ell_w >= max_px {
        return ELL.to_string();
    }
    let chars: Vec<char> = text.chars().collect();
    let mut best = 0usize;
    let mut lo = 0usize;
    let mut hi = chars.len();
    while lo <= hi {
        let mid = (lo + hi) / 2;
        let mut candidate: String = chars[..mid].iter().collect();
        candidate.push_str(ELL);
        if measure(&candidate) <= max_px {
            best = mid;
            lo = mid + 1;
        } else if mid == 0 {
            break;
        } else {
            hi = mid - 1;
        }
    }
    if best == 0 {
        ELL.to_string()
    } else {
        let mut out: String = chars[..best].iter().collect();
        out.push_str(ELL);
        out
    }
}

fn kind_title_slot() -> gtk::Box {
    let slot = gtk::Box::new(Orientation::Horizontal, 0);
    slot.add_css_class("op-kind-slot");
    slot.set_size_request(PREVIEW_INNER_WIDTH, -1);
    slot.set_hexpand(false);
    slot.set_halign(Align::Fill);
    slot.set_overflow(Overflow::Hidden);
    slot
}

fn kind_title_field(text: &str) -> gtk::Overlay {
    // Same overlay chrome for display and edit so entering rename cannot
    // shift padding or baseline.
    let field = gtk::Overlay::new();
    field.add_css_class("op-kind-edit-field");
    field.set_hexpand(true);
    field.set_halign(Align::Fill);
    field.set_valign(Align::Fill);
    field.set_overflow(Overflow::Hidden);

    let label = gtk::Label::new(None);
    label.add_css_class("op-kind");
    label.add_css_class("op-kind-edit-text");
    label.set_xalign(0.0);
    label.set_yalign(0.5);
    label.set_halign(Align::Fill);
    label.set_valign(Align::Center);
    label.set_hexpand(true);
    label.set_overflow(Overflow::Hidden);
    // Reserve caret width in both modes so showing the caret does not reflow.
    label.set_margin_end(SEARCH_CARET_WIDTH);
    field.set_child(Some(&label));

    let caret = gtk::Box::new(Orientation::Vertical, 0);
    caret.add_css_class("op-search-caret");
    caret.set_size_request(SEARCH_CARET_WIDTH, KIND_CARET_HEIGHT);
    caret.set_halign(Align::Start);
    caret.set_valign(Align::Center);
    caret.set_visible(false);
    field.add_overlay(&caret);

    show_kind_title_display(&field, text);
    field
}

fn show_kind_title_display(field: &gtk::Overlay, text: &str) {
    let Some(label) = find_css(field.upcast_ref(), "op-kind-edit-text")
        .and_then(|w| w.downcast::<gtk::Label>().ok())
    else {
        return;
    };
    let Some(caret) =
        find_css(field.upcast_ref(), "op-search-caret").and_then(|w| w.downcast::<gtk::Box>().ok())
    else {
        return;
    };
    clear_label_selection_attrs(&label);
    caret.set_visible(false);
    caret.set_margin_start(0);
    let max_px = (PREVIEW_INNER_WIDTH - SEARCH_CARET_WIDTH).max(1);
    // GTK size_request is a minimum only — truncate to pixels so the label's
    // natural width cannot stretch the card.
    let shown = ellipsize_to_width(|s| search_text_width(&label, s), text, max_px);
    label.set_width_chars(1);
    label.set_ellipsize(pango::EllipsizeMode::End);
    label.set_text(&shown);
}

fn kind_label_widget(text: &str) -> gtk::Box {
    let slot = kind_title_slot();
    slot.append(&kind_title_field(text));
    slot
}

fn is_select_all_key(key: gdk::Key, ctrl: bool) -> bool {
    ctrl && (key == gdk::Key::a || key == gdk::Key::A)
}

fn parse_hex_rgb_u16(hex: &str) -> (u16, u16, u16) {
    let h = hex.trim().trim_start_matches('#');
    let byte = |i: usize| u8::from_str_radix(h.get(i..i + 2).unwrap_or("00"), 16).unwrap_or(0);
    let widen = |b: u8| ((b as u16) << 8) | b as u16;
    if h.len() >= 6 {
        (widen(byte(0)), widen(byte(2)), widen(byte(4)))
    } else {
        (0x7a7a, 0xa2a2, 0xf7f7)
    }
}

/// Whether a card drag may start. Renaming, or a press that just committed a
/// rename, must not hide the bar via drag.
fn card_drag_prepare_allowed(kind_edit_index: Option<usize>, block_card_drag: bool) -> bool {
    kind_edit_index.is_none() && !block_card_drag
}

/// True when a rename-outside click landed on the bar interior (card or chrome)
/// and should commit without dismissing.
fn kind_edit_click_stays_in_bar(on_card: bool, on_bar: bool) -> bool {
    on_card || on_bar
}

fn selection_byte_range(text: &str, sel_start: usize, sel_end: usize) -> Option<(u32, u32)> {
    let len = text.chars().count();
    let sel_start = sel_start.min(len);
    let sel_end = sel_end.min(len);
    if sel_start >= sel_end {
        return None;
    }
    Some((
        char_byte_index(text, sel_start) as u32,
        char_byte_index(text, sel_end) as u32,
    ))
}

fn build_selection_attrs(
    text: &str,
    sel_start: usize,
    sel_end: usize,
    rgb: (u16, u16, u16),
) -> Option<pango::AttrList> {
    let (start, end) = selection_byte_range(text, sel_start, sel_end)?;
    let attrs = pango::AttrList::new();
    let mut bg = pango::AttrColor::new_background(rgb.0, rgb.1, rgb.2).upcast();
    bg.set_start_index(start);
    bg.set_end_index(end);
    attrs.insert(bg);
    // Match the previous overlay alpha(~0.45) so the accent wash stays soft.
    let mut alpha = pango::AttrInt::new_background_alpha((0.45 * 65535.0) as u16).upcast();
    alpha.set_start_index(start);
    alpha.set_end_index(end);
    attrs.insert(alpha);
    Some(attrs)
}

fn clear_label_selection_attrs(label: &gtk::Label) {
    label.set_attributes(None::<&pango::AttrList>);
}

fn apply_label_selection_attrs(
    label: &gtk::Label,
    text: &str,
    sel_start: usize,
    sel_end: usize,
    rgb: (u16, u16, u16),
) {
    if let Some(attrs) = build_selection_attrs(text, sel_start, sel_end, rgb) {
        label.set_attributes(Some(&attrs));
    } else {
        clear_label_selection_attrs(label);
    }
}

fn sync_text_selection_chrome(
    label: &gtk::Label,
    caret: &gtk::Box,
    text: &str,
    sel_start: usize,
    sel_end: usize,
    cursor: usize,
    editing: bool,
    rgb: (u16, u16, u16),
) {
    let len = text.chars().count();
    let sel_start = sel_start.min(len);
    let sel_end = sel_end.min(len);
    let cursor = cursor.min(len);
    if editing && sel_start < sel_end {
        apply_label_selection_attrs(label, text, sel_start, sel_end, rgb);
        caret.set_visible(false);
    } else {
        clear_label_selection_attrs(label);
        let caret_x = label_caret_x(label, text, cursor);
        caret.set_margin_start(caret_x);
        caret.set_visible(editing);
    }
}

fn kind_edit_range(anchor: usize, cursor: usize) -> (usize, usize) {
    if anchor != cursor {
        (anchor.min(cursor), anchor.max(cursor))
    } else {
        (cursor, cursor)
    }
}

fn kind_edit_viewport(
    measure: impl Fn(&str) -> i32,
    text: &str,
    cursor: usize,
    max_width: i32,
) -> (String, usize) {
    if text.is_empty() {
        return (String::new(), 0);
    }
    let chars: Vec<char> = text.chars().collect();
    let cursor = cursor.min(chars.len());

    let mut scroll_start = cursor;
    let mut scroll_end = cursor;
    let mut best_len = 0usize;
    let mut best_center_dist = usize::MAX;
    for start in 0..=cursor {
        for end in cursor..=chars.len() {
            let len = end - start;
            let slice: String = chars[start..end].iter().collect();
            if measure(&slice) > max_width {
                continue;
            }
            let center_dist = (cursor * 2).abs_diff(start + end);
            if len > best_len || (len == best_len && center_dist < best_center_dist) {
                scroll_start = start;
                scroll_end = end;
                best_len = len;
                best_center_dist = center_dist;
            }
        }
    }
    if best_len == 0 {
        scroll_end = (scroll_start + 1).min(chars.len());
    }

    let visible: String = chars[scroll_start..scroll_end].iter().collect();
    (visible, scroll_start)
}

fn visible_clip_indices(clips: &[Clip], filter: &str) -> Vec<usize> {
    let needle = filter.trim().to_lowercase();
    if needle.is_empty() {
        (0..clips.len()).collect()
    } else {
        clips
            .iter()
            .enumerate()
            .filter(|(_, clip)| clip_matches_filter(clip, &needle))
            .map(|(i, _)| i)
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq)]
enum KeyIntent {
    Dismiss,
    Paste,
    Left,
    Right,
    Home,
    End,
    Delete,
    CycleKeep,
    OpenSearch,
    TypeSearch(char),
    PasteNth(usize),
    Other,
}

fn key_intent(key: gdk::Key, ctrl: bool, searching: bool) -> KeyIntent {
    if key == gdk::Key::Escape {
        return KeyIntent::Dismiss;
    }
    if key == gdk::Key::Return || key == gdk::Key::KP_Enter {
        return KeyIntent::Paste;
    }
    if key == gdk::Key::Left {
        return KeyIntent::Left;
    }
    if key == gdk::Key::Right {
        return KeyIntent::Right;
    }
    if key == gdk::Key::Home {
        return KeyIntent::Home;
    }
    if key == gdk::Key::End {
        return KeyIntent::End;
    }
    if key == gdk::Key::Delete
        || key == gdk::Key::KP_Delete
        || (key == gdk::Key::BackSpace && !searching)
    {
        return KeyIntent::Delete;
    }
    if ctrl && (key == gdk::Key::k || key == gdk::Key::K) {
        return KeyIntent::CycleKeep;
    }
    if ctrl && (key == gdk::Key::f || key == gdk::Key::F || key == gdk::Key::slash) {
        return KeyIntent::OpenSearch;
    }
    if key == gdk::Key::slash && !searching {
        return KeyIntent::OpenSearch;
    }
    if ctrl {
        for (n, k) in [
            gdk::Key::_1,
            gdk::Key::_2,
            gdk::Key::_3,
            gdk::Key::_4,
            gdk::Key::_5,
            gdk::Key::_6,
            gdk::Key::_7,
            gdk::Key::_8,
            gdk::Key::_9,
        ]
        .into_iter()
        .enumerate()
        {
            if key == k {
                return KeyIntent::PasteNth(n);
            }
        }
    }
    if !ctrl && !searching {
        if let Some(ch) = key.to_unicode() {
            if !ch.is_control() {
                return KeyIntent::TypeSearch(ch);
            }
        }
    }
    KeyIntent::Other
}

fn clamp_select(index: usize, n: usize) -> Option<usize> {
    if n == 0 {
        None
    } else {
        Some(index.min(n - 1))
    }
}

fn select_after_delete(selected: usize, len: usize) -> usize {
    if len == 0 {
        0
    } else if selected + 1 >= len {
        selected.saturating_sub(1)
    } else {
        selected
    }
}

fn bar_width_for(output_px: i32) -> i32 {
    (output_px - 2 * SIDE_MARGIN).max(800)
}

/// True when a click in surface coords falls outside the bar strip.
#[cfg(test)]
fn click_is_outside_bar(x: f64, y: f64, bar_x: f64, bar_y: f64, bar_w: f64, bar_h: f64) -> bool {
    x < bar_x || y < bar_y || x >= bar_x + bar_w || y >= bar_y + bar_h
}

/// Monitor logical size `(width, height)` for layout and dismiss hit-testing.
fn output_size() -> (i32, i32) {
    let Some(display) = gdk::Display::default() else {
        return (1400, 900);
    };
    let monitors = display.monitors();
    if monitors.n_items() == 0 {
        return (1400, 900);
    }
    let Some(monitor) = monitors
        .item(0)
        .and_then(|o| o.downcast::<gdk::Monitor>().ok())
    else {
        return (1400, 900);
    };
    let geo = monitor.geometry();
    (geo.width().max(800), geo.height().max(600))
}

fn output_width() -> i32 {
    bar_width_for(output_size().0)
}

fn format_age(ts: i64) -> String {
    format_age_at(ts, chrono::Utc::now().timestamp())
}

fn format_age_at(ts: i64, now: i64) -> String {
    let delta = (now - ts).max(0);
    if delta < 12 {
        return "just now".into();
    }
    if delta < 60 {
        return format!("{delta}s ago");
    }
    if delta < 3600 {
        return format!("{}m ago", delta / 60);
    }
    if delta < 86400 {
        return format!("{}h ago", delta / 3600);
    }
    let days = delta / 86400;
    if days == 1 {
        return "yesterday".into();
    }
    if days < 7 {
        return format!("{days}d ago");
    }
    Local
        .timestamp_opt(ts, 0)
        .single()
        .map(|t| t.format("%b %-d").to_string())
        .unwrap_or_else(|| format!("{days}d ago"))
}

/// Empty strip so glyph ink is not the first painted row of a clip box.
struct ClipDragPayload {
    mime: String,
    bytes: Vec<u8>,
    is_text: bool,
    drag_path: Option<PathBuf>,
}

fn image_drag_png_path(bytes: &[u8]) -> Option<PathBuf> {
    crate::paths::cleanup_drag_temps();
    let dir = crate::paths::runtime_dir();
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_nanos();
    let pid = std::process::id();
    let path = dir.join(format!("drag-{stamp}-{pid}.png"));
    crate::secure_fs::write_private_new_file(&path, bytes).ok()?;
    Some(path)
}

fn clip_drag_payload(
    clip: &Clip,
    read_image: impl FnOnce(&Path) -> Option<Vec<u8>>,
) -> Option<ClipDragPayload> {
    if clip.kind == "image" {
        let path = clip.image_path.as_ref()?;
        let payload = read_image(Path::new(path))?;
        let mime = clip.mime.clone();
        let drag_path = image_drag_png_path(&payload);
        Some(ClipDragPayload {
            mime,
            bytes: payload,
            is_text: false,
            drag_path,
        })
    } else {
        let text = clip.text.as_ref()?;
        Some(ClipDragPayload {
            mime: "text/plain;charset=utf-8".into(),
            bytes: text.as_bytes().to_vec(),
            is_text: true,
            drag_path: None,
        })
    }
}

fn text_drag_mime_types() -> &'static [&'static str] {
    &["text/plain;charset=utf-8", "text/plain"]
}

fn clip_drag_provider(payload: &ClipDragPayload) -> gdk::ContentProvider {
    if payload.is_text {
        let bytes = glib::Bytes::from(&payload.bytes[..]);
        let text = String::from_utf8_lossy(&payload.bytes).into_owned();
        let mut providers = vec![gdk::ContentProvider::for_value(&text.to_value())];
        for mime in text_drag_mime_types() {
            providers.push(gdk::ContentProvider::for_bytes(mime, &bytes));
        }
        gdk::ContentProvider::new_union(&providers)
    } else {
        let bytes = glib::Bytes::from(&payload.bytes[..]);
        let mut providers = Vec::new();
        if let Some(ref path) = payload.drag_path {
            let file = gio::File::for_path(path);
            providers.push(gdk::ContentProvider::for_value(&file.to_value()));
            let uri_list = format!("{}\r\n", file.uri());
            providers.push(gdk::ContentProvider::for_bytes(
                "text/uri-list",
                &glib::Bytes::from(uri_list.as_bytes()),
            ));
        }
        providers.push(gdk::ContentProvider::for_bytes("image/png", &bytes));
        if payload.mime != "image/png" {
            providers.push(gdk::ContentProvider::for_bytes(&payload.mime, &bytes));
        }
        if let Ok(pixbuf) =
            gdk_pixbuf::Pixbuf::from_read(std::io::Cursor::new(payload.bytes.clone()))
        {
            let texture = gdk::Texture::for_pixbuf(&pixbuf);
            providers.push(gdk::ContentProvider::for_value(&texture.to_value()));
        }
        if providers.len() == 1 {
            providers.into_iter().next().unwrap()
        } else {
            gdk::ContentProvider::new_union(&providers)
        }
    }
}

fn should_reopen_after_drag(cancelled: bool, panel_hidden: bool) -> bool {
    cancelled && panel_hidden
}

fn image_drag_provider_parts(payload: &ClipDragPayload) -> usize {
    debug_assert!(!payload.is_text);
    let mut count = 0;
    if payload.drag_path.is_some() {
        count += 2;
    }
    count += 1; // image/png
    if payload.mime != "image/png" {
        count += 1;
    }
    if gdk_pixbuf::Pixbuf::from_read(std::io::Cursor::new(payload.bytes.clone())).is_ok() {
        count += 1;
    }
    count
}

fn ink_gap(px: i32) -> gtk::Box {
    let gap = gtk::Box::new(Orientation::Horizontal, 0);
    gap.set_size_request(1, px);
    gap.set_hexpand(false);
    gap.set_vexpand(false);
    gap
}

fn clip_card(clip: &Clip) -> gtk::Box {
    // Fixed ignores children's natural size for its own request, so a long
    // title cannot widen the tile in the card strip.
    let shell = gtk::Box::new(Orientation::Vertical, 0);
    shell.add_css_class("op-card");
    shell.set_size_request(CARD_WIDTH, CARD_HEIGHT);
    shell.set_hexpand(false);
    shell.set_vexpand(false);
    shell.set_halign(Align::Start);
    shell.set_valign(Align::Center);
    shell.set_overflow(Overflow::Hidden);

    let fixed = gtk::Fixed::new();
    fixed.set_size_request(CARD_WIDTH, CARD_HEIGHT);
    fixed.set_overflow(Overflow::Hidden);

    let content = gtk::Box::new(Orientation::Vertical, 0);
    content.set_size_request(CARD_WIDTH, CARD_HEIGHT);
    content.set_hexpand(false);
    content.set_vexpand(false);
    content.set_overflow(Overflow::Hidden);

    let header = gtk::Box::new(Orientation::Vertical, 2);
    header.add_css_class("op-card-header");
    header.set_overflow(Overflow::Hidden);
    header.set_hexpand(false);
    header.set_size_request(PREVIEW_INNER_WIDTH, -1);
    let kind = kind_label_widget(&clip.display_label());
    let age = gtk::Label::new(Some(&format_age(clip.last_used_at)));
    age.set_xalign(0.0);
    age.add_css_class("op-meta");
    age.set_ellipsize(pango::EllipsizeMode::End);
    age.set_width_chars(1);
    age.set_max_width_chars(PREVIEW_MAX_CHARS);
    header.append(&kind);
    header.append(&age);
    content.append(&header);

    let body = gtk::Box::new(Orientation::Vertical, 0);
    body.add_css_class("op-card-body");
    body.set_vexpand(true);
    body.set_hexpand(false);
    body.set_overflow(Overflow::Hidden);
    body.set_size_request(PREVIEW_INNER_WIDTH, -1);
    if clip.kind == "image" {
        if let Some(ref path) = clip.image_path {
            body.append(&image_preview(path));
        }
    } else {
        // Full clip text can wrap past the tile and shove the footer off-card.
        let raw = clip.text.as_deref().unwrap_or(clip.preview.as_str());
        let text = make_preview(raw, (PREVIEW_MAX_CHARS * PREVIEW_LINES) as usize);
        body.append(&ink_gap(4));
        let label = gtk::Label::new(Some(&text));
        label.set_xalign(0.0);
        label.set_yalign(0.0);
        label.set_valign(Align::Start);
        label.set_wrap(true);
        label.set_wrap_mode(pango::WrapMode::WordChar);
        label.set_natural_wrap_mode(gtk::NaturalWrapMode::Word);
        label.set_ellipsize(pango::EllipsizeMode::End);
        label.set_lines(PREVIEW_LINES);
        label.add_css_class("op-preview");
        label.set_width_request(PREVIEW_INNER_WIDTH);
        label.set_width_chars(1);
        label.set_max_width_chars(PREVIEW_MAX_CHARS);
        label.set_hexpand(false);
        label.set_vexpand(false);
        label.set_overflow(Overflow::Hidden);
        body.append(&label);
    }
    content.append(&body);

    let footer = gtk::Box::new(Orientation::Horizontal, 0);
    footer.add_css_class("op-card-footer");
    footer.set_vexpand(false);
    footer.set_hexpand(false);
    footer.set_size_request(PREVIEW_INNER_WIDTH, -1);
    let chars = gtk::Label::new(Some(&clip.format_chars()));
    chars.set_xalign(0.5);
    chars.add_css_class("op-chars");
    chars.set_halign(Align::Center);
    chars.set_hexpand(true);
    footer.append(&chars);
    content.append(&footer);

    fixed.put(&content, 0.0, 0.0);
    shell.append(&fixed);
    shell
}

fn image_preview(path: &str) -> gtk::Widget {
    match gdk_pixbuf::Pixbuf::from_file_at_scale(path, 190, 140, true) {
        Ok(pixbuf) => {
            let texture = gdk::Texture::for_pixbuf(&pixbuf);
            let picture = gtk::Picture::for_paintable(&texture);
            picture.set_content_fit(gtk::ContentFit::Cover);
            picture.set_can_shrink(true);
            picture.set_hexpand(false);
            picture.set_vexpand(true);
            picture.set_size_request(190, 140);
            picture.set_overflow(Overflow::Hidden);
            picture.upcast()
        }
        Err(_) => {
            let label = gtk::Label::new(Some("Image"));
            label.set_xalign(0.0);
            label.set_yalign(0.0);
            label.add_css_class("op-preview");
            label.upcast()
        }
    }
}

fn shortcuts_popover() -> gtk::Popover {
    let popover = gtk::Popover::new();
    let box_ = gtk::Box::new(Orientation::Vertical, 6);
    box_.add_css_class("op-shortcuts");
    for (key, action) in SHORTCUTS {
        let row = gtk::Box::new(Orientation::Horizontal, 20);
        let key_label = gtk::Label::new(Some(key));
        key_label.set_xalign(0.0);
        key_label.add_css_class("op-shortcut-key");
        key_label.set_width_chars(8);
        let action_label = gtk::Label::new(Some(action));
        action_label.set_xalign(0.0);
        action_label.add_css_class("op-shortcut-action");
        action_label.set_hexpand(true);
        row.append(&key_label);
        row.append(&action_label);
        box_.append(&row);
    }
    let issues = gtk::Label::new(Some(
        "Report issues: https://github.com/pkayokay/omapaste/issues",
    ));
    issues.set_xalign(0.0);
    issues.set_wrap(true);
    issues.set_selectable(true);
    issues.add_css_class("op-issues");
    box_.append(&issues);
    popover.set_has_arrow(false);
    popover.set_child(Some(&box_));
    popover
}

#[cfg(test)]
impl Overlay {
    fn test_clip_len(&self) -> usize {
        self.state.borrow().clips.len()
    }

    fn test_open_search(self: &Rc<Self>) {
        self.open_search("");
    }

    fn test_searching(&self) -> bool {
        self.is_searching()
    }

    fn test_set_search_selection(&self, text: &str, anchor: usize, cursor: usize) {
        self.set_search_selection(text, anchor, cursor, false);
    }

    fn test_search_anchor(&self) -> usize {
        self.search_anchor()
    }

    fn test_search_cursor(&self) -> usize {
        self.search_cursor()
    }

    fn test_search_label_has_selection(&self) -> bool {
        self.search_label.attributes().is_some()
    }

    fn test_selected(&self) -> usize {
        self.state.borrow().selected
    }

    fn test_kind_edit_index(&self) -> Option<usize> {
        self.state.borrow().kind_edit_index
    }

    fn test_block_card_drag(&self) -> bool {
        self.state.borrow().block_card_drag
    }

    fn test_set_block_card_drag(&self, block: bool) {
        self.state.borrow_mut().block_card_drag = block;
    }

    fn test_start_kind_edit(self: &Rc<Self>, index: usize) {
        self.start_kind_edit(index);
    }

    fn test_select(self: &Rc<Self>, index: usize) {
        self.select(index, false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    fn ensure_gtk() -> bool {
        if gtk::is_initialized() {
            return true;
        }
        gtk::init().is_ok()
    }

    #[test]
    fn age_labels() {
        let now = 1_700_000_000;
        assert_eq!(format_age_at(now, now), "just now");
        assert_eq!(format_age_at(now - 11, now), "just now");
        assert_eq!(format_age_at(now - 12, now), "12s ago");
        assert_eq!(format_age_at(now - 59, now), "59s ago");
        assert_eq!(format_age_at(now - 60, now), "1m ago");
        assert_eq!(format_age_at(now - 3599, now), "59m ago");
        assert_eq!(format_age_at(now - 3600, now), "1h ago");
        assert_eq!(format_age_at(now - 86399, now), "23h ago");
        assert_eq!(format_age_at(now - 86400, now), "yesterday");
        assert_eq!(format_age_at(now - 86400 * 2, now), "2d ago");
        assert_eq!(format_age_at(now - 86400 * 6, now), "6d ago");
        let week = format_age_at(now - 86400 * 7, now);
        assert!(!week.ends_with("d ago"), "{week}");
        let older = format_age_at(now - 86400 * 10, now);
        assert!(!older.ends_with("d ago"), "{older}");
        assert!(!older.is_empty());
    }

    #[test]
    fn future_timestamps_are_just_now() {
        let now = 1_700_000_000;
        assert_eq!(format_age_at(now + 60, now), "just now");
    }

    #[test]
    fn bar_width_tracks_output_minus_side_margins() {
        assert_eq!(bar_width_for(1280), 1280 - 2 * SIDE_MARGIN);
        assert_eq!(bar_width_for(1920), 1920 - 2 * SIDE_MARGIN);
        // Scale-up shrinks logical width; still follow the output.
        assert_eq!(bar_width_for(640), 800);
        assert_eq!(bar_width_for(800 + 2 * SIDE_MARGIN), 800);
    }

    #[test]
    fn click_outside_the_bar_is_a_dismissal() {
        let bar = (20.0, 700.0, 800.0, 300.0);
        assert!(click_is_outside_bar(
            400.0, 40.0, bar.0, bar.1, bar.2, bar.3
        ));
        assert!(click_is_outside_bar(
            10.0, 750.0, bar.0, bar.1, bar.2, bar.3
        ));
        assert!(!click_is_outside_bar(
            400.0, 850.0, bar.0, bar.1, bar.2, bar.3
        ));
        assert!(!click_is_outside_bar(
            20.0, 700.0, bar.0, bar.1, bar.2, bar.3
        ));
    }

    #[test]
    fn shortcut_table_covers_core_actions() {
        let keys: Vec<_> = SHORTCUTS.iter().map(|(k, _)| *k).collect();
        assert!(keys.contains(&"← →"));
        assert!(keys.contains(&"Enter"));
        assert!(keys.contains(&"Del"));
        assert!(keys.contains(&"Ctrl+K"));
        assert!(keys.contains(&"Esc"));
        assert!(keys.contains(&"Type"));
        assert!(keys.contains(&"Click"));
        assert!(keys.contains(&"Drag"));
    }

    fn sample_clip(kind: &str, text: Option<&str>, image_path: Option<&Path>) -> Clip {
        Clip {
            id: 1,
            created_at: 0,
            last_used_at: 0,
            keep_preset: "1d".into(),
            keep_until: None,
            mime: if kind == "image" {
                "image/png".into()
            } else {
                "text/plain".into()
            },
            kind: kind.into(),
            text: text.map(str::to_string),
            preview: text.unwrap_or("").into(),
            hash: "x".into(),
            image_path: image_path.map(|p| p.display().to_string()),
            byte_size: 0,
            custom_label: None,
        }
    }

    #[test]
    fn clip_drag_payload_text() {
        let clip = sample_clip("text", Some("hello drag"), None);
        let payload = clip_drag_payload(&clip, |_| None).unwrap();
        assert!(payload.is_text);
        assert_eq!(payload.mime, "text/plain;charset=utf-8");
        assert_eq!(payload.bytes, b"hello drag");
        assert_eq!(text_drag_mime_types().len() + 1, 3);
        assert_eq!(
            content_hash("text", "text/plain", &payload.bytes),
            content_hash("text", "text/plain", b"hello drag")
        );
    }

    #[test]
    fn clip_drag_payload_image() {
        let _lock = crate::env_lock();
        #[cfg(unix)]
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        env::set_var("XDG_RUNTIME_DIR", dir.path());
        let path = dir.path().join("clip.png");
        let png = include_bytes!("../share/sample-images/sample-red.png");
        std::fs::write(&path, png).unwrap();
        let clip = sample_clip("image", None, Some(path.as_path()));
        let payload = clip_drag_payload(&clip, |p| std::fs::read(p).ok()).unwrap();
        assert!(!payload.is_text);
        assert_eq!(payload.mime, "image/png");
        assert_eq!(payload.bytes, png.as_slice());
        assert!(payload.drag_path.as_ref().is_some_and(|p| p.exists()));
        #[cfg(unix)]
        assert_eq!(
            payload
                .drag_path
                .as_ref()
                .unwrap()
                .metadata()
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        assert_eq!(text_drag_mime_types().len(), 2);
        assert_eq!(image_drag_provider_parts(&payload), 4);
        crate::paths::cleanup_drag_temps();
        assert!(payload.drag_path.as_ref().is_none_or(|p| !p.exists()));
        env::remove_var("XDG_RUNTIME_DIR");
    }

    #[test]
    fn image_drag_provider_falls_back_without_temp_file() {
        let payload = ClipDragPayload {
            mime: "image/png".into(),
            bytes: b"not-a-png".to_vec(),
            is_text: false,
            drag_path: None,
        };
        assert_eq!(image_drag_provider_parts(&payload), 1);
    }

    #[test]
    fn clip_drag_payload_skips_missing_image() {
        let clip = sample_clip("image", None, Some(Path::new("/no/such/file.png")));
        assert!(clip_drag_payload(&clip, |_| None).is_none());
    }

    #[test]
    fn clip_drag_payload_skips_text_without_body() {
        let clip = sample_clip("text", None, None);
        assert!(clip_drag_payload(&clip, |_| None).is_none());
    }

    #[test]
    fn text_drag_offers_plain_and_utf8_mimes() {
        assert_eq!(
            text_drag_mime_types(),
            &["text/plain;charset=utf-8", "text/plain"]
        );
    }

    #[test]
    fn reopen_after_drag_only_on_cancel_when_panel_was_hidden() {
        assert!(!should_reopen_after_drag(false, true));
        assert!(!should_reopen_after_drag(false, false));
        assert!(!should_reopen_after_drag(true, false));
        assert!(should_reopen_after_drag(true, true));
    }

    #[test]
    fn card_drag_threshold_is_above_the_gtk_default() {
        assert!(CARD_DRAG_THRESHOLD_PX >= 20);
    }

    #[test]
    fn key_intents_match_the_bar() {
        assert_eq!(
            key_intent(gdk::Key::Escape, false, false),
            KeyIntent::Dismiss
        );
        assert_eq!(
            key_intent(gdk::Key::Escape, false, true),
            KeyIntent::Dismiss
        );
        assert_eq!(key_intent(gdk::Key::Return, false, false), KeyIntent::Paste);
        assert_eq!(
            key_intent(gdk::Key::KP_Enter, false, false),
            KeyIntent::Paste
        );
        assert_eq!(key_intent(gdk::Key::Left, false, false), KeyIntent::Left);
        assert_eq!(key_intent(gdk::Key::Right, false, false), KeyIntent::Right);
        assert_eq!(key_intent(gdk::Key::Home, false, false), KeyIntent::Home);
        assert_eq!(key_intent(gdk::Key::End, false, false), KeyIntent::End);
        assert_eq!(key_intent(gdk::Key::Delete, false, true), KeyIntent::Delete);
        assert_eq!(
            key_intent(gdk::Key::KP_Delete, false, true),
            KeyIntent::Delete
        );
        assert_eq!(
            key_intent(gdk::Key::BackSpace, false, false),
            KeyIntent::Delete
        );
        assert_eq!(
            key_intent(gdk::Key::BackSpace, false, true),
            KeyIntent::Other
        );
        assert_eq!(key_intent(gdk::Key::k, true, false), KeyIntent::CycleKeep);
        assert_eq!(key_intent(gdk::Key::K, true, false), KeyIntent::CycleKeep);
        assert_eq!(key_intent(gdk::Key::f, true, false), KeyIntent::OpenSearch);
        assert_eq!(
            key_intent(gdk::Key::slash, true, true),
            KeyIntent::OpenSearch
        );
        assert_eq!(
            key_intent(gdk::Key::slash, false, false),
            KeyIntent::OpenSearch
        );
        assert_eq!(key_intent(gdk::Key::slash, false, true), KeyIntent::Other);
        assert_eq!(
            key_intent(gdk::Key::_1, true, false),
            KeyIntent::PasteNth(0)
        );
        assert_eq!(
            key_intent(gdk::Key::_9, true, false),
            KeyIntent::PasteNth(8)
        );
        assert_eq!(
            key_intent(gdk::Key::a, false, false),
            KeyIntent::TypeSearch('a')
        );
        assert_eq!(key_intent(gdk::Key::a, false, true), KeyIntent::Other);
        assert_eq!(key_intent(gdk::Key::a, true, false), KeyIntent::Other);
    }

    #[test]
    fn search_text_edits_insert_and_backspace_at_cursor() {
        assert_eq!(entry_edit_insert("", 0, 0, 'a'), ("a".into(), 1));
        assert_eq!(entry_edit_insert("ab", 2, 2, 'c'), ("abc".into(), 3));
        assert_eq!(entry_edit_insert("ac", 1, 1, 'b'), ("abc".into(), 2));
        assert_eq!(entry_edit_backspace("abc", 3, 3), ("ab".into(), 2));
        assert_eq!(entry_edit_backspace("", 0, 0), ("".into(), 0));
    }

    #[test]
    fn search_text_pop_word_deletes_the_previous_token() {
        assert_eq!(search_text_pop_word("hello world"), "hello");
        assert_eq!(search_text_pop_word("one two three"), "one two");
        assert_eq!(search_text_pop_word("solo"), "");
        assert_eq!(search_text_pop_word(""), "");
        assert_eq!(search_text_pop_word("ab cd  "), "ab");
    }

    #[test]
    fn visible_clip_indices_match_store_search() {
        let clips = vec![
            sample_clip("text", Some("hello world"), None),
            sample_clip("text", Some("goodbye"), None),
            sample_clip("text", Some("HELLO again"), None),
        ];
        assert_eq!(visible_clip_indices(&clips, ""), vec![0, 1, 2]);
        assert_eq!(visible_clip_indices(&clips, "hello"), vec![0, 2]);
        assert_eq!(visible_clip_indices(&clips, "xyz"), Vec::<usize>::new());
    }

    #[test]
    fn type_to_search_accepts_non_ascii() {
        let key = gdk::Key::from_name("eacute").expect("eacute key");
        assert_eq!(key.to_unicode(), Some('é'));
        assert_eq!(key_intent(key, false, false), KeyIntent::TypeSearch('é'));
        assert_eq!(key_intent(key, false, true), KeyIntent::Other);
    }

    #[test]
    fn selection_clamps_and_delete_keeps_a_neighbor() {
        assert_eq!(clamp_select(0, 0), None);
        assert_eq!(clamp_select(9, 3), Some(2));
        assert_eq!(clamp_select(0, 3), Some(0));
        assert_eq!(select_after_delete(0, 1), 0);
        assert_eq!(select_after_delete(4, 5), 3);
        assert_eq!(select_after_delete(2, 5), 2);
        assert_eq!(select_after_delete(0, 0), 0);
    }

    #[test]
    #[ignore = "needs a display; run with cargo test -- --ignored --test-threads=1"]
    fn overlay_builds_and_opens_search() {
        if !ensure_gtk() {
            return;
        }
        let app = Application::builder()
            .application_id("io.github.pkayokay.omapaste.tests")
            .flags(gtk::gio::ApplicationFlags::NON_UNIQUE)
            .build();
        app.register(gtk::gio::Cancellable::NONE).unwrap();
        let dir = tempfile::TempDir::new().unwrap();
        let store = Rc::new(Store::open(&dir.path().join("db")).unwrap());
        store
            .add(
                "text",
                "text/plain",
                b"hello",
                Some("hello"),
                "hello",
                None,
                "1d",
                50,
                None,
            )
            .unwrap();
        let overlay = Overlay::new(&app, Rc::clone(&store), Config::default(), Rc::new(|_| {}));
        overlay.refresh(false);
        assert_eq!(overlay.test_clip_len(), 1);
        assert!(!overlay.test_searching());
        overlay.test_open_search();
        assert!(overlay.test_searching());
        assert!(!overlay.is_open());
        let query = "hello filter";
        overlay.test_set_search_selection(query, 0, query.chars().count());
        assert_eq!(overlay.test_search_anchor(), 0);
        assert_eq!(overlay.test_search_cursor(), query.chars().count());
        assert!(overlay.test_search_label_has_selection());
        if overlay.layer_shell {
            let win: &gtk::Widget = overlay.window.upcast_ref();
            assert!(
                find_css(win, "op-dismiss").is_some(),
                "fullscreen dismiss catcher should exist so clicks outside the bar close it"
            );
        }

        let clip = Clip {
            id: 1,
            created_at: 0,
            last_used_at: 0,
            keep_preset: "1d".into(),
            keep_until: None,
            mime: "text/plain".into(),
            kind: "text".into(),
            text: Some("Omarchy. Bottom-of-screen clip bar inspired by Paste for Mac".into()),
            preview: "Omarchy. Bottom-of-screen clip bar inspired by Paste for Mac".into(),
            hash: "x".into(),
            image_path: None,
            byte_size: 60,
            custom_label: None,
        };
        let card = clip_card(&clip);
        let preview = find_css(card.upcast_ref(), "op-preview").expect("preview label");
        let label = preview
            .downcast::<gtk::Label>()
            .expect("preview is a label");
        assert!(label.wraps());
        assert_eq!(label.width_chars(), 1);
        assert_eq!(label.max_width_chars(), PREVIEW_MAX_CHARS);
        assert_eq!(label.width_request(), PREVIEW_INNER_WIDTH);
        assert_eq!(label.natural_wrap_mode(), gtk::NaturalWrapMode::Word);
        assert_eq!(label.overflow(), Overflow::Hidden);
        assert_eq!(label.yalign(), 0.0);
        assert!(!label.vexpands());
        assert_eq!(label.lines(), PREVIEW_LINES);
        let kind_slot = find_css(card.upcast_ref(), "op-kind-slot").expect("kind slot");
        assert_eq!(kind_slot.width_request(), PREVIEW_INNER_WIDTH);
        assert_eq!(kind_slot.overflow(), Overflow::Hidden);
        let kind = find_css(card.upcast_ref(), "op-kind-edit-text").expect("kind title");
        let kind = kind.downcast::<gtk::Label>().expect("kind is a label");
        assert_eq!(kind.text().as_str(), "Text");
        assert_eq!(kind.ellipsize(), pango::EllipsizeMode::End);
        assert_eq!(kind.width_chars(), 1);
        assert_eq!(kind.margin_end(), SEARCH_CARET_WIDTH);
        let caret = find_css(card.upcast_ref(), "op-search-caret")
            .and_then(|w| w.downcast::<gtk::Box>().ok())
            .expect("kind caret");
        assert!(!caret.is_visible());
        let long = Clip {
            custom_label: Some("A very long custom title that must not widen the card".into()),
            ..clip.clone()
        };
        let long_card = clip_card(&long);
        assert_eq!(long_card.width_request(), CARD_WIDTH);
        assert_eq!(long_card.overflow(), Overflow::Hidden);
        let long_slot = find_css(long_card.upcast_ref(), "op-kind-slot").expect("long kind slot");
        assert_eq!(long_slot.width_request(), PREVIEW_INNER_WIDTH);
        let long_kind = find_css(long_card.upcast_ref(), "op-kind-edit-text")
            .and_then(|w| w.downcast::<gtk::Label>().ok())
            .expect("long kind label");
        assert!(
            long_kind.text().ends_with('…'),
            "long title should be pixel-truncated: {}",
            long_kind.text()
        );
        assert!(long_kind.text().chars().count() < long.display_label().chars().count());
        assert_eq!(card.overflow(), Overflow::Hidden);
        assert!(
            find_css(card.upcast_ref(), "op-chars").is_some(),
            "footer with char count must stay on the card"
        );

        store
            .add(
                "text",
                "text/plain",
                b"two",
                Some("two"),
                "two",
                None,
                "1d",
                50,
                None,
            )
            .unwrap();
        overlay.refresh(false);
        assert_eq!(overlay.test_clip_len(), 2);
        overlay.test_start_kind_edit(0);
        assert_eq!(overlay.test_kind_edit_index(), Some(0));
        overlay.test_set_block_card_drag(true);
        assert!(!card_drag_prepare_allowed(
            overlay.test_kind_edit_index(),
            overlay.test_block_card_drag()
        ));
        overlay.test_select(1);
        assert_eq!(overlay.test_kind_edit_index(), None);
        assert_eq!(overlay.test_selected(), 1);
    }

    #[test]
    fn card_preview_text_fits_the_tile() {
        let long = "word ".repeat(200);
        let shown = make_preview(&long, (PREVIEW_MAX_CHARS * PREVIEW_LINES) as usize);
        assert!(shown.chars().count() <= (PREVIEW_MAX_CHARS * PREVIEW_LINES) as usize);
        assert!(shown.ends_with('…'), "{shown}");
    }

    fn find_css(widget: &gtk::Widget, class: &str) -> Option<gtk::Widget> {
        super::find_css(widget, class)
    }

    #[test]
    fn clip_filter_matches_custom_label() {
        let mut clip = sample_clip("text", Some("hello"), None);
        clip.custom_label = Some("todo item".into());
        assert!(clip_matches_filter(&clip, "todo"));
        assert!(!clip_matches_filter(&clip, "world"));
    }

    #[test]
    fn ellipsize_to_width_fits_inside_the_pixel_budget() {
        let measure = |s: &str| s.chars().count() as i32;
        assert_eq!(ellipsize_to_width(measure, "hello", 10), "hello");
        assert_eq!(ellipsize_to_width(measure, "hello world", 8), "hello w…");
        assert_eq!(ellipsize_to_width(measure, "hello world", 1), "…");
        assert!(ellipsize_to_width(measure, "abcdefghij", 5).ends_with('…'));
        assert!(ellipsize_to_width(measure, "abcdefghij", 5).chars().count() <= 5);
    }

    #[test]
    fn kind_edit_range_normalizes_reversed_selection() {
        assert_eq!(kind_edit_range(0, 4), (0, 4));
        assert_eq!(kind_edit_range(4, 0), (0, 4));
        assert_eq!(kind_edit_range(2, 2), (2, 2));
    }

    #[test]
    fn card_drag_prepare_allowed_blocks_while_renaming_or_after_commit_press() {
        assert!(!card_drag_prepare_allowed(Some(0), false));
        assert!(!card_drag_prepare_allowed(None, true));
        assert!(card_drag_prepare_allowed(None, false));
        assert!(!card_drag_prepare_allowed(Some(1), true));
    }

    #[test]
    fn kind_edit_click_stays_in_bar_only_for_cards_and_chrome() {
        assert!(kind_edit_click_stays_in_bar(true, false));
        assert!(kind_edit_click_stays_in_bar(false, true));
        assert!(kind_edit_click_stays_in_bar(true, true));
        assert!(!kind_edit_click_stays_in_bar(false, false));
    }

    #[test]
    fn selection_byte_range_spans_full_string_for_select_all() {
        let text = "café 🦀";
        let len = text.chars().count();
        let (start, end) = selection_byte_range(text, 0, len).unwrap();
        assert_eq!(start, 0);
        assert_eq!(end, text.len() as u32);
    }

    #[test]
    fn selection_byte_range_is_empty_for_collapsed_caret() {
        assert!(selection_byte_range("hello", 2, 2).is_none());
    }

    #[test]
    fn build_selection_attrs_is_none_for_collapsed_caret() {
        assert!(build_selection_attrs("hello", 2, 2, (0x7a7a, 0xa2a2, 0xf7f7)).is_none());
    }

    #[test]
    fn build_selection_attrs_is_some_for_non_empty_ranges() {
        let text = "café 🦀";
        let len = text.chars().count();
        assert!(build_selection_attrs(text, 0, len, (0x7a7a, 0xa2a2, 0xf7f7)).is_some());
        assert!(build_selection_attrs(text, 1, 4, (0x7a7a, 0xa2a2, 0xf7f7)).is_some());
    }

    #[test]
    fn select_all_key_matches_ctrl_a() {
        assert!(is_select_all_key(gdk::Key::a, true));
        assert!(is_select_all_key(gdk::Key::A, true));
        assert!(!is_select_all_key(gdk::Key::a, false));
        assert!(!is_select_all_key(gdk::Key::b, true));
    }

    #[test]
    fn parse_hex_rgb_u16_widens_bytes_for_pango() {
        assert_eq!(parse_hex_rgb_u16("#7aa2f7"), (0x7a7a, 0xa2a2, 0xf7f7));
        assert_eq!(parse_hex_rgb_u16("abcdef"), (0xabab, 0xcdcd, 0xefef));
    }

    #[test]
    fn char_byte_index_covers_full_utf8_select_all_range() {
        let text = "café 🦀";
        let len = text.chars().count();
        assert_eq!(char_byte_index(text, 0), 0);
        assert_eq!(char_byte_index(text, len), text.len());
        assert_eq!(char_byte_index(text, 4), "café".len());
    }

    #[test]
    fn kind_edit_text_helpers_replace_selection_and_move() {
        assert_eq!(kind_edit_range(0, 4), (0, 4));
        assert_eq!(kind_edit_range(2, 2), (2, 2));
        assert_eq!(entry_edit_insert("Text", 0, 4, 'N'), ("N".into(), 1));
        assert_eq!(entry_edit_insert("ab", 1, 1, 'x'), ("axb".into(), 2));
        assert_eq!(entry_edit_backspace("abc", 1, 1), ("bc".into(), 0));
        assert_eq!(entry_edit_backspace("abc", 0, 2), ("c".into(), 0));
        assert_eq!(
            entry_edit_backspace_word("hello world", 5, 5),
            (" world".into(), 0)
        );
        assert_eq!(
            entry_edit_backspace_word("hello world", 11, 11),
            ("hello".into(), 5)
        );
        assert_eq!(entry_edit_delete("abc", 1, 1), ("ac".into(), 1));
        assert_eq!(entry_edit_move("abc", 0, 3, 0, 1), 3);
        assert_eq!(entry_edit_move("abc", 0, 3, 0, -1), 0);
        assert_eq!(entry_edit_move_word("hello world", 11, false), 6);
        assert_eq!(entry_edit_move_word("hello world", 6, false), 0);
        assert_eq!(entry_edit_move_word("hello world", 0, true), 6);
        assert_eq!(entry_edit_move_word("hello world", 6, true), 11);
        assert_eq!(entry_edit_move_word("ab  cd", 2, true), 4);
    }

    #[test]
    fn kind_edit_viewport_scrolls_long_text_to_the_cursor() {
        let measure = |s: &str| s.chars().count() as i32;
        let text = "abcdefghijklmnopqrstuvwxyz";
        let (visible, start) = kind_edit_viewport(measure, text, 0, 10);
        assert_eq!(visible, "abcdefghij");
        assert_eq!(start, 0);

        let (visible, start) = kind_edit_viewport(measure, text, text.len(), 10);
        assert_eq!(visible, "qrstuvwxyz");
        assert_eq!(start, 16);

        let (visible, start) = kind_edit_viewport(measure, text, 13, 10);
        assert_eq!(visible, "ijklmnopqr");
        assert_eq!(start, 8);
    }

    #[test]
    fn char_byte_index_matches_rust_char_offsets() {
        assert_eq!(char_byte_index("hello", 0), 0);
        assert_eq!(char_byte_index("hello", 5), 5);
        assert_eq!(char_byte_index("é", 1), 2);
    }

    #[test]
    fn preview_max_chars_fits_inside_the_card() {
        let inner = CARD_WIDTH - 2 * CARD_BORDER - 2 * CARD_BODY_PAD_X;
        assert!(PREVIEW_MAX_CHARS > 0);
        assert!(PREVIEW_MAX_CHARS * PREVIEW_CELL_PX <= inner);
        assert!(PREVIEW_MAX_CHARS < 28);
    }

    #[test]
    fn seed_lines_fit_the_card_wrap() {
        let max = PREVIEW_MAX_CHARS as usize;
        for (text, _) in crate::store::SEED_CLIPS {
            for line in text.lines() {
                let n = line.chars().count();
                if n <= max {
                    continue;
                }
                for token in line.split_whitespace() {
                    if token.chars().count() <= max {
                        continue;
                    }
                    assert!(
                        token.starts_with("http://") || token.starts_with("https://"),
                        "seed {token:?} is longer than {max} chars and is not a URL"
                    );
                }
            }
        }
    }
}
