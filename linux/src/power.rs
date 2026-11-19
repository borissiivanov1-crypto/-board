//! Reboot / Shut down. Every server type has a driver: run(action, user, pass, base, settings).
//! action Some(Restart | Off) sends the command; None only checks the login (Test connection, Enter login).
//!
//! ZimaOS:   log in (POST /v1/users/login), then PUT /v1/sys/state/restart or /off
//! Proxmox:  log in (POST /api2/json/access/ticket), then POST /api2/json/nodes/<node>/status command=reboot|shutdown
//! TrueNAS:  JSON-RPC over WebSocket (/api/current: auth.login, then system.reboot / system.shutdown);
//!           older versions without it: the REST API (/api/v2.0/system/reboot or /shutdown)
//! Unraid:   log in (POST /login), read the page's csrf_token, then POST /webGui/include/Boot.php cmd=reboot|shutdown
//! Synology: DSM Web API: SYNO.API.Auth login, then SYNO.Core.System reboot / shutdown
//! OpenMediaVault: rpc.php: Session.login, then System.reboot / System.shutdown
//! Linux:    the small "board-power" helper (set up from Settings), POST /reboot or /shutdown with its secret key

use crate::config::{self, HELPER_PORT};
use crate::net::{basic_auth, cookies_of, form_body, http, send};
use crate::secrets::Secrets;
use crate::settings::{host_of, Settings};
use regex::Regex;
use reqwest::blocking::RequestBuilder;
use serde_json::{json, Value};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Restart,
    Off,
}

