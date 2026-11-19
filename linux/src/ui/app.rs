//! The live dashboard window: header (server name, LIVE / OFFLINE, Settings), server dots,
//! the fold-away Monitoring panel, the service buttons and stacks, and the live updates.

use super::draw::{self, press_squish, Background, BgLook, Ring, Scaler};
use super::modal::{self, Modals, Toast};
use super::{bg, dialogs, settings_ui, splash, Ctx};
use crate::config::{self, format_bytes, hex_rgb};
use crate::glances::{self, Detail, FsCache, Stats};
use crate::layout::{self, Item, Layout, Service, Stack};
use crate::net;
use crate::settings::{look_vars, Look, Settings};
use crate::zima::{self, ZApp, ZDrive, ZSys};
use adw::prelude::*;
use gtk::{gdk, gio, glib};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet, VecDeque};
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

const HMAX: usize = 100;

pub struct Card {
    root: gtk::Box,
    scaler: Scaler,
    ring: Ring,
    lbl: gtk::Label,
    det: gtk::Box,
}

/// The widgets the dashboard updates.
pub struct W {
    pub root: gtk::Overlay,
    pub bg: Background,
    pub scroller: gtk::ScrolledWindow,
    pub main: gtk::Box,
    pub title: gtk::Label,
    pub title_scaler: Scaler,
    pub tchev: gtk::Image,
    pub tw: gtk::Box,
    pub pmenu: gtk::Popover,
    pub dot: gtk::Box,
    pub st: gtk::Label,
    pub sdots: gtk::Box,
    pub monwrap: gtk::Box,
    pub monb: gtk::Button,
    pub monst: gtk::Label,
    pub mon: gtk::Revealer,
    pub grid: gtk::Grid,
    pub down: gtk::Label,
    pub up: gtk::Label,
    pub sect: gtk::Label,
    /// ZimaOS: "↑ 2 updates"
    pub updb: gtk::Button,
    pub layb: gtk::Button,
    pub svc: gtk::Box,
    pub wextra: gtk::Box,
    pub wnote: gtk::Label,
    pub hid: gtk::Box,
    pub addwrap: gtk::Box,
}

/// What the dashboard shows and remembers between updates.
pub struct State {
    pub set: Settings,
    pub cur: String,
    pub layout: Layout,
    pub servers: Vec<(String, String)>,
    pub title: String,
    pub has_login: bool,
    pub widget_mode: bool,
    pub open_groups: HashSet<String>,
    pub svc_up: HashMap<String, bool>,
    pub mon_open: bool,
    pub expanded: Option<String>,
    pub hist_t: VecDeque<Instant>,
    pub hist: HashMap<&'static str, VecDeque<f64>>,
    pub last: Option<Stats>,
    pub dets: HashMap<String, Detail>,
    /// bumped when the server on screen changes: late answers for the last one are dropped
    pub gen: u64,
    pub stats_busy: bool,
    pub next_stats: Instant,
    pub reach_busy: bool,
    pub last_reach: Option<Instant>,
    pub reach_for: String,
    pub reach_known: bool,
    pub status_busy: bool,
    pub status_dirty: bool,
    pub last_status: Option<Instant>,
    pub logo_busy: bool,
    pub look: Look,
    // ZimaOS (only for servers of type ZimaOS with a saved login)
    pub z_apps: Option<Vec<ZApp>>,
    /// store ids the App Store has a newer version of
    pub z_upg: HashSet<String>,
    pub z_sys: Option<ZSys>,
    pub z_drives: Option<Vec<ZDrive>>,
    pub z_drives_at: Option<Instant>,
    pub z_busy: bool,
    pub last_z_apps: Option<Instant>,
}

pub struct App {
    pub ctx: Rc<Ctx>,
    pub win: adw::ApplicationWindow,
    pub w: W,
    pub st: RefCell<State>,
    pub cards: RefCell<HashMap<String, Card>>,
    pub modals: Modals,
    pub toast: Toast,
    pub glass: Rc<Cell<bool>>,
    pub fs_cache: Arc<FsCache>,
    /// item id being dragged
    pub dragging: RefCell<Option<String>>,
    /// the status dots on screen, by button / stack id
    pub dots: RefCell<Vec<(String, gtk::Box)>>,
}

fn bx(o: gtk::Orientation, spacing: i32, class: &str) -> gtk::Box {
    let b = gtk::Box::new(o, spacing);
    for c in class.split(' ').filter(|c| !c.is_empty()) {
        b.add_css_class(c);
    }
    b
}

fn chevron(class: &str) -> gtk::Image {
    let i = gtk::Image::from_icon_name("pan-down-symbolic");
    i.add_css_class(class);
    i
}

impl App {
    pub fn new(application: &adw::Application, ctx: Rc<Ctx>, server: Option<String>) -> Rc<App> {
        let cur = {
            let mut store = ctx.store.borrow_mut();
            let id = store.initial_server(server.as_deref(), false);
            store.cur = id.clone();
            id
        };
        let set = ctx.store.borrow_mut().settings();
        let win = adw::ApplicationWindow::new(application);
        win.set_title(Some("-board"));
        let (dw, dh) = std::env::var("BOARD_WINDOW_SIZE").ok().and_then(|v| {
            let (a, b) = v.split_once('x')?;
            Some((a.parse().ok()?, b.parse().ok()?))
        }).unwrap_or((470, 900));
        win.set_default_size(dw, dh);
        win.set_size_request(340, 480);
        win.add_css_class("board");
        win.set_icon_name(Some(super::APP_ID));

        let root = gtk::Overlay::new();
        let bg = Background::new();
        root.set_child(Some(&bg.area));
        let column = bx(gtk::Orientation::Vertical, 0, "");
        let hb = adw::HeaderBar::new();
        hb.set_show_title(false);
        column.append(&hb);
        let scroller = gtk::ScrolledWindow::new();
        scroller.set_hscrollbar_policy(gtk::PolicyType::Never);
        scroller.set_vexpand(true);
        column.append(&scroller);
        root.add_overlay(&column);
        root.set_measure_overlay(&column, true);
        let clamp = adw::Clamp::new();
        clamp.set_maximum_size(640);
        clamp.set_tightening_threshold(480);
        let main = bx(gtk::Orientation::Vertical, 16, "");
        main.set_margin_top(6);
        main.set_margin_start(18);
        main.set_margin_end(18);
        main.set_margin_bottom(40);
        clamp.set_child(Some(&main));
        scroller.set_child(Some(&clamp));

        // header: name (tap = Reboot / Shut down, hold = rename, swipe = other server), LIVE, Settings
        let header = bx(gtk::Orientation::Horizontal, 8, "");
        let tw = bx(gtk::Orientation::Horizontal, 8, "tw");
        tw.set_hexpand(true);
        let title = gtk::Label::new(Some(""));
        title.add_css_class("h1");
        title.set_xalign(0.0);
        let title_scaler = Scaler::new(&title);
        tw.append(&title_scaler);
        let tchev = chevron("tchev");
        tw.append(&tchev);
        let tw_space = bx(gtk::Orientation::Horizontal, 0, "");
        tw_space.set_hexpand(true);
        let twrap = bx(gtk::Orientation::Horizontal, 0, "");
        twrap.set_hexpand(true);
        tw.set_hexpand(false);
        twrap.append(&tw);
        twrap.append(&tw_space);
        header.append(&twrap);
        let hr = bx(gtk::Orientation::Horizontal, 8, "");
        let live = bx(gtk::Orientation::Horizontal, 8, "live");
        live.set_valign(gtk::Align::Center);
        let dot = bx(gtk::Orientation::Horizontal, 0, "dot wait");
        dot.set_valign(gtk::Align::Center);
        let stl = gtk::Label::new(Some("..."));
        live.append(&dot);
        live.append(&stl);
        hr.append(&live);
        let gear = gtk::Button::from_icon_name("emblem-system-symbolic");
        gear.add_css_class("gear");
        gear.set_valign(gtk::Align::Center);
        gear.set_tooltip_text(Some("Settings (Ctrl+,)"));
        let gear_sc = Scaler::new(&gear);
        gear_sc.set_valign(gtk::Align::Center);
        hr.append(&gear_sc);
        header.append(&hr);
        main.append(&header);

        // Reboot / Shut down menu
        let pmenu = gtk::Popover::new();
        pmenu.add_css_class("pmenu");
        pmenu.set_has_arrow(false);
        pmenu.set_position(gtk::PositionType::Bottom);
        pmenu.set_parent(&tw);
        let pbox = bx(gtk::Orientation::Vertical, 2, "");
        let mk = |icon: &str, text: &str, red: bool| {
            let b = gtk::Button::new();
            let r = bx(gtk::Orientation::Horizontal, 10, "");
            r.append(&gtk::Image::from_icon_name(icon));
            let l = gtk::Label::new(Some(text));
            l.set_xalign(0.0);
            r.append(&l);
            b.set_child(Some(&r));
            if red {
                b.add_css_class("red");
            }
            b
        };
        let reboot = mk("view-refresh-symbolic", "Reboot", false);
        let shut = mk("system-shutdown-symbolic", "Shut down", true);
        pbox.append(&reboot);
        pbox.append(&shut);
        pmenu.set_child(Some(&pbox));
        let t2 = tw.clone();
        pmenu.connect_show(move |_| t2.add_css_class("open"));
        let t2 = tw.clone();
        pmenu.connect_closed(move |_| t2.remove_css_class("open"));

        // server dots
        let sdots = bx(gtk::Orientation::Horizontal, 7, "sdots");
        sdots.set_margin_top(-8);
        sdots.set_margin_start(2);
        sdots.set_size_request(-1, 16);
        main.append(&sdots);

        // Monitoring
        let monwrap = bx(gtk::Orientation::Vertical, 0, "monwrap");
        let monb = gtk::Button::new();
        monb.add_css_class("monbtn");
        let mrow = bx(gtk::Orientation::Horizontal, 0, "");
        let ml = gtk::Label::new(Some("MONITORING"));
        mrow.append(&ml);
        let monst = gtk::Label::new(None);
        monst.add_css_class("monst");
        monst.set_hexpand(true);
        monst.set_xalign(1.0);
        monst.set_margin_end(10);
        mrow.append(&monst);
        mrow.append(&chevron("chev"));
        monb.set_child(Some(&mrow));
        let monb_sc = Scaler::new(&monb);
        monwrap.append(&monb_sc);
        let mon = gtk::Revealer::new();
        mon.set_transition_type(gtk::RevealerTransitionType::SlideDown);
        mon.set_transition_duration(400);
        let monpad = bx(gtk::Orientation::Vertical, 16, "");
        monpad.set_margin_top(16);
        let grid = gtk::Grid::new();
        grid.set_column_spacing(14);
        grid.set_row_spacing(14);
        grid.set_column_homogeneous(true);
        monpad.append(&grid);
        let netcard = bx(gtk::Orientation::Horizontal, 0, "netcard");
        netcard.set_homogeneous(true);
        let netcol = |arrow: &str, cls: &str, cap: &str| {
            let c = bx(gtk::Orientation::Vertical, 6, "");
            let a = gtk::Label::new(Some(arrow));
            a.add_css_class("arrow");
            a.add_css_class(cls);
            c.append(&a);
            let v = gtk::Label::new(Some("0 B/s"));
            v.add_css_class("netv");
            c.append(&v);
            let l = gtk::Label::new(Some(cap));
            l.add_css_class("netc");
            c.append(&l);
            (c, v)
        };
        let (dn, down) = netcol("↓", "dn", "DOWNLOAD");
        let (upc, up) = netcol("↑", "up", "UPLOAD");
        netcard.append(&dn);
        netcard.append(&upc);
        monpad.append(&netcard);
        mon.set_child(Some(&monpad));
        monwrap.append(&mon);
        main.append(&monwrap);

        // services
        let sechead = bx(gtk::Orientation::Horizontal, 0, "");
        sechead.set_margin_top(6);
        sechead.set_margin_start(4);
        sechead.set_margin_end(4);
        sechead.set_margin_bottom(-4);
        let sect = gtk::Label::new(Some("SERVICES"));
        sect.add_css_class("sec");
        sect.set_hexpand(true);
        sect.set_xalign(0.0);
        sechead.append(&sect);
        let updb = gtk::Button::with_label("");
        updb.add_css_class("lay");
        updb.set_valign(gtk::Align::Center);
        updb.set_margin_end(8);
        updb.set_visible(false);
        sechead.append(&updb);
        let layb = gtk::Button::with_label("Widget layout");
        layb.add_css_class("lay");
        layb.set_valign(gtk::Align::Center);
        sechead.append(&layb);
        main.append(&sechead);
        let svc = bx(gtk::Orientation::Vertical, 0, "dash");
        main.append(&svc);
        let wextra = bx(gtk::Orientation::Vertical, 14, "");
        wextra.set_visible(false);
        let wnote = modal::hint("", "wnote");
        wnote.set_margin_start(4);
        wnote.set_margin_end(4);
        wextra.append(&wnote);
        let hidwrap = bx(gtk::Orientation::Vertical, 8, "");
        let hl = gtk::Label::new(Some("NOT ON WIDGET"));
        hl.add_css_class("sec");
        hl.set_xalign(0.0);
        hl.set_margin_start(4);
        hidwrap.append(&hl);
        let hid = bx(gtk::Orientation::Vertical, 0, "hidbox wide");
        hidwrap.append(&hid);
        wextra.append(&hidwrap);
        main.append(&wextra);
        let addwrap = bx(gtk::Orientation::Horizontal, 0, "");
        addwrap.set_halign(gtk::Align::Center);
        addwrap.set_margin_top(2);
        let addbtn = gtk::Button::with_label("+");
        addbtn.add_css_class("addbtn");
        addbtn.set_tooltip_text(Some("Add a service (Ctrl+N)"));
        let add_sc = Scaler::new(&addbtn);
        addwrap.append(&add_sc);
        main.append(&addwrap);

        win.set_content(Some(&root));
        let modals = Modals::new(&root);
        let toast = Toast::new(&root);
        let glass = modals.glass.clone();
        for s in [&monb_sc, &add_sc, &gear_sc] {
            press_squish(s, 0.94, glass.clone());
        }

        let look = set.look();
        let app = Rc::new(App {
            ctx,
            win,
            w: W { root, bg, scroller, main, title, title_scaler, tchev, tw, pmenu, dot, st: stl, sdots, monwrap, monb: monb.clone(), monst, mon, grid, down, up, sect, updb: updb.clone(), layb: layb.clone(), svc, wextra, wnote, hid, addwrap },
            st: RefCell::new(State {
                set,
                cur,
                layout: Layout::default(),
                servers: vec![],
                title: String::new(),
                has_login: false,
                widget_mode: false,
                open_groups: HashSet::new(),
                svc_up: HashMap::new(),
                mon_open: false,
                expanded: None,
                hist_t: VecDeque::new(),
                hist: HashMap::new(),
                last: None,
                dets: HashMap::new(),
                gen: 0,
                stats_busy: false,
                next_stats: Instant::now(),
                reach_busy: false,
                last_reach: None,
                reach_for: String::new(),
                reach_known: false,
                status_busy: false,
                status_dirty: true,
                last_status: None,
                logo_busy: false,
                look,
                z_apps: None,
                z_upg: HashSet::new(),
                z_sys: None,
                z_drives: None,
                z_drives_at: None,
                z_busy: false,
                last_z_apps: None,
            }),
            cards: RefCell::default(),
            modals,
            toast,
            glass,
            fs_cache: Arc::new(FsCache::default()),
            dragging: RefCell::new(None),
            dots: RefCell::default(),
        });

        // ---- signals ----
        let a = app.clone();
        gear.connect_clicked(move |_| {
            if !a.modals.ignoring() {
                a.w.pmenu.popdown();
                settings_ui::open(&a);
            }
        });
        let a = app.clone();
        monb.connect_clicked(move |_| a.toggle_mon());
        let a = app.clone();
        layb.connect_clicked(move |_| {
            let wm = !a.st.borrow().widget_mode;
            a.st.borrow_mut().widget_mode = wm;
            a.draw(true);
        });
        let a = app.clone();
        addbtn.connect_clicked(move |_| dialogs::open_add(&a, None));
        let a = app.clone();
        updb.connect_clicked(move |_| dialogs::open_updates(&a));
        let a = app.clone();
        reboot.connect_clicked(move |_| {
            a.w.pmenu.popdown();
            dialogs::open_power(&a, "restart", "", false);
        });
        let a = app.clone();
        shut.connect_clicked(move |_| {
            a.w.pmenu.popdown();
            dialogs::open_power(&a, "off", "", false);
        });
        app.bind_title();
        app.bind_keys();

        app.refresh(true);
        app.apply_look(&app.st.borrow().look.clone());
        let (mon_open, monitoring) = {
            let s = app.st.borrow();
            (s.set.monitoring_open || s.set.monitoring_fixed, s.set.monitoring)
        };
        if mon_open && monitoring {
            app.toggle_mon();
        }
        // the live updates
        let a = app.clone();
        glib::timeout_add_local(Duration::from_millis(250), move || {
            // the window was closed: stop
            if a.win.application().is_none() {
                return glib::ControlFlow::Break;
            }
            a.tick();
            glib::ControlFlow::Continue
        });
        app.refresh_logos();
        app
    }

