//! Settings: the defaults, the cleaning of anything read from disk / an import / the Settings form,
//! and the addresses derived from them. Same keys and rules as the original script.

use crate::config::{self, TempLimit, ACCENTS, GLOW_LEVELS, PLATFORMS, TEMP_KEYS, THEMES};
use regex::Regex;
use serde::Serialize;
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::sync::OnceLock;

/// Settings that belong to one server (everything else is shared by all servers).
pub const SERVER_SETTING_KEYS: [&str; 24] = [
    "serverHost", "serverScheme", "serverPort", "monitoring", "baseUrl", "apiVersion", "username", "platform", "zimaUrl", "proxmoxUrl", "proxmoxNode",
    "truenasUrl", "unraidUrl", "synologyUrl", "omvUrl", "helperUrl", "showPower", "diskMounts", "maxDisks",
    "networkInterface", "monitoringOpen", "monitoringFixed", "tempLimits", "liveSeconds",
];

/// The settings that make up the look (previewed live behind the Settings sheet).
pub const LOOK_KEYS: [&str; 9] = ["theme", "accent", "glow", "movingBg", "cardStyle", "corners", "font", "buttonSize", "style"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Limit {
    pub warm: i64,
    pub hot: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub server_host: String,
    pub server_scheme: String,
    pub server_port: String,
    pub monitoring: bool,
    pub base_url: String,
    pub api_version: i64,
    pub username: String,
    pub platform: String,
    pub zima_url: String,
    pub proxmox_url: String,
    pub proxmox_node: String,
    pub truenas_url: String,
    pub unraid_url: String,
    pub synology_url: String,
    pub omv_url: String,
    pub helper_url: String,
    pub show_power: bool,
    pub disk_mounts: Vec<String>,
    pub max_disks: i64,
    pub network_interface: String,
    pub monitoring_open: bool,
    pub monitoring_fixed: bool,
    pub temp_limits: BTreeMap<String, Limit>,
    pub live_seconds: i64,
    pub refresh_minutes: i64,
    pub link_url: String,
    pub widget_status_dots: bool,
    pub widget_title: String,
    pub widget_rows: i64,
    pub use_logos: bool,
    pub columns: i64,
    pub status_seconds: i64,
    pub ping_timeout: i64,
    pub theme: String,
    pub accent: String,
    pub glow: String,
    pub moving_bg: bool,
    pub card_style: String,
    pub corners: String,
    pub font: String,
    pub button_size: String,
    pub style: String,
    pub splash: bool,
}

impl Default for Settings {
    fn default() -> Self {
        let mut temp_limits = BTreeMap::new();
        for k in TEMP_KEYS {
            let TempLimit { warm, hot } = config::temp_default(k);
            temp_limits.insert(k.to_string(), Limit { warm, hot });
        }
        Settings {
            server_host: String::new(),
            server_scheme: String::new(),
            server_port: String::new(),
            monitoring: true,
            base_url: String::new(),
            api_version: 4,
            username: String::new(),
            platform: "none".into(),
            zima_url: String::new(),
            proxmox_url: String::new(),
            proxmox_node: String::new(),
            truenas_url: String::new(),
            unraid_url: String::new(),
            synology_url: String::new(),
            omv_url: String::new(),
            helper_url: String::new(),
            show_power: true,
            disk_mounts: vec![],
            max_disks: 3,
            network_interface: String::new(),
            monitoring_open: false,
            monitoring_fixed: false,
            temp_limits,
            live_seconds: 3,
            refresh_minutes: 5,
            link_url: String::new(),
            widget_status_dots: true,
            widget_title: String::new(),
            widget_rows: 6,
            use_logos: true,
            columns: 2,
            status_seconds: 30,
            ping_timeout: 3,
            theme: "aurora".into(),
            accent: String::new(),
            glow: "normal".into(),
            moving_bg: true,
            card_style: "normal".into(),
            corners: "round".into(),
            font: "system".into(),
            button_size: "normal".into(),
            style: "classic".into(),
            splash: false,
        }
    }
}

fn re(cell: &'static OnceLock<Regex>, pat: &str) -> &'static Regex {
    cell.get_or_init(|| Regex::new(pat).unwrap())
}

macro_rules! rx {
    ($pat:expr) => {{
        static CELL: OnceLock<Regex> = OnceLock::new();
        re(&CELL, $pat)
    }};
}

/// JavaScript's Number(v) for the kinds of values settings hold (None = NaN).
fn js_number(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
        Value::String(s) => {
            let t = s.trim();
            if t.is_empty() {
                Some(0.0)
            } else {
                t.parse::<f64>().ok()
            }
        }
        _ => None,
    }
}

/// clampNum: a whole number between lo and hi, or the default (empty, missing or not a number).
pub fn clamp_num(v: Option<&Value>, lo: i64, hi: i64, def: i64) -> i64 {
    match v {
        None | Some(Value::Null) => def,
        Some(Value::String(s)) if s.is_empty() => def,
        Some(x) => match js_number(x) {
            Some(n) if n.is_finite() => config::js_round(n).clamp(lo, hi),
            _ => def,
        },
    }
}

/// Keep at most `max` characters.
fn cut(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

/// "http://192.168.1.10:61208/x" -> "192.168.1.10" ("localhost" when it isn't an address)
pub fn host_of(url: &str) -> String {
    rx!(r"(?i)^https?://([^:/]+)")
        .captures(url)
        .map(|c| c[1].to_string())
        .unwrap_or_else(|| "localhost".into())
}

impl Settings {
    pub fn plat_url(&self, url_key: &str) -> &str {
        match url_key {
            "zimaUrl" => &self.zima_url,
            "proxmoxUrl" => &self.proxmox_url,
            "truenasUrl" => &self.truenas_url,
            "unraidUrl" => &self.unraid_url,
            "synologyUrl" => &self.synology_url,
            "omvUrl" => &self.omv_url,
            "helperUrl" => &self.helper_url,
            _ => "",
        }
    }

    pub fn plat_url_mut(&mut self, url_key: &str) -> Option<&mut String> {
        Some(match url_key {
            "zimaUrl" => &mut self.zima_url,
            "proxmoxUrl" => &mut self.proxmox_url,
            "truenasUrl" => &mut self.truenas_url,
            "unraidUrl" => &mut self.unraid_url,
            "synologyUrl" => &mut self.synology_url,
            "omvUrl" => &mut self.omv_url,
            "helperUrl" => &mut self.helper_url,
            _ => return None,
        })
    }

    /// The server's IP or name ("" = not set up yet).
    pub fn server_host(&self) -> String {
        if !self.server_host.is_empty() {
            self.server_host.clone()
        } else if !self.base_url.is_empty() {
            host_of(&self.base_url)
        } else {
            String::new()
        }
    }

    /// Where Glances answers: the address in Settings, or the server on Glances' usual port 61208.
    pub fn glances_url(&self) -> String {
        let h = self.server_host();
        if !self.base_url.is_empty() {
            self.base_url.clone()
        } else if !h.is_empty() {
            format!("http://{h}:61208")
        } else {
            String::new()
        }
    }

    /// The server address as a full address: its http / https and port, or the usual ones for the server type.
    pub fn server_url(&self) -> String {
        self.server_url_for(&self.platform)
    }

    pub fn server_url_for(&self, platform: &str) -> String {
        let pl = config::platform(platform);
        let scheme = if !self.server_scheme.is_empty() {
            self.server_scheme.clone()
        } else {
            pl.map(|p| p.scheme).unwrap_or("http").to_string()
        };
        let port = if !self.server_port.is_empty() {
            self.server_port.clone()
        } else {
            pl.and_then(|p| p.port_for(&scheme)).map(|p| p.to_string()).unwrap_or_default()
        };
        let host = self.server_host();
        let host = if host.is_empty() { "localhost".to_string() } else { host };
        if port.is_empty() {
            format!("{scheme}://{host}")
        } else {
            format!("{scheme}://{host}:{port}")
        }
    }

    /// The address Reboot / Shut down uses for a server type: an older separate address, or the server address.
    pub fn platform_base(&self, platform: &str) -> String {
        if let Some(p) = config::platform(platform) {
            let own = self.plat_url(p.url_key);
            if !own.is_empty() {
                return own.trim_end_matches('/').to_string();
            }
        }
        self.server_url_for(platform)
    }

    /// The Linux helper's port (from its address).
    pub fn helper_port(&self) -> u16 {
        let base = self.platform_base("linux");
        if let Some(c) = rx!(r"(?i)^https?://[^/:]+:(\d+)").captures(&base) {
            return c[1].parse().unwrap_or(config::HELPER_PORT);
        }
        if base.to_lowercase().starts_with("https:") {
            443
        } else {
            80
        }
    }

    pub fn limit(&self, k: &str) -> Limit {
        self.temp_limits.get(k).copied().unwrap_or_else(|| {
            let d = config::temp_default(k);
            Limit { warm: d.warm, hot: d.hot }
        })
    }

    pub fn to_value(&self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Null)
    }

    /// Only the look (theme & look settings).
    pub fn look(&self) -> Look {
        Look {
            theme: self.theme.clone(),
            accent: self.accent.clone(),
            glow: self.glow.clone(),
            moving_bg: self.moving_bg,
            card_style: self.card_style.clone(),
            corners: self.corners.clone(),
            font: self.font.clone(),
            button_size: self.button_size.clone(),
            style: self.style.clone(),
        }
    }
}

/// Theme & look (a subset of Settings that the dashboard can preview).
#[derive(Debug, Clone, PartialEq)]
pub struct Look {
    pub theme: String,
    pub accent: String,
    pub glow: String,
    pub moving_bg: bool,
    pub card_style: String,
    pub corners: String,
    pub font: String,
    pub button_size: String,
    pub style: String,
}

impl Look {
    pub fn from_value(v: &Value) -> Look {
        clean_settings(v).look()
    }
    pub fn glass(&self) -> bool {
        self.style == "glass"
    }
}

/// Resolved colours for a look.
#[derive(Debug, Clone, PartialEq)]
pub struct LookVars {
    pub bg: String,
    pub g1: (u8, u8, u8),
    pub g2: (u8, u8, u8),
    pub acc: String,
    pub ink: String,
    pub panel: String,
    pub panel2: String,
    pub glowop: f64,
    pub glass: f64,
}

pub fn look_vars(look: &Look) -> LookVars {
    let t = config::theme(&look.theme);
    let a = ACCENTS.iter().find(|a| a.key == look.accent);
    let own = a.and_then(|a| a.c);
    LookVars {
        bg: t.bg.into(),
        g1: t.g1,
        g2: t.g2,
        acc: own.unwrap_or(t.acc).into(),
        ink: if own.is_some() { a.unwrap().ink.into() } else { t.ink.into() },
        panel: t.panel.into(),
        panel2: t.panel2.into(),
        glowop: if t.no_glow { 0.0 } else { config::glow_level(&look.glow) },
        glass: match look.card_style.as_str() {
            "subtle" => 0.04,
            "strong" => 0.12,
            _ => 0.07,
        },
    }
}

/// cleanSettings: anything (saved file, import, the Settings form) -> valid settings.
pub fn clean_settings(raw: &Value) -> Settings {
    let empty = Map::new();
    let r = raw.as_object().unwrap_or(&empty);
    let d = Settings::default();
    let get = |k: &str| r.get(k);
    let s = |k: &str, max: usize, def: &str| -> String {
        match get(k) {
            Some(Value::String(v)) => cut(v.trim(), max),
            _ => def.to_string(),
        }
    };
    let b = |k: &str, def: bool| -> bool {
        match get(k) {
            Some(Value::Bool(v)) => *v,
            _ => def,
        }
    };
    let n = |k: &str, lo: i64, hi: i64, def: i64| clamp_num(get(k), lo, hi, def);
    let pick = |k: &str, allowed: &[&str], def: &str| -> String {
        match get(k) {
            Some(Value::String(v)) if allowed.contains(&v.as_str()) => v.clone(),
            _ => def.to_string(),
        }
    };

    let mut o = Settings::default();
    o.base_url = s("baseUrl", 300, "");
    if !rx!(r"(?i)^https?://[^/:]+").is_match(&o.base_url) {
        o.base_url.clear();
    }
    o.base_url = o.base_url.trim_end_matches('/').to_string();

    // the server address: IP or name, plus http / https and a port when they aren't the usual ones
    o.server_host = s("serverHost", 200, "");
    o.server_scheme = match get("serverScheme") {
        Some(Value::String(v)) if v == "http" || v == "https" => v.clone(),
        _ => String::new(),
    };
    let port_raw = match get("serverPort") {
        Some(Value::String(v)) => v.clone(),
        Some(Value::Number(v)) => v.to_string(),
        _ => String::new(),
    };
    o.server_port = if rx!(r"^\d{1,5}$").is_match(&port_raw) { port_raw } else { String::new() };
    if let Some(c) = rx!(r"(?i)^(https?)://").captures(&o.server_host) {
        if o.server_scheme.is_empty() {
            o.server_scheme = c[1].to_lowercase();
        }
    }
    o.server_host = rx!(r"(?i)^[a-z]+://").replace(&o.server_host, "").to_string();
    o.server_host = rx!(r"[/?#].*$").replace(&o.server_host, "").to_string();
    if let Some(c) = rx!(r"^([^\[\]:]+):(\d+)$").captures(&o.server_host.clone()) {
        o.server_host = c[1].to_string();
        if o.server_port.is_empty() {
            o.server_port = c[2].to_string();
        }
    }
    if o.server_host.is_empty() && !o.base_url.is_empty() {
        o.server_host = host_of(&o.base_url);
    }
    o.monitoring = b("monitoring", d.monitoring);
    o.api_version = n("apiVersion", 3, 4, d.api_version);
    o.username = s("username", 100, "");
    // "none" = not picked yet. Settings saved by the older ZimaOS-only -board
    // (they have showPower but no platform) stay on ZimaOS, so their Reboot menu keeps working.
    let rp = match get("platform") {
        Some(Value::String(v)) => v.as_str(),
        _ => "",
    };
    o.platform = if config::platform(rp).is_some() {
        rp.to_string()
    } else if rp == "none" || rp == "auto" {
        "none".into()
    } else if matches!(get("showPower"), Some(Value::Bool(_))) {
        "zimaos".into()
    } else {
        "none".into()
    };
    for p in PLATFORMS.iter() {
        let mut v = s(p.url_key, 300, "");
        if !v.is_empty() && !rx!(r"(?i)^https?://").is_match(&v) {
            v.clear();
        }
        *o.plat_url_mut(p.url_key).unwrap() = v.trim_end_matches('/').to_string();
    }
    // An older separate address for the server system on the same host becomes part of the server address
    // (http / https and port), so there's only one address to keep up to date
    if let Some(pl) = config::platform(&o.platform) {
        let own = o.plat_url(pl.url_key).to_string();
        if !own.is_empty() && !o.server_host.is_empty() && host_of(&own).to_lowercase() == o.server_host.to_lowercase() {
            if let Some(m) = rx!(r"(?i)^(https?)://[^/:]+(?::(\d+))?").captures(&own) {
                if o.server_scheme.is_empty() && o.server_port.is_empty() {
                    let sc = m[1].to_lowercase();
                    o.server_scheme = if sc == pl.scheme { String::new() } else { sc.clone() };
                    let usual = pl.port_for(&sc).map(|p| p as i64).unwrap_or(0);
                    o.server_port = match m.get(2) {
                        Some(p) if p.as_str().parse::<i64>().unwrap_or(-1) != usual => p.as_str().to_string(),
                        _ => String::new(),
                    };
                }
            }
            o.plat_url_mut(pl.url_key).unwrap().clear();
        }
    }
    o.proxmox_node = s("proxmoxNode", 64, "").chars().filter(|c| c.is_ascii_alphanumeric() || "._-".contains(*c)).collect();
    o.show_power = b("showPower", d.show_power);
    o.disk_mounts = match get("diskMounts") {
        Some(Value::Array(a)) => a.iter().filter_map(|x| x.as_str()).filter(|x| !x.is_empty()).take(12).map(String::from).collect(),
        _ => vec![],
    };
    o.max_disks = n("maxDisks", 1, 12, d.max_disks);
    o.network_interface = s("networkInterface", 60, "");
    o.monitoring_open = b("monitoringOpen", d.monitoring_open);
    o.monitoring_fixed = b("monitoringFixed", d.monitoring_fixed);
    let t = get("tempLimits").and_then(|v| v.as_object());
    o.temp_limits.clear();
    for k in TEMP_KEYS {
        let def = config::temp_default(k);
        let x = t.and_then(|t| t.get(k)).and_then(|v| v.as_object());
        let warm = clamp_num(x.and_then(|x| x.get("warm")), 20, 120, def.warm);
        let hot = clamp_num(x.and_then(|x| x.get("hot")), 21, 130, def.hot).max(warm + 1);
        o.temp_limits.insert(k.to_string(), Limit { warm, hot });
    }
    o.live_seconds = n("liveSeconds", 1, 30, d.live_seconds);
    o.refresh_minutes = n("refreshMinutes", 1, 120, d.refresh_minutes);
    o.link_url = s("linkUrl", 300, "");
    o.widget_status_dots = b("widgetStatusDots", d.widget_status_dots);
    o.widget_title = s("widgetTitle", 20, "");
    o.widget_rows = n("widgetRows", 1, 6, d.widget_rows);
    o.use_logos = b("useLogos", d.use_logos);
    o.columns = n("columns", 2, 3, d.columns);
    o.status_seconds = n("statusSeconds", 10, 600, d.status_seconds);
    o.ping_timeout = n("pingTimeout", 1, 15, d.ping_timeout);
    let themes: Vec<&str> = THEMES.iter().map(|t| t.key).collect();
    o.theme = pick("theme", &themes, &d.theme);
    let accents: Vec<&str> = ACCENTS.iter().map(|a| a.key).collect();
    o.accent = pick("accent", &accents, &d.accent);
    let glows: Vec<&str> = GLOW_LEVELS.iter().map(|g| g.0).collect();
    o.glow = pick("glow", &glows, &d.glow);
    o.moving_bg = b("movingBg", d.moving_bg);
    o.card_style = pick("cardStyle", &["subtle", "normal", "strong"], &d.card_style);
    o.corners = pick("corners", &["round", "sharp"], &d.corners);
    o.font = pick("font", &["system", "rounded", "serif"], &d.font);
    o.button_size = pick("buttonSize", &["normal", "compact"], &d.button_size);
    o.style = pick("style", &["classic", "glass"], &d.style);
    o.splash = b("splash", d.splash);
    o
}

/// The default settings as JSON (what a brand-new server starts from).
pub fn defaults_value() -> Value {
    Settings::default().to_value()
}

/// A JSON object of only some keys of a JSON object.
pub fn pick_keys(v: &Value, keys: &[&str]) -> Value {
    let mut m = Map::new();
    if let Some(o) = v.as_object() {
        for k in keys {
            if let Some(x) = o.get(*k) {
                m.insert((*k).to_string(), x.clone());
            }
        }
    }
    Value::Object(m)
}

pub fn empty_object() -> Value {
    json!({})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_when_empty() {
        let s = clean_settings(&json!({}));
        assert_eq!(s, Settings::default());
        assert_eq!(s.platform, "none");
        assert_eq!(s.limit("cpu"), Limit { warm: 70, hot: 85 });
    }

    #[test]
    fn server_address_is_split_into_host_scheme_and_port() {
        let s = clean_settings(&json!({ "serverHost": " https://nas.lan:8443/ui?x=1 " }));
        assert_eq!(s.server_host, "nas.lan");
        assert_eq!(s.server_scheme, "https");
        assert_eq!(s.server_port, "8443");
        assert_eq!(s.server_url(), "https://nas.lan:8443");
        let s = clean_settings(&json!({ "serverHost": "10.0.0.2", "serverPort": 90 }));
        assert_eq!(s.server_port, "90");
        let s = clean_settings(&json!({ "baseUrl": "http://192.168.1.4:61208///" }));
        assert_eq!(s.base_url, "http://192.168.1.4:61208");
        assert_eq!(s.server_host, "192.168.1.4");
        assert_eq!(s.glances_url(), "http://192.168.1.4:61208");
        let s = clean_settings(&json!({ "baseUrl": "ftp://x" }));
        assert_eq!(s.base_url, "");
    }

    #[test]
    fn usual_ports_per_server_type() {
        let s = clean_settings(&json!({ "serverHost": "pve", "platform": "proxmox" }));
        assert_eq!(s.server_url(), "https://pve:8006");
        let s = clean_settings(&json!({ "serverHost": "ds", "platform": "synology", "serverScheme": "https" }));
        assert_eq!(s.server_url(), "https://ds:5001");
        let s = clean_settings(&json!({ "serverHost": "box", "platform": "linux" }));
        assert_eq!(s.platform_base("linux"), "http://box:61209");
        assert_eq!(s.helper_port(), 61209);
        let s = clean_settings(&json!({ "serverHost": "z", "platform": "zimaos" }));
        assert_eq!(s.server_url(), "http://z");
        assert_eq!(s.glances_url(), "http://z:61208");
    }

    #[test]
    fn older_zimaos_settings_keep_their_reboot_menu() {
        assert_eq!(clean_settings(&json!({ "showPower": true })).platform, "zimaos");
        assert_eq!(clean_settings(&json!({ "platform": "auto", "showPower": true })).platform, "none");
        assert_eq!(clean_settings(&json!({ "platform": "bogus" })).platform, "none");
    }

    #[test]
    fn an_older_separate_address_on_the_same_host_is_merged() {
        let s = clean_settings(&json!({ "serverHost": "10.0.0.5", "platform": "proxmox", "proxmoxUrl": "https://10.0.0.5:8007/" }));
        assert_eq!(s.proxmox_url, "");
        assert_eq!(s.server_scheme, "");
        assert_eq!(s.server_port, "8007");
        // on another host it stays and wins
        let s = clean_settings(&json!({ "serverHost": "10.0.0.5", "platform": "proxmox", "proxmoxUrl": "https://10.0.0.9:8006" }));
        assert_eq!(s.platform_base("proxmox"), "https://10.0.0.9:8006");
        let s = clean_settings(&json!({ "serverHost": "nas", "platform": "zimaos", "zimaUrl": "https://nas" }));
        assert_eq!(s.server_scheme, "https");
        assert_eq!(s.zima_url, "");
    }

    #[test]
    fn numbers_are_clamped_and_rounded() {
        let s = clean_settings(&json!({ "liveSeconds": "0", "maxDisks": 99, "columns": "2.5", "pingTimeout": "x", "widgetRows": "" }));
        assert_eq!(s.live_seconds, 1);
        assert_eq!(s.max_disks, 12);
        assert_eq!(s.columns, 3);
        assert_eq!(s.ping_timeout, 3);
        assert_eq!(s.widget_rows, 6);
    }

    #[test]
    fn temperature_limits_stay_ordered() {
        let s = clean_settings(&json!({ "tempLimits": { "cpu": { "warm": 90, "hot": 80 }, "hdd": { "warm": "40" } } }));
        assert_eq!(s.limit("cpu"), Limit { warm: 90, hot: 91 });
        assert_eq!(s.limit("hdd"), Limit { warm: 40, hot: 55 });
        assert_eq!(s.limit("nvme"), Limit { warm: 60, hot: 70 });
    }

    #[test]
    fn picks_and_strings() {
        let s = clean_settings(&json!({ "theme": "ocean", "accent": "nope", "style": "glass", "widgetTitle": "123456789012345678901234", "proxmoxNode": "pve 1!", "diskMounts": ["/", "", 3, "/data"] }));
        assert_eq!(s.theme, "ocean");
        assert_eq!(s.accent, "");
        assert_eq!(s.style, "glass");
        assert_eq!(s.widget_title.chars().count(), 20);
        assert_eq!(s.proxmox_node, "pve1");
        assert_eq!(s.disk_mounts, vec!["/", "/data"]);
    }

    #[test]
    fn round_trip_is_stable() {
        let s = clean_settings(&json!({ "serverHost": "a", "theme": "rose", "tempLimits": { "cpu": { "warm": 50, "hot": 60 } } }));
        assert_eq!(clean_settings(&s.to_value()), s);
    }

    #[test]
    fn look_colours() {
        let mut l = Settings::default().look();
        let v = look_vars(&l);
        assert_eq!(v.acc, "#34e0a1");
        assert_eq!(v.glowop, 0.5);
        l.accent = "blue".into();
        l.theme = "oled".into();
        l.card_style = "strong".into();
        let v = look_vars(&l);
        assert_eq!(v.acc, "#4fc3ff");
        assert_eq!(v.ink, "#04203a");
        assert_eq!(v.glowop, 0.0);
        assert_eq!(v.glass, 0.12);
    }
}
