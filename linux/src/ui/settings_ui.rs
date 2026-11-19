//! The Settings sheet (gear icon): this server's settings (server, Monitoring) and the ones shared by
//! all servers (widget, buttons, theme & look, data), then Delete server at the very bottom.

use super::app::App;
use super::draw;
use super::modal::{self, ModalOpts};
use super::{bg, widget};
use crate::config::{self, ACCENTS, PLATFORMS, THEMES};
use crate::glances;
use crate::net;
use crate::power;
use crate::settings::{clean_settings, Look, LOOK_KEYS};
use gtk::prelude::*;
use gtk::{gdk, glib};
use regex::Regex;
use serde_json::{json, Value};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

type Redraw = Rc<RefCell<Option<Rc<dyn Fn()>>>>;

pub fn type_opts() -> Vec<(String, String)> {
    PLATFORMS.iter().map(|p| (p.key.to_string(), p.short_name().to_string())).collect()
}

/// What Settings loaded about the server.
#[derive(Default, Clone)]
struct Info {
    host: String,
    first_run: bool,
    mounts: Option<Vec<(String, f64, f64, String)>>,
    ifaces: Option<Vec<String>>,
    logins: Vec<(String, bool)>,
    has_gpass: bool,
    hidden: Vec<String>,
}

struct Form {
    f: Value,
    title: String,
    gpass: String,
    clear_gpass: bool,
    info: Info,
    login_open: Option<String>,
    lu: String,
    lp: String,
    lerr: String,
    previewed: bool,
    saved: bool,
    busy: bool,
    test: Vec<(bool, String)>,
    testing: bool,
}

struct Sheet {
    app: Rc<App>,
    panel: gtk::Box,
    body: gtk::Box,
    err: gtk::Label,
    save: gtk::Button,
    form: RefCell<Form>,
}

/// Opens Settings for the server on screen.
pub fn open(app: &Rc<App>) {
    if app.modals.is_open("settings") {
        return;
    }
    let sheet_cell: Rc<RefCell<Option<Rc<Sheet>>>> = Rc::default();
    let sc2 = sheet_cell.clone();
    let a2 = app.clone();
    // closing: a look that was only previewed goes back to the saved one
    let on_close: Box<dyn Fn()> = Box::new(move || {
        if let Some(s) = sc2.borrow_mut().take() {
            let f = s.form.borrow();
            if f.previewed && !f.saved {
                let look = a2.st.borrow().look.clone();
                a2.apply_look(&look);
            }
        }
    });
    let panel = app.modals.show(ModalOpts { width: 460, top: true, name: "settings" }, Some(on_close));
    panel.set_spacing(18);
    let shd = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    shd.add_css_class("shd");
    let cancel = modal::button("Cancel", "");
    cancel.set_valign(gtk::Align::Center);
    shd.append(&cancel);
    let tb = gtk::Box::new(gtk::Orientation::Vertical, 3);
    tb.set_hexpand(true);
    let t = gtk::Label::new(Some("SETTINGS"));
    t.add_css_class("h2");
    tb.append(&t);
    let sub = gtk::Label::new(Some(&app.st.borrow().title));
    sub.add_css_class("h2sub");
    sub.set_ellipsize(gtk::pango::EllipsizeMode::End);
    tb.append(&sub);
    shd.append(&tb);
    let save = modal::button("Save", "ok");
    save.set_valign(gtk::Align::Center);
    save.set_visible(false);
    shd.append(&save);
    panel.append(&shd);
    let err = gtk::Label::new(None);
    err.add_css_class("err");
    err.set_wrap(true);
    err.set_justify(gtk::Justification::Center);
    panel.append(&err);
    let body = gtk::Box::new(gtk::Orientation::Vertical, 20);
    let wait = gtk::Label::new(Some("Loading..."));
    wait.add_css_class("swait");
    wait.set_margin_top(18);
    wait.set_margin_bottom(18);
    body.append(&wait);
    panel.append(&body);
    let (set, cur, title) = {
        let s = app.st.borrow();
        (s.set.clone(), s.cur.clone(), s.title.clone())
    };
    let sheet = Rc::new(Sheet {
        app: app.clone(),
        panel: panel.clone(),
        body,
        err,
        save: save.clone(),
        form: RefCell::new(Form {
            f: set.to_value(),
            title,
            gpass: String::new(),
            clear_gpass: false,
            info: Info::default(),
            login_open: None,
            lu: String::new(),
            lp: String::new(),
            lerr: String::new(),
            previewed: false,
            saved: false,
            busy: true,
            test: vec![],
            testing: false,
        }),
    });
    *sheet_cell.borrow_mut() = Some(sheet.clone());
    let (a, p) = (app.clone(), panel.clone());
    cancel.connect_clicked(move |_| a.modals.close_of(&p));
    let s2 = sheet.clone();
    save.connect_clicked(move |_| s2.save());
    // the drive and network lists come from Glances (only asked when Monitoring is on)
    let secrets = app.ctx.secrets.clone();
    let pass = secrets.glances_pass(&cur);
    let hidden = app.ctx.store.borrow_mut().load_data().hidden.clone();
    let s3 = sheet.clone();
    let logins: Vec<(String, bool)> = PLATFORMS.iter().map(|p| (p.key.to_string(), secrets.has_login(p.key, &cur))).collect();
    let has_gpass = secrets.has(&crate::secrets::Secrets::gpass_key(&cur));
    bg(
        move || {
            if set.monitoring && !set.server_host().is_empty() {
                let fs = net::glances_request(&set, "fs", &pass).ok();
                let n = net::glances_request(&set, "network", &pass).ok();
                (fs.map(|v| glances::mounts_for_settings(&v)), n.map(|v| glances::ifaces_for_settings(&v)), set)
            } else {
                (None, None, set)
            }
        },
        move |(mounts, ifaces, set)| {
            {
                let mut f = s3.form.borrow_mut();
                f.busy = false;
                f.info = Info { host: set.server_host(), first_run: set.server_host().is_empty(), mounts, ifaces, logins, has_gpass, hidden };
            }
            s3.save.set_visible(true);
            s3.render(true);
        },
    );
}

fn sec(body: &gtk::Box, title: &str) -> gtk::Box {
    let s = gtk::Box::new(gtk::Orientation::Vertical, 10);
    let h = modal::label(title, "sec");
    h.set_margin_start(4);
    s.append(&h);
    let card = gtk::Box::new(gtk::Orientation::Vertical, 14);
    card.add_css_class("scard");
    s.append(&card);
    body.append(&s);
    card
}

fn shint(parent: &gtk::Box, text: &str) -> gtk::Label {
    let l = modal::hint(text, "shint");
    l.set_margin_top(-6);
    parent.append(&l);
    l
}