    pub fn present(self: &Rc<Self>, stack: Option<String>) {
        let splash_on = self.st.borrow().set.splash;
        self.win.present();
        if splash_on {
            splash::show(self);
        }
        if let Some(id) = stack {
            self.focus_stack(&id);
        }
        // first run: no server yet, so straight to Settings (after the startup screen)
        if self.st.borrow().set.server_host().is_empty() {
            let a = self.clone();
            glib::timeout_add_local_once(Duration::from_millis(if splash_on { 1700 } else { 400 }), move || settings_ui::open(&a));
        }
    }

    // ---------- data ----------

    /// Reads the server on screen again and redraws (after any change).
    pub fn refresh(self: &Rc<Self>, anim: bool) {
        let (set, layout, servers, title, has_login, cur) = {
            let mut store = self.ctx.store.borrow_mut();
            let set = store.settings();
            let d = store.load_data();
            let layout = layout::get_layout(&d, &set.server_host_or_localhost());
            let cur = store.cur.clone();
            let has = self.ctx.secrets.has_login(&set.platform, &cur);
            (set, layout, store.list_servers(), store.title(), has, cur)
        };
        {
            let mut s = self.st.borrow_mut();
            s.set = set;
            s.layout = layout;
            s.servers = servers;
            s.title = title;
            s.has_login = has_login;
            s.cur = cur;
        }
        self.apply_ui();
        self.render_dots();
        // the desktop widgets show the change too
        super::widget::refresh_soon();
        if self.dragging.borrow().is_none() {
            self.draw(anim);
        }
    }

    /// Settings that apply right away.
    fn apply_ui(self: &Rc<Self>) {
        let s = self.st.borrow();
        let set = &s.set;
        self.w.title.set_text(&s.title);
        self.win.set_title(Some(&format!("{} · -board", s.title)));
        if set.columns == 3 {
            self.win.add_css_class("c3");
        } else {
            self.win.remove_css_class("c3");
        }
        self.w.monwrap.set_visible(set.monitoring);
        let power = set.show_power && config::platform(&set.platform).is_some();
        self.w.tchev.set_visible(power);
        if !power {
            self.w.pmenu.popdown();
        }
        self.w.monb.parent().unwrap().set_visible(!set.monitoring_fixed);
        let max = set.widget_rows * 4;
        self.w.wnote.set_text(&format!(
            "The large widget fits {max} buttons. Dimmed ones will not show. Drag to reorder, or drag a button between the two panels to show or hide it."
        ));
        let fixed = set.monitoring_fixed && set.monitoring && !s.mon_open;
        drop(s);
        if fixed {
            self.toggle_mon();
        }
    }

    /// Theme & look (also used for the live preview behind Settings).
    pub fn apply_look(self: &Rc<Self>, look: &Look) {
        super::load_css(look);
        for (c, on) in super::style::look_classes(look) {
            if on {
                self.win.add_css_class(c);
            } else {
                self.win.remove_css_class(c);
            }
        }
        let v = look_vars(look);
        let rgb = |c: (u8, u8, u8)| (c.0 as f64 / 255.0, c.1 as f64 / 255.0, c.2 as f64 / 255.0);
        self.w.bg.set_look(BgLook { bg: hex_rgb(&v.bg), g1: rgb(v.g1), g2: rgb(v.g2), acc: hex_rgb(&v.acc), glowop: v.glowop, moving: look.moving_bg, glass: look.glass() });
        self.glass.set(look.glass());
        for c in self.cards.borrow().values() {
            c.ring.set_glass(look.glass());
        }
    }

    pub fn set_status(&self, ok: bool) {
        self.st.borrow_mut().reach_known = true;
        self.w.dot.set_css_classes(&["dot", if ok { "" } else { "off" }].into_iter().filter(|c| !c.is_empty()).collect::<Vec<_>>());
        self.w.st.set_text(if ok { "LIVE" } else { "OFFLINE" });
    }

    fn glances_state(&self, ok: bool) {
        self.w.monst.set_text(if ok { "" } else { "NO DATA" });
        if ok && !self.st.borrow().reach_known {
            self.set_status(true);
        }
    }

    fn offline_setup(&self) {
        self.w.dot.set_css_classes(&["dot", "wait"]);
        self.w.st.set_text("SETUP");
    }

    // ---------- server dots and switching ----------

    fn render_dots(self: &Rc<Self>) {
        while let Some(c) = self.w.sdots.first_child() {
            self.w.sdots.remove(&c);
        }
        let s = self.st.borrow();
        for (i, (id, title)) in s.servers.iter().enumerate() {
            let b = gtk::Button::new();
            b.add_css_class("sdotbtn");
            let d = bx(gtk::Orientation::Horizontal, 0, if *id == s.cur { "sdot on" } else { "sdot" });
            d.set_valign(gtk::Align::Center);
            b.set_child(Some(&d));
            b.set_tooltip_text(Some(title));
            b.update_property(&[gtk::accessible::Property::Label(title)]);
            let a = self.clone();
            let id2 = id.clone();
            let mine = s.servers.iter().position(|x| x.0 == s.cur).unwrap_or(0);
            b.connect_clicked(move |_| {
                if id2 != a.st.borrow().cur {
                    a.go_server(&id2, if i > mine { 1 } else { -1 });
                }
            });
            self.w.sdots.append(&b);
        }
        let plus = gtk::Button::with_label("+");
        plus.add_css_class("splus");
        plus.set_tooltip_text(Some("Add server"));
        let a = self.clone();
        plus.connect_clicked(move |_| dialogs::open_add_server(&a));
        self.w.sdots.append(&plus);
    }