impl Action {
    pub fn parse(s: &str) -> Action {
        if s == "off" {
            Action::Off
        } else {
            Action::Restart
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct PowerResult {
    pub ok: bool,
    /// the login was refused (ask for it again)
    pub auth: bool,
    /// the server couldn't be reached
    pub net: bool,
    pub msg: String,
}

impl PowerResult {
    fn ok(msg: impl Into<String>) -> Self {
        PowerResult { ok: true, msg: msg.into(), ..Default::default() }
    }
    fn fail(msg: impl Into<String>) -> Self {
        PowerResult { msg: msg.into(), ..Default::default() }
    }
    fn auth(msg: impl Into<String>) -> Self {
        PowerResult { auth: true, msg: msg.into(), ..Default::default() }
    }
    fn net(msg: impl Into<String>) -> Self {
        PowerResult { net: true, msg: msg.into(), ..Default::default() }
    }
}

pub fn power_sent(a: Action) -> String {
    match a {
        Action::Off => "Shutdown sent. Switch the server on by hand to bring it back.".into(),
        Action::Restart => "Reboot sent. The server should be back in a minute or two.".into(),
    }
}

const T8: Duration = Duration::from_secs(8);

/// After the command: a dropped connection is normal (the server is going down).
fn send_command(req: RequestBuilder, a: Action, name: &str) -> PowerResult {
    match send(req) {
        Err(_) => PowerResult::ok(power_sent(a)),
        Ok((401, _, _)) => PowerResult::auth(format!("{name} refused the login. Enter it again.")),
        Ok((code, _, _)) if code >= 300 => PowerResult::fail(format!("{name} didn't accept the command (HTTP {code}).")),
        Ok(_) => PowerResult::ok(power_sent(a)),
    }
}

fn json_of(body: &str) -> Option<Value> {
    serde_json::from_str(body).ok()
}

// --- ZimaOS ---
fn zima_run(a: Option<Action>, user: &str, pass: &str, base: &str) -> PowerResult {
    let login = http().strict.post(format!("{base}/v1/users/login")).timeout(T8).header("Content-Type", "application/json").body(json!({ "username": user, "password": pass }).to_string());
    let r = match send(login).ok().and_then(|(_, _, b)| json_of(&b)) {
        Some(r) => r,
        None => return PowerResult::net(format!("Can't reach ZimaOS at {base}")),
    };
    let token = r.pointer("/data/token/access_token").and_then(|t| t.as_str()).filter(|t| !t.is_empty());
    let Some(token) = token else {
        let m = r.get("message").and_then(|m| m.as_str()).filter(|m| !m.is_empty()).unwrap_or("wrong username or password");
        return PowerResult::auth(format!("Login failed: {m}"));
    };
    let Some(a) = a else { return PowerResult::ok("saved login works") };
    let req = http().strict.put(format!("{base}/v1/sys/state/{}", if a == Action::Off { "off" } else { "restart" })).timeout(T8).header("Authorization", token);
    let r = send_command(req, a, "ZimaOS");
    if r.auth || r.msg.contains("HTTP 403") {
        return PowerResult::auth("ZimaOS refused the login. Enter it again.");
    }
    r
}

// --- Proxmox VE ---
/// "root" -> "root@pam" (the Linux login realm Proxmox uses by default)
pub fn pve_user(u: &str) -> String {
    let u = u.trim();
    if !u.is_empty() && !u.contains('@') {
        format!("{u}@pam")
    } else {
        u.to_string()
    }
}

struct PveAuth {
    ticket: String,
    csrf: String,
}

fn pve_req(b: RequestBuilder, auth: Option<&PveAuth>) -> RequestBuilder {
    let b = b.timeout(T8);
    match auth {
        Some(a) => b.header("Cookie", format!("PVEAuthCookie={}", a.ticket)).header("CSRFPreventionToken", &a.csrf),
        None => b,
    }
}

fn pve_login(user: &str, pass: &str, base: &str) -> Result<PveAuth, PowerResult> {
    let req = pve_req(http().insecure.post(format!("{base}/api2/json/access/ticket")), None)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(form_body(&[("username", &pve_user(user)), ("password", pass)]));
    let r = match send(req).ok().and_then(|(_, _, b)| json_of(&b)) {
        Some(r) => r,
        None => return Err(PowerResult::net(format!("Can't reach Proxmox at {base}"))),
    };
    let d = r.get("data").filter(|d| d.is_object());
    if d.and_then(|d| d.get("NeedTFA")).map(crate::store::truthy).unwrap_or(false) {
        return Err(PowerResult::auth("This account uses two-factor login. Use an account without it (e.g. a separate user with the Sys.PowerMgmt permission)."));
    }
    match d.and_then(|d| d.get("ticket")).and_then(|t| t.as_str()) {
        Some(t) if !t.is_empty() => Ok(PveAuth { ticket: t.into(), csrf: d.unwrap().get("CSRFPreventionToken").and_then(|c| c.as_str()).unwrap_or("").into() }),
        _ => Err(PowerResult::auth("Login failed: wrong username or password (e.g. root@pam)")),
    }
}

/// The node to reboot: the one named in Settings, the only one, or the one at the server address.
fn pve_node(base: &str, auth: &PveAuth, s: &Settings) -> Result<String, String> {
    if !s.proxmox_node.is_empty() {
        return Ok(s.proxmox_node.clone());
    }
    let r = send(pve_req(http().insecure.get(format!("{base}/api2/json/nodes")), Some(auth))).ok().and_then(|(_, _, b)| json_of(&b));
    let Some(r) = r else { return Err("Couldn't list the Proxmox nodes".into()) };
    let nodes: Vec<String> = r.get("data").and_then(|d| d.as_array()).map(|a| a.iter().filter_map(|n| n.get("node").and_then(|x| x.as_str())).filter(|x| !x.is_empty()).map(String::from).collect()).unwrap_or_default();
    if nodes.len() == 1 {
        return Ok(nodes[0].clone());
    }
    if nodes.is_empty() {
        return Err("Proxmox didn't list any nodes. Enter the node name in Settings.".into());
    }
    let host = s.server_host().to_lowercase();
    let mut hit = nodes.iter().find(|n| n.to_lowercase() == host || host.starts_with(&format!("{}.", n.to_lowercase()))).cloned();
    if hit.is_none() {
        if let Some(r) = send(pve_req(http().insecure.get(format!("{base}/api2/json/cluster/status")), Some(auth))).ok().and_then(|(_, _, b)| json_of(&b)) {
            hit = r.get("data").and_then(|d| d.as_array()).and_then(|a| a.iter().find(|x| x["type"] == "node" && x["ip"] == host.as_str())).and_then(|m| m["name"].as_str()).map(String::from);
        }
    }
    hit.ok_or_else(|| format!("Several Proxmox nodes found ({}). Enter the node name in Settings.", nodes.join(", ")))
}

fn pve_run(a: Option<Action>, user: &str, pass: &str, base: &str, s: &Settings) -> PowerResult {
    let lg = match pve_login(user, pass, base) {
        Ok(l) => l,
        Err(e) => return e,
    };
    let node = match pve_node(base, &lg, s) {
        Ok(n) => n,
        Err(e) => return PowerResult::fail(e),
    };
    let Some(a) = a else { return PowerResult::ok(format!("saved login works (node {node})")) };
    let req = pve_req(http().insecure.post(format!("{base}/api2/json/nodes/{}/status", urlencoding::encode(&node))), Some(&lg))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(format!("command={}", if a == Action::Off { "shutdown" } else { "reboot" }));
    let r = send_command(req, a, "Proxmox");
    if r.msg.contains("HTTP 403") {
        return PowerResult::fail(format!("This Proxmox user isn't allowed to power off node \"{node}\" (needs Sys.PowerMgmt)."));
    }
    r
}

// --- TrueNAS (SCALE / Community Edition, and CORE) ---

#[derive(Debug, Default)]
struct WsOutcome {
    ok: bool,
    auth: bool,
    timeout: bool,
    closed: bool,
    error: Option<String>,
    /// "connect", "login" or "call"
    stage: &'static str,
    results: Vec<Value>,
}

/// TrueNAS 25.04 and newer: JSON-RPC 2.0 over a WebSocket. calls: (method, params, fallback params),
/// run one after another after logging in.
fn truenas_ws(base: &str, user: &str, pass: &str, calls: &[(&str, Value, Option<Value>)]) -> WsOutcome {
    use std::net::{TcpStream, ToSocketAddrs};
    use tungstenite::{Message, WebSocket};
    let deadline = Instant::now() + Duration::from_secs(12);
    let mut out = WsOutcome { stage: "connect", ..Default::default() };
    let url = Regex::new(r"(?i)^http").unwrap().replace(base, "ws").to_string() + "/api/current";
    let parsed = match reqwest::Url::parse(&url) {
        Ok(u) => u,
        Err(e) => {
            out.error = Some(e.to_string());
            return out;
        }
    };
    let host = parsed.host_str().unwrap_or("").to_string();
    let port = parsed.port_or_known_default().unwrap_or(80);
    let addr = match (host.as_str(), port).to_socket_addrs().ok().and_then(|mut a| a.next()) {
        Some(a) => a,
        None => {
            out.error = Some("connection".into());
            return out;
        }
    };
    let stream = match TcpStream::connect_timeout(&addr, Duration::from_secs(8)) {
        Ok(s) => s,
        Err(_) => {
            out.error = Some("connection".into());
            return out;
        }
    };
    let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
    let ws = tungstenite::client_tls_with_config(url.as_str(), stream, None, None);
    let mut ws: WebSocket<_> = match ws {
        Ok((w, _)) => w,
        Err(e) => {
            out.error = Some(format!("connection ({e})"));
            return out;
        }
    };
    let mut id = 0u64;
    // send a call and wait for its answer
    let mut call = |ws: &mut WebSocket<_>, method: &str, params: &Value| -> Result<Value, &'static str> {
        id += 1;
        let msg = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }).to_string();
        if ws.send(Message::text(msg)).is_err() {
            return Err("closed");
        }
        loop {
            if Instant::now() > deadline {
                return Err("timeout");
            }
            match ws.read() {
                Ok(Message::Text(t)) => {
                    if let Ok(m) = serde_json::from_str::<Value>(t.as_str()) {
                        if m["id"] == json!(id) {
                            return Ok(m);
                        }
                    }
                }
                Ok(Message::Close(_)) => return Err("closed"),
                Ok(_) => {}
                Err(tungstenite::Error::Io(e)) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {}
                Err(_) => return Err("closed"),
            }
        }
    };
    let err_msg = |e: &Value| -> String {
        e.get("message").and_then(|m| m.as_str()).or_else(|| e.pointer("/data/reason").and_then(|m| m.as_str())).unwrap_or("error").to_string()
    };
    out.stage = "login";
    match call(&mut ws, "auth.login", &json!([user, pass])) {
        Err(k) => {
            out.timeout = k == "timeout";
            out.closed = k == "closed";
            return out;
        }
        Ok(m) => {
            if let Some(e) = m.get("error").filter(|e| !e.is_null()) {
                out.error = Some(err_msg(e));
                return out;
            }
            if m.get("result") != Some(&json!(true)) {
                out.auth = true;
                return out;
            }
        }
    }
    out.stage = "call";
    for (method, params, fallback) in calls {
        let mut r = call(&mut ws, method, params);
        if let (Ok(m), Some(fb)) = (&r, fallback) {
            if m.get("error").map(|e| !e.is_null()).unwrap_or(false) {
                r = call(&mut ws, method, fb);
            }
        }
        match r {
            Err(k) => {
                out.timeout = k == "timeout";
                out.closed = k == "closed";
                return out;
            }
            Ok(m) => {
                if let Some(e) = m.get("error").filter(|e| !e.is_null()) {
                    out.error = Some(err_msg(e));
                    return out;
                }
                out.results.push(m.get("result").cloned().unwrap_or(Value::Null));
            }
        }
    }
    let _ = ws.close(None);
    out.ok = true;
    out
}

