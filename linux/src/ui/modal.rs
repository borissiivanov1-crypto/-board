//! Popups drawn inside the window (dimmed backdrop, rounded panel, pop-in animation), the toast,
//! and the form building blocks shared by the popups and Settings.

use super::draw::{ease_soft, ease_spring, Scaler};
use gtk::glib;
use gtk::prelude::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};

struct Entry {
    wrap: gtk::Box,
    on_close: Option<Box<dyn Fn()>>,
    name: String,
}

/// The popup layer of a window.
#[derive(Clone)]
pub struct Modals {
    root: gtk::Overlay,
    open: Rc<RefCell<Vec<Entry>>>,
    pub glass: Rc<Cell<bool>>,
    /// clicks are ignored until then (the press that opened a popup doesn't press its buttons)
    pub ign: Rc<Cell<Option<Instant>>>,
}

pub struct ModalOpts {
    pub width: i32,
    /// at the top (Settings) instead of the middle
    pub top: bool,
    pub name: &'static str,
}

impl Default for ModalOpts {
    fn default() -> Self {
        ModalOpts { width: 340, top: false, name: "" }
    }
}

impl Modals {
    pub fn new(root: &gtk::Overlay) -> Modals {
        Modals { root: root.clone(), open: Rc::default(), glass: Rc::new(Cell::new(false)), ign: Rc::new(Cell::new(None)) }
    }

    pub fn ignoring(&self) -> bool {
        self.ign.get().map(|t| Instant::now() < t).unwrap_or(false)
    }

    pub fn ignore_for(&self, ms: u64) {
        self.ign.set(Some(Instant::now() + Duration::from_millis(ms)));
    }

    pub fn is_open(&self, name: &str) -> bool {
        self.open.borrow().iter().any(|e| e.name == name)
    }

    pub fn any_open(&self) -> bool {
        !self.open.borrow().is_empty()
    }

    /// Shows a panel; returns the panel box to fill (class "modal").
    pub fn show(&self, opts: ModalOpts, on_close: Option<Box<dyn Fn()>>) -> gtk::Box {
        if !opts.name.is_empty() {
            self.close(opts.name);
        }
        let panel = gtk::Box::new(gtk::Orientation::Vertical, 12);
        panel.add_css_class("modal");
        panel.set_size_request(opts.width, -1);
        let scaler = Scaler::new(&panel);
        scaler.set_halign(gtk::Align::Center);
        scaler.set_valign(if opts.top { gtk::Align::Start } else { gtk::Align::Center });
        scaler.set_margin_top(if opts.top { 16 } else { 20 });
        scaler.set_margin_bottom(24);
        scaler.set_margin_start(12);
        scaler.set_margin_end(12);
        let clamp = adw::Clamp::new();
        clamp.set_maximum_size(opts.width + 24);
        clamp.set_child(Some(&scaler));
        clamp.set_valign(if opts.top { gtk::Align::Start } else { gtk::Align::Center });
        clamp.set_vexpand(true);
        let sc = gtk::ScrolledWindow::new();
        sc.set_hscrollbar_policy(gtk::PolicyType::Never);
        sc.set_propagate_natural_height(false);
        sc.set_vexpand(true);
        sc.set_hexpand(true);
        sc.set_child(Some(&clamp));
        let wrap = gtk::Box::new(gtk::Orientation::Vertical, 0);
        wrap.add_css_class("scrim");
        wrap.set_hexpand(true);
        wrap.set_vexpand(true);
        wrap.append(&sc);
        wrap.set_opacity(0.0);
        // a click on the dimmed backdrop (outside the panel) closes the popup
        let click = gtk::GestureClick::new();
        let me = self.clone();
        let (w2, p2) = (wrap.clone(), panel.clone());
        click.connect_released(move |g, _, x, y| {
            if me.ignoring() {
                return;
            }
            if let Some(b) = p2.compute_bounds(&w2) {
                let inside = x >= b.x() as f64 && x <= (b.x() + b.width()) as f64 && y >= b.y() as f64 && y <= (b.y() + b.height()) as f64;
                if !inside {
                    g.set_state(gtk::EventSequenceState::Claimed);
                    me.close_wrap(&w2);
                }
            }
        });
        wrap.add_controller(click);
        self.root.add_overlay(&wrap);
        self.open.borrow_mut().push(Entry { wrap: wrap.clone(), on_close, name: opts.name.to_string() });
        // fade in, and the panel pops
        fade(&wrap, 0.0, 1.0, 250);
        if self.glass.get() {
            let dy = if opts.top { 90.0 } else { 18.0 };
            scaler.animate((if opts.top { 0.96 } else { 0.84 }, dy), (1.0, 0.0), 600, ease_spring, None);
        } else {
            scaler.animate((0.9, 0.0), (1.0, 0.0), 220, ease_soft, None);
        }
        self.ignore_for(350);
        panel
    }

