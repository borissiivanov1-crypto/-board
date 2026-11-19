//! Fixed tables: server types, themes, accents, glow levels and the default settings.
//! These mirror the original -board script one to one, so exported settings move between
//! the iOS and the GNOME version unchanged.

/// Starting defaults (the values in CONFIG at the top of the original script).
pub const DEFAULT_TITLE: &str = "MY SERVER";
pub const LOGO_CDN: &str = "https://cdn.jsdelivr.net/gh/homarr-labs/dashboard-icons/png/";
pub const STACK_ICON: &str = "🗂️";
pub const HELPER_PORT: u16 = 61209;
/// How often the dashboard checks LIVE / OFFLINE (seconds).
pub const REACH_SECONDS: u64 = 10;
/// Stored logo size in pixels.
pub const LOGO_SIZE: u32 = 96;
/// A logo that failed to download is retried after this long.
pub const LOGO_RETRY_HOURS: u64 = 12;

/// A server type: how Reboot / Shut down reach the server.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Platform {
    pub key: &'static str,
    pub name: &'static str,
    pub short: Option<&'static str>,
    /// Name in the login popup (None = no login: the Linux helper uses its secret key).
    pub login: Option<&'static str>,
    /// An older setting with a separate address for the server system.
    pub url_key: &'static str,
    /// What's used when the address doesn't say.
    pub scheme: &'static str,
    pub port_http: Option<u16>,
    pub port_https: Option<u16>,
    pub user_ph: &'static str,
}

impl Platform {
    pub fn short_name(&self) -> &'static str {
        self.short.unwrap_or(self.name)
    }
    pub fn port_for(&self, scheme: &str) -> Option<u16> {
        if scheme == "https" {
            self.port_https
        } else {
            self.port_http
        }
    }
}

pub const PLATFORMS: [Platform; 7] = [
    Platform { key: "zimaos", name: "ZimaOS", short: None, login: Some("ZimaOS"), url_key: "zimaUrl", scheme: "http", port_http: None, port_https: None, user_ph: "" },
    Platform { key: "proxmox", name: "Proxmox VE", short: Some("Proxmox"), login: Some("Proxmox"), url_key: "proxmoxUrl", scheme: "https", port_http: Some(8006), port_https: Some(8006), user_ph: "root@pam" },
    Platform { key: "truenas", name: "TrueNAS", short: None, login: Some("TrueNAS"), url_key: "truenasUrl", scheme: "http", port_http: None, port_https: None, user_ph: "truenas_admin" },
    Platform { key: "unraid", name: "Unraid", short: None, login: Some("Unraid"), url_key: "unraidUrl", scheme: "http", port_http: None, port_https: None, user_ph: "root" },
    Platform { key: "synology", name: "Synology DSM", short: Some("Synology"), login: Some("Synology"), url_key: "synologyUrl", scheme: "http", port_http: Some(5000), port_https: Some(5001), user_ph: "" },
    Platform { key: "omv", name: "OpenMediaVault", short: Some("OMV"), login: Some("OpenMediaVault"), url_key: "omvUrl", scheme: "http", port_http: None, port_https: None, user_ph: "admin" },
    Platform { key: "linux", name: "Linux", short: None, login: None, url_key: "helperUrl", scheme: "http", port_http: Some(HELPER_PORT), port_https: Some(HELPER_PORT), user_ph: "" },
];

pub fn platform(key: &str) -> Option<&'static Platform> {
    PLATFORMS.iter().find(|p| p.key == key)
}

pub const TYPE_HINT_NONE: &str = "Pick the system your server runs. It decides how Reboot / Shut down reach the server; until you pick one there is no Reboot / Shut down menu. Not sure? Debian, Ubuntu and similar are Linux.";

pub fn type_hint(key: &str) -> &'static str {
    match key {
        "zimaos" => "ZimaOS: Reboot / Shut down use your ZimaOS login. Nothing to install.",
        "proxmox" => "Proxmox VE: Reboot / Shut down use the Proxmox API with your Proxmox login (e.g. root@pam). Nothing to install. Its self-signed certificate is accepted.",
        "truenas" => "TrueNAS: Reboot / Shut down use your TrueNAS login (the admin account, e.g. truenas_admin, or root on older versions). Nothing to install. With https, TrueNAS needs a certificate this computer trusts; plain http on your home network or VPN is fine.",
        "unraid" => "Unraid: Reboot / Shut down log in with your Unraid login (root) and send what Unraid's own Reboot / Shut down buttons send. Nothing to install.",
        "synology" => "Synology DSM: Reboot / Shut down use the DSM Web API with an administrator account. Nothing to install.",
        "omv" => "OpenMediaVault: Reboot / Shut down use OpenMediaVault's own API with an administrator account (admin by default). Nothing to install.",
        "linux" => "Linux (Debian, Ubuntu Server, Fedora ...): there's no web API for reboot, so -board uses a tiny helper service on the server, set up with one command over SSH from Settings. No login needed.",
        _ => TYPE_HINT_NONE,
    }
}