fn truenas_run(a: Option<Action>, user: &str, pass: &str, base: &str) -> PowerResult {
    let method = if a == Some(Action::Off) { "system.shutdown" } else { "system.reboot" };
    // 25.10+ needs a reason; earlier JSON-RPC versions take only options, hence the fallback
    let calls: Vec<(&str, Value, Option<Value>)> = match a {
        Some(_) => vec![(method, json!(["Requested from the -board app"]), Some(json!([])))],
        None => vec![("system.info", json!([]), None)],
    };
    let w = truenas_ws(base, user, pass, &calls);
    let version = |v: Option<&Value>| v.and_then(|i| i.get("version")).and_then(|x| x.as_str()).map(|x| format!(" ({x})")).unwrap_or_default();
    if w.ok {
        return match a {
            Some(a) => PowerResult::ok(power_sent(a)),
            None => PowerResult::ok(format!("saved login works{}", version(w.results.first()))),
        };
    }
    if w.auth {
        return PowerResult::auth("TrueNAS refused the login (wrong username or password, or the account uses two-factor login).");
    }
    if let Some(a) = a {
        if w.stage == "call" && (w.closed || w.timeout) {
            return PowerResult::ok(power_sent(a));
        }
    }
    if w.stage != "connect" {
        return PowerResult::fail(format!("TrueNAS: {}", w.error.unwrap_or_else(|| "no answer".into())));
    }
    // No JSON-RPC WebSocket: an older TrueNAS (24.10 and earlier, or CORE) with the REST API
    let auth = basic_auth(user, pass);
    let Some(a) = a else {
        let req = http().insecure.get(format!("{base}/api/v2.0/system/info")).timeout(T8).header("Authorization", &auth);
        return match send(req) {
            Err(_) => PowerResult::net(format!(
                "Can't reach TrueNAS at {base}{}",
                if base.to_lowercase().starts_with("https:") { " (for https it needs a certificate this computer trusts)" } else { "" }
            )),
            Ok((401, _, _)) => PowerResult::auth("TrueNAS refused the login"),
            Ok((200, _, body)) => PowerResult::ok(format!("saved login works{}", version(json_of(&body).as_ref()))),
            Ok((code, _, _)) => PowerResult::fail(format!("TrueNAS answered HTTP {code}")),
        };
    };
    let req = http()
        .insecure
        .post(format!("{base}/api/v2.0/system/{}", if a == Action::Off { "shutdown" } else { "reboot" }))
        .timeout(T8)
        .header("Authorization", &auth)
        .header("Content-Type", "application/json")
        .body("{}");
    send_command(req, a, "TrueNAS")
}

// --- Unraid ---
fn unraid_run(a: Option<Action>, user: &str, pass: &str, base: &str) -> PowerResult {
    // 1. log in; the answer is a redirect that sets the session cookie, so don't follow it
    let user = if user.is_empty() { "root" } else { user };
    let lg = http()
        .no_redirect
        .post(format!("{base}/login"))
        .timeout(T8)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(form_body(&[("username", user), ("password", pass)]));
    let (code, hdr, page) = match send(lg) {
        Ok(x) => x,
        Err(_) => return PowerResult::net(format!("Can't reach Unraid at {base}")),
    };
    let loc = hdr.get("location").and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
    let is_redirect = (300..400).contains(&code);
    if is_redirect && Regex::new(r"(?i)^https?://").unwrap().is_match(&loc) && host_of(&loc) != host_of(base) {
        let to = Regex::new(r"(?i)/login.*$").unwrap().replace(&loc, "").to_string();
        return PowerResult::fail(format!("Unraid sends you on to {to}. Enter that as the Unraid address in Settings."));
    }
    if !is_redirect {
        return if Regex::new(r"(?i)password").unwrap().is_match(&page) {
            PowerResult::auth("Unraid refused the login (wrong username or password).")
        } else {
            PowerResult::fail(format!("Unraid answered HTTP {code}"))
        };
    }
    let cookie = cookies_of(&hdr);
    // 2. the csrf_token every Unraid page carries
    let html = send(http().insecure.get(format!("{base}/Main")).timeout(T8).header("Cookie", &cookie)).map(|x| x.2).unwrap_or_default();
    let m = Regex::new(r#"csrf_token["'\s]*[:=]\s*["']([0-9A-Za-z]{8,})["']"#).unwrap().captures(&html).map(|c| c[1].to_string());
    let Some(token) = m else {
        return if Regex::new(r#"(?i)name=["']?password"#).unwrap().is_match(&html) {
            PowerResult::auth("Unraid refused the login.")
        } else {
            PowerResult::fail("Logged in to Unraid, but couldn't read its security token.")
        };
    };
    let Some(a) = a else { return PowerResult::ok("saved login works") };
    // 3. what Unraid's own Reboot / Shut down buttons send
    let req = http()
        .insecure
        .post(format!("{base}/webGui/include/Boot.php"))
        .timeout(T8)
        .header("Cookie", &cookie)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(form_body(&[("cmd", if a == Action::Off { "shutdown" } else { "reboot" }), ("csrf_token", &token)]));
    send_command(req, a, "Unraid")
}

// --- Synology DSM ---
fn syno_error(c: i64) -> Option<&'static str> {
    Some(match c {
        400 => "wrong username or password",
        401 => "this account is disabled",
        402 => "this account isn't allowed to log in",
        403 => "this account uses two-factor login. Use an administrator account without it",
        404 => "the two-factor code failed",
        406 => "two-factor login is enforced for this account",
        407 => "too many failed logins: this address is blocked for a while",
        _ => return None,
    })
}