    fn close_wrap(&self, wrap: &gtk::Box) {
        let e = {
            let mut v = self.open.borrow_mut();
            match v.iter().position(|e| &e.wrap == wrap) {
                Some(i) => v.remove(i),
                None => return,
            }
        };
        if let Some(f) = &e.on_close {
            f();
        }
        let root = self.root.clone();
        let w = e.wrap.clone();
        w.set_can_target(false);
        fade(&w, w.opacity(), 0.0, 200);
        glib::timeout_add_local_once(Duration::from_millis(220), move || {
            if w.parent().is_some() {
                root.remove_overlay(&w);
            }
        });
    }

    /// Closes the popup that holds this widget.
    pub fn close_of(&self, w: &impl IsA<gtk::Widget>) {
        let found = self.open.borrow().iter().find(|e| w.is_ancestor(&e.wrap)).map(|e| e.wrap.clone());
        if let Some(wr) = found {
            self.close_wrap(&wr);
        }
    }

    /// Closes a named popup.
    pub fn close(&self, name: &str) {
        let found = self.open.borrow().iter().find(|e| e.name == name).map(|e| e.wrap.clone());
        if let Some(w) = found {
            self.close_wrap(&w);
        }
    }

    /// Closes the top popup (Escape). False when none is open.
    pub fn close_top(&self) -> bool {
        let top = self.open.borrow().last().map(|e| e.wrap.clone());
        match top {
            Some(w) => {
                self.close_wrap(&w);
                true
            }
            None => false,
        }
    }

    /// Scrolls the top popup (used by the screen checks).
    pub fn scroll_top(&self, v: f64) {
        if let Some(e) = self.open.borrow().last() {
            if let Some(sc) = e.wrap.first_child().and_then(|c| c.downcast::<gtk::ScrolledWindow>().ok()) {
                sc.vadjustment().set_value(v);
            }
        }
    }

    pub fn close_all(&self) {
        while self.close_top() {}
    }
}

/// Fade a widget's opacity.
pub fn fade(w: &impl IsA<gtk::Widget>, from: f64, to: f64, ms: u64) {
    let t0 = Instant::now();
    w.set_opacity(from);
    w.add_tick_callback(move |w, _| {
        let k = (t0.elapsed().as_secs_f64() * 1000.0 / ms as f64).min(1.0);
        w.set_opacity(from + (to - from) * ease_soft(k));
        if k >= 1.0 {
            glib::ControlFlow::Break
        } else {
            glib::ControlFlow::Continue
        }
    });
}

/// The message at the bottom of the window.
#[derive(Clone)]
pub struct Toast {
    rev: gtk::Revealer,
    label: gtk::Label,
    timer: Rc<RefCell<Option<glib::SourceId>>>,
}