    pub fn go_server(self: &Rc<Self>, id: &str, dir: i32) {
        self.w.pmenu.popdown();
        if self.ctx.store.borrow_mut().set_current(id) {
            self.show_server(dir);
        }
    }

    /// Next (+1) or previous (-1) server; past the last one: add a server.
    pub fn step_server(self: &Rc<Self>, dir: i32) {
        let (list, cur) = {
            let s = self.st.borrow();
            (s.servers.clone(), s.cur.clone())
        };
        let i = list.iter().position(|x| x.0 == cur).unwrap_or(0) as i32;
        let j = i + dir;
        if j >= 0 && (j as usize) < list.len() {
            self.go_server(&list[j as usize].0.clone(), dir);
        } else if dir > 0 {
            dialogs::open_add_server(self);
        }
    }

    /// Another server is on screen: drop what was shown for the last one and draw this one.
    pub fn show_server(self: &Rc<Self>, dir: i32) {
        self.fs_cache.clear();
        {
            let mut s = self.st.borrow_mut();
            s.gen += 1;
            s.hist.clear();
            s.hist_t.clear();
            s.dets.clear();
            s.last = None;
            s.expanded = None;
            s.svc_up.clear();
            s.open_groups.clear();
            s.widget_mode = false;
            s.reach_known = false;
            s.stats_busy = false;
            s.reach_busy = false;
            s.status_busy = false;
            s.status_dirty = true;
            s.last_reach = None;
            s.reach_for.clear();
            s.next_stats = Instant::now();
            s.z_apps = None;
            s.z_upg.clear();
            s.z_sys = None;
            s.z_drives = None;
            s.z_drives_at = None;
            s.last_z_apps = None;
        }
        zima::forget_sys(&self.ctx.store.borrow().cur);
        while let Some(c) = self.w.grid.first_child() {
            self.w.grid.remove(&c);
        }
        self.cards.borrow_mut().clear();
        self.w.dot.set_css_classes(&["dot", "wait"]);
        self.w.st.set_text("...");
        self.w.monst.set_text("");
        self.w.down.set_text(&fmt_rate(0.0));
        self.w.up.set_text(&fmt_rate(0.0));
        self.w.pmenu.popdown();
        self.refresh(true);
        self.w.title_scaler.slide_in(if dir > 0 { 44.0 } else if dir < 0 { -44.0 } else { 0.0 });
        // the Monitoring panel follows this server's settings
        let (open, set) = {
            let s = self.st.borrow();
            (s.mon_open, s.set.clone())
        };
        if open && (!set.monitoring || (!set.monitoring_open && !set.monitoring_fixed)) {
            self.st.borrow_mut().mon_open = false;
            self.w.mon.set_reveal_child(false);
            self.w.monb.remove_css_class("open");
        } else if !open && set.monitoring && (set.monitoring_open || set.monitoring_fixed) {
            self.toggle_mon();
        }
        self.refresh_logos();
    }

    // ---------- header gestures ----------

