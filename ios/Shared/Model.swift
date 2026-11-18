import Foundation

// ---------- Default settings (the CONFIG block of the Scriptable script) ----------
enum CONFIG {
    static let title = "MY SERVER"
    static let monitoring = true
    static let apiVersion = 4
    static let maxDisks = 3
    static let refreshMinutes = 5
    static let widgetStatusDots = true
    static let platform = "none"
}

// Server types: they only change how Reboot / Shut down reaches the server.
struct Platform {
    let key: String, name: String, short: String?, login: String?, url: String, scheme: String, ports: [String: Int], userPh: String?
    var json: J {
        var o: J = ["name": name, "login": login as Any? ?? NSNull(), "url": url, "scheme": scheme, "ports": ports]
        if let s = short { o["short"] = s }
        if let u = userPh { o["userPh"] = u }
        return o
    }
}

let PLATFORM_KEYS = ["zimaos", "proxmox", "truenas", "unraid", "synology", "omv", "linux"]
let PLATFORMS: [String: Platform] = [
    "zimaos": Platform(key: "zimaos", name: "ZimaOS", short: nil, login: "ZimaOS", url: "zimaUrl", scheme: "http", ports: [:], userPh: nil),
    "proxmox": Platform(key: "proxmox", name: "Proxmox VE", short: "Proxmox", login: "Proxmox", url: "proxmoxUrl", scheme: "https", ports: ["http": 8006, "https": 8006], userPh: "root@pam"),
    "truenas": Platform(key: "truenas", name: "TrueNAS", short: nil, login: "TrueNAS", url: "truenasUrl", scheme: "http", ports: [:], userPh: "truenas_admin"),
    "unraid": Platform(key: "unraid", name: "Unraid", short: nil, login: "Unraid", url: "unraidUrl", scheme: "http", ports: [:], userPh: "root"),
    "synology": Platform(key: "synology", name: "Synology DSM", short: "Synology", login: "Synology", url: "synologyUrl", scheme: "http", ports: ["http": 5000, "https": 5001], userPh: nil),
    "omv": Platform(key: "omv", name: "OpenMediaVault", short: "OMV", login: "OpenMediaVault", url: "omvUrl", scheme: "http", ports: [:], userPh: "admin"),
    "linux": Platform(key: "linux", name: "Linux", short: nil, login: nil, url: "helperUrl", scheme: "http", ports: ["http": 61209, "https": 61209], userPh: nil),
]
var platformsJSON: OJ { OJ(pairs: PLATFORM_KEYS.map { ($0, PLATFORMS[$0]!.json) }) }

let TEMP_DEFAULTS: [String: (Int, Int)] = ["cpu": (70, 85), "nvme": (60, 70), "hdd": (45, 55)]

let SERVER_SETTING_KEYS: Set<String> = [
    "serverHost", "serverScheme", "serverPort", "monitoring", "baseUrl", "apiVersion", "username", "platform", "zimaUrl", "proxmoxUrl", "proxmoxNode",
    "truenasUrl", "unraidUrl", "synologyUrl", "omvUrl", "helperUrl", "showPower", "diskMounts", "maxDisks",
    "networkInterface", "monitoringOpen", "monitoringFixed", "tempLimits", "liveSeconds",
]
let SERVER_PARTS = ["custom", "edits", "hidden", "order", "widgetOrder", "widgetHidden", "groups"]