impl Toast {
    pub fn new(root: &gtk::Overlay) -> Toast {
        let label = gtk::Label::new(None);
        label.add_css_class("toast");
        label.set_wrap(true);
        label.set_justify(gtk::Justification::Center);
        label.set_max_width_chars(46);
        let rev = gtk::Revealer::new();
        rev.set_transition_type(gtk::RevealerTransitionType::SlideUp);
        rev.set_transition_duration(250);
        rev.set_child(Some(&label));
        rev.set_halign(gtk::Align::Center);
        rev.set_valign(gtk::Align::End);
        rev.set_margin_bottom(24);
        rev.set_margin_start(20);
        rev.set_margin_end(20);
        rev.set_can_target(false);
        root.add_overlay(&rev);
        Toast { rev, label, timer: Rc::default() }
    }

    /// Show a message for ms milliseconds ("" hides it).
    pub fn show(&self, msg: &str, ms: u64) {
        if let Some(t) = self.timer.borrow_mut().take() {
            t.remove();
        }
        if msg.is_empty() {
            self.rev.set_reveal_child(false);
            return;
        }
        self.label.set_text(msg);
        self.rev.set_reveal_child(true);
        let rev = self.rev.clone();
        let timer = self.timer.clone();
        let id = glib::timeout_add_local_once(Duration::from_millis(ms.max(1)), move || {
            timer.borrow_mut().take();
            rev.set_reveal_child(false);
        });
        *self.timer.borrow_mut() = Some(id);
    }
}

// ---------- building blocks ----------

pub fn label(text: &str, class: &str) -> gtk::Label {
    let l = gtk::Label::new(Some(text));
    if !class.is_empty() {
        for c in class.split(' ') {
            l.add_css_class(c);
        }
    }
    l.set_xalign(0.0);
    l
}

/// A hint under a field (wraps).
pub fn hint(text: &str, class: &str) -> gtk::Label {
    let l = label(text, class);
    l.set_wrap(true);
    l.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    l.set_max_width_chars(20);
    l.set_hexpand(true);
    l
}

pub fn heading(text: &str) -> gtk::Label {
    let l = gtk::Label::new(Some(text));
    l.add_css_class("h2");
    l.set_wrap(true);
    l.set_justify(gtk::Justification::Center);
    l
}

/// "LABEL" above a field.
pub fn field(parent: &gtk::Box, text: &str) -> gtk::Box {
    let f = gtk::Box::new(gtk::Orientation::Vertical, 5);
    f.append(&label(text, "flabel"));
    parent.append(&f);
    f
}

/// A text input in the rounded input style. Returns (frame, entry).
pub fn input(value: &str, placeholder: &str, password: bool) -> (gtk::Box, gtk::Entry) {
    let w = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    w.add_css_class("in");
    let e = gtk::Entry::new();
    e.set_text(value);
    e.set_placeholder_text(Some(placeholder));
    e.set_hexpand(true);
    e.set_has_frame(false);
    if password {
        e.set_visibility(false);
        e.set_input_purpose(gtk::InputPurpose::Password);
    }
    w.append(&e);
    (w, e)
}

/// A drop-down list in the input style. options: (value, label). Returns (frame, dropdown).
pub fn select(options: &[(String, String)], value: &str) -> (gtk::Box, gtk::DropDown) {
    let w = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    w.add_css_class("in");
    let labels: Vec<&str> = options.iter().map(|o| o.1.as_str()).collect();
    let dd = gtk::DropDown::from_strings(&labels);
    dd.set_hexpand(true);
    if let Some(i) = options.iter().position(|o| o.0 == value) {
        dd.set_selected(i as u32);
    }
    w.append(&dd);
    (w, dd)
}

/// The http:// / https:// list in front of an address.
pub fn scheme_select(value: &str) -> gtk::DropDown {
    let dd = gtk::DropDown::from_strings(&["http://", "https://"]);
    dd.set_selected(if value == "https" { 1 } else { 0 });
    dd.set_hexpand(false);
    dd
}

pub fn scheme_of(dd: &gtk::DropDown) -> &'static str {
    if dd.selected() == 1 {
        "https"
    } else {
        "http"
    }
}

