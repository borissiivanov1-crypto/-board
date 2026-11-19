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
use crate::zima::ZApp;
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
    // ZimaOS: add installed apps instead of typing them
    if sv.is_none() && app.zima_ready() {
        let pick = modal::chip("Pick from ZimaOS apps");
        pick.set_halign(gtk::Align::Center);
        let (a, mm) = (app.clone(), m.clone());
        pick.connect_clicked(move |_| {
            a.modals.close_of(&mm);
            open_zima_pick(&a);
        });
        m.append(&pick);
    }
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
    // ZimaOS app: Start / Stop / Restart / Logs / Update
    if let Some(za) = app.z_app_for(sv) {
        let upd = app.z_updatable(&za);
        let st = if za.running() { "running".to_string() } else if za.status.is_empty() { "stopped".into() } else { za.status.clone() };
        let note = modal::hint(&format!("ZimaOS app · {st}{}", if upd { " · update available" } else { "" }), "cnote");
        note.set_justify(gtk::Justification::Center);
        note.set_xalign(0.5);
        m.append(&note);
        let zb = modal::btns(true);
        let run = {
            let (a, mm, za) = (app.clone(), m.clone(), za.clone());
            Rc::new(move |action: &str, busy: &str| {
                a.modals.close_of(&mm);
                a.toast.show(&format!("{busy} {}...", za.title), 120_000);
                a.z_action(&za, action, None);
            })
        };
        if za.controllable() {
            if za.running() {
                let stop = modal::button("Stop", "rtxt");
                let armed = Rc::new(Cell::new(false));
                let r = run.clone();
                stop.connect_clicked(move |b| {
                    if !armed.get() {
                        armed.set(true);
                        b.set_label("Click again to stop");
                    } else {
                        r("stop", "Stopping");
                    }
                });
                zb.append(&stop);
                let re = modal::button("Restart", "");
                let r = run.clone();
                re.connect_clicked(move |_| r("restart", "Restarting"));
                zb.append(&re);
            } else {
                let start = modal::button("Start", "gtxt");
                let r = run.clone();
                start.connect_clicked(move |_| r("start", "Starting"));
                zb.append(&start);
            }
        }
        let lg = modal::button("Logs", "");
        let (a, mm, z2) = (app.clone(), m.clone(), za.clone());
        lg.connect_clicked(move |_| {
            a.modals.close_of(&mm);
            open_logs(&a, &z2);
        });
        zb.append(&lg);
        if upd && za.controllable() {
            let u = modal::button("Update", "gtxt");
            let r = run.clone();
            u.connect_clicked(move |_| r("update", "Updating"));
            zb.append(&u);
        }
        m.append(&zb);
    }
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

/// An app's icon from ZimaOS (or an empty square while it loads / if it has none).
fn app_icon(app: &Rc<App>, url: &str) -> gtk::Widget {
    let frame = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    frame.add_css_class("appicon");
    frame.set_size_request(36, 36);
    frame.set_valign(gtk::Align::Center);
    frame.set_hexpand(false);
    let img = gtk::Image::new();
    img.set_pixel_size(32);
    img.set_halign(gtk::Align::Center);
    img.set_valign(gtk::Align::Center);
    img.set_margin_start(2);
    frame.append(&img);
    if let Some(t) = app.ctx.texture(url) {
        img.set_paintable(Some(&t));
    } else if url.to_lowercase().starts_with("http") {
        let (u, i) = (url.to_string(), img.clone());
        bg(move || net::get_bytes(&u, 8.0), move |r| {
            if let Ok(t) = r.and_then(|b| gdk::Texture::from_bytes(&glib::Bytes::from(&b)).map_err(|e| e.to_string())) {
                i.set_paintable(Some(&t));
            }
        });
    }
    frame.upcast()
}