/// The hint under the server address: which port is used when none is typed.
pub fn addr_hint(key: &str) -> String {
    let usual = match platform(key) {
        Some(p) if p.port_http.is_some() => {
            let (h, s) = (p.port_http.unwrap(), p.port_https.unwrap_or(0));
            let ports = if h == s { h.to_string() } else { format!("{h} (http) or {s} (https)") };
            format!(
                " For {}, port {}{} is used unless you type another.",
                p.short_name(),
                ports,
                if key == "linux" { " (the helper)" } else { "" }
            )
        }
        _ => String::new(),
    };
    format!("Pick http or https, then type the IP or name, plus a port if it isn't the usual one (e.g. 192.168.1.10:90).{usual} LIVE / OFFLINE checks this address, Reboot / Shut down use it, and Glances is looked for on it. Use your VPN address to reach it away from home.")
}

/// Temperatures (°C) where the number turns orange (warm) and red (hot).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TempLimit {
    pub warm: i64,
    pub hot: i64,
}
pub const TEMP_KEYS: [&str; 3] = ["cpu", "nvme", "hdd"];
pub fn temp_default(k: &str) -> TempLimit {
    match k {
        "nvme" => TempLimit { warm: 60, hot: 70 },
        "hdd" => TempLimit { warm: 45, hot: 55 },
        _ => TempLimit { warm: 70, hot: 85 },
    }
}

/// g1 / g2: the two background glows (r,g,b). panel / panel2: popup and dropdown backgrounds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Theme {
    pub key: &'static str,
    pub name: &'static str,
    pub bg: &'static str,
    pub g1: (u8, u8, u8),
    pub g2: (u8, u8, u8),
    pub acc: &'static str,
    pub ink: &'static str,
    pub panel: &'static str,
    pub panel2: &'static str,
    pub no_glow: bool,
}

pub const THEMES: [Theme; 7] = [
    Theme { key: "aurora", name: "Aurora", bg: "#0b1220", g1: (18, 194, 165), g2: (106, 92, 255), acc: "#34e0a1", ink: "#06281d", panel: "rgba(30,38,64,.94)", panel2: "#1e2640", no_glow: false },
    Theme { key: "ocean", name: "Ocean", bg: "#07111f", g1: (0, 132, 255), g2: (0, 206, 209), acc: "#4fc3ff", ink: "#04203a", panel: "rgba(20,40,68,.94)", panel2: "#142844", no_glow: false },
    Theme { key: "sunset", name: "Sunset", bg: "#150d12", g1: (255, 122, 69), g2: (214, 51, 132), acc: "#ff9f6b", ink: "#3a1606", panel: "rgba(54,30,40,.94)", panel2: "#361e28", no_glow: false },
    Theme { key: "forest", name: "Forest", bg: "#09140f", g1: (46, 160, 67), g2: (18, 120, 140), acc: "#7ddc6f", ink: "#0d2a0a", panel: "rgba(24,44,36,.94)", panel2: "#182c24", no_glow: false },
    Theme { key: "rose", name: "Rose", bg: "#140b18", g1: (255, 92, 160), g2: (124, 77, 255), acc: "#ff7eb6", ink: "#3a0b22", panel: "rgba(48,28,58,.94)", panel2: "#301c3a", no_glow: false },
    Theme { key: "graphite", name: "Graphite", bg: "#111317", g1: (120, 130, 150), g2: (70, 80, 100), acc: "#d6dbe4", ink: "#15181e", panel: "rgba(36,38,44,.95)", panel2: "#24262c", no_glow: false },
    Theme { key: "oled", name: "OLED black", bg: "#000000", g1: (0, 0, 0), g2: (0, 0, 0), acc: "#34e0a1", ink: "#06281d", panel: "rgba(22,22,26,.97)", panel2: "#16161a", no_glow: true },
];

