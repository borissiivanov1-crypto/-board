//! The popups: add / edit a service, the button menu, stacks, rename, Reboot / Shut down,
//! add a server and delete a server.

use super::app::App;
use super::modal::{self, ModalOpts};
use super::{bg, settings_ui};
use crate::config::{self, LOGO_CDN};
use crate::layout::{self, Service, ServiceForm, Stack, StackAct};
use crate::net;
use crate::power::{self, Action};
use crate::settings::clean_settings;
use gtk::prelude::*;
use gtk::{gdk, glib};
use serde_json::json;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

fn err_label() -> gtk::Label {
    let l = gtk::Label::new(None);
    l.add_css_class("err");
    l.set_wrap(true);
    l.set_justify(gtk::Justification::Center);
    l.set_size_request(-1, 14);
    l
}

fn focus_later(e: &gtk::Entry) {
    let e = e.clone();
    glib::timeout_add_local_once(Duration::from_millis(80), move || {
        e.grab_focus();
    });
}

/// Logo field: URL entry, a preview and "Auto" (guesses a dashboard-icons logo from the name).
fn logo_field(app: &Rc<App>, parent: &gtk::Box, value: &str, name_entry: &gtk::Entry, hint_text: &str) -> gtk::Entry {
    let f = modal::field(parent, "LOGO URL (OPTIONAL)");
    let (w, e) = modal::input(value, "https://.../logo.png", false);
    e.set_input_purpose(gtk::InputPurpose::Url);
    let img = gtk::Image::new();
    img.set_pixel_size(26);
    img.add_css_class("lp");
    img.set_visible(false);
    w.append(&img);
    let auto = modal::chip("Auto");
    auto.set_valign(gtk::Align::Center);
    auto.set_margin_start(8);
    w.append(&auto);
    f.append(&w);
    let h = modal::hint(hint_text, "fh");
    f.append(&h);
    // preview (after a short pause in typing)
    let timer: Rc<RefCell<Option<glib::SourceId>>> = Rc::default();
    let preview = {
        let img = img.clone();
        let timer = timer.clone();
        let e = e.clone();
        Rc::new(move || {
            if let Some(t) = timer.borrow_mut().take() {
                t.remove();
            }
            let img = img.clone();
            let e = e.clone();
            let t2 = timer.clone();
            let id = glib::timeout_add_local_once(Duration::from_millis(350), move || {
                t2.borrow_mut().take();
                let v = e.text().trim().to_string();
                if !v.to_lowercase().starts_with("http://") && !v.to_lowercase().starts_with("https://") {
                    img.set_visible(false);
                    return;
                }
                let img2 = img.clone();
                let e2 = e.clone();
                bg(move || net::get_bytes(&v, 8.0).map(|b| (v, b)), move |r| {
                    match r {
                        Ok((v, b)) if e2.text().trim() == v => match gdk::Texture::from_bytes(&glib::Bytes::from(&b)) {
                            Ok(t) => {
                                img2.set_paintable(Some(&t));
                                img2.set_visible(true);
                            }
                            Err(_) => img2.set_visible(false),
                        },
                        _ => img2.set_visible(false),
                    }
                });
            });
            *timer.borrow_mut() = Some(id);
        })
    };
    let p = preview.clone();
    e.connect_changed(move |_| p());
    preview();
    let (e2, h2, n2, p2) = (e.clone(), h.clone(), name_entry.clone(), preview.clone());
    let _ = app;
    auto.connect_clicked(move |_| {
        let n = n2.text().trim().to_lowercase();
        if n.is_empty() {
            h2.set_text("Type a name first.");
            return;
        }
        let slug = layout::logo_slug(&n);
        let url = format!("{LOGO_CDN}{slug}.png");
        h2.set_text(&format!("Checking {slug}.png ..."));
        let (e3, h3, p3) = (e2.clone(), h2.clone(), p2.clone());
        bg(move || net::get_bytes(&url, 8.0).map(|_| url), move |r| match r {
            Ok(url) => {
                e3.set_text(&url);
                p3();
                h3.set_text(&format!("Found {slug}.png"));
            }
            Err(_) => h3.set_text(&format!("No logo named {slug}.png. Paste a URL instead.")),
        });
    });
    e
}