    fn bind_title(self: &Rc<Self>) {
        let tw = &self.w.tw;
        // click: Reboot / Shut down
        let click = gtk::GestureClick::new();
        click.set_button(gdk::BUTTON_PRIMARY);
        let a = self.clone();
        click.connect_released(move |_, _, _, _| {
            if a.modals.ignoring() {
                return;
            }
            let s = a.st.borrow();
            if !(s.set.show_power && config::platform(&s.set.platform).is_some()) {
                return;
            }
            drop(s);
            if a.w.pmenu.is_visible() {
                a.w.pmenu.popdown();
            } else {
                a.w.pmenu.popup();
            }
        });
        tw.add_controller(click);
        // hold or right-click: rename
        let hold = gtk::GestureLongPress::new();
        let a = self.clone();
        hold.connect_pressed(move |_, _, _| {
            a.modals.ignore_for(450);
            dialogs::open_rename(&a);
        });
        tw.add_controller(hold);
        let rc = gtk::GestureClick::new();
        rc.set_button(gdk::BUTTON_SECONDARY);
        let a = self.clone();
        rc.connect_pressed(move |_, _, _, _| dialogs::open_rename(&a));
        tw.add_controller(rc);
        // swipe the name: left = next server (past the last one: add a server), right = previous
        let drag = gtk::GestureDrag::new();
        let t0 = Rc::new(Cell::new(Instant::now()));
        let t1 = t0.clone();
        drag.connect_drag_begin(move |_, _, _| t1.set(Instant::now()));
        let a = self.clone();
        drag.connect_drag_end(move |_, dx, dy| {
            if dx.abs() < 45.0 || dx.abs() < dy.abs() * 1.5 || t0.get().elapsed() > Duration::from_millis(800) {
                return;
            }
            a.modals.ignore_for(450);
            a.step_server(if dx < 0.0 { 1 } else { -1 });
        });
        tw.add_controller(drag);
        // two-finger swipe on a touchpad over the name
        let scroll = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::HORIZONTAL | gtk::EventControllerScrollFlags::KINETIC);
        let acc = Rc::new(Cell::new(0.0));
        let a = self.clone();
        let acc2 = acc.clone();
        scroll.connect_scroll(move |_, dx, _| {
            acc2.set(acc2.get() + dx);
            if acc2.get().abs() > 6.0 {
                let d = if acc2.get() > 0.0 { 1 } else { -1 };
                acc2.set(-1000.0 * d as f64);
                a.step_server(d);
            }
            glib::Propagation::Stop
        });
        scroll.connect_scroll_end(move |_| acc.set(0.0));
        tw.add_controller(scroll);
    }

    fn bind_keys(self: &Rc<Self>) {
        let k = gtk::EventControllerKey::new();
        let a = self.clone();
        k.connect_key_pressed(move |_, key, _, mods| {
            let ctrl = mods.contains(gdk::ModifierType::CONTROL_MASK);
            let alt = mods.contains(gdk::ModifierType::ALT_MASK);
            match key {
                gdk::Key::Escape => {
                    if a.w.pmenu.is_visible() {
                        a.w.pmenu.popdown();
                        return glib::Propagation::Stop;
                    }
                    if a.modals.close_top() {
                        return glib::Propagation::Stop;
                    }
                    if a.st.borrow().widget_mode {
                        a.st.borrow_mut().widget_mode = false;
                        a.draw(true);
                        return glib::Propagation::Stop;
                    }
                }
                gdk::Key::comma if ctrl => {
                    if !a.modals.any_open() {
                        settings_ui::open(&a);
                    }
                    return glib::Propagation::Stop;
                }
                gdk::Key::n if ctrl => {
                    if !a.modals.any_open() {
                        dialogs::open_add(&a, None);
                    }
                    return glib::Propagation::Stop;
                }
                gdk::Key::Page_Down if ctrl => {
                    if !a.modals.any_open() {
                        a.step_server(1);
                    }
                    return glib::Propagation::Stop;
                }
                gdk::Key::Page_Up if ctrl => {
                    if !a.modals.any_open() {
                        a.step_server(-1);
                    }
                    return glib::Propagation::Stop;
                }
                gdk::Key::Right if alt && !a.modals.any_open() => {
                    a.step_server(1);
                    return glib::Propagation::Stop;
                }
                gdk::Key::Left if alt && !a.modals.any_open() => {
                    a.step_server(-1);
                    return glib::Propagation::Stop;
                }
                gdk::Key::F5 | gdk::Key::r if key == gdk::Key::F5 || ctrl => {
                    a.refresh_now();
                    return glib::Propagation::Stop;
                }
                _ => {}
            }
            glib::Propagation::Proceed
        });
        self.win.add_controller(k);
    }

    /// Check everything again now.
    pub fn refresh_now(self: &Rc<Self>) {
        let mut s = self.st.borrow_mut();
        s.next_stats = Instant::now();
        s.last_reach = None;
        s.status_dirty = true;
    }

    // ---------- Monitoring ----------

    pub fn toggle_mon(self: &Rc<Self>) {
        let (open, fixed) = {
            let s = self.st.borrow();
            (s.mon_open, s.set.monitoring_fixed)
        };
        if open && fixed {
            return; // can't be folded away while "always open" is on
        }
        let open = !open;
        self.st.borrow_mut().mon_open = open;
        self.w.mon.set_reveal_child(open);
        if open {
            self.w.monb.add_css_class("open");
            for c in self.cards.borrow().values() {
                c.ring.replay();
            }
            // an enlarged card's details are fetched again while the panel is open
            self.st.borrow_mut().next_stats = Instant::now();
        } else {
            self.w.monb.remove_css_class("open");
        }
    }

    fn card(self: &Rc<Self>, id: &str, label: &str) {
        if let Some(c) = self.cards.borrow().get(id) {
            c.lbl.set_text(label);
            return;
        }
        let root = bx(gtk::Orientation::Vertical, 10, "card");
        let ring = Ring::new(140);
        ring.set_glass(self.glass.get());
        root.append(&ring.area);
        let lbl = gtk::Label::new(Some(label));
        lbl.add_css_class("lbl");
        root.append(&lbl);
        let det = bx(gtk::Orientation::Vertical, 16, "detail");
        det.set_visible(false);
        det.set_margin_top(6);
        det.set_margin_start(4);
        det.set_margin_end(4);
        root.append(&det);
        let scaler = Scaler::new(&root);
        press_squish(&scaler, 0.965, self.glass.clone());
        let click = gtk::GestureClick::new();
        let a = self.clone();
        let id2 = id.to_string();
        click.connect_released(move |g, n, _, _| {
            if n == 1 {
                g.set_state(gtk::EventSequenceState::Claimed);
                a.toggle_card(&id2);
            }
        });
        root.add_controller(click);
        root.set_cursor_from_name(Some("pointer"));
        self.cards.borrow_mut().insert(id.to_string(), Card { root, scaler, ring, lbl, det });
        self.layout_cards();
    }

    /// The cards in the grid: an enlarged card goes first, across both columns.
    fn layout_cards(&self) {
        let cards = self.cards.borrow();
        let exp = self.st.borrow().expanded.clone();
        for c in cards.values() {
            if c.scaler.parent().is_some() {
                self.w.grid.remove(&c.scaler);
            }
        }
        let mut order: Vec<&str> = ["cpu", "ram", "disk"].into_iter().filter(|k| cards.contains_key(*k)).collect();
        if let Some(e) = exp.as_deref() {
            if let Some(i) = order.iter().position(|k| *k == e) {
                let k = order.remove(i);
                order.insert(0, k);
            }
        }
        let (mut col, mut row) = (0, 0);
        for k in order {
            let c = &cards[k];
            let big = exp.as_deref() == Some(k);
            if big {
                self.w.grid.attach(&c.scaler, 0, row, 2, 1);
                row += 1;
                c.ring.area.set_content_width(160);
                c.ring.area.set_content_height(160);
                c.root.add_css_class("big");
            } else {
                self.w.grid.attach(&c.scaler, col, row, 1, 1);
                c.ring.area.set_content_width(140);
                c.ring.area.set_content_height(140);
                c.root.remove_css_class("big");
                col += 1;
                if col == 2 {
                    col = 0;
                    row += 1;
                }
            }
        }
    }

    fn put(self: &Rc<Self>, id: &str, label: &str, p: f64) {
        self.card(id, label);
        let open = self.st.borrow().mon_open;
        if let Some(c) = self.cards.borrow().get(id) {
            // panel closed: nothing to see, so jump (opening the panel replays it)
            c.ring.set(p, open);
        }
    }

    pub fn toggle_card(self: &Rc<Self>, id: &str) {
        let was = self.st.borrow().expanded.clone();
        let now = if was.as_deref() == Some(id) { None } else { Some(id.to_string()) };
        if let Some(w) = &was {
            if let Some(c) = self.cards.borrow().get(w) {
                c.det.set_visible(false);
                while let Some(ch) = c.det.first_child() {
                    c.det.remove(&ch);
                }
            }
        }
        self.st.borrow_mut().expanded = now.clone();
        self.layout_cards();
        if let Some(k) = now {
            self.render_detail(&k);
            // the details are fetched straight away
            self.st.borrow_mut().next_stats = Instant::now();
            let a = self.clone();
            glib::timeout_add_local_once(Duration::from_millis(80), move || {
                if let Some(c) = a.cards.borrow().get(&k) {
                    a.scroll_to(&c.scaler);
                }
            });
        }
    }

    fn scroll_to(&self, w: &impl IsA<gtk::Widget>) {
        let Some(p) = w.compute_point(&self.w.main, &gtk::graphene::Point::new(0.0, 0.0)) else { return };
        let adj = self.w.scroller.vadjustment();
        let target = (p.y() as f64 - 8.0).clamp(adj.lower(), (adj.upper() - adj.page_size()).max(adj.lower()));
        let from = adj.value();
        let t0 = Instant::now();
        self.w.scroller.add_tick_callback(move |_, _| {
            let k = (t0.elapsed().as_secs_f64() / 0.45).min(1.0);
            adj.set_value(from + (target - from) * draw::ease_soft(k));
            if k >= 1.0 {
                glib::ControlFlow::Break
            } else {
                glib::ControlFlow::Continue
            }
        });
    }

    fn update_stats(self: &Rc<Self>, s: Stats) {
        {
            let mut st = self.st.borrow_mut();
            st.hist_t.push_back(Instant::now());
            if st.hist_t.len() > HMAX {
                st.hist_t.pop_front();
            }
            let mut push = |k: &'static str, v: f64| {
                let h = st.hist.entry(k).or_default();
                h.push_back(v);
                if h.len() > HMAX {
                    h.pop_front();
                }
            };
            push("cpu", s.cpu);
            push("ram", s.ram);
            if let Some(sto) = &s.storage {
                push("disk", sto.percent);
            }
            st.last = Some(s.clone());
        }
        self.glances_state(true);
        self.put("cpu", "CPU", s.cpu);
        self.put("ram", "RAM", s.ram);
        if let Some(sto) = &s.storage {
            self.put("disk", "STORAGE", sto.percent);
        }
        self.w.down.set_text(&fmt_rate(s.net.down));
        self.w.up.set_text(&fmt_rate(s.net.up));
        let exp = self.st.borrow().expanded.clone();
        if let Some(e) = exp {
            self.render_detail(&e);
        }
    }

    pub fn render_detail(self: &Rc<Self>, id: &str) {
        let cards = self.cards.borrow();
        let Some(c) = cards.get(id) else { return };
        let st = self.st.borrow();
        let Some(last) = st.last.clone() else { return };
        let det = &c.det;
        while let Some(ch) = det.first_child() {
            det.remove(&ch);
        }
        det.set_visible(true);
        let glass = self.glass.get();
        let d = st.dets.get(id);
        let lim = |k: &str| st.set.limit(k);
        let tcol = |t: f64, k: &str| {
            let l = lim(k);
            if t >= l.hot as f64 {
                Some(config::RED)
            } else if t >= l.warm as f64 {
                Some(config::ORANGE)
            } else {
                None
            }
        };
        let graph = |key: &str| -> gtk::Widget { history(&st, key) };
        let zready = st.set.platform == "zimaos" && st.has_login;
        let z = if zready { st.z_sys } else { None };
        let zd = if zready { st.z_drives.clone() } else { None };
        match (id, d) {
            ("cpu", d) => {
                if let Some(Detail::Cpu(d)) = d {
                    let g = dgrid();
                    g.append(&stat(&pc(d.user), "USER", None));
                    g.append(&stat(&pc(d.system), "SYSTEM", None));
                    if d.iowait.is_some() {
                        g.append(&stat(&pc(d.iowait), "IO WAIT", None));
                    }
                    if let Some(l) = d.load {
                        g.append(&stat(&format!("{:.2}", l[0]), "LOAD 1M", None));
                        g.append(&stat(&format!("{:.2}", l[1]), "LOAD 5M", None));
                        g.append(&stat(&format!("{:.2}", l[2]), "LOAD 15M", None));
                    }
                    g.append(&stat(&trim_num(d.n_cores), "CORES", None));
                    // ZimaOS: the CPU power draw, and its CPU temperature when Glances has none
                    if let Some(w) = z.and_then(|z| z.watts) {
                        g.append(&stat(&format!("{w:.1} W"), "POWER", None));
                    }
                    if let Some(t) = d.temp.or(z.and_then(|z| z.cpu_temp)) {
                        g.append(&stat(&format!("{} °C", config::js_round(t)), "TEMP", tcol(t, "cpu")));
                    }
                    if let (Some(t), true) = (d.hot, d.n_core_temps > 1) {
                        g.append(&stat(&format!("{} °C", config::js_round(t)), "HOTTEST CORE", tcol(t, "cpu")));
                    }
                    det.append(&g);
                    if !d.cores.is_empty() {
                        det.append(&section("PER CORE", &draw::cores(d.cores.clone())));
                    }
                }
                det.append(&section("HISTORY", &graph("cpu")));
                if let Some(Detail::Cpu(d)) = d {
                    if let Some(t) = toplist("TOP CPU USERS", d.top.iter().map(|t| (t.name.clone(), t.pct, format!("{:.1}%", t.pct))).collect(), glass) {
                        det.append(&t);
                    }
                }
            }
            ("ram", d) => {
                if let Some(Detail::Ram(d)) = d {
                    det.append(&bigval(&format_bytes(d.used), &format!("/ {}", format_bytes(d.total))));
                    let g = dgrid();
                    g.append(&stat(&format_bytes(d.available), "AVAILABLE", None));
                    if let Some(c) = d.cached {
                        g.append(&stat(&format_bytes(c), "CACHED", None));
                    }
                    if let (Some(u), Some(t)) = (d.swap_used, d.swap_total) {
                        if t > 0.0 {
                            g.append(&stat(&format!("{} / {}", format_bytes(u), format_bytes(t)), "SWAP", None));
                        }
                    }
                    det.append(&g);
                }
                det.append(&section("HISTORY", &graph("ram")));
                if let Some(Detail::Ram(d)) = d {
                    let rows = d.top.iter().map(|t| (t.name.clone(), t.bytes, format!("{} · {:.1}%", format_bytes(t.bytes), t.pct))).collect();
                    if let Some(t) = toplist("TOP MEMORY USERS", rows, glass) {
                        det.append(&t);
                    }
                }
            }
            (_, d) => {
                if let Some(sm) = &last.storage {
                    det.append(&bigval(&format_bytes(sm.used), &format!("/ {} · {} {}", format_bytes(sm.size), sm.count, if sm.count == 1 { "drive" } else { "drives" })));
                    let g = dgrid();
                    g.append(&stat(&format_bytes(sm.free), "FREE", None));
                    g.append(&stat(&pc(Some(sm.percent)), "USED", None));
                    det.append(&g);
                }
                det.append(&section("HISTORY", &graph("disk")));
                let dd = match d {
                    Some(Detail::Disk(x)) => Some(x),
                    _ => None,
                };
                if let Some(dd) = dd {
                    if !dd.nvme.is_empty() || !dd.hdd.is_empty() {
                        let g = dgrid();
                        for (j, s) in dd.nvme.iter().enumerate() {
                            g.append(&stat(&format!("{} °C", config::js_round(s.value)), &format!("NVME {}", j + 1), tcol(s.value, "nvme")));
                        }
                        for (j, s) in dd.hdd.iter().enumerate() {
                            g.append(&stat(&format!("{} °C", config::js_round(s.value)), &hdd_label(&s.label, j), tcol(s.value, "hdd")));
                        }
                        det.append(&section("TEMPERATURES", &g));
                    } else if zd.as_ref().map(|z| z.is_empty()).unwrap_or(true) {
                        det.append(&hintl("NO DRIVE TEMPERATURES REPORTED BY GLANCES"));
                    }
                }
                // ZimaOS: each drive's SMART health and temperature (no drivetemp module needed)
                if let Some(zd) = zd.as_ref().filter(|z| !z.is_empty()) {
                    let list = bx(gtk::Orientation::Vertical, 10, "");
                    for x in zd {
                        let k = if x.path.contains("nvme") || x.name.starts_with("nvme") { "nvme" } else if x.kind.eq_ignore_ascii_case("SSD") { "ssd" } else { "hdd" };
                        let b = bx(gtk::Orientation::Vertical, 10, "drv");
                        let h = bx(gtk::Orientation::Horizontal, 8, "");
                        let n = modal::label(if x.model.is_empty() { &x.name } else { &x.model }, "drvn");
                        n.set_hexpand(true);
                        n.set_ellipsize(gtk::pango::EllipsizeMode::End);
                        h.append(&n);
                        h.append(&modal::label(&format!("{}{}", x.name, if x.size > 0.0 { format!(" · {}", format_bytes(x.size)) } else { String::new() }), "drvs"));
                        b.append(&h);
                        let g = dgrid();
                        g.append(&stat(if x.health { "OK" } else { "FAILING" }, "SMART", Some(if x.health { config::GREEN } else { config::RED })));
                        if let Some(t) = x.temp {
                            g.append(&stat(&format!("{} °C", config::js_round(t)), "TEMP", tcol(t, if k == "hdd" { "hdd" } else { "nvme" })));
                        }
                        g.append(&stat(&k.to_uppercase(), "TYPE", None));
                        b.append(&g);
                        list.append(&b);
                    }
                    det.append(&section("DRIVE HEALTH (ZIMAOS)", &list));
                }
                if !last.disks.is_empty() {
                    let list = bx(gtk::Orientation::Vertical, 10, "");
                    for (i, dk) in last.disks.iter().enumerate() {
                        let b = bx(gtk::Orientation::Vertical, 10, "drv");
                        let h = bx(gtk::Orientation::Horizontal, 8, "");
                        let n = modal::label(&dk.name, "drvn");
                        n.set_hexpand(true);
                        n.set_ellipsize(gtk::pango::EllipsizeMode::End);
                        h.append(&n);
                        h.append(&modal::label(&format!("{} / {} · {}", format_bytes(dk.used), format_bytes(dk.size), pc(Some(dk.percent))), "drvs"));
                        b.append(&h);
                        b.append(&draw::bar(dk.percent / 100.0, config::color_for(dk.percent), 8, glass));
                        let g = dgrid();
                        g.append(&stat(&format_bytes(dk.free.unwrap_or(dk.size - dk.used)), "FREE", None));
                        let dev = dk.device.rsplit('/').next().unwrap_or("");
                        g.append(&stat(if dev.is_empty() { "-" } else { dev }, "DEVICE", None));
                        g.append(&stat(if dk.fstype.is_empty() { "-" } else { &dk.fstype }, "TYPE", None));
                        if let Some(Some(io)) = dd.map(|x| x.io.get(i).copied().flatten()) {
                            g.append(&stat(&format!("{}/s", format_bytes(io.read)), "READ", None));
                            g.append(&stat(&format!("{}/s", format_bytes(io.write)), "WRITE", None));
                        }
                        if let Some(Some(tp)) = dd.map(|x| x.temps.get(i).cloned().flatten()) {
                            let mx = tp.values.iter().copied().fold(f64::MIN, f64::max);
                            let lab = if tp.kind == "nvme" { "NVME TEMP" } else if tp.values.len() > 1 { "HOTTEST HDD" } else { "HDD TEMP" };
                            g.append(&stat(&format!("{} °C", config::js_round(mx)), lab, tcol(mx, tp.kind)));
                        } else if let Some(t) = zd.as_ref().and_then(|z| zima::drive_for(&dk.device, z)).and_then(|x| x.temp) {
                            g.append(&stat(&format!("{} °C", config::js_round(t)), "TEMP", tcol(t, if dk.device.contains("nvme") { "nvme" } else { "hdd" })));
                        }
                        b.append(&g);
                        list.append(&b);
                    }
                    det.append(&section("DRIVES", &list));
                }
            }
        }
        let h = hintl("CLICK TO CLOSE");
        h.set_xalign(0.5);
        det.append(&h);
    }

    // ---------- ZimaOS ----------

    pub fn is_zima(&self) -> bool {
        self.st.borrow().set.platform == "zimaos"
    }

    /// ZimaOS server with its login saved: the app controls, updates, power and drive health show.
    pub fn zima_ready(&self) -> bool {
        let s = self.st.borrow();
        s.set.platform == "zimaos" && s.has_login
    }

    /// Installed apps the App Store has a newer version of.
    pub fn z_updates(&self) -> Vec<ZApp> {
        if !self.zima_ready() {
            return vec![];
        }
        let s = self.st.borrow();
        s.z_apps.clone().unwrap_or_default().into_iter().filter(|a| s.z_upg.contains(&a.id.to_lowercase())).collect()
    }

    pub fn z_updatable(&self, a: &ZApp) -> bool {
        self.st.borrow().z_upg.contains(&a.id.to_lowercase())
    }

    /// The ZimaOS app a button points at.
    pub fn z_app_for(&self, sv: &Service) -> Option<ZApp> {
        if !self.zima_ready() {
            return None;
        }
        let s = self.st.borrow();
        let host = s.set.server_host();
        s.z_apps.as_ref().and_then(|apps| zima::app_for(sv, apps, &host).cloned())
    }

    /// The "↑ N updates" chip next to Widget layout (hidden with nothing to update).
    pub fn show_updates_chip(&self) {
        let n = self.z_updates().len();
        let wide = self.st.borrow().widget_mode;
        self.w.updb.set_visible(!wide && n > 0);
        self.w.updb.set_label(&format!("↑ {n} {}", if n == 1 { "update" } else { "updates" }));
    }

    /// Asks ZimaOS for its apps and their updates; done gets an error or None.
    pub fn refresh_zima(self: &Rc<Self>, done: Option<Box<dyn FnOnce(Option<String>)>>) {
        if !self.zima_ready() {
            if let Some(d) = done {
                d(Some(zima::NEED_LOGIN.into()));
            }
            return;
        }
        if self.st.borrow().z_busy {
            // already asking: try again shortly
            if let Some(d) = done {
                let a = self.clone();
                glib::timeout_add_local_once(Duration::from_millis(150), move || a.refresh_zima(Some(d)));
            }
            return;
        }
        let (set, cur, gen) = {
            let mut s = self.st.borrow_mut();
            s.z_busy = true;
            (s.set.clone(), s.cur.clone(), s.gen)
        };
        let secrets = self.ctx.secrets.clone();
        let a = self.clone();
        bg(
            move || {
                let (apps, upg) = std::thread::scope(|sc| {
                    let u = sc.spawn(|| zima::updates(&set, &cur, &secrets).unwrap_or_default());
                    (zima::apps(&set, &cur, &secrets), u.join().unwrap_or_default())
                });
                (apps, upg)
            },
            move |(apps, upg)| {
                let err = {
                    let mut s = a.st.borrow_mut();
                    s.z_busy = false;
                    s.last_z_apps = Some(Instant::now());
                    match apps {
                        Ok(list) if s.gen == gen => {
                            s.z_apps = Some(list);
                            s.z_upg = upg;
                            None
                        }
                        Ok(_) => None,
                        Err(e) => Some(e.msg),
                    }
                };
                a.show_updates_chip();
                if let Some(d) = done {
                    d(err);
                }
            },
        );
    }

    /// Start / Stop / Restart / Update; the result goes to the toast.
    pub fn z_action(self: &Rc<Self>, app: &ZApp, action: &str, done: Option<Box<dyn FnOnce(bool, String)>>) {
        let (set, cur) = {
            let s = self.st.borrow();
            (s.set.clone(), s.cur.clone())
        };
        let secrets = self.ctx.secrets.clone();
        let (id, title, act) = (app.id.clone(), app.title.clone(), action.to_string());
        let a = self.clone();
        bg(
            move || {
                let r = if act == "update" { zima::update(&set, &id, &cur, &secrets) } else { zima::set_status(&set, &id, &act, &cur, &secrets) };
                if act != "update" && r.is_ok() {
                    std::thread::sleep(Duration::from_millis(1500));
                }
                r.map(|_| match act.as_str() {
                    "update" => format!("{title}: update started. ZimaOS pulls the new version in the background."),
                    "start" => format!("{title} started"),
                    "stop" => format!("{title} stopped"),
                    _ => format!("{title} restarted"),
                })
            },
            move |r| {
                a.st.borrow_mut().status_dirty = true;
                a.refresh_zima(None);
                let (ok, msg) = match r {
                    Ok(m) => (true, m),
                    Err(e) => (false, if e.msg.is_empty() { "ZimaOS didn't accept the command".into() } else { e.msg }),
                };
                match done {
                    Some(d) => d(ok, msg),
                    None => a.toast.show(&msg, 4000),
                }
            },
        );
    }

    /// Adds picked ZimaOS apps as buttons.
    pub fn add_zima_apps(self: &Rc<Self>, apps: &[ZApp]) {
        if apps.is_empty() {
            return;
        }
        {
            let host = self.st.borrow().set.server_host();
            let mut store = self.ctx.store.borrow_mut();
            let mut d = store.load_data();
            for l in layout::add_zima_apps(&mut d, apps, &host) {
                self.ctx.logos.clear_fail(&l);
            }
            store.save_data(&d);
        }
        self.st.borrow_mut().status_dirty = true;
        self.refresh(false);
        self.refresh_logos();
    }

    // ---------- live updates ----------

    fn tick(self: &Rc<Self>) {
        if !self.win.is_visible() {
            return;
        }
        let (set, gen, cur) = {
            let s = self.st.borrow();
            (s.set.clone(), s.gen, s.cur.clone())
        };
        if set.server_host().is_empty() {
            self.offline_setup();
            return;
        }
        let services = self.st.borrow().layout.all.clone();
        // LIVE / OFFLINE: is anything on the server answering? A changed address is checked straight away.
        let rk = format!("{}|{}|{}|{}", set.server_url(), set.platform, set.monitoring, set.base_url);
        let due_reach = {
            let s = self.st.borrow();
            !s.reach_busy && (rk != s.reach_for || s.last_reach.map(|t| t.elapsed() > Duration::from_secs(config::REACH_SECONDS)).unwrap_or(true))
        };
        if due_reach {
            {
                let mut s = self.st.borrow_mut();
                s.reach_busy = true;
                s.reach_for = rk;
            }
            let (s2, sv) = (set.clone(), services.clone());
            let a = self.clone();
            bg(move || net::server_reachable(&s2, &sv, 4.0), move |r| {
                let same = a.st.borrow().gen == gen;
                {
                    let mut s = a.st.borrow_mut();
                    if same {
                        s.reach_busy = false;
                        s.last_reach = Some(Instant::now());
                    }
                }
                if same {
                    a.set_status(!r.is_empty());
                }
            });
        }
        // Monitoring off: no requests to Glances at all
        let due_stats = {
            let s = self.st.borrow();
            set.monitoring && !s.stats_busy && Instant::now() >= s.next_stats
        };
        if due_stats {
            self.st.borrow_mut().stats_busy = true;
            let expanded = {
                let s = self.st.borrow();
                if s.mon_open {
                    s.expanded.clone()
                } else {
                    None
                }
            };
            let s2 = set.clone();
            let pass = self.ctx.secrets.glances_pass(&cur);
            let cache = self.fs_cache.clone();
            // ZimaOS: CPU power and temperature for the CPU card, drive health for the storage card (once a minute)
            let zready = self.zima_ready();
            let want_drives = {
                let s = self.st.borrow();
                s.z_drives.is_none() || s.z_drives_at.map(|t| t.elapsed() > Duration::from_secs(60)).unwrap_or(true)
            };
            let secrets = self.ctx.secrets.clone();
            let a = self.clone();
            bg(
                move || {
                    let api = |p: &str| net::glances_request(&s2, p, &pass);
                    let st = glances::get_stats(&api, &s2, true, true, Some((&cache, Duration::from_secs(10))));
                    let det = match (&st, &expanded) {
                        (Ok(stats), Some(k)) => glances::get_detail(&api, k, stats).ok().map(|d| (k.clone(), d)),
                        _ => None,
                    };
                    let (mut zs, mut zd) = (None, None);
                    if zready && st.is_ok() {
                        match expanded.as_deref() {
                            Some("cpu") => zs = zima::sys(&s2, &cur, &secrets).ok(),
                            Some("disk") if want_drives => zd = zima::drives(&s2, &cur, &secrets).ok(),
                            _ => {}
                        }
                    }
                    (st, det, zs, zd)
                },
                move |(st, det, zs, zd)| {
                    if a.st.borrow().gen != gen {
                        return;
                    }
                    {
                        let mut s = a.st.borrow_mut();
                        s.stats_busy = false;
                        s.next_stats = Instant::now() + Duration::from_secs(s.set.live_seconds.max(1) as u64);
                        if let Some((k, d)) = det {
                            s.dets.insert(k, d);
                        }
                        if zs.is_some() {
                            s.z_sys = zs;
                        }
                        if zd.is_some() {
                            s.z_drives = zd;
                            s.z_drives_at = Some(Instant::now());
                        }
                    }
                    match st {
                        Ok(stats) => a.update_stats(stats),
                        Err(_) => a.glances_state(false),
                    }
                },
            );
        }
        // ZimaOS apps and their updates: asked again once a minute
        let due_zima = {
            let s = self.st.borrow();
            !s.z_busy && s.last_z_apps.map(|t| t.elapsed() > Duration::from_secs(60)).unwrap_or(true)
        };
        if due_zima && self.zima_ready() {
            self.refresh_zima(None);
        }
        // up / down dots
        let due_status = {
            let s = self.st.borrow();
            !s.status_busy && (s.status_dirty || s.last_status.map(|t| t.elapsed() > Duration::from_secs(set.status_seconds as u64)).unwrap_or(true))
        };
        if due_status {
            {
                let mut s = self.st.borrow_mut();
                s.status_busy = true;
                s.status_dirty = false;
            }
            let timeout = set.ping_timeout as f64;
            let a = self.clone();
            bg(move || net::check_services(&services, timeout), move |m| {
                if a.st.borrow().gen != gen {
                    return;
                }
                {
                    let mut s = a.st.borrow_mut();
                    s.status_busy = false;
                    s.last_status = Some(Instant::now());
                    s.svc_up = m;
                }
                a.apply_dots();
            });
        }
    }

    /// Downloads logos not saved yet, then redraws (without holding up the dashboard).
    pub fn refresh_logos(self: &Rc<Self>) {
        let (use_logos, busy) = {
            let s = self.st.borrow();
            (s.set.use_logos, s.logo_busy)
        };
        if !use_logos || busy {
            return;
        }
        self.st.borrow_mut().logo_busy = true;
        let (urls, all_urls) = {
            let mut store = self.ctx.store.borrow_mut();
            let s = self.st.borrow();
            let mut u: Vec<String> = s.layout.all.iter().map(|x| x.logo.clone()).collect();
            u.extend(s.layout.groups.iter().map(|g| g.logo.clone()));
            // logos of every server stay (only the ones no button uses are removed)
            let mut all = vec![];
            let ids: Vec<String> = store.list_servers().into_iter().map(|x| x.0).collect();
            let keep = store.cur.clone();
            for id in ids {
                store.cur = id;
                let d = store.load_data();
                let lay = layout::get_layout(&d, "localhost");
                all.extend(lay.all.iter().map(|x| x.logo.clone()));
                all.extend(lay.groups.iter().map(|g| g.logo.clone()));
            }
            store.cur = keep;
            (u, all)
        };
        let logos = self.ctx.logos.clone();
        let a = self.clone();
        bg(
            move || {
                let got = logos.ensure(&urls, 8.0, &net::get_bytes);
                logos.prune(&all_urls);
                got
            },
            move |got| {
                a.st.borrow_mut().logo_busy = false;
                if got {
                    a.ctx.textures.borrow_mut().clear();
                    a.draw(false);
                }
            },
        );
    }

    // ---------- service buttons ----------

    /// An icon: the saved logo, or the emoji.
    pub fn icon_widget(&self, logo: &str, icon: &str, px: i32) -> gtk::Widget {
        if self.st.borrow().set.use_logos {
            if let Some(t) = self.ctx.texture(logo) {
                let img = gtk::Image::from_paintable(Some(&t));
                img.set_pixel_size(px);
                img.add_css_class("ic");
                img.set_halign(gtk::Align::Start);
                return img.upcast();
            }
        }
        let l = gtk::Label::new(Some(icon));
        l.add_css_class("ic");
        l.set_xalign(0.0);
        l.upcast()
    }

    pub fn open_url(&self, url: &str) {
        let l = gtk::UriLauncher::new(url);
        let t = self.toast.clone();
        let u = url.to_string();
        l.launch(Some(&self.win), gio::Cancellable::NONE, move |r| {
            if r.is_err() {
                t.show(&format!("Couldn't open {u}"), 3000);
            }
        });
    }

    fn svc_dot_class(&self, item: &Item) -> &'static str {
        let s = self.st.borrow();
        match item {
            Item::Svc(sv) => match s.svc_up.get(&sv.id) {
                Some(true) => "up",
                Some(false) => "dn",
                None => "",
            },
            Item::Group(g) => group_state(g, &s.svc_up),
        }
    }

    /// Update every status dot (without redrawing the buttons).
    pub fn apply_dots(self: &Rc<Self>) {
        let s = self.st.borrow();
        for (id, dot) in self.dots.borrow().iter() {
            let cls = match s.layout.groups.iter().find(|g| &g.id == id) {
                Some(g) => group_state(g, &s.svc_up),
                None => match s.svc_up.get(id) {
                    Some(true) => "up",
                    Some(false) => "dn",
                    None => "",
                },
            };
            for c in ["up", "dn", "part"] {
                if c == cls {
                    dot.add_css_class(c);
                } else {
                    dot.remove_css_class(c);
                }
            }
        }
    }

    /// Redraw the buttons and stacks (anim: the glass look's entrance).
    pub fn draw(self: &Rc<Self>, _anim: bool) {
        let (wide, dash, widget, hidden, rows) = {
            let s = self.st.borrow();
            (s.widget_mode, s.layout.dash.clone(), s.layout.widget.clone(), s.layout.hidden.clone(), s.set.widget_rows)
        };
        let cols = if wide { 4 } else { self.st.borrow().set.columns as i32 };
        self.dots.borrow_mut().clear();
        let svc = &self.w.svc;
        while let Some(c) = svc.first_child() {
            svc.remove(&c);
        }
        while let Some(c) = self.w.hid.first_child() {
            self.w.hid.remove(&c);
        }
        if wide {
            svc.set_css_classes(&["wide"]);
            self.win.add_css_class("wide");
        } else {
            svc.set_css_classes(&["dash"]);
            self.win.remove_css_class("wide");
        }
        let list = if wide { &widget } else { &dash };
        let widget_max = (rows * 4) as usize;
        if list.is_empty() {
            let e = bx(gtk::Orientation::Vertical, 0, "emptybox");
            let l = gtk::Label::new(Some(if wide { "Nothing on the widget yet. Drag buttons up from the panel below." } else { "No buttons yet. Click + below to add your first service." }));
            l.set_wrap(true);
            l.set_justify(gtk::Justification::Center);
            l.set_vexpand(true);
            e.append(&l);
            self.add_drop(&e, Target::End(if wide { Cont::Shown } else { Cont::Top }));
            svc.append(&e);
        } else {
            let grid = self.grid(cols, wide);
            let mut used = 0usize;
            let (mut col, mut row) = (0, 0);
            for item in list {
                let w: gtk::Widget = match (item, wide) {
                    (Item::Group(g), false) => self.make_group(g, cols),
                    (Item::Group(g), true) => self.make_group_pill(g, if wide { Cont::Shown } else { Cont::Top }),
                    (Item::Svc(sv), _) => self.make_btn(sv, if wide { Cont::Shown } else { Cont::Top }),
                };
                if wide {
                    let sl = layout::widget_slots(item);
                    if used >= widget_max && sl > 0 {
                        w.add_css_class("over");
                        if let Some(b) = w.first_child() {
                            b.add_css_class("over");
                        }
                    }
                    used += sl;
                }
                let full = !wide && item.is_group();
                if full {
                    if col != 0 {
                        row += 1;
                        col = 0;
                    }
                    grid.attach(&w, 0, row, cols, 1);
                    row += 1;
                } else {
                    grid.attach(&w, col, row, 1, 1);
                    col += 1;
                    if col == cols {
                        col = 0;
                        row += 1;
                    }
                }
            }
            Self::keep_columns(&grid, cols);
            self.add_drop(&grid, Target::End(if wide { Cont::Shown } else { Cont::Top }));
            svc.append(&grid);
        }
        if wide {
            if hidden.is_empty() {
                let l = gtk::Label::new(Some("Drag buttons here to hide them from the widget"));
                l.add_css_class("hid-empty");
                l.set_wrap(true);
                l.set_vexpand(true);
                l.set_justify(gtk::Justification::Center);
                self.w.hid.append(&l);
            } else {
                let grid = self.grid(4, true);
                for (i, item) in hidden.iter().enumerate() {
                    let w = match item {
                        Item::Group(g) => self.make_group_pill(g, Cont::Hidden),
                        Item::Svc(sv) => self.make_btn(sv, Cont::Hidden),
                    };
                    grid.attach(&w, (i % 4) as i32, (i / 4) as i32, 1, 1);
                }
                Self::keep_columns(&grid, 4);
                self.w.hid.append(&grid);
            }
            self.add_drop(&self.w.hid, Target::End(Cont::Hidden));
        }
        self.w.sect.set_text(if wide { "WIDGET LAYOUT" } else { "SERVICES" });
        self.w.layb.set_label(if wide { "Done" } else { "Widget layout" });
        if wide {
            self.w.layb.add_css_class("on");
        } else {
            self.w.layb.remove_css_class("on");
        }
        self.w.wextra.set_visible(wide);
        self.w.addwrap.set_visible(!wide);
        self.show_updates_chip();
    }

    fn grid(&self, cols: i32, wide: bool) -> gtk::Grid {
        let g = gtk::Grid::new();
        g.set_column_homogeneous(true);
        let (c3, compact) = (cols == 3, self.st.borrow().set.button_size == "compact");
        let gap = if wide { 6 } else if compact { 8 } else if c3 { 10 } else { 12 };
        g.set_column_spacing(gap as u32);
        g.set_row_spacing(gap as u32);
        g
    }

    /// Keeps the column count when no button reaches the last column (an empty cell in row 0 holds it).
    fn keep_columns(g: &gtk::Grid, cols: i32) {
        let mut c = g.first_child();
        while let Some(w) = c {
            let (col, _, span, _) = g.query_child(&w);
            if col + span >= cols {
                return;
            }
            c = w.next_sibling();
        }
        let ph = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        ph.set_hexpand(true);
        ph.set_can_target(false);
        g.attach(&ph, cols - 1, 0, 1, 1);
    }

    fn make_btn(self: &Rc<Self>, sv: &Service, cont: Cont) -> gtk::Widget {
        let wide = self.st.borrow().widget_mode;
        let compact = self.st.borrow().set.button_size == "compact";
        let b = gtk::Button::new();
        b.add_css_class("svc");
        let ov = gtk::Overlay::new();
        let content = if wide {
            let r = bx(gtk::Orientation::Horizontal, 4, "");
            r.append(&self.icon_widget(&sv.logo, &sv.icon, 14));
            let n = modal::label(&sv.name, "nm");
            n.set_ellipsize(gtk::pango::EllipsizeMode::End);
            n.set_hexpand(true);
            n.set_margin_end(9);
            r.append(&n);
            r
        } else {
            let c = bx(gtk::Orientation::Vertical, if compact { 2 } else { 4 }, "");
            c.append(&self.icon_widget(&sv.logo, &sv.icon, if compact { 22 } else { 26 }));
            let n = modal::label(&sv.name, "nm");
            n.set_ellipsize(gtk::pango::EllipsizeMode::End);
            n.set_margin_end(10);
            c.append(&n);
            let p = modal::label(&sv.label, "pt");
            p.set_ellipsize(gtk::pango::EllipsizeMode::End);
            c.append(&p);
            c
        };
        ov.set_child(Some(&content));
        let dot = bx(gtk::Orientation::Horizontal, 0, "sd");
        self.dots.borrow_mut().push((sv.id.clone(), dot.clone()));
        let cls = self.svc_dot_class(&Item::Svc(sv.clone()));
        if !cls.is_empty() {
            dot.add_css_class(cls);
        }
        dot.set_halign(gtk::Align::End);
        dot.set_valign(if wide { gtk::Align::Center } else { gtk::Align::Start });
        dot.set_can_target(false);
        ov.add_overlay(&dot);
        b.set_child(Some(&ov));
        b.set_tooltip_text(Some(&format!("{}\n{}", sv.name, sv.url)));
        let url = sv.url.clone();
        let a = self.clone();
        b.connect_clicked(move |_| {
            if !a.modals.ignoring() {
                a.open_url(&url);
            }
        });
        let item = Item::Svc(sv.clone());
        let sc = Scaler::new(&b);
        press_squish(&sc, 0.92, self.glass.clone());
        self.bind_item(&sc, &b, &item, cont);
        sc.upcast()
    }

    fn make_group_pill(self: &Rc<Self>, g: &Stack, cont: Cont) -> gtk::Widget {
        let b = gtk::Button::new();
        b.add_css_class("svc");
        let ov = gtk::Overlay::new();
        let r = bx(gtk::Orientation::Horizontal, 4, "");
        r.append(&self.icon_widget(&g.logo, &g.icon, 14));
        let n = modal::label(&format!("{}{}", g.name, if g.expand { format!(" ×{}", g.members.len()) } else { String::new() }), "nm");
        n.set_ellipsize(gtk::pango::EllipsizeMode::End);
        n.set_hexpand(true);
        n.set_margin_end(9);
        r.append(&n);
        ov.set_child(Some(&r));
        let dot = bx(gtk::Orientation::Horizontal, 0, "sd");
        self.dots.borrow_mut().push((g.id.clone(), dot.clone()));
        let cls = self.svc_dot_class(&Item::Group(g.clone()));
        if !cls.is_empty() {
            dot.add_css_class(cls);
        }
        dot.set_halign(gtk::Align::End);
        dot.set_valign(gtk::Align::Center);
        ov.add_overlay(&dot);
        b.set_child(Some(&ov));
        let a = self.clone();
        let g2 = g.clone();
        b.connect_clicked(move |_| {
            if !a.modals.ignoring() {
                dialogs::open_stack(&a, Some(&g2), None);
            }
        });
        let sc = Scaler::new(&b);
        press_squish(&sc, 0.92, self.glass.clone());
        self.bind_item(&sc, &b, &Item::Group(g.clone()), cont);
        sc.upcast()
    }

    /// A stack in the dashboard: header (click = open / close, hold = edit) and its buttons.
    fn make_group(self: &Rc<Self>, g: &Stack, cols: i32) -> gtk::Widget {
        let wrap = bx(gtk::Orientation::Vertical, 0, "grp");
        let open = self.st.borrow().open_groups.contains(&g.id);
        let h = gtk::Button::new();
        h.add_css_class("monbtn");
        h.add_css_class("grph");
        if open {
            h.add_css_class("open");
        }
        let row = bx(gtk::Orientation::Horizontal, 10, "");
        row.append(&self.icon_widget(&g.logo, &g.icon, 22));
        // no ellipsis: GTK measures ellipsized labels without their letter spacing (names are at most 24 characters)
        let n = spaced_label(&g.name, "gname", 1.8);
        row.append(&n);
        let c = modal::label(&g.members.len().to_string(), "gc");
        c.set_hexpand(true);
        row.append(&c);
        let dot = bx(gtk::Orientation::Horizontal, 0, "sd");
        self.dots.borrow_mut().push((g.id.clone(), dot.clone()));
        let cls = self.svc_dot_class(&Item::Group(g.clone()));
        if !cls.is_empty() {
            dot.add_css_class(cls);
        }
        dot.set_valign(gtk::Align::Center);
        row.append(&dot);
        row.append(&chevron("chev"));
        h.set_child(Some(&row));
        let hsc = Scaler::new(&h);
        press_squish(&hsc, 0.92, self.glass.clone());
        wrap.append(&hsc);
        let rev = gtk::Revealer::new();
        rev.set_transition_type(gtk::RevealerTransitionType::SlideDown);
        rev.set_transition_duration(400);
        rev.set_reveal_child(open);
        let body = bx(gtk::Orientation::Vertical, 0, "gbody");
        body.set_margin_top(12);
        if g.members.is_empty() {
            let e = bx(gtk::Orientation::Vertical, 0, "");
            e.set_size_request(-1, 58);
            let l = gtk::Label::new(Some("Drag buttons here"));
            l.add_css_class("gbody-empty");
            l.set_vexpand(true);
            e.append(&l);
            body.append(&e);
        } else {
            let grid = self.grid(cols, false);
            for (i, sv) in g.members.iter().enumerate() {
                let w = self.make_btn(sv, Cont::Body(g.id.clone()));
                grid.attach(&w, i as i32 % cols, i as i32 / cols, 1, 1);
            }
            Self::keep_columns(&grid, cols);
            body.append(&grid);
        }
        self.add_drop(&body, Target::End(Cont::Body(g.id.clone())));
        rev.set_child(Some(&body));
        wrap.append(&rev);
        let a = self.clone();
        let id = g.id.clone();
        let (h2, rev2) = (h.clone(), rev.clone());
        h.connect_clicked(move |_| {
            if a.modals.ignoring() {
                return;
            }
            let open = {
                let mut s = a.st.borrow_mut();
                if !s.open_groups.remove(&id) {
                    s.open_groups.insert(id.clone());
                    true
                } else {
                    false
                }
            };
            rev2.set_reveal_child(open);
            if open {
                h2.add_css_class("open");
            } else {
                h2.remove_css_class("open");
            }
        });
        self.bind_item(&wrap, &h, &Item::Group(g.clone()), Cont::Top);
        wrap.upcast()
    }

    /// Hold / right-click = menu; drag = move it (between panels and into open stacks).
    fn bind_item(self: &Rc<Self>, moving: &impl IsA<gtk::Widget>, handle: &impl IsA<gtk::Widget>, item: &Item, cont: Cont) {
        let hold = gtk::GestureLongPress::new();
        let a = self.clone();
        let it = item.clone();
        hold.connect_pressed(move |g, _, _| {
            g.set_state(gtk::EventSequenceState::Claimed);
            a.modals.ignore_for(450);
            a.show_menu(&it);
        });
        handle.add_controller(hold);
        let rc = gtk::GestureClick::new();
        rc.set_button(gdk::BUTTON_SECONDARY);
        let a = self.clone();
        let it = item.clone();
        rc.connect_pressed(move |_, _, _, _| a.show_menu(&it));
        handle.add_controller(rc);
        // drag and drop
        let src = gtk::DragSource::new();
        src.set_actions(gdk::DragAction::MOVE);
        let id = item.id().to_string();
        src.connect_prepare(move |_, _, _| Some(gdk::ContentProvider::for_value(&format!("board-item:{id}").to_value())));
        let a = self.clone();
        let mv = moving.clone().upcast::<gtk::Widget>();
        let id = item.id().to_string();
        src.connect_drag_begin(move |s, _| {
            let p = gtk::WidgetPaintable::new(Some(&mv));
            s.set_icon(Some(&p), mv.width() / 2, mv.height() / 2);
            mv.add_css_class("lifted");
            *a.dragging.borrow_mut() = Some(id.clone());
        });
        let a = self.clone();
        let mv = moving.clone().upcast::<gtk::Widget>();
        src.connect_drag_end(move |_, _, _| {
            mv.remove_css_class("lifted");
            a.dragging.borrow_mut().take();
            a.modals.ignore_for(300);
        });
        handle.add_controller(src);
        self.add_drop(moving, Target::Item(cont, item.id().to_string()));
    }

    fn add_drop(self: &Rc<Self>, w: &impl IsA<gtk::Widget>, target: Target) {
        let dt = gtk::DropTarget::new(glib::Type::STRING, gdk::DragAction::MOVE);
        let a = self.clone();
        let t = target.clone();
        let wd = w.clone().upcast::<gtk::Widget>();
        dt.connect_enter(move |_, _, _| {
            if a.can_drop(&t) {
                wd.add_css_class("dropon");
                gdk::DragAction::MOVE
            } else {
                gdk::DragAction::empty()
            }
        });
        let wd = w.clone().upcast::<gtk::Widget>();
        dt.connect_leave(move |_| wd.remove_css_class("dropon"));
        let a = self.clone();
        let wd = w.clone().upcast::<gtk::Widget>();
        dt.connect_drop(move |_, v, _, _| {
            wd.remove_css_class("dropon");
            let Ok(s) = v.get::<String>() else { return false };
            let Some(src) = s.strip_prefix("board-item:") else { return false };
            if !a.can_drop(&target) {
                return false;
            }
            let src = src.to_string();
            let t = target.clone();
            let a2 = a.clone();
            // after the drag has finished
            glib::idle_add_local_once(move || a2.drop_on(&src, &t));
            true
        });
        w.add_controller(dt);
    }

    fn can_drop(&self, t: &Target) -> bool {
        let Some(src) = self.dragging.borrow().clone() else { return true };
        let is_group = src.starts_with("g:");
        let cont = match t {
            Target::Item(c, id) => {
                if *id == src {
                    return false;
                }
                c
            }
            Target::End(c) => c,
        };
        // a stack can't go inside another stack
        !(is_group && matches!(cont, Cont::Body(_)))
    }

    /// Moves an item in front of / after another one, or to the end of a panel, then saves.
    fn drop_on(self: &Rc<Self>, src: &str, target: &Target) {
        let wide = self.st.borrow().widget_mode;
        let lay = self.st.borrow().layout.clone();
        let mut store = self.ctx.store.borrow_mut();
        let mut data = store.load_data();
        if wide {
            let shown: Vec<String> = lay.widget.iter().map(|x| x.id().to_string()).collect();
            let hidden: Vec<String> = lay.hidden.iter().map(|x| x.id().to_string()).collect();
            let from = if shown.iter().any(|x| x == src) { Cont::Shown } else { Cont::Hidden };
            let mut lists = vec![(Cont::Shown, shown.clone()), (Cont::Hidden, hidden.clone())];
            move_in_lists(src, &from, target, &mut lists);
            let (shown2, hidden2) = (lists[0].1.clone(), lists[1].1.clone());
            if (&shown, &hidden) == (&shown2, &hidden2) {
                return;
            }
            let (shown, hidden) = (shown2, hidden2);
            layout::set_widget_order(&mut data, shown, hidden);
        } else {
            let top: Vec<String> = lay.dash.iter().map(|x| x.id().to_string()).collect();
            let bodies: Vec<(String, Vec<String>)> = lay.groups.iter().map(|g| (g.id.clone(), g.members.iter().map(|m| m.id.clone()).collect())).collect();
            let from = if top.iter().any(|x| x == src) {
                Cont::Top
            } else {
                bodies.iter().find(|(_, m)| m.iter().any(|x| x == src)).map(|(g, _)| Cont::Body(g.clone())).unwrap_or(Cont::Top)
            };
            let mut lists: Vec<(Cont, Vec<String>)> = vec![(Cont::Top, top.clone())];
            lists.extend(bodies.iter().map(|(g, m)| (Cont::Body(g.clone()), m.clone())));
            move_in_lists(src, &from, target, &mut lists);
            let top2 = lists[0].1.clone();
            let bodies2: Vec<(String, Vec<String>)> = lists[1..].iter().map(|(c, m)| (if let Cont::Body(g) = c { g.clone() } else { String::new() }, m.clone())).collect();
            if top2 == top && bodies2 == bodies {
                return;
            }
            let (top, bodies) = (top2, bodies2);
            layout::set_dash_order(&mut data, top, &bodies);
        }
        store.save_data(&data);
        drop(store);
        self.refresh(false);
    }

    pub fn show_menu(self: &Rc<Self>, item: &Item) {
        match item {
            Item::Group(g) => dialogs::open_stack(self, Some(g), None),
            Item::Svc(sv) => dialogs::open_menu(self, sv),
        }
    }

    /// Opened from a stack pill on the widget: open that stack and scroll to it.
    pub fn focus_stack(self: &Rc<Self>, id: &str) {
        if !self.st.borrow().layout.groups.iter().any(|g| g.id == id) {
            return;
        }
        if self.st.borrow().widget_mode {
            self.st.borrow_mut().widget_mode = false;
        }
        self.st.borrow_mut().open_groups.insert(id.to_string());
        self.draw(false);
        let a = self.clone();
        glib::timeout_add_local_once(Duration::from_millis(150), move || {
            let grid = a.w.svc.first_child();
            if let Some(g) = grid {
                let mut c = g.first_child();
                while let Some(w) = c {
                    if w.has_css_class("grp") {
                        a.scroll_to(&w);
                        break;
                    }
                    c = w.next_sibling();
                }
            }
        });
    }
}