/// A row with a title (and a smaller line under it); chips go on its right.
fn srow(parent: &gtk::Box, title: &str, sub: &str) -> gtk::Box {
    let r = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    let t = gtk::Box::new(gtk::Orientation::Vertical, 2);
    t.set_hexpand(true);
    let b = modal::label(title, "srow-t");
    b.set_ellipsize(gtk::pango::EllipsizeMode::End);
    t.append(&b);
    if !sub.is_empty() {
        let s = modal::hint(sub, "srow-s");
        t.append(&s);
    }
    r.append(&t);
    parent.append(&r);
    r
}

fn sdiv(body: &gtk::Box, text: &str) {
    let d = gtk::Label::new(Some(text));
    d.add_css_class("sdiv");
    d.set_margin_top(6);
    d.set_margin_bottom(-6);
    body.append(&d);
}

fn fs(f: &Value, k: &str) -> String {
    match &f[k] {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        _ => String::new(),
    }
}

fn is_local(h: &str) -> bool {
    Regex::new(r"^[0-9.]+$").unwrap().is_match(h)
        || h.starts_with('[')
        || !h.contains('.')
        || Regex::new(r"(?i)\.(local|lan|home|internal|localdomain|home\.arpa|ts\.net)$").unwrap().is_match(h)
}

/// The one server address field: http / https list + IP or name with an optional port.
/// on_change(host, scheme ("" = the usual one), port).
pub fn server_addr_field(parent: &gtk::Box, label: &str, platform: &str, host: &str, scheme: &str, port: &str, on_change: Rc<dyn Fn(String, String, String)>) -> gtk::Entry {
    let f = modal::field(parent, label);
    let usual = config::platform(platform).map(|p| p.scheme).unwrap_or("http").to_string();
    let sel = modal::scheme_select(if scheme.is_empty() { &usual } else { scheme });
    let val = format!("{host}{}", if port.is_empty() { String::new() } else { format!(":{port}") });
    let (w, e) = modal::input(&val, "192.168.1.10 or a name", false);
    e.set_input_purpose(gtk::InputPurpose::Url);
    w.prepend(&sel);
    f.append(&w);
    let changed = {
        let (sel, e, usual) = (sel.clone(), e.clone(), usual.clone());
        Rc::new(move || {
            let mut v = e.text().trim().to_string();
            // a full address was typed or pasted: its http / https goes into the list
            if let Some(m) = Regex::new(r"(?i)^(https?)://(.*)$").unwrap().captures(&v.clone()) {
                sel.set_selected(if m[1].to_lowercase() == "https" { 1 } else { 0 });
                v = m[2].to_string();
                e.set_text(&v);
                return;
            }
            v = Regex::new(r"[/?#].*$").unwrap().replace(&v, "").to_string();
            let (h, p) = match Regex::new(r"^([^:]+)(?::([0-9]{1,5}))?$").unwrap().captures(&v) {
                Some(m) => (m[1].to_string(), m.get(2).map(|x| x.as_str().to_string()).unwrap_or_default()),
                None => (v.clone(), String::new()),
            };
            let sc = modal::scheme_of(&sel);
            on_change(h, if sc == usual { String::new() } else { sc.to_string() }, p);
        })
    };
    let c = changed.clone();
    e.connect_changed(move |_| c());
    sel.connect_selected_notify(move |_| changed());
    e
}

impl Sheet {
    fn f(&self) -> Value {
        self.form.borrow().f.clone()
    }

    fn set_f(&self, k: &str, v: Value) {
        self.form.borrow_mut().f[k] = v;
    }

    fn msg(&self, ok: bool, text: &str) -> bool {
        if !ok {
            self.err.set_text(if text.is_empty() { "Something went wrong" } else { text });
            return false;
        }
        self.err.set_text("");
        if !text.is_empty() {
            self.app.toast.show(text, 3000);
        }
        true
    }

    /// Theme / look changes are previewed on the dashboard straight away.
    fn changed(&self, key: &str) {
        if !LOOK_KEYS.contains(&key) {
            return;
        }
        self.form.borrow_mut().previewed = true;
        let look = Look::from_value(&self.f());
        self.app.apply_look(&look);
    }

    fn scroller(&self) -> Option<gtk::ScrolledWindow> {
        let mut w = self.panel.parent();
        while let Some(x) = w {
            if let Ok(s) = x.clone().downcast::<gtk::ScrolledWindow>() {
                return Some(s);
            }
            w = x.parent();
        }
        None
    }

    fn text(self: &Rc<Self>, parent: &gtk::Box, label: &str, key: &str, ph: &str, hint: &str) -> gtk::Entry {
        let f = modal::field(parent, label);
        let (w, e) = modal::input(&fs(&self.f(), key), ph, false);
        f.append(&w);
        let (s, k) = (self.clone(), key.to_string());
        e.connect_changed(move |e| s.set_f(&k, json!(e.text().trim())));
        if !hint.is_empty() {
            shint(parent, hint);
        }
        e
    }

    fn select(self: &Rc<Self>, parent: &gtk::Box, label: &str, key: &str, opts: Vec<(String, String)>, hint: &str, as_number: bool) {
        let f = modal::field(parent, label);
        let (w, dd) = modal::select(&opts, &fs(&self.f(), key));
        f.append(&w);
        let (s, k) = (self.clone(), key.to_string());
        dd.connect_selected_notify(move |d| {
            if let Some(o) = opts.get(d.selected() as usize) {
                let v = if as_number { json!(o.0.parse::<i64>().unwrap_or(0)) } else { json!(o.0) };
                s.set_f(&k, v);
                s.changed(&k);
            }
        });
        if !hint.is_empty() {
            shint(parent, hint);
        }
    }

    fn toggle(self: &Rc<Self>, parent: &gtk::Box, label: &str, key: &str, hint: &str) {
        let on = self.f()[key].as_bool().unwrap_or(false);
        let (row, sw) = modal::toggle_row(label, on);
        parent.append(&row);
        let (s, k) = (self.clone(), key.to_string());
        sw.connect_active_notify(move |w| {
            s.set_f(&k, json!(w.is_active()));
            s.changed(&k);
        });
        if !hint.is_empty() {
            shint(parent, hint);
        }
    }

