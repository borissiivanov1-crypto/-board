//! The data file: every server, its buttons and layout, and the shared settings.
//!
//! ```text
//! { v: 2, current (the server shown last), settings (shared by all servers),
//!   servers: [ { id, title, settings (this server's own), custom, edits, hidden, order,
//!                widgetOrder, widgetHidden, groups } ] }
//! ```
//! `load_data` / `save_data` work on one server at a time (`cur`): the shared and the server's own
//! settings come merged in `.settings`. A file from before there were several servers becomes
//! server 1 ("s1"). The format is the same as the iOS version's, so export / import work across both.

use crate::settings::{clean_settings, Settings, SERVER_SETTING_KEYS};
use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};

pub const SERVER_PARTS: [&str; 7] = ["custom", "edits", "hidden", "order", "widgetOrder", "widgetHidden", "groups"];

/// A stack: a group of connected services shown as one collapsible card.
#[derive(Debug, Clone, PartialEq)]
pub struct Group {
    pub id: String,
    pub name: String,
    pub logo: String,
    /// show the members (instead of one pill) on the widget
    pub expand: bool,
    pub members: Vec<String>,
}

impl Group {
    pub fn to_value(&self) -> Value {
        json!({ "id": self.id, "name": self.name, "logo": self.logo, "expand": self.expand, "members": self.members })
    }
}

/// One server's part of the data file, with the shared and its own settings merged.
#[derive(Debug, Clone, PartialEq)]
pub struct Data {
    /// the server's id (not saved; built-in buttons belong to the first server, "s1")
    pub id: String,
    pub title: String,
    pub settings: Value,
    /// buttons you added (kept as JSON so nothing is lost on the way through)
    pub custom: Vec<Value>,
    /// changes to built-in buttons
    pub edits: Map<String, Value>,
    /// deleted built-in buttons
    pub hidden: Vec<String>,
    pub order: Vec<String>,
    pub widget_order: Vec<String>,
    pub widget_hidden: Vec<String>,
    pub groups: Vec<Group>,
}

fn is_obj(v: Option<&Value>) -> bool {
    matches!(v, Some(Value::Object(_)))
}

fn str_list(v: Option<&Value>) -> Vec<String> {
    match v {
        Some(Value::Array(a)) => a.iter().filter_map(|x| x.as_str().map(String::from)).collect(),
        _ => vec![],
    }
}

/// Anything list-like as a JSON array (non-arrays become empty).
fn arr(v: Option<&Value>) -> Vec<Value> {
    match v {
        Some(Value::Array(a)) => a.clone(),
        _ => vec![],
    }
}

/// Milliseconds since 1970 in base 36 (the id style of the original).
pub fn time_id() -> String {
    let mut n = chrono::Utc::now().timestamp_millis().max(0) as u64;
    const DIG: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut out = vec![];
    loop {
        out.push(DIG[(n % 36) as usize]);
        n /= 36;
        if n == 0 {
            break;
        }
    }
    out.reverse();
    String::from_utf8(out).unwrap()
}

/// Whole-file JSON -> the normalised v2 shape.
pub fn normalize(d: Value) -> Value {
    let mut d = match d {
        Value::Array(a) => json!({ "custom": a }),
        Value::Object(_) => d,
        _ => json!({}),
    };
    if !matches!(d.get("servers"), Some(Value::Array(_))) {
        let st = d.get("settings").and_then(|v| v.as_object()).cloned().unwrap_or_default();
        let mut own = Map::new();
        let mut shared = Map::new();
        for (k, v) in st {
            if SERVER_SETTING_KEYS.contains(&k.as_str()) {
                own.insert(k, v);
            } else {
                shared.insert(k, v);
            }
        }
        let title = d.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let mut s1 = json!({ "id": "s1", "title": title, "settings": own });
        for k in SERVER_PARTS {
            if let Some(v) = d.get(k) {
                s1[k] = v.clone();
            }
        }
        d = json!({ "v": 2, "current": "s1", "settings": shared, "servers": [s1] });
    }
    let o = d.as_object_mut().unwrap();
    if !is_obj(o.get("settings")) {
        o.insert("settings".into(), json!({}));
    }
    let mut servers: Vec<Value> = arr(o.get("servers"))
        .into_iter()
        .filter(|x| x.is_object() && x.get("id").map(|i| i.is_string()).unwrap_or(false))
        .collect();
    if servers.is_empty() {
        servers.push(json!({ "id": "s1", "title": "", "settings": {} }));
    }
    for x in servers.iter_mut() {
        if !is_obj(x.get("settings")) {
            x["settings"] = json!({});
        }
    }
    let cur = o.get("current").and_then(|v| v.as_str()).unwrap_or("").to_string();
    if !servers.iter().any(|x| x["id"] == cur.as_str()) {
        let first = servers[0]["id"].clone();
        o.insert("current".into(), first);
    }
    o.insert("servers".into(), Value::Array(servers));
    d
}