/// Where an item can go.
#[derive(Clone, Debug, PartialEq)]
pub enum Cont {
    Top,
    Body(String),
    Shown,
    Hidden,
}

#[derive(Clone, Debug)]
pub enum Target {
    /// on top of another item
    Item(Cont, String),
    /// empty space at the end of a panel
    End(Cont),
}

/// Takes src out of its list and puts it at the target. Dropped on an item further down the same
/// panel, it goes after that item; otherwise in front of it.
fn move_in_lists(src: &str, from: &Cont, target: &Target, lists: &mut [(Cont, Vec<String>)]) {
    let (to, anchor) = match target {
        Target::Item(c, id) => (c.clone(), Some(id.clone())),
        Target::End(c) => (c.clone(), None),
    };
    let idx = |lists: &[(Cont, Vec<String>)], c: &Cont| lists.iter().position(|(k, _)| k == c);
    let (Some(fi), Some(ti)) = (idx(lists, from), idx(lists, &to)) else { return };
    let src_idx = lists[fi].1.iter().position(|x| x == src);
    let anchor_idx = anchor.as_ref().and_then(|a| lists[ti].1.iter().position(|x| x == a));
    let after = fi == ti && matches!((src_idx, anchor_idx), (Some(s), Some(t)) if s < t);
    lists[fi].1.retain(|x| x != src);
    let dst = &mut lists[ti].1;
    match anchor.and_then(|a| dst.iter().position(|x| *x == a)) {
        Some(i) => dst.insert(if after { i + 1 } else { i }, src.to_string()),
        None => dst.push(src.to_string()),
    }
}