/// A row: icon, title and a smaller line, and something on the right.
fn app_row(app: &Rc<App>, a: &ZApp, sub: &str, right: &impl IsA<gtk::Widget>) -> gtk::Box {
    let r = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    r.append(&app_icon(app, &a.icon));
    let t = gtk::Box::new(gtk::Orientation::Vertical, 2);
    t.set_hexpand(true);
    t.set_valign(gtk::Align::Center);
    let n = modal::label(&a.title, "srow-t");
    n.set_ellipsize(gtk::pango::EllipsizeMode::End);
    t.append(&n);
    let s = modal::label(sub, "srow-s");
    s.set_ellipsize(gtk::pango::EllipsizeMode::End);
    t.append(&s);
    r.append(&t);
    right.set_valign(gtk::Align::Center);
    r.append(right);
    r
}

/// ZimaOS app logs: the last 300 lines, with Refresh.
pub fn open_logs(app: &Rc<App>, za: &ZApp) {
    let m = app.modals.show(ModalOpts { name: "logs", width: 420, ..Default::default() }, None);
    m.append(&modal::heading(&format!("{} LOGS", za.title.to_uppercase())));
    let text = gtk::Label::new(Some("Loading..."));
    text.add_css_class("logtext");
    text.set_xalign(0.0);
    text.set_yalign(0.0);
    text.set_wrap(true);
    text.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    text.set_selectable(true);
    let sc = gtk::ScrolledWindow::new();
    sc.add_css_class("logbox");
    sc.set_hscrollbar_policy(gtk::PolicyType::Never);
    sc.set_min_content_height(440);
    sc.set_max_content_height(440);
    sc.set_child(Some(&text));
    m.append(&sc);
    let b = modal::btns(false);
    let refresh = modal::button("Refresh", "");
    let close = modal::button("Close", "ok");
    b.append(&refresh);
    b.append(&close);
    m.append(&b);
    let (a, mm) = (app.clone(), m.clone());
    close.connect_clicked(move |_| a.modals.close_of(&mm));
    let load = {
        let (a, za, text, sc) = (app.clone(), za.clone(), text.clone(), sc.clone());
        Rc::new(move || {
            text.set_text("Loading...");
            text.remove_css_class("bad");
            let (set, cur) = {
                let s = a.st.borrow();
                (s.set.clone(), s.cur.clone())
            };
            let (secrets, id) = (a.ctx.secrets.clone(), za.id.clone());
            let (t2, sc2) = (text.clone(), sc.clone());
            bg(move || crate::zima::logs(&set, &id, 300, &cur, &secrets), move |r| {
                match r {
                    Ok(t) => t2.set_text(if t.is_empty() { "(no log lines)" } else { &t }),
                    Err(e) => {
                        t2.set_text(&e.msg);
                        t2.add_css_class("bad");
                    }
                }
                // the newest lines at the bottom, in view
                let adj = sc2.vadjustment();
                glib::timeout_add_local_once(Duration::from_millis(60), move || adj.set_value(adj.upper()));
            });
        })
    };
    let l = load.clone();
    refresh.connect_clicked(move |_| l());
    load();
}

/// ZimaOS app updates (the "↑ 2 updates" chip): an Update button per app.
pub fn open_updates(app: &Rc<App>) {
    let m = app.modals.show(ModalOpts { name: "updates", ..Default::default() }, None);
    m.append(&modal::heading("APP UPDATES"));
    let n = modal::hint("Newer versions in the ZimaOS App Store. Updating pulls the new image and recreates the app; its data stays.", "cnote");
    n.set_justify(gtk::Justification::Center);
    n.set_xalign(0.5);
    m.append(&n);
    // the list as it was when the popup opened (an app stays listed with its result after updating)
    let list = gtk::Box::new(gtk::Orientation::Vertical, 10);
    for za in app.z_updates() {
        let chip = modal::chip("Update");
        let (a, z2) = (app.clone(), za.clone());
        chip.connect_clicked(move |c| {
            if c.label().as_deref() != Some("Update") {
                return;
            }
            c.set_label("Updating...");
            let (c2, a2) = (c.clone(), a.clone());
            a.z_action(&z2, "update", Some(Box::new(move |ok, msg| {
                c2.set_label(if ok { "Started" } else { "Failed" });
                if !ok {
                    a2.toast.show(&msg, 6000);
                }
            })));
        });
        list.append(&app_row(app, &za, "Update available", &chip));
    }
    m.append(&list);
    let b = modal::btns(true);
    let close = modal::button("Close", "");
    b.append(&close);
    m.append(&b);
    let (a, mm) = (app.clone(), m.clone());
    close.connect_clicked(move |_| a.modals.close_of(&mm));
}

