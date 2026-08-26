use std::cell::RefCell;
use std::path::Path;
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
use crate::store::{content_hash, next_keep, Clip, Store};
use crate::theme::{css_for, load_theme};

const BAR_HEIGHT: i32 = 304;
const CARD_WIDTH: i32 = 210;
const CARD_HEIGHT: i32 = 236;
const CARD_BORDER: i32 = 1;
const CARD_BODY_PAD_X: i32 = 10;
/// 12px JetBrains Mono is ~7px/cell; 1.25 scale hinting can be ~8px.
const PREVIEW_CELL_PX: i32 = 8;
const PREVIEW_MAX_CHARS: i32 =
    (CARD_WIDTH - 2 * CARD_BORDER - 2 * CARD_BODY_PAD_X) / PREVIEW_CELL_PX;
const VISIBLE_MARGIN: i32 = 14;
const SLIDE_PX: i32 = BAR_HEIGHT + 24;
const ANIM_DURATION: f64 = 0.220;

const SHORTCUTS: &[(&str, &str)] = &[
    ("← →", "Select"),
    ("Enter", "Paste"),
    ("Click", "Copy"),
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
}

pub struct Overlay {
    pub window: gtk::Window,
    store: Rc<Store>,
    config: Config,
    on_copy: Rc<dyn Fn(&str)>,
    bar: gtk::Box,
    brand: gtk::Box,
    search: gtk::Entry,
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
        window.set_default_size(width, BAR_HEIGHT);
        window.set_size_request(width, BAR_HEIGHT);

        let layer_shell = gtk4_layer_shell::is_supported();
        if layer_shell {
            window.init_layer_shell();
            window.set_namespace(Some("omapaste"));
            window.set_layer(Layer::Overlay);
            window.set_anchor(Edge::Bottom, true);
            window.set_anchor(Edge::Left, true);
            window.set_anchor(Edge::Right, true);
            window.set_anchor(Edge::Top, false);
            window.set_exclusive_zone(0);
            window.set_margin(Edge::Left, 18);
            window.set_margin(Edge::Right, 18);
            window.set_margin(Edge::Bottom, VISIBLE_MARGIN);
            window.set_keyboard_mode(KeyboardMode::None);
        }

        let css = gtk::CssProvider::new();
        let bar = gtk::Box::new(Orientation::Vertical, 8);
        bar.add_css_class("op-bar");
        bar.set_hexpand(true);
        bar.set_halign(Align::Fill);

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

        let search_open_btn = gtk::Button::new();
        search_open_btn.set_has_frame(false);
        search_open_btn.set_icon_name("system-search-symbolic");
        search_open_btn.set_tooltip_text(Some("Search"));
        search_open_btn.add_css_class("op-icon-btn");
        search_open_btn.set_valign(Align::Center);

        let search = gtk::Entry::new();
        search.set_placeholder_text(Some("Search clips"));
        search.add_css_class("op-search");
        search.set_hexpand(true);
        search.set_vexpand(false);
        search.set_valign(Align::Center);
        search.set_size_request(-1, 28);
        search.set_icon_from_icon_name(
            gtk::EntryIconPosition::Primary,
            Some("system-search-symbolic"),
        );
        search.set_icon_from_icon_name(
            gtk::EntryIconPosition::Secondary,
            Some("edit-clear-symbolic"),
        );
        search.set_icon_tooltip_text(gtk::EntryIconPosition::Secondary, Some("Close search"));

        let shortcuts_btn = gtk::Button::new();
        shortcuts_btn.set_has_frame(false);
        shortcuts_btn.set_icon_name("input-keyboard-symbolic");
        shortcuts_btn.set_tooltip_text(Some("Shortcuts"));
        shortcuts_btn.add_css_class("op-icon-btn");
        shortcuts_btn.set_valign(Align::Center);
        let shortcuts = shortcuts_popover();
        shortcuts.set_parent(&shortcuts_btn);

        let issues_btn = gtk::Button::new();
        issues_btn.set_has_frame(false);
        issues_btn.set_icon_name("help-about-symbolic");
        issues_btn.set_tooltip_text(Some("Report an issue"));
        issues_btn.add_css_class("op-icon-btn");
        issues_btn.set_valign(Align::Center);
        issues_btn.connect_clicked(|_| {
            let _ = std::process::Command::new("xdg-open")
                .arg(ISSUES_URL)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn();
        });

        header.append(&brand);
        header.append(&search);
        header.append(&search_open_btn);
        header.append(&shortcuts_btn);
        header.append(&issues_btn);

