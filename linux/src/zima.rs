//! ZimaOS integration (only for servers whose type is ZimaOS, with the ZimaOS login saved).
//! Apps: start / stop / restart, logs, updates and "Pick from ZimaOS apps" (CasaOS App Management API,
//! /v2/app_management). System: CPU power draw and temperature (/v1/sys/utilization), drive health and
//! temperature (/v1/disks). Ported from the Android app (Zima.kt); works the same way.

use crate::layout::{split_url, Service};
use crate::net::http;
use crate::secrets::Secrets;
use crate::settings::Settings;
use regex::Regex;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

pub const NEED_LOGIN: &str = "Enter your ZimaOS login first: Settings > Server > Enter login.";

#[derive(Debug, Clone, PartialEq)]
pub struct ZApp {
    /// the compose app name, e.g. "jellyfin"
    pub id: String,
    pub title: String,
    pub icon: String,
    pub port: String,
    pub scheme: String,
    /// empty = the server's own address
    pub host: String,
    /// "running", "exited" ...
    pub status: String,
    /// the web page path ("" or "/web" ...)
    pub path: String,
    pub app_type: String,
    pub store_id: String,
    pub uncontrolled: bool,
}

impl ZApp {
    pub fn running(&self) -> bool {
        let s = self.status.to_lowercase();
        s.contains("running") || s.contains("healthy")
    }
    /// only compose apps that ZimaOS manages can be started / stopped / updated through the API
    pub fn controllable(&self) -> bool {
        self.app_type == "v2app" && !self.uncontrolled
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ZDrive {
    pub name: String,
    pub model: String,
    pub path: String,
    pub kind: String,
    /// false = the health check failed
    pub health: bool,
    pub temp: Option<f64>,
    pub size: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ZSys {
    pub watts: Option<f64>,
    pub cpu_temp: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ZimaErr {
    pub msg: String,
    /// the login is missing or was refused
    pub auth: bool,
}

impl ZimaErr {
    fn new(m: impl Into<String>) -> Self {
        ZimaErr { msg: m.into(), auth: false }
    }
    fn auth(m: impl Into<String>) -> Self {
        ZimaErr { msg: m.into(), auth: true }
    }
}

type R<T> = Result<T, ZimaErr>;

#[derive(Default)]
struct Cache {
    /// login token per server (renewed after 15 minutes, or when refused)
    tokens: HashMap<String, (String, Instant)>,
    /// last CPU energy reading per server: (time, energy in µJ); watts = energy difference / time difference
    energy: HashMap<String, (Instant, f64)>,
    watts: HashMap<String, f64>,
    temp: HashMap<String, f64>,
}

fn cache() -> &'static Mutex<Cache> {
    static C: OnceLock<Mutex<Cache>> = OnceLock::new();
    C.get_or_init(Mutex::default)
}

pub fn base(s: &Settings) -> String {
    s.platform_base("zimaos")
}

fn key(s: &Settings, id: &str) -> String {
    format!("{id}|{}", base(s))
}

/// Forget the login token of a server (after its login changed).
pub fn forget(id: &str) {
    let p = format!("{id}|");
    cache().lock().unwrap().tokens.retain(|k, _| !k.starts_with(&p));
}

/// Forget the CPU power and temperature readings of a server (another server is on screen).
pub fn forget_sys(id: &str) {
    let p = format!("{id}|");
    let mut c = cache().lock().unwrap();
    c.energy.retain(|k, _| !k.starts_with(&p));
    c.watts.retain(|k, _| !k.starts_with(&p));
    c.temp.retain(|k, _| !k.starts_with(&p));
}

fn enc(id: &str) -> String {
    urlencoding::encode(id).to_string()
}

fn login(s: &Settings, id: &str, secrets: &Secrets) -> R<String> {
    let k = key(s, id);
    if let Some((t, at)) = cache().lock().unwrap().tokens.get(&k) {
        if at.elapsed() < Duration::from_secs(15 * 60) {
            return Ok(t.clone());
        }
    }
    let Some((u, p)) = secrets.saved_login("zimaos", id) else { return Err(ZimaErr::auth(NEED_LOGIN)) };
    let b = base(s);
    let r = http()
        .insecure
        .post(format!("{b}/v1/users/login"))
        .timeout(Duration::from_secs(8))
        .header("Content-Type", "application/json")
        .body(json!({ "username": u, "password": p }).to_string())
        .send()
        .map_err(|_| ZimaErr::new(format!("Can't reach ZimaOS at {b}")))?;
    let code = r.status().as_u16();
    let j: Value = serde_json::from_str(&r.text().unwrap_or_default()).map_err(|_| ZimaErr::new(format!("ZimaOS didn't answer the login (HTTP {code})")))?;
    let t = j.pointer("/data/token/access_token").and_then(|v| v.as_str()).unwrap_or("");
    if t.is_empty() {
        return Err(ZimaErr::auth("ZimaOS refused the saved login. Enter it again in Settings > Server."));
    }
    cache().lock().unwrap().tokens.insert(k, (t.to_string(), Instant::now()));
    Ok(t.to_string())
}

/// One API call with the saved login; an expired token is renewed once.
/// drop_ok: a dropped connection counts as done (an update recreates the app and can cut the answer off): None then.
#[allow(clippy::too_many_arguments)]
pub fn call(s: &Settings, path: &str, method: &str, body: Option<&str>, timeout: f64, id: &str, secrets: &Secrets, drop_ok: bool) -> R<Option<(u16, String)>> {
    for attempt in 0..2 {
        let t = login(s, id, secrets)?;
        let url = format!("{}{path}", base(s));
        let m = reqwest::Method::from_bytes(method.as_bytes()).unwrap_or(reqwest::Method::GET);
        let mut rb = http().insecure.request(m, &url).timeout(Duration::from_secs_f64(timeout)).header("Authorization", &t).header("Content-Type", "application/json");
        if let Some(b) = body {
            rb = rb.body(b.to_string());
        }
        let r = match rb.send() {
            Ok(r) => r,
            Err(_) if drop_ok => return Ok(None),
            Err(_) => return Err(ZimaErr::new(format!("Can't reach ZimaOS at {}", base(s)))),
        };
        let code = r.status().as_u16();
        let text = match r.text() {
            Ok(t) => t,
            Err(_) if drop_ok => return Ok(None),
            Err(_) => String::new(),
        };
        if code == 401 || code == 403 {
            forget(id);
            if attempt == 0 {
                continue;
            }
            return Err(ZimaErr::auth("ZimaOS refused the saved login. Enter it again in Settings > Server."));
        }
        return Ok(Some((code, text)));
    }
    Err(ZimaErr::auth("ZimaOS refused the saved login."))
}

/// The "data" part of ZimaOS's { message, data } answers.
pub fn data(r: Option<(u16, String)>) -> R<Value> {
    let Some((code, body)) = r else { return Ok(Value::Null) };
    let j: Value = match serde_json::from_str(&body) {
        Ok(j) => j,
        Err(_) => {
            if code >= 400 {
                return Err(ZimaErr::new(format!("ZimaOS answered HTTP {code}")));
            }
            return Ok(Value::String(body));
        }
    };
    if let Value::Object(o) = &j {
        if code >= 400 {
            let m = o.get("message").and_then(|m| m.as_str()).filter(|m| !m.is_empty()).map(String::from);
            return Err(ZimaErr::new(m.unwrap_or_else(|| format!("ZimaOS answered HTTP {code}"))));
        }
        return Ok(o.get("data").cloned().unwrap_or(j.clone()));
    }
    if code >= 400 {
        return Err(ZimaErr::new(format!("ZimaOS answered HTTP {code}")));
    }
    Ok(j)
}

/// Titles are { en_us: "Jellyfin", ... } or a plain string.
pub fn loc_text(x: Option<&Value>) -> String {
    match x {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Object(o)) => {
            for k in ["en_us", "en_US"] {
                if let Some(s) = o.get(k).and_then(|v| v.as_str()).filter(|s| !s.is_empty()) {
                    return s.to_string();
                }
            }
            o.values().filter_map(|v| v.as_str()).find(|s| !s.is_empty()).unwrap_or("").to_string()
        }
        _ => String::new(),
    }
}

fn st(o: &Value, k: &str) -> String {
    match o.get(k) {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        _ => String::new(),
    }
}

pub fn parse_app(o: &Value) -> Option<ZApp> {
    let mut id = st(o, "name");
    if id.is_empty() {
        id = st(o, "store_app_id");
    }
    if id.is_empty() {
        return None;
    }
    let mut title = loc_text(o.get("title"));
    if title.is_empty() {
        let mut c = id.chars();
        title = c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default();
    }
    let port: String = st(o, "port").chars().filter(|c| c.is_ascii_digit()).take(5).collect();
    let mut path = st(o, "index").trim().to_string();
    if !path.is_empty() && !path.starts_with('/') {
        path = format!("/{path}");
    }
    if path == "/" {
        path.clear();
    }
    Some(ZApp {
        title,
        icon: st(o, "icon"),
        port,
        scheme: if st(o, "scheme").eq_ignore_ascii_case("https") { "https".into() } else { "http".into() },
        host: st(o, "hostname").trim().to_string(),
        status: st(o, "status").to_lowercase(),
        path,
        app_type: st(o, "app_type"),
        store_id: st(o, "store_app_id"),
        uncontrolled: o.get("is_uncontrolled").and_then(|v| v.as_bool()).unwrap_or(false),
        id,
    })
}

/// The installed apps.
pub fn apps(s: &Settings, id: &str, secrets: &Secrets) -> R<Vec<ZApp>> {
    let d = data(call(s, "/v2/app_management/web/appgrid", "GET", None, 10.0, id, secrets, false)?)?;
    let arr = match &d {
        Value::Array(a) => a.clone(),
        Value::Object(o) => o.get("list").and_then(|l| l.as_array()).cloned().unwrap_or_default(),
        _ => vec![],
    };
    let mut seen = HashSet::new();
    Ok(arr.iter().filter_map(parse_app).filter(|a| seen.insert(a.id.clone())).collect())
}

/// action: "start", "stop" or "restart"
pub fn set_status(s: &Settings, app: &str, action: &str, id: &str, secrets: &Secrets) -> R<()> {
    let body = Value::String(action.into()).to_string();
    data(call(s, &format!("/v2/app_management/compose/{}/status", enc(app)), "PUT", Some(&body), 60.0, id, secrets, false)?)?;
    Ok(())
}

/// The last lines of an app's log (colour codes removed).
pub fn logs(s: &Settings, app: &str, lines: usize, id: &str, secrets: &Secrets) -> R<String> {
    let d = data(call(s, &format!("/v2/app_management/compose/{}/logs?lines={lines}", enc(app)), "GET", None, 20.0, id, secrets, false)?)?;
    let text = match d {
        Value::String(s) => s,
        Value::Null => String::new(),
        v => v.to_string(),
    };
    static ANSI: OnceLock<Regex> = OnceLock::new();
    let ansi = ANSI.get_or_init(|| Regex::new(r"\x1B\[[0-9;?]*[ -/]*[@-~]").unwrap());
    let clean = ansi.replace_all(&text, "").replace('\r', "");
    let all: Vec<&str> = clean.trim_end().lines().collect();
    Ok(all[all.len().saturating_sub(lines)..].join("\n"))
}

/// Store ids (lowercase) of the apps the App Store has a newer version of (ones already updating are left out).
pub fn updates(s: &Settings, id: &str, secrets: &Secrets) -> R<HashSet<String>> {
    let d = data(call(s, "/v2/app_management/apps/upgradable", "GET", None, 10.0, id, secrets, false)?)?;
    Ok(d.as_array()
        .map(|a| {
            a.iter()
                .filter(|x| st(x, "status") != "updating")
                .map(|x| st(x, "store_app_id").to_lowercase())
                .filter(|x| !x.is_empty())
                .collect()
        })
        .unwrap_or_default())
}

/// Pulls the app's newest images from the App Store (ZimaOS carries on in the background).
pub fn update(s: &Settings, app: &str, id: &str, secrets: &Secrets) -> R<()> {
    data(call(s, &format!("/v2/app_management/compose/{}", enc(app)), "PATCH", Some(""), 300.0, id, secrets, true)?)?;
    Ok(())
}

fn num(x: Option<&Value>) -> Option<f64> {
    match x {
        Some(Value::Number(n)) => n.as_f64(),
        Some(Value::String(s)) => s.trim().parse().ok(),
        _ => None,
    }
}

/// CPU power: ZimaOS gives the CPU's energy counter (Intel RAPL, µJ); two readings make watts, the same
/// figure ZimaOS shows. Also its CPU temperature.
pub fn sys(s: &Settings, id: &str, secrets: &Secrets) -> R<ZSys> {
    let k = key(s, id);
    let d = data(call(s, "/v1/sys/utilization", "GET", None, 6.0, id, secrets, false)?)?;
    let mut c = cache().lock().unwrap();
    if let Some(cpu) = d.get("cpu") {
        if let Some(t) = num(cpu.get("temperature")).filter(|t| *t > 0.0 && *t < 150.0) {
            c.temp.insert(k.clone(), t);
        }
        let pw = cpu.get("power");
        let v = match pw {
            Some(o @ Value::Object(_)) => num(o.get("value")),
            other => num(other),
        };
        if let Some(v) = v.filter(|v| *v > 0.0) {
            let now = Instant::now();
            if let Some((t0, e0)) = c.energy.get(&k).copied() {
                let dt = now.duration_since(t0).as_secs_f64();
                if v >= e0 && dt > 0.5 {
                    let w = (v - e0) / 1e6 / dt;
                    if w < 2000.0 {
                        c.watts.insert(k.clone(), w);
                    }
                }
            }
            c.energy.insert(k.clone(), (now, v));
        }
    }
    Ok(ZSys { watts: c.watts.get(&k).copied(), cpu_temp: c.temp.get(&k).copied() })
}

/// Drives with ZimaOS's health check (smartctl) and temperature.
pub fn drives(s: &Settings, id: &str, secrets: &Secrets) -> R<Vec<ZDrive>> {
    let d = data(call(s, "/v1/disks", "GET", None, 10.0, id, secrets, false)?)?;
    Ok(parse_drives(&d))
}

pub fn parse_drives(d: &Value) -> Vec<ZDrive> {
    let arr = match d {
        Value::Array(a) => a.clone(),
        Value::Object(o) => o.get("disks").or(o.get("list")).and_then(|v| v.as_array()).cloned().unwrap_or_default(),
        _ => vec![],
    };
    arr.iter()
        .map(|o| {
            // ZimaOS sends "true" / "false" (smartctl's verdict); only an explicit failure counts as FAILING
            let health = match o.get("health") {
                Some(Value::Bool(false)) => false,
                Some(Value::String(h)) => !["false", "failed", "failing", "bad"].contains(&h.to_lowercase().as_str()),
                _ => true,
            };
            let path = st(o, "path");
            let mut name = st(o, "name");
            if name.is_empty() {
                name = path.rsplit('/').next().unwrap_or("").to_string();
            }
            ZDrive {
                name,
                model: st(o, "model").trim().to_string(),
                path,
                kind: st(o, "disk_type"),
                health,
                temp: num(o.get("temperature")).filter(|t| *t > 0.0 && *t < 150.0),
                size: num(o.get("size")).unwrap_or(0.0),
            }
        })
        .filter(|d| !d.name.is_empty() || !d.path.is_empty())
        .collect()
}

/// Which ZimaOS app a button points at: the app it was added from, else the only app on that port of the server.
pub fn app_for<'a>(sv: &Service, apps: &'a [ZApp], server_host: &str) -> Option<&'a ZApp> {
    if !sv.zima.is_empty() {
        return apps.iter().find(|a| a.id == sv.zima);
    }
    let u = split_url(&sv.url);
    if !u.host.eq_ignore_ascii_case(server_host) {
        return None;
    }
    let port = if u.port.is_empty() { if u.scheme == "https" { "443".to_string() } else { "80".to_string() } } else { u.port };
    let mut on = apps.iter().filter(|a| a.port == port);
    match (on.next(), on.next()) {
        (Some(a), None) => Some(a),
        _ => None,
    }
}