    /// Glances address: an http:// / https:// list plus the IP or name (optionally :port and a path).
    /// The usual port 61208 is added for IP addresses and local names; empty = the server on 61208.
    fn glances_url_field(self: &Rc<Self>, parent: &gtk::Box, host: &str, hint: &str) {
        let f = modal::field(parent, "GLANCES ADDRESS");
        let cur = fs(&self.f(), "baseUrl");
        let m = Regex::new(r"(?i)^(https?)://(.*)$").unwrap().captures(&cur).map(|c| (c[1].to_lowercase(), c[2].to_string()));
        let scheme = m.as_ref().map(|x| x.0.clone()).unwrap_or_else(|| "http".into());
        let mut rest = m.map(|x| x.1).unwrap_or_default();
        rest = Regex::new(r"^([^/:]+):61208(/|$)").unwrap().replace(&rest, "$1$2").to_string();
        let ph = if host.is_empty() { "IP or name".to_string() } else { format!("{host} (the server)") };
        let (w, e) = modal::input(&rest, &ph, false);
        let sel = modal::scheme_select(&scheme);
        w.prepend(&sel);
        f.append(&w);
        let compose = {
            let (s, e, sel, host) = (self.clone(), e.clone(), sel.clone(), host.to_string());
            Rc::new(move || {
                let mut v = e.text().trim().to_string();
                if let Some(mm) = Regex::new(r"(?i)^(https?)://(.*)$").unwrap().captures(&v.clone()) {
                    sel.set_selected(if mm[1].to_lowercase() == "https" { 1 } else { 0 });
                    v = mm[2].trim_end_matches('/').to_string();
                    e.set_text(&v);
                    return;
                }
                v = v.trim_end_matches('/').to_string();
                let sc = modal::scheme_of(&sel);
                let mut h = v.clone();
                if h.is_empty() {
                    // empty: the usual address, unless the other http / https was picked
                    if sc == "http" || host.is_empty() {
                        s.set_f("baseUrl", json!(""));
                        return;
                    }
                    h = host.clone();
                }
                let out = match Regex::new(r"^([^/:]+)(:[0-9]+)?(.*)$").unwrap().captures(&h) {
                    Some(hp) => {
                        let port = if hp.get(2).is_none() && is_local(&hp[1]) { ":61208" } else { "" };
                        format!("{sc}://{}{}{}{}", &hp[1], hp.get(2).map(|x| x.as_str()).unwrap_or(""), port, &hp[3])
                    }
                    None => format!("{sc}://{h}"),
                };
                s.set_f("baseUrl", json!(out));
            })
        };
        let c = compose.clone();
        e.connect_changed(move |_| c());
        sel.connect_selected_notify(move |_| compose());
        shint(parent, hint);
    }

    fn swatches(self: &Rc<Self>, parent: &gtk::Box, label: &str, key: &'static str, after: Redraw) -> Rc<dyn Fn()> {
        let f = modal::field(parent, label);
        let row = gtk::FlowBox::new();
        row.set_selection_mode(gtk::SelectionMode::None);
        row.set_column_spacing(10);
        row.set_row_spacing(10);
        row.set_max_children_per_line(8);
        f.append(&row);
        let nm = modal::label("", "swn");
        f.append(&nm);
        let redraw: Redraw = Rc::default();
        let (s, row2, nm2, r2) = (self.clone(), row.clone(), nm.clone(), redraw.clone());
        let draw_fn: Rc<dyn Fn()> = Rc::new(move || {
            while let Some(c) = row2.first_child() {
                row2.remove(&c);
            }
            let cur = fs(&s.f(), key);
            let theme = config::theme(&fs(&s.f(), "theme"));
            let items: Vec<(String, String, gtk::DrawingArea, bool)> = if key == "theme" {
                THEMES
                    .iter()
                    .map(|t| {
                        let c = |x: (u8, u8, u8)| (x.0 as f64 / 255.0, x.1 as f64 / 255.0, x.2 as f64 / 255.0);
                        let a = if t.no_glow { draw::swatch((0.0, 0.0, 0.0), None, None) } else { draw::swatch(config::hex_rgb(t.bg), Some(c(t.g1)), Some(c(t.g2))) };
                        (t.key.to_string(), t.name.to_string(), a, false)
                    })
                    .collect()
            } else {
                ACCENTS
                    .iter()
                    .map(|a| {
                        let col = config::hex_rgb(a.c.unwrap_or(theme.acc));
                        (a.key.to_string(), if a.c.is_some() { a.name.to_string() } else { "Same as theme".into() }, draw::swatch(col, None, None), a.c.is_none())
                    })
                    .collect()
            };
            for (v, name, area, dashed) in items {
                let b = gtk::Button::new();
                b.add_css_class("swt");
                if dashed {
                    b.add_css_class("dashed");
                }
                if v == cur {
                    b.add_css_class("on");
                    nm2.set_text(&name);
                }
                b.set_child(Some(&area));
                b.set_tooltip_text(Some(&name));
                b.update_property(&[gtk::accessible::Property::Label(&name)]);
                let (s2, r3, af) = (s.clone(), r2.clone(), after.clone());
                b.connect_clicked(move |_| {
                    s2.set_f(key, json!(v));
                    s2.changed(key);
                    if let Some(f) = r3.borrow().as_ref() {
                        f();
                    }
                    if let Some(f) = af.borrow().as_ref() {
                        f();
                    }
                });
                row2.append(&b);
            }
        });
        *redraw.borrow_mut() = Some(draw_fn.clone());
        draw_fn();
        draw_fn
    }