/// Add or edit a service button.
pub fn open_add(app: &Rc<App>, sv: Option<&Service>) {
    let m = app.modals.show(ModalOpts { name: "add", ..Default::default() }, None);
    m.append(&modal::heading(if sv.is_some() { "EDIT SERVICE" } else { "ADD SERVICE" }));
    let f = modal::field(&m, "SERVICE NAME");
    let (w, name) = modal::input(sv.map(|s| s.name.as_str()).unwrap_or(""), "e.g. Jellyfin", false);
    name.set_max_length(24);
    f.append(&w);
    let f = modal::field(&m, "ADDRESS");
    let host = app.st.borrow().set.server_host();
    let (w, addr) = modal::input(sv.map(|s| s.addr.as_str()).unwrap_or(""), "", false);
    addr.set_input_purpose(gtk::InputPurpose::Url);
    let scheme = modal::scheme_select(sv.map(|s| s.scheme.as_str()).unwrap_or("http"));
    w.prepend(&scheme);
    f.append(&w);
    let use_port = sv.map(|s| !s.port.is_empty()).unwrap_or(true);
    let (prow, psw) = modal::toggle_row("USE PORT", use_port);
    m.append(&prow);
    let pf = modal::field(&m, "PORT");
    let (w, port) = modal::input(sv.map(|s| s.port.as_str()).unwrap_or(""), "8080", false);
    port.set_max_length(5);
    port.set_input_purpose(gtk::InputPurpose::Digits);
    pf.append(&w);
    let apply_port = {
        let (pf, addr, host) = (pf.clone(), addr.clone(), host.clone());
        move |on: bool| {
            pf.set_visible(on);
            addr.set_placeholder_text(Some(if on { if host.is_empty() { "192.168.1.10" } else { &host } } else { "app.example.com" }));
        }
    };
    apply_port(use_port);
    psw.connect_active_notify(move |s| apply_port(s.is_active()));
    let logo = logo_field(app, &m, sv.map(|s| s.logo.as_str()).unwrap_or(""), &name, "Empty = emoji. Auto guesses from the name.");
    let (wrow, wsw) = modal::toggle_row("SHOW ON WIDGET", sv.map(|s| s.on_widget).unwrap_or(true));
    wrow.set_visible(!sv.map(|s| s.group.is_some()).unwrap_or(false));
    m.append(&wrow);
    let er = err_label();
    m.append(&er);
    let b = modal::btns(false);
    let cancel = modal::button("Cancel", "");
    let ok = modal::button(if sv.is_some() { "Save" } else { "Add" }, "ok");
    b.append(&cancel);
    b.append(&ok);
    m.append(&b);
    let a = app.clone();
    let mm = m.clone();
    cancel.connect_clicked(move |_| a.modals.close_of(&mm));
    let a = app.clone();
    let id = sv.map(|s| s.id.clone());
    let submit = {
        let (name, addr, port, logo, er, psw, wsw, scheme, mm) = (name.clone(), addr.clone(), port.clone(), logo.clone(), er.clone(), psw.clone(), wsw.clone(), scheme.clone(), m.clone());
        Rc::new(move || {
            if a.modals.ignoring() {
                return;
            }
            let n = name.text().trim().to_string();
            let ad = addr.text().trim().to_string();
            let lu = logo.text().trim().to_string();
            if n.is_empty() || ad.is_empty() {
                er.set_text("Enter a name and an address");
                return;
            }
            let ll = lu.to_lowercase();
            if !lu.is_empty() && !(ll.starts_with("http://") || ll.starts_with("https://")) {
                er.set_text("Logo URL must start with http:// or https://");
                return;
            }
            let p: String = if psw.is_active() { port.text().chars().filter(|c| c.is_ascii_digit()).collect() } else { String::new() };
            a.modals.close_of(&mm);
            let form = ServiceForm { id: id.clone(), name: n, scheme: modal::scheme_of(&scheme).into(), addr: ad, port: p, logo: lu, on_widget: wsw.is_active() };
            {
                let mut store = a.ctx.store.borrow_mut();
                let mut d = store.load_data();
                let (_, logo) = layout::save_service(&mut d, &form);
                store.save_data(&d);
                a.ctx.logos.clear_fail(&logo); // a re-saved logo gets a fresh download try
            }
            a.st.borrow_mut().status_dirty = true;
            a.refresh(false);
            a.refresh_logos();
        })
    };
    let s = submit.clone();
    ok.connect_clicked(move |_| s());
    for e in [&name, &addr, &port, &logo] {
        let s = submit.clone();
        modal::on_enter(e, move || s());
    }
    if sv.is_none() {
        focus_later(&name);
    }
}