fn syno_api(base: &str, cgi: &str, params: &[(&str, &str)]) -> RequestBuilder {
    http().insecure.post(format!("{base}/webapi/{cgi}")).timeout(T8).header("Content-Type", "application/x-www-form-urlencoded").body(form_body(params))
}

fn synology_run(a: Option<Action>, user: &str, pass: &str, base: &str) -> PowerResult {
    let login = |cgi: &str, version: &str| -> Option<Value> {
        let p = [("api", "SYNO.API.Auth"), ("method", "login"), ("account", user), ("passwd", pass), ("session", "board"), ("format", "sid"), ("version", version)];
        send(syno_api(base, cgi, &p)).ok().and_then(|(_, _, b)| json_of(&b))
    };
    let Some(mut r) = login("entry.cgi", "6") else { return PowerResult::net(format!("Can't reach Synology DSM at {base}")) };
    // DSM 6: the login API lives in auth.cgi
    let code_of = |r: &Value| r.pointer("/error/code").and_then(|c| c.as_i64());
    if r["success"] != json!(true) && matches!(code_of(&r), Some(102..=104)) {
        match login("auth.cgi", "3") {
            Some(x) => r = x,
            None => return PowerResult::net(format!("Can't reach Synology DSM at {base}")),
        }
    }
    let sid = r.pointer("/data/sid").and_then(|s| s.as_str()).filter(|s| !s.is_empty());
    let Some(sid) = sid.filter(|_| r["success"] == json!(true)) else {
        let c = code_of(&r);
        let msg = match c.and_then(syno_error) {
            Some(m) => m.to_string(),
            None => format!("login failed (error {})", c.map(|c| c.to_string()).unwrap_or_else(|| "undefined".into())),
        };
        return PowerResult { auth: matches!(c, Some(400..=404) | Some(406)), msg: format!("Synology: {msg}"), ..Default::default() };
    };
    let Some(a) = a else { return PowerResult::ok("saved login works") };
    let req = syno_api(base, "entry.cgi", &[("api", "SYNO.Core.System"), ("version", "1"), ("method", if a == Action::Off { "shutdown" } else { "reboot" }), ("_sid", sid)]);
    let j = match send(req) {
        Err(_) => return PowerResult::ok(power_sent(a)),
        Ok((_, _, b)) => match json_of(if b.trim().is_empty() { "{}" } else { &b }) {
            Some(j) => j,
            None => return PowerResult::ok(power_sent(a)),
        },
    };
    if j.get("success") == Some(&json!(false)) {
        let c = code_of(&j);
        return PowerResult::fail(if c == Some(105) {
            "This Synology account isn't an administrator.".to_string()
        } else {
            format!("Synology didn't accept the command (error {}).", c.map(|c| c.to_string()).unwrap_or_else(|| "undefined".into()))
        });
    }
    PowerResult::ok(power_sent(a))
}

// --- OpenMediaVault ---
fn omv_rpc(base: &str, body: Value, cookie: &str) -> RequestBuilder {
    let mut r = http().insecure.post(format!("{base}/rpc.php")).timeout(T8).header("Content-Type", "application/json").body(body.to_string());
    if !cookie.is_empty() {
        r = r.header("Cookie", cookie);
    }
    r
}

fn omv_run(a: Option<Action>, user: &str, pass: &str, base: &str) -> PowerResult {
    let lg = omv_rpc(base, json!({ "service": "Session", "method": "login", "params": { "username": user, "password": pass }, "options": null }), "");
    let (hdr, r) = match send(lg) {
        Ok((_, h, b)) => match json_of(if b.trim().is_empty() { "{}" } else { &b }) {
            Some(j) => (h, j),
            None => return PowerResult::net(format!("Can't reach OpenMediaVault at {base}")),
        },
        Err(_) => return PowerResult::net(format!("Can't reach OpenMediaVault at {base}")),
    };
    let resp = r.get("response").cloned().unwrap_or(json!({}));
    if resp["status"] == "challengeRequired" {
        return PowerResult::auth("This OpenMediaVault account uses two-factor login. Use an administrator account without it.");
    }
    if !(resp["authenticated"] == json!(true) || resp["status"] == "authenticated") {
        let m = r.pointer("/error/message").and_then(|m| m.as_str()).filter(|m| !m.is_empty()).unwrap_or("wrong username or password");
        return PowerResult::auth(format!("OpenMediaVault: {m}"));
    }
    let mut cookie = cookies_of(&hdr);
    if cookie.is_empty() {
        if let Some(sid) = resp.get("sessionid").and_then(|s| s.as_str()) {
            cookie = format!("X-OPENMEDIAVAULT-SESSIONID={sid}");
        }
    }
    let Some(a) = a else { return PowerResult::ok("saved login works") };
    // a 1 second delay lets OpenMediaVault answer before it goes down
    let req = omv_rpc(base, json!({ "service": "System", "method": if a == Action::Off { "shutdown" } else { "reboot" }, "params": { "delay": 1 }, "options": null }), &cookie);
    let j = match send(req) {
        Err(_) => return PowerResult::ok(power_sent(a)),
        Ok((_, _, b)) => match json_of(if b.trim().is_empty() { "{}" } else { &b }) {
            Some(j) => j,
            None => return PowerResult::ok(power_sent(a)),
        },
    };
    if let Some(e) = j.get("error").filter(|e| !e.is_null()) {
        let m = e.get("message").and_then(|m| m.as_str()).filter(|m| !m.is_empty()).unwrap_or("error");
        return PowerResult { auth: Regex::new(r"(?i)session|authenticat").unwrap().is_match(m), msg: format!("OpenMediaVault didn't accept the command: {m}"), ..Default::default() };
    }
    PowerResult::ok(power_sent(a))
}

/// Runs the driver of a server type with a login (every type except Linux).
pub fn driver(platform: &str, a: Option<Action>, user: &str, pass: &str, base: &str, s: &Settings) -> Option<PowerResult> {
    Some(match platform {
        "zimaos" => zima_run(a, user, pass, base),
        "proxmox" => pve_run(a, user, pass, base, s),
        "truenas" => truenas_run(a, user, pass, base),
        "unraid" => unraid_run(a, user, pass, base),
        "synology" => synology_run(a, user, pass, base),
        "omv" => omv_run(a, user, pass, base),
        _ => return None,
    })
}