    /// Builds the whole sheet from the form (keeps the scroll position).
    fn render(self: &Rc<Self>, first: bool) {
        let scroll = self.scroller().map(|s| s.vadjustment().value());
        let b = &self.body;
        while let Some(c) = b.first_child() {
            b.remove(&c);
        }
        let f = self.f();
        let info = self.form.borrow().info.clone();
        let app = &self.app;
        let place = app.ctx.secrets.place();

        if info.first_run {
            let wel = modal::hint("Welcome to -board. Enter your server's IP or name below, pick its server type, click Test connection, then Save. After that, add your services with the + button.", "cnote");
            wel.set_justify(gtk::Justification::Center);
            wel.set_xalign(0.5);
            b.append(&wel);
        }
        sdiv(b, "THIS SERVER");

        // ---------- SERVER ----------
        let c = sec(b, "SERVER");
        let p = fs(&f, "platform");
        let host_ph = {
            let h = fs(&f, "serverHost");
            if !h.is_empty() {
                h
            } else if !info.host.is_empty() {
                info.host.clone()
            } else {
                "192.168.1.10".into()
            }
        };
        let nf = modal::field(&c, "SERVER NAME");
        let (w, ne) = modal::input(&self.form.borrow().title, "e.g. NAS", false);
        ne.set_max_length(20);
        nf.append(&w);
        let s = self.clone();
        ne.connect_changed(move |e| s.form.borrow_mut().title = e.text().to_string());
        let tf = modal::field(&c, "SERVER TYPE");
        let s = self.clone();
        tf.append(&modal::radio_row(&type_opts(), &p, true, Rc::new(move |v| {
            s.set_f("platform", json!(v));
            s.render(false);
        })));
        shint(&c, config::type_hint(&p));
        let s = self.clone();
        server_addr_field(&c, "SERVER ADDRESS", &p, &fs(&f, "serverHost"), &fs(&f, "serverScheme"), &fs(&f, "serverPort"), Rc::new(move |h, sc, pt| {
            let mut fm = s.form.borrow_mut();
            fm.f["serverHost"] = json!(h);
            fm.f["serverScheme"] = json!(sc);
            fm.f["serverPort"] = json!(pt);
        }));
        shint(&c, &config::addr_hint(&p));
        // an older separate address for the server system on another host (still used until it's cleared)
        if let Some(pl) = config::platform(&p) {
            let old = fs(&f, pl.url_key);
            if !old.is_empty() {
                let r = srow(&c, &format!("Reboot / Shut down use {old}"), "A separate address from an older version of -board");
                let ch = modal::chip("Use the server address");
                ch.set_valign(gtk::Align::Center);
                let (s, k) = (self.clone(), pl.url_key);
                ch.connect_clicked(move |_| {
                    s.set_f(k, json!(""));
                    s.render(false);
                });
                r.append(&ch);
            }
        }
        let logged = |k: &str| info.logins.iter().any(|(p, on)| p == k && *on);
        if p == "proxmox" {
            self.text(&c, "NODE NAME", "proxmoxNode", "Automatic (e.g. pve)", "Only needed in a cluster: the node to reboot, as shown in the Proxmox web UI.");
        } else if p == "linux" {
            let hb = modal::chip(if logged("linux") { "Copy setup command" } else { "Create key & copy setup command" });
            c.append(&hb);
            let s = self.clone();
            hb.connect_clicked(move |_| s.helper());
            shint(&c, &format!("Then SSH into the server (ssh you@{host_ph}), paste and press Enter. It asks for your sudo password once, installs the helper as a system service and opens its port in ufw / firewalld if they're on. Run it again after changing the address or port."));
        }
        if let Some(ln) = config::platform(&p).and_then(|pl| pl.login) {
            let open_login = {
                let (s, p2) = (self.clone(), p.clone());
                move || {
                    {
                        let mut fm = s.form.borrow_mut();
                        fm.login_open = Some(p2.clone());
                        fm.lu.clear();
                        fm.lp.clear();
                        fm.lerr.clear();
                    }
                    s.render(false);
                }
            };
            if logged(&p) {
                let r = srow(&c, &format!("{ln} login saved"), place);
                let ch = modal::chip("Change");
                ch.set_valign(gtk::Align::Center);
                let ol = open_login.clone();
                ch.connect_clicked(move |_| ol());
                r.append(&ch);
                let fg = modal::chip("Forget");
                fg.set_valign(gtk::Align::Center);
                let (s, p2) = (self.clone(), p.clone());
                fg.connect_clicked(move |_| s.forget(&p2));
                r.append(&fg);
            } else {
                let r = srow(&c, &format!("No {ln} login saved"), "Enter it now, or you'll be asked the first time you use Reboot / Shut down");
                let ch = modal::chip("Enter login");
                ch.set_valign(gtk::Align::Center);
                ch.connect_clicked(move |_| open_login());
                r.append(&ch);
            }
            // the login form (checked against the server before it's saved)
            if self.form.borrow().login_open.as_deref() == Some(p.as_str()) {
                let lf = gtk::Box::new(gtk::Orientation::Vertical, 12);
                lf.add_css_class("slogin");
                c.append(&lf);
                let up = config::platform(&p).unwrap().user_ph;
                let uf = modal::field(&lf, &format!("{} USERNAME", ln.to_uppercase()));
                let (w, ue) = modal::input(&self.form.borrow().lu, up, false);
                uf.append(&w);
                let s = self.clone();
                ue.connect_changed(move |e| s.form.borrow_mut().lu = e.text().to_string());
                let pf = modal::field(&lf, &format!("{} PASSWORD", ln.to_uppercase()));
                let (w, pe) = modal::input(&self.form.borrow().lp, "", true);
                pf.append(&w);
                let s = self.clone();
                pe.connect_changed(move |e| s.form.borrow_mut().lp = e.text().to_string());
                let lerr = self.form.borrow().lerr.clone();
                if !lerr.is_empty() {
                    lf.append(&result_line(false, &lerr));
                }
                let lb = gtk::Box::new(gtk::Orientation::Horizontal, 10);
                lf.append(&lb);
                let cc = modal::chip("Cancel");
                let s = self.clone();
                cc.connect_clicked(move |_| {
                    {
                        let mut fm = s.form.borrow_mut();
                        fm.login_open = None;
                        fm.lp.clear();
                    }
                    s.render(false);
                });
                lb.append(&cc);
                let cs = modal::chip("Check & save login");
                let (s, p2) = (self.clone(), p.clone());
                let check = Rc::new(move || s.check_login(&p2));
                let ck = check.clone();
                cs.connect_clicked(move |_| ck());
                lb.append(&cs);
                let ck = check.clone();
                modal::on_enter(&pe, move || ck());
                let hint = format!("Checked against the server at the address above (even before you save Settings), then saved ({}).{}", place.to_lowercase(), if p == "proxmox" { " Typing root means root@pam." } else { "" });
                lf.append(&modal::hint(&hint, "shint"));
                if self.form.borrow().lu.is_empty() {
                    let e = ue.clone();
                    glib::timeout_add_local_once(Duration::from_millis(60), move || {
                        e.grab_focus();
                    });
                }
            }
        } else if p == "linux" && logged("linux") {
            let r = srow(&c, "Helper key saved", place);
            let fg = modal::chip("Forget");
            fg.set_valign(gtk::Align::Center);
            let s = self.clone();
            fg.connect_clicked(move |_| s.forget("linux"));
            r.append(&fg);
        }
        self.toggle(&c, "SHOW REBOOT / SHUT DOWN", "showPower", "Off hides the arrow and menu next to the server name.");
        let tb = modal::chip("Test connection");
        c.append(&tb);
        let res = gtk::Box::new(gtk::Orientation::Vertical, 6);
        res.add_css_class("sres");
        {
            let fm = self.form.borrow();
            if fm.testing {
                res.append(&modal::label("Testing...", ""));
            }
            for (ok, t) in &fm.test {
                res.append(&result_line(*ok, t));
            }
        }
        c.append(&res);
        let s = self.clone();
        tb.connect_clicked(move |_| s.test());
        shint(&c, "Tests the server, Glances (below) and Reboot / Shut down with the values in this form, even before you save them.");

        // ---------- MONITORING ----------
        let c = sec(b, "MONITORING");
        let mon = f["monitoring"].as_bool().unwrap_or(true);
        let (row, sw) = modal::toggle_row("MONITORING (GLANCES)", mon);
        c.append(&row);
        let s = self.clone();
        sw.connect_active_notify(move |w| {
            s.set_f("monitoring", json!(w.is_active()));
            let s2 = s.clone();
            glib::idle_add_local_once(move || s2.render(false));
        });
        shint(&c, if mon {
            "CPU, RAM, storage and network from Glances, in a fold-away panel on the dashboard and as gauges on the widget."
        } else {
            "Off: no Monitoring panel, no gauges on the widget (it shows your service buttons instead), and -board sends no requests to Glances. The LIVE / OFFLINE status still works."
        });
        if mon {
            let gh = {
                let h = fs(&f, "serverHost");
                if h.is_empty() {
                    info.host.clone()
                } else {
                    h
                }
            };
            let hint = format!(
                "Leave empty for {}. Otherwise pick http or https and type the IP or name; port 61208 is added for IP addresses and local names unless you type another. A public domain (behind a reverse proxy) gets no port added.",
                if gh.is_empty() { "the server address on port 61208".to_string() } else { format!("http://{gh}:61208") }
            );
            self.glances_url_field(&c, &gh, &hint);
            self.select(&c, "GLANCES API VERSION", "apiVersion", vec![("4".into(), "4 (Glances 4.x)".into()), ("3".into(), "3 (Glances 3.x)".into())], "", true);
            self.text(&c, "GLANCES USERNAME", "username", "Only if Glances has a password", "");
            let pf = modal::field(&c, "GLANCES PASSWORD");
            let (w, pe) = modal::input(&self.form.borrow().gpass, if info.has_gpass { "Saved. Leave empty to keep it" } else { "Optional" }, true);
            pf.append(&w);
            let s = self.clone();
            pe.connect_changed(move |e| s.form.borrow_mut().gpass = e.text().to_string());
            if info.has_gpass {
                let cg = modal::chip(if self.form.borrow().clear_gpass { "Will be removed when you save (click to undo)" } else { "Remove saved Glances password" });
                c.append(&cg);
                let s = self.clone();
                cg.connect_clicked(move |b| {
                    let now = !s.form.borrow().clear_gpass;
                    s.form.borrow_mut().clear_gpass = now;
                    b.set_label(if now { "Will be removed when you save (click to undo)" } else { "Remove saved Glances password" });
                });
            }
            let dl = modal::field(&c, "DRIVES IN THE STORAGE CARD");
            let picked: Vec<String> = f["diskMounts"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default();
            match info.mounts.as_ref().filter(|m| !m.is_empty()) {
                Some(mounts) => {
                    #[allow(clippy::type_complexity)]
                    let mut shown: Vec<(String, Option<(f64, f64, String)>)> = mounts.iter().map(|m| (m.0.clone(), Some((m.1, m.2, m.3.clone())))).collect();
                    for m in &picked {
                        if !shown.iter().any(|x| &x.0 == m) {
                            shown.push((m.clone(), None));
                        }
                    }
                    for (mnt, det) in shown {
                        let sub = match det {
                            None => "Not found in Glances right now".to_string(),
                            Some((size, used, dev)) => format!(
                                "{}{}{}",
                                config::format_bytes(size),
                                if dev.is_empty() { String::new() } else { format!(" · {dev}") },
                                if size > 0.0 { format!(" · {}% used", config::js_round(used / size * 100.0)) } else { String::new() }
                            ),
                        };
                        let r = srow(&dl, &mnt, &sub);
                        let sw = gtk::Switch::new();
                        sw.set_valign(gtk::Align::Center);
                        sw.set_active(picked.contains(&mnt));
                        let s = self.clone();
                        sw.connect_active_notify(move |w| {
                            let mut fm = s.form.borrow_mut();
                            let mut list: Vec<Value> = fm.f["diskMounts"].as_array().cloned().unwrap_or_default();
                            list.retain(|x| x.as_str() != Some(&mnt));
                            if w.is_active() {
                                list.push(json!(mnt));
                            }
                            fm.f["diskMounts"] = Value::Array(list);
                        });
                        r.append(&sw);
                    }
                    shint(&c, "None picked = pick automatically. Paths are as the Glances container sees them.");
                }
                None => {
                    let (w, e) = modal::input(&picked.join(", "), "/, /mnt/data", false);
                    dl.append(&w);
                    let s = self.clone();
                    e.connect_changed(move |e| {
                        let v: Vec<Value> = e.text().split(',').map(|x| x.trim()).filter(|x| !x.is_empty()).map(|x| json!(x)).collect();
                        s.set_f("diskMounts", Value::Array(v));
                    });
                    shint(&c, "Couldn't reach Glances to list the drives, so type the paths, separated by commas. Empty = pick automatically.");
                }
            }
            self.text(&c, "MAX DRIVES WHEN PICKED AUTOMATICALLY", "maxDisks", "3", "");
            let mut ifo: Vec<(String, String)> = vec![("".into(), "Automatic (the busiest)".into())];
            for n in info.ifaces.clone().unwrap_or_default() {
                ifo.push((n.clone(), n));
            }
            let ni = fs(&f, "networkInterface");
            if !ni.is_empty() && !ifo.iter().any(|o| o.0 == ni) {
                ifo.push((ni.clone(), ni));
            }
            self.select(&c, "NETWORK INTERFACE", "networkInterface", ifo, if info.ifaces.is_some() { "" } else { "Couldn't reach Glances to list the interfaces." }, false);
            self.toggle(&c, "OPEN MONITORING ON START", "monitoringOpen", "");
            self.toggle(&c, "KEEP MONITORING ALWAYS OPEN", "monitoringFixed", "Removes the Monitoring button, so the cards and network speed always show.");
            self.text(&c, "DASHBOARD UPDATES EVERY (SECONDS)", "liveSeconds", "3", "Lower = smoother graphs, more work while the dashboard is open.");
            let tl = modal::field(&c, "TEMPERATURE COLOURS (°C)");
            let g = gtk::Grid::new();
            g.set_column_spacing(8);
            g.set_row_spacing(8);
            for (i, t) in ["", "ORANGE FROM", "RED FROM"].iter().enumerate() {
                let l = gtk::Label::new(Some(t));
                l.add_css_class("trow-h");
                if i == 0 {
                    l.set_size_request(60, -1);
                } else {
                    l.set_hexpand(true);
                }
                g.attach(&l, i as i32, 0, 1, 1);
            }
            for (r, (k, name)) in [("cpu", "CPU"), ("nvme", "NVMe"), ("hdd", "HDD")].iter().enumerate() {
                let l = modal::label(name, "trow-n");
                g.attach(&l, 0, r as i32 + 1, 1, 1);
                for (cidx, w) in ["warm", "hot"].iter().enumerate() {
                    let v = &f["tempLimits"][*k][*w];
                    let (frame, e) = modal::input(&match v {
                        Value::Number(n) => n.to_string(),
                        Value::String(s) => s.clone(),
                        _ => String::new(),
                    }, "", false);
                    e.set_width_chars(4);
                    frame.set_hexpand(true);
                    let (s, k2, w2) = (self.clone(), k.to_string(), w.to_string());
                    e.connect_changed(move |e| {
                        s.form.borrow_mut().f["tempLimits"][&k2][&w2] = json!(e.text().trim());
                    });
                    g.attach(&frame, cidx as i32 + 1, r as i32 + 1, 1, 1);
                }
            }
            tl.append(&g);
        }

        // ---------- WIDGET ----------
        sdiv(b, "SHARED BY ALL SERVERS");
        let c = sec(b, "WIDGET");
        shint(&c, "The desktop widget shows one server: the one on screen when you open it, or run  board --widget large --server 2  (a name or number; 1 = the first). Right-click a widget for its size and refresh.");
        let wr = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        for (lab, size) in [("Open small widget", "small"), ("Medium", "medium"), ("Large", "large")] {
            let ch = modal::chip(lab);
            let a = app.clone();
            ch.connect_clicked(move |_| {
                let cur = a.st.borrow().cur.clone();
                widget::open(&a.ctx, a.win.application().as_ref(), size, Some(cur));
            });
            wr.append(&ch);
        }
        c.append(&wr);
        self.text(&c, "REFRESH EVERY (MINUTES)", "refreshMinutes", "5", "How often the desktop widget checks the server again.");
        self.text(&c, "CORNER BUTTON LINK", "linkUrl", "https://... (empty = no button)", "A small button in the widget corner, e.g. your ZimaOS or Proxmox page.");
        self.toggle(&c, "STATUS DOTS ON THE WIDGET", "widgetStatusDots", "Each widget refresh checks every button, which takes a couple of seconds.");
        self.text(&c, "WIDGET TITLE", "widgetTitle", "Same as the dashboard name", "");
        let rows: Vec<(String, String)> = (1..=6).map(|i| (i.to_string(), format!("{i} row{} ({} buttons)", if i == 1 { "" } else { "s" }, i * 4))).collect();
        self.select(&c, "BUTTON ROWS", "widgetRows", rows, "", true);

        // ---------- BUTTONS ----------
        let c = sec(b, "BUTTONS");
        self.toggle(&c, "SHOW LOGOS", "useLogos", "Off shows the emoji instead.");
        self.select(&c, "DASHBOARD COLUMNS", "columns", vec![("2".into(), "2 columns".into()), ("3".into(), "3 columns".into())], "", true);
        self.text(&c, "CHECK UP/DOWN EVERY (SECONDS)", "statusSeconds", "30", "");
        self.text(&c, "COUNT AS DOWN AFTER (SECONDS)", "pingTimeout", "3", "How long a service may take to answer. The widget waits at most 3 seconds.");
        if !crate::config::start().builtin.is_empty() || !info.hidden.is_empty() {
            let rl = modal::field(&c, "DELETED BUTTONS");
            if info.hidden.is_empty() {
                srow(&rl, "None", "Built-in buttons you delete show up here");
            } else {
                for id in &info.hidden {
                    let r = srow(&rl, id.trim_start_matches("b:"), "");
                    let ch = modal::chip("Restore");
                    let (s, id2) = (self.clone(), id.clone());
                    ch.connect_clicked(move |_| s.restore(Some(&id2)));
                    r.append(&ch);
                }
                if info.hidden.len() > 1 {
                    let ra = modal::chip("Restore all");
                    let s = self.clone();
                    ra.connect_clicked(move |_| s.restore(None));
                    rl.append(&ra);
                }
            }
        }

        // ---------- THEME & LOOK ----------
        let c = sec(b, "THEME & LOOK");
        let st = fs(&f, "style");
        let sf = modal::field(&c, "STYLE");
        let s = self.clone();
        sf.append(&modal::radio_row(&[("classic".into(), "Classic".into()), ("glass".into(), "Liquid Glass".into())], &st, false, Rc::new(move |v| {
            s.set_f("style", json!(v));
            s.changed("style");
            s.render(false);
        })));
        shint(&c, if st == "glass" {
            "Liquid Glass: glass cards with a bright rim, a third background glow, springy animation on every press, menu and sheet, and glass rings on the widget."
        } else {
            "Classic: the plain -board look. Switch to Liquid Glass for glass cards and animations."
        });
        self.toggle(&c, "STARTUP ANIMATION", "splash", "A short animated -board logo when the dashboard opens (click it to skip). Works with either style.");
        // the accent swatch "Same as theme" follows the theme
        let acc_redraw: Redraw = Rc::default();
        self.swatches(&c, "THEME", "theme", acc_redraw.clone());
        let acc = self.swatches(&c, "ACCENT COLOUR", "accent", Rc::default());
        *acc_redraw.borrow_mut() = Some(acc);
        shint(&c, "Used for switches, Save buttons and the selected tab. Status colours (green / orange / red) stay the same.");
        let o = |v: &[(&str, &str)]| v.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect::<Vec<_>>();
        self.select(&c, "BACKGROUND GLOW", "glow", o(&[("off", "Off"), ("soft", "Soft"), ("normal", "Normal"), ("strong", "Strong")]), "", false);
        self.toggle(&c, "MOVING BACKGROUND", "movingBg", "Off keeps the glow still (saves a little power).");
        self.select(&c, "CARD STYLE", "cardStyle", o(&[("subtle", "Subtle glass"), ("normal", "Normal glass"), ("strong", "Strong glass")]), "", false);
        self.select(&c, "CORNERS", "corners", o(&[("round", "Rounded"), ("sharp", "Sharper")]), "", false);
        self.select(&c, "FONT", "font", o(&[("system", "System"), ("rounded", "Rounded"), ("serif", "Serif")]), "", false);
        self.select(&c, "BUTTON SIZE", "buttonSize", o(&[("normal", "Normal"), ("compact", "Compact")]), "", false);
        shint(&c, "Changes show on the dashboard behind this sheet as you pick them. Cancel puts the old look back. The widget background follows the theme and glow.");

        // ---------- DATA ----------
        let c = sec(b, "DATA");
        let bx = gtk::Box::new(gtk::Orientation::Vertical, 8);
        bx.add_css_class("sbtns");
        c.append(&bx);
        let b1 = modal::button("Clear logo cache", "");
        let s = self.clone();
        b1.connect_clicked(move |_| s.clear_logos());
        bx.append(&b1);
        let b2 = modal::button("Export settings", "");
        let s = self.clone();
        b2.connect_clicked(move |_| s.export());
        bx.append(&b2);
        let b3 = modal::button("", "");
        let s = self.clone();
        modal::armed(&b3, "Import settings", "Click again to replace everything", Rc::new(move || s.import()));
        bx.append(&b3);
        let b4 = modal::button("", "rtxt");
        let s = self.clone();
        modal::armed(&b4, "Reset layout", "Click again to reset the layout", Rc::new(move || s.reset()));
        bx.append(&b4);
        shint(&c, "Export copies all settings, buttons, stacks and layout to the clipboard (passwords stay on this computer). Import reads them back from the clipboard, also from the iOS version. Reset layout clears the button order and stacks but keeps your own buttons.");

        // ---------- DELETE SERVER (the very bottom, asks to confirm) ----------
        let dname = app.st.borrow().title.clone();
        let only = app.st.borrow().servers.len() < 2;
        let c = sec(b, "DELETE SERVER");
        let dbx = gtk::Box::new(gtk::Orientation::Vertical, 8);
        dbx.add_css_class("sbtns");
        c.append(&dbx);
        let dbt = modal::button(&format!("Delete {dname}"), "del");
        dbt.set_sensitive(!only);
        let (a, dn) = (app.clone(), dname.clone());
        dbt.connect_clicked(move |_| super::dialogs::open_del_server(&a, &dn));
        dbx.append(&dbt);
        shint(&c, if only {
            "This is your only server, so it can't be deleted. Add another server first (swipe the server name left, or the + next to the dots)."
        } else {
            "Deletes this server's settings, logins, buttons, stacks and layout from this computer. Your other servers stay. Asks to confirm."
        });

        if let (Some(v), false) = (scroll, first) {
            let s = self.clone();
            glib::idle_add_local_once(move || {
                if let Some(sc) = s.scroller() {
                    sc.vadjustment().set_value(v);
                }
            });
        }
        if self.form.borrow().busy {
            self.body.add_css_class("sbusy");
        } else {
            self.body.remove_css_class("sbusy");
        }
    }

    fn busy(&self, on: bool) {
        self.form.borrow_mut().busy = on;
        self.body.set_sensitive(!on);
        if on {
            self.body.add_css_class("sbusy");
        } else {
            self.body.remove_css_class("sbusy");
        }
    }

    fn cur(&self) -> String {
        self.app.st.borrow().cur.clone()
    }

    fn test(self: &Rc<Self>) {
        if self.form.borrow().busy {
            return;
        }
        {
            let mut fm = self.form.borrow_mut();
            fm.testing = true;
            fm.test.clear();
        }
        self.render(false);
        self.busy(true);
        let s = clean_settings(&self.f());
        let gpass = {
            let g = self.form.borrow().gpass.clone();
            if g.is_empty() {
                self.app.ctx.secrets.glances_pass(&self.cur())
            } else {
                g
            }
        };
        let services = self.app.st.borrow().layout.all.clone();
        let (cur, secrets) = (self.cur(), self.app.ctx.secrets.clone());
        let me = self.clone();
        bg(
            move || {
                let mut lines = vec![];
                if s.server_host().is_empty() {
                    return vec![(false, "Enter your server's IP or name first, e.g. 192.168.1.10".to_string())];
                }
                let reach = net::server_reachable(&s, &services, 4.0);
                lines.push(if !reach.is_empty() {
                    (true, format!("Server: reachable ({reach} answered)"))
                } else {
                    (false, format!("Server: nothing answered at {} (check the address, and your VPN if you're away from home)", s.server_host()))
                });
                if s.monitoring {
                    let t0 = std::time::Instant::now();
                    match net::glances_request(&s, "quicklook", &gpass) {
                        Ok(q) => {
                            let cpu = q.get("cpu_name").and_then(|v| v.as_str()).filter(|x| !x.is_empty()).map(|x| format!(", {x}")).unwrap_or_default();
                            lines.push((true, format!("Glances: OK ({} ms{cpu})", t0.elapsed().as_millis())));
                        }
                        Err(_) => {
                            // maybe it's the other Glances API version (distro packages are often Glances 3.x)
                            let mut other = s.clone();
                            other.api_version = if s.api_version == 4 { 3 } else { 4 };
                            if net::glances_request(&other, "quicklook", &gpass).is_ok() {
                                lines.push((false, format!("Glances: answers with API version {}, not {}. Pick {} under GLANCES API VERSION.", other.api_version, s.api_version, other.api_version)));
                            } else {
                                lines.push((false, format!("Glances: no answer at {} (check the address and that Glances runs with -w, or turn Monitoring off)", s.glances_url())));
                            }
                        }
                    }
                }
                lines.push(power::test_power(&s, &cur, &secrets));
                lines
            },
            move |lines| {
                {
                    let mut fm = me.form.borrow_mut();
                    fm.testing = false;
                    fm.test = lines;
                }
                me.busy(false);
                me.render(false);
            },
        );
    }

    fn check_login(self: &Rc<Self>, p: &str) {
        if self.form.borrow().busy {
            return;
        }
        let (u, pw) = {
            let fm = self.form.borrow();
            (fm.lu.clone(), fm.lp.clone())
        };
        if u.trim().is_empty() || pw.is_empty() {
            self.form.borrow_mut().lerr = "Enter a username and a password.".into();
            self.render(false);
            return;
        }
        self.form.borrow_mut().lerr.clear();
        self.busy(true);
        let s = clean_settings(&self.f());
        let (cur, secrets, p2) = (self.cur(), self.app.ctx.secrets.clone(), p.to_string());
        let p = p.to_string();
        let me = self.clone();
        bg(move || power::check_and_save_login(&p2, &u, &pw, &s, &cur, &secrets), move |r| {
            me.busy(false);
            match r {
                Err(e) => {
                    me.form.borrow_mut().lerr = e;
                }
                Ok(msg) => {
                    {
                        let mut fm = me.form.borrow_mut();
                        for l in fm.info.logins.iter_mut() {
                            if l.0 == p {
                                l.1 = true;
                            }
                        }
                        fm.login_open = None;
                        fm.lp.clear();
                    }
                    if me.app.st.borrow().set.platform == p {
                        me.app.st.borrow_mut().has_login = true;
                    }
                    if p == "zimaos" {
                        crate::zima::forget(&me.cur());
                        me.app.st.borrow_mut().last_z_apps = None;
                    }
                    me.app.toast.show(&msg, 3500);
                }
            }
            me.render(false);
        });
    }

    fn forget(self: &Rc<Self>, p: &str) {
        let cur = self.cur();
        self.app.ctx.secrets.forget_login(p, &cur);
        {
            let mut fm = self.form.borrow_mut();
            for l in fm.info.logins.iter_mut() {
                if l.0 == p {
                    l.1 = false;
                }
            }
        }
        if self.app.st.borrow().set.platform == p {
            self.app.st.borrow_mut().has_login = false;
        }
        if p == "zimaos" {
            crate::zima::forget(&cur);
            let mut s = self.app.st.borrow_mut();
            s.z_apps = None;
            s.z_upg.clear();
        }
        let msg = if p == "linux" { "Helper key forgotten".to_string() } else { format!("{} login forgotten", config::platform(p).and_then(|x| x.login).unwrap_or("")) };
        self.msg(true, &msg);
        self.render(false);
    }

    /// Linux power helper: keep the saved key (or make one) and copy the setup command.
    fn helper(self: &Rc<Self>) {
        let cur = self.cur();
        let secrets = &self.app.ctx.secrets;
        let key_name = crate::secrets::Secrets::helper_key(&cur);
        if !secrets.has(&key_name) {
            let k = power::new_helper_key();
            if let Err(e) = secrets.set(&key_name, &k) {
                self.msg(false, &format!("Couldn't save the key: {e}"));
                return;
            }
        }
        let s = clean_settings(&self.f());
        let script = power::helper_setup_script(&secrets.get(&key_name).unwrap_or_default(), s.helper_port());
        copy_text(&script);
        {
            let mut fm = self.form.borrow_mut();
            for l in fm.info.logins.iter_mut() {
                if l.0 == "linux" {
                    l.1 = true;
                }
            }
        }
        if self.app.st.borrow().set.platform == "linux" {
            self.app.st.borrow_mut().has_login = true;
        }
        self.msg(true, "Setup command copied. Paste it into an SSH session on the server.");
        self.render(false);
    }

    fn restore(self: &Rc<Self>, id: Option<&str>) {
        {
            let mut store = self.app.ctx.store.borrow_mut();
            let mut d = store.load_data();
            match id {
                Some(id) => d.hidden.retain(|x| x != id),
                None => d.hidden.clear(),
            }
            store.save_data(&d);
            self.form.borrow_mut().info.hidden = d.hidden.clone();
        }
        self.app.st.borrow_mut().status_dirty = true;
        self.msg(true, if id.is_some() { "Button restored" } else { "All buttons restored" });
        self.app.refresh(false);
        self.render(false);
    }

    fn clear_logos(self: &Rc<Self>) {
        self.app.ctx.logos.clear_all();
        self.app.ctx.textures.borrow_mut().clear();
        self.msg(true, "Logo cache cleared. Logos download again now.");
        self.app.refresh(false);
        self.app.refresh_logos();
    }

    fn export(self: &Rc<Self>) {
        let text = self.app.ctx.store.borrow_mut().export();
        copy_text(&text);
        self.msg(true, "All servers' settings copied to the clipboard. Passwords are not included.");
    }

    fn import(self: &Rc<Self>) {
        let Some(d) = gdk::Display::default() else { return };
        let me = self.clone();
        d.clipboard().read_text_async(gio_none(), move |r| {
            let text = r.ok().flatten().map(|t| t.to_string()).unwrap_or_default();
            let res = me.app.ctx.store.borrow_mut().import(&text);
            match res {
                Err(e) => {
                    me.msg(false, &e);
                }
                Ok(()) => {
                    me.msg(true, "Settings imported");
                    me.form.borrow_mut().saved = true;
                    let look = me.app.ctx.store.borrow_mut().settings().look();
                    me.app.st.borrow_mut().look = look.clone();
                    me.app.apply_look(&look);
                    me.app.modals.close_of(&me.panel);
                    me.app.show_server(0);
                }
            }
        });
    }

    fn reset(self: &Rc<Self>) {
        {
            let mut store = self.app.ctx.store.borrow_mut();
            let mut d = store.load_data();
            d.order.clear();
            d.widget_order.clear();
            d.widget_hidden.clear();
            d.groups.clear();
            store.save_data(&d);
        }
        self.msg(true, "Layout reset. Your own buttons are kept.");
        self.app.refresh(false);
    }

    fn save(self: &Rc<Self>) {
        if self.app.modals.ignoring() || self.form.borrow().busy {
            return;
        }
        let f = self.f();
        let whole = Regex::new(r"^\d+$").unwrap();
        let n = |v: &Value| whole.is_match(&match v {
            Value::Number(x) => x.to_string(),
            Value::String(s) => s.clone(),
            _ => String::new(),
        });
        for k in ["maxDisks", "liveSeconds", "refreshMinutes", "statusSeconds", "pingTimeout"] {
            if !n(&f[k]) {
                self.err.set_text("Use whole numbers for the number fields");
                return;
            }
        }
        for k in ["cpu", "nvme", "hdd"] {
            let t = &f["tempLimits"][k];
            if !n(&t["warm"]) || !n(&t["hot"]) {
                self.err.set_text("Use whole numbers for the temperatures");
                return;
            }
            let num = |v: &Value| v.as_i64().or_else(|| v.as_str().and_then(|s| s.parse().ok())).unwrap_or(0);
            if num(&t["hot"]) <= num(&t["warm"]) {
                self.err.set_text(&format!("Red has to start above orange ({})", k.to_uppercase()));
                return;
            }
        }
        if fs(&f, "serverHost").trim().is_empty() {
            self.err.set_text("Enter your server's IP or name (Settings > Server)");
            return;
        }
        let bu = fs(&f, "baseUrl");
        if !bu.is_empty() && !Regex::new(r"(?i)^https?://[^/:]+").unwrap().is_match(&bu) {
            self.err.set_text("The Glances address must start with http:// or https://");
            return;
        }
        let cleaned = clean_settings(&f);
        let (title, gpass, clear) = {
            let fm = self.form.borrow();
            (fm.title.trim().chars().take(20).collect::<String>(), fm.gpass.clone(), fm.clear_gpass)
        };
        let cur = self.cur();
        {
            let mut store = self.app.ctx.store.borrow_mut();
            let mut d = store.load_data();
            d.settings = cleaned.to_value();
            if !title.is_empty() {
                d.title = title;
            }
            store.save_data(&d);
        }
        let gk = crate::secrets::Secrets::gpass_key(&cur);
        if clear {
            self.app.ctx.secrets.remove(&gk);
        }
        if !gpass.is_empty() {
            let _ = self.app.ctx.secrets.set(&gk, &gpass);
        }
        self.form.borrow_mut().saved = true;
        {
            let mut s = self.app.st.borrow_mut();
            s.look = cleaned.look();
            s.status_dirty = true;
            s.next_stats = std::time::Instant::now();
        }
        self.app.fs_cache.clear();
        // ZimaOS: the address or type may have changed
        crate::zima::forget(&cur);
        {
            let mut s = self.app.st.borrow_mut();
            s.last_z_apps = None;
            if cleaned.platform != "zimaos" {
                s.z_apps = None;
                s.z_upg.clear();
                s.z_sys = None;
                s.z_drives = None;
            }
        }
        self.app.apply_look(&cleaned.look());
        self.msg(true, "Settings saved");
        self.app.modals.close_of(&self.panel);
        self.app.refresh(false);
        self.app.refresh_logos();
        super::widget::refresh_all();
    }
}

fn gio_none() -> Option<&'static gtk::gio::Cancellable> {
    None
}

pub fn copy_text(text: &str) {
    if let Some(d) = gdk::Display::default() {
        d.clipboard().set_text(text);
    }
}

fn result_line(ok: bool, text: &str) -> gtk::Box {
    let r = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    if !ok {
        r.add_css_class("bad");
    }
    let d = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    d.add_css_class("rdot");
    d.set_valign(gtk::Align::Start);
    r.append(&d);
    let l = modal::hint(text, "");
    r.append(&l);
    r
}