/// Hold a button: add to a stack, edit, delete.
pub fn open_menu(app: &Rc<App>, sv: &Service) {
    let m = app.modals.show(ModalOpts { name: "menu", ..Default::default() }, None);
    m.append(&modal::heading(&sv.name));
    let b = modal::btns(true);
    let stack = modal::button(if sv.group.is_some() { "Change stack" } else { "Add to stack" }, "");
    let edit = modal::button("Edit", "gtxt");
    let del = modal::button("Delete", "rtxt");
    let cancel = modal::button("Cancel", "");
    for x in [&stack, &edit, &del, &cancel] {
        b.append(x);
    }
    m.append(&b);
    let (a, mm, s2) = (app.clone(), m.clone(), sv.clone());
    stack.connect_clicked(move |_| {
        if a.modals.ignoring() {
            return;
        }
        a.modals.close_of(&mm);
        open_pick(&a, &s2);
    });
    let (a, mm, s2) = (app.clone(), m.clone(), sv.clone());
    edit.connect_clicked(move |_| {
        if a.modals.ignoring() {
            return;
        }
        a.modals.close_of(&mm);
        open_add(&a, Some(&s2));
    });
    let (a, mm, id) = (app.clone(), m.clone(), sv.id.clone());
    let armed = Rc::new(Cell::new(false));
    del.connect_clicked(move |b| {
        if a.modals.ignoring() {
            return;
        }
        if !armed.get() {
            armed.set(true);
            b.set_label("Click again to delete");
            return;
        }
        a.modals.close_of(&mm);
        {
            let mut store = a.ctx.store.borrow_mut();
            let mut d = store.load_data();
            layout::delete_service(&mut d, &id);
            store.save_data(&d);
        }
        a.st.borrow_mut().status_dirty = true;
        a.refresh(false);
    });
    let (a, mm) = (app.clone(), m.clone());
    cancel.connect_clicked(move |_| a.modals.close_of(&mm));
}

fn stack_act(app: &Rc<App>, act: StackAct) {
    {
        let mut store = app.ctx.store.borrow_mut();
        let mut d = store.load_data();
        let lay = app.st.borrow().layout.clone();
        if let StackAct::Save { logo, .. } = &act {
            app.ctx.logos.clear_fail(logo.trim());
        }
        layout::stack_action(&mut d, &lay, act);
        store.save_data(&d);
    }
    app.st.borrow_mut().status_dirty = true;
    app.refresh(false);
    app.refresh_logos();
}

/// Pick a stack for a button.
pub fn open_pick(app: &Rc<App>, sv: &Service) {
    let m = app.modals.show(ModalOpts { name: "pick", ..Default::default() }, None);
    m.append(&modal::heading(if sv.group.is_some() { "MOVE TO STACK" } else { "ADD TO STACK" }));
    let groups: Vec<Stack> = app.st.borrow().layout.groups.clone();
    let list = modal::btns(true);
    let mut any = false;
    for g in groups.iter().filter(|g| Some(&g.id) != sv.group.as_ref()) {
        any = true;
        let b = modal::button(&g.name, "");
        let (a, mm, id, to) = (app.clone(), m.clone(), sv.id.clone(), g.id.clone());
        b.connect_clicked(move |_| {
            if a.modals.ignoring() {
                return;
            }
            a.modals.close_of(&mm);
            stack_act(&a, StackAct::Move { id: id.clone(), to: Some(to.clone()) });
        });
        list.append(&b);
    }
    if any {
        m.append(&list);
    }
    let b = modal::btns(true);
    let new = modal::button("New stack", "ok");
    let rm = modal::button("Remove from stack", "del");
    rm.set_visible(sv.group.is_some());
    let cancel = modal::button("Cancel", "");
    b.append(&new);
    b.append(&rm);
    b.append(&cancel);
    m.append(&b);
    let (a, mm, s2) = (app.clone(), m.clone(), sv.clone());
    new.connect_clicked(move |_| {
        if a.modals.ignoring() {
            return;
        }
        a.modals.close_of(&mm);
        open_stack(&a, None, Some(&s2));
    });
    let (a, mm, id) = (app.clone(), m.clone(), sv.id.clone());
    rm.connect_clicked(move |_| {
        if a.modals.ignoring() {
            return;
        }
        a.modals.close_of(&mm);
        stack_act(&a, StackAct::Move { id: id.clone(), to: None });
    });
    let (a, mm) = (app.clone(), m.clone());
    cancel.connect_clicked(move |_| a.modals.close_of(&mm));
}