/// Stack status: up = all up, dn = all down, part = some down.
pub fn group_state(g: &Stack, up: &HashMap<String, bool>) -> &'static str {
    let n = g.members.len();
    let (mut u, mut d) = (0, 0);
    for m in &g.members {
        match up.get(&m.id) {
            Some(true) => u += 1,
            Some(false) => d += 1,
            None => {}
        }
    }
    if n == 0 || u + d == 0 {
        ""
    } else if u == n {
        "up"
    } else if d == n {
        "dn"
    } else if d > 0 {
        "part"
    } else {
        ""
    }
}

pub fn fmt_rate(b: f64) -> String {
    format!("{}/s", format_bytes(b))
}

fn pc(v: Option<f64>) -> String {
    match v {
        None => "-".into(),
        Some(v) => format!("{}%", trim_num((v * 10.0).round() / 10.0)),
    }
}

fn trim_num(v: f64) -> String {
    if v.fract() == 0.0 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

fn hdd_label(l: &str, j: usize) -> String {
    let re = regex::Regex::new(r"(?i)(sd[a-z]+)").unwrap();
    match re.captures(l) {
        Some(m) => m[1].to_uppercase(),
        None => format!("HDD {}", j + 1),
    }
}

/// A label with letter spacing (in px) as a Pango attribute, so its width is measured right.
pub fn spaced_label(text: &str, class: &str, px: f64) -> gtk::Label {
    let l = modal::label(text, class);
    let attrs = gtk::pango::AttrList::new();
    attrs.insert(gtk::pango::AttrInt::new_letter_spacing((px * gtk::pango::SCALE as f64) as i32));
    l.set_attributes(Some(&attrs));
    l
}

fn dgrid() -> gtk::FlowBox {
    let f = gtk::FlowBox::new();
    f.set_selection_mode(gtk::SelectionMode::None);
    f.set_homogeneous(true);
    f.set_column_spacing(8);
    f.set_row_spacing(8);
    f.set_min_children_per_line(2);
    f.set_max_children_per_line(6);
    f.set_can_target(false);
    f
}

fn stat(v: &str, l: &str, color: Option<&str>) -> gtk::Widget {
    let b = bx(gtk::Orientation::Vertical, 4, "ds");
    b.set_size_request(86, -1);
    let vl = gtk::Label::new(None);
    match color {
        Some(c) => vl.set_markup(&format!("<span foreground=\"{c}\">{}</span>", glib::markup_escape_text(v))),
        None => vl.set_text(v),
    }
    vl.add_css_class("dsv");
    vl.set_ellipsize(gtk::pango::EllipsizeMode::End);
    b.append(&vl);
    let ll = gtk::Label::new(Some(l));
    ll.add_css_class("dsl");
    ll.set_ellipsize(gtk::pango::EllipsizeMode::End);
    b.append(&ll);
    b.upcast()
}

fn section(title: &str, w: &impl IsA<gtk::Widget>) -> gtk::Box {
    let b = bx(gtk::Orientation::Vertical, 8, "");
    let h = modal::label(title, "dh");
    h.set_margin_start(2);
    b.append(&h);
    b.append(w);
    b
}

fn hintl(t: &str) -> gtk::Label {
    let l = gtk::Label::new(Some(t));
    l.add_css_class("hint");
    l.set_wrap(true);
    l.set_justify(gtk::Justification::Center);
    l
}

fn bigval(main: &str, small: &str) -> gtk::Label {
    let l = gtk::Label::new(None);
    l.set_markup(&format!(
        "{} <span size=\"small\" font_weight=\"bold\" alpha=\"60%\">{}</span>",
        glib::markup_escape_text(main),
        glib::markup_escape_text(small)
    ));
    l.add_css_class("bigval");
    l
}

fn toplist(title: &str, rows: Vec<(String, f64, String)>, glass: bool) -> Option<gtk::Box> {
    if rows.is_empty() {
        return None;
    }
    let mx = rows.iter().map(|r| r.1).fold(0.0, f64::max);
    let list = bx(gtk::Orientation::Vertical, 0, "");
    for (name, v, txt) in rows {
        let r = bx(gtk::Orientation::Horizontal, 8, "");
        r.set_margin_top(5);
        r.set_margin_bottom(5);
        let n = modal::label(&name, "pn");
        n.set_ellipsize(gtk::pango::EllipsizeMode::End);
        n.set_width_chars(12);
        n.set_max_width_chars(14);
        r.append(&n);
        let w = if mx > 0.0 { (v / mx * 100.0).max(3.0) / 100.0 } else { 0.0 };
        r.append(&draw::bar(w, config::GREEN, 6, glass));
        let t = modal::label(&txt, "pv");
        t.set_xalign(1.0);
        t.set_width_chars(11);
        r.append(&t);
        list.append(&r);
    }
    Some(section(title, &list))
}

/// The history graph of a card, with its caption.
fn history(st: &State, key: &str) -> gtk::Widget {
    let b = bx(gtk::Orientation::Vertical, 6, "gbox");
    let v: Vec<f64> = st.hist.get(key).map(|h| h.iter().copied().collect()).unwrap_or_default();
    if v.len() < 2 {
        let l = gtk::Label::new(Some("Collecting history..."));
        l.add_css_class("gwait");
        l.set_size_request(-1, 80);
        b.append(&l);
        return b.upcast();
    }
    let mx = v.iter().copied().fold(0.0, f64::max);
    let top = ((mx / 10.0).ceil() * 10.0 + 10.0).clamp(20.0, 100.0);
    b.append(&draw::graph(v, top));
    let span = match (st.hist_t.front(), st.hist_t.back()) {
        (Some(a), Some(z)) => z.duration_since(*a).as_secs_f64().round() as i64,
        _ => 0,
    };
    let ago = if span >= 90 { format!("{} min ago", config::js_round(span as f64 / 60.0)) } else { format!("{span} s ago") };
    let cap = bx(gtk::Orientation::Horizontal, 0, "gcap");
    let l1 = modal::label(&ago, "gcap");
    l1.set_hexpand(true);
    let l2 = gtk::Label::new(Some(&format!("scale 0-{top}%")));
    l2.add_css_class("gcap");
    l2.set_hexpand(true);
    let l3 = modal::label("now", "gcap");
    l3.set_xalign(1.0);
    l3.set_hexpand(true);
    cap.append(&l1);
    cap.append(&l2);
    cap.append(&l3);
    b.append(&cap);
    b.upcast()
}

trait HostOrLocal {
    fn server_host_or_localhost(&self) -> String;
}

impl HostOrLocal for Settings {
    fn server_host_or_localhost(&self) -> String {
        let h = self.server_host();
        if h.is_empty() {
            "localhost".into()
        } else {
            h
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(top: &[&str], src: &str, t: Target) -> Vec<String> {
        let mut l = vec![(Cont::Top, top.iter().map(|x| x.to_string()).collect::<Vec<_>>())];
        move_in_lists(src, &Cont::Top, &t, &mut l);
        l.remove(0).1
    }

    #[test]
    fn drag_rules() {
        assert_eq!(run(&["a", "b", "c"], "a", Target::Item(Cont::Top, "c".into())), vec!["b", "c", "a"]);
        assert_eq!(run(&["a", "b", "c"], "c", Target::Item(Cont::Top, "a".into())), vec!["c", "a", "b"]);
        assert_eq!(run(&["a", "b", "c"], "a", Target::End(Cont::Top)), vec!["b", "c", "a"]);
        // between two lists
        let mut l = vec![(Cont::Shown, vec!["a".to_string(), "b".to_string()]), (Cont::Hidden, vec!["x".to_string()])];
        move_in_lists("b", &Cont::Shown, &Target::Item(Cont::Hidden, "x".into()), &mut l);
        assert_eq!(l[0].1, vec!["a"]);
        assert_eq!(l[1].1, vec!["b", "x"]);
    }

    #[test]
    fn stack_dots() {
        let sv = |id: &str| Service { id: id.into(), name: String::new(), port: String::new(), icon: String::new(), logo: String::new(), url: String::new(), scheme: String::new(), addr: String::new(), label: String::new(), on_widget: true, group: None, zima: String::new() };
        let g = Stack { id: "g".into(), name: String::new(), logo: String::new(), icon: String::new(), expand: false, members: vec![sv("a"), sv("b")], on_widget: true };
        let mut m = HashMap::new();
        assert_eq!(group_state(&g, &m), "");
        m.insert("a".to_string(), true);
        assert_eq!(group_state(&g, &m), "");
        m.insert("b".to_string(), false);
        assert_eq!(group_state(&g, &m), "part");
        m.insert("a".to_string(), false);
        assert_eq!(group_state(&g, &m), "dn");
        assert_eq!(pc(Some(12.345)), "12.3%");
        assert_eq!(pc(None), "-");
        assert_eq!(hdd_label("/dev/sdb", 0), "SDB");
        assert_eq!(hdd_label("drivetemp-1", 2), "HDD 3");
    }
}