/// The address of an app's web page (the usual port for http / https is left out).
pub fn app_url(a: &ZApp, server_host: &str) -> String {
    let host = if !a.host.is_empty() {
        a.host.clone()
    } else if !server_host.is_empty() {
        server_host.to_string()
    } else {
        "localhost".into()
    };
    let usual = if a.scheme == "https" { "443" } else { "80" };
    let port = if !a.port.is_empty() && a.port != usual { format!(":{}", a.port) } else { String::new() };
    format!("{}://{host}{port}{}", a.scheme, a.path)
}

/// A mounted drive (e.g. /dev/sda1, /dev/nvme0n1p2) to the ZimaOS drive it's on (/dev/sda, /dev/nvme0n1).
pub fn drive_for<'a>(device: &str, drives: &'a [ZDrive]) -> Option<&'a ZDrive> {
    let dev = device.rsplit('/').next().unwrap_or("");
    if dev.is_empty() {
        return None;
    }
    drives
        .iter()
        .filter(|d| {
            let n = d.path.rsplit('/').next().unwrap_or("");
            !n.is_empty() && dev.starts_with(n)
        })
        .max_by_key(|d| d.path.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secrets::Memory;
    use crate::settings::clean_settings;

    fn svc(url: &str, zima: &str) -> Service {
        Service {
            id: "c:1".into(), name: "X".into(), port: String::new(), icon: String::new(), logo: String::new(), url: url.into(),
            scheme: "http".into(), addr: String::new(), label: String::new(), on_widget: true, group: None, zima: zima.into(),
        }
    }

    fn app(id: &str, port: &str, status: &str) -> ZApp {
        ZApp { id: id.into(), title: id.into(), icon: String::new(), port: port.into(), scheme: "http".into(), host: String::new(), status: status.into(), path: String::new(), app_type: "v2app".into(), store_id: id.into(), uncontrolled: false }
    }

    #[test]
    fn parses_grid_items() {
        let a = parse_app(&json!({"name":"jellyfin","title":{"en_us":"Jellyfin"},"port":8096,"scheme":"http","index":"web","status":"running","app_type":"v2app","icon":"https://x/j.png"})).unwrap();
        assert_eq!(a.title, "Jellyfin");
        assert_eq!(a.port, "8096");
        assert_eq!(a.path, "/web");
        assert!(a.running());
        assert!(!parse_app(&json!({"name":"x","app_type":"link"})).unwrap().controllable());
        let b = parse_app(&json!({"name":"x","app_type":"v2app","status":"Up (healthy)","is_uncontrolled":true})).unwrap();
        assert!(b.running() && !b.controllable());
        assert_eq!(parse_app(&json!({"store_app_id":"immich","title":"Photos"})).unwrap().id, "immich");
        assert_eq!(parse_app(&json!({"name":"adguard"})).unwrap().title, "Adguard");
        assert!(parse_app(&json!({})).is_none());
    }

    #[test]
    fn links_by_port_or_id() {
        let apps = vec![app("jellyfin", "8096", "running"), app("immich", "2283", "exited")];
        assert_eq!(app_for(&svc("http://192.168.1.10:8096", ""), &apps, "192.168.1.10").unwrap().id, "jellyfin");
        assert!(app_for(&svc("http://other.host:8096", ""), &apps, "192.168.1.10").is_none());
        assert!(app_for(&svc("http://192.168.1.10:9999", ""), &apps, "192.168.1.10").is_none());
        assert_eq!(app_for(&svc("https://photos.example.com", "immich"), &apps, "192.168.1.10").unwrap().id, "immich");
        // two apps on one port: not linked; no port = the usual one for http / https
        let mut two = apps.clone();
        two.push(app("other", "8096", "running"));
        assert!(app_for(&svc("http://192.168.1.10:8096", ""), &two, "192.168.1.10").is_none());
        let web = vec![app("web", "80", "running")];
        assert_eq!(app_for(&svc("http://192.168.1.10", ""), &web, "192.168.1.10").unwrap().id, "web");
        let mut w = web[0].clone();
        w.path = "/web".into();
        assert_eq!(app_url(&w, "192.168.1.10"), "http://192.168.1.10/web");
        w.port = "8080".into();
        w.host = "nas.lan".into();
        assert_eq!(app_url(&w, "192.168.1.10"), "http://nas.lan:8080/web");
    }

    #[test]
    fn drives_and_health() {
        let d = parse_drives(&json!([
            {"name":"sda","path":"/dev/sda","disk_type":"HDD","health":"true","temperature":35,"model":" WD Red ","size":4e12},
            {"path":"/dev/nvme0n1","disk_type":"SSD","health":"failed","temperature":"41"},
            {"health":true}
        ]));
        assert_eq!(d.len(), 2);
        assert!(d[0].health && d[0].model == "WD Red" && d[0].temp == Some(35.0));
        assert!(!d[1].health && d[1].name == "nvme0n1" && d[1].temp == Some(41.0));
        assert_eq!(drive_for("/dev/sda1", &d).unwrap().name, "sda");
        assert_eq!(drive_for("/dev/nvme0n1p2", &d).unwrap().name, "nvme0n1");
        assert!(drive_for("/dev/sdb1", &d).is_none());
        assert_eq!(parse_drives(&json!({"disks":[{"name":"sdb"}]})).len(), 1);
    }

    #[test]
    fn titles_and_data() {
        assert_eq!(loc_text(Some(&json!({"de_de":"X","en_US":"Y"}))), "Y");
        assert_eq!(loc_text(Some(&json!({"de_de":"X"}))), "X");
        assert_eq!(data(Some((200, r#"{"message":"ok","data":[1]}"#.into()))).unwrap(), json!([1]));
        assert_eq!(data(Some((200, "plain text".into()))).unwrap(), json!("plain text"));
        assert_eq!(data(Some((500, r#"{"message":"boom"}"#.into()))).unwrap_err().msg, "boom");
        assert_eq!(data(None).unwrap(), Value::Null);
    }

    #[test]
    fn against_a_fake_zimaos() {
        use std::sync::{Arc, Mutex as M};
        use tiny_http::{Response, Server};
        let srv = Server::http("127.0.0.1:0").unwrap();
        let port = srv.server_addr().to_ip().unwrap().port();
        let log: Arc<M<Vec<(String, String, String)>>> = Arc::default();
        let l2 = log.clone();
        let energy = Arc::new(M::new(0.0f64));
        std::thread::spawn(move || {
            let mut logins = 0;
            for mut rq in srv.incoming_requests() {
                let mut body = String::new();
                let _ = rq.as_reader().read_to_string(&mut body);
                let auth = rq.headers().iter().find(|h| h.field.equiv("Authorization")).map(|h| h.value.to_string()).unwrap_or_default();
                let url = rq.url().to_string();
                l2.lock().unwrap().push((rq.method().to_string(), url.clone(), body.clone()));
                let (code, out) = match url.as_str() {
                    "/v1/users/login" if body.contains("\"good\"") => {
                        logins += 1;
                        (200, format!(r#"{{"data":{{"token":{{"access_token":"T{logins}"}}}}}}"#))
                    }
                    "/v1/users/login" => (200, r#"{"message":"bad"}"#.to_string()),
                    // the first token expires
                    _ if auth == "T1" => (401, "{}".into()),
                    "/v2/app_management/web/appgrid" => (200, r#"{"data":[{"name":"jellyfin","port":"8096","status":"running","app_type":"v2app"},{"name":"jellyfin"},{"name":"immich","port":2283,"status":"exited","app_type":"v2app"}]}"#.into()),
                    "/v2/app_management/apps/upgradable" => (200, r#"{"data":[{"store_app_id":"Jellyfin","status":"idle"},{"store_app_id":"immich","status":"updating"}]}"#.into()),
                    "/v2/app_management/compose/jellyfin/status" => (200, r#"{"message":"ok"}"#.into()),
                    "/v2/app_management/compose/jellyfin/logs?lines=2" => (200, "{\"data\":\"one\\n\\u001b[32mtwo\\u001b[0m\\r\\nthree\\n\"}".into()),
                    "/v1/sys/utilization" => {
                        let mut e = energy.lock().unwrap();
                        *e += 15e6;
                        (200, format!(r#"{{"data":{{"cpu":{{"temperature":52,"power":{{"value":"{}"}}}}}}}}"#, *e))
                    }
                    "/v1/disks" => (200, r#"{"data":[{"name":"sda","path":"/dev/sda","health":"true","temperature":33}]}"#.into()),
                    _ => (404, "{}".into()),
                };
                let _ = rq.respond(Response::from_string(out).with_status_code(code));
            }
        });
        let s = clean_settings(&json!({ "serverHost": "127.0.0.1", "serverPort": port.to_string(), "platform": "zimaos" }));
        let id = format!("sz{port}");
        let sec = Secrets::new(Box::<Memory>::default(), "file");
        assert_eq!(apps(&s, &id, &sec).unwrap_err().msg, NEED_LOGIN);
        sec.save_login("zimaos", &id, "u", "bad").unwrap();
        assert!(apps(&s, &id, &sec).unwrap_err().auth);
        sec.save_login("zimaos", &id, "u", "good").unwrap();
        // the first token is refused, so it logs in again once
        let list = apps(&s, &id, &sec).unwrap();
        assert_eq!(list.iter().map(|a| a.id.as_str()).collect::<Vec<_>>(), vec!["jellyfin", "immich"]);
        assert_eq!(updates(&s, &id, &sec).unwrap(), ["jellyfin".to_string()].into_iter().collect());
        set_status(&s, "jellyfin", "stop", &id, &sec).unwrap();
        assert_eq!(log.lock().unwrap().iter().rev().find(|x| x.1.ends_with("/status")).unwrap().2, "\"stop\"");
        assert_eq!(logs(&s, "jellyfin", 2, &id, &sec).unwrap(), "two\nthree");
        let first = sys(&s, &id, &sec).unwrap();
        assert_eq!((first.watts, first.cpu_temp), (None, Some(52.0)), "watts need two readings");
        std::thread::sleep(Duration::from_millis(600));
        let w = sys(&s, &id, &sec).unwrap().watts.unwrap();
        assert!(w > 5.0 && w < 30.0, "{w}");
        forget_sys(&id);
        assert_eq!(sys(&s, &id, &sec).unwrap().watts, None);
        assert_eq!(drives(&s, &id, &sec).unwrap()[0].temp, Some(33.0));
        assert!(set_status(&s, "nope", "start", &id, &sec).is_err());
    }
}