/// Create / edit / ungroup a stack. member: the button a new stack starts with.
pub fn open_stack(app: &Rc<App>, g: Option<&Stack>, member: Option<&Service>) {
    let m = app.modals.show(ModalOpts { name: "stack", ..Default::default() }, None);
    m.append(&modal::heading(if g.is_some() { "EDIT STACK" } else { "NEW STACK" }));
    let f = modal::field(&m, "STACK NAME");
    let (w, name) = modal::input(g.map(|g| g.name.as_str()).unwrap_or(""), "e.g. Media", false);
    name.set_max_length(24);
    f.append(&w);
    let logo = logo_field(app, &m, g.map(|g| g.logo.as_str()).unwrap_or(""), &name, &format!("Empty = {}. Auto guesses from the name.", config::STACK_ICON));
    let (er_row, exp) = modal::toggle_row("SHOW MEMBERS ON WIDGET", g.map(|g| g.expand).unwrap_or(false));
    m.append(&er_row);
    let er = err_label();
    m.append(&er);
    let b = modal::btns(false);
    let cancel = modal::button("Cancel", "");
    let ok = modal::button(if g.is_some() { "Save" } else { "Create" }, "ok");
    b.append(&cancel);
    b.append(&ok);
    m.append(&b);
    if let Some(g) = g {
        let c = modal::btns(true);
        let un = modal::button("Ungroup", "del");
        c.append(&un);
        m.append(&c);
        let (a, mm, gid) = (app.clone(), m.clone(), g.id.clone());
        let armed = Rc::new(Cell::new(false));
        un.connect_clicked(move |b| {
            if a.modals.ignoring() {
                return;
            }
            if !armed.get() {
                armed.set(true);
                b.set_label("Click again to ungroup");
                return;
            }
            a.modals.close_of(&mm);
            stack_act(&a, StackAct::Del { gid: gid.clone() });
        });
    }
    let (a, mm) = (app.clone(), m.clone());
    cancel.connect_clicked(move |_| a.modals.close_of(&mm));
    let gid = g.map(|g| g.id.clone());
    let mem = member.map(|s| s.id.clone());
    let a = app.clone();
    let submit = {
        let (name, logo, er, exp, mm) = (name.clone(), logo.clone(), er.clone(), exp.clone(), m.clone());
        Rc::new(move || {
            if a.modals.ignoring() {
                return;
            }
            let n = name.text().trim().to_string();
            let lu = logo.text().trim().to_string();
            if n.is_empty() {
                er.set_text("Enter a name");
                return;
            }
            let ll = lu.to_lowercase();
            if !lu.is_empty() && !(ll.starts_with("http://") || ll.starts_with("https://")) {
                er.set_text("Logo URL must start with http:// or https://");
                return;
            }
            a.modals.close_of(&mm);
            stack_act(&a, StackAct::Save { gid: gid.clone(), member: mem.clone(), name: n, logo: lu, expand: exp.is_active() });
        })
    };
    let s = submit.clone();
    ok.connect_clicked(move |_| s());
    for e in [&name, &logo] {
        let s = submit.clone();
        modal::on_enter(e, move || s());
    }
    if g.is_none() {
        focus_later(&name);
    }
}

/// Hold the server name: rename it.
pub fn open_rename(app: &Rc<App>) {
    if app.modals.is_open("rename") {
        return;
    }
    let m = app.modals.show(ModalOpts { name: "rename", ..Default::default() }, None);
    m.append(&modal::heading("RENAME"));
    let f = modal::field(&m, "DASHBOARD NAME");
    let (w, e) = modal::input(&app.st.borrow().title, "", false);
    e.set_max_length(20);
    f.append(&w);
    let er = err_label();
    m.append(&er);
    let b = modal::btns(false);
    let cancel = modal::button("Cancel", "");
    let ok = modal::button("Save", "ok");
    b.append(&cancel);
    b.append(&ok);
    m.append(&b);
    let (a, mm) = (app.clone(), m.clone());
    cancel.connect_clicked(move |_| a.modals.close_of(&mm));
    let a = app.clone();
    let save = {
        let (e, er, mm) = (e.clone(), er.clone(), m.clone());
        Rc::new(move || {
            let v: String = e.text().trim().chars().take(20).collect();
            if v.is_empty() {
                er.set_text("Enter a name");
                return;
            }
            a.modals.close_of(&mm);
            {
                let mut store = a.ctx.store.borrow_mut();
                let mut d = store.load_data();
                d.title = v;
                store.save_data(&d);
            }
            a.refresh(false);
        })
    };
    let s = save.clone();
    ok.connect_clicked(move |_| s());
    modal::on_enter(&e, move || save());
    let e2 = e.clone();
    glib::timeout_add_local_once(Duration::from_millis(80), move || {
        e2.grab_focus();
        e2.select_region(0, -1);
    });
}