pub fn has_driver(platform: &str) -> bool {
    matches!(platform, "zimaos" | "proxmox" | "truenas" | "unraid" | "synology" | "omv")
}

// --- Linux (Debian, Ubuntu, Fedora, ... any systemd distro) via the board-power helper ---

/// The HTTP status code, or 0 when the helper can't be reached.
pub fn helper_call(base: &str, key: &str, path: &str, post: bool) -> u16 {
    let url = format!("{base}{path}");
    let b = if post { http().strict.post(url) } else { http().strict.get(url) };
    match send(b.timeout(T8).header("Authorization", format!("Bearer {key}"))) {
        Ok((code, _, _)) => code,
        Err(_) => 0,
    }
}

fn helper_power(a: Action, s: &Settings, id: &str, secrets: &Secrets) -> PowerResult {
    let Some(key) = secrets.get(&Secrets::helper_key(id)) else {
        return PowerResult::fail("Set up the power helper first: Settings > Server > Copy setup command.");
    };
    let base = s.platform_base("linux");
    match helper_call(&base, &key, if a == Action::Off { "/shutdown" } else { "/reboot" }, true) {
        0 => PowerResult::fail(format!("Can't reach the power helper at {base}. Is it set up on the server?")),
        401 => PowerResult::fail("The helper refused the key. Copy the setup command from Settings and run it again."),
        c if c >= 300 => PowerResult::fail(format!("The helper didn't accept the command (HTTP {c}).")),
        _ => PowerResult::ok(power_sent(a)),
    }
}

/// The helper: one Python 3 file (standard library only) run by systemd as root.
/// It only answers requests that carry the secret key, and only knows /health, /reboot and /shutdown.
pub fn helper_py() -> String {
    format!(
        r#"#!/usr/bin/env python3
# board-power: lets the -board app reboot or shut down this server.
# Remove it with: systemctl disable --now board-power; rm /usr/local/bin/board-power /etc/board-power.key /etc/systemd/system/board-power.service
import hmac, json, os, socket, subprocess, threading, time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

KEY = open("/etc/board-power.key").read().strip()
PORT = int(os.environ.get("BOARD_POWER_PORT", "{HELPER_PORT}"))
CMDS = {{"/reboot": ["systemctl", "reboot"], "/shutdown": ["systemctl", "poweroff"]}}

class Handler(BaseHTTPRequestHandler):
    def reply(self, code, obj):
        body = json.dumps(obj).encode()
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def authed(self):
        given = self.headers.get("Authorization", "").encode()
        if hmac.compare_digest(given, ("Bearer " + KEY).encode()):
            return True
        time.sleep(1)  # slows down guessing
        self.reply(401, {{"ok": False, "error": "wrong key"}})
        return False

    def do_GET(self):
        if self.path != "/health":
            return self.reply(404, {{"ok": False}})
        if self.authed():
            self.reply(200, {{"ok": True, "host": socket.gethostname()}})

    def do_POST(self):
        cmd = CMDS.get(self.path)
        if not cmd:
            return self.reply(404, {{"ok": False}})
        if self.authed():
            self.reply(200, {{"ok": True}})
            threading.Timer(1.0, subprocess.run, [cmd]).start()

    def log_message(self, *args):
        pass

ThreadingHTTPServer(("", PORT), Handler).serve_forever()
"#
    )
}

/// What "Copy setup command" puts on the clipboard: paste it into an SSH session on the server.
/// It asks for sudo once (or runs straight away when you're root, e.g. on Proxmox).
pub fn helper_setup_script(key: &str, port: u16) -> String {
    let py = helper_py();
    format!(
        r#"$(command -v sudo) bash -c "$(cat <<'BOARD_SETUP'
# -board power helper setup (Debian, Ubuntu, Fedora and other systemd Linux)
set -e
if ! command -v python3 >/dev/null; then
  if command -v apt-get >/dev/null; then apt-get update -qq && DEBIAN_FRONTEND=noninteractive apt-get install -y -qq python3 </dev/null
  elif command -v dnf >/dev/null; then dnf install -y -q python3 </dev/null
  else echo "Please install python3 first, then paste this again."; exit 1; fi
fi
(umask 077; printf '%s\n' '{key}' > /etc/board-power.key)
cat > /usr/local/bin/board-power <<'BOARD_PY'
{py}BOARD_PY
chmod 755 /usr/local/bin/board-power
cat > /etc/systemd/system/board-power.service <<'BOARD_UNIT'
[Unit]
Description=-board power helper (reboot / shut down from the -board app)
After=network-online.target
Wants=network-online.target

[Service]
Environment=BOARD_POWER_PORT={port}
ExecStart=/usr/bin/env python3 /usr/local/bin/board-power
Restart=on-failure

[Install]
WantedBy=multi-user.target
BOARD_UNIT
systemctl daemon-reload
systemctl enable board-power >/dev/null 2>&1
systemctl restart board-power
if command -v ufw >/dev/null && ufw status 2>/dev/null | grep -q "Status: active"; then ufw allow {port}/tcp >/dev/null; fi
if command -v firewall-cmd >/dev/null && firewall-cmd --state >/dev/null 2>&1; then firewall-cmd -q --permanent --add-port={port}/tcp && firewall-cmd -q --reload; fi
echo "-board power helper is running on port {port}. Go back to -board and tap Test connection."
BOARD_SETUP
)"
"#
    )
}