pub fn theme(key: &str) -> &'static Theme {
    THEMES.iter().find(|t| t.key == key).unwrap_or(&THEMES[0])
}

/// "" = the theme's own accent.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Accent {
    pub key: &'static str,
    pub name: &'static str,
    pub c: Option<&'static str>,
    pub ink: &'static str,
}

pub const ACCENTS: [Accent; 7] = [
    Accent { key: "", name: "Theme", c: None, ink: "" },
    Accent { key: "green", name: "Green", c: Some("#34e0a1"), ink: "#06281d" },
    Accent { key: "blue", name: "Blue", c: Some("#4fc3ff"), ink: "#04203a" },
    Accent { key: "purple", name: "Purple", c: Some("#a78bfa"), ink: "#1e1240" },
    Accent { key: "orange", name: "Orange", c: Some("#ff9f6b"), ink: "#3a1606" },
    Accent { key: "pink", name: "Pink", c: Some("#ff7eb6"), ink: "#3a0b22" },
    Accent { key: "white", name: "White", c: Some("#e8ecf3"), ink: "#15181e" },
];

pub const GLOW_LEVELS: [(&str, f64); 4] = [("off", 0.0), ("soft", 0.3), ("normal", 0.5), ("strong", 0.75)];

pub fn glow_level(key: &str) -> f64 {
    GLOW_LEVELS.iter().find(|g| g.0 == key).map(|g| g.1).unwrap_or(0.5)
}

/// Status colours (green / orange / red) for a percentage.
pub fn color_for(pct: f64) -> &'static str {
    if pct >= 85.0 {
        "#ff5d73"
    } else if pct >= 60.0 {
        "#ffb454"
    } else {
        "#34e0a1"
    }
}

pub const GREEN: &str = "#34e0a1";
pub const ORANGE: &str = "#ffb454";
pub const RED: &str = "#ff5d73";
pub const BLUE: &str = "#5ac8fa";
pub const PEACH: &str = "#ff9f6b";

/// "#12c2a5" -> (r, g, b) in 0..1
pub fn hex_rgb(hex: &str) -> (f64, f64, f64) {
    let h = hex.trim_start_matches('#');
    let p = |i: usize| u8::from_str_radix(h.get(i..i + 2).unwrap_or("00"), 16).unwrap_or(0) as f64 / 255.0;
    if h.len() >= 6 {
        (p(0), p(2), p(4))
    } else {
        (0.0, 0.0, 0.0)
    }
}

/// Bytes as "1.5 GB" (whole numbers from 100 up and for plain bytes).
pub fn format_bytes(b: f64) -> String {
    let units = ["B", "KB", "MB", "GB", "TB"];
    let mut b = if b.is_finite() { b } else { 0.0 };
    let mut i = 0;
    while b >= 1024.0 && i < units.len() - 1 {
        b /= 1024.0;
        i += 1;
    }
    if b >= 100.0 || i == 0 {
        format!("{} {}", js_round(b), units[i])
    } else {
        format!("{:.1} {}", b, units[i])
    }
}

/// JavaScript's Math.round (halves round up, towards +infinity).
pub fn js_round(v: f64) -> i64 {
    (v + 0.5).floor() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes_format_like_the_original() {
        assert_eq!(format_bytes(0.0), "0 B");
        assert_eq!(format_bytes(512.0), "512 B");
        assert_eq!(format_bytes(1536.0), "1.5 KB");
        assert_eq!(format_bytes(150.0 * 1024.0 * 1024.0), "150 MB");
        assert_eq!(format_bytes(1024.0f64.powi(5) * 2.0), "2048 TB");
    }

    #[test]
    fn colours_by_percentage() {
        assert_eq!(color_for(10.0), GREEN);
        assert_eq!(color_for(60.0), ORANGE);
        assert_eq!(color_for(85.0), RED);
    }

    #[test]
    fn address_hints() {
        assert!(addr_hint("proxmox").contains("port 8006 is used"));
        assert!(addr_hint("synology").contains("5000 (http) or 5001 (https)"));
        assert!(addr_hint("linux").contains("61209 (the helper)"));
        assert!(!addr_hint("zimaos").contains("is used unless"));
    }

    #[test]
    fn hex_parsing() {
        assert_eq!(hex_rgb("#ff0000"), (1.0, 0.0, 0.0));
        assert_eq!(js_round(2.5), 3);
        assert_eq!(js_round(-2.5), -2);
    }
}