pub struct Store {
    path: PathBuf,
    raw: Option<String>,
    /// The server shown now.
    pub cur: String,
    /// the first server's name when none is set (CONFIG.title)
    pub default_title: String,
    /// built-in buttons that start off the large widget (widget: false), on the first run of the first server
    pub builtin_hidden: Vec<String>,
}

impl Store {
    pub fn open(path: impl AsRef<Path>) -> Store {
        let c = crate::config::start();
        let mut s = Store {
            path: path.as_ref().to_path_buf(),
            raw: None,
            cur: String::new(),
            default_title: c.title.clone(),
            builtin_hidden: c.builtin.iter().filter(|b| !b.widget).map(|b| format!("b:{}", b.name)).collect(),
        };
        s.cur = s.read()["current"].as_str().unwrap_or("s1").to_string();
        s
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn raw(&mut self) -> String {
        if self.raw.is_none() {
            self.raw = Some(std::fs::read_to_string(&self.path).unwrap_or_default());
        }
        self.raw.clone().unwrap()
    }

    /// The whole file, normalised (a fresh copy every time).
    pub fn read(&mut self) -> Value {
        let raw = self.raw();
        let d = if raw.trim().is_empty() { json!({}) } else { serde_json::from_str(&raw).unwrap_or(json!({})) };
        normalize(d)
    }

    pub fn write(&mut self, st: &Value) {
        let raw = serde_json::to_string(st).unwrap_or_default();
        if let Some(dir) = self.path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        // write to a temporary file first so a crash never leaves half a file
        let tmp = self.path.with_extension("json.tmp");
        if std::fs::write(&tmp, &raw).is_ok() {
            let _ = std::fs::rename(&tmp, &self.path);
        }
        self.raw = Some(raw);
    }

    fn cur_index(&self, st: &Value) -> usize {
        let list = st["servers"].as_array().unwrap();
        list.iter()
            .position(|x| x["id"] == self.cur.as_str())
            .or_else(|| list.iter().position(|x| x["id"] == st["current"]))
            .unwrap_or(0)
    }

    /// [(id, title)] in order, with the name each one shows.
    pub fn list_servers(&mut self) -> Vec<(String, String)> {
        let st = self.read();
        st["servers"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
            .map(|(i, x)| (x["id"].as_str().unwrap().to_string(), display_title(x, i, &self.default_title)))
            .collect()
    }

    /// Which server a run starts on: `want` as the server's id, number (1, 2, ...) or name; otherwise
    /// the widget shows the first server and the dashboard the one shown last.
    pub fn initial_server(&mut self, want: Option<&str>, in_widget: bool) -> String {
        let st = self.read();
        let list = st["servers"].as_array().unwrap();
        let want = want.unwrap_or("").trim();
        if !want.is_empty() {
            let by_num = want.parse::<usize>().ok().filter(|n| *n >= 1).and_then(|n| list.get(n - 1));
            let hit = list
                .iter()
                .find(|x| x["id"] == want)
                .or(by_num)
                .or_else(|| {
                    list.iter().enumerate().find(|(i, x)| {
                        let t = x["title"].as_str().unwrap_or("");
                        let t = if t.is_empty() && *i == 0 { self.default_title.as_str() } else { t };
                        t.to_lowercase() == want.to_lowercase()
                    }).map(|(_, x)| x)
                });
            if let Some(h) = hit {
                return h["id"].as_str().unwrap().to_string();
            }
        }
        if in_widget {
            list[0]["id"].as_str().unwrap().to_string()
        } else {
            st["current"].as_str().unwrap().to_string()
        }
    }

    /// Show another server (remembered for the next time the dashboard opens).
    pub fn set_current(&mut self, id: &str) -> bool {
        let mut st = self.read();
        if !st["servers"].as_array().unwrap().iter().any(|x| x["id"] == id) {
            return false;
        }
        self.cur = id.to_string();
        st["current"] = json!(id);
        self.write(&st);
        true
    }

    /// Adds a server (own: its settings: address, type ...) and shows it. Returns its id.
    pub fn add_server(&mut self, name: &str, own: Value) -> String {
        let mut st = self.read();
        let mut id = format!("s{}", time_id());
        while st["servers"].as_array().unwrap().iter().any(|x| x["id"] == id.as_str()) {
            id.push('0');
        }
        let n = st["servers"].as_array().unwrap().len() + 1;
        let name: String = name.trim().chars().take(20).collect();
        let mut settings = json!({ "platform": "none" });
        if let Value::Object(m) = own {
            for (k, v) in m {
                settings[k] = v;
            }
        }
        let title = if name.is_empty() { format!("SERVER {n}") } else { name };
        st["servers"].as_array_mut().unwrap().push(json!({ "id": id, "title": title, "settings": settings }));
        self.write(&st);
        self.set_current(&id);
        id
    }

    /// Removes a server and its buttons and layout (the caller forgets its saved logins).
    pub fn remove_server(&mut self, id: &str) -> bool {
        let mut st = self.read();
        let list = st["servers"].as_array_mut().unwrap();
        if list.len() < 2 {
            return false;
        }
        let Some(i) = list.iter().position(|x| x["id"] == id) else { return false };
        list.remove(i);
        let next = list[i.min(list.len() - 1)]["id"].as_str().unwrap().to_string();
        st["current"] = json!(next);
        self.write(&st);
        self.cur = next;
        true
    }

    pub fn load_data(&mut self) -> Data {
        let st = self.read();
        let sv = &st["servers"][self.cur_index(&st)];
        let mut settings = st["settings"].clone();
        for (k, v) in sv["settings"].as_object().unwrap() {
            settings[k] = v.clone();
        }
        let custom = arr(sv.get("custom"))
            .into_iter()
            .enumerate()
            .filter(|(_, c)| c.is_object())
            .map(|(i, mut c)| {
                let has_id = c.get("id").map(|x| x.as_str().map(|s| !s.is_empty()).unwrap_or(false)).unwrap_or(false);
                if !has_id {
                    c["id"] = json!(format!("c:legacy{i}"));
                }
                c
            })
            .collect();
        let groups = arr(sv.get("groups"))
            .iter()
            .filter(|g| g.get("id").map(|x| x.is_string()).unwrap_or(false))
            .map(|g| {
                let name = match g.get("name") {
                    Some(Value::String(s)) if !s.is_empty() => s.clone(),
                    Some(Value::Number(n)) => n.to_string(),
                    _ => "Stack".into(),
                };
                Group {
                    id: g["id"].as_str().unwrap().to_string(),
                    name: name.chars().take(24).collect(),
                    logo: g.get("logo").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    expand: g.get("expand").map(truthy).unwrap_or(false),
                    members: str_list(g.get("members")),
                }
            })
            .collect();
        let id = sv["id"].as_str().unwrap_or("").to_string();
        // first run on the first server: built-in buttons marked widget: false start off the widget
        let widget_hidden = match sv.get("widgetHidden") {
            Some(Value::Array(_)) => str_list(sv.get("widgetHidden")),
            _ if id == "s1" => self.builtin_hidden.clone(),
            _ => vec![],
        };
        Data {
            id,
            title: sv.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            settings,
            custom,
            edits: sv.get("edits").and_then(|v| v.as_object()).cloned().unwrap_or_default(),
            hidden: str_list(sv.get("hidden")),
            order: str_list(sv.get("order")),
            widget_order: str_list(sv.get("widgetOrder")),
            widget_hidden,
            groups,
        }
    }

    /// The current server's settings, cleaned.
    pub fn settings(&mut self) -> Settings {
        clean_settings(&self.load_data().settings)
    }

    /// Dashboard / widget name of the current server.
    pub fn title(&mut self) -> String {
        let st = self.read();
        let i = self.cur_index(&st);
        display_title(&st["servers"][i], i, &self.default_title)
    }

    /// Saves the current server's part (and the shared settings) back into the file.
    pub fn save_data(&mut self, d: &Data) {
        let mut st = self.read();
        let i = self.cur_index(&st);
        let mut shared = st["settings"].as_object().cloned().unwrap_or_default();
        let mut own = Map::new();
        if let Some(m) = d.settings.as_object() {
            for (k, v) in m {
                if SERVER_SETTING_KEYS.contains(&k.as_str()) {
                    own.insert(k.clone(), v.clone());
                } else {
                    shared.insert(k.clone(), v.clone());
                }
            }
        }
        let sv = &mut st["servers"][i];
        sv["title"] = json!(d.title);
        sv["custom"] = Value::Array(d.custom.clone());
        sv["edits"] = Value::Object(d.edits.clone());
        sv["hidden"] = json!(d.hidden);
        sv["order"] = json!(d.order);
        sv["widgetOrder"] = json!(d.widget_order);
        sv["widgetHidden"] = json!(d.widget_hidden);
        sv["groups"] = Value::Array(d.groups.iter().map(|g| g.to_value()).collect());
        if d.settings.is_object() {
            sv["settings"] = Value::Object(own);
            st["settings"] = Value::Object(shared);
        }
        self.write(&st);
    }

    /// Export: every server, its buttons and layout, and the shared settings (no passwords).
    pub fn export(&mut self) -> String {
        let out = json!({ "board": 2, "exported": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true), "data": self.read() });
        serde_json::to_string_pretty(&out).unwrap_or_default()
    }

    /// Import exported settings. Err = not exported settings.
    pub fn import(&mut self, text: &str) -> Result<(), String> {
        const NOPE: &str = "The clipboard doesn't contain exported settings.";
        let obj: Value = serde_json::from_str(text.trim()).map_err(|_| NOPE.to_string())?;
        let ok = truthy(obj.get("board").unwrap_or(&Value::Null)) && obj.get("data").map(|d| d.is_object() || d.is_array()).unwrap_or(false);
        if !ok {
            return Err(NOPE.into());
        }
        let imp = obj["data"].clone();
        if matches!(imp.get("servers"), Some(Value::Array(_))) {
            // every server (an export from this version): replaces all of them
            let st = normalize(imp);
            self.write(&st);
            self.cur = st["current"].as_str().unwrap().to_string();
        } else {
            // an export from before there were several servers: goes into the server shown now
            let mut d = self.load_data();
            if let Some(m) = imp.get("settings").and_then(|v| v.as_object()) {
                for (k, v) in m {
                    d.settings[k] = v.clone();
                }
            }
            if let Some(t) = imp.get("title").and_then(|v| v.as_str()) {
                d.title = t.to_string();
            }
            let mut tmp = Store { path: PathBuf::new(), raw: Some(serde_json::to_string(&imp).unwrap()), cur: "s1".into(), default_title: self.default_title.clone(), builtin_hidden: vec![] };
            let part = tmp.load_data();
            for k in SERVER_PARTS {
                if imp.get(k).is_some() {
                    match k {
                        "custom" => d.custom = part.custom.clone(),
                        "edits" => d.edits = part.edits.clone(),
                        "hidden" => d.hidden = part.hidden.clone(),
                        "order" => d.order = part.order.clone(),
                        "widgetOrder" => d.widget_order = part.widget_order.clone(),
                        "widgetHidden" => d.widget_hidden = part.widget_hidden.clone(),
                        _ => d.groups = part.groups.clone(),
                    }
                }
            }
            self.save_data(&d);
        }
        Ok(())
    }
}

/// JavaScript truthiness for JSON values.
pub fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().map(|f| f != 0.0 && !f.is_nan()).unwrap_or(false),
        Value::String(s) => !s.is_empty(),
        _ => true,
    }
}