// ---------- Themes ----------
struct Theme { let key, name, bg, g1, g2, acc, ink, panel, panel2: String; let noGlow: Bool }
let THEME_LIST: [Theme] = [
    Theme(key: "aurora", name: "Aurora", bg: "#0b1220", g1: "18,194,165", g2: "106,92,255", acc: "#34e0a1", ink: "#06281d", panel: "rgba(30,38,64,.94)", panel2: "#1e2640", noGlow: false),
    Theme(key: "ocean", name: "Ocean", bg: "#07111f", g1: "0,132,255", g2: "0,206,209", acc: "#4fc3ff", ink: "#04203a", panel: "rgba(20,40,68,.94)", panel2: "#142844", noGlow: false),
    Theme(key: "sunset", name: "Sunset", bg: "#150d12", g1: "255,122,69", g2: "214,51,132", acc: "#ff9f6b", ink: "#3a1606", panel: "rgba(54,30,40,.94)", panel2: "#361e28", noGlow: false),
    Theme(key: "forest", name: "Forest", bg: "#09140f", g1: "46,160,67", g2: "18,120,140", acc: "#7ddc6f", ink: "#0d2a0a", panel: "rgba(24,44,36,.94)", panel2: "#182c24", noGlow: false),
    Theme(key: "rose", name: "Rose", bg: "#140b18", g1: "255,92,160", g2: "124,77,255", acc: "#ff7eb6", ink: "#3a0b22", panel: "rgba(48,28,58,.94)", panel2: "#301c3a", noGlow: false),
    Theme(key: "graphite", name: "Graphite", bg: "#111317", g1: "120,130,150", g2: "70,80,100", acc: "#d6dbe4", ink: "#15181e", panel: "rgba(36,38,44,.95)", panel2: "#24262c", noGlow: false),
    Theme(key: "oled", name: "OLED black", bg: "#000000", g1: "0,0,0", g2: "0,0,0", acc: "#34e0a1", ink: "#06281d", panel: "rgba(22,22,26,.97)", panel2: "#16161a", noGlow: true),
]
var themesJSON: OJ {
    OJ(pairs: THEME_LIST.map { t in
        var o: J = ["name": t.name, "bg": t.bg, "g1": t.g1, "g2": t.g2, "acc": t.acc, "ink": t.ink, "panel": t.panel, "panel2": t.panel2]
        if t.noGlow { o["noGlow"] = true }
        return (t.key, o)
    })
}
func theme(_ k: String) -> Theme { THEME_LIST.first { $0.key == k } ?? THEME_LIST[0] }

let ACCENT_LIST: [(String, String, String?, String?)] = [
    ("", "Theme", nil, nil), ("green", "Green", "#34e0a1", "#06281d"), ("blue", "Blue", "#4fc3ff", "#04203a"),
    ("purple", "Purple", "#a78bfa", "#1e1240"), ("orange", "Orange", "#ff9f6b", "#3a1606"), ("pink", "Pink", "#ff7eb6", "#3a0b22"),
    ("white", "White", "#e8ecf3", "#15181e"),
]
var accentsJSON: OJ {
    OJ(pairs: ACCENT_LIST.map { a in
        var o: J = ["name": a.1, "c": a.2 as Any? ?? NSNull()]
        if let ink = a.3 { o["ink"] = ink }
        return (a.0, o)
    })
}
let GLOW_LEVELS: [String: Double] = ["off": 0, "soft": 0.3, "normal": 0.5, "strong": 0.75]
var glowsJSON: OJ { OJ(pairs: ["off", "soft", "normal", "strong"].map { ($0, GLOW_LEVELS[$0]!) }) }

let LOGO_CDN = "https://cdn.jsdelivr.net/gh/homarr-labs/dashboard-icons/png/"
let STACK_ICON = "🗂️"
let HELPER_PORT = 61209

// ---------- Settings (gear icon in the live dashboard) ----------
var DEFAULTS: J {
    var d: J = [
        "serverHost": "", "serverScheme": "", "serverPort": "", "monitoring": CONFIG.monitoring, "baseUrl": "",
        "apiVersion": CONFIG.apiVersion, "username": "", "platform": CONFIG.platform, "proxmoxNode": "", "showPower": true,
        "diskMounts": [String](), "maxDisks": CONFIG.maxDisks, "networkInterface": "", "monitoringOpen": false, "monitoringFixed": false,
        "liveSeconds": 3, "refreshMinutes": CONFIG.refreshMinutes, "linkUrl": "", "widgetStatusDots": CONFIG.widgetStatusDots,
        "widgetTitle": "", "widgetRows": 6, "useLogos": true, "columns": 2, "statusSeconds": 30, "pingTimeout": 3,
        "theme": "aurora", "accent": "", "glow": "normal", "movingBg": true, "cardStyle": "normal", "corners": "round",
        "font": "system", "buttonSize": "normal", "style": "classic", "splash": false,
    ]
    for k in PLATFORM_KEYS { d[PLATFORMS[k]!.url] = "" }
    var t = J()
    for (k, v) in TEMP_DEFAULTS { t[k] = ["warm": v.0, "hot": v.1] }
    d["tempLimits"] = t
    return d
}

