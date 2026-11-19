//! Service buttons and stacks: what the dashboard and the widget show, in which order,
//! and every change you can make to them (add / edit / delete, drag order, stacks).

use crate::config::STACK_ICON;
use crate::store::{Data, Group};
use regex::Regex;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::sync::OnceLock;

/// Built-in buttons. Empty on purpose: add your own with the + button in the dashboard.
/// You can list some here, e.g. ("Jellyfin", 8096, "🎬", logo, path, https); they use the server address.
pub const BUILTIN: [(&str, u16, &str, &str, &str, bool); 0] = [];

#[derive(Debug, Clone, PartialEq)]
pub struct Service {
    pub id: String,
    pub name: String,
    pub port: String,
    pub icon: String,
    pub logo: String,
    pub url: String,
    pub scheme: String,
    /// host + path (what the address field shows)
    pub addr: String,
    /// ":port", or the host when there's no port
    pub label: String,
    pub on_widget: bool,
    /// the stack it's in
    pub group: Option<String>,
    /// the ZimaOS app it was added from ("Pick from ZimaOS apps"); others are linked by port
    pub zima: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Stack {
    pub id: String,
    pub name: String,
    pub logo: String,
    pub icon: String,
    pub expand: bool,
    pub members: Vec<Service>,
    pub on_widget: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    Svc(Service),
    Group(Stack),
}

impl Item {
    pub fn id(&self) -> &str {
        match self {
            Item::Svc(s) => &s.id,
            Item::Group(g) => &g.id,
        }
    }
    pub fn name(&self) -> &str {
        match self {
            Item::Svc(s) => &s.name,
            Item::Group(g) => &g.name,
        }
    }
    pub fn logo(&self) -> &str {
        match self {
            Item::Svc(s) => &s.logo,
            Item::Group(g) => &g.logo,
        }
    }
    pub fn icon(&self) -> &str {
        match self {
            Item::Svc(s) => &s.icon,
            Item::Group(g) => &g.icon,
        }
    }
    pub fn is_group(&self) -> bool {
        matches!(self, Item::Group(_))
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Layout {
    pub all: Vec<Service>,
    pub groups: Vec<Stack>,
    /// dashboard order (buttons not in a stack, plus stacks)
    pub dash: Vec<Item>,
    /// large widget order
    pub widget: Vec<Item>,
    /// hidden from the widget
    pub hidden: Vec<Item>,
}

macro_rules! rx {
    ($pat:expr) => {{
        static CELL: OnceLock<Regex> = OnceLock::new();
        CELL.get_or_init(|| Regex::new($pat).unwrap())
    }};
}

pub struct UrlParts {
    pub scheme: String,
    pub host: String,
    pub port: String,
    pub path: String,
}

pub fn split_url(url: &str) -> UrlParts {
    match rx!(r"(?i)^(https?)://([^/:]+)(?::(\d+))?(.*)$").captures(url) {
        Some(m) => UrlParts {
            scheme: m[1].to_lowercase(),
            host: m[2].to_string(),
            port: m.get(3).map(|x| x.as_str().to_string()).unwrap_or_default(),
            path: m.get(4).map(|x| x.as_str().to_string()).unwrap_or_default(),
        },
        None => UrlParts { scheme: "http".into(), host: url.to_string(), port: String::new(), path: String::new() },
    }
}

/// Applies a saved order (ids not in it go to the end, in their own order).
pub fn sort_by_order<T, F: Fn(&T) -> &str>(list: Vec<T>, order: &[String], id: F) -> Vec<T> {
    if order.is_empty() {
        return list;
    }
    let idx = |s: &T| order.iter().position(|o| o == id(s)).unwrap_or(usize::MAX);
    let mut v: Vec<(usize, T)> = list.into_iter().enumerate().collect();
    v.sort_by(|a, b| idx(&a.1).cmp(&idx(&b.1)).then(a.0.cmp(&b.0)));
    v.into_iter().map(|x| x.1).collect()
}

fn val_str(v: Option<&Value>) -> String {
    match v {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        _ => String::new(),
    }
}

/// The built-in buttons on this server (see BUILTIN).
pub fn builtin(host: &str) -> Vec<Value> {
    BUILTIN
        .iter()
        .map(|(name, port, icon, logo, path, https)| {
            json!({
                "id": format!("b:{name}"), "name": name, "port": port.to_string(), "icon": icon, "logo": logo,
                "url": format!("{}://{}:{}{}", if *https { "https" } else { "http" }, host, port, path),
            })
        })
        .collect()
}

/// All buttons: built-in (minus deleted, with edits applied), then custom.
/// for_dashboard = true applies your dashboard drag-and-drop order.
pub fn get_services(d: &Data, host: &str, for_dashboard: bool) -> Vec<Service> {
    let mut list: Vec<Value> = builtin(host)
        .into_iter()
        .filter(|s| !d.hidden.iter().any(|h| s["id"] == h.as_str()))
        .map(|mut s| {
            if let Some(Value::Object(e)) = d.edits.get(s["id"].as_str().unwrap()) {
                for (k, v) in e {
                    s[k] = v.clone();
                }
            }
            s
        })
        .collect();
    list.extend(d.custom.iter().cloned());
    let services: Vec<Service> = list
        .iter()
        .map(|s| {
            let url = val_str(s.get("url"));
            let u = split_url(&url);
            let port = val_str(s.get("port"));
            let id = val_str(s.get("id"));
            Service {
                on_widget: !d.widget_hidden.contains(&id),
                id,
                name: val_str(s.get("name")),
                label: if port.is_empty() { u.host.clone() } else { format!(":{port}") },
                port,
                icon: match val_str(s.get("icon")) {
                    i if i.is_empty() => "🔗".into(),
                    i => i,
                },
                logo: val_str(s.get("logo")),
                scheme: u.scheme,
                addr: format!("{}{}", u.host, u.path),
                zima: match val_str(s.get("zima")) {
                    z if z.is_empty() => val_str(s.get("app")),
                    z => z,
                },
                url,
                group: None,
            }
        })
        .collect();
    if for_dashboard {
        sort_by_order(services, &d.order, |s| &s.id)
    } else {
        services
    }
}

/// Top level = buttons not in a stack, plus stacks (each stack carries its member buttons).
pub fn get_layout(d: &Data, host: &str) -> Layout {
    let mut all = get_services(d, host, false);
    let mut in_stack: HashSet<String> = HashSet::new();
    let mut groups = vec![];
    for g in &d.groups {
        let mut members = vec![];
        for id in &g.members {
            if in_stack.contains(id) {
                continue;
            }
            if let Some(s) = all.iter_mut().find(|s| &s.id == id) {
                in_stack.insert(id.clone());
                s.group = Some(g.id.clone());
                members.push(s.clone());
            }
        }
        groups.push(Stack {
            id: g.id.clone(),
            name: g.name.clone(),
            logo: g.logo.clone(),
            icon: STACK_ICON.into(),
            expand: g.expand,
            members,
            on_widget: !d.widget_hidden.contains(&g.id),
        });
    }
    let mut top: Vec<Item> = all.iter().filter(|s| !in_stack.contains(&s.id)).cloned().map(Item::Svc).collect();
    top.extend(groups.iter().cloned().map(Item::Group));
    let on_w = |x: &Item| match x {
        Item::Svc(s) => s.on_widget,
        Item::Group(g) => g.on_widget,
    };
    let widget = sort_by_order(top.iter().filter(|x| on_w(x)).cloned().collect(), &d.widget_order, |x| x.id());
    let hidden = top.iter().filter(|x| !on_w(x)).cloned().collect();
    Layout { all, groups, dash: sort_by_order(top, &d.order, |x| x.id()), widget, hidden }
}

/// What the large widget actually draws: stacks become one pill, or their members if "show members" is on.
pub fn widget_cells(layout: &Layout, rows: i64) -> Vec<Item> {
    let mut cells = vec![];
    for x in &layout.widget {
        match x {
            Item::Group(g) if g.expand => cells.extend(g.members.iter().cloned().map(Item::Svc)),
            Item::Group(g) if g.members.is_empty() => {}
            other => cells.push(other.clone()),
        }
    }
    cells.truncate((rows.max(0) * 4) as usize);
    cells
}

/// Create / edit / ungroup stacks and move buttons in or out of them.
#[derive(Debug, Clone)]
pub enum StackAct {
    /// gid: the stack to edit (None = a new one), member: the button a new stack starts with
    Save { gid: Option<String>, member: Option<String>, name: String, logo: String, expand: bool },
    /// to: the stack (None = out of its stack)
    Move { id: String, to: Option<String> },
    /// ungroup
    Del { gid: String },
}

fn insert_at(list: &[String], anchor: Option<&str>, ids: &[String], after: bool) -> Vec<String> {
    let mut out: Vec<String> = list.iter().filter(|x| !ids.contains(x)).cloned().collect();
    let i = anchor.and_then(|a| out.iter().position(|x| x == a));
    match i {
        None => {
            out.extend(ids.iter().cloned());
            out
        }
        Some(mut i) => {
            if after {
                i += 1;
            }
            for (k, id) in ids.iter().enumerate() {
                out.insert(i + k, id.clone());
            }
            out
        }
    }
}

fn replace_in(list: &[String], old: &str, ids: &[String]) -> Vec<String> {
    insert_at(list, Some(old), ids, false).into_iter().filter(|x| x != old).collect()
}

/// Applies a stack change to the data (the caller saves it). Returns the stack's logo when it was saved.
pub fn stack_action(data: &mut Data, layout: &Layout, a: StackAct) {
    // freeze the current order so a new stack lands where its first button was
    data.order = layout.dash.iter().map(|x| x.id().to_string()).collect();
    data.widget_order = layout.widget.iter().map(|x| x.id().to_string()).collect();
    let holder = |data: &Data, id: &str| data.groups.iter().find(|g| g.members.iter().any(|m| m == id)).map(|g| g.id.clone());
    let strip = |data: &mut Data, id: &str| {
        for g in data.groups.iter_mut() {
            g.members.retain(|m| m != id);
        }
    };
    match a {
        StackAct::Save { gid, member, name, logo, expand } => {
            let name: String = name.trim().chars().take(24).collect();
            let name = if name.is_empty() { "Stack".to_string() } else { name };
            let logo = logo.trim().to_string();
            if let Some(g) = gid.as_ref().and_then(|gid| data.groups.iter_mut().find(|g| &g.id == gid)) {
                g.name = name;
                g.logo = logo;
                g.expand = expand;
            } else {
                let mut g = Group { id: format!("g:{}", chrono::Utc::now().timestamp_millis()), name, logo, expand, members: vec![] };
                while data.groups.iter().any(|x| x.id == g.id) {
                    g.id.push('0');
                }
                if let Some(m) = member {
                    let old = holder(data, &m);
                    strip(data, &m);
                    let gid = vec![g.id.clone()];
                    if let Some(old) = old {
                        data.order = insert_at(&data.order, Some(&old), &gid, true);
                        data.widget_order = insert_at(&data.widget_order, Some(&old), &gid, true);
                    } else {
                        data.order = replace_in(&data.order, &m, &gid);
                        data.widget_order = replace_in(&data.widget_order, &m, &gid);
                    }
                    g.members.push(m);
                } else {
                    data.order.push(g.id.clone());
                    data.widget_order.push(g.id.clone());
                }
                data.groups.push(g);
            }
        }
        StackAct::Move { id, to } => {
            let old = holder(data, &id);
            strip(data, &id);
            let target = to.as_ref().and_then(|t| data.groups.iter_mut().find(|g| &g.id == t));
            if let Some(t) = target {
                t.members.push(id.clone());
                data.order.retain(|x| x != &id);
                data.widget_order.retain(|x| x != &id);
            } else {
                // out of the stack: placed right after it
                let ids = vec![id];
                data.order = insert_at(&data.order, old.as_deref(), &ids, true);
                data.widget_order = insert_at(&data.widget_order, old.as_deref(), &ids, true);
            }
        }
        StackAct::Del { gid } => {
            // ungroup: the buttons go back where the stack was
            if let Some(g) = data.groups.iter().find(|g| g.id == gid).cloned() {
                data.order = replace_in(&data.order, &g.id, &g.members);
                data.widget_order = replace_in(&data.widget_order, &g.id, &g.members);
                if data.widget_hidden.contains(&g.id) {
                    data.widget_hidden.retain(|x| x != &g.id);
                    data.widget_hidden.extend(g.members.iter().cloned());
                }
                data.groups.retain(|x| x.id != gid);
            }
        }
    }
}

/// What the Add / Edit service popup sends.
#[derive(Debug, Clone, Default)]
pub struct ServiceForm {
    /// None = a new button
    pub id: Option<String>,
    pub name: String,
    pub scheme: String,
    pub addr: String,
    pub port: String,
    pub logo: String,
    pub on_widget: bool,
}

/// A button from the form: http / https, address (with an optional path) and port.
pub fn make_custom(f: &ServiceForm) -> Value {
    let mut addr = f.addr.trim().to_string();
    let scheme = match rx!(r"(?i)^(https?)://").captures(&addr) {
        Some(m) => m[1].to_lowercase(),
        None => {
            if f.scheme == "https" {
                "https".into()
            } else {
                "http".into()
            }
        }
    };
    addr = rx!(r"(?i)^https?://").replace(&addr, "").to_string();
    let mut path = String::new();
    if let Some(i) = addr.find('/') {
        path = addr[i..].to_string();
        addr.truncate(i);
    }
    let port: String = f.port.chars().filter(|c| c.is_ascii_digit()).collect();
    json!({
        "name": f.name.trim().chars().take(24).collect::<String>(),
        "port": port,
        "icon": "🔗",
        "logo": f.logo.trim(),
        "custom": true,
        "url": format!("{}://{}{}{}", scheme, addr, if port.is_empty() { String::new() } else { format!(":{port}") }, path),
    })
}

/// Saves an added or edited button. Returns its id and the logo URL.
pub fn save_service(data: &mut Data, f: &ServiceForm) -> (String, String) {
    let mut item = make_custom(f);
    let logo = item["logo"].as_str().unwrap_or("").to_string();
    let id = match &f.id {
        Some(id) if id.starts_with("b:") => {
            data.edits.insert(id.clone(), json!({ "name": item["name"], "port": item["port"], "url": item["url"], "logo": item["logo"] }));
            id.clone()
        }
        Some(id) => {
            if let Some(c) = data.custom.iter_mut().find(|c| c["id"] == id.as_str()) {
                for k in ["name", "port", "url", "logo"] {
                    c[k] = item[k].clone();
                }
            }
            id.clone()
        }
        None => {
            let mut id = format!("c:{}", chrono::Utc::now().timestamp_millis());
            while data.custom.iter().any(|c| c["id"] == id.as_str()) {
                id.push('0');
            }
            item["id"] = json!(id);
            data.custom.push(item);
            id
        }
    };
    data.widget_hidden.retain(|x| x != &id);
    if !f.on_widget {
        data.widget_hidden.push(id.clone());
    }
    (id, logo)
}

/// ZimaOS: adds installed apps as buttons, with their real address and icon. Returns the logo URLs.
pub fn add_zima_apps(data: &mut Data, apps: &[crate::zima::ZApp], host: &str) -> Vec<String> {
    let t = chrono::Utc::now().timestamp_millis();
    let mut logos = vec![];
    for (n, a) in apps.iter().enumerate() {
        let mut id = format!("c:{t}-{n}");
        while data.custom.iter().any(|c| c["id"] == id.as_str()) {
            id.push('0');
        }
        data.custom.push(json!({
            "id": id, "name": a.title.chars().take(24).collect::<String>(), "port": a.port, "icon": "📦", "logo": a.icon,
            "custom": true, "zima": a.id, "app": a.id, "url": crate::zima::app_url(a, host),
        }));
        logos.push(a.icon.clone());
    }
    logos
}

/// Deletes a button (a built-in one is remembered as deleted so it can be restored).
pub fn delete_service(data: &mut Data, id: &str) {
    if id.starts_with("b:") {
        data.edits.remove(id);
        if !data.hidden.iter().any(|h| h == id) {
            data.hidden.push(id.to_string());
        }
    } else {
        data.custom.retain(|c| c["id"] != id);
    }
    for g in data.groups.iter_mut() {
        g.members.retain(|m| m != id);
    }
}

/// Dashboard drag order: the top level, and each stack's buttons (buttons can be dragged in and out of open stacks).
pub fn set_dash_order(data: &mut Data, order: Vec<String>, bodies: &[(String, Vec<String>)]) {
    data.order = order;
    for g in data.groups.iter_mut() {
        if let Some((_, m)) = bodies.iter().find(|(id, _)| id == &g.id) {
            g.members = m.clone();
        }
    }
}

/// Widget layout: what's on the widget (in order) and what's hidden from it.
pub fn set_widget_order(data: &mut Data, shown: Vec<String>, hidden: Vec<String>) {
    data.widget_order = shown;
    data.widget_hidden = hidden;
}

/// How many widget slots an entry takes in "Widget layout" (a stack with "show members" counts its buttons).
pub fn widget_slots(x: &Item) -> usize {
    match x {
        Item::Svc(_) => 1,
        Item::Group(g) if g.expand => g.members.len(),
        Item::Group(g) => usize::from(!g.members.is_empty()),
    }
}

/// Guess a dashboard-icons logo slug from a name ("Uptime Kuma" -> "uptime-kuma").
pub fn logo_slug(name: &str) -> String {
    let n = name.trim().to_lowercase();
    let s = rx!(r"[^a-z0-9]+").replace_all(&n, "-");
    s.trim_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Data;
    use serde_json::Map;

    fn data() -> Data {
        Data {
            title: String::new(),
            settings: json!({}),
            custom: vec![],
            edits: Map::new(),
            hidden: vec![],
            order: vec![],
            widget_order: vec![],
            widget_hidden: vec![],
            groups: vec![],
        }
    }

    fn add(d: &mut Data, name: &str) -> String {
        save_service(d, &ServiceForm { name: name.into(), addr: "10.0.0.2".into(), port: "80".into(), on_widget: true, ..Default::default() }).0
    }

    fn ids(v: &[Item]) -> Vec<String> {
        v.iter().map(|x| x.id().to_string()).collect()
    }

    #[test]
    fn make_custom_splits_the_address() {
        let v = make_custom(&ServiceForm { name: " Jellyfin ".into(), scheme: "http".into(), addr: "HTTPS://media.lan/web".into(), port: "8 096".into(), logo: " x ".into(), on_widget: true, ..Default::default() });
        assert_eq!(v["url"], "https://media.lan:8096/web");
        assert_eq!(v["name"], "Jellyfin");
        assert_eq!(v["port"], "8096");
        assert_eq!(v["logo"], "x");
        let v = make_custom(&ServiceForm { name: "a".into(), scheme: "https".into(), addr: "app.example.com".into(), ..Default::default() });
        assert_eq!(v["url"], "https://app.example.com");
    }

    #[test]
    fn services_carry_their_address_parts() {
        let mut d = data();
        let id = add(&mut d, "A");
        let s = &get_services(&d, "h", false)[0];
        assert_eq!(s.id, id);
        assert_eq!(s.label, ":80");
        assert_eq!(s.addr, "10.0.0.2");
        assert_eq!(s.scheme, "http");
        assert_eq!(s.icon, "🔗");
        save_service(&mut d, &ServiceForm { id: Some(id.clone()), name: "A2".into(), addr: "x.y/p".into(), on_widget: false, ..Default::default() });
        let s = &get_services(&d, "h", false)[0];
        assert_eq!(s.name, "A2");
        assert_eq!(s.label, "x.y");
        assert_eq!(s.addr, "x.y/p");
        assert!(!s.on_widget);
        delete_service(&mut d, &id);
        assert!(get_services(&d, "h", false).is_empty());
    }

    #[test]
    fn ordering() {
        let v = sort_by_order(vec!["a", "b", "c", "d"], &["c".to_string(), "a".to_string()], |x| x);
        assert_eq!(v, vec!["c", "a", "b", "d"]);
    }

    #[test]
    fn stacks_create_move_ungroup() {
        let mut d = data();
        let a = add(&mut d, "A");
        let b = add(&mut d, "B");
        let c = add(&mut d, "C");
        d.order = vec![a.clone(), b.clone(), c.clone()];
        // new stack from B lands where B was
        let l = get_layout(&d, "h");
        stack_action(&mut d, &l, StackAct::Save { gid: None, member: Some(b.clone()), name: "Media".into(), logo: String::new(), expand: false });
        let g = d.groups[0].id.clone();
        let l = get_layout(&d, "h");
        assert_eq!(ids(&l.dash), vec![a.clone(), g.clone(), c.clone()]);
        assert_eq!(l.groups[0].members[0].id, b);
        assert_eq!(l.all.iter().find(|s| s.id == b).unwrap().group.as_deref(), Some(g.as_str()));
        // move C in
        stack_action(&mut d, &l, StackAct::Move { id: c.clone(), to: Some(g.clone()) });
        let l = get_layout(&d, "h");
        assert_eq!(ids(&l.dash), vec![a.clone(), g.clone()]);
        assert_eq!(l.groups[0].members.len(), 2);
        // widget: one pill, or the members with "show members"
        assert_eq!(widget_cells(&l, 6).len(), 2);
        assert_eq!(widget_slots(&l.dash[1]), 1);
        // move B out: right after the stack
        stack_action(&mut d, &l, StackAct::Move { id: b.clone(), to: None });
        let l = get_layout(&d, "h");
        assert_eq!(ids(&l.dash), vec![a.clone(), g.clone(), b.clone()]);
        // edit: show members on the widget
        stack_action(&mut d, &l, StackAct::Save { gid: Some(g.clone()), member: None, name: "  ".into(), logo: "".into(), expand: true });
        let l = get_layout(&d, "h");
        assert_eq!(l.groups[0].name, "Stack");
        assert_eq!(ids(&widget_cells(&l, 6)), vec![a.clone(), c.clone(), b.clone()]);
        // hidden from the widget, then ungrouped: its buttons stay hidden
        d.widget_hidden.push(g.clone());
        let l = get_layout(&d, "h");
        assert_eq!(ids(&l.hidden), vec![g.clone()]);
        stack_action(&mut d, &l, StackAct::Del { gid: g.clone() });
        let l = get_layout(&d, "h");
        assert!(l.groups.is_empty());
        assert_eq!(ids(&l.dash), vec![a.clone(), c.clone(), b.clone()]);
        assert_eq!(d.widget_hidden, vec![c.clone()]);
        // a new empty stack goes at the end
        stack_action(&mut d, &l, StackAct::Save { gid: None, member: None, name: "Empty".into(), logo: String::new(), expand: false });
        let l = get_layout(&d, "h");
        assert_eq!(l.dash.last().unwrap().name(), "Empty");
        assert!(widget_cells(&l, 6).iter().all(|x| x.name() != "Empty"), "empty stacks aren't drawn");
    }

    #[test]
    fn widget_cells_are_limited_by_rows() {
        let mut d = data();
        for i in 0..10 {
            add(&mut d, &format!("S{i}"));
        }
        assert_eq!(widget_cells(&get_layout(&d, "h"), 2).len(), 8);
    }

    #[test]
    fn dash_and_widget_orders() {
        let mut d = data();
        let a = add(&mut d, "A");
        let b = add(&mut d, "B");
        set_widget_order(&mut d, vec![b.clone()], vec![a.clone()]);
        let l = get_layout(&d, "h");
        assert_eq!(ids(&l.widget), vec![b.clone()]);
        assert_eq!(ids(&l.hidden), vec![a.clone()]);
        set_dash_order(&mut d, vec![b.clone(), a.clone()], &[]);
        assert_eq!(ids(&get_layout(&d, "h").dash), vec![b, a]);
    }

    #[test]
    fn zima_apps_become_linked_buttons() {
        let mut d = data();
        let a = crate::zima::ZApp { id: "immich".into(), title: "Immich".into(), icon: "https://x/i.png".into(), port: "2283".into(), scheme: "http".into(), host: String::new(), status: "running".into(), path: String::new(), app_type: "v2app".into(), store_id: "immich".into(), uncontrolled: false };
        assert_eq!(add_zima_apps(&mut d, &[a], "10.0.0.2"), vec!["https://x/i.png"]);
        let s = &get_services(&d, "10.0.0.2", false)[0];
        assert_eq!((s.url.as_str(), s.zima.as_str(), s.icon.as_str(), s.label.as_str()), ("http://10.0.0.2:2283", "immich", "📦", ":2283"));
        // exports from the Android app may only carry "app"
        d.custom[0].as_object_mut().unwrap().remove("zima");
        assert_eq!(get_services(&d, "h", false)[0].zima, "immich");
    }

    #[test]
    fn slugs() {
        assert_eq!(logo_slug(" Uptime Kuma! "), "uptime-kuma");
        assert_eq!(logo_slug("Home  Assistant"), "home-assistant");
    }
}