/// Reboot / Shut down: confirm (and the login when none is saved, or it was refused).
pub fn open_power(app: &Rc<App>, a: &str, err: &str, need_login: bool) {
    let re = a == "restart";
    let (login, user_ph, has_login) = {
        let s = app.st.borrow();
        let p = config::platform(&s.set.platform);
        (p.and_then(|p| p.login), p.map(|p| p.user_ph).unwrap_or(""), s.has_login)
    };
    let m = app.modals.show(ModalOpts { name: "power", ..Default::default() }, None);
    m.append(&modal::heading(if re { "REBOOT SERVER?" } else { "SHUT DOWN SERVER?" }));
    let note = modal::hint(
        if re { "Every service goes offline until the server is back up, usually a minute or two." } else { "Every service goes offline. You'll have to switch the server back on by hand." },
        "cnote",
    );
    note.set_justify(gtk::Justification::Center);
    note.set_xalign(0.5);
    m.append(&note);
    // Linux uses the helper's key from Settings, so it never asks for a login here
    let show_login = login.is_some() && (need_login || !has_login);
    let (mut ue, mut pe) = (None, None);
    if show_login {
        let ln = login.unwrap().to_uppercase();
        let f = modal::field(&m, &format!("{ln} USERNAME"));
        let (w, u) = modal::input("", user_ph, false);
        f.append(&w);
        let f = modal::field(&m, &format!("{ln} PASSWORD"));
        let (w, p) = modal::input("", "", true);
        f.append(&w);
        m.append(&modal::hint(&format!("Asked once, then saved. {}.", app.ctx.secrets.place()), "fh"));
        focus_later(&u);
        ue = Some(u);
        pe = Some(p);
    }
    let er = err_label();
    er.set_text(err);
    m.append(&er);
    let b = modal::btns(false);
    let cancel = modal::button("Cancel", "");
    let ok = modal::button(if re { "Reboot" } else { "Shut down" }, "del");
    b.append(&cancel);
    b.append(&ok);
    m.append(&b);
    let (ap, mm) = (app.clone(), m.clone());
    cancel.connect_clicked(move |_| ap.modals.close_of(&mm));
    let ap = app.clone();
    let action = a.to_string();
    let go = Rc::new(move || {
        if ap.modals.ignoring() {
            return;
        }
        let typed = match (&ue, &pe) {
            (Some(u), Some(p)) => {
                let (u, p) = (u.text().trim().to_string(), p.text().to_string());
                if u.is_empty() || p.is_empty() {
                    er.set_text(&format!("Enter your {} username and password", login.unwrap_or("")));
                    return;
                }
                Some((u, p))
            }
            _ => None,
        };
        ap.modals.close_of(&m);
        ap.toast.show(if action == "restart" { "Sending reboot..." } else { "Sending shutdown..." }, 15000);
        let (set, cur) = {
            let s = ap.st.borrow();
            (s.set.clone(), s.cur.clone())
        };
        let secrets = ap.ctx.secrets.clone();
        let act = Action::parse(&action);
        let a2 = ap.clone();
        let action2 = action.clone();
        bg(move || power::handle_power(act, typed, &set, &cur, &secrets), move |(r, has)| {
            a2.st.borrow_mut().has_login = has;
            if r.auth {
                a2.toast.show("", 1);
                open_power(&a2, &action2, &r.msg, true);
                return;
            }
            a2.toast.show(&r.msg, 5000);
        });
    });
    let g = go.clone();
    ok.connect_clicked(move |_| g());
}