func hostOf(_ url: String) -> String { match(url, "^https?://([^:/]+)", true)?[1] ?? "localhost" }

func cleanSettings(_ raw: Any?) -> J {
    let r = raw as? J ?? J()
    let D = DEFAULTS
    var out = J()
    func strv(_ k: String, _ max: Int = 300) { out[k] = (r[k] as? String).map { clip(trim($0), max) } ?? (D[k] as? String ?? "") }
    func boolv(_ k: String) { out[k] = isBool(r[k]) ? (r[k] as! NSNumber).boolValue : (D[k] as? Bool ?? false) }
    func numv(_ k: String, _ lo: Int, _ hi: Int) { out[k] = clampNum(r[k], lo, hi, D[k] as! Int) }
    func pick(_ k: String, _ allowed: [String]) { out[k] = (r[k] as? String).flatMap { allowed.contains($0) ? $0 : nil } ?? (D[k] as? String ?? "") }

    strv("baseUrl")
    var baseUrl = out["baseUrl"] as! String
    if !test(baseUrl, "^https?://[^/:]+", true) { baseUrl = "" }
    out["baseUrl"] = replace(baseUrl, "/+$", "")

    strv("serverHost", 200)
    var host = out["serverHost"] as! String
    var scheme = ["http", "https"].contains(r["serverScheme"] as? String ?? "") ? r["serverScheme"] as! String : ""
    let rp = r["serverPort"]
    var port = ""
    if let rp = rp, !(rp is NSNull), !isBool(rp) {
        let s = (rp as? String) ?? (isNum(rp) ? numString((rp as! NSNumber).doubleValue) : "")
        if test(s, "^\\d{1,5}$") && s != "0" { port = s }
    }
    if let typed = match(host, "^(https?)://", true), scheme.isEmpty { scheme = typed[1].lowercased() }
    host = replace(replace(host, "^[a-z]+://", "", true), "[/?#].*$", "")
    if let hp = match(host, "^([^\\[\\]:]+):(\\d+)$") {
        host = hp[1]
        if port.isEmpty { port = hp[2] }
    }
    if host.isEmpty && !(out["baseUrl"] as! String).isEmpty { host = hostOf(out["baseUrl"] as! String) }
    out["serverHost"] = host

    boolv("monitoring")
    numv("apiVersion", 3, 4)
    strv("username", 100)
    // "none" = not picked yet. Settings from the older ZimaOS-only -board stay on ZimaOS.
    let p = r["platform"] as? String
    if let p = p, PLATFORMS[p] != nil { out["platform"] = p }
    else if p == "none" || p == "auto" { out["platform"] = "none" }
    else if isBool(r["showPower"]) { out["platform"] = "zimaos" }
    else { out["platform"] = PLATFORMS[CONFIG.platform] != nil ? CONFIG.platform : "none" }
    for k in PLATFORM_KEYS.map({ PLATFORMS[$0]!.url }) {
        strv(k)
        var u = out[k] as! String
        if !u.isEmpty && !test(u, "^https?://", true) { u = "" }
        out[k] = replace(u, "/+$", "")
    }
    // An older separate address for the server system on the same host becomes part of the server address
    if let PLo = PLATFORMS[out["platform"] as! String] {
        let u = out[PLo.url] as! String
        if !u.isEmpty && !host.isEmpty && hostOf(u).lowercased() == host.lowercased() {
            if let m = match(u, "^(https?)://[^/:]+(?::(\\d+))?", true), scheme.isEmpty && port.isEmpty {
                let sc = m[1].lowercased()
                scheme = sc == PLo.scheme ? "" : sc
                port = !m[2].isEmpty && Int(m[2]) != (PLo.ports[sc] ?? 0) ? m[2] : ""
            }
            out[PLo.url] = ""
        }
    }
    out["serverScheme"] = scheme
    out["serverPort"] = port
    strv("proxmoxNode", 64)
    out["proxmoxNode"] = replace(out["proxmoxNode"] as! String, "[^A-Za-z0-9._-]", "")
    boolv("showPower")
    if let a = r["diskMounts"] as? [Any] { out["diskMounts"] = Array(a.compactMap { $0 as? String }.filter { !$0.isEmpty }.prefix(12)) }
    else { out["diskMounts"] = D["diskMounts"] }
    numv("maxDisks", 1, 12)
    strv("networkInterface", 60)
    boolv("monitoringOpen")
    boolv("monitoringFixed")
    let t = r["tempLimits"] as? J ?? J()
    var tl = J()
    for k in ["cpu", "nvme", "hdd"] {
        let d = TEMP_DEFAULTS[k]!
        let x = t[k] as? J ?? J()
        let warm = clampNum(x["warm"], 20, 120, d.0)
        tl[k] = ["warm": warm, "hot": max(clampNum(x["hot"], 21, 130, d.1), warm + 1)]
    }
    out["tempLimits"] = tl
    numv("liveSeconds", 1, 30)
    numv("refreshMinutes", 1, 120)
    strv("linkUrl")
    boolv("widgetStatusDots")
    strv("widgetTitle", 20)
    numv("widgetRows", 1, 6)
    boolv("useLogos")
    numv("columns", 2, 3)
    numv("statusSeconds", 10, 600)
    numv("pingTimeout", 1, 15)
    pick("theme", THEME_LIST.map { $0.key })
    pick("accent", ACCENT_LIST.map { $0.0 })
    pick("glow", ["off", "soft", "normal", "strong"])
    boolv("movingBg")
    pick("cardStyle", ["subtle", "normal", "strong"])
    pick("corners", ["round", "sharp"])
    pick("font", ["system", "rounded", "serif"])
    pick("buttonSize", ["normal", "compact"])
    pick("style", ["classic", "glass"])
    boolv("splash")
    return out
}