/// A label with a switch on the right.
pub fn toggle_row(text: &str, on: bool) -> (gtk::Box, gtk::Switch) {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    row.add_css_class("tg");
    row.set_margin_start(4);
    row.set_margin_end(4);
    let l = label(text, "flabel");
    l.set_hexpand(true);
    l.set_wrap(true);
    row.append(&l);
    let sw = gtk::Switch::new();
    sw.set_active(on);
    sw.set_valign(gtk::Align::Center);
    row.append(&sw);
    (row, sw)
}

pub fn button(text: &str, class: &str) -> gtk::Button {
    let b = gtk::Button::with_label(text);
    for c in class.split(' ').filter(|c| !c.is_empty()) {
        b.add_css_class(c);
    }
    b
}

/// A row of buttons (side by side, or stacked with col = true).
pub fn btns(col: bool) -> gtk::Box {
    let b = gtk::Box::new(if col { gtk::Orientation::Vertical } else { gtk::Orientation::Horizontal }, 10);
    b.add_css_class("btns");
    b.set_homogeneous(!col);
    b
}

pub fn chip(text: &str) -> gtk::Button {
    let b = button(text, "chip");
    b.set_halign(gtk::Align::Start);
    b
}

/// A segmented row of radio buttons. on_pick(value) runs when the choice changes.
pub fn radio_row(opts: &[(String, String)], value: &str, wrap: bool, on_pick: Rc<dyn Fn(String)>) -> gtk::Widget {
    let make = |o: &(String, String), cut: bool| {
        let b = gtk::Button::new();
        let inner = gtk::Box::new(gtk::Orientation::Horizontal, 7);
        inner.set_halign(gtk::Align::Center);
        let dot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        dot.add_css_class("rd");
        dot.set_valign(gtk::Align::Center);
        inner.append(&dot);
        let l = gtk::Label::new(Some(&o.1));
        if cut {
            l.set_ellipsize(gtk::pango::EllipsizeMode::End);
        }
        inner.append(&l);
        b.set_child(Some(&inner));
        b.set_hexpand(true);
        if o.0 == value {
            b.add_css_class("on");
        }
        b.update_property(&[gtk::accessible::Property::Label(&o.1)]);
        let v = o.0.clone();
        let cur = value.to_string();
        let f = on_pick.clone();
        b.connect_clicked(move |_| {
            if v != cur {
                f(v.clone());
            }
        });
        b
    };
    if wrap {
        let fb = gtk::FlowBox::new();
        fb.add_css_class("seg");
        fb.set_selection_mode(gtk::SelectionMode::None);
        fb.set_homogeneous(true);
        fb.set_min_children_per_line(2);
        fb.set_max_children_per_line(4);
        fb.set_column_spacing(3);
        fb.set_row_spacing(3);
        for o in opts {
            let b = make(o, false);
            fb.append(&b);
            if let Some(c) = b.parent() {
                c.set_focusable(false);
            }
        }
        fb.upcast()
    } else {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 3);
        row.add_css_class("seg");
        row.set_homogeneous(true);
        for o in opts {
            row.append(&make(o, true));
        }
        row.upcast()
    }
}

/// A button that needs a second click (the label changes to armed in between).
pub fn armed(btn: &gtk::Button, text: &str, armed_text: &str, f: Rc<dyn Fn()>) {
    let state = Rc::new(Cell::new(false));
    btn.set_label(text);
    let (t, at) = (text.to_string(), armed_text.to_string());
    btn.connect_clicked(move |b| {
        if !state.get() {
            state.set(true);
            b.set_label(&at);
            return;
        }
        state.set(false);
        b.set_label(&t);
        f();
    });
}

/// Pressing Enter in an entry runs f.
pub fn on_enter(e: &gtk::Entry, f: impl Fn() + 'static) {
    e.connect_activate(move |_| f());
}