fn display_title(x: &Value, i: usize, default_title: &str) -> String {
    match x.get("title").and_then(|v| v.as_str()) {
        Some(t) if !t.is_empty() => t.to_string(),
        _ if i == 0 => default_title.to_string(),
        _ => format!("SERVER {}", i + 1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store_with(content: &str) -> (tempfile::TempDir, Store) {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("board-data.json");
        if !content.is_empty() {
            std::fs::write(&p, content).unwrap();
        }
        let s = Store::open(&p);
        (dir, s)
    }

    #[test]
    fn empty_file_gives_one_server() {
        let (_d, mut s) = store_with("");
        assert_eq!(s.cur, "s1");
        assert_eq!(s.list_servers(), vec![("s1".to_string(), "MY SERVER".to_string())]);
        assert_eq!(s.title(), "MY SERVER");
        let d = s.load_data();
        assert!(d.custom.is_empty() && d.groups.is_empty());
    }

    #[test]
    fn a_single_server_file_becomes_server_one() {
        let old = r#"{"title":"NAS","settings":{"serverHost":"10.0.0.2","theme":"ocean","showPower":true},
            "custom":[{"name":"Jellyfin","url":"http://10.0.0.2:8096","port":"8096"}],"order":["c:legacy0"]}"#;
        let (_d, mut s) = store_with(old);
        let st = s.read();
        assert_eq!(st["servers"][0]["settings"]["serverHost"], "10.0.0.2");
        assert_eq!(st["settings"]["theme"], "ocean");
        assert!(st["servers"][0]["settings"].get("theme").is_none());
        let d = s.load_data();
        assert_eq!(d.title, "NAS");
        assert_eq!(d.custom[0]["id"], "c:legacy0");
        assert_eq!(s.settings().platform, "zimaos");
        // a list of buttons only (the very first format)
        let (_d, mut s) = store_with(r#"[{"name":"A","url":"http://a"}]"#);
        assert_eq!(s.load_data().custom.len(), 1);
    }

    #[test]
    fn servers_add_switch_remove() {
        let (_d, mut s) = store_with("");
        let mut d = s.load_data();
        d.settings["theme"] = json!("rose");
        d.settings["serverHost"] = json!("one");
        s.save_data(&d);
        let id = s.add_server("  Backup box that has a very long name  ", json!({ "serverHost": "two", "platform": "linux" }));
        assert_eq!(s.cur, id);
        assert_eq!(s.title(), "Backup box that has ");
        let st = s.settings();
        assert_eq!(st.server_host, "two");
        assert_eq!(st.theme, "rose", "shared settings apply to every server");
        assert_eq!(s.initial_server(Some("2"), true), id);
        assert_eq!(s.initial_server(Some("my server"), true), "s1");
        assert_eq!(s.initial_server(None, true), "s1");
        assert_eq!(s.initial_server(None, false), id);
        assert!(s.set_current("s1"));
        assert!(!s.set_current("nope"));
        assert_eq!(s.settings().server_host, "one");
        assert!(s.remove_server("s1"));
        assert_eq!(s.cur, id);
        assert!(!s.remove_server(&id), "the only server can't be removed");
        let added = s.add_server("", json!({}));
        assert_eq!(s.list_servers()[1].1, "SERVER 2");
        assert_eq!(s.settings().platform, "none");
        assert_ne!(added, id);
    }

    #[test]
    fn export_and_import() {
        let (_d, mut s) = store_with("");
        s.add_server("B", json!({ "serverHost": "b" }));
        let text = s.export();
        let (_d2, mut t) = store_with("");
        assert!(t.import("nonsense").is_err());
        assert!(t.import(r#"{"x":1}"#).is_err());
        t.import(&text).unwrap();
        assert_eq!(t.list_servers().len(), 2);
        assert_eq!(t.cur, s.cur);
        // an export from the single-server version goes into the server shown now
        let (_d3, mut u) = store_with("");
        u.import(r#"{"board":1,"data":{"title":"OLD","settings":{"serverHost":"x","theme":"forest"},"custom":[{"id":"c:1","name":"A","url":"http://a"}]}}"#).unwrap();
        assert_eq!(u.title(), "OLD");
        assert_eq!(u.settings().theme, "forest");
        assert_eq!(u.settings().server_host, "x");
        assert_eq!(u.load_data().custom.len(), 1);
    }

    #[test]
    fn builtin_buttons_start_off_the_widget_on_the_first_server() {
        let (_d, mut s) = store_with("");
        s.builtin_hidden = vec!["b:ZimaOS".into()];
        s.default_title = "HOMELAB".into();
        assert_eq!(s.load_data().widget_hidden, vec!["b:ZimaOS"]);
        assert_eq!(s.title(), "HOMELAB");
        let mut d = s.load_data();
        d.widget_hidden.clear();
        s.save_data(&d);
        assert!(s.load_data().widget_hidden.is_empty(), "once saved, your layout wins");
        s.add_server("Two", json!({}));
        assert!(s.load_data().widget_hidden.is_empty());
        assert_eq!(s.load_data().id, s.cur);
    }

    #[test]
    fn data_survives_a_reopen() {
        let (dir, mut s) = store_with("");
        let mut d = s.load_data();
        d.groups.push(Group { id: "g:1".into(), name: "Media".into(), logo: String::new(), expand: true, members: vec!["c:1".into()] });
        d.order = vec!["g:1".into()];
        s.save_data(&d);
        let mut again = Store::open(dir.path().join("board-data.json"));
        assert_eq!(again.load_data(), d);
    }
}