        let scroller = gtk::ScrolledWindow::new();
        scroller.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Never);
        scroller.set_hexpand(true);
        scroller.set_size_request(-1, CARD_HEIGHT + 8);
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
        bar.set_size_request(width, BAR_HEIGHT);
        bar.set_valign(Align::Start);
        bar.set_vexpand(false);
        bar.set_margin_top(SLIDE_PX);

        let stage = gtk::Box::new(Orientation::Horizontal, 0);
        stage.set_size_request(width, BAR_HEIGHT);
        let clipper = gtk::Overlay::new();
        clipper.set_overflow(Overflow::Hidden);
        clipper.set_hexpand(true);
        clipper.set_size_request(width, BAR_HEIGHT);
        clipper.set_child(Some(&stage));
        clipper.add_overlay(&bar);
        window.set_child(Some(&clipper));

        let ov = Rc::new(Self {
            window: window.clone(),
            store,
            config,
            on_copy,
            bar,
            brand,
            search: search.clone(),
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
            }),
        });
        ov.sync_search_chrome();
        ov.apply_theme();

        {
            let o = ov.clone();
            ov.window.connect_close_request(move |_| {
                o.hide_rc();
                glib::Propagation::Stop
            });
        }
        {
            let o = ov.clone();
            search_open_btn.connect_clicked(move |_| o.open_search(""));
        }
        {
            let o = ov.clone();
            search.connect_changed(move |e| {
                o.state.borrow_mut().filter = e.text().to_string();
                o.state.borrow_mut().selected = 0;
                o.refresh_rc(false);
            });
        }
        {
            let o = ov.clone();
            search.connect_icon_press(move |_, pos| {
                if pos == gtk::EntryIconPosition::Secondary {
                    o.close_search_rc();
                }
            });
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
            let keys = gtk::EventControllerKey::new();
            keys.set_propagation_phase(gtk::PropagationPhase::Capture);
            let o = ov.clone();
            keys.connect_key_pressed(move |_, key, _, state| o.on_key(key, state));
            ov.search.add_controller(keys);
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

    fn apply_theme(&self) {
        let theme = load_theme();
        let extra = format!(
            "\n.op-card {{\n  min-width: {CARD_WIDTH}px;\n  min-height: {CARD_HEIGHT}px;\n}}\n"
        );
        self.css.load_from_string(&(css_for(&theme) + &extra));
        if let Some(display) = gdk::Display::default() {
            gtk::style_context_add_provider_for_display(
                &display,
                &self.css,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
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
        self.animate_slide_rc(0.0, None);
        let win = self.window.clone();
        glib::idle_add_local_once(move || {
            win.grab_focus();
        });
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
        let clips = self.store.list(&filter, None).unwrap_or_default();
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
        let total = self.store.count().unwrap_or(0);
        let shown = clips.len();
        if !filter.is_empty() {
            self.count_label.set_text(&format!("{shown} / {total}"));
        } else {
            self.count_label.set_text(&format!(
                "{shown} clip{}",
                if shown == 1 { "" } else { "s" }
            ));
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
        let this = Rc::clone(self);
        glib::idle_add_local_once(move || {
            this.scroll_selected();
        });
    }

    fn bind_card_clicks(self: &Rc<Self>) {
        let cards = self.state.borrow().cards.clone();
        for (index, card) in cards.into_iter().enumerate() {
            let click = gtk::GestureClick::new();
            let this = Rc::clone(self);
            click.connect_pressed(move |_, n_press, _, _| {
                this.select(index, true);
                if n_press >= 2 {
                    this.paste_selected();
                }
            });
            card.add_controller(click);
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
        self.search.set_visible(open);
        self.search_open_btn.set_visible(!open);
        self.brand.set_hexpand(!open);
    }

    fn open_search(&self, prefix: &str) {
        self.state.borrow_mut().search_open = true;
        self.sync_search_chrome();
        if !prefix.is_empty() {
            let mut text = self.search.text().to_string();
            text.push_str(prefix);
            self.search.set_text(&text);
            self.search.set_position(-1);
        }
        self.search.grab_focus();
    }

    fn close_search_rc(self: &Rc<Self>) {
        if !self.search.text().is_empty() {
            self.search.set_text("");
        } else if !self.state.borrow().filter.is_empty() {
            self.state.borrow_mut().filter.clear();
            self.refresh_rc(false);
        }
        self.state.borrow_mut().search_open = false;
        self.sync_search_chrome();
        self.window.grab_focus();
    }

    fn on_scroll(&self, dx: f64, dy: f64) {
        let adj = self.scroller.hadjustment();
        adj.set_value(adj.value() + (dx + dy) * 40.0);
    }

    fn on_key(self: &Rc<Self>, key: gdk::Key, state: gdk::ModifierType) -> glib::Propagation {
        let ctrl = state.contains(gdk::ModifierType::CONTROL_MASK);
        match key_intent(key, ctrl, self.is_searching()) {
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
                let i = self.state.borrow().selected;
                self.select(i.saturating_sub(1), true);
                glib::Propagation::Stop
            }
            KeyIntent::Right => {
                let i = self.state.borrow().selected;
                self.select(i + 1, true);
                glib::Propagation::Stop
            }
            KeyIntent::Home => {
                self.select(0, true);
                glib::Propagation::Stop
            }
            KeyIntent::End => {
                let n = self.state.borrow().clips.len();
                if n > 0 {
                    self.select(n - 1, true);
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

fn output_width() -> i32 {
    let Some(display) = gdk::Display::default() else {
        return 1400;
    };
    let monitors = display.monitors();
    if monitors.n_items() == 0 {
        return 1400;
    }
    let Some(monitor) = monitors
        .item(0)
        .and_then(|o| o.downcast::<gdk::Monitor>().ok())
    else {
        return 1400;
    };
    let geo = monitor.geometry();
    (geo.width() - 36).max(800)
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

fn clip_card(clip: &Clip) -> gtk::Box {
    let card = gtk::Box::new(Orientation::Vertical, 0);
    card.add_css_class("op-card");
    card.set_size_request(CARD_WIDTH, CARD_HEIGHT);
    card.set_hexpand(false);
    card.set_vexpand(false);
    card.set_halign(Align::Start);
    card.set_valign(Align::Center);
    card.set_overflow(Overflow::Hidden);

    let header = gtk::Box::new(Orientation::Vertical, 2);
    header.add_css_class("op-card-header");
    let kind = gtk::Label::new(Some(&clip.kind_label()));
    kind.set_xalign(0.0);
    kind.add_css_class("op-kind");
    let age = gtk::Label::new(Some(&format_age(clip.last_used_at)));
    age.set_xalign(0.0);
    age.add_css_class("op-meta");
    header.append(&kind);
    header.append(&age);
    card.append(&header);

    let body = gtk::Box::new(Orientation::Vertical, 0);
    body.add_css_class("op-card-body");
    body.set_vexpand(true);
    body.set_hexpand(true);
    if clip.kind == "image" {
        if let Some(ref path) = clip.image_path {
            body.append(&image_preview(path));
        }
    } else {
        let text = clip.text.clone().unwrap_or_else(|| clip.preview.clone());
        let label = gtk::Label::new(Some(&text));
        label.set_xalign(0.0);
        label.set_yalign(0.0);
        label.set_wrap(true);
        label.set_wrap_mode(pango::WrapMode::WordChar);
        label.set_natural_wrap_mode(gtk::NaturalWrapMode::Word);
        label.set_ellipsize(pango::EllipsizeMode::End);
        label.set_lines(8);
        label.add_css_class("op-preview");
        // Cap wrap to the padded 210px card. 28ch at this font overflows and
        // Overflow::Hidden clips the last glyphs on each line.
        label.set_max_width_chars(PREVIEW_MAX_CHARS);
        label.set_hexpand(true);
        label.set_vexpand(true);
        body.append(&label);
    }
    card.append(&body);

    let footer = gtk::Box::new(Orientation::Horizontal, 0);
    footer.add_css_class("op-card-footer");
    let chars = gtk::Label::new(Some(&clip.format_chars()));
    chars.set_xalign(0.5);
    chars.add_css_class("op-chars");
    chars.set_halign(Align::Center);
    chars.set_hexpand(true);
    footer.append(&chars);
    card.append(&footer);
    card
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

    fn test_open_search(&self) {
        self.open_search("");
    }

    fn test_searching(&self) -> bool {
        self.is_searching()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn shortcut_table_covers_core_actions() {
        let keys: Vec<_> = SHORTCUTS.iter().map(|(k, _)| *k).collect();
        assert!(keys.contains(&"← →"));
        assert!(keys.contains(&"Enter"));
        assert!(keys.contains(&"Del"));
        assert!(keys.contains(&"Ctrl+K"));
        assert!(keys.contains(&"Esc"));
        assert!(keys.contains(&"Type"));
        assert!(keys.contains(&"Click"));
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
        if gtk::init().is_err() {
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
        let overlay = Overlay::new(&app, store, Config::default(), Rc::new(|_| {}));
        overlay.refresh(false);
        assert_eq!(overlay.test_clip_len(), 1);
        assert!(!overlay.test_searching());
        overlay.test_open_search();
        assert!(overlay.test_searching());
        assert!(!overlay.is_open());

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
        };
        let card = clip_card(&clip);
        let preview = find_css(card.upcast_ref(), "op-preview").expect("preview label");
        let label = preview
            .downcast::<gtk::Label>()
            .expect("preview is a label");
        assert!(label.wraps());
        assert_eq!(label.width_chars(), -1);
        assert_eq!(label.max_width_chars(), PREVIEW_MAX_CHARS);
        assert_eq!(label.natural_wrap_mode(), gtk::NaturalWrapMode::Word);
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