/// Add apps from ZimaOS (from the + popup): installed apps, ones with a button greyed out.
pub fn open_zima_pick(app: &Rc<App>) {
    let m = app.modals.show(ModalOpts { name: "zpick", ..Default::default() }, None);
    m.append(&modal::heading("ADD FROM ZIMAOS"));
    let n = modal::hint("Your installed ZimaOS apps. Pick the ones to add as buttons; they get the app's address, logo and the Start / Stop / Logs controls.", "cnote");
    n.set_justify(gtk::Justification::Center);
    n.set_xalign(0.5);
    m.append(&n);
    let body = gtk::Box::new(gtk::Orientation::Vertical, 12);
    let wait = modal::hint("Loading your apps...", "cnote");
    body.append(&wait);
    m.append(&body);
    let b = modal::btns(false);
    let cancel = modal::button("Cancel", "");
    let add = modal::button("Add", "ok");
    b.append(&cancel);
    b.append(&add);
    m.append(&b);
    let (a, mm) = (app.clone(), m.clone());
    cancel.connect_clicked(move |_| a.modals.close_of(&mm));
    let picked: Rc<RefCell<Vec<ZApp>>> = Rc::default();
    let (a, mm, p2) = (app.clone(), m.clone(), picked.clone());
    add.connect_clicked(move |_| {
        let sel = p2.borrow().clone();
        a.modals.close_of(&mm);
        a.add_zima_apps(&sel);
        a.toast.show(&format!("{} app{} added from ZimaOS", sel.len(), if sel.len() == 1 { "" } else { "s" }), 3500);
    });
    let (a, body2, add2) = (app.clone(), body.clone(), add.clone());
    app.refresh_zima(Some(Box::new(move |err| {
        while let Some(c) = body2.first_child() {
            body2.remove(&c);
        }
        if let Some(e) = err {
            let l = modal::hint(&e, "err");
            l.set_justify(gtk::Justification::Center);
            l.set_xalign(0.5);
            body2.append(&l);
            return;
        }
        let apps = a.st.borrow().z_apps.clone().unwrap_or_default();
        if apps.is_empty() {
            body2.append(&modal::hint("No apps installed on ZimaOS.", "cnote"));
            return;
        }
        let taken: std::collections::HashSet<String> = {
            let all = a.st.borrow().layout.all.clone();
            all.iter().filter_map(|s| a.z_app_for(s).map(|z| z.id)).collect()
        };
        for za in apps {
            let has = taken.contains(&za.id);
            let sub = if has { "Already a button".to_string() } else { format!("{}{}", if za.port.is_empty() { String::new() } else { format!("port {} · ", za.port) }, za.status) };
            let right: gtk::Widget = if has {
                gtk::Box::new(gtk::Orientation::Horizontal, 0).upcast()
            } else {
                let sw = gtk::Switch::new();
                let (p3, z3, add3) = (picked.clone(), za.clone(), add2.clone());
                sw.connect_active_notify(move |w| {
                    let mut p = p3.borrow_mut();
                    p.retain(|x| x.id != z3.id);
                    if w.is_active() {
                        p.push(z3.clone());
                    }
                    add3.set_label(&if p.is_empty() { "Add".to_string() } else { format!("Add {}", p.len()) });
                });
                sw.upcast()
            };
            let row = app_row(&a, &za, &sub, &right);
            if has {
                row.set_opacity(0.4);
            }
            body2.append(&row);
        }
    })));
}