struct AddServer {
    name: String,
    platform: String,
    host: String,
    scheme: String,
    port: String,
    u: String,
    p: String,
    err: String,
    busy: bool,
}

/// Add a server: the same questions in the same order as its Settings (name, type, address, login).
pub fn open_add_server(app: &Rc<App>) {
    app.w.pmenu.popdown();
    let st = Rc::new(RefCell::new(AddServer { name: String::new(), platform: "none".into(), host: String::new(), scheme: String::new(), port: String::new(), u: String::new(), p: String::new(), err: String::new(), busy: false }));
    let m = app.modals.show(ModalOpts { name: "addserver", ..Default::default() }, None);
    render_add_server(app, &m, &st, true);
}

fn render_add_server(app: &Rc<App>, m: &gtk::Box, st: &Rc<RefCell<AddServer>>, focus: bool) {
    while let Some(c) = m.first_child() {
        m.remove(&c);
    }
    m.append(&modal::heading("ADD SERVER"));
    let note = modal::hint("Each server has its own address, type, logins, Monitoring, buttons and layout. Swipe the server name (or press Ctrl+Page Down) to switch between them.", "cnote");
    note.set_justify(gtk::Justification::Center);
    note.set_xalign(0.5);
    m.append(&note);
    let s = st.borrow();
    let f = modal::field(m, "NAME");
    let (w, name) = modal::input(&s.name, "e.g. NAS", false);
    name.set_max_length(20);
    f.append(&w);
    let st2 = st.clone();
    name.connect_changed(move |e| st2.borrow_mut().name = e.text().to_string());
    let f = modal::field(m, "SERVER TYPE");
    let (a, m2, st2) = (app.clone(), m.clone(), st.clone());
    f.append(&modal::radio_row(&settings_ui::type_opts(), &s.platform, true, Rc::new(move |v| {
        {
            let mut x = st2.borrow_mut();
            x.platform = v;
            x.scheme.clear();
            x.err.clear();
        }
        render_add_server(&a, &m2, &st2, false);
    })));
    m.append(&modal::hint(config::type_hint(&s.platform), "fh"));
    let st2 = st.clone();
    settings_ui::server_addr_field(m, "SERVER ADDRESS", &s.platform, &s.host, &s.scheme, &s.port, Rc::new(move |h, sc, p| {
        let mut x = st2.borrow_mut();
        x.host = h;
        x.scheme = sc;
        x.port = p;
    }));
    let pl = config::platform(&s.platform);
    let mut entries = vec![name.clone()];
    if let Some(login) = pl.and_then(|p| p.login) {
        let f = modal::field(m, &format!("{} USERNAME", login.to_uppercase()));
        let (w, u) = modal::input(&s.u, pl.unwrap().user_ph, false);
        f.append(&w);
        let st2 = st.clone();
        u.connect_changed(move |e| st2.borrow_mut().u = e.text().to_string());
        let f = modal::field(m, &format!("{} PASSWORD", login.to_uppercase()));
        let (w, p) = modal::input(&s.p, "", true);
        f.append(&w);
        let st2 = st.clone();
        p.connect_changed(move |e| st2.borrow_mut().p = e.text().to_string());
        m.append(&modal::hint(&format!("For Reboot / Shut down. Checked against the server before it's added, then saved ({}). Leave empty to be asked the first time.", app.ctx.secrets.place().to_lowercase()), "fh"));
        entries.push(u);
        entries.push(p);
    } else if s.platform == "linux" {
        m.append(&modal::hint("After adding, its Settings open so you can set up the helper (one command over SSH).", "fh"));
    }
    let er = err_label();
    er.set_text(&s.err);
    m.append(&er);
    let b = modal::btns(false);
    let cancel = modal::button("Cancel", "");
    let ok = modal::button(if s.busy { "Checking..." } else { "Add" }, "ok");
    b.append(&cancel);
    b.append(&ok);
    m.append(&b);
    drop(s);
    let (a, mm) = (app.clone(), m.clone());
    cancel.connect_clicked(move |_| a.modals.close_of(&mm));
    let (a, mm, st2) = (app.clone(), m.clone(), st.clone());
    let submit = Rc::new(move || submit_add_server(&a, &mm, &st2));
    let s2 = submit.clone();
    ok.connect_clicked(move |_| s2());
    for e in &entries {
        let s2 = submit.clone();
        modal::on_enter(e, move || s2());
    }
    if focus {
        focus_later(&name);
    }
}