/// A new random helper key (48 hex characters).
pub fn new_helper_key() -> String {
    use rand::RngCore;
    let mut b = [0u8; 24];
    rand::thread_rng().fill_bytes(&mut b);
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// Reboot / Shut down from the dashboard. user / pass only when typed in the popup; otherwise the
/// saved login is used. A refused saved login is forgotten; a typed login that works is saved.
/// Returns the result and whether a login is saved now.
pub fn handle_power(a: Action, typed: Option<(String, String)>, s: &Settings, id: &str, secrets: &Secrets) -> (PowerResult, bool) {
    let p = s.platform.as_str();
    let r = if config::platform(p).is_none() {
        PowerResult::fail("Pick your server type first: Settings > Server > Server type.")
    } else if p == "linux" {
        helper_power(a, s, id, secrets)
    } else {
        let mut saved = false;
        let (user, pass) = match typed.filter(|(u, p)| !u.is_empty() && !p.is_empty()) {
            Some(x) => x,
            None => match secrets.saved_login(p, id) {
                Some(x) => {
                    saved = true;
                    x
                }
                None => (String::new(), String::new()),
            },
        };
        let r = if user.is_empty() || pass.is_empty() {
            PowerResult::auth("")
        } else {
            driver(p, Some(a), &user, &pass, &s.platform_base(p), s).unwrap()
        };
        if r.auth && saved {
            secrets.forget_login(p, id);
        }
        if !r.auth && !r.net && !saved && !user.is_empty() && !pass.is_empty() {
            let _ = secrets.save_login(p, id, &user, &pass);
        }
        r
    };
    let has = secrets.has_login(p, id);
    (r, has)
}

/// One line for "Test connection" about the Reboot / Shut down side of the chosen server type.
pub fn test_power(s: &Settings, id: &str, secrets: &Secrets) -> (bool, String) {
    let p = s.platform.as_str();
    let Some(pl) = config::platform(p) else {
        return (false, "Server type: not picked yet, so Reboot / Shut down is off. Pick it under SERVER TYPE.".into());
    };
    let base = s.platform_base(p);
    if p == "linux" {
        let Some(key) = secrets.get(&Secrets::helper_key(id)) else {
            return (false, "Power helper: not set up yet (tap Copy setup command)".into());
        };
        return match helper_call(&base, &key, "/health", false) {
            200 => (true, format!("Power helper: OK at {base}")),
            401 => (false, "Power helper: answers, but refused the key. Run the setup command again.".into()),
            c => (false, format!("Power helper: no answer at {base}{}", if c != 0 { format!(" (HTTP {c})") } else { String::new() })),
        };
    }
    if let Some((u, pw)) = secrets.saved_login(p, id) {
        let r = driver(p, None, &u, &pw, &base, s).unwrap();
        if r.ok {
            return (true, format!("{}: OK, {}", pl.name, r.msg));
        }
        let why = if r.msg.is_empty() { format!("no answer at {base}") } else { r.msg };
        return (false, format!("{}: {}{}", pl.name, if r.auth { "the saved login was refused. " } else { "" }, why));
    }
    match http().insecure.get(&base).timeout(Duration::from_secs(6)).send() {
        Ok(_) => (true, format!("{}: reachable at {base} (no login saved yet)", pl.name)),
        Err(_) => (false, format!("{}: no answer at {base}", pl.name)),
    }
}

/// Settings > Server > Enter login: check the login against the server, then save it.
pub fn check_and_save_login(platform: &str, user: &str, pass: &str, s: &Settings, id: &str, secrets: &Secrets) -> Result<String, String> {
    if !has_driver(platform) {
        return Err("This server type doesn't use a login.".into());
    }
    let user = user.trim();
    if user.is_empty() || pass.is_empty() {
        return Err("Enter a username and a password.".into());
    }
    let base = s.platform_base(platform);
    let r = driver(platform, None, user, pass, &base, s).unwrap();
    if !r.ok {
        let why = if r.msg.is_empty() { format!("no answer at {base}") } else { r.msg };
        return Err(if r.net { format!("{why}. The login wasn't saved: try again when the server can be reached.") } else { why });
    }
    secrets.save_login(platform, id, user, pass)?;
    let extra = Regex::new(r"\(([^)]*)\)\s*$").unwrap().captures(&r.msg).map(|c| format!(" ({})", &c[1])).unwrap_or_default();
    Ok(format!("{} login checked and saved{extra}", config::platform(platform).unwrap().login.unwrap()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secrets::Memory;
    use crate::settings::clean_settings;
    use std::sync::{Arc, Mutex};
    use tiny_http::{Header, Response, Server};

    type Log = Arc<Mutex<Vec<(String, String, String, String)>>>;

    /// A fake server: handler(method, url, headers-as-text, body) -> (status, headers, body)
    fn fake<F>(handler: F) -> (String, Log)
    where
        F: Fn(&str, &str, &str, &str) -> (u16, Vec<(&'static str, String)>, String) + Send + 'static,
    {
        let srv = Server::http("127.0.0.1:0").unwrap();
        let base = format!("http://{}", srv.server_addr().to_ip().unwrap());
        let log: Log = Arc::default();
        let l2 = log.clone();
        std::thread::spawn(move || {
            for mut rq in srv.incoming_requests() {
                let mut body = String::new();
                let _ = rq.as_reader().read_to_string(&mut body);
                let hdrs = rq.headers().iter().map(|h| format!("{}: {}", h.field, h.value).to_lowercase()).collect::<Vec<_>>().join("\n");
                let (code, hs, out) = handler(rq.method().as_str(), rq.url(), &hdrs, &body);
                l2.lock().unwrap().push((rq.method().to_string(), rq.url().to_string(), hdrs, body));
                let mut resp = Response::from_string(out).with_status_code(code);
                for (k, v) in hs {
                    resp.add_header(Header::from_bytes(k.as_bytes(), v.as_bytes()).unwrap());
                }
                let _ = rq.respond(resp);
            }
        });
        (base, log)
    }

    fn mem() -> Secrets {
        Secrets::new(Box::<Memory>::default(), "file")
    }

    #[test]
    fn zimaos_login_and_command() {
        let (base, log) = fake(|m, u, h, b| match (m, u) {
            ("POST", "/v1/users/login") if b.contains("\"password\":\"good\"") => (200, vec![], r#"{"data":{"token":{"access_token":"T1"}}}"#.into()),
            ("POST", "/v1/users/login") => (200, vec![], r#"{"message":"user or password invalid"}"#.into()),
            ("PUT", "/v1/sys/state/off") if h.contains("authorization: t1") => (200, vec![], "{}".into()),
            _ => (404, vec![], String::new()),
        });
        let s = clean_settings(&json!({}));
        assert_eq!(driver("zimaos", None, "u", "good", &base, &s).unwrap(), PowerResult::ok("saved login works"));
        let bad = driver("zimaos", None, "u", "bad", &base, &s).unwrap();
        assert!(bad.auth && bad.msg == "Login failed: user or password invalid");
        let r = driver("zimaos", Some(Action::Off), "u", "good", &base, &s).unwrap();
        assert_eq!(r, PowerResult::ok(power_sent(Action::Off)));
        assert_eq!(log.lock().unwrap().last().unwrap().1, "/v1/sys/state/off");
        let r = driver("zimaos", None, "u", "good", "http://127.0.0.1:1", &s).unwrap();
        assert!(r.net);
    }

    #[test]
    fn proxmox_finds_the_node() {
        let (base, log) = fake(|m, u, _h, b| match (m, u) {
            ("POST", "/api2/json/access/ticket") if b.contains("username=root%40pam") => (200, vec![], r#"{"data":{"ticket":"TK","CSRFPreventionToken":"CS"}}"#.into()),
            ("POST", "/api2/json/access/ticket") => (200, vec![], r#"{"data":null}"#.into()),
            ("GET", "/api2/json/nodes") => (200, vec![], r#"{"data":[{"node":"pve1"},{"node":"pve2"}]}"#.into()),
            ("GET", "/api2/json/cluster/status") => (200, vec![], r#"{"data":[{"type":"node","name":"pve2","ip":"127.0.0.1"}]}"#.into()),
            ("POST", "/api2/json/nodes/pve2/status") => (200, vec![], r#"{"data":"UPID"}"#.into()),
            _ => (404, vec![], String::new()),
        });
        let s = clean_settings(&json!({ "serverHost": "127.0.0.1", "platform": "proxmox" }));
        let r = driver("proxmox", None, "root", "x", &base, &s).unwrap();
        assert_eq!(r.msg, "saved login works (node pve2)");
        let r = driver("proxmox", Some(Action::Restart), "root", "x", &base, &s).unwrap();
        assert!(r.ok);
        let last = log.lock().unwrap().last().unwrap().clone();
        assert_eq!(last.3, "command=reboot");
        assert!(last.2.contains("pveauthcookie=tk") && last.2.contains("csrfpreventiontoken: cs"));
        assert!(driver("proxmox", None, "bob@pve", "x", &base, &s).unwrap().auth);
        let named = clean_settings(&json!({ "serverHost": "elsewhere", "proxmoxNode": "pve1" }));
        assert_eq!(driver("proxmox", None, "root", "x", &base, &named).unwrap().msg, "saved login works (node pve1)");
        let other = clean_settings(&json!({ "serverHost": "10.9.9.9" }));
        assert!(driver("proxmox", None, "root", "x", &base, &other).unwrap().msg.starts_with("Several Proxmox nodes found (pve1, pve2)"));
    }

    #[test]
    fn synology_errors_and_dsm6() {
        let (base, _log) = fake(|_m, u, _h, b| match u {
            "/webapi/entry.cgi" if b.contains("SYNO.API.Auth") => (200, vec![], r#"{"success":false,"error":{"code":102}}"#.into()),
            "/webapi/auth.cgi" if b.contains("passwd=good") => (200, vec![], r#"{"success":true,"data":{"sid":"S"}}"#.into()),
            "/webapi/auth.cgi" => (200, vec![], r#"{"success":false,"error":{"code":400}}"#.into()),
            "/webapi/entry.cgi" if b.contains("_sid=S") => (200, vec![], r#"{"success":false,"error":{"code":105}}"#.into()),
            _ => (404, vec![], String::new()),
        });
        let s = Settings::default();
        assert!(driver("synology", None, "a", "good", &base, &s).unwrap().ok);
        let r = driver("synology", None, "a", "bad", &base, &s).unwrap();
        assert_eq!((r.auth, r.msg.as_str()), (true, "Synology: wrong username or password"));
        let r = driver("synology", Some(Action::Off), "a", "good", &base, &s).unwrap();
        assert_eq!(r.msg, "This Synology account isn't an administrator.");
    }

    #[test]
    fn openmediavault_session() {
        let (base, log) = fake(|_m, _u, h, b| {
            if b.contains("\"Session\"") {
                if b.contains("\"good\"") {
                    (200, vec![("Set-Cookie", "X-OPENMEDIAVAULT-SESSIONID=abc; path=/".into())], r#"{"response":{"authenticated":true},"error":null}"#.into())
                } else {
                    (200, vec![], r#"{"response":null,"error":{"message":"Incorrect username or password."}}"#.into())
                }
            } else if h.contains("sessionid=abc") {
                (200, vec![], r#"{"response":null,"error":null}"#.into())
            } else {
                (200, vec![], r#"{"error":{"message":"Session not authenticated."}}"#.into())
            }
        });
        let s = Settings::default();
        let r = driver("omv", None, "admin", "bad", &base, &s).unwrap();
        assert_eq!(r.msg, "OpenMediaVault: Incorrect username or password.");
        assert!(driver("omv", Some(Action::Restart), "admin", "good", &base, &s).unwrap().ok);
        assert!(log.lock().unwrap().last().unwrap().3.contains("\"delay\":1"));
    }

    #[test]
    fn unraid_flow() {
        let (base, log) = fake(|m, u, h, b| match (m, u) {
            ("POST", "/login") if b.contains("password=good") => (302, vec![("Location", "/Main".into()), ("Set-Cookie", "unraid_1=sess; path=/".into())], String::new()),
            ("POST", "/login") => (200, vec![], "<input name=password>".into()),
            ("GET", "/Main") if h.contains("unraid_1=sess") => (200, vec![], "var csrf_token = \"ABCDEF0123456789\";".into()),
            ("POST", "/webGui/include/Boot.php") => (200, vec![], String::new()),
            _ => (404, vec![], String::new()),
        });
        let s = Settings::default();
        assert!(driver("unraid", None, "root", "bad", &base, &s).unwrap().auth);
        assert!(driver("unraid", None, "", "good", &base, &s).unwrap().ok);
        assert!(driver("unraid", Some(Action::Off), "root", "good", &base, &s).unwrap().ok);
        assert_eq!(log.lock().unwrap().last().unwrap().3, "cmd=shutdown&csrf_token=ABCDEF0123456789");
        let (redir, _) = fake(|_, _, _, _| (302, vec![("Location", "https://abc.myunraid.net/login".into())], String::new()));
        assert_eq!(driver("unraid", None, "root", "x", &redir, &s).unwrap().msg, "Unraid sends you on to https://abc.myunraid.net. Enter that as the Unraid address in Settings.");
    }

    #[test]
    fn truenas_rest_fallback() {
        // no WebSocket answer at /api/current: the older REST API is used
        let (base, log) = fake(|m, u, h, _b| match (m, u) {
            ("GET", "/api/v2.0/system/info") if h.contains("basic dtpw") => (200, vec![], r#"{"version":"TrueNAS-13.0"}"#.into()),
            ("GET", "/api/v2.0/system/info") => (401, vec![], String::new()),
            ("POST", "/api/v2.0/system/shutdown") => (200, vec![], "null".into()),
            _ => (404, vec![], String::new()),
        });
        let s = Settings::default();
        assert_eq!(driver("truenas", None, "u", "p", &base, &s).unwrap().msg, "saved login works (TrueNAS-13.0)");
        assert!(driver("truenas", None, "u", "x", &base, &s).unwrap().auth);
        assert!(driver("truenas", Some(Action::Off), "u", "p", &base, &s).unwrap().ok);
        assert_eq!(log.lock().unwrap().last().unwrap().1, "/api/v2.0/system/shutdown");
    }

    #[test]
    fn truenas_websocket() {
        // a tiny JSON-RPC WebSocket server
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", l.local_addr().unwrap());
        std::thread::spawn(move || {
            for c in l.incoming().take(2) {
                let mut ws = tungstenite::accept(c.unwrap()).unwrap();
                loop {
                    let Ok(m) = ws.read() else { break };
                    if !m.is_text() {
                        break;
                    }
                    let Ok(t) = m.to_text() else { break };
                    let v: Value = serde_json::from_str(t).unwrap();
                    let res = match v["method"].as_str().unwrap() {
                        "auth.login" => json!({ "jsonrpc": "2.0", "id": v["id"], "result": v["params"][1] == "pw" }),
                        "system.info" => json!({ "jsonrpc": "2.0", "id": v["id"], "result": { "version": "25.04.1" } }),
                        _ => json!({ "jsonrpc": "2.0", "id": v["id"], "result": null }),
                    };
                    if ws.send(tungstenite::Message::text(res.to_string())).is_err() {
                        break;
                    }
                }
            }
        });
        let s = Settings::default();
        assert_eq!(driver("truenas", None, "u", "pw", &base, &s).unwrap().msg, "saved login works (25.04.1)");
        assert!(driver("truenas", None, "u", "nope", &base, &s).unwrap().auth);
    }

    #[test]
    fn linux_helper_and_handle_power() {
        let (base, _log) = fake(|m, u, h, _b| {
            if !h.contains("bearer k1") {
                return (401, vec![], String::new());
            }
            match (m, u) {
                ("GET", "/health") => (200, vec![], "{}".into()),
                ("POST", "/reboot") => (200, vec![], "{}".into()),
                _ => (404, vec![], String::new()),
            }
        });
        let port = base.rsplit(':').next().unwrap().to_string();
        let s = clean_settings(&json!({ "serverHost": "127.0.0.1", "serverPort": port, "platform": "linux" }));
        let sec = mem();
        assert!(!test_power(&s, "s1", &sec).0);
        sec.set(&Secrets::helper_key("s1"), "k1").unwrap();
        let (ok, t) = test_power(&s, "s1", &sec);
        assert!(ok, "{t}");
        let (r, has) = handle_power(Action::Restart, None, &s, "s1", &sec);
        assert!(r.ok && has);
        sec.set(&Secrets::helper_key("s1"), "wrong").unwrap();
        assert!(handle_power(Action::Restart, None, &s, "s1", &sec).0.msg.contains("refused the key"));
        let none = clean_settings(&json!({}));
        assert!(handle_power(Action::Off, None, &none, "s1", &sec).0.msg.starts_with("Pick your server type"));
    }

    #[test]
    fn typed_logins_are_saved_and_refused_ones_forgotten() {
        let (base, _log) = fake(|m, u, _h, b| match (m, u) {
            ("POST", "/v1/users/login") if b.contains("good") => (200, vec![], r#"{"data":{"token":{"access_token":"T"}}}"#.into()),
            ("POST", "/v1/users/login") => (200, vec![], "{}".into()),
            _ => (200, vec![], String::new()),
        });
        let port = base.rsplit(':').next().unwrap().to_string();
        let s = clean_settings(&json!({ "serverHost": "127.0.0.1", "serverPort": port, "platform": "zimaos" }));
        let sec = mem();
        let (r, has) = handle_power(Action::Restart, None, &s, "s1", &sec);
        assert!(r.auth && !has, "no login yet: ask for it");
        let (r, has) = handle_power(Action::Restart, Some(("me".into(), "good".into())), &s, "s1", &sec);
        assert!(r.ok && has);
        sec.save_login("zimaos", "s1", "me", "changed").unwrap();
        let (r, has) = handle_power(Action::Restart, None, &s, "s1", &sec);
        assert!(r.auth && !has, "a refused saved login is forgotten");
        assert_eq!(check_and_save_login("zimaos", " me ", "good", &s, "s1", &sec).unwrap(), "ZimaOS login checked and saved");
        assert!(check_and_save_login("linux", "a", "b", &s, "s1", &sec).is_err());
        assert_eq!(check_and_save_login("zimaos", "", "b", &s, "s1", &sec).unwrap_err(), "Enter a username and a password.");
    }

    #[test]
    fn helper_script() {
        let k = new_helper_key();
        assert_eq!(k.len(), 48);
        assert!(k.chars().all(|c| c.is_ascii_hexdigit()));
        let sc = helper_setup_script(&k, 61300);
        assert!(sc.contains(&format!("printf '%s\\n' '{k}' > /etc/board-power.key")));
        assert!(sc.contains("Environment=BOARD_POWER_PORT=61300"));
        assert!(sc.contains("PORT = int(os.environ.get(\"BOARD_POWER_PORT\", \"61209\"))"));
        assert!(sc.contains("CMDS = {\"/reboot\": [\"systemctl\", \"reboot\"]"));
        assert!(sc.contains("serve_forever()\nBOARD_PY\n"));
        assert!(sc.starts_with("$(command -v sudo) bash -c \"$(cat <<'BOARD_SETUP'"));
        assert_eq!(pve_user("root"), "root@pam");
        assert_eq!(pve_user(" a@pve "), "a@pve");
    }
}