func lookSettings(_ s: J) -> J {
    var o = J()
    for k in ["theme", "accent", "glow", "movingBg", "cardStyle", "corners", "font", "buttonSize", "style"] { o[k] = s[k] }
    return o
}

// The server's IP or name ("" = not set up yet)
func serverHostOf(_ s: J) -> String {
    let h = str(s, "serverHost")
    if !h.isEmpty { return h }
    return str(s, "baseUrl").isEmpty ? "" : hostOf(str(s, "baseUrl"))
}

// Where Glances answers: the address in Settings, or the server on Glances' usual port 61208
func glancesUrl(_ s: J) -> String {
    let b = str(s, "baseUrl")
    if !b.isEmpty { return b }
    let h = serverHostOf(s)
    return h.isEmpty ? "" : "http://\(h):61208"
}

// The server address as a full address: its http / https and port, or the usual ones for the server type
func serverUrlOf(_ s: J, _ P: String? = nil) -> String {
    let pl = PLATFORMS[P ?? str(s, "platform")]
    let scheme = str(s, "serverScheme").isEmpty ? (pl?.scheme ?? "http") : str(s, "serverScheme")
    var port = str(s, "serverPort")
    if port.isEmpty, let p = pl?.ports[scheme] { port = String(p) }
    let h = serverHostOf(s).isEmpty ? "localhost" : serverHostOf(s)
    return "\(scheme)://\(h)\(port.isEmpty ? "" : ":" + port)"
}

// The address Reboot / Shut down uses for a server type
func platformBase(_ P: String, _ s: J) -> String {
    let u = str(s, PLATFORMS[P]!.url)
    if !u.isEmpty { return replace(u, "/+$", "") }
    return serverUrlOf(s, P)
}

func splitUrl(_ url: String) -> (scheme: String, host: String, port: String, path: String) {
    if let m = match(url, "^(https?)://([^/:]+)(?::(\\d+))?(.*)$", true) { return (m[1].lowercased(), m[2], m[3], m[4]) }
    return ("http", url, "", "")
}

func formatBytes(_ b0: Double) -> String {
    let units = ["B", "KB", "MB", "GB", "TB"]
    var b = b0
    var i = 0
    while b >= 1024 && i < units.count - 1 { b /= 1024; i += 1 }
    let n = b >= 100 || i == 0 ? String(Int(jsRound(b))) : String(format: "%.1f", b)
    return "\(n) \(units[i])"
}

func colorHexFor(_ pct: Double) -> String { pct >= 85 ? "#ff5d73" : pct >= 60 ? "#ffb454" : "#34e0a1" }