fn submit_add_server(app: &Rc<App>, m: &gtk::Box, st: &Rc<RefCell<AddServer>>) {
    if app.modals.ignoring() || st.borrow().busy {
        return;
    }
    let fail = |t: &str| {
        st.borrow_mut().err = t.into();
        render_add_server(app, m, st, false);
    };
    let (platform, host, u, p) = {
        let s = st.borrow();
        (s.platform.clone(), s.host.trim().to_string(), s.u.trim().to_string(), s.p.clone())
    };
    if platform == "none" {
        return fail("Pick the server type");
    }
    if host.is_empty() {
        return fail("Enter the server's IP or name");
    }
    if (!u.is_empty() && p.is_empty()) || (u.is_empty() && !p.is_empty()) {
        return fail("Enter both the username and the password, or neither");
    }
    {
        let mut s = st.borrow_mut();
        s.err.clear();
        s.busy = true;
    }
    render_add_server(app, m, st, false);
    // the new server's address and type; the login (if given) is checked before the server is added
    let (name, scheme, port) = {
        let s = st.borrow();
        (s.name.trim().to_string(), s.scheme.clone(), s.port.clone())
    };
    let own = clean_settings(&json!({ "serverHost": host, "serverScheme": scheme, "serverPort": port, "platform": platform }));
    let with_login = power::has_driver(&own.platform) && !u.is_empty() && !p.is_empty();
    let (a, mm, st2) = (app.clone(), m.clone(), st.clone());
    let own2 = own.clone();
    let (u2, p2) = (u.clone(), p.clone());
    bg(
        move || if with_login { power::driver(&own2.platform, None, &u2, &p2, &own2.platform_base(&own2.platform), &own2) } else { None },
        move |r| {
            if let Some(r) = r.filter(|r| !r.ok) {
                let msg = if r.msg.is_empty() { "The login didn't work.".to_string() } else { r.msg.clone() };
                let error = format!("{msg}{}", if r.net { " Check the address, or add the server without the login and enter it later." } else { "" });
                {
                    let mut s = st2.borrow_mut();
                    s.busy = false;
                    s.err = error;
                }
                render_add_server(&a, &mm, &st2, false);
                return;
            }
            let id = a.ctx.store.borrow_mut().add_server(
                &name,
                json!({ "serverHost": own.server_host, "serverScheme": own.server_scheme, "serverPort": own.server_port, "platform": own.platform }),
            );
            if with_login {
                let _ = a.ctx.secrets.save_login(&own.platform, &id, &u, &p);
            }
            a.modals.close_of(&mm);
            a.show_server(1);
            // Linux still needs its helper set up: straight to its settings
            if own.platform == "linux" {
                let a2 = a.clone();
                glib::timeout_add_local_once(Duration::from_millis(450), move || settings_ui::open(&a2));
            } else {
                a.toast.show(if with_login { "Server added, login checked and saved" } else { "Server added" }, 3000);
            }
        },
    );
}

/// Delete server: a confirmation popup over the Settings sheet.
pub fn open_del_server(app: &Rc<App>, name: &str) {
    let id = app.st.borrow().cur.clone();
    let m = app.modals.show(ModalOpts { name: "delserver", ..Default::default() }, None);
    m.append(&modal::heading(&format!("DELETE {}?", if name.is_empty() { "SERVER".into() } else { name.to_uppercase() })));
    let n = modal::hint("Its settings, saved logins, buttons, stacks and layout are deleted from this computer. Your other servers stay. This can't be undone (unless you exported your settings).", "cnote");
    n.set_justify(gtk::Justification::Center);
    n.set_xalign(0.5);
    m.append(&n);
    let b = modal::btns(false);
    let cancel = modal::button("Cancel", "");
    let del = modal::button("Delete", "del");
    b.append(&cancel);
    b.append(&del);
    m.append(&b);
    let (a, mm) = (app.clone(), m.clone());
    cancel.connect_clicked(move |_| a.modals.close_of(&mm));
    let a = app.clone();
    del.connect_clicked(move |_| {
        if a.modals.ignoring() {
            return;
        }
        a.modals.close_all();
        let ok = a.ctx.store.borrow_mut().remove_server(&id);
        if ok {
            a.ctx.secrets.forget_server(&id);
            a.show_server(-1);
            a.toast.show("Server deleted", 2500);
        }
    });
}
