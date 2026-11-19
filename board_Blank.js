// Variables used by Scriptable.
// These must be at the very top of the file. Do not edit.
// icon-color: deep-green; icon-glyph: magic;
// -board: a live server dashboard and home screen widget for Scriptable (iOS)
//
// WHAT YOU NEED
// - The free Scriptable app on your iPhone.
// - A home server running ZimaOS, Proxmox VE, TrueNAS, Unraid, Synology DSM, OpenMediaVault,
//   or any mainstream Linux (Debian, Ubuntu Server, Fedora, ...).
// - Optional, for the Monitoring panel and the widget's gauges (Settings > Monitoring; on by default):
//   Glances running on your server with its web server / API turned on (default port 61208).
//   Docker (any OS):  docker run -d --name glances --restart unless-stopped -p 61208:61208 --pid host \
//                       -e GLANCES_OPT="-w" -v /var/run/docker.sock:/var/run/docker.sock:ro nicolargo/glances
//   (TrueNAS and Unraid: as a custom app / from Community Apps; Synology: Container Manager)
//   Debian / Ubuntu / Proxmox host without Docker:  sudo apt install glances, then run "glances -w"
//   (older distro packages are Glances 3.x: pick API version 3 in Settings).
//   A Glances container only sees the drives you give it, so mount each drive you want to watch.
// - Optional: the Reboot / Shut down menu. Pick the system your server runs in Settings > Server > Server type;
//   Settings then shows only that system's options.
//   ZimaOS   - your ZimaOS login. Nothing to install.
//   Proxmox  - the Proxmox VE API (port 8006) with your Proxmox login, e.g. root@pam. Nothing to install.
//   TrueNAS  - your TrueNAS login, over its WebSocket API (25.04+) or REST API (older, CORE). Nothing to install.
//   Unraid   - your Unraid login (root); sends what Unraid's own Reboot button sends. Nothing to install.
//   Synology - the DSM Web API (port 5000 / 5001) with an administrator account. Nothing to install.
//   OpenMediaVault - its own API with an administrator account (admin). Nothing to install.
//   Linux    - Debian, Ubuntu and other systemd distros have no web API for this, so Settings gives you a
//              one-time setup command for a tiny helper service (Python 3, one file, protected by a secret key).
//   Logins without two-factor are needed; they are stored in the iOS Keychain.
//
// SET UP
// 1. In Scriptable, tap +, paste this whole script and give it a name.
// 2. Run it. Settings opens: give the server a name, pick its type, type its address (http / https + IP or name,
//    with a port only if it isn't the usual one, e.g. 192.168.1.10:90) and its login if asked; turn Monitoring off
//    if you don't run Glances, tap "Test connection", then Save.
// 3. Add your services with the + button (name, address, port; "Auto" finds a logo).
// 4. Add a Scriptable widget to your home screen (large size shows the buttons), long-press it,
//    choose Edit Widget and pick this script.
//
// HOW TO USE
// - Tap the widget to open the live dashboard. LIVE / OFFLINE shows whether anything on the server answers.
//   The CPU / RAM / storage cards and network speed sit in the fold-away "Monitoring" panel; tap a card for
//   details, temperatures and a history graph. Monitoring off: no panel, no gauges, no requests to Glances.
// - Press and hold a button and drag to rearrange it; hold and release to edit, delete or add it to a stack.
// - Stacks group related services into one collapsible card with a combined status dot.
// - "Widget layout" arranges the large widget separately and lets you hide buttons from it.
// - Hold the server name to rename it. Tap it for Reboot / Shut down (asks to confirm).
// - Several servers: swipe the server name left / right to switch (the dots under it show how many and which
//   one is on screen); swipe left past the last one, or tap +, to add a server. Each server has its own
//   settings window (gear icon): address, type, logins, Monitoring, buttons and layout. Theme, widget and
//   button options are shared. A widget shows the server named or numbered in its Parameter (Edit Widget).
// - Settings > Theme & look > Style: "Classic" (default) or "Liquid Glass" (iOS 26 style glass, springy
//   animation on every press, menu and sheet, glass rings on the widget). "Startup animation" adds a short
//   animated logo when the dashboard opens. Both are off by default.
// - The gear icon opens Settings: server, monitoring, widget, buttons, theme & look, export / import.
//   The values in CONFIG below are only the starting defaults; Settings saves your choices on the phone.
// - Live polling only runs while the dashboard is open, so the widget stays light on battery.

// ---------- Default settings (change them here or in the dashboard's Settings) ----------
const CONFIG = {
  title: "MY SERVER",
  serverHost: "",                       // your server's IP or name, e.g. "192.168.1.10" (empty = asked on first run)
  monitoring: true,                     // Monitoring panel and widget gauges from Glances; false = no Glances requests at all
  baseUrl: "",                          // Glances address; empty = the server address on port 61208
  apiVersion: 4,                        // Glances 4.x = 4, Glances 3.x = 3
  username: "",                         // only if you enabled Glances auth
  password: "",
  diskMounts: [],                       // drives to add up, as Glances sees them (e.g. "/", "/DATA"); empty = auto-detect
  maxDisks: 3,
  networkInterface: "",                 // e.g. "eth0"; empty = busiest interface
  refreshMinutes: 5,
  widgetStatusDots: true,               // up/down dots on the large widget buttons (adds ~2s of checking per widget refresh)
  linkUrl: "",                          // corner button on the widget (e.g. your ZimaOS / Proxmox page); empty = no button
  platform: "none",                     // server type for Reboot / Shut down, picked in Settings: "none" (not picked yet), "zimaos",
                                        // "proxmox", "truenas", "unraid", "synology", "omv" (OpenMediaVault) or "linux"
  // Addresses used by Reboot / Shut down; empty = the server address with the usual port for that system
  zimaUrl: "",                          // ZimaOS: http, port 80
  proxmoxUrl: "",                       // Proxmox VE: https, port 8006
  proxmoxNode: "",                      // Proxmox node name; empty = found automatically
  truenasUrl: "",                       // TrueNAS: http, port 80
  unraidUrl: "",                        // Unraid: http, port 80
  synologyUrl: "",                      // Synology DSM: http, port 5000
  omvUrl: "",                           // OpenMediaVault: http, port 80
  helperUrl: "",                        // Linux power helper: http, port 61209
};

// Server types (Settings > Server). Monitoring works the same on all of them (Glances);
// they only change how Reboot / Shut down reaches the server.
// The server address in Settings (http / https, IP or name, optional port) is what Reboot / Shut down uses.
// scheme / ports: what's used when the address doesn't say (e.g. Proxmox: https, port 8006).
// url: an older setting with a separate address for the server system (still honoured if it names another host).
// login: name in the login popup (null = no login: the Linux helper uses its secret key)
const PLATFORMS = {
  zimaos: { name: "ZimaOS", login: "ZimaOS", url: "zimaUrl", scheme: "http", ports: {} },
  proxmox: { name: "Proxmox VE", short: "Proxmox", login: "Proxmox", url: "proxmoxUrl", scheme: "https", ports: { http: 8006, https: 8006 }, userPh: "root@pam" },
  truenas: { name: "TrueNAS", login: "TrueNAS", url: "truenasUrl", scheme: "http", ports: {}, userPh: "truenas_admin" },
  unraid: { name: "Unraid", login: "Unraid", url: "unraidUrl", scheme: "http", ports: {}, userPh: "root" },
  synology: { name: "Synology DSM", short: "Synology", login: "Synology", url: "synologyUrl", scheme: "http", ports: { http: 5000, https: 5001 } },
  omv: { name: "OpenMediaVault", short: "OMV", login: "OpenMediaVault", url: "omvUrl", scheme: "http", ports: {}, userPh: "admin" },
  linux: { name: "Linux", login: null, url: "helperUrl", scheme: "http", ports: { http: 61209, https: 61209 } },
};
const PLATFORM_KEYS = Object.keys(PLATFORMS);

// Temperatures (°C) where the number turns orange (warm) and red (hot) in the enlarged cards.
// Typical limits: most desktop CPUs throttle around 95-100°C, NVMe drives around 70-85°C,
// and hard drives are usually rated to about 55-60°C. Adjust them in Settings for your hardware.
const TEMP_DEFAULTS = {
  cpu: { warm: 70, hot: 85 },
  nvme: { warm: 60, hot: 70 },
  hdd: { warm: 45, hot: 55 },
};

// In-memory copy of the data file, and the server shown (declared before anything reads the settings)
let dataRaw = null;
let CUR = null;
// Settings that belong to one server (everything else in Settings is shared by all servers), and the parts
// of the data file each server has for itself (see "Servers" below)
const SERVER_SETTING_KEYS = [
  "serverHost", "serverScheme", "serverPort", "monitoring", "baseUrl", "apiVersion", "username", "platform", "zimaUrl", "proxmoxUrl", "proxmoxNode",
  "truenasUrl", "unraidUrl", "synologyUrl", "omvUrl", "helperUrl", "showPower", "diskMounts", "maxDisks",
  "networkInterface", "monitoringOpen", "monitoringFixed", "tempLimits", "liveSeconds",
];
const SERVER_PARTS = ["custom", "edits", "hidden", "order", "widgetOrder", "widgetHidden", "groups"];
function isObj(x) {
  return !!x && typeof x === "object" && !Array.isArray(x);
}

// ---------- Themes (Settings > Theme & look) ----------
// g1 / g2: the two background glows (r,g,b). panel / panel2: popup and dropdown backgrounds.
const THEMES = {
  aurora: { name: "Aurora", bg: "#0b1220", g1: "18,194,165", g2: "106,92,255", acc: "#34e0a1", ink: "#06281d", panel: "rgba(30,38,64,.94)", panel2: "#1e2640" },
  ocean: { name: "Ocean", bg: "#07111f", g1: "0,132,255", g2: "0,206,209", acc: "#4fc3ff", ink: "#04203a", panel: "rgba(20,40,68,.94)", panel2: "#142844" },
  sunset: { name: "Sunset", bg: "#150d12", g1: "255,122,69", g2: "214,51,132", acc: "#ff9f6b", ink: "#3a1606", panel: "rgba(54,30,40,.94)", panel2: "#361e28" },
  forest: { name: "Forest", bg: "#09140f", g1: "46,160,67", g2: "18,120,140", acc: "#7ddc6f", ink: "#0d2a0a", panel: "rgba(24,44,36,.94)", panel2: "#182c24" },
  rose: { name: "Rose", bg: "#140b18", g1: "255,92,160", g2: "124,77,255", acc: "#ff7eb6", ink: "#3a0b22", panel: "rgba(48,28,58,.94)", panel2: "#301c3a" },
  graphite: { name: "Graphite", bg: "#111317", g1: "120,130,150", g2: "70,80,100", acc: "#d6dbe4", ink: "#15181e", panel: "rgba(36,38,44,.95)", panel2: "#24262c" },
  oled: { name: "OLED black", bg: "#000000", g1: "0,0,0", g2: "0,0,0", acc: "#34e0a1", ink: "#06281d", panel: "rgba(22,22,26,.97)", panel2: "#16161a", noGlow: true },
};
// "" = the theme's own accent
const ACCENTS = {
  "": { name: "Theme", c: null },
  green: { name: "Green", c: "#34e0a1", ink: "#06281d" },
  blue: { name: "Blue", c: "#4fc3ff", ink: "#04203a" },
  purple: { name: "Purple", c: "#a78bfa", ink: "#1e1240" },
  orange: { name: "Orange", c: "#ff9f6b", ink: "#3a1606" },
  pink: { name: "Pink", c: "#ff7eb6", ink: "#3a0b22" },
  white: { name: "White", c: "#e8ecf3", ink: "#15181e" },
};
const GLOW_LEVELS = { off: 0, soft: 0.3, normal: 0.5, strong: 0.75 };

// ---------- Settings (gear icon in the live dashboard) ----------
const DEFAULTS = {
  serverHost: CONFIG.serverHost,
  serverScheme: "",                // "" = the usual one for the server type, or "http" / "https"
  serverPort: "",                  // "" = the usual one for the server type
  monitoring: CONFIG.monitoring,
  baseUrl: CONFIG.baseUrl,
  apiVersion: CONFIG.apiVersion,
  username: CONFIG.username,
  platform: CONFIG.platform,
  zimaUrl: CONFIG.zimaUrl,
  proxmoxUrl: CONFIG.proxmoxUrl,
  proxmoxNode: CONFIG.proxmoxNode,
  truenasUrl: CONFIG.truenasUrl,
  unraidUrl: CONFIG.unraidUrl,
  synologyUrl: CONFIG.synologyUrl,
  omvUrl: CONFIG.omvUrl,
  helperUrl: CONFIG.helperUrl,
  showPower: true,                 // Reboot / Shut down under the server name
  diskMounts: CONFIG.diskMounts,
  maxDisks: CONFIG.maxDisks,       // used when no drives are picked (auto-detect)
  networkInterface: CONFIG.networkInterface,
  monitoringOpen: false,           // Monitoring panel open when the dashboard starts
  monitoringFixed: false,          // Monitoring panel always open (no button to fold it away)
  tempLimits: TEMP_DEFAULTS,
  liveSeconds: 3,                  // dashboard update interval
  refreshMinutes: CONFIG.refreshMinutes,
  linkUrl: CONFIG.linkUrl,
  widgetStatusDots: CONFIG.widgetStatusDots,
  widgetTitle: "",                 // empty = same as the dashboard name
  widgetRows: 6,                   // rows of 4 buttons on the large widget
  useLogos: true,
  columns: 2,                      // dashboard button columns
  statusSeconds: 30,               // how often the up/down dots are checked in the dashboard
  pingTimeout: 3,                  // seconds without an answer before a service counts as down
  theme: "aurora",
  accent: "",                      // "" = the theme's accent
  glow: "normal",                  // background glow: off / soft / normal / strong
  movingBg: true,                  // drifting background glow in the dashboard
  cardStyle: "normal",             // card glass: subtle / normal / strong
  corners: "round",                // round / sharp
  font: "system",                  // system / rounded / serif
  buttonSize: "normal",            // normal / compact
  style: "classic",                // classic (the plain -board look) / glass (Liquid Glass: iOS 26 style glass and motion)
  splash: false,                   // short animated startup screen when the dashboard opens
};

function clampNum(v, lo, hi, def) {
  const n = Number(v);
  return v !== "" && v != null && isFinite(n) ? Math.min(hi, Math.max(lo, Math.round(n))) : def;
}

function cleanSettings(raw) {
  const r = raw && typeof raw === "object" ? raw : {};
  const out = {};
  const str = (k, max) => (out[k] = typeof r[k] === "string" ? r[k].trim().slice(0, max || 300) : DEFAULTS[k]);
  const bool = (k) => (out[k] = typeof r[k] === "boolean" ? r[k] : DEFAULTS[k]);
  const num = (k, lo, hi) => (out[k] = clampNum(r[k], lo, hi, DEFAULTS[k]));
  str("baseUrl");
  if (!/^https?:\/\/[^\/:]+/i.test(out.baseUrl)) out.baseUrl = "";
  out.baseUrl = out.baseUrl.replace(/\/+$/, "");
  // the server's IP or name only (a pasted address is trimmed to its host); settings from before it
  // existed take it from their Glances address
  // the server address: IP or name, plus http / https and a port when they aren't the usual ones
  str("serverHost", 200);
  out.serverScheme = ["http", "https"].includes(r.serverScheme) ? r.serverScheme : "";
  out.serverPort = /^\d{1,5}$/.test(String(r.serverPort || "")) ? String(r.serverPort) : "";
  const typed = out.serverHost.match(/^(https?):\/\//i);
  if (typed && !out.serverScheme) out.serverScheme = typed[1].toLowerCase();
  out.serverHost = out.serverHost.replace(/^[a-z]+:\/\//i, "").replace(/[\/?#].*$/, "");
  const hp = out.serverHost.match(/^([^\[\]:]+):(\d+)$/);
  if (hp) {
    out.serverHost = hp[1];
    if (!out.serverPort) out.serverPort = hp[2];
  }
  if (!out.serverHost && out.baseUrl) out.serverHost = hostOf(out.baseUrl);
  bool("monitoring");
  num("apiVersion", 3, 4);
  str("username", 100);
  // "none" = not picked yet. Settings saved by the older ZimaOS-only -board
  // (they have showPower but no platform) stay on ZimaOS, so their Reboot menu keeps working.
  out.platform = PLATFORMS[r.platform]
    ? r.platform
    : r.platform === "none" || r.platform === "auto" ? "none" : typeof r.showPower === "boolean" ? "zimaos" : PLATFORMS[DEFAULTS.platform] ? DEFAULTS.platform : "none";
  for (const k of PLATFORM_KEYS.map((x) => PLATFORMS[x].url)) {
    str(k);
    if (out[k] && !/^https?:\/\//i.test(out[k])) out[k] = "";
    out[k] = out[k].replace(/\/+$/, "");
  }
  // An older separate address for the server system on the same host becomes part of the server address
  // (http / https and port), so there's only one address to keep up to date
  const PLo = PLATFORMS[out.platform];
  if (PLo && out[PLo.url] && out.serverHost && hostOf(out[PLo.url]).toLowerCase() === out.serverHost.toLowerCase()) {
    const m = out[PLo.url].match(/^(https?):\/\/[^\/:]+(?::(\d+))?/i);
    if (m && !out.serverScheme && !out.serverPort) {
      const sc = m[1].toLowerCase();
      out.serverScheme = sc === PLo.scheme ? "" : sc;
      out.serverPort = m[2] && Number(m[2]) !== (PLo.ports[sc] || 0) ? m[2] : "";
    }
    out[PLo.url] = "";
  }
  str("proxmoxNode", 64);
  out.proxmoxNode = out.proxmoxNode.replace(/[^A-Za-z0-9._-]/g, "");
  bool("showPower");
  out.diskMounts = Array.isArray(r.diskMounts)
    ? r.diskMounts.filter((x) => typeof x === "string" && x).slice(0, 12)
    : DEFAULTS.diskMounts;
  num("maxDisks", 1, 12);
  str("networkInterface", 60);
  bool("monitoringOpen");
  bool("monitoringFixed");
  const t = r.tempLimits && typeof r.tempLimits === "object" ? r.tempLimits : {};
  out.tempLimits = {};
  for (const k of ["cpu", "nvme", "hdd"]) {
    const d = TEMP_DEFAULTS[k];
    const x = t[k] || {};
    const warm = clampNum(x.warm, 20, 120, d.warm);
    out.tempLimits[k] = { warm, hot: Math.max(clampNum(x.hot, 21, 130, d.hot), warm + 1) };
  }
  num("liveSeconds", 1, 30);
  num("refreshMinutes", 1, 120);
  str("linkUrl");
  bool("widgetStatusDots");
  str("widgetTitle", 20);
  num("widgetRows", 1, 6);
  bool("useLogos");
  num("columns", 2, 3);
  num("statusSeconds", 10, 600);
  num("pingTimeout", 1, 15);
  const pick = (k, allowed) => (out[k] = allowed.includes(r[k]) ? r[k] : DEFAULTS[k]);
  pick("theme", Object.keys(THEMES));
  pick("accent", Object.keys(ACCENTS));
  pick("glow", Object.keys(GLOW_LEVELS));
  bool("movingBg");
  pick("cardStyle", ["subtle", "normal", "strong"]);
  pick("corners", ["round", "sharp"]);
  pick("font", ["system", "rounded", "serif"]);
  pick("buttonSize", ["normal", "compact"]);
  pick("style", ["classic", "glass"]);
  bool("splash");
  return out;
}

function loadSettings() {
  return cleanSettings(loadData().settings);
}

function lookSettings(s) {
  const o = {};
  for (const k of ["theme", "accent", "glow", "movingBg", "cardStyle", "corners", "font", "buttonSize", "style"]) o[k] = s[k];
  return o;
}

function hostOf(url) {
  const m = String(url).match(/^https?:\/\/([^:\/]+)/i);
  return m ? m[1] : "localhost";
}

// The server's IP or name ("" = not set up yet)
function serverHostOf(s) {
  return s.serverHost || (s.baseUrl ? hostOf(s.baseUrl) : "");
}

// Where Glances answers: the address in Settings, or the server on Glances' usual port 61208
function glancesUrl(s) {
  const h = serverHostOf(s);
  return s.baseUrl || (h ? `http://${h}:61208` : "");
}

CUR = initialServer();
let SET = loadSettings();

// Services shown as buttons in the live dashboard.
// Ports are defaults; edit any that differ on your server.
// Optional fields: path ("/admin"), https (true)
// logo: image URL for the button (emoji "icon" is the fallback). Logos come from
// the free dashboard-icons set: https://github.com/homarr-labs/dashboard-icons
let HOST = serverHostOf(SET) || "localhost"; // built-in buttons use the server address
const LOGO_CDN = "https://cdn.jsdelivr.net/gh/homarr-labs/dashboard-icons/png/";
// Built-in buttons. Empty on purpose: add your own with the + button in the dashboard.
// You can also list them here, e.g. { name: "Jellyfin", port: 8096, icon: "🎬", logo: LOGO_CDN + "jellyfin.png" }
// (optional: path: "/admin", https: true). They use the server address from Settings.
const BUILTIN = [];
function buildServices() {
  return BUILTIN.map((s) => ({
    ...s,
    id: "b:" + s.name,
    url: `${s.https ? "https" : "http"}://${HOST}:${s.port}${s.path || ""}`,
  }));
}
let SERVICES = buildServices();

// After Settings are saved (or imported): use the new values right away
function applySettings() {
  SET = loadSettings();
  HOST = serverHostOf(SET) || "localhost";
  SERVICES = buildServices();
}


// ---------- Saved button changes (made in the live dashboard, stored on the phone) ----------
// custom: buttons you added; edits: changes to built-in buttons; hidden: deleted built-in buttons
function dataPath() {
  const fm = FileManager.local();
  return fm.joinPath(fm.documentsDirectory(), "board-data.json");
}

// The data file is read from storage once per run and kept in memory (saveData updates the copy).
// loadData still returns a fresh object every time, so callers can change it freely.
function readDataFile() {
  if (dataRaw === null) {
    const fm = FileManager.local();
    const p = dataPath();
    dataRaw = fm.fileExists(p) ? fm.readString(p) || "" : "";
  }
  return dataRaw;
}

// ---------- Servers ----------
// The data file holds every server:
//   { v: 2, current (the server shown last), settings (shared by all servers),
//     servers: [ { id, title, settings (this server's own), custom, edits, hidden, order, widgetOrder, widgetHidden, groups } ] }
// loadData() / saveData() work on one server at a time (CUR) in the same shape as before:
// the shared and the server's own settings come merged in .settings.
// A file from before there were several servers becomes server 1 ("s1").

function readStore() {
  let d = {};
  try {
    const raw = readDataFile();
    d = raw ? JSON.parse(raw) : {};
  } catch (e) {
    d = {};
  }
  if (Array.isArray(d)) d = { custom: d };
  if (!Array.isArray(d.servers)) {
    const st = isObj(d.settings) ? d.settings : {};
    const own = {};
    const shared = {};
    for (const k of Object.keys(st)) (SERVER_SETTING_KEYS.includes(k) ? own : shared)[k] = st[k];
    const s1 = { id: "s1", title: typeof d.title === "string" ? d.title : "", settings: own };
    for (const k of SERVER_PARTS) if (d[k] !== undefined) s1[k] = d[k];
    d = { v: 2, current: "s1", settings: shared, servers: [s1] };
  }
  d.settings = isObj(d.settings) ? d.settings : {};
  d.servers = d.servers.filter((x) => isObj(x) && typeof x.id === "string");
  if (!d.servers.length) d.servers = [{ id: "s1", title: "", settings: {} }];
  for (const x of d.servers) x.settings = isObj(x.settings) ? x.settings : {};
  if (!d.servers.some((x) => x.id === d.current)) d.current = d.servers[0].id;
  return d;
}

function writeStore(st) {
  const raw = JSON.stringify(st);
  FileManager.local().writeString(dataPath(), raw);
  dataRaw = raw;
}

function curServer(st) {
  return st.servers.find((x) => x.id === CUR) || st.servers.find((x) => x.id === st.current) || st.servers[0];
}

// [{ id, title }] in order, with the name each one shows
function listServers() {
  return readStore().servers.map((x, i) => ({ id: x.id, title: x.title || (i === 0 ? CONFIG.title : "SERVER " + (i + 1)) }));
}

// Which server this run starts on. The widget: its Parameter (Edit Widget) as the server's name or number
// (1, 2, ...), else the first server. The dashboard: ?server= from the widget, else the one shown last.
function initialServer() {
  const st = readStore();
  const inWidget = typeof config !== "undefined" && config.runsInWidget;
  const want = String((inWidget ? args.widgetParameter : (args.queryParameters || {}).server) || "").trim();
  if (want) {
    const n = Number(want);
    const hit =
      st.servers.find((x) => x.id === want) ||
      (Number.isInteger(n) && n >= 1 ? st.servers[n - 1] : null) ||
      st.servers.find((x, i) => (x.title || (i === 0 ? CONFIG.title : "")).toLowerCase() === want.toLowerCase());
    if (hit) return hit.id;
  }
  return inWidget ? st.servers[0].id : st.current;
}

// Show another server (remembered for the next time the dashboard opens)
function setCurrentServer(id) {
  const st = readStore();
  if (!st.servers.some((x) => x.id === id)) return false;
  CUR = id;
  st.current = id;
  writeStore(st);
  applySettings();
  return true;
}

// own: the new server's settings (address, type ...)
function addServer(name, own) {
  const st = readStore();
  const id = "s" + Date.now().toString(36);
  const n = st.servers.length + 1;
  st.servers.push({ id, title: String(name || "").trim().slice(0, 20) || "SERVER " + n, settings: { platform: "none", ...(isObj(own) ? own : {}) } });
  writeStore(st);
  setCurrentServer(id);
  return id;
}

// Removes a server, its buttons and layout, and the logins saved for it
function removeServer(id) {
  const st = readStore();
  if (st.servers.length < 2) return false;
  const i = st.servers.findIndex((x) => x.id === id);
  if (i < 0) return false;
  forgetServerKeys(id);
  st.servers.splice(i, 1);
  const next = st.servers[Math.min(i, st.servers.length - 1)].id;
  st.current = next;
  writeStore(st);
  CUR = next;
  applySettings();
  return true;
}

function loadData() {
  const st = readStore();
  const sv = curServer(st);
  const d = { title: typeof sv.title === "string" ? sv.title : "", settings: { ...st.settings, ...sv.settings } };
  for (const k of SERVER_PARTS) d[k] = sv[k];
  d.custom = (Array.isArray(d.custom) ? d.custom : []).map((c, i) => (c.id ? c : { ...c, id: "c:legacy" + i }));
  d.edits = isObj(d.edits) ? d.edits : {};
  d.hidden = Array.isArray(d.hidden) ? d.hidden : [];
  d.order = Array.isArray(d.order) ? d.order : [];
  d.widgetOrder = Array.isArray(d.widgetOrder) ? d.widgetOrder : [];
  d.widgetHidden = Array.isArray(d.widgetHidden) ? d.widgetHidden : [];
  // stacks: { id: "g:…", name, logo, expand (show members on the widget), members: [button ids] }
  d.groups = (Array.isArray(d.groups) ? d.groups : [])
    .filter((g) => g && typeof g.id === "string")
    .map((g) => ({
      id: g.id,
      name: String(g.name || "Stack").slice(0, 24),
      logo: String(g.logo || ""),
      expand: !!g.expand,
      members: Array.isArray(g.members) ? g.members.filter((x) => typeof x === "string") : [],
    }));
  return d;
}

// Dashboard / widget name of the current server (renamed by holding the header in the live dashboard)
function getTitle() {
  const st = readStore();
  const sv = curServer(st);
  return sv.title || (st.servers.indexOf(sv) === 0 ? CONFIG.title : "SERVER " + (st.servers.indexOf(sv) + 1));
}

// Saves the current server's part (and the shared settings) back into the file
function saveData(d) {
  const st = readStore();
  const sv = curServer(st);
  if (typeof d.title === "string") sv.title = d.title;
  for (const k of SERVER_PARTS) if (d[k] !== undefined) sv[k] = d[k];
  if (isObj(d.settings)) {
    const own = {};
    const shared = { ...st.settings };
    for (const k of Object.keys(d.settings)) {
      if (SERVER_SETTING_KEYS.includes(k)) own[k] = d.settings[k];
      else shared[k] = d.settings[k];
    }
    sv.settings = own;
    st.settings = shared;
  }
  writeStore(st);
}

function splitUrl(url) {
  const m = String(url).match(/^(https?):\/\/([^\/:]+)(?::(\d+))?(.*)$/i);
  return m
    ? { scheme: m[1].toLowerCase(), host: m[2], port: m[3] || "", path: m[4] || "" }
    : { scheme: "http", host: String(url), port: "", path: "" };
}

function sortByOrder(list, order) {
  if (!order.length) return list;
  const idx = (s) => {
    const i = order.indexOf(s.id);
    return i < 0 ? 1e9 : i;
  };
  return list
    .map((s, i) => ({ s, i }))
    .sort((a, b) => idx(a.s) - idx(b.s) || a.i - b.i)
    .map((x) => x.s);
}

// All buttons: built-in (minus deleted, with edits applied), then custom.
// forDashboard = true applies your dashboard drag-and-drop order.
function getServices(forDashboard) {
  const d = loadData();
  const base = SERVICES.filter((s) => !d.hidden.includes(s.id)).map((s) =>
    d.edits[s.id] ? { ...s, ...d.edits[s.id] } : s
  );
  const list = base.concat(d.custom).map((s) => {
    const u = splitUrl(s.url);
    return {
      ...s,
      scheme: u.scheme,
      addr: u.host + u.path,
      label: s.port ? ":" + s.port : u.host,
      onWidget: !d.widgetHidden.includes(s.id),
    };
  });
  return forDashboard ? sortByOrder(list, d.order) : list;
}

// ---------- Stacks (groups of connected services) ----------
const STACK_ICON = "🗂️";

// Top level = buttons not in a stack, plus stacks (each stack carries its member buttons).
// dash: dashboard order. widget / hidden: large widget order and the entries hidden from it.
function getLayout() {
  const d = loadData();
  const all = getServices(false);
  const byId = {};
  for (const s of all) byId[s.id] = s;
  const inStack = new Set();
  const groups = d.groups.map((g) => {
    const members = g.members.map((id) => byId[id]).filter((s) => s && !inStack.has(s.id));
    for (const s of members) {
      inStack.add(s.id);
      s.group = g.id;
    }
    return {
      id: g.id,
      kind: "group",
      name: g.name,
      logo: g.logo,
      icon: STACK_ICON,
      expand: g.expand,
      members,
      onWidget: !d.widgetHidden.includes(g.id),
    };
  });
  const top = all.filter((s) => !inStack.has(s.id)).concat(groups);
  return {
    all,
    groups,
    dash: sortByOrder(top, d.order),
    widget: sortByOrder(top.filter((x) => x.onWidget), d.widgetOrder),
    hidden: top.filter((x) => !x.onWidget),
  };
}

// What the large widget actually draws: stacks become one pill, or their members if "show members" is on
function widgetCells(layout) {
  const cells = [];
  for (const x of layout.widget) {
    if (x.kind !== "group") cells.push(x);
    else if (x.expand) cells.push(...x.members);
    else if (x.members.length) cells.push(x);
  }
  return cells.slice(0, SET.widgetRows * 4);
}

// Create / edit / ungroup stacks and move buttons in or out of them
function stackAction(a) {
  const L = getLayout();
  const data = loadData();
  // freeze the current order so a new stack lands where its first button was
  data.order = L.dash.map((x) => x.id);
  data.widgetOrder = L.widget.map((x) => x.id);
  const strip = (id) => data.groups.forEach((g) => (g.members = g.members.filter((x) => x !== id)));
  const holder = (id) => data.groups.find((g) => g.members.includes(id));
  const insertAt = (list, anchor, ids, after) => {
    const out = list.filter((x) => !ids.includes(x));
    let i = anchor ? out.indexOf(anchor) : -1;
    if (i < 0) return out.concat(ids);
    if (after) i++;
    out.splice(i, 0, ...ids);
    return out;
  };
  const replaceIn = (list, oldId, ids) => insertAt(list, oldId, ids, false).filter((x) => x !== oldId);

  if (a.act === "save") {
    const name = String(a.name || "").trim().slice(0, 24) || "Stack";
    const logo = String(a.logo || "").trim();
    let g = data.groups.find((x) => x.id === a.gid);
    if (g) {
      g.name = name;
      g.logo = logo;
      g.expand = !!a.expand;
    } else {
      g = { id: "g:" + Date.now(), name, logo, expand: !!a.expand, members: [] };
      if (a.member) {
        const old = holder(a.member);
        strip(a.member);
        if (old) {
          data.order = insertAt(data.order, old.id, [g.id], true);
          data.widgetOrder = insertAt(data.widgetOrder, old.id, [g.id], true);
        } else {
          data.order = replaceIn(data.order, a.member, [g.id]);
          data.widgetOrder = replaceIn(data.widgetOrder, a.member, [g.id]);
        }
        g.members.push(a.member);
      } else {
        data.order.push(g.id);
        data.widgetOrder.push(g.id);
      }
      data.groups.push(g);
    }
    clearLogoFail(logo);
  } else if (a.act === "move") {
    const old = holder(a.id);
    strip(a.id);
    const to = a.to ? data.groups.find((x) => x.id === a.to) : null;
    if (to) {
      to.members.push(a.id);
      data.order = data.order.filter((x) => x !== a.id);
      data.widgetOrder = data.widgetOrder.filter((x) => x !== a.id);
    } else {
      // out of the stack: placed right after it
      data.order = insertAt(data.order, old ? old.id : null, [a.id], true);
      data.widgetOrder = insertAt(data.widgetOrder, old ? old.id : null, [a.id], true);
    }
  } else if (a.act === "del") {
    // ungroup: the buttons go back where the stack was
    const g = data.groups.find((x) => x.id === a.gid);
    if (g) {
      data.order = replaceIn(data.order, g.id, g.members);
      data.widgetOrder = replaceIn(data.widgetOrder, g.id, g.members);
      if (data.widgetHidden.includes(g.id)) {
        data.widgetHidden = data.widgetHidden.filter((x) => x !== g.id).concat(g.members);
      }
      data.groups = data.groups.filter((x) => x.id !== g.id);
    }
  }
  saveData(data);
}

function getPayload() {
  const L = getLayout();
  // cached logos as base64, sent once per button / stack id (only ones already downloaded)
  const logos = {};
  for (const s of SET.useLogos ? L.all.concat(L.groups) : []) {
    const b = logoBase64(s.logo);
    if (b) logos[s.id] = b;
  }
  return {
    dash: L.dash,
    widget: L.widget,
    hidden: L.hidden,
    limits: SET.tempLimits,
    logos,
    cdn: LOGO_CDN,
    hasLogin: PLATFORMS[SET.platform] ? hasLogin() : false,
    title: getTitle(),
    host: serverHostOf(SET),
    servers: listServers(),
    cur: CUR,
    platforms: PLATFORMS,
    ui: {
      cols: SET.columns,
      // the Reboot / Shut down menu needs a known server type
      power: SET.showPower && !!PLATFORMS[SET.platform],
      platform: SET.platform,
      login: PLATFORMS[SET.platform] ? PLATFORMS[SET.platform].login : null,  // name in the login popup; null = no login
      userPh: PLATFORMS[SET.platform] ? PLATFORMS[SET.platform].userPh || "" : "",
      widgetMax: SET.widgetRows * 4,
      monitoring: SET.monitoring,
      monOpen: SET.monitoringOpen,
      monFixed: SET.monitoringFixed,
      look: lookSettings(SET),
      splash: SET.splash,
      themes: THEMES,
      accents: ACCENTS,
      glows: GLOW_LEVELS,
    },
  };
}

// ---------- Reboot / shut down ----------
// Every server type has a driver: run(action, user, pass, base, s). action "restart" / "off" sends the command;
// action null only checks the login (for Test connection). It returns { ok, msg } or { ok: false, auth, net, msg }.
// ZimaOS:   log in (POST /v1/users/login), then PUT /v1/sys/state/restart or /off
// Proxmox:  log in (POST /api2/json/access/ticket), then POST /api2/json/nodes/<node>/status command=reboot|shutdown
// TrueNAS:  JSON-RPC over WebSocket (/api/current: auth.login, then system.reboot / system.shutdown);
//           older versions without it: the REST API (/api/v2.0/system/reboot or /shutdown)
// Unraid:   log in (POST /login), read the page's csrf_token, then POST /webGui/include/Boot.php cmd=reboot|shutdown
//           (what Unraid's own Reboot button does)
// Synology: DSM Web API: SYNO.API.Auth login, then SYNO.Core.System reboot / shutdown
// OpenMediaVault: rpc.php: Session.login, then System.reboot / System.shutdown
// Linux:    the small "board-power" helper (set up from Settings), POST /reboot or /shutdown with its secret key
const LOGIN_KEYS = {
  zimaos: { user: "board-zima-user", pass: "board-zima-pass" },
  proxmox: { user: "board-pve-user", pass: "board-pve-pass" },
  truenas: { user: "board-truenas-user", pass: "board-truenas-pass" },
  unraid: { user: "board-unraid-user", pass: "board-unraid-pass" },
  synology: { user: "board-syno-user", pass: "board-syno-pass" },
  omv: { user: "board-omv-user", pass: "board-omv-pass" },
};
const HELPER_KEY_NAME = "board-helper-key";
const GLANCES_PASS_NAME = "board-glances-pass";

// Every server keeps its own logins. Server 1 ("s1") uses the names from before there were several servers.
function kname(base, id = CUR) {
  return !id || id === "s1" ? base : `${base}@${id}`;
}
function loginKeys(platform, id = CUR) {
  const k = LOGIN_KEYS[platform];
  return k ? { user: kname(k.user, id), pass: kname(k.pass, id) } : null;
}
const helperKey = (id = CUR) => kname(HELPER_KEY_NAME, id);
const gpassKey = (id = CUR) => kname(GLANCES_PASS_NAME, id);

// Everything saved in the Keychain for one server (when it's removed)
function forgetServerKeys(id) {
  const names = [helperKey(id), gpassKey(id)];
  for (const p of Object.keys(LOGIN_KEYS)) names.push(loginKeys(p, id).user, loginKeys(p, id).pass);
  for (const n of names) kcRemove(n);
}
const HELPER_PORT = 61209;

function kcHas(k) {
  try {
    return Keychain.contains(k);
  } catch (e) {
    return false;
  }
}

function kcRemove(k) {
  try {
    if (Keychain.contains(k)) Keychain.remove(k);
  } catch (e) {}
}

// Linux: "logged in" = the helper's secret key is saved
function hasLogin(platform = SET.platform) {
  if (platform === "linux") return kcHas(helperKey());
  const k = loginKeys(platform);
  return !!k && kcHas(k.user) && kcHas(k.pass);
}

function forgetLogin(platform = SET.platform) {
  if (platform === "linux") return kcRemove(helperKey());
  const k = loginKeys(platform);
  if (k) for (const x of Object.values(k)) kcRemove(x);
}

function savedLogin(platform) {
  const k = loginKeys(platform);
  return { user: Keychain.get(k.user), pass: Keychain.get(k.pass) };
}

// The address Reboot / Shut down uses for a server type: the one in Settings, or the usual one on the server address
function platformBase(P, s = SET) {
  const pl = PLATFORMS[P];
  // an older separate address on another host still wins
  if (s[pl.url]) return s[pl.url].replace(/\/+$/, "");
  return serverUrlOf(s, P);
}

// The server address as a full address: its http / https and port, or the usual ones for the server type
function serverUrlOf(s = SET, P = s.platform) {
  const pl = PLATFORMS[P];
  const scheme = s.serverScheme || (pl ? pl.scheme : "http");
  const port = s.serverPort || (pl && pl.ports[scheme]) || "";
  return `${scheme}://${serverHostOf(s) || "localhost"}${port ? ":" + port : ""}`;
}

const powerSent = (action) =>
  action === "off"
    ? "Shutdown sent. Switch the server on by hand to bring it back."
    : "Reboot sent. The server should be back in a minute or two.";

const formBody = (o) => Object.keys(o).map((k) => encodeURIComponent(k) + "=" + encodeURIComponent(o[k])).join("&");

// "name=value; ..." from the cookies a response set
function cookiesOf(req) {
  const list = (req.response && req.response.cookies) || [];
  return list.map((c) => `${c.name}=${c.value}`).join("; ");
}

// A request that accepts self-signed certificates (most NAS systems use one out of the box)
function nasRequest(url, method, timeout) {
  const req = new Request(url);
  req.allowInsecureRequest = true;
  req.timeoutInterval = timeout || 8;
  if (method) req.method = method;
  return req;
}

// After the command: a dropped connection is normal (the server is going down)
async function sendCommand(req, action, name) {
  try {
    await req.loadString();
  } catch (e) {
    return { ok: true, msg: powerSent(action) };
  }
  const code = req.response && req.response.statusCode;
  if (code === 401) return { ok: false, auth: true, msg: `${name} refused the login. Enter it again.` };
  if (code && code >= 300) return { ok: false, msg: `${name} didn't accept the command (HTTP ${code}).` };
  return { ok: true, msg: powerSent(action) };
}

// --- ZimaOS ---
async function zimaLogin(user, pass, base) {
  const login = new Request(base + "/v1/users/login");
  login.method = "POST";
  login.headers = { "Content-Type": "application/json" };
  login.body = JSON.stringify({ username: user, password: pass });
  login.timeoutInterval = 8;
  let r;
  try {
    r = await login.loadJSON();
  } catch (e) {
    return { net: true, error: "Can't reach ZimaOS at " + base };
  }
  const token = r && r.data && r.data.token && r.data.token.access_token;
  if (!token) return { auth: true, error: "Login failed: " + ((r && r.message) || "wrong username or password") };
  return { token };
}

async function zimaRun(action, user, pass, base) {
  const lg = await zimaLogin(user, pass, base);
  if (!lg.token) return { ok: false, auth: !!lg.auth, net: !!lg.net, msg: lg.error };
  if (!action) return { ok: true, msg: "saved login works" };
  const req = new Request(`${base}/v1/sys/state/${action === "off" ? "off" : "restart"}`);
  req.method = "PUT";
  req.headers = { Authorization: lg.token };
  req.timeoutInterval = 8;
  const r = await sendCommand(req, action, "ZimaOS");
  if (r.auth || /HTTP 403/.test(r.msg)) return { ok: false, auth: true, msg: "ZimaOS refused the login. Enter it again." };
  return r;
}

// --- Proxmox VE ---
// "root" -> "root@pam" (the Linux login realm Proxmox uses by default)
function pveUser(u) {
  u = String(u || "").trim();
  return u && !u.includes("@") ? u + "@pam" : u;
}

function pveRequest(url, auth) {
  const req = nasRequest(url);
  if (auth) req.headers = { Cookie: "PVEAuthCookie=" + auth.ticket, CSRFPreventionToken: auth.csrf };
  return req;
}

async function pveLogin(user, pass, base) {
  const req = pveRequest(base + "/api2/json/access/ticket");
  req.method = "POST";
  req.headers = { "Content-Type": "application/x-www-form-urlencoded" };
  req.body = formBody({ username: pveUser(user), password: pass });
  let r;
  try {
    r = await req.loadJSON();
  } catch (e) {
    return { net: true, error: "Can't reach Proxmox at " + base };
  }
  const d = r && r.data;
  if (d && d.NeedTFA) return { auth: true, error: "This account uses two-factor login. Use an account without it (e.g. a separate user with the Sys.PowerMgmt permission)." };
  if (!d || !d.ticket) return { auth: true, error: "Login failed: wrong username or password (e.g. root@pam)" };
  return { ticket: d.ticket, csrf: d.CSRFPreventionToken };
}

// The node to reboot: the one named in Settings, the only one, or the one at the Glances address
async function pveNode(base, auth, s = SET) {
  if (s.proxmoxNode) return { node: s.proxmoxNode };
  let nodes = [];
  try {
    const r = await pveRequest(base + "/api2/json/nodes", auth).loadJSON();
    nodes = ((r && r.data) || []).map((n) => n.node).filter(Boolean);
  } catch (e) {
    return { error: "Couldn't list the Proxmox nodes" };
  }
  if (nodes.length === 1) return { node: nodes[0] };
  if (!nodes.length) return { error: "Proxmox didn't list any nodes. Enter the node name in Settings." };
  const host = serverHostOf(s).toLowerCase();
  let hit = nodes.find((n) => n.toLowerCase() === host || host.startsWith(n.toLowerCase() + "."));
  if (!hit) {
    try {
      const r = await pveRequest(base + "/api2/json/cluster/status", auth).loadJSON();
      const m = ((r && r.data) || []).find((x) => x.type === "node" && x.ip === host);
      if (m) hit = m.name;
    } catch (e) {}
  }
  return hit ? { node: hit } : { error: `Several Proxmox nodes found (${nodes.join(", ")}). Enter the node name in Settings.` };
}

async function pveRun(action, user, pass, base, s) {
  const lg = await pveLogin(user, pass, base);
  if (!lg.ticket) return { ok: false, auth: !!lg.auth, net: !!lg.net, msg: lg.error };
  const nd = await pveNode(base, lg, s);
  if (!nd.node) return { ok: false, msg: nd.error };
  if (!action) return { ok: true, msg: `saved login works (node ${nd.node})` };
  const req = pveRequest(`${base}/api2/json/nodes/${encodeURIComponent(nd.node)}/status`, lg);
  req.method = "POST";
  req.headers = { ...req.headers, "Content-Type": "application/x-www-form-urlencoded" };
  req.body = "command=" + (action === "off" ? "shutdown" : "reboot");
  const r = await sendCommand(req, action, "Proxmox");
  if (/HTTP 403/.test(r.msg || "")) return { ok: false, msg: `This Proxmox user isn't allowed to power off node "${nd.node}" (needs Sys.PowerMgmt).` };
  return r;
}

// --- TrueNAS (SCALE / Community Edition, and CORE) ---
// TrueNAS 25.04 and newer: JSON-RPC 2.0 over a WebSocket (its REST API is deprecated and being removed).
// Scriptable has no WebSocket of its own, so a hidden web view makes the connection.
// calls: [[method, params, fallbackParams]], run one after another after logging in.
async function truenasWs(base, user, pass, calls) {
  const url = base.replace(/^http/i, "ws") + "/api/current";
  const wv = new WebView();
  await wv.loadHTML("<html><body></body></html>");
  const js = `(function(){
    var url = ${JSON.stringify(url)}, user = ${JSON.stringify(user)}, pass = ${JSON.stringify(pass)}, calls = ${JSON.stringify(calls)};
    var done = false, id = 0, stage = "connect", pending = {}, ws, results = [];
    function finish(r){ if (done) return; done = true; r.stage = stage; try { ws.close(); } catch (e) {} completion(r); }
    setTimeout(function(){ finish({ timeout: true }); }, 12000);
    try { ws = new WebSocket(url); } catch (e) { finish({ error: String(e) }); return; }
    function call(method, params, cb){ var i = ++id; pending[i] = cb; ws.send(JSON.stringify({ jsonrpc: "2.0", id: i, method: method, params: params })); }
    function msg(e){ return (e && (e.message || (e.data && e.data.reason))) || "error"; }
    ws.onerror = function(){ finish({ error: "connection" }); };
    ws.onclose = function(){ finish({ closed: true }); };
    ws.onmessage = function(e){ var m; try { m = JSON.parse(e.data); } catch (x) { return; } var cb = pending[m.id]; if (cb) { delete pending[m.id]; cb(m); } };
    ws.onopen = function(){
      stage = "login";
      call("auth.login", [user, pass], function(m){
        if (m.error) { finish({ error: msg(m.error) }); return; }
        if (m.result !== true) { finish({ auth: true }); return; }
        stage = "call";
        var k = 0;
        (function next(){
          if (k >= calls.length) { finish({ ok: true, results: results }); return; }
          var c = calls[k];
          call(c[0], c[1], function(r){
            if (r.error && c[2]) {
              call(c[0], c[2], function(r2){
                if (r2.error) { finish({ error: msg(r2.error) }); return; }
                results.push(r2.result); k++; next();
              });
              return;
            }
            if (r.error) { finish({ error: msg(r.error) }); return; }
            results.push(r.result); k++; next();
          });
        })();
      });
    };
  })();`;
  try {
    return await wv.evaluateJavaScript(js, true);
  } catch (e) {
    return { error: String((e && e.message) || e), stage: "connect" };
  }
}

async function truenasRun(action, user, pass, base) {
  const method = action === "off" ? "system.shutdown" : "system.reboot";
  // 25.10+ needs a reason; earlier JSON-RPC versions take only options, hence the fallback
  const calls = action ? [[method, ["Requested from the -board app"], []]] : [["system.info", []]];
  const w = await truenasWs(base, user, pass, calls);
  if (w.ok) {
    if (action) return { ok: true, msg: powerSent(action) };
    const info = w.results && w.results[0];
    return { ok: true, msg: "saved login works" + (info && info.version ? ` (${info.version})` : "") };
  }
  if (w.auth) return { ok: false, auth: true, msg: "TrueNAS refused the login (wrong username or password, or the account uses two-factor login)." };
  if (action && w.stage === "call" && (w.closed || w.timeout)) return { ok: true, msg: powerSent(action) };
  if (w.stage !== "connect") return { ok: false, msg: "TrueNAS: " + (w.error || "no answer") };

  // No JSON-RPC WebSocket: an older TrueNAS (24.10 and earlier, or CORE) with the REST API
  const auth = { Authorization: "Basic " + Data.fromString(`${user}:${pass}`).toBase64String() };
  if (!action) {
    const req = nasRequest(base + "/api/v2.0/system/info");
    req.headers = auth;
    let info;
    try {
      info = await req.loadJSON();
    } catch (e) {
      return { ok: false, net: true, msg: "Can't reach TrueNAS at " + base + (/^https:/i.test(base) ? " (for https it needs a certificate the iPhone trusts)" : "") };
    }
    const code = req.response && req.response.statusCode;
    if (code === 401) return { ok: false, auth: true, msg: "TrueNAS refused the login" };
    if (code !== 200) return { ok: false, msg: `TrueNAS answered HTTP ${code}` };
    return { ok: true, msg: "saved login works" + (info && info.version ? ` (${info.version})` : "") };
  }
  const req = nasRequest(`${base}/api/v2.0/system/${action === "off" ? "shutdown" : "reboot"}`, "POST");
  req.headers = { ...auth, "Content-Type": "application/json" };
  req.body = "{}";
  return sendCommand(req, action, "TrueNAS");
}

// --- Unraid ---
async function unraidRun(action, user, pass, base) {
  // 1. log in; the answer is a redirect that sets the session cookie, so don't follow it
  const lg = nasRequest(base + "/login", "POST");
  lg.headers = { "Content-Type": "application/x-www-form-urlencoded" };
  lg.body = formBody({ username: user || "root", password: pass });
  lg.onRedirect = () => null;
  let page;
  try {
    page = await lg.loadString();
  } catch (e) {
    return { ok: false, net: true, msg: "Can't reach Unraid at " + base };
  }
  const code = (lg.response && lg.response.statusCode) || 0;
  const hdr = (lg.response && lg.response.headers) || {};
  const loc = String(hdr.Location || hdr.location || "");
  if (code >= 300 && code < 400 && /^https?:\/\//i.test(loc) && hostOf(loc) !== hostOf(base)) {
    return { ok: false, msg: `Unraid sends you on to ${loc.replace(/\/login.*$/i, "")}. Enter that as the Unraid address in Settings.` };
  }
  if (code < 300 || code >= 400) {
    return /password/i.test(page || "") ? { ok: false, auth: true, msg: "Unraid refused the login (wrong username or password)." } : { ok: false, msg: `Unraid answered HTTP ${code}` };
  }
  const cookie = cookiesOf(lg) || String(hdr["Set-Cookie"] || hdr["set-cookie"] || "").split(";")[0];
  // 2. the csrf_token every Unraid page carries
  const pg = nasRequest(base + "/Main");
  pg.headers = { Cookie: cookie };
  let html = "";
  try {
    html = await pg.loadString();
  } catch (e) {}
  const m = html.match(/csrf_token["'\s]*[:=]\s*["']([0-9A-Za-z]{8,})["']/);
  if (!m) return /name=["']?password/i.test(html) ? { ok: false, auth: true, msg: "Unraid refused the login." } : { ok: false, msg: "Logged in to Unraid, but couldn't read its security token." };
  if (!action) return { ok: true, msg: "saved login works" };
  // 3. what Unraid's own Reboot / Shut down buttons send
  const req = nasRequest(base + "/webGui/include/Boot.php", "POST");
  req.headers = { Cookie: cookie, "Content-Type": "application/x-www-form-urlencoded" };
  req.body = formBody({ cmd: action === "off" ? "shutdown" : "reboot", csrf_token: m[1] });
  return sendCommand(req, action, "Unraid");
}

// --- Synology DSM ---
const SYNO_ERRORS = {
  400: "wrong username or password",
  401: "this account is disabled",
  402: "this account isn't allowed to log in",
  403: "this account uses two-factor login. Use an administrator account without it",
  404: "the two-factor code failed",
  406: "two-factor login is enforced for this account",
  407: "too many failed logins: this address is blocked for a while",
};

async function synoApi(base, cgi, params) {
  const req = nasRequest(`${base}/webapi/${cgi}`, "POST");
  req.headers = { "Content-Type": "application/x-www-form-urlencoded" };
  req.body = formBody(params);
  return req;
}

async function synologyRun(action, user, pass, base) {
  let r;
  try {
    const login = { api: "SYNO.API.Auth", method: "login", account: user, passwd: pass, session: "board", format: "sid" };
    r = await (await synoApi(base, "entry.cgi", { ...login, version: 6 })).loadJSON();
    // DSM 6: the login API lives in auth.cgi
    if (r && !r.success && r.error && [102, 103, 104].includes(r.error.code)) {
      r = await (await synoApi(base, "auth.cgi", { ...login, version: 3 })).loadJSON();
    }
  } catch (e) {
    return { ok: false, net: true, msg: "Can't reach Synology DSM at " + base };
  }
  if (!r || !r.success || !r.data || !r.data.sid) {
    const c = r && r.error && r.error.code;
    return { ok: false, auth: [400, 401, 402, 403, 404, 406].includes(c), msg: "Synology: " + (SYNO_ERRORS[c] || `login failed (error ${c})`) };
  }
  if (!action) return { ok: true, msg: "saved login works" };
  const req = await synoApi(base, "entry.cgi", { api: "SYNO.Core.System", version: 1, method: action === "off" ? "shutdown" : "reboot", _sid: r.data.sid });
  let j;
  try {
    j = JSON.parse((await req.loadString()) || "{}");
  } catch (e) {
    return { ok: true, msg: powerSent(action) };
  }
  if (j && j.success === false) {
    const c = j.error && j.error.code;
    return { ok: false, msg: c === 105 ? "This Synology account isn't an administrator." : `Synology didn't accept the command (error ${c}).` };
  }
  return { ok: true, msg: powerSent(action) };
}

// --- OpenMediaVault ---
async function omvRpc(base, body, cookie) {
  const req = nasRequest(base + "/rpc.php", "POST");
  req.headers = { "Content-Type": "application/json", ...(cookie ? { Cookie: cookie } : {}) };
  req.body = JSON.stringify(body);
  return req;
}

async function omvRun(action, user, pass, base) {
  const lg = await omvRpc(base, { service: "Session", method: "login", params: { username: user, password: pass }, options: null });
  let r;
  try {
    r = JSON.parse((await lg.loadString()) || "{}");
  } catch (e) {
    return { ok: false, net: true, msg: "Can't reach OpenMediaVault at " + base };
  }
  const resp = (r && r.response) || {};
  if (resp.status === "challengeRequired") return { ok: false, auth: true, msg: "This OpenMediaVault account uses two-factor login. Use an administrator account without it." };
  if (!(resp.authenticated === true || resp.status === "authenticated")) {
    return { ok: false, auth: true, msg: "OpenMediaVault: " + ((r && r.error && r.error.message) || "wrong username or password") };
  }
  const cookie = cookiesOf(lg) || (resp.sessionid ? "X-OPENMEDIAVAULT-SESSIONID=" + resp.sessionid : "");
  if (!action) return { ok: true, msg: "saved login works" };
  // a 1 second delay lets OpenMediaVault answer before it goes down
  const req = await omvRpc(base, { service: "System", method: action === "off" ? "shutdown" : "reboot", params: { delay: 1 }, options: null }, cookie);
  let j;
  try {
    j = JSON.parse((await req.loadString()) || "{}");
  } catch (e) {
    return { ok: true, msg: powerSent(action) };
  }
  if (j && j.error) {
    const m = j.error.message || "error";
    return { ok: false, auth: /session|authenticat/i.test(m), msg: "OpenMediaVault didn't accept the command: " + m };
  }
  return { ok: true, msg: powerSent(action) };
}

const DRIVERS = { zimaos: zimaRun, proxmox: pveRun, truenas: truenasRun, unraid: unraidRun, synology: synologyRun, omv: omvRun };

// --- Linux (Debian, Ubuntu, Fedora, ... any systemd distro) via the board-power helper ---
function helperBase(s = SET) {
  return platformBase("linux", s);
}

function helperPort(s = SET) {
  const m = helperBase(s).match(/^https?:\/\/[^\/:]+:(\d+)/i);
  return m ? Number(m[1]) : /^https:/i.test(helperBase(s)) ? 443 : 80;
}

// returns the HTTP status code, or 0 when the helper can't be reached
async function helperCall(base, key, path, method) {
  const req = new Request(base + path);
  req.method = method;
  req.headers = { Authorization: "Bearer " + key };
  req.timeoutInterval = 8;
  try {
    await req.loadString();
  } catch (e) {
    return 0;
  }
  return (req.response && req.response.statusCode) || 0;
}

async function helperPower(action) {
  if (!hasLogin("linux")) {
    return { ok: false, msg: "Set up the power helper first: Settings > Server > Copy setup command." };
  }
  const base = helperBase();
  const code = await helperCall(base, Keychain.get(helperKey()), action === "off" ? "/shutdown" : "/reboot", "POST");
  if (!code) return { ok: false, msg: "Can't reach the power helper at " + base + ". Is it set up on the server?" };
  if (code === 401) return { ok: false, msg: "The helper refused the key. Copy the setup command from Settings and run it again." };
  if (code >= 300) return { ok: false, msg: `The helper didn't accept the command (HTTP ${code}).` };
  return { ok: true, msg: powerSent(action) };
}

// The helper: one Python 3 file (standard library only) run by systemd as root.
// It only answers requests that carry the secret key, and only knows /health, /reboot and /shutdown.
const HELPER_PY = `#!/usr/bin/env python3
# board-power: lets the -board app reboot or shut down this server.
# Remove it with: systemctl disable --now board-power; rm /usr/local/bin/board-power /etc/board-power.key /etc/systemd/system/board-power.service
import hmac, json, os, socket, subprocess, threading, time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

KEY = open("/etc/board-power.key").read().strip()
PORT = int(os.environ.get("BOARD_POWER_PORT", "${HELPER_PORT}"))
CMDS = {"/reboot": ["systemctl", "reboot"], "/shutdown": ["systemctl", "poweroff"]}

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
        self.reply(401, {"ok": False, "error": "wrong key"})
        return False

    def do_GET(self):
        if self.path != "/health":
            return self.reply(404, {"ok": False})
        if self.authed():
            self.reply(200, {"ok": True, "host": socket.gethostname()})

    def do_POST(self):
        cmd = CMDS.get(self.path)
        if not cmd:
            return self.reply(404, {"ok": False})
        if self.authed():
            self.reply(200, {"ok": True})
            threading.Timer(1.0, subprocess.run, [cmd]).start()

    def log_message(self, *args):
        pass

ThreadingHTTPServer(("", PORT), Handler).serve_forever()
`;

// What "Copy setup command" puts on the clipboard: paste it into an SSH session on the server.
// It asks for sudo once (or runs straight away when you're root, e.g. on Proxmox).
function helperSetupScript(key, port) {
  return `$(command -v sudo) bash -c "$(cat <<'BOARD_SETUP'
# -board power helper setup (Debian, Ubuntu, Fedora and other systemd Linux)
set -e
if ! command -v python3 >/dev/null; then
  if command -v apt-get >/dev/null; then apt-get update -qq && DEBIAN_FRONTEND=noninteractive apt-get install -y -qq python3 </dev/null
  elif command -v dnf >/dev/null; then dnf install -y -q python3 </dev/null
  else echo "Please install python3 first, then paste this again."; exit 1; fi
fi
(umask 077; printf '%s\\n' '${key}' > /etc/board-power.key)
cat > /usr/local/bin/board-power <<'BOARD_PY'
${HELPER_PY}BOARD_PY
chmod 755 /usr/local/bin/board-power
cat > /etc/systemd/system/board-power.service <<'BOARD_UNIT'
[Unit]
Description=-board power helper (reboot / shut down from the -board app)
After=network-online.target
Wants=network-online.target

[Service]
Environment=BOARD_POWER_PORT=${port}
ExecStart=/usr/bin/env python3 /usr/local/bin/board-power
Restart=on-failure

[Install]
WantedBy=multi-user.target
BOARD_UNIT
systemctl daemon-reload
systemctl enable board-power >/dev/null 2>&1
systemctl restart board-power
if command -v ufw >/dev/null && ufw status 2>/dev/null | grep -q "Status: active"; then ufw allow ${port}/tcp >/dev/null; fi
if command -v firewall-cmd >/dev/null && firewall-cmd --state >/dev/null 2>&1; then firewall-cmd -q --permanent --add-port=${port}/tcp && firewall-cmd -q --reload; fi
echo "-board power helper is running on port ${port}. Go back to -board and tap Test connection."
BOARD_SETUP
)"
`;
}

// d: { a: "restart" | "off", u, p } (u/p only when typed in the popup; otherwise the saved login is used)
async function handlePower(d) {
  const a = d.a === "off" ? "off" : "restart";
  const P = SET.platform;
  let r;
  if (!PLATFORMS[P]) {
    r = { ok: false, msg: "Pick your server type first: Settings > Server > Server type." };
  } else if (P === "linux") {
    r = await helperPower(a);
  } else {
    let user = d.u;
    let pass = d.p;
    let saved = false;
    if ((!user || !pass) && hasLogin(P)) {
      ({ user, pass } = savedLogin(P));
      saved = true;
    }
    if (!user || !pass) r = { ok: false, auth: true, msg: "" };
    else r = await DRIVERS[P](a, user, pass, platformBase(P), SET);
    if (r.auth && saved) forgetLogin(P);
    if (!r.auth && !r.net && !saved && user && pass) {
      Keychain.set(loginKeys(P).user, user);
      Keychain.set(loginKeys(P).pass, pass);
    }
  }
  r.a = a;
  r.hasLogin = hasLogin(P);
  return r;
}

// One line for "Test connection" about the Reboot / Shut down side of the chosen server type
async function testPower(s) {
  const P = s.platform;
  if (!PLATFORMS[P]) return { ok: false, t: "Server type: not picked yet, so Reboot / Shut down is off. Pick it under SERVER TYPE." };
  const name = PLATFORMS[P].name;
  const base = platformBase(P, s);
  if (P === "linux") {
    if (!hasLogin("linux")) return { ok: false, t: "Power helper: not set up yet (tap Copy setup command)" };
    const code = await helperCall(base, Keychain.get(helperKey()), "/health", "GET");
    if (code === 200) return { ok: true, t: "Power helper: OK at " + base };
    if (code === 401) return { ok: false, t: "Power helper: answers, but refused the key. Run the setup command again." };
    return { ok: false, t: "Power helper: no answer at " + base + (code ? ` (HTTP ${code})` : "") };
  }
  if (hasLogin(P)) {
    const { user, pass } = savedLogin(P);
    const r = await DRIVERS[P](null, user, pass, base, s);
    if (r.ok) return { ok: true, t: `${name}: OK, ${r.msg}` };
    return { ok: false, t: `${name}: ` + (r.auth ? "the saved login was refused. " : "") + (r.msg || "no answer at " + base) };
  }
  try {
    const req = nasRequest(base, "GET", 6);
    await req.load();
    return { ok: true, t: `${name}: reachable at ${base} (no login saved yet)` };
  } catch (e) {
    return { ok: false, t: `${name}: no answer at ${base}` };
  }
}

// ---------- Settings sheet (gear icon): load / save / test / maintenance ----------
// d: { op, s (settings from the form), gpass (new Glances password), clearGpass, id }
async function settingsOp(d) {
  const data = loadData();
  if (d.op === "load") {
    // the drive and network lists come from Glances (only asked when Monitoring is on)
    const [fs, net] = SET.monitoring && serverHostOf(SET)
      ? await Promise.all([api("fs").catch(() => null), api("network").catch(() => null)])
      : [null, null];
    const mounts = Array.isArray(fs)
      ? fs
          .filter((x) => x && x.mnt_point && isRealDrive(x))
          .map((x) => ({ mnt: x.mnt_point, size: x.size || 0, used: x.used || 0, dev: String(x.device_name || "").split("/").pop() }))
      : null;
    const ifaces = Array.isArray(net)
      ? net.map((n) => n.interface_name).filter((n) => n && !SKIP_IFACE.test(n))
      : null;
    let hasGpass = false;
    try {
      hasGpass = Keychain.contains(gpassKey());
    } catch (e) {}
    return {
      ok: true,
      s: SET,
      defaults: DEFAULTS,
      host: serverHostOf(SET),
      hasBuiltin: BUILTIN.length > 0,
      firstRun: !serverHostOf(SET),
      mounts,
      ifaces,
      hidden: data.hidden.map((id) => ({ id, name: id.replace(/^b:/, "") })),
      logins: Object.fromEntries(PLATFORM_KEYS.map((k) => [k, hasLogin(k)])),
      platforms: PLATFORMS,
      helperPort: HELPER_PORT,
      hasGpass,
    };
  }
  if (d.op === "save") {
    data.settings = cleanSettings(d.s);
    if (typeof d.title === "string" && d.title.trim()) data.title = d.title.trim().slice(0, 20);
    saveData(data);
    try {
      if (d.clearGpass && Keychain.contains(gpassKey())) Keychain.remove(gpassKey());
      if (d.gpass) Keychain.set(gpassKey(), d.gpass);
    } catch (e) {}
    applySettings();
    statusDirty = true;
    return { ok: true, msg: "Settings saved" };
  }
  if (d.op === "test") {
    // tests the values in the form, saved or not
    const s = cleanSettings(d.s);
    const lines = [];
    if (!serverHostOf(s)) return { ok: true, lines: [{ ok: false, t: "Enter your server's IP or name first, e.g. 192.168.1.10" }] };
    const reach = await serverReachable(s);
    lines.push(reach
      ? { ok: true, t: `Server: reachable (${reach} answered)` }
      : { ok: false, t: `Server: nothing answered at ${serverHostOf(s)} (check the address, and your VPN if you're away from home)` });
    if (s.monitoring) {
      const t0 = Date.now();
      try {
        const q = await glancesRequest(s, "quicklook", d.gpass || null);
        lines.push({ ok: true, t: `Glances: OK (${Date.now() - t0} ms${q && q.cpu_name ? ", " + q.cpu_name : ""})` });
      } catch (e) {
        // maybe it's the other Glances API version (distro packages are often Glances 3.x)
        const other = s.apiVersion === 4 ? 3 : 4;
        let otherOk = false;
        try {
          await glancesRequest({ ...s, apiVersion: other }, "quicklook", d.gpass || null);
          otherOk = true;
        } catch (e2) {}
        lines.push(
          otherOk
            ? { ok: false, t: `Glances: answers with API version ${other}, not ${s.apiVersion}. Pick ${other} under GLANCES API VERSION.` }
            : { ok: false, t: "Glances: no answer at " + glancesUrl(s) + " (check the address and that Glances runs with -w, or turn Monitoring off)" }
        );
      }
    }
    lines.push(await testPower(s));
    return { ok: true, lines };
  }
  if (d.op === "login") {
    // Settings > Server > Enter login: check the login against the server (address from the form), then save it
    const P = d.platform;
    if (!DRIVERS[P]) return { ok: false, error: "This server type doesn't use a login." };
    const user = String(d.u || "").trim();
    const pass = String(d.p || "");
    if (!user || !pass) return { ok: false, error: "Enter a username and a password." };
    const s = cleanSettings(d.s);
    const base = platformBase(P, s);
    const r = await DRIVERS[P](null, user, pass, base, s);
    if (!r.ok) {
      const why = r.msg || "no answer at " + base;
      return { ok: false, error: (r.net ? why + ". The login wasn't saved: try again when the server can be reached." : why) };
    }
    Keychain.set(loginKeys(P).user, user);
    Keychain.set(loginKeys(P).pass, pass);
    const extra = (String(r.msg || "").match(/\(([^)]*)\)\s*$/) || [])[1];
    return { ok: true, msg: `${PLATFORMS[P].login} login checked and saved` + (extra ? ` (${extra})` : "") };
  }
  if (d.op === "forget") {
    const P = PLATFORMS[d.platform] ? d.platform : SET.platform;
    forgetLogin(P);
    return { ok: true, msg: P === "linux" ? "Helper key forgotten" : PLATFORMS[P].login + " login forgotten" };
  }
  if (d.op === "helper") {
    // Linux power helper: keep the saved key (or save the new one made in the dashboard) and copy the setup command
    const s = cleanSettings(d.s);
    if (!hasLogin("linux")) {
      if (!/^[a-f0-9]{32,128}$/.test(String(d.key || ""))) return { ok: false, error: "Couldn't make a key. Try again." };
      Keychain.set(helperKey(), d.key);
    }
    Pasteboard.copy(helperSetupScript(Keychain.get(helperKey()), helperPort(s)));
    return { ok: true, msg: "Setup command copied. Paste it into an SSH session on the server.", hasKey: true };
  }
  if (d.op === "restore") {
    data.hidden = d.id ? data.hidden.filter((x) => x !== d.id) : [];
    saveData(data);
    statusDirty = true;
    return { ok: true, msg: d.id ? "Button restored" : "All buttons restored" };
  }
  if (d.op === "clearlogos") {
    const fm = FileManager.local();
    const dir = logoDir();
    for (const f of fm.listContents(dir)) fm.remove(fm.joinPath(dir, f));
    for (const k of Object.keys(logoB64)) delete logoB64[k];
    return { ok: true, msg: "Logo cache cleared. Logos download again now." };
  }
  if (d.op === "export") {
    // every server, its buttons and layout, and the shared settings
    Pasteboard.copy(JSON.stringify({ board: 2, exported: new Date().toISOString(), data: readStore() }, null, 2));
    return { ok: true, msg: "All servers' settings copied to the clipboard. Passwords are not included." };
  }
  if (d.op === "import") {
    let obj;
    try {
      obj = JSON.parse(Pasteboard.paste() || "");
    } catch (e) {
      return { ok: false, error: "The clipboard doesn't contain exported settings." };
    }
    const imp = obj && obj.board && obj.data && typeof obj.data === "object" ? obj.data : null;
    if (!imp) return { ok: false, error: "The clipboard doesn't contain exported settings." };
    if (Array.isArray(imp.servers)) {
      // every server (an export from this version): replaces all of them
      dataRaw = JSON.stringify(imp);
      const st = readStore();
      writeStore(st);
      CUR = st.current;
    } else {
      // an export from before there were several servers: goes into the server shown now
      saveData({ ...imp, settings: { ...loadData().settings, ...(isObj(imp.settings) ? imp.settings : {}) } });
    }
    applySettings();
    statusDirty = true;
    return { ok: true, msg: "Settings imported", reload: true };
  }
  if (d.op === "reset") {
    data.order = [];
    data.widgetOrder = [];
    data.widgetHidden = [];
    data.groups = [];
    saveData(data);
    return { ok: true, msg: "Layout reset. Your own buttons are kept." };
  }
  return { ok: false, error: "Unknown request" };
}

// ---------- Service logos (downloaded once, shrunk, stored on the phone) ----------
const LOGO_SIZE = 96;          // stored size in pixels (small = light on memory and fast)
const LOGO_RETRY_HOURS = 12;   // a logo that failed to download is retried after this long

function logoDir() {
  const fm = FileManager.local();
  const dir = fm.joinPath(fm.documentsDirectory(), "board-logos");
  if (!fm.fileExists(dir)) fm.createDirectory(dir, true);
  return dir;
}

function logoKey(url) {
  let h = 5381;
  for (let i = 0; i < url.length; i++) h = ((h * 33) ^ url.charCodeAt(i)) >>> 0;
  return h.toString(36) + "-" + url.length.toString(36);
}

function logoPath(url) {
  return FileManager.local().joinPath(logoDir(), logoKey(url) + ".png");
}

function cachedLogo(url) {
  if (!url) return null;
  try {
    const fm = FileManager.local();
    const p = logoPath(url);
    return fm.fileExists(p) ? Image.fromFile(p) : null;
  } catch (e) {
    return null;
  }
}

// base64 of saved logos, kept in memory once read (every button update sends them to the dashboard)
const logoB64 = {};
function logoBase64(url) {
  if (!url) return null;
  if (logoB64[url]) return logoB64[url];
  try {
    const fm = FileManager.local();
    const p = logoPath(url);
    if (!fm.fileExists(p)) return null;
    return (logoB64[url] = fm.read(p).toBase64String());
  } catch (e) {
    return null;
  }
}

function clearLogoFail(url) {
  if (!url) return;
  try {
    const fm = FileManager.local();
    const f = logoPath(url) + ".fail";
    if (fm.fileExists(f)) fm.remove(f);
  } catch (e) {}
}

// Fit the logo inside a LOGO_SIZE square, keeping its shape and transparency
function shrinkLogo(img) {
  const w0 = img.size.width;
  const h0 = img.size.height;
  const scale = Math.min(1, LOGO_SIZE / Math.max(w0, h0));
  const w = Math.max(1, Math.round(w0 * scale));
  const h = Math.max(1, Math.round(h0 * scale));
  const side = Math.max(w, h);
  const dc = new DrawContext();
  dc.size = new Size(side, side);
  dc.opaque = false;
  dc.respectScreenScale = false;
  dc.drawImageInRect(img, new Rect((side - w) / 2, (side - h) / 2, w, h));
  return dc.getImage();
}

// Downloads a logo if it isn't saved yet. Returns true only when something new was saved.
async function fetchLogo(url, timeout) {
  if (!/^https?:\/\//i.test(url || "")) return false;
  const fm = FileManager.local();
  const p = logoPath(url);
  const fail = p + ".fail";
  if (fm.fileExists(p)) return false;
  if (fm.fileExists(fail)) {
    const age = Date.now() - fm.modificationDate(fail).getTime();
    if (age < LOGO_RETRY_HOURS * 3600 * 1000) return false;
  }
  try {
    const req = new Request(url);
    req.timeoutInterval = timeout;
    const img = await req.loadImage();
    fm.write(p, Data.fromPNG(shrinkLogo(img)));
    if (fm.fileExists(fail)) fm.remove(fail);
    return true;
  } catch (e) {
    try {
      fm.writeString(fail, String(Date.now()));
    } catch (err) {}
    return false;
  }
}

async function ensureLogos(list, timeout) {
  const urls = [...new Set(list.map((s) => s.logo).filter(Boolean))];
  const got = await Promise.all(urls.map((u) => fetchLogo(u, timeout)));
  return got.some(Boolean);
}

// Removes saved logos that no button uses any more (after a logo URL was changed)
function pruneLogos() {
  try {
    const fm = FileManager.local();
    const dir = logoDir();
    const keep = new Set(
      getServices(false).concat(loadData().groups).filter((s) => s.logo).map((s) => logoKey(s.logo))
    );
    for (const f of fm.listContents(dir)) {
      if (!keep.has(f.split(".")[0])) fm.remove(fm.joinPath(dir, f));
    }
  } catch (e) {}
}

let logoBusy = false;
async function refreshLogos(wv) {
  if (logoBusy || !SET.useLogos) return;
  logoBusy = true;
  try {
    if (await ensureLogos(getServices(false).concat(loadData().groups), 8)) await pushServices(wv);
    pruneLogos();
  } catch (e) {}
  logoBusy = false;
}

function makeCustom(d) {
  let addr = String(d.addr).trim();
  const m = addr.match(/^(https?):\/\//i);
  const scheme = m ? m[1].toLowerCase() : d.scheme === "https" ? "https" : "http";
  addr = addr.replace(/^https?:\/\//i, "");
  let path = "";
  const slash = addr.indexOf("/");
  if (slash >= 0) {
    path = addr.slice(slash);
    addr = addr.slice(0, slash);
  }
  const port = String(d.port || "").replace(/[^0-9]/g, "");
  return {
    name: String(d.name).trim().slice(0, 24),
    port,
    icon: "🔗",
    logo: String(d.logo || "").trim(),
    custom: true,
    url: `${scheme}://${addr}${port ? ":" + port : ""}${path}`,
  };
}

function pushServices(wv) {
  return wv.evaluateJavaScript(`renderServices(${JSON.stringify(getPayload())})`, false);
}

// Set to true to see the home screen widget when running inside Scriptable
// (instead of the live dashboard).
const PREVIEW_WIDGET_IN_APP = false;

// ---------- API ----------
async function api(path) {
  return glancesRequest(SET, path);
}

function glancesPass() {
  try {
    if (Keychain.contains(gpassKey())) return Keychain.get(gpassKey());
  } catch (e) {}
  return CONFIG.password;
}

async function glancesRequest(set, path, pass) {
  const req = new Request(`${glancesUrl(set)}/api/${set.apiVersion}/${path}`);
  req.timeoutInterval = 5;
  if (set.username) {
    const token = Data.fromString(`${set.username}:${pass != null ? pass : glancesPass()}`).toBase64String();
    req.headers = { Authorization: `Basic ${token}` };
  }
  const j = await req.loadJSON();
  const code = req.response && req.response.statusCode;
  if (code && code >= 400) throw new Error(`Glances answered HTTP ${code}`);
  return j;
}

// opts.disks / opts.net = false skip what a view doesn't show (the small widget only has CPU and RAM).
// opts.fsMaxAge (ms) reuses the last drive list for that long (drive usage changes slowly).
let fsCache = null;
let fsTime = 0;
async function getStats(opts = {}) {
  const wantDisks = opts.disks !== false;
  const wantNet = opts.net !== false;
  const fsFresh = wantDisks && opts.fsMaxAge && fsCache && Date.now() - fsTime < opts.fsMaxAge;
  // quicklook returns CPU and RAM together (3 requests instead of 4)
  const [quick, fs, net] = await Promise.all([
    api("quicklook").catch(() => null),
    !wantDisks ? null : fsFresh ? fsCache : api("fs"),
    wantNet ? api("network") : null,
  ]);
  if (wantDisks && !fsFresh) {
    fsCache = fs;
    fsTime = Date.now();
  }
  let cpu, ram;
  if (quick && typeof quick.cpu === "number" && typeof quick.mem === "number") {
    cpu = quick.cpu;
    ram = quick.mem;
  } else {
    // fallback if quicklook is unavailable on your Glances
    const [c, m] = await Promise.all([api("cpu"), api("mem")]);
    cpu = c.total;
    ram = m.percent;
  }
  const disks = wantDisks ? pickDisks(fs) : [];
  return { cpu, ram, disks, storage: sumStorage(disks), net: wantNet ? pickNetwork(net) : { down: 0, up: 0 } };
}

// Total across all drives (drives that are the same filesystem are only counted once)
function sumStorage(disks) {
  if (!disks.length) return null;
  const seen = new Set();
  let used = 0;
  let size = 0;
  let free = 0;
  for (const d of disks) {
    const k = d.fstype === "zfs" ? driveKey({ device_name: d.device, fs_type: "zfs" }) : `${d.device}|${d.size}|${d.used}`;
    if (seen.has(k)) continue;
    seen.add(k);
    used += d.used || 0;
    size += d.size || 0;
    free += d.free ?? (d.size || 0) - (d.used || 0);
  }
  return { percent: size ? (used / size) * 100 : 0, used, size, free, count: seen.size };
}

// Mounts that aren't real drives: system folders, container layers, Proxmox's config and LXC file systems
const SKIP_MNT = /^\/(etc|run|dev|sys|proc|boot|snap|var\/lib\/(docker|lxcfs|containers)|var\/snap)(\/|$)/;
const SKIP_FSTYPE = /^(overlay|tmpfs|squashfs|devtmpfs|ramfs|nsfs|autofs|efivarfs|fuse|fuse\.lxcfs|fuse\.gvfsd-fuse|fuse\.portal|fusectl)$/;
// Virtual network interfaces (Docker, Proxmox VM / container taps and firewall bridges, Kubernetes, LXD)
const SKIP_IFACE = /^(lo|docker|veth|br-|virbr|tap|fwbr|fwpr|fwln|vnet|cali|flannel|cni|kube|lxcbr|lxdbr)/;

function isRealDrive(d) {
  return !SKIP_MNT.test(d.mnt_point || "") && !SKIP_FSTYPE.test(d.fs_type || "");
}

// The same storage seen at several mount points counts once. ZFS datasets (rpool/data, rpool/ROOT/pve-1, ...)
// all share their pool's space, so they count once per pool.
function driveKey(d) {
  const dev = String(d.device_name || "");
  return d.fs_type === "zfs" ? "zfs:" + dev.split("/")[0] : dev;
}

function pickDisks(fs) {
  let list = fs;
  if (SET.diskMounts.length) {
    list = SET.diskMounts
      .map((m) => fs.find((d) => d.mnt_point === m))
      .filter(Boolean);
  } else {
    const seen = new Set();
    // biggest first, so a ZFS pool is shown by its largest dataset
    list = fs
      .filter(isRealDrive)
      .sort((a, b) => b.size - a.size)
      .filter((d) => (seen.has(driveKey(d)) ? false : seen.add(driveKey(d))));
  }
  return list.slice(0, SET.diskMounts.length ? 12 : SET.maxDisks).map((d) => ({
    name: d.mnt_point === "/" ? "Root" : d.mnt_point.split("/").filter(Boolean).pop(),
    percent: d.percent,
    used: d.used,
    size: d.size,
    free: d.free,
    mnt: d.mnt_point,
    device: d.device_name,
    fstype: d.fs_type,
  }));
}

function pickNetwork(net) {
  const skip = SKIP_IFACE;
  const total = (n) => (n.bytes_recv || n.rx || 0) + (n.bytes_sent || n.tx || 0);
  let iface = SET.networkInterface
    ? net.find((n) => n.interface_name === SET.networkInterface)
    : net.filter((n) => !skip.test(n.interface_name)).sort((a, b) => total(b) - total(a))[0];
  if (!iface) return { down: 0, up: 0 };
  const dt = iface.time_since_update || 1;
  return {
    down: iface.bytes_recv_rate_per_sec ?? (iface.rx || 0) / dt,
    up: iface.bytes_sent_rate_per_sec ?? (iface.tx || 0) / dt,
  };
}

// ---------- Details for the expanded CPU / RAM / storage cards (fetched only while a card is open) ----------
let expanded = null;
let lastStats = null;

async function getDetail(key, stats) {
  const list = (x) => (Array.isArray(x) ? x : []);
  if (key === "cpu") {
    const [cpu, per, load, procs, sens] = await Promise.all([
      api("cpu").catch(() => ({})),
      api("percpu").catch(() => []),
      api("load").catch(() => null),
      api("processlist").catch(() => []),
      api("sensors").catch(() => []),
    ]);
    const cores = (load && load.cpucore) || list(per).length || 1;
    const agg = {};
    for (const p of list(procs)) {
      const n = p.name || "?";
      agg[n] = (agg[n] || 0) + (p.cpu_percent || 0);
    }
    const top = Object.keys(agg)
      .map((n) => ({ name: n, pct: agg[n] / cores }))
      .sort((a, b) => b.pct - a.pct)
      .slice(0, 5);
    const ct = cpuTemp(parseTemps(sens));
    return {
      kind: "cpu",
      user: cpu.user,
      system: cpu.system,
      iowait: cpu.iowait,
      load: load ? [load.min1, load.min5, load.min15] : null,
      cores: list(per).map((c) => c.total),
      nCores: cores,
      temp: ct.pkg,
      hot: ct.hot,
      nCoreTemps: ct.cores.length,
      top,
    };
  }
  if (key === "ram") {
    const [mem, swap, procs] = await Promise.all([
      api("mem"),
      api("memswap").catch(() => null),
      api("processlist").catch(() => []),
    ]);
    const rssOf = (p) => {
      const mi = p.memory_info;
      let r = 0;
      if (Array.isArray(mi)) r = mi[0] || 0;
      else if (mi && typeof mi === "object") r = mi.rss || 0;
      if (!r && p.memory_percent) r = (p.memory_percent / 100) * mem.total;
      return r;
    };
    const agg = {};
    for (const p of list(procs)) {
      const n = p.name || "?";
      agg[n] = (agg[n] || 0) + rssOf(p);
    }
    const top = Object.keys(agg)
      .map((n) => ({ name: n, bytes: agg[n], pct: (agg[n] / mem.total) * 100 }))
      .sort((a, b) => b.bytes - a.bytes)
      .slice(0, 5);
    return {
      kind: "ram",
      total: mem.total,
      used: mem.used,
      available: mem.available,
      cached: mem.cached,
      swapUsed: swap ? swap.used : null,
      swapTotal: swap ? swap.total : null,
      top,
    };
  }
  if (key === "disk") {
    let rows = [];
    try {
      rows = list(await api("diskio"));
    } catch (err) {}
    const io = stats.disks.map((disk) => {
      const base = String(disk.device || "").split("/").pop();
      const row = rows.find((x) => x.disk_name === base);
      if (!row) return null;
      const dt = row.time_since_update || 1;
      return {
        read: row.read_bytes_rate_per_sec ?? (row.read_bytes || 0) / dt,
        write: row.write_bytes_rate_per_sec ?? (row.write_bytes || 0) / dt,
      };
    });
    let t = null;
    try {
      t = parseTemps(await api("sensors"));
    } catch (err) {}
    const temps = stats.disks.map((disk) => (t ? driveTemp(disk, t) : null));
    return { kind: "disk", io, temps, nvme: t ? t.nvme : [], hdd: t ? t.hdd : [] };
  }
  return null;
}

async function pushDetail(wv) {
  if (!expanded || !lastStats) return;
  const key = expanded;
  const det = await getDetail(key, lastStats);
  if (det) await wv.evaluateJavaScript(`detail(${JSON.stringify(key)}, ${JSON.stringify(det)})`, false);
}

// ---------- Service status (up/down dots) ----------
// Any HTTP answer (even an error page) counts as up. Refused or timed-out connections count as down.
async function pingUrl(url, timeout) {
  try {
    const req = new Request(url);
    req.method = "HEAD";
    req.timeoutInterval = timeout;
    await req.load();
    return true;
  } catch (e) {
    // self-signed https or a non-web port (like SSH) still means something is listening
    return /certificate|secure connection|ssl|tls|bad response|bad server|malformed|could not be parsed/i.test(
      String((e && e.message) || e)
    );
  }
}

async function checkServices(list, timeout) {
  const out = {};
  const byUrl = {}; // buttons with the same address are only checked once
  await Promise.all(
    list.map(async (s) => {
      if (!byUrl[s.url]) byUrl[s.url] = pingUrl(s.url, timeout);
      out[s.id] = await byUrl[s.url];
    })
  );
  return out;
}

let statusBusy = false;
let statusDirty = false;
let lastStatus = 0;

// One place on the server: a quick HEAD request; some servers turn HEAD away, so a refusal that isn't a
// timeout gets one ordinary GET before the place counts as not answering
async function reachProbe(url, timeout) {
  const t0 = Date.now();
  if (await pingUrl(url, timeout)) return true;
  if (Date.now() - t0 >= timeout * 1000 - 200) return false; // timed out: nothing there
  try {
    const req = new Request(url);
    req.allowInsecureRequest = true;
    req.timeoutInterval = timeout;
    await req.load();
    return true;
  } catch (e) {
    return /certificate|secure connection|ssl|tls|bad response|bad server|malformed|could not be parsed/i.test(String((e && e.message) || e));
  }
}

// ---------- Is the server reachable? (the LIVE / OFFLINE status) ----------
// There is no ping on iOS, so this asks the usual places on the server at once: its web ports (80 / 443),
// the server system's own address (Reboot / Shut down), Glances when Monitoring is on, and your service
// buttons on that host. Any answer, even an error page, counts. Returns what answered first, or "" for none.
const REACH_SECONDS = 10; // how often the dashboard checks
async function serverReachable(s = SET, timeout = 4) {
  const host = serverHostOf(s);
  if (!host) return "";
  // the server address from Settings first (with its port), then the usual web ports
  const urls = new Set([serverUrlOf(s) + "/", `http://${host}/`, `https://${host}/`]);
  if (PLATFORMS[s.platform]) urls.add(platformBase(s.platform, s) + "/");
  if (s.monitoring) urls.add(glancesUrl(s) + "/");
  try {
    for (const sv of getServices(false)) if (hostOf(sv.url) === host && urls.size < 12) urls.add(sv.url);
  } catch (e) {}
  const label = (u) => (u.match(/^https?:\/\/[^\/]+/i) || [u])[0].replace(/^https?:\/\//i, "");
  return new Promise((resolve) => {
    let left = urls.size;
    for (const u of urls) {
      reachProbe(u, timeout).then((ok) => {
        if (ok) resolve(label(u));
        else if (--left === 0) resolve("");
      });
    }
  });
}

// ---------- Temperatures (from the Glances sensors plugin) ----------
function parseTemps(sens) {
  const all = (Array.isArray(sens) ? sens : []).filter(
    (x) => x && typeof x.value === "number" && (x.unit === "C" || /temp/i.test(x.type || ""))
  );
  const lb = (x) => String(x.label || "");
  const isHdd = (x) => x.type === "temperature_hdd" || /^(drivetemp|hddtemp)|^\/dev\/sd|^sd[a-z]/i.test(lb(x));
  const isNvme = (x) => /composite/i.test(lb(x)) || /^nvme/i.test(lb(x));
  const hdd = all.filter(isHdd);
  const nvme = all.filter((x) => !isHdd(x) && isNvme(x));
  const rest = all.filter((x) => !isHdd(x) && !isNvme(x));
  const pkg = rest.find((x) => /package|tdie|tctl|^cpu/i.test(lb(x)));
  return {
    pkg: pkg ? pkg.value : null,
    cores: rest.filter((x) => /^core/i.test(lb(x))).map((x) => x.value),
    nvme: nvme.map((x) => ({ label: lb(x), value: x.value })),
    hdd: hdd.map((x) => ({ label: lb(x), value: x.value })),
    other: rest.filter((x) => !/^sensor/i.test(lb(x))).map((x) => x.value),
  };
}

function cpuTemp(t) {
  const hottest = t.cores.length ? Math.max(...t.cores) : null;
  const pkg =
    t.pkg != null ? t.pkg : hottest != null ? hottest : t.other.length ? Math.max(...t.other) : null;
  return { pkg, hot: hottest, cores: t.cores };
}

// Match a mounted drive to its temperature sensor(s) by device name (nvme0n1 = first NVMe sensor, md0 = all HDD sensors)
function driveTemp(disk, t) {
  const dev = String(disk.device || "").split("/").pop();
  let m = dev.match(/^nvme(\d+)n/);
  if (m) {
    const x = t.nvme[Number(m[1])];
    return x ? { kind: "nvme", values: [x.value] } : null;
  }
  m = dev.match(/^sd([a-z])/);
  if (m) {
    const x = t.hdd[m[1].charCodeAt(0) - 97];
    return x ? { kind: "hdd", values: [x.value] } : null;
  }
  if (/^md/.test(dev) && t.hdd.length) return { kind: "hdd", values: t.hdd.map((x) => x.value) };
  return null;
}

// ---------- Helpers ----------
function formatBytes(b) {
  const units = ["B", "KB", "MB", "GB", "TB"];
  let i = 0;
  while (b >= 1024 && i < units.length - 1) {
    b /= 1024;
    i++;
  }
  return `${b >= 100 || i === 0 ? Math.round(b) : b.toFixed(1)} ${units[i]}`;
}

function colorFor(pct) {
  if (pct >= 85) return new Color("#ff5d73");
  if (pct >= 60) return new Color("#ffb454");
  return new Color("#34e0a1");
}

// ---------- Drawing ----------
function glow(dc, x, y, r, hex, strength = 1) {
  const steps = 28;
  for (let i = 0; i < steps; i++) {
    const rr = r * (1 - i / steps);
    dc.setFillColor(new Color(hex, 0.014 * strength));
    dc.fillEllipse(new Rect(x - rr, y - rr, rr * 2, rr * 2));
  }
}

// "18,194,165" -> "#12c2a5"
function rgbHex(rgb) {
  return "#" + String(rgb).split(",").map((n) => Math.max(0, Math.min(255, Number(n) || 0)).toString(16).padStart(2, "0")).join("");
}

function widgetLook() {
  const t = THEMES[SET.theme] || THEMES.aurora;
  const strength = t.noGlow ? 0 : GLOW_LEVELS[SET.glow] / GLOW_LEVELS.normal;
  return { bg: t.bg, g1: rgbHex(t.g1), g2: rgbHex(t.g2), strength };
}

function backgroundImage() {
  const fm = FileManager.local();
  const L = widgetLook();
  // one cached picture per theme / glow combination
  const key = [L.bg, L.g1, L.g2, L.strength].join("-").replace(/[^a-z0-9.-]/gi, "");
  const path = fm.joinPath(fm.cacheDirectory(), `board-bg-${key}.png`);
  try {
    if (fm.fileExists(path)) return Image.fromFile(path);
  } catch (e) {}
  const img = drawBackground();
  try {
    fm.writeImage(path, img);
  } catch (e) {}
  return img;
}

function drawBackground() {
  const W = 660;
  const H = 310;
  const dc = new DrawContext();
  dc.size = new Size(W, H);
  dc.opaque = true;
  dc.respectScreenScale = false;
  const L = widgetLook();
  dc.setFillColor(new Color(L.bg));
  dc.fillRect(new Rect(0, 0, W, H));
  if (L.strength > 0) {
    glow(dc, 90, 20, 280, L.g1, L.strength);
    glow(dc, W - 40, H + 20, 320, L.g2, L.strength);
  }
  return dc.getImage();
}

function ringImage(pct, color, size = 140) {
  const dc = new DrawContext();
  dc.size = new Size(size, size);
  dc.opaque = false;
  dc.respectScreenScale = true;

  const cx = size / 2;
  const cy = size / 2;
  const lw = size * 0.115;
  const r = size / 2 - lw * 1.1;

  // track
  dc.setStrokeColor(new Color("#ffffff", 0.12));
  dc.setLineWidth(lw);
  dc.strokeEllipse(new Rect(cx - r, cy - r, r * 2, r * 2));

  // arc as a polyline, starting at 12 o'clock
  const p = Math.max(0.01, Math.min(pct, 100) / 100);
  const steps = Math.max(2, Math.ceil(90 * p));
  const pts = [];
  for (let i = 0; i <= steps; i++) {
    const a = -Math.PI / 2 + 2 * Math.PI * p * (i / steps);
    pts.push(new Point(cx + r * Math.cos(a), cy + r * Math.sin(a)));
  }
  const path = new Path();
  path.addLines(pts);

  // soft glow layers, then the solid arc
  const layers = [
    [lw * 1.9, 0.1],
    [lw * 1.45, 0.16],
    [lw, 1],
  ];
  for (const [width, alpha] of layers) {
    dc.addPath(path);
    dc.setStrokeColor(new Color(color.hex, alpha));
    dc.setLineWidth(width);
    dc.strokePath();
  }

  // rounded caps
  dc.setFillColor(color);
  for (const pt of [pts[0], pts[pts.length - 1]]) {
    dc.fillEllipse(new Rect(pt.x - lw / 2, pt.y - lw / 2, lw, lw));
  }

  // Liquid Glass look: glass lens in the middle
  if (SET.style === "glass") {
    const lr = r - lw * 0.95;
    dc.setFillColor(new Color("#ffffff", 0.07));
    dc.fillEllipse(new Rect(cx - lr, cy - lr, lr * 2, lr * 2));
    dc.setStrokeColor(new Color("#ffffff", 0.16));
    dc.setLineWidth(1);
    dc.strokeEllipse(new Rect(cx - lr, cy - lr, lr * 2, lr * 2));
  }

  // value in the middle
  dc.setFont(Font.boldRoundedSystemFont(size * 0.25));
  dc.setTextColor(Color.white());
  dc.setTextAlignedCenter();
  dc.drawTextInRect(`${Math.round(pct)}%`, new Rect(0, cy - size * 0.16, size, size * 0.32));

  return dc.getImage();
}

// Widget pill / button background. Classic: flat see-through fill.
// Liquid Glass: top-lit gradient fill + a thin bright rim (k = strength).
function glassStack(st, radius, k, flatAlpha) {
  if (SET.style !== "glass") {
    st.backgroundColor = new Color("#ffffff", flatAlpha);
    st.cornerRadius = radius.classic;
    return;
  }
  const g = new LinearGradient();
  g.colors = [new Color("#ffffff", 0.2 * k), new Color("#ffffff", 0.06 * k), new Color("#ffffff", 0.1 * k)];
  g.locations = [0, 0.55, 1];
  g.startPoint = new Point(0, 0);
  g.endPoint = new Point(0.4, 1);
  st.backgroundGradient = g;
  st.borderColor = new Color("#ffffff", 0.22 * k);
  st.borderWidth = 1;
  st.cornerRadius = radius.glass;
}

function addSymbolText(parent, symbol, text, color) {
  const sf = SFSymbol.named(symbol);
  sf.applyFont(Font.boldSystemFont(10));
  const img = parent.addImage(sf.image);
  img.imageSize = new Size(10, 10);
  img.tintColor = color;
  parent.addSpacer(4);
  const t = parent.addText(text);
  t.font = Font.mediumMonospacedSystemFont(11);
  t.textColor = Color.white();
}

// The slow part of the button grid (status checks, logo downloads). The widget starts it at the same
// time as the stats request, so the two run side by side.
async function prepareGrid(maxCells) {
  // Uses your widget layout (set in the live dashboard: Widget layout)
  const list = widgetCells(getLayout()).slice(0, maxCells || 99);
  // a stack pill needs the status of every button inside it
  const toCheck = [];
  for (const c of list) toCheck.push(...(c.kind === "group" ? c.members : [c]));
  // status checks and any missing logo downloads run together (logos are only downloaded once)
  const [status] = await Promise.all([
    SET.widgetStatusDots ? checkServices(toCheck, Math.min(SET.pingTimeout, 3)) : {},
    SET.useLogos ? ensureLogos(list, 4).catch(() => false) : false,
  ]);
  // stack dot: green = all up, orange = some down, red = all down
  const dotColor = (s) => {
    if (s.kind !== "group") return status[s.id] ? "#34e0a1" : "#ff5d73";
    const up = s.members.filter((m) => status[m.id]).length;
    return up === s.members.length ? "#34e0a1" : up === 0 ? "#ff5d73" : "#ffb454";
  };
  const upCount = toCheck.filter((x) => status[x.id]).length;
  return { list, dotColor, upCount, total: toCheck.length, checked: SET.widgetStatusDots };
}

// Service buttons grid for the large widget (tap = open in Safari)
function addServiceGrid(w, prep) {
  const { list, dotColor } = prep;
  const cols = 4;
  const gap = 5;
  const inner = Device.screenSize().width * 0.84 - 28;
  const pw = Math.floor((inner - gap * (cols - 1)) / cols);
  for (let i = 0; i < list.length; i += cols) {
    if (i > 0) w.addSpacer(gap);
    const row = w.addStack();
    row.layoutHorizontally();
    for (let j = 0; j < cols; j++) {
      if (j > 0) row.addSpacer();
      const s = list[i + j];
      const pill = row.addStack();
      pill.size = new Size(pw, 26);
      if (!s) continue;
      pill.centerAlignContent();
      glassStack(pill, { classic: 8, glass: 9 }, 1, 0.1);
      pill.setPadding(0, 5, 0, 6);
      // a stack pill opens the live dashboard with that stack open
      pill.url =
        s.kind === "group"
          ? URLScheme.forRunningScript() + "?server=" + encodeURIComponent(CUR) + "&stack=" + encodeURIComponent(s.id)
          : s.url;
      // logo / emoji in a fixed-size box, so every name starts at the same spot
      const icBox = pill.addStack();
      icBox.size = new Size(13, 13);
      icBox.centerAlignContent();
      const logo = SET.useLogos ? cachedLogo(s.logo) : null;
      if (logo) {
        const li = icBox.addImage(logo);
        li.imageSize = new Size(12, 12);
      } else {
        const ic = icBox.addText(s.icon);
        ic.font = Font.systemFont(10);
      }
      pill.addSpacer(3);
      const t = pill.addText(s.name);
      t.font = Font.semiboldSystemFont(9);
      t.textColor = Color.white();
      t.lineLimit = 1;
      t.minimumScaleFactor = 0.6;
      // the status dot sits at the right edge of every button, so the dots line up in a column
      pill.addSpacer();
      if (SET.widgetStatusDots) {
        pill.addSpacer(3);
        const dot = pill.addStack();
        dot.size = new Size(6, 6);
        dot.cornerRadius = 3;
        dot.backgroundColor = new Color(dotColor(s));
      }
    }
  }
}

// ---------- Widget ----------
async function buildWidget() {
  const family = config.widgetFamily || "medium";
  const small = family === "small";
  const large = family === "large";

  const w = new ListWidget();
  w.backgroundImage = backgroundImage();
  w.setPadding(12, 14, 12, 14);
  w.refreshAfterDate = new Date(Date.now() + SET.refreshMinutes * 60 * 1000);
  w.url = URLScheme.forRunningScript() + "?server=" + encodeURIComponent(CUR); // tap = live dashboard on this server

  // not set up yet: ask to open the app
  if (!serverHostOf(SET)) {
    const t = w.addText(getTitle());
    t.font = Font.heavyRoundedSystemFont(12);
    t.textColor = Color.white();
    w.addSpacer();
    const m = w.addText("Tap to set up -board:\nenter your server's address in Settings.");
    m.font = Font.systemFont(12);
    m.textColor = new Color("#ffffff", 0.75);
    w.addSpacer();
    return w;
  }

  // header
  const header = w.addStack();
  header.centerAlignContent();
  const title = header.addText(SET.widgetTitle || getTitle());
  title.font = Font.heavyRoundedSystemFont(12);
  title.textColor = Color.white();
  header.addSpacer();

  // everything runs side by side: is the server reachable, the buttons' checks and (with Monitoring) the stats.
  // Without Monitoring the widget shows buttons instead of gauges (medium: 2 rows; small: a summary).
  const mon = SET.monitoring;
  const reachP = serverReachable(SET).catch(() => "");
  const gridPrep = large || !mon ? prepareGrid(large ? 0 : small ? 0 : 8).catch(() => null) : null;
  let stats = null;
  if (mon) {
    try {
      // the small widget only shows CPU and RAM
      stats = await getStats(small ? { disks: false, net: false } : {});
    } catch (e) {
      stats = null;
    }
  }
  // Glances answering means the server is up, so there's no need to wait for the other checks then
  const reach = stats ? true : !!(await reachP);

  // status pill
  const pill = header.addStack();
  pill.centerAlignContent();
  glassStack(pill, { classic: 9, glass: 10 }, 1, 0.1);
  pill.setPadding(3, 8, 3, 8);
  const time = new DateFormatter();
  time.dateFormat = "HH:mm";
  const dot = pill.addText("●");
  dot.font = Font.systemFont(8);
  dot.textColor = reach ? new Color("#34e0a1") : new Color("#ff5d73");
  pill.addSpacer(4);
  const pillText = pill.addText(reach ? time.string(new Date()) : "OFFLINE");
  pillText.font = Font.boldMonospacedSystemFont(10);
  pillText.textColor = Color.white();

  // corner button (only works in medium and large widgets; only when a link is set)
  if (!small && SET.linkUrl) {
    header.addSpacer(6);
    const btn = header.addStack();
    btn.size = new Size(24, 24);
    btn.centerAlignContent();
    glassStack(btn, { classic: 12, glass: 12 }, 1.3, 0.14);
    btn.url = SET.linkUrl;
    btn.addSpacer();
    const sf = SFSymbol.named("arrow.up.right");
    sf.applyFont(Font.boldSystemFont(11));
    const icon = btn.addImage(sf.image);
    icon.imageSize = new Size(11, 11);
    icon.tintColor = Color.white();
    btn.addSpacer();
  }

  // Monitoring off: service buttons (medium / large) or a summary (small)
  if (!mon) {
    const prep = await gridPrep;
    if (small) {
      w.addSpacer();
      const big = w.addText(reach ? "ONLINE" : "OFFLINE");
      big.font = Font.heavyRoundedSystemFont(20);
      big.textColor = reach ? new Color("#34e0a1") : new Color("#ff5d73");
      if (prep && prep.checked && prep.total) {
        w.addSpacer(2);
        const sub = w.addText(`${prep.upCount} of ${prep.total} services up`);
        sub.font = Font.mediumSystemFont(11);
        sub.textColor = new Color("#ffffff", 0.75);
      }
      w.addSpacer();
    } else {
      w.addSpacer(10);
      if (prep && prep.list.length) addServiceGrid(w, prep);
      else {
        const m = w.addText(reach ? "Add service buttons in the dashboard with +." : "Can't reach the server.\nCheck the address in Settings, and your VPN if you're away from home.");
        m.font = Font.systemFont(11);
        m.textColor = new Color("#ffffff", 0.7);
      }
      w.addSpacer();
    }
    return w;
  }

  if (!stats) {
    w.addSpacer();
    const msg = w.addText(reach
      ? "The server is up, but Glances isn't answering.\nCheck the Glances address in Settings > Monitoring."
      : "Can't reach the server.\nCheck the address in Settings, and your VPN if you're away from home.");
    msg.font = Font.systemFont(11);
    msg.textColor = new Color("#ffffff", 0.7);
    w.addSpacer();
    if (large) {
      const prep = await gridPrep;
      if (prep && prep.list.length) addServiceGrid(w, prep);
      w.addSpacer();
    }
    return w;
  }

  w.addSpacer(6);

  // ring gauges
  const items = [
    { label: "CPU", pct: stats.cpu },
    { label: "RAM", pct: stats.ram },
    ...(stats.storage ? [{ label: "STORAGE", pct: stats.storage.percent }] : []),
  ];
  const shown = small ? items.slice(0, 2) : items;

  const row = w.addStack();
  row.centerAlignContent();
  shown.forEach((it, i) => {
    if (i > 0) row.addSpacer();
    const col = row.addStack();
    col.layoutVertically();
    col.centerAlignContent();
    const img = col.addImage(ringImage(it.pct, colorFor(it.pct)));
    img.imageSize = new Size(54, 54);
    col.addSpacer(2);
    const lbl = col.addText(it.label);
    lbl.font = Font.boldRoundedSystemFont(9);
    lbl.textColor = new Color("#ffffff", 0.7);
    lbl.centerAlignText();
  });

  // network
  if (!small) {
    if (large) {
      w.addSpacer(10);
      const prep = await gridPrep;
      if (prep) addServiceGrid(w, prep);
    }
    w.addSpacer();
    const net = w.addStack();
    net.centerAlignContent();
    glassStack(net, { classic: 9, glass: 11 }, 0.85, 0.08);
    net.setPadding(4, 10, 4, 10);
    addSymbolText(net, "arrow.down", `${formatBytes(stats.net.down)}/s`, new Color("#5ac8fa"));
    net.addSpacer();
    addSymbolText(net, "arrow.up", `${formatBytes(stats.net.up)}/s`, new Color("#ff9f6b"));
  }

  return w;
}

// ---------- Live animated dashboard (shown when you tap the widget) ----------
const LIVE_HTML = `<!DOCTYPE html>
<html><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1,viewport-fit=cover">
<style>
*{box-sizing:border-box;margin:0;-webkit-tap-highlight-color:transparent}
html,body{min-height:100%;background:#0b1220;color:#fff;font-family:-apple-system,"SF Pro Rounded",system-ui,sans-serif}
body{overflow-y:auto;-webkit-overflow-scrolling:touch}
.bg{position:fixed;inset:0;overflow:hidden;pointer-events:none}
.blob{position:absolute;width:95vmax;height:95vmax;border-radius:50%;opacity:.5;will-change:transform;animation:drift 14s ease-in-out infinite alternate}
.b1{background:radial-gradient(circle,rgba(18,194,165,.75) 0%,rgba(18,194,165,0) 68%);top:-40vmax;left:-35vmax}
.b2{background:radial-gradient(circle,rgba(106,92,255,.75) 0%,rgba(106,92,255,0) 68%);bottom:-45vmax;right:-40vmax;animation-delay:-7s}
@keyframes drift{to{transform:translate(14vmax,10vmax) scale(1.2)}}
main{position:relative;min-height:100vh;padding:calc(env(safe-area-inset-top,0px) + 56px) 18px calc(env(safe-area-inset-bottom,0px) + 40px);display:flex;flex-direction:column;gap:16px}
header{display:flex;align-items:center;justify-content:space-between}
h1{font-size:22px;font-weight:800;letter-spacing:.12em}
.live{display:flex;align-items:center;gap:8px;font-size:12px;font-weight:700;letter-spacing:.1em;padding:6px 12px;border-radius:99px;background:rgba(255,255,255,.1)}
.dot{width:8px;height:8px;border-radius:50%;background:#34e0a1;box-shadow:0 0 6px rgba(52,224,161,.7)}
.dot.off{background:#ff5d73;box-shadow:0 0 6px rgba(255,93,115,.7)}
.dot.wait{background:rgba(255,255,255,.35);box-shadow:none}
#monst{flex:1;text-align:right;margin-right:10px;font-size:10px;letter-spacing:.1em;color:#ffb454;opacity:.9}
body.nomon .monwrap{display:none}
html body #dv{z-index:14} /* above the Settings sheet (#sv is 12) */
.sbtns button.del{background:rgba(255,93,115,.16);color:#ff5d73}
.sbtns button:disabled{opacity:.4}
/* server dots under the name (swipe the name to change server; the + adds one) */
.sdots{display:flex;align-items:center;gap:7px;margin:-8px 2px 0;min-height:16px}
.sdots i{display:block;width:7px;height:7px;border-radius:4px;background:rgba(255,255,255,.28);transition:width .3s,background .3s;cursor:pointer}
.sdots i.on{width:20px;background:var(--acc)}
.sdots b{font-size:15px;font-weight:600;line-height:1;opacity:.45;padding:0 4px;cursor:pointer}
@keyframes tslide{from{opacity:0;transform:translateX(var(--sx,0))}}
.tslide{animation:tslide .35s cubic-bezier(.22,1,.36,1)}
.shd h2 small{display:block;font-size:10px;font-weight:700;letter-spacing:.14em;opacity:.55;margin-top:3px;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}
.sdiv{font-size:11px;font-weight:800;letter-spacing:.16em;opacity:.5;text-align:center;margin:6px 0 -6px}
@keyframes pulse{0%{box-shadow:0 0 0 0 rgba(52,224,161,.7)}70%{box-shadow:0 0 0 9px rgba(52,224,161,0)}100%{box-shadow:0 0 0 0 rgba(52,224,161,0)}}
#grid{display:grid;grid-template-columns:1fr 1fr;gap:14px}
.card{background:rgba(255,255,255,.07);border:1px solid rgba(255,255,255,.1);border-radius:24px;padding:18px 12px 14px;display:flex;flex-direction:column;align-items:center;gap:10px}
.ringwrap{position:relative;width:min(34vw,150px);aspect-ratio:1}
svg{width:100%;height:100%;overflow:visible}
circle{fill:none;stroke-width:10;stroke-linecap:round}
.track{stroke:rgba(255,255,255,.12)}
.arc{stroke-dasharray:326.7;stroke-dashoffset:326.7;transform:rotate(-90deg);transform-origin:60px 60px;transition:stroke-dashoffset 1.1s cubic-bezier(.22,1,.36,1),stroke .6s;filter:drop-shadow(0 0 7px var(--c,#34e0a1))}
.num{position:absolute;inset:0;display:flex;align-items:center;justify-content:center;font-size:30px;font-weight:800;font-variant-numeric:tabular-nums}
.num small{font-size:15px;font-weight:700;opacity:.7;margin-left:1px}
.lbl{font-size:12px;font-weight:700;letter-spacing:.14em;opacity:.7}
.netcard{display:flex;justify-content:space-around;background:rgba(255,255,255,.07);border:1px solid rgba(255,255,255,.1);border-radius:24px;padding:18px 10px}
.net{display:flex;flex-direction:column;align-items:center;gap:6px}
.net b{font-size:20px;font-variant-numeric:tabular-nums}
.net span{font-size:11px;letter-spacing:.14em;opacity:.65;font-weight:700}
.arrow{font-size:20px;display:inline-block}
.dn{color:#5ac8fa}
.up{color:#ff9f6b}
.sec{font-size:12px;font-weight:700;letter-spacing:.14em;opacity:.6;margin:6px 4px -4px}
#svc{display:grid;grid-template-columns:1fr 1fr;gap:12px}
.svc{-webkit-touch-callout:none;-webkit-user-select:none;user-select:none;min-width:0;display:flex;flex-direction:column;gap:4px;padding:14px;border-radius:20px;text-decoration:none;color:#fff;background:rgba(255,255,255,.07);border:1px solid rgba(255,255,255,.1)}
.svc:active{background:rgba(255,255,255,.18)}
.ic{font-size:22px}
img.ic{width:26px;height:26px;object-fit:contain;display:block;pointer-events:none;-webkit-user-drag:none;-webkit-touch-callout:none}
.wide img.ic{width:14px;height:14px;flex:none}
.in img.lp{width:26px;height:26px;object-fit:contain;margin-left:8px;display:none;flex:none}
.in img.lp.ok{display:block}
.chip{flex:none;margin-left:8px;font-size:12px;font-weight:700;color:#fff;background:rgba(255,255,255,.14);border:0;border-radius:99px;padding:5px 10px;font-family:inherit}
.chip:active{background:rgba(255,255,255,.26)}
.fh{font-size:11px;opacity:.45;margin:0 4px}
.grp{grid-column:1/-1;min-width:0;scroll-margin-top:70px}
.grp.lifted{opacity:.25}
.grph{text-decoration:none;gap:10px;cursor:pointer;-webkit-touch-callout:none;-webkit-user-select:none;user-select:none}
.grph .gt{display:flex;align-items:center;gap:10px;min-width:0}
.grph .gt b{white-space:nowrap;overflow:hidden;text-overflow:ellipsis}
.grph .gc{font-size:11px;opacity:.5;font-weight:700}
.grph .gr{display:flex;align-items:center;gap:10px;flex:none}
.grph .ic{font-size:18px;letter-spacing:0}
.grph img.ic{width:22px;height:22px}
.grph .sd{position:static}
.grph.ghost{position:fixed;z-index:20;margin:0;pointer-events:none;transform:scale(1.03);background:rgba(70,84,128,.96);box-shadow:0 14px 34px rgba(0,0,0,.5)}
.gbody{display:grid;grid-template-columns:1fr 1fr;gap:12px;padding-top:12px}
.gbody:empty{display:flex;align-items:center;justify-content:center;min-height:58px;font-size:12px;opacity:.5}
.gbody:empty::before{content:"Drag buttons here"}
.sd.part{background:#ffb454;box-shadow:0 0 6px rgba(255,180,84,.8)}
#pl:empty{display:none}
#mv .btns button{flex:none}
#mv .btns button.gtxt{color:#34e0a1}
#mv .btns button.rtxt{color:#ff5d73}
#pv .btns button,#gdw button{flex:none}
.nm{font-size:15px;font-weight:700}
.pt{font-size:11px;opacity:.55;font-variant-numeric:tabular-nums;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}
.svc.lifted{opacity:.25}
.svc.ghost{position:fixed;z-index:20;margin:0;pointer-events:none;transform:scale(1.07);background:rgba(70,84,128,.96);box-shadow:0 14px 34px rgba(0,0,0,.5);-webkit-backdrop-filter:none;backdrop-filter:none}
.card{cursor:pointer}
.card{scroll-margin-top:70px}
.card.big{grid-column:1/-1;order:-1}
.card.big .ringwrap{width:min(40vw,160px)}
.detail{display:none;width:100%;flex-direction:column;gap:16px;padding:6px 4px 2px}
.card.big .detail{display:flex;animation:fade .3s ease}
@keyframes fade{from{opacity:0;transform:translateY(-6px)}}
.bigval{font-size:24px;font-weight:800;text-align:center;font-variant-numeric:tabular-nums}
.bigval small{font-size:14px;opacity:.6;font-weight:700}
.dgrid{display:grid;grid-template-columns:repeat(auto-fit,minmax(92px,1fr));gap:8px}
.ds{background:rgba(255,255,255,.06);border-radius:14px;padding:10px 6px;text-align:center;display:flex;flex-direction:column;gap:4px;min-width:0}
.ds b{font-size:14px;font-variant-numeric:tabular-nums;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}
.ds span{font-size:10px;letter-spacing:.12em;opacity:.55;font-weight:700}
.dh{font-size:11px;font-weight:700;letter-spacing:.14em;opacity:.6;margin:0 2px 8px}
.gbox{background:rgba(255,255,255,.05);border-radius:16px;padding:10px}
.gbox svg{width:100%;height:80px;display:block}
.gcap{display:flex;justify-content:space-between;font-size:10px;opacity:.5;margin-top:6px;letter-spacing:.06em}
.gwait{height:80px;display:flex;align-items:center;justify-content:center;font-size:12px;opacity:.5}
.prow{display:flex;align-items:center;gap:8px;font-size:13px;padding:5px 2px}
.pn{flex:0 0 36%;white-space:nowrap;overflow:hidden;text-overflow:ellipsis;font-weight:600}
.pb{flex:1;height:6px;border-radius:3px;background:rgba(255,255,255,.1);overflow:hidden}
.pb i{display:block;height:100%;border-radius:3px}
.pv{flex:0 0 auto;min-width:84px;text-align:right;font-variant-numeric:tabular-nums;opacity:.85;font-size:12px}
.cores{display:flex;gap:6px;height:46px}
.cores div{flex:1;background:rgba(255,255,255,.08);border-radius:6px;position:relative;overflow:hidden}
.cores i{position:absolute;left:0;right:0;bottom:0;border-radius:6px;transition:height .6s}
.drv{background:rgba(255,255,255,.05);border-radius:16px;padding:12px;display:flex;flex-direction:column;gap:10px;margin-bottom:10px}
.drvh{display:flex;justify-content:space-between;align-items:baseline;gap:8px}
.drvh b{font-size:15px}
.drvh span{font-size:12px;opacity:.7;font-variant-numeric:tabular-nums;white-space:nowrap}
.drv .pb{flex:none;width:100%;height:8px;border-radius:4px}
.svc{position:relative}
.sd{position:absolute;top:14px;right:14px;width:8px;height:8px;border-radius:50%;background:rgba(255,255,255,.25)}
.sd.up{background:#34e0a1;box-shadow:0 0 6px rgba(52,224,161,.8)}
.sd.dn{background:#ff5d73;box-shadow:0 0 6px rgba(255,93,115,.8)}
#svc.wide .svc,#hid.wide .svc{padding-right:15px}
.wide .sd{top:50%;right:6px;width:6px;height:6px;margin-top:-3px;box-shadow:none}
.hint{text-align:center;font-size:10px;letter-spacing:.14em;opacity:.35;font-weight:700}
.monbtn{display:flex;align-items:center;justify-content:space-between;width:100%;padding:14px 18px;border-radius:20px;border:1px solid rgba(255,255,255,.1);background:rgba(255,255,255,.07);color:#fff;font-family:inherit;font-size:13px;font-weight:800;letter-spacing:.14em}
.monbtn:active{background:rgba(255,255,255,.14)}
.monbtn svg{width:18px;height:18px;opacity:.8;transition:transform .35s}
.monbtn.open svg{transform:rotate(180deg)}
.mon{display:grid;grid-template-rows:0fr;transition:grid-template-rows .4s cubic-bezier(.22,1,.36,1)}
.mon.open{grid-template-rows:1fr}
.moninner{min-height:0;overflow:hidden}
.monpad{display:flex;flex-direction:column;gap:16px;padding-top:16px}
.sechead{display:flex;align-items:center;justify-content:space-between;margin:6px 4px -4px}
.sechead .sec{margin:0}
.lay{font-size:12px;font-weight:700;color:#fff;background:rgba(255,255,255,.12);border:1px solid rgba(255,255,255,.14);border-radius:99px;padding:6px 12px;font-family:inherit}
.lay.on{background:#34e0a1;color:#06281d;border-color:transparent}
#svc.wide,#hid{display:grid;grid-template-columns:repeat(4,1fr);gap:6px}
.wide .svc{flex-direction:row;align-items:center;gap:4px;padding:0 6px;height:34px;border-radius:10px}
.wide .ic{font-size:13px}
.wide .nm{font-size:10px;white-space:nowrap;overflow:hidden;text-overflow:ellipsis;min-width:0}
.wide .pt{display:none}
#hid{min-height:46px;padding:6px;border:1px dashed rgba(255,255,255,.16);border-radius:14px}
#hid:empty{display:flex;align-items:center;justify-content:center;font-size:12px;opacity:.5}
#hid:empty::before{content:"Drag buttons here to hide them from the widget"}
#svc:empty{display:flex;align-items:center;justify-content:center;min-height:90px;padding:16px;border:1px dashed rgba(255,255,255,.16);border-radius:20px;font-size:13px;opacity:.6;text-align:center}
#svc:not(.wide):empty::before{content:"No buttons yet. Tap + below to add your first service."}
#svc.wide:empty::before{content:"Nothing on the widget yet. Drag buttons up from the panel below."}
#hid .svc{opacity:.7}
.svc.over{opacity:.4;border-style:dashed}
#wextra{flex-direction:column;gap:14px}
.wnote{font-size:12px;opacity:.55;line-height:1.4;padding:0 4px}
.addwrap{display:flex;justify-content:center;margin-top:2px}
.addbtn{width:52px;height:52px;border-radius:16px;border:1px solid rgba(255,255,255,.14);background:rgba(255,255,255,.1);color:#fff;font-size:28px;font-weight:300;display:flex;align-items:center;justify-content:center;font-family:inherit}
.addbtn:active{background:rgba(255,255,255,.22)}
#title{-webkit-user-select:none;user-select:none;-webkit-touch-callout:none}
.tw{position:relative;display:flex;align-items:center;gap:8px;min-width:0;cursor:pointer;-webkit-user-select:none;user-select:none;-webkit-touch-callout:none}
.tchev{width:14px;height:14px;flex:none;opacity:.6;transition:transform .3s cubic-bezier(.22,1,.36,1)}
.tw.open .tchev{transform:rotate(180deg)}
.pmenu{position:absolute;top:calc(100% + 10px);left:0;z-index:9;min-width:190px;padding:6px;border-radius:18px;background:#1e2640;border:1px solid rgba(255,255,255,.14);box-shadow:0 20px 60px rgba(0,0,0,.5);display:none;flex-direction:column;gap:2px;transform-origin:top left}
.tw.open .pmenu{display:flex;animation:pop .2s cubic-bezier(.22,1,.36,1)}
.pmenu button{display:flex;align-items:center;gap:10px;height:42px;padding:0 12px;border:0;border-radius:12px;background:none;color:#fff;font-family:inherit;font-size:15px;font-weight:700;text-align:left}
.pmenu button:active{background:rgba(255,255,255,.12)}
.pmenu button.red{color:#ff5d73}
.pmenu svg{width:18px;height:18px;flex:none}
#cvl{display:none;flex-direction:column;gap:12px}
.cnote{font-size:13px;opacity:.65;line-height:1.4;text-align:center;padding:0 4px}
.toast{position:fixed;left:50%;bottom:calc(env(safe-area-inset-bottom,0px) + 24px);z-index:30;max-width:86vw;padding:10px 16px;border-radius:18px;background:rgba(30,38,64,.96);border:1px solid rgba(255,255,255,.14);box-shadow:0 10px 30px rgba(0,0,0,.4);font-size:13px;font-weight:600;text-align:center;opacity:0;transform:translate(-50%,20px);transition:opacity .25s,transform .25s;pointer-events:none}
.toast.show{opacity:1;transform:translate(-50%,0)}
#ov,#mv,#rv,#gv,#pv,#cv,#av,#dv{position:fixed;inset:0;z-index:10;display:none;align-items:center;justify-content:center;background:rgba(5,10,20,.55);-webkit-backdrop-filter:blur(8px);backdrop-filter:blur(8px);padding:20px;overflow:auto}
#ov.show,#mv.show,#rv.show,#gv.show,#pv.show,#cv.show,#av.show,#dv.show{display:flex}
.modal{width:min(86vw,340px);padding:22px 18px 18px;border-radius:30px;background:rgba(30,38,64,.94);border:1px solid rgba(255,255,255,.14);box-shadow:0 20px 60px rgba(0,0,0,.5);display:flex;flex-direction:column;gap:12px;animation:pop .22s cubic-bezier(.22,1,.36,1)}
@keyframes pop{from{transform:scale(.9);opacity:0}}
.modal h2{font-size:17px;font-weight:800;letter-spacing:.06em;text-align:center}
.f{display:flex;flex-direction:column;gap:5px}
.f label{font-size:11px;font-weight:700;letter-spacing:.12em;opacity:.6;margin-left:4px}
.in{display:flex;align-items:center;background:rgba(255,255,255,.09);border:1px solid rgba(255,255,255,.12);border-radius:14px;padding:0 12px;height:44px}
.in span{opacity:.55;font-size:16px;margin-right:2px}
.in input{flex:1;min-width:0;background:none;border:0;outline:0;color:#fff;font-size:16px;font-family:inherit;height:100%}
.in input::placeholder{color:rgba(255,255,255,.3)}
.in select{background:none;border:0;outline:0;color:#fff;font-size:16px;font-family:inherit;opacity:.85;-webkit-appearance:none;appearance:none;padding:0;margin:0}
.in .chev{font-size:11px;opacity:.5;margin:0 8px 0 3px}
.tg{display:flex;align-items:center;justify-content:space-between;padding:0 4px}
.tg label{font-size:11px;font-weight:700;letter-spacing:.12em;opacity:.6}
.sw{position:relative;width:46px;height:28px;border-radius:14px;background:rgba(255,255,255,.18);transition:background .2s}
.sw i{position:absolute;top:3px;left:3px;width:22px;height:22px;border-radius:50%;background:#fff;transition:transform .2s;box-shadow:0 1px 4px rgba(0,0,0,.35)}
.sw.on{background:#34e0a1}
.sw.on i{transform:translateX(18px)}
.err{font-size:12px;color:#ff5d73;text-align:center;min-height:14px}
.btns{display:flex;gap:10px}
.btns button{flex:1;height:44px;border:0;border-radius:14px;font-size:15px;font-weight:700;font-family:inherit;color:#fff;background:rgba(255,255,255,.12)}
.btns button.ok{background:#34e0a1;color:#06281d}
.btns button.del{background:#ff5d73;color:#fff}
.btns.col{flex-direction:column}
.hr{display:flex;align-items:center;gap:8px;flex:none}
.gear{width:32px;height:32px;border-radius:50%;border:0;padding:0;background:rgba(255,255,255,.1);color:#fff;display:flex;align-items:center;justify-content:center}
.gear:active{background:rgba(255,255,255,.2)}
.gear svg{width:17px;height:17px;opacity:.85}
body.monfixed #monb{display:none}
body.monfixed .monpad{padding-top:0}
.tw.nop{cursor:default}
.tw.nop .tchev{display:none}
body.c3 #svc:not(.wide),body.c3 .gbody{grid-template-columns:repeat(3,1fr);gap:10px}
body.c3 #svc:not(.wide) .svc,body.c3 .gbody .svc{padding:12px}
body.c3 #svc:not(.wide) .nm,body.c3 .gbody .nm{font-size:13px;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}
body.c3 #svc:not(.wide) .sd,body.c3 .gbody .sd{top:12px;right:12px}
#sv{position:fixed;inset:0;z-index:12;display:none;align-items:flex-start;justify-content:center;background:rgba(5,10,20,.55);-webkit-backdrop-filter:blur(8px);backdrop-filter:blur(8px);padding:calc(env(safe-area-inset-top,0px) + 16px) 12px calc(env(safe-area-inset-bottom,0px) + 24px);overflow:auto;-webkit-overflow-scrolling:touch}
#sv.show{display:flex}
.smod{width:min(94vw,460px);gap:18px;padding-top:14px}
.shd{display:flex;align-items:center;justify-content:space-between;gap:10px}
.shd h2{flex:1}
.shd button{height:36px;padding:0 14px;border:0;border-radius:12px;font-size:15px;font-weight:700;font-family:inherit;color:#fff;background:rgba(255,255,255,.12)}
.shd button.ok{background:#34e0a1;color:#06281d}
#sbody{display:flex;flex-direction:column;gap:20px}
.ssec{display:flex;flex-direction:column;gap:10px}
.ssec>.sec{margin:0 4px}
.scard{display:flex;flex-direction:column;gap:14px;padding:14px;border-radius:18px;background:rgba(255,255,255,.05);border:1px solid rgba(255,255,255,.08)}
.shint{font-size:11px;opacity:.45;margin:-6px 4px 0;line-height:1.4}
.srow{display:flex;align-items:center;gap:10px;min-width:0}
.srow .st{flex:1;min-width:0}
.srow .st b{display:block;font-size:14px;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}
.srow .st span{display:block;font-size:11px;opacity:.5;margin-top:2px}
.srow .chip{margin-left:0}
.trow{display:grid;grid-template-columns:60px 1fr 1fr;gap:8px;align-items:center}
.trow b{font-size:13px;opacity:.8;margin-left:4px}
.trow .in{height:40px;min-width:0}
.trow>*{min-width:0}
.trow.th span{font-size:10px;font-weight:700;letter-spacing:.12em;opacity:.5;text-align:center}
.sres{display:flex;flex-direction:column;gap:6px;font-size:12px}
.sres div{display:flex;gap:8px;align-items:flex-start;line-height:1.35}
.sres i{flex:none;width:8px;height:8px;border-radius:50%;margin-top:4px;background:#34e0a1}
.sres .bad i{background:#ff5d73}
.sbtns{display:flex;flex-direction:column;gap:8px}
.sbtns button{height:44px;border:0;border-radius:14px;font-size:15px;font-weight:700;font-family:inherit;color:#fff;background:rgba(255,255,255,.12)}
.sbtns button:active{background:rgba(255,255,255,.2)}
.sbtns button.rtxt{color:#ff5d73}
.swait{text-align:center;font-size:13px;opacity:.6;padding:18px 0}
.sbusy{opacity:.5;pointer-events:none}
.serr{font-size:12px;color:#ff5d73;text-align:center;min-height:14px}
/* ---- Theme & look (Settings). Defaults match the original look. ---- */
:root{--bg:#0b1220;--g1:18,194,165;--g2:106,92,255;--acc:#34e0a1;--acc-ink:#06281d;--panel:rgba(30,38,64,.94);--panel2:#1e2640;--glass:.07;--glowop:.5}
html,body{background:var(--bg)}
.b1{background:radial-gradient(circle,rgba(var(--g1),.75) 0%,rgba(var(--g1),0) 68%)}
.b2{background:radial-gradient(circle,rgba(var(--g2),.75) 0%,rgba(var(--g2),0) 68%)}
.blob{opacity:var(--glowop);transition:opacity .4s}
body.noanim .blob{animation:none;will-change:auto}
.sw.on{background:var(--acc)}
.seg{display:flex;gap:3px;padding:3px;background:rgba(255,255,255,.09);border:1px solid rgba(255,255,255,.12);border-radius:14px}
.seg button{flex:1;min-width:0;height:38px;border:0;border-radius:11px;background:none;color:#fff;font-size:14px;font-weight:700;font-family:inherit;opacity:.75;display:flex;align-items:center;justify-content:center;gap:7px}
.seg button i{flex:none;width:14px;height:14px;border-radius:50%;border:2px solid rgba(255,255,255,.5);box-sizing:border-box}
.seg button.on{background:var(--acc);color:var(--acc-ink);opacity:1}
.seg button.on i{border:4px solid var(--acc-ink);background:var(--acc)}
.segw{flex-wrap:wrap}
.slogin{display:flex;flex-direction:column;gap:12px;padding:12px;border-radius:14px;background:rgba(255,255,255,.04);border:1px solid rgba(255,255,255,.1)}
.slogin .srow{gap:8px}
.segw button{flex:1 1 30%}
.btns button.ok,.shd button.ok,.lay.on{background:var(--acc);color:var(--acc-ink)}
.modal{background:var(--panel)}
.pmenu{background:var(--panel2)}
.card,.svc,.monbtn,.netcard{background:rgba(255,255,255,var(--glass))}
body.sharp .card,body.sharp .netcard{border-radius:12px}
body.sharp .svc,body.sharp .monbtn,body.sharp .scard{border-radius:10px}
body.sharp .wide .svc{border-radius:6px}
body.sharp .modal{border-radius:16px}
body.sharp .in,body.sharp .btns button,body.sharp .sbtns button,body.sharp .addbtn{border-radius:8px}
body.frnd{font-family:ui-rounded,"SF Pro Rounded",-apple-system,system-ui,sans-serif}
body.fserif{font-family:ui-serif,"New York",Georgia,serif}
body.compact #svc:not(.wide) .svc,body.compact .gbody .svc{padding:10px 12px;gap:2px}
body.compact #svc:not(.wide) .ic,body.compact .gbody .ic{font-size:18px}
body.compact #svc:not(.wide) img.ic,body.compact .gbody img.ic{width:22px;height:22px}
body.compact #svc:not(.wide) .nm,body.compact .gbody .nm{font-size:13px}
body.compact #svc:not(.wide) .sd,body.compact .gbody .sd{top:10px;right:10px}
body.compact #svc:not(.wide),body.compact .gbody{gap:8px}
/* swatches in Settings */
.swl{display:flex;flex-wrap:wrap;gap:10px}
.swt{width:44px;height:44px;border-radius:50%;border:2px solid rgba(255,255,255,.15);padding:0;position:relative}
.swt.on{border-color:#fff;box-shadow:0 0 0 3px rgba(255,255,255,.18)}
.swn{font-size:12px;opacity:.6;margin:2px 4px 0}
/* ===================== LIQUID GLASS LOOK (Settings > Theme & look > Style) =====================
   Every rule below only applies while body has the "glass" class. :where() adds no specificity,
   so these rules override the classic ones exactly as the Liquid Glass edition does. */
:root{--spring:cubic-bezier(.34,1.56,.64,1);--soft:cubic-bezier(.22,1,.36,1);--glassdark:rgba(14,18,34,.38)}
@keyframes drift2{0%{transform:translate(-12vmax,-8vmax) scale(.85)}100%{transform:translate(14vmax,16vmax) scale(1.15)}}
@keyframes liftup{from{transform:scale(.92)}}
@keyframes rise{from{opacity:0;transform:translateY(16px) scale(.92)}}
body:where(.glass){-webkit-font-smoothing:antialiased}
:where(body.glass) h1{font-size:24px;letter-spacing:.1em}
:where(body.glass) .blob{width:110vmax;height:110vmax;animation-duration:18s;filter:saturate(1.25)}
:where(body.glass) .b3{position:absolute;width:75vmax;height:75vmax;border-radius:50%;top:35%;left:15%;background:radial-gradient(circle,color-mix(in srgb,var(--acc) 45%,transparent) 0%,transparent 65%);opacity:calc(var(--glowop)*.6);animation:drift2 23s ease-in-out infinite alternate;will-change:transform}
body:where(.glass).noanim .b3{animation:none}
:where(body.glass) header{position:relative;z-index:8}
:where(body.glass) .topfade{position:fixed;top:0;left:0;right:0;height:calc(env(safe-area-inset-top,0px) + 44px);z-index:9;pointer-events:none;-webkit-backdrop-filter:blur(14px) saturate(160%);backdrop-filter:blur(14px) saturate(160%);-webkit-mask-image:linear-gradient(#000 35%,transparent);mask-image:linear-gradient(#000 35%,transparent)}
:where(body.glass) .card,:where(body.glass) .netcard,:where(body.glass) .svc,:where(body.glass) .monbtn,:where(body.glass) .live,:where(body.glass) .gear,:where(body.glass) .addbtn,:where(body.glass) .lay,:where(body.glass) .chip,:where(body.glass) .btns button,:where(body.glass) .sbtns button,:where(body.glass) .shd button,:where(body.glass) .scard,:where(body.glass) .seg,:where(body.glass) .svc:active,:where(body.glass) .monbtn:active,:where(body.glass) .addbtn:active,:where(body.glass) .gear:active,:where(body.glass) .chip:active,:where(body.glass) .sbtns button:active{background:linear-gradient(165deg,rgba(255,255,255,calc(var(--glass)*2.3)) 0%,rgba(255,255,255,calc(var(--glass)*.6)) 52%,rgba(255,255,255,calc(var(--glass)*1.3)) 100%);
  border:0;
  -webkit-backdrop-filter:blur(22px) saturate(185%);backdrop-filter:blur(22px) saturate(185%);
  box-shadow:inset 0 1px 0 rgba(255,255,255,.26),inset 0 -1px 0 rgba(255,255,255,.05),0 12px 30px -10px rgba(0,0,0,.5);}
:where(body.glass) .card,:where(body.glass) .netcard,:where(body.glass) .svc,:where(body.glass) .monbtn,:where(body.glass) .live,:where(body.glass) .gear,:where(body.glass) .addbtn,:where(body.glass) .lay,:where(body.glass) .chip,:where(body.glass) .btns button,:where(body.glass) .sbtns button,:where(body.glass) .shd button,:where(body.glass) .scard,:where(body.glass) .modal,:where(body.glass) .pmenu button,:where(body.glass) .seg{position:relative}
:where(body.glass) .modal,:where(body.glass) .pmenu,:where(body.glass) .toast{background:linear-gradient(165deg,rgba(255,255,255,.17),rgba(255,255,255,.04) 55%,rgba(255,255,255,.08)),var(--glassdark);
  border:0;-webkit-backdrop-filter:blur(40px) saturate(190%);backdrop-filter:blur(40px) saturate(190%);
  box-shadow:inset 0 1px 0 rgba(255,255,255,.3),0 30px 70px -10px rgba(0,0,0,.6);}
:where(body.glass) .card::before,:where(body.glass) .netcard::before,:where(body.glass) .svc::before,:where(body.glass) .monbtn::before,:where(body.glass) .live::before,:where(body.glass) .gear::before,:where(body.glass) .addbtn::before,:where(body.glass) .lay::before,:where(body.glass) .chip::before,:where(body.glass) .btns button::before,:where(body.glass) .sbtns button::before,:where(body.glass) .shd button::before,:where(body.glass) .scard::before,:where(body.glass) .modal::before,:where(body.glass) .pmenu::before,:where(body.glass) .toast::before,:where(body.glass) .seg::before{content:"";position:absolute;inset:0;border-radius:inherit;padding:1px;pointer-events:none;
  background:linear-gradient(140deg,rgba(255,255,255,.62),rgba(255,255,255,.14) 28%,rgba(255,255,255,0) 48%,rgba(255,255,255,.06) 68%,rgba(255,255,255,.34));
  -webkit-mask:linear-gradient(#000 0 0) content-box,linear-gradient(#000 0 0);-webkit-mask-composite:xor;mask-composite:exclude;}
:where(body.glass) .card::after,:where(body.glass) .netcard::after,:where(body.glass) .svc::after,:where(body.glass) .monbtn::after,:where(body.glass) .live::after,:where(body.glass) .gear::after,:where(body.glass) .addbtn::after,:where(body.glass) .lay::after,:where(body.glass) .chip::after,:where(body.glass) .btns button::after,:where(body.glass) .sbtns button::after,:where(body.glass) .shd button::after,:where(body.glass) .pmenu button::after,:where(body.glass) .seg button::after{content:"";position:absolute;inset:0;border-radius:inherit;pointer-events:none;
  background:radial-gradient(circle at var(--mx,50%) var(--my,50%),rgba(255,255,255,.34),rgba(255,255,255,.08) 35%,rgba(255,255,255,0) 65%);
  opacity:0;transition:opacity .6s var(--soft);}
:where(body.glass) .prs::after{opacity:1;transition:opacity .07s}
:where(body.glass) .card,:where(body.glass) .netcard,:where(body.glass) .svc,:where(body.glass) .monbtn,:where(body.glass) .live,:where(body.glass) .gear,:where(body.glass) .addbtn,:where(body.glass) .lay,:where(body.glass) .chip,:where(body.glass) .btns button,:where(body.glass) .sbtns button,:where(body.glass) .shd button,:where(body.glass) .pmenu button,:where(body.glass) .swt,:where(body.glass) .seg button{transition:transform .6s var(--spring),opacity .35s,background .3s,box-shadow .3s;}
:where(body.glass) .prs{transform:scale(.94)}
:where(body.glass) .svc.prs,:where(body.glass) .grph.prs{transform:scale(.92);transition:transform .5s cubic-bezier(.2,.8,.2,1),opacity .3s}
:where(body.glass) .card.prs{transform:scale(.965)}
:where(body.glass) .card.big.prs{transform:scale(.99)}
:where(body.glass) .addbtn.prs{transform:scale(.88) rotate(90deg)}
:where(body.glass) .gear svg{transition:transform .7s var(--spring)}
:where(body.glass) .gear.prs svg{transform:rotate(75deg)}
:where(body.glass) .swt.prs{transform:scale(.85)}
:where(body.glass) .card,:where(body.glass) .netcard{border-radius:30px}
:where(body.glass) .svc{border-radius:24px}
:where(body.glass) .monbtn{border-radius:24px}
:where(body.glass) .modal{border-radius:36px}
:where(body.glass) .live{padding:7px 13px}
:where(body.glass) .gear{width:36px;height:36px}
:where(body.glass) .addbtn{width:58px;height:58px;border-radius:50%;font-size:30px}
:where(body.glass) .lay{padding:7px 14px}
:where(body.glass) .seg{border-radius:16px}
:where(body.glass) .seg button{position:relative;border-radius:13px}
:where(body.glass) .lay.on,:where(body.glass) .btns button.ok,:where(body.glass) .shd button.ok,:where(body.glass) .lay.on:active,:where(body.glass) .seg button.on{background:linear-gradient(180deg,rgba(255,255,255,.42),rgba(255,255,255,0) 58%),var(--acc);color:var(--acc-ink);box-shadow:inset 0 1px 0 rgba(255,255,255,.5),0 8px 22px -8px var(--acc)}
:where(body.glass) .btns button.del{background:linear-gradient(180deg,rgba(255,255,255,.38),rgba(255,255,255,0) 58%),#ff5d73;box-shadow:inset 0 1px 0 rgba(255,255,255,.45),0 8px 22px -8px #ff5d73}
:where(body.glass) .btns button,:where(body.glass) .sbtns button{height:48px;border-radius:16px}
:where(body.glass) .nm{font-weight:650}
:where(body.glass) #hid .svc{opacity:.7}
:where(body.glass) .svc.over{opacity:.32;border-style:none}
:where(body.glass) .wide .svc{border-radius:12px}
:where(body.glass) .svc.lifted,:where(body.glass) .grp.lifted{opacity:.15;transform:scale(.88)}
:where(body.glass) .svc.ghost,:where(body.glass) .grph.ghost{background:linear-gradient(165deg,rgba(255,255,255,.3),rgba(255,255,255,.1)),rgba(30,36,60,.35);
  -webkit-backdrop-filter:blur(24px) saturate(190%);backdrop-filter:blur(24px) saturate(190%);
  box-shadow:inset 0 1px 0 rgba(255,255,255,.4),0 28px 60px -8px rgba(0,0,0,.65);
  transform:scale(1.1);animation:liftup .45s var(--spring);transition:none;}
:where(body.glass) .ds,:where(body.glass) .gbox,:where(body.glass) .drv{background:rgba(255,255,255,.06);box-shadow:inset 0 1px 0 rgba(255,255,255,.12),inset 0 0 0 .5px rgba(255,255,255,.06)}
:where(body.glass) .ds{border-radius:16px}
:where(body.glass) .pb,:where(body.glass) .cores div{background:rgba(255,255,255,.08);box-shadow:inset 0 1px 2px rgba(0,0,0,.25)}
:where(body.glass) .pb i,:where(body.glass) .cores i{box-shadow:inset 0 1px 0 rgba(255,255,255,.4)}
:where(body.glass) .track{stroke:rgba(255,255,255,.09)}
:where(body.glass) .arc{transition:stroke-dashoffset 1.4s var(--soft),stroke .6s}
:where(body.glass) .ringwrap::before{content:"";position:absolute;inset:17%;border-radius:50%;background:radial-gradient(circle at 34% 28%,rgba(255,255,255,.16),rgba(255,255,255,.02) 70%);box-shadow:inset 0 1px 0 rgba(255,255,255,.28),inset 0 -10px 20px rgba(0,0,0,.18)}
:where(body.glass) .sd{transition:background .5s,box-shadow .5s}
:where(body.glass) .sd.up{box-shadow:0 0 0 3px rgba(52,224,161,.16),0 0 10px rgba(52,224,161,.9)}
:where(body.glass) .sd.dn{box-shadow:0 0 0 3px rgba(255,93,115,.16),0 0 10px rgba(255,93,115,.9)}
:where(body.glass) .sd.part{box-shadow:0 0 0 3px rgba(255,180,84,.16),0 0 10px rgba(255,180,84,.9)}
:where(body.glass) .in{background:rgba(255,255,255,.06);border:0;border-radius:16px;box-shadow:inset 0 1px 3px rgba(0,0,0,.3),inset 0 0 0 .5px rgba(255,255,255,.14);transition:box-shadow .35s var(--soft),background .35s}
:where(body.glass) .in:focus-within{background:rgba(255,255,255,.11);box-shadow:inset 0 1px 3px rgba(0,0,0,.25),0 0 0 1.5px var(--acc),0 0 22px -4px var(--acc)}
:where(body.glass) .sw{background:rgba(120,120,135,.32);box-shadow:inset 0 1px 3px rgba(0,0,0,.3),inset 0 0 0 .5px rgba(255,255,255,.12);transition:background .4s var(--soft)}
:where(body.glass) .sw.on{background:var(--acc)}
:where(body.glass) .sw i{transition:transform .5s var(--spring),width .35s var(--spring),left .35s var(--spring),background .3s,box-shadow .3s}
:where(body.glass) .sw:active i{width:30px;background:rgba(255,255,255,.6);box-shadow:inset 0 1px 1px rgba(255,255,255,.95),inset 0 -1px 2px rgba(0,0,0,.1),0 3px 10px rgba(0,0,0,.35)}
:where(body.glass) .sw.on:active i{transform:translateX(10px)}
:where(body.glass) .swt{transition:transform .5s var(--spring),box-shadow .3s,border-color .3s}
:where(body.glass) .swt.on{transform:scale(1.08)}
:where(body.glass) #ov,:where(body.glass) #mv,:where(body.glass) #rv,:where(body.glass) #gv,:where(body.glass) #pv,:where(body.glass) #cv,:where(body.glass) #av,:where(body.glass) #dv,:where(body.glass) #sv{display:flex!important;visibility:hidden;opacity:0;pointer-events:none;background:rgba(0,0,0,.28);
  -webkit-backdrop-filter:blur(0);backdrop-filter:blur(0);
  transition:opacity .3s var(--soft),visibility 0s linear .35s,-webkit-backdrop-filter .35s,backdrop-filter .35s;}
:where(body.glass) #ov.show,:where(body.glass) #mv.show,:where(body.glass) #rv.show,:where(body.glass) #gv.show,:where(body.glass) #pv.show,:where(body.glass) #cv.show,:where(body.glass) #av.show,:where(body.glass) #dv.show,:where(body.glass) #sv.show{visibility:visible;opacity:1;pointer-events:auto;-webkit-backdrop-filter:blur(10px);backdrop-filter:blur(10px);
  transition:opacity .25s var(--soft),visibility 0s,-webkit-backdrop-filter .4s,backdrop-filter .4s;}
:where(body.glass) .modal{animation:none;transform:scale(.84) translateY(18px);opacity:0;transition:transform .3s var(--soft),opacity .22s}
:where(body.glass) .show>.modal{transform:none;opacity:1;transition:transform .6s var(--spring),opacity .25s}
:where(body.glass) #sv>.modal{transform:translateY(90px) scale(.96)}
:where(body.glass) #sv.show>.modal{transform:none}
:where(body.glass) .show>.modal>*{animation:rise .55s var(--spring) both}
:where(body.glass) .show>.modal>*:nth-child(2){animation-delay:.03s}
:where(body.glass) .show>.modal>*:nth-child(3){animation-delay:.06s}
:where(body.glass) .show>.modal>*:nth-child(4){animation-delay:.09s}
:where(body.glass) .show>.modal>*:nth-child(5){animation-delay:.12s}
:where(body.glass) .show>.modal>*:nth-child(6){animation-delay:.15s}
:where(body.glass) .show>.modal>*:nth-child(7){animation-delay:.18s}
:where(body.glass) .show>.modal>*:nth-child(n+8){animation-delay:.21s}
:where(body.glass) .show>.modal .btns.col button{animation:rise .55s var(--spring) both}
:where(body.glass) .show>.modal .btns.col button:nth-child(2){animation-delay:.05s}
:where(body.glass) .show>.modal .btns.col button:nth-child(3){animation-delay:.1s}
:where(body.glass) .show>.modal .btns.col button:nth-child(4){animation-delay:.15s}
:where(body.glass) .show>.modal .btns.col button:nth-child(n+5){animation-delay:.2s}
:where(body.glass) .pmenu{background:linear-gradient(165deg,rgba(255,255,255,.16),rgba(255,255,255,.03) 55%,rgba(255,255,255,.07)),rgba(16,20,36,.8)}
:where(body.glass) .pmenu{display:flex!important;visibility:hidden;opacity:0;transform:scale(.35) translate(-14px,-30px);transform-origin:14px 0;border-radius:22px;
  transition:transform .3s var(--soft),opacity .18s,visibility 0s linear .3s;animation:none!important}
:where(body.glass) .tw.open .pmenu{visibility:visible;opacity:1;transform:none;transition:transform .55s var(--spring),opacity .18s,visibility 0s;animation:none!important}
:where(body.glass) .tw.open .pmenu button{animation:rise .5s var(--spring) both}
:where(body.glass) .tw.open .pmenu button:nth-child(2){animation-delay:.06s}
:where(body.glass) .pmenu button{border-radius:14px}
:where(body.glass) .tchev{transition:transform .5s var(--spring)}
:where(body.glass) .mon{transition:grid-template-rows .6s var(--soft)}
:where(body.glass) .moninner{margin:0 -16px;padding:0 16px;opacity:0;transition:opacity .25s}
:where(body.glass) .mon.open>.moninner{opacity:1;transition:opacity .5s .06s}
:where(body.glass) .monpad{padding-bottom:16px}
:where(body.glass) .gbody{padding-bottom:14px}
:where(body.glass) .monbtn svg{transition:transform .6s var(--spring)}
:where(body.glass) .toast{border-radius:22px;padding:12px 18px;transform:translate(-50%,40px) scale(.85);transition:opacity .3s,transform .6s var(--spring)}
:where(body.glass) .toast.show{transform:translate(-50%,0) scale(1)}
:where(body.glass) .detail.fresh>*{animation:rise .6s var(--spring) both}
:where(body.glass) .detail.fresh>*:nth-child(2){animation-delay:.05s}
:where(body.glass) .detail.fresh>*:nth-child(3){animation-delay:.1s}
:where(body.glass) .detail.fresh>*:nth-child(4){animation-delay:.15s}
:where(body.glass) .detail.fresh>*:nth-child(5){animation-delay:.2s}
:where(body.glass) .detail.fresh>*:nth-child(n+6){animation-delay:.25s}
:where(body.glass) .card.big .detail{animation:none}
:where(body.glass) .enter{animation:rise .75s var(--spring) both;animation-delay:calc(var(--i,0) * 32ms)}
:where(body.glass) .intro{animation:rise .9s var(--spring) both}
body:where(.glass):not(.ready) .enter,body:where(.glass):not(.ready) .intro{animation-play-state:paused}
:where(body.glass) .ssec.enter{animation-delay:calc(var(--i,0) * 55ms)}
@media (prefers-reduced-motion:reduce){body:where(.glass) .blob,body:where(.glass) .b3{animation:none}body:where(.glass) .enter,body:where(.glass) .intro{animation-duration:.01s}}
/* ---------- startup screen (Settings > Theme & look > Startup animation; works with either style) ---------- */
#splash{position:fixed;inset:0;z-index:100;display:flex;flex-direction:column;align-items:center;justify-content:center;gap:26px;background:var(--bg);overflow:hidden;
  transition:opacity .7s var(--soft),transform .9s var(--soft),filter .7s var(--soft)}
#splash.out{opacity:0;transform:scale(1.25);filter:blur(16px);pointer-events:none}
.sp-glow{position:absolute;width:120vmax;height:120vmax;border-radius:50%;opacity:calc(var(--glowop) * 1.3);animation:spglow 2.4s var(--soft) both}
.sp-g1{background:radial-gradient(circle,rgba(var(--g1),.7),rgba(var(--g1),0) 60%);top:-60vmax;left:-50vmax}
.sp-g2{background:radial-gradient(circle,rgba(var(--g2),.7),rgba(var(--g2),0) 60%);bottom:-60vmax;right:-50vmax}
@keyframes spglow{from{transform:scale(.4);opacity:0}}
.sp-orb{position:relative;width:138px;height:138px;border-radius:42px;display:flex;align-items:center;justify-content:center;
  background:linear-gradient(160deg,rgba(255,255,255,.22),rgba(255,255,255,.04) 55%,rgba(255,255,255,.1));
  -webkit-backdrop-filter:blur(30px) saturate(190%);backdrop-filter:blur(30px) saturate(190%);
  box-shadow:inset 0 1px 0 rgba(255,255,255,.45),inset 0 -1px 0 rgba(255,255,255,.08),0 30px 60px -12px rgba(0,0,0,.6);
  animation:orbin 1s var(--spring) both;overflow:hidden}
.sp-orb::before{content:"";position:absolute;inset:0;border-radius:inherit;padding:1.2px;background:linear-gradient(140deg,rgba(255,255,255,.8),rgba(255,255,255,.1) 35%,rgba(255,255,255,0) 55%,rgba(255,255,255,.45));-webkit-mask:linear-gradient(#000 0 0) content-box,linear-gradient(#000 0 0);-webkit-mask-composite:xor;mask-composite:exclude}
.sp-orb::after{content:"";position:absolute;top:-50%;left:-80%;width:60%;height:200%;background:linear-gradient(90deg,transparent,rgba(255,255,255,.35),transparent);transform:rotate(20deg);animation:sweep 1.1s .55s var(--soft) both}
@keyframes sweep{from{left:-80%}to{left:150%}}
@keyframes orbin{0%{transform:scale(.2) rotate(-25deg);opacity:0;border-radius:69px}100%{transform:none;opacity:1}}
.sp-orb svg{position:absolute;inset:13px;width:auto;height:auto;overflow:visible}
.sp-tr{fill:none;stroke:rgba(255,255,255,.12);stroke-width:8}
.sp-arc{fill:none;stroke:var(--acc);stroke-width:8;stroke-linecap:round;stroke-dasharray:301.6;stroke-dashoffset:301.6;transform:rotate(-90deg);transform-origin:60px 60px;filter:drop-shadow(0 0 8px var(--acc));animation:spdraw 1s .2s var(--soft) forwards}
@keyframes spdraw{to{stroke-dashoffset:52}}
.sp-b{position:relative;font-size:40px;font-weight:900;letter-spacing:-.05em;animation:spb .7s .4s var(--spring) both}
@keyframes spb{from{opacity:0;transform:scale(.4)}}
.sp-name{font-size:14px;font-weight:800;letter-spacing:.34em;opacity:.85;animation:spname 1s .45s var(--soft) both;padding-left:.34em}
@keyframes spname{from{opacity:0;letter-spacing:1.1em;filter:blur(8px)}}
body:not(.boot) #splash,body:not(.boot) #splash *{animation-play-state:paused!important}
body:not(.splashon) #splash{display:none}
</style></head><body>
<div id="splash" onclick="skipSplash()"><div class="sp-glow sp-g1"></div><div class="sp-glow sp-g2"></div><div class="sp-orb"><svg viewBox="0 0 120 120"><circle class="sp-tr" cx="60" cy="60" r="48"/><circle class="sp-arc" cx="60" cy="60" r="48"/></svg><div class="sp-b">-b</div></div><div class="sp-name">-BOARD</div></div>
<div class="bg"><div class="blob b1"></div><div class="blob b2"></div><div class="b3"></div></div>
<div class="topfade"></div>
<main>
<header class="intro"><div class="tw" id="tw"><h1 id="title"></h1><svg class="tchev" viewBox="0 0 24 24"><path d="M6 9l6 6 6-6" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"/></svg><div class="pmenu"><button onclick="openPower('restart')"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><path d="M20 12a8 8 0 1 1-2.34-5.66"/><path d="M20 4v5h-5"/></svg>Reboot</button><button class="red" onclick="openPower('off')"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round"><path d="M12 3v9"/><path d="M6.3 6.8a8 8 0 1 0 11.4 0"/></svg>Shut down</button></div></div><div class="hr"><div class="live"><div class="dot wait" id="dot"></div><span id="st">...</span></div><button class="gear" onclick="openSettings()" aria-label="Settings"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z"/></svg></button></div></header>
<div class="sdots intro" id="sdots" style="animation-delay:.04s"></div>
<div class="monwrap intro" style="animation-delay:.08s"><button class="monbtn" id="monb" onclick="toggleMon()"><span>MONITORING</span><span id="monst"></span><svg viewBox="0 0 24 24"><path d="M6 9l6 6 6-6" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"/></svg></button><div class="mon" id="mon"><div class="moninner"><div class="monpad">
<div id="grid"></div>
<div class="netcard">
<div class="net"><span class="arrow dn">&#8595;</span><b id="down">0 B/s</b><span>DOWNLOAD</span></div>
<div class="net"><span class="arrow up">&#8593;</span><b id="up">0 B/s</b><span>UPLOAD</span></div>
</div>
</div></div></div></div>
<div class="sechead intro" style="animation-delay:.14s"><div class="sec" id="sect">SERVICES</div><button class="lay" id="layb" onclick="toggleMode()">Widget layout</button></div>
<div id="svc"></div>
<div id="wextra" style="display:none"><div class="wnote" id="wnote">The large widget fits 24 buttons. Dimmed ones will not show. Press, hold and drag to reorder, or drag a button between the two panels to show or hide it.</div><div id="hidwrap" style="display:none"><div class="sec" style="margin:0 4px 8px">NOT ON WIDGET</div><div id="hid" class="wide"></div></div></div>
<div class="addwrap intro" style="animation-delay:.3s" id="addwrap"><button class="addbtn" onclick="openAdd()">+</button></div>
</main>
<div id="cv" onclick="if(event.target===this&&Date.now()>=ign)closePower()">
<div class="modal">
<h2 id="cvt">REBOOT SERVER?</h2>
<div class="cnote" id="cvn"></div>
<div id="cvl">
<div class="f"><label id="cul">USERNAME</label><div class="in"><input id="cu" type="text" autocapitalize="none" autocorrect="off" spellcheck="false" autocomplete="username"></div></div>
<div class="f"><label id="cpl">PASSWORD</label><div class="in"><input id="cp" type="password" autocomplete="current-password"></div></div>
<div class="fh">Asked once, then saved in the iOS Keychain.</div>
</div>
<div class="err" id="cer"></div>
<div class="btns"><button onclick="closePower()">Cancel</button><button class="del" id="cok" onclick="confirmPower()">Reboot</button></div>
</div>
</div>
<div class="toast" id="toast"></div>
<div id="sv" onclick="if(event.target===this&&Date.now()>=ign)closeSettings()">
<div class="modal smod">
<div class="shd"><button onclick="closeSettings()">Cancel</button><h2 id="sttl">SETTINGS</h2><button class="ok" id="ssave" onclick="saveSettings()">Save</button></div>
<div class="serr" id="serr"></div>
<div id="sbody"><div class="swait">Loading...</div></div>
</div>
</div>
<div id="ov" onclick="if(event.target===this&&Date.now()>=ign)closeAdd()">
<div class="modal">
<h2 id="at">ADD SERVICE</h2>
<div class="f"><label>SERVICE NAME</label><div class="in"><input id="fn" type="text" placeholder="e.g. Jellyfin" maxlength="24" autocomplete="off"></div></div>
<div class="f"><label>ADDRESS</label><div class="in"><select id="fs"><option value="http">http://</option><option value="https">https://</option></select><span class="chev">&#9662;</span><input id="fa" type="text" inputmode="url" placeholder="192.168.1.10" autocapitalize="none" autocorrect="off" spellcheck="false"></div></div>
<div class="tg"><label>USE PORT</label><div class="sw on" id="sw" onclick="togglePort()"><i></i></div></div>
<div class="f" id="pf"><label>PORT</label><div class="in"><input id="fp" type="text" inputmode="numeric" placeholder="8080" maxlength="5"></div></div>
<div class="f"><label>LOGO URL (OPTIONAL)</label><div class="in"><input id="fl" type="text" inputmode="url" placeholder="https://.../logo.png" autocapitalize="none" autocorrect="off" spellcheck="false" oninput="previewLogo('fl','flp')"><img id="flp" class="lp" alt="" onload="this.className='lp ok'" onerror="this.className='lp'"><button class="chip" type="button" onclick="guessLogo('fn','fl','flp','flh')">Auto</button></div><div class="fh" id="flh">Empty = emoji. Auto guesses from the name.</div></div>
<div class="tg" id="wtg"><label>SHOW ON WIDGET</label><div class="sw on" id="sw2" onclick="toggleWidget()"><i></i></div></div>
<div class="err" id="er"></div>
<div class="btns"><button onclick="closeAdd()">Cancel</button><button class="ok" id="okb" onclick="submitAdd()">Add</button></div>
</div>
</div>
<div id="av" onclick="if(event.target===this&&Date.now()>=ign)closeAddServer()">
<div class="modal" id="avm"></div>
</div>
<div id="dv" onclick="if(event.target===this&&Date.now()>=ign)closeDelServer()">
<div class="modal">
<h2 id="dvt">DELETE SERVER?</h2>
<div class="cnote" id="dvn"></div>
<div class="btns"><button onclick="closeDelServer()">Cancel</button><button class="del" onclick="confirmDelServer()">Delete</button></div>
</div>
</div>
<div id="rv" onclick="if(event.target===this&&Date.now()>=ign)closeRename()">
<div class="modal">
<h2>RENAME</h2>
<div class="f"><label>DASHBOARD NAME</label><div class="in"><input id="rn" type="text" maxlength="20" autocomplete="off"></div></div>
<div class="err" id="rer"></div>
<div class="btns"><button onclick="closeRename()">Cancel</button><button class="ok" onclick="saveRename()">Save</button></div>
</div>
</div>
<div id="mv" onclick="if(event.target===this&&Date.now()>=ign)closeMenu()">
<div class="modal">
<h2 id="mt"></h2>
<div class="btns col"><button id="sbtn" onclick="menuStack()">Add to stack</button><button class="gtxt" onclick="menuEdit()">Edit</button><button class="rtxt" id="dbtn" onclick="menuDel()">Delete</button><button onclick="closeMenu()">Cancel</button></div>
</div>
</div>
<div id="pv" onclick="if(event.target===this&&Date.now()>=ign)closePick()">
<div class="modal">
<h2 id="pvt">ADD TO STACK</h2>
<div class="btns col" id="pl"></div>
<div class="btns col"><button class="ok" onclick="pickNew()">New stack</button><button class="del" id="prm" onclick="pickRemove()">Remove from stack</button><button onclick="closePick()">Cancel</button></div>
</div>
</div>
<div id="gv" onclick="if(event.target===this&&Date.now()>=ign)closeStack()">
<div class="modal">
<h2 id="gvt">NEW STACK</h2>
<div class="f"><label>STACK NAME</label><div class="in"><input id="gnm" type="text" placeholder="e.g. Media" maxlength="24" autocomplete="off"></div></div>
<div class="f"><label>LOGO URL (OPTIONAL)</label><div class="in"><input id="gl" type="text" inputmode="url" placeholder="https://.../logo.png" autocapitalize="none" autocorrect="off" spellcheck="false" oninput="previewLogo('gl','glp')"><img id="glp" class="lp" alt="" onload="this.className='lp ok'" onerror="this.className='lp'"><button class="chip" type="button" onclick="guessLogo('gnm','gl','glp','glh')">Auto</button></div><div class="fh" id="glh"></div></div>
<div class="tg"><label>SHOW MEMBERS ON WIDGET</label><div class="sw" id="sw3" onclick="toggleExp()"><i></i></div></div>
<div class="err" id="ger"></div>
<div class="btns"><button onclick="closeStack()">Cancel</button><button class="ok" id="gok" onclick="saveStack()">Create</button></div>
<div class="btns col" id="gdw"><button class="del" id="gdel" onclick="ungroup()">Ungroup</button></div>
</div>
</div>
<script>
var C = 326.7;
var cards = {};
function col(p){ return p >= 85 ? "#ff5d73" : p >= 60 ? "#ffb454" : "#34e0a1"; }
function fmt(b){
  var u = ["B","KB","MB","GB","TB"], i = 0;
  while (b >= 1024 && i < 4) { b /= 1024; i++; }
  return (b >= 100 || i === 0 ? Math.round(b) : b.toFixed(1)) + " " + u[i] + "/s";
}
function card(id, label){
  if (cards[id]) { cards[id].l.textContent = label; return cards[id]; }
  var el = document.createElement("div");
  el.className = "card enter";
  el.style.setProperty("--i", Object.keys(cards).length);
  el.innerHTML = '<div class="ringwrap"><svg viewBox="0 0 120 120"><circle class="track" cx="60" cy="60" r="52"/><circle class="arc" cx="60" cy="60" r="52"/></svg><div class="num">0<small>%</small></div></div><div class="lbl"></div><div class="detail"></div>';
  el.addEventListener("click", function(){ toggleCard(id); });
  document.getElementById("grid").appendChild(el);
  var o = { el: el, arc: el.querySelector(".arc"), num: el.querySelector(".num"), l: el.querySelector(".lbl"), det: el.querySelector(".detail"), cur: 0, target: 0 };
  o.l.textContent = label;
  cards[id] = o;
  return o;
}
function put(id, label, p){
  var o = card(id, label);
  var c = col(p);
  o.target = p;
  o.arc.style.stroke = c;
  o.el.style.setProperty("--c", c);
  // panel closed: nothing is visible, so skip the animation (opening the panel replays it from the latest value)
  if (!monOpen) { o.cur = p; o.num.firstChild.nodeValue = Math.round(p); return; }
  o.arc.style.strokeDashoffset = C * (1 - Math.min(p, 100) / 100);
  kick();
}
var running = false;
function kick(){
  if (!running) { running = true; requestAnimationFrame(tick); }
}
function tick(){
  var busy = false;
  for (var k in cards) {
    var o = cards[k];
    o.cur += (o.target - o.cur) * 0.12;
    if (Math.abs(o.target - o.cur) < 0.05) o.cur = o.target; else busy = true;
    o.num.firstChild.nodeValue = Math.round(o.cur);
  }
  if (busy) requestAnimationFrame(tick); else running = false;
}
// LIVE / OFFLINE: whether the server answers at all (not just Glances)
var reachKnown = false;
function setStatus(ok){
  reachKnown = true;
  document.getElementById("dot").className = ok ? "dot" : "dot off";
  document.getElementById("st").textContent = ok ? "LIVE" : "OFFLINE";
}
function setReach(ok){ setStatus(!!ok); }
// Glances answering or not: shown on the Monitoring bar, separate from LIVE / OFFLINE
function glancesState(ok){
  document.getElementById("monst").textContent = ok ? "" : "NO DATA";
  // a Glances answer also means the server is up
  if (ok && !reachKnown) setStatus(true);
}
var HMAX = 100, hist = { t: [] }, last = null, dets = {}, expanded = null;
function esc(t){ var d = document.createElement("div"); d.textContent = String(t); return d.innerHTML; }
function sz(b){
  var u = ["B","KB","MB","GB","TB"], i = 0;
  while (b >= 1024 && i < 4) { b /= 1024; i++; }
  return (b >= 100 || i === 0 ? Math.round(b) : b.toFixed(1)) + " " + u[i];
}
function pc(v){ return v == null ? "-" : (Math.round(v * 10) / 10) + "%"; }
function pushHist(key, v){
  if (!hist[key]) hist[key] = [];
  hist[key].push(v);
  if (hist[key].length > HMAX) hist[key].shift();
}
function stat(v, l){ return '<div class="ds"><b>' + v + '</b><span>' + l + '</span></div>'; }
function tcol(t, k){
  var l = (st.limits || {})[k];
  if (!l) return "";
  return t >= l.hot ? "#ff5d73" : t >= l.warm ? "#ffb454" : "";
}
function tstat(v, label, k){
  var c = tcol(v, k);
  return '<div class="ds"><b' + (c ? ' style="color:' + c + '"' : '') + '>' + Math.round(v) + ' &deg;C</b><span>' + label + '</span></div>';
}
function hlabel(l, j){ var m = /(sd[a-z]+)/i.exec(l); return m ? m[1].toUpperCase() : "HDD " + (j + 1); }
function graph(key){
  var v = hist[key] || [], n = v.length, tl = hist.t;
  if (n < 2) return '<div class="gbox"><div class="gwait">Collecting history...</div></div>';
  var w = 300, h = 80, mx = 0, i;
  for (i = 0; i < n; i++) if (v[i] > mx) mx = v[i];
  var top = Math.min(100, Math.max(20, Math.ceil(mx / 10) * 10 + 10));
  var pts = "";
  for (i = 0; i < n; i++) {
    var x = (i / (n - 1)) * w, y = h - Math.min(v[i], top) / top * (h - 4) - 2;
    pts += (i ? " L" : "M") + x.toFixed(1) + "," + y.toFixed(1);
  }
  var c = col(v[n - 1]);
  var span = Math.round((tl[tl.length - 1] - tl[0]) / 1000);
  var ago = span >= 90 ? Math.round(span / 60) + " min ago" : span + " s ago";
  return '<div class="gbox"><svg viewBox="0 0 300 80" preserveAspectRatio="none"><line x1="0" y1="40" x2="300" y2="40" stroke="rgba(255,255,255,.08)" stroke-dasharray="3 4"/><path d="' + pts + ' L300,80 L0,80 Z" fill="' + c + '" fill-opacity=".16"/><path d="' + pts + '" fill="none" stroke="' + c + '" stroke-width="2" stroke-linejoin="round" vector-effect="non-scaling-stroke"/></svg><div class="gcap"><span>' + ago + '</span><span>scale 0-' + top + '%</span><span>now</span></div></div>';
}
function toplist(title, rows){
  if (!rows || !rows.length) return "";
  var mx = 0, h = '<div><div class="dh">' + title + '</div>', i;
  for (i = 0; i < rows.length; i++) if (rows[i].v > mx) mx = rows[i].v;
  for (i = 0; i < rows.length; i++) {
    var r = rows[i], wd = mx > 0 ? Math.max(3, r.v / mx * 100) : 0;
    h += '<div class="prow"><div class="pn">' + esc(r.name) + '</div><div class="pb"><i style="width:' + wd.toFixed(0) + '%;background:#34e0a1"></i></div><div class="pv">' + r.txt + '</div></div>';
  }
  return h + '</div>';
}
function renderDetail(id){
  var o = cards[id];
  if (!o || !last) return;
  var d = dets[id], h = "", i;
  var stats = [];
  if (id === "cpu") {
    if (d) {
      stats.push(stat(pc(d.user), "USER"), stat(pc(d.system), "SYSTEM"));
      if (d.iowait != null) stats.push(stat(pc(d.iowait), "IO WAIT"));
      if (d.load) stats.push(stat(d.load[0].toFixed(2), "LOAD 1M"), stat(d.load[1].toFixed(2), "LOAD 5M"), stat(d.load[2].toFixed(2), "LOAD 15M"));
      stats.push(stat(d.nCores, "CORES"));
      if (d.temp != null) stats.push(tstat(d.temp, "TEMP", "cpu"));
      if (d.hot != null && d.nCoreTemps > 1) stats.push(tstat(d.hot, "HOTTEST CORE", "cpu"));
      h += '<div class="dgrid">' + stats.join("") + '</div>';
      if (d.cores && d.cores.length) {
        h += '<div><div class="dh">PER CORE</div><div class="cores">';
        for (i = 0; i < d.cores.length; i++) h += '<div><i style="height:' + Math.min(100, d.cores[i]) + '%;background:' + col(d.cores[i]) + '"></i></div>';
        h += '</div></div>';
      }
    }
    h += '<div><div class="dh">HISTORY</div>' + graph("cpu") + '</div>';
    if (d) h += toplist("TOP CPU USERS", d.top.map(function(t){ return { name: t.name, v: t.pct, txt: t.pct.toFixed(1) + "%" }; }));
  } else if (id === "ram") {
    if (d) {
      h += '<div class="bigval">' + sz(d.used) + ' <small>/ ' + sz(d.total) + '</small></div>';
      stats.push(stat(sz(d.available), "AVAILABLE"));
      if (d.cached != null) stats.push(stat(sz(d.cached), "CACHED"));
      if (d.swapTotal) stats.push(stat(sz(d.swapUsed) + " / " + sz(d.swapTotal), "SWAP"));
      h += '<div class="dgrid">' + stats.join("") + '</div>';
    }
    h += '<div><div class="dh">HISTORY</div>' + graph("ram") + '</div>';
    if (d) h += toplist("TOP MEMORY USERS", d.top.map(function(t){ return { name: t.name, v: t.bytes, txt: sz(t.bytes) + " · " + t.pct.toFixed(1) + "%" }; }));
  } else {
    var sm = last.storage, drv = last.disks || [];
    if (sm) {
      h += '<div class="bigval">' + sz(sm.used) + ' <small>/ ' + sz(sm.size) + ' &middot; ' + sm.count + (sm.count === 1 ? ' drive' : ' drives') + '</small></div>';
      stats.push(stat(sz(sm.free), "FREE"), stat(pc(sm.percent), "USED"));
      h += '<div class="dgrid">' + stats.join("") + '</div>';
    }
    h += '<div><div class="dh">HISTORY</div>' + graph("disk") + '</div>';
    if (d && (d.nvme.length || d.hdd.length)) {
      var ts = [], j;
      for (j = 0; j < d.nvme.length; j++) ts.push(tstat(d.nvme[j].value, "NVME " + (j + 1), "nvme"));
      for (j = 0; j < d.hdd.length; j++) ts.push(tstat(d.hdd[j].value, hlabel(d.hdd[j].label, j), "hdd"));
      h += '<div><div class="dh">TEMPERATURES</div><div class="dgrid">' + ts.join("") + '</div></div>';
    } else if (d) {
      h += '<div class="hint">NO DRIVE TEMPERATURES REPORTED BY GLANCES</div>';
    }
    if (drv.length) {
      h += '<div><div class="dh">DRIVES</div>';
      for (i = 0; i < drv.length; i++) {
        var dk = drv[i], io = d && d.io ? d.io[i] : null, ds = [];
        ds.push(stat(sz(dk.free != null ? dk.free : dk.size - dk.used), "FREE"));
        ds.push(stat(esc(String(dk.device || "-").split("/").pop()), "DEVICE"));
        ds.push(stat(esc(dk.fstype || "-"), "TYPE"));
        if (io) ds.push(stat(sz(io.read) + "/s", "READ"), stat(sz(io.write) + "/s", "WRITE"));
        var tp = d && d.temps ? d.temps[i] : null;
        if (tp) ds.push(tstat(Math.max.apply(null, tp.values), tp.kind === "nvme" ? "NVME TEMP" : tp.values.length > 1 ? "HOTTEST HDD" : "HDD TEMP", tp.kind));
        h += '<div class="drv"><div class="drvh"><b>' + esc(dk.name) + '</b><span>' + sz(dk.used) + ' / ' + sz(dk.size) + ' &middot; ' + pc(dk.percent) + '</span></div>';
        h += '<div class="pb"><i style="width:' + Math.min(100, dk.percent) + '%;background:' + col(dk.percent) + '"></i></div>';
        h += '<div class="dgrid">' + ds.join("") + '</div></div>';
      }
      h += '</div>';
    }
  }
  o.det.innerHTML = h + '<div class="hint">TAP TO CLOSE</div>';
}
function detail(key, d){
  dets[key] = d;
  if (expanded === key) renderDetail(key);
}
var monOpen = false;
function replay(){
  for (var k in cards) {
    var o = cards[k];
    o.cur = 0;
    o.arc.style.transition = "none";
    o.arc.style.strokeDashoffset = C;
    void o.arc.getBoundingClientRect();
    o.arc.style.transition = "";
    o.arc.style.strokeDashoffset = C * (1 - Math.min(o.target, 100) / 100);
  }
  kick();
}
function toggleMon(){
  if (monOpen && UI.monFixed) return; // can't be folded away while "always open" is on
  monOpen = !monOpen;
  document.getElementById("mon").className = monOpen ? "mon open" : "mon";
  document.getElementById("monb").className = monOpen ? "monbtn open" : "monbtn";
  if (monOpen) { replay(); restagger(document.querySelectorAll("#grid .card, .netcard")); }
  // an enlarged card's details are only fetched while the panel is open
  if (expanded) location.href = "exp://go?k=" + (monOpen ? expanded : "");
}
function openRename(){
  var i = document.getElementById("rn");
  i.value = document.getElementById("title").textContent;
  document.getElementById("rer").textContent = "";
  document.getElementById("rv").className = "show";
  setTimeout(function(){ i.focus(); i.select(); }, 80);
}
function closeRename(){
  document.getElementById("rv").className = "";
  document.activeElement.blur();
}
function saveRename(){
  if (Date.now() < ign) return;
  var v = document.getElementById("rn").value.trim();
  if (!v) { document.getElementById("rer").textContent = "Enter a name"; return; }
  document.getElementById("title").textContent = v;
  closeRename();
  location.href = "ttl://go?t=" + encodeURIComponent(v);
}
(function(){
  var t = null, fired = false, el = document.getElementById("title");
  el.addEventListener("touchstart", function(){
    fired = false;
    clearTimeout(t);
    t = setTimeout(function(){ t = null; fired = true; openRename(); }, 600);
  }, { passive: true });
  ["touchmove", "touchcancel"].forEach(function(n){
    el.addEventListener(n, function(){ clearTimeout(t); }, { passive: true });
  });
  el.addEventListener("touchend", function(){
    clearTimeout(t);
    if (fired) ign = Date.now() + 450;
  });
  el.addEventListener("contextmenu", function(e){ e.preventDefault(); });
})();
function toggleCard(id){
  var els = [];
  for (var k in cards) els.push(cards[k].el);
  els.push(document.querySelector(".netcard"), document.querySelector(".sechead"), document.getElementById("svc"), document.getElementById("addwrap"));
  flipEls(els, function(){ toggleCardRaw(id); }, { clip: true });
  if (isGlass() && expanded && cards[expanded]) {
    var dt = cards[expanded].det;
    dt.classList.add("fresh");
    clearTimeout(dt._ft);
    dt._ft = setTimeout(function(){ dt.classList.remove("fresh"); }, 1100);
  }
}
function toggleCardRaw(id){
  var o = cards[id];
  if (!o) return;
  if (expanded === id) {
    expanded = null;
    o.el.classList.remove("big");
    o.det.innerHTML = "";
    location.href = "exp://go?k=";
    return;
  }
  if (expanded && cards[expanded]) {
    cards[expanded].el.classList.remove("big");
    cards[expanded].det.innerHTML = "";
  }
  expanded = id;
  o.el.classList.add("big");
  renderDetail(id);
  setTimeout(function(){ o.el.scrollIntoView({ behavior: "smooth", block: "start" }); }, 60);
  location.href = "exp://go?k=" + id;
}
function update(s){
  last = s;
  hist.t.push(Date.now());
  if (hist.t.length > HMAX) hist.t.shift();
  pushHist("cpu", s.cpu);
  pushHist("ram", s.ram);
  if (s.storage) pushHist("disk", s.storage.percent);
  glancesState(true);
  put("cpu", "CPU", s.cpu);
  put("ram", "RAM", s.ram);
  if (s.storage) put("disk", "STORAGE", s.storage.percent);
  document.getElementById("down").textContent = fmt(s.net.down);
  document.getElementById("up").textContent = fmt(s.net.up);
  if (expanded) renderDetail(expanded);
}
// setup: no server address yet; otherwise Glances didn't answer
function offline(setup){
  if (setup) {
    document.getElementById("dot").className = "dot wait";
    document.getElementById("st").textContent = "SETUP";
    return;
  }
  glancesState(false);
}
var ign = 0, editId = null, cur = null, delArmed = false;
var pTimer = null, sx = 0, sy = 0, lastX = 0, lastY = 0, pEl = null, pSv = null;
var pDrag = true, dragging = false, moved = false, fired = false, ghost = null, ox = 0, oy = 0, edge = null, startOrder = "";
function idsOf(el){
  var r = [], k = el.children;
  for (var i = 0; i < k.length; i++) r.push(k[i].dataset.id);
  return r;
}
function ids(b){ return idsOf(document.getElementById(b)); }
function order(){ return ids("svc"); }
// every stack's buttons, by stack id
function bodyMap(){
  var m = {}, k = document.querySelectorAll(".gbody");
  for (var i = 0; i < k.length; i++) m[k[i].dataset.gid] = idsOf(k[i]);
  return m;
}
function sig(){ return JSON.stringify([ids("svc"), ids("hid"), bodyMap()]); }
function commitWidget(){
  var all = st.widget.concat(st.hidden), byId = {}, i;
  for (i = 0; i < all.length; i++) byId[all[i].id] = all[i];
  var sh = ids("svc"), hi = ids("hid"), ns = [], nh = [];
  for (i = 0; i < sh.length; i++) if (byId[sh[i]]) { byId[sh[i]].onWidget = true; ns.push(byId[sh[i]]); }
  for (i = 0; i < hi.length; i++) if (byId[hi[i]]) { byId[hi[i]].onWidget = false; nh.push(byId[hi[i]]); }
  st.widget = ns;
  st.hidden = nh;
  for (i = 0; i < st.dash.length; i++) if (byId[st.dash[i].id]) st.dash[i].onWidget = byId[st.dash[i].id].onWidget;
}
// moveEl: the element that moves when dragged (a whole stack card when you hold its header)
var pH = null;
function bindPress(a, sv, drag, moveEl){
  a.addEventListener("touchstart", function(e){
    var t = e.touches[0];
    sx = lastX = t.clientX; sy = lastY = t.clientY;
    pEl = moveEl || a; pH = a; pSv = sv; pDrag = drag; fired = false; dragging = false; moved = false;
    clearTimeout(pTimer);
    pTimer = setTimeout(startDrag, 500);
  }, { passive: true });
  a.addEventListener("contextmenu", function(e){ e.preventDefault(); });
  a.addEventListener("click", function(e){
    if (fired) { e.preventDefault(); fired = false; }
  });
}
function startDrag(){
  pTimer = null;
  clearPress();
  fired = true;
  if (!pDrag) { ign = Date.now() + 450; showMenu(pSv); return; }
  dragging = true;
  moved = false;
  startOrder = sig();
  var r = pH.getBoundingClientRect(), rw = pH.offsetWidth, rh = pH.offsetHeight;
  // measured while squished by the press (glass look): use the real size, centred on the same spot
  r = { left: r.left + (r.width - rw) / 2, top: r.top + (r.height - rh) / 2, width: rw, height: rh };
  ox = sx - r.left; oy = sy - r.top;
  ghost = pH.cloneNode(true);
  ghost.className = pH === pEl ? "svc ghost" : "monbtn grph ghost";
  ghost.style.width = r.width + "px";
  ghost.style.height = r.height + "px";
  ghost.style.left = r.left + "px";
  ghost.style.top = r.top + "px";
  document.body.appendChild(ghost);
  pEl.classList.add("lifted");
  edge = setInterval(function(){
    var se = document.scrollingElement || document.body;
    if (lastY > window.innerHeight - 90) se.scrollTop += 10;
    else if (lastY < 90) se.scrollTop -= 10;
    reorder(lastX, lastY);
  }, 16);
}
function reorder(x, y){
  var isGrp = pEl.classList.contains("grp");
  var boxes = mode === "widget" ? [document.getElementById("svc"), document.getElementById("hid")] : [document.getElementById("svc")];
  // a button can also be dropped into any open stack (a stack can't go inside another stack)
  if (mode !== "widget" && !isGrp) boxes = openBodies().concat(boxes);
  var bi, i, j, c, r, k;
  for (bi = 0; bi < boxes.length; bi++) {
    k = boxes[bi].children;
    for (i = 0; i < k.length; i++) {
      c = k[i];
      if (c === pEl) continue;
      // over an open stack's button area: handled by that stack's own box
      if (!isGrp && c.classList.contains("grp")) {
        var gb = c.querySelector(".mon.open .gbody");
        if (gb) {
          var br = gb.getBoundingClientRect(), off = rectOf(c).top - c.getBoundingClientRect().top;
          if (x >= br.left && x <= br.right && y >= br.top + off - 8 && y <= br.bottom + off + 8) continue;
        }
      }
      r = rectOf(c);
      if (x >= r.left && x <= r.right && y >= r.top && y <= r.bottom) {
        var par = c.parentNode, before = false;
        if (pEl.parentNode === par) {
          for (j = 0; j < par.children.length; j++) {
            if (par.children[j] === c) break;
            if (par.children[j] === pEl) { before = true; break; }
          }
        }
        // before === true: dragged item is currently ahead of c in the same panel, so move it after c
        moveFlip(function(){
          if (before) par.insertBefore(pEl, c.nextSibling);
          else par.insertBefore(pEl, c);
        });
        markOverflow();
        return;
      }
    }
  }
  // empty space at the end of a panel (or an empty panel): drop at the end
  for (bi = 0; bi < boxes.length; bi++) {
    var box = boxes[bi];
    r = box.getBoundingClientRect();
    if (x < r.left || x > r.right || y < r.top - 8 || y > r.bottom + 8) continue;
    k = box.children;
    var lastC = null;
    for (i = 0; i < k.length; i++) if (k[i] !== pEl) lastC = k[i];
    if (!lastC) {
      if (pEl.parentNode !== box) { moveFlip(function(){ box.appendChild(pEl); }); markOverflow(); }
      return;
    }
    if (y > rectOf(lastC).bottom && box.lastElementChild !== pEl) {
      moveFlip(function(){ box.appendChild(pEl); });
      markOverflow();
      return;
    }
  }
}
document.addEventListener("touchmove", function(e){
  var t = e.touches[0];
  lastX = t.clientX; lastY = t.clientY;
  if (dragging) {
    e.preventDefault();
    if (Math.abs(lastX - sx) > 10 || Math.abs(lastY - sy) > 10) moved = true;
    ghost.style.left = (lastX - ox) + "px";
    ghost.style.top = (lastY - oy) + "px";
    reorder(lastX, lastY);
    return;
  }
  if (pTimer && (Math.abs(lastX - sx) > 8 || Math.abs(lastY - sy) > 8)) {
    clearTimeout(pTimer);
    pTimer = null;
  }
}, { passive: false });
function endTouch(){
  clearTimeout(pTimer);
  pTimer = null;
  if (!dragging) return;
  dragging = false;
  clearInterval(edge);
  ign = Date.now() + 450;
  if (ghost) { ghost.remove(); ghost = null; }
  pEl.classList.remove("lifted");
  var changed = false;
  if (!moved) {
    showMenu(pSv);
  } else {
    if (sig() !== startOrder) {
      changed = true;
      if (mode === "widget") {
        var payload = JSON.stringify({ s: ids("svc"), h: ids("hid") });
        commitWidget();
        location.href = "wrd://go?o=" + encodeURIComponent(payload);
      } else {
        location.href = "ord://go?o=" + encodeURIComponent(JSON.stringify({ o: order(), g: bodyMap() }));
      }
    }
  }
  // an update that arrived mid-drag is drawn now (after a change, a fresh one is on its way)
  if (pendingDraw && !changed) draw();
  pendingDraw = false;
}
function openBodies(){
  var r = [], k = document.querySelectorAll(".grp .mon.open .gbody");
  for (var i = 0; i < k.length; i++) r.push(k[i]);
  return r;
}
document.addEventListener("touchend", endTouch);
document.addEventListener("touchcancel", endTouch);
var st = { dash: [], widget: [], hidden: [] }, mode = "dash";
var svcUp = {}, gmap = {}, openG = {}, pendingDraw = false;
// stack status: up = all up, dn = all down, part = some down
function gstate(g){
  var up = 0, dn = 0, n = g.members.length;
  for (var i = 0; i < n; i++) { var v = svcUp[g.members[i].id]; if (v === true) up++; else if (v === false) dn++; }
  if (!n || up + dn === 0) return "";
  if (up === n) return " up";
  if (dn === n) return " dn";
  return dn ? " part" : "";
}
function applyDots(){
  var k = document.querySelectorAll(".svc[data-id],.grph[data-gid]");
  for (var i = 0; i < k.length; i++) {
    var d = k[i].querySelector(".sd");
    if (!d) continue;
    var id = k[i].dataset.gid || k[i].dataset.id;
    if (gmap[id]) { d.className = "sd" + gstate(gmap[id]); continue; }
    var v = svcUp[id];
    d.className = "sd" + (v === true ? " up" : v === false ? " dn" : "");
  }
}
function iconHtml(sv){
  var lg = st.logos && st.logos[sv.id];
  return lg ? '<img class="ic" alt="" src="data:image/png;base64,' + lg + '">' : '<span class="ic">' + sv.icon + '</span>';
}
var CHEV = '<svg viewBox="0 0 24 24"><path d="M6 9l6 6 6-6" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"/></svg>';
// Stack card in the dashboard: header (tap = open/close, hold = drag or menu) + its buttons
function makeGroup(g){
  var w = document.createElement("div");
  w.className = "grp";
  w.dataset.id = g.id;
  var open = !!openG[g.id];
  var h = document.createElement("a");
  h.className = open ? "monbtn grph open" : "monbtn grph";
  h.dataset.gid = g.id;
  h.innerHTML = '<span class="gt">' + iconHtml(g) + '<b></b><span class="gc"></span></span><span class="gr"><span class="sd"></span>' + CHEV + '</span>';
  h.querySelector("b").textContent = g.name;
  h.querySelector(".gc").textContent = g.members.length;
  var m = document.createElement("div");
  m.className = open ? "mon open" : "mon";
  m.innerHTML = '<div class="moninner"><div class="gbody"></div></div>';
  var body = m.querySelector(".gbody");
  body.dataset.gid = g.id;
  g.members.forEach(function(sv){ body.appendChild(makeBtn(sv, true)); });
  w.appendChild(h);
  w.appendChild(m);
  bindPress(h, g, true, w);
  h.addEventListener("click", function(e){ if (!e.defaultPrevented) toggleGroup(g.id); });
  return w;
}
// Stack in "Widget layout": one pill (counts as all its buttons when "show members" is on)
function makeGroupPill(g){
  var a = document.createElement("a");
  a.className = "svc";
  a.dataset.id = g.id;
  var n = g.members.length;
  a.dataset.slots = g.expand ? n : (n ? 1 : 0);
  a.innerHTML = iconHtml(g) + '<span class="nm"></span><span class="pt"></span><span class="sd"></span>';
  a.querySelector(".nm").textContent = g.name + (g.expand ? " \u00d7" + n : "");
  bindPress(a, g, true);
  return a;
}
function toggleGroup(id){
  openG[id] = !openG[id];
  var w = document.querySelector('.grp[data-id="' + id + '"]');
  if (!w) return;
  w.querySelector(".grph").className = openG[id] ? "monbtn grph open" : "monbtn grph";
  w.querySelector(".mon").className = openG[id] ? "mon open" : "mon";
  if (openG[id]) restagger(w.querySelectorAll(".gbody > .svc"));
}
function focusStack(id){
  if (!gmap[id]) return;
  if (mode !== "dash") { mode = "dash"; draw(); }
  if (!openG[id]) toggleGroup(id);
  var w = document.querySelector('.grp[data-id="' + id + '"]');
  if (w) setTimeout(function(){ w.scrollIntoView({ behavior: "smooth", block: "start" }); }, 150);
}
function setUp(m){ svcUp = m; applyDots(); }
function makeBtn(sv, drag){
  var a = document.createElement("a");
  a.className = "svc";
  a.dataset.id = sv.id;
  a.href = "open://go?u=" + encodeURIComponent(sv.url);
  a.innerHTML = iconHtml(sv) + '<span class="nm"></span><span class="pt"></span><span class="sd"></span>';
  a.querySelector(".nm").textContent = sv.name;
  a.querySelector(".pt").textContent = sv.label || "";
  bindPress(a, sv, drag);
  return a;
}
function markOverflow(){
  var k = document.getElementById("svc").children, h = document.getElementById("hid").children, i;
  var used = 0;
  for (i = 0; i < k.length; i++) {
    var sl = k[i].dataset.slots != null ? Number(k[i].dataset.slots) : 1;
    if (mode === "widget" && used >= UI.widgetMax && sl > 0) k[i].classList.add("over");
    else k[i].classList.remove("over");
    used += sl;
  }
  for (i = 0; i < h.length; i++) h[i].classList.remove("over");
}
function draw(){
  var wide = mode === "widget";
  var box = document.getElementById("svc"), hb = document.getElementById("hid");
  box.className = wide ? "wide" : "";
  document.body.classList.toggle("wide", wide);
  box.innerHTML = "";
  hb.innerHTML = "";
  (wide ? st.widget : st.dash).forEach(function(sv){
    box.appendChild(sv.kind === "group" ? (wide ? makeGroupPill(sv) : makeGroup(sv)) : makeBtn(sv, true));
  });
  if (wide) st.hidden.forEach(function(sv){ hb.appendChild(sv.kind === "group" ? makeGroupPill(sv) : makeBtn(sv, true)); });
  document.getElementById("sect").textContent = wide ? "WIDGET LAYOUT" : "SERVICES";
  var lb = document.getElementById("layb");
  lb.textContent = wide ? "Done" : "Widget layout";
  lb.className = wide ? "lay on" : "lay";
  document.getElementById("wextra").style.display = wide ? "flex" : "none";
  document.getElementById("hidwrap").style.display = wide ? "" : "none";
  document.getElementById("addwrap").style.display = wide ? "none" : "";
  markOverflow();
  applyDots();
  if (animDraw) {
    animDraw = false;
    if (isGlass()) {
      var all = [].slice.call(box.children).concat([].slice.call(hb.children));
      all.forEach(function(e, i){ e.style.setProperty("--i", Math.min(i, 18)); e.classList.add("enter"); });
    }
  }
}
var animDraw = false;
// ---- Servers: the dots under the name, swiping the name, adding a server ----
function idxOf(id){
  var l = st.servers || [];
  for (var i = 0; i < l.length; i++) if (l[i].id === id) return i;
  return -1;
}
function renderDots(){
  var box = document.getElementById("sdots");
  if (!box) return;
  box.innerHTML = "";
  (st.servers || []).forEach(function(sv){
    var i = document.createElement("i");
    if (sv.id === st.cur) i.className = "on";
    i.setAttribute("aria-label", sv.title);
    i.onclick = function(){ if (sv.id !== st.cur) goServer(sv.id, idxOf(sv.id) > idxOf(st.cur) ? 1 : -1); };
    box.appendChild(i);
  });
  var plus = document.createElement("b");
  plus.textContent = "+";
  plus.setAttribute("aria-label", "Add server");
  plus.onclick = openAddServer;
  box.appendChild(plus);
}
function goServer(id, dir){
  togglePmenu(false);
  location.href = "srv://go?d=" + encodeURIComponent(JSON.stringify({ act: "switch", id: id, dir: dir }));
}
// swipe the name: left = next server (past the last one: add a server), right = previous
(function(){
  var tw = document.getElementById("tw"), x0 = 0, y0 = 0, t0 = 0;
  tw.addEventListener("touchstart", function(e){ var t = e.touches[0]; x0 = t.clientX; y0 = t.clientY; t0 = Date.now(); }, { passive: true });
  tw.addEventListener("touchend", function(e){
    var t = e.changedTouches[0], dx = t.clientX - x0, dy = t.clientY - y0;
    if (Math.abs(dx) < 45 || Math.abs(dx) < Math.abs(dy) * 1.5 || Date.now() - t0 > 800) return;
    ign = Date.now() + 450; // a swipe isn't a tap (Reboot menu) or a hold (rename)
    var l = st.servers || [], i = idxOf(st.cur);
    if (dx < 0) { if (i >= 0 && i < l.length - 1) goServer(l[i + 1].id, 1); else openAddServer(); }
    else if (i > 0) goServer(l[i - 1].id, -1);
  }, { passive: true });
})();
// another server is on screen: drop what was shown for the last one and draw this one
function switchServer(t, p, dir){
  hist = { t: [] };
  dets = {};
  last = null;
  if (expanded && cards[expanded]) { cards[expanded].el.classList.remove("big"); cards[expanded].det.innerHTML = ""; }
  expanded = null;
  document.getElementById("grid").innerHTML = "";
  cards = {};
  svcUp = {};
  openG = {};
  mode = "dash";
  reachKnown = false;
  document.getElementById("dot").className = "dot wait";
  document.getElementById("st").textContent = "...";
  document.getElementById("monst").textContent = "";
  document.getElementById("down").textContent = fmt(0);
  document.getElementById("up").textContent = fmt(0);
  togglePmenu(false);
  var h = document.getElementById("title");
  h.textContent = t;
  h.classList.remove("tslide");
  void h.offsetWidth;
  h.style.setProperty("--sx", (dir > 0 ? 44 : dir < 0 ? -44 : 0) + "px");
  h.classList.add("tslide");
  animDraw = true;
  renderServices(p);
  // the Monitoring panel follows this server's settings
  if (monOpen && (UI.monitoring === false || (!UI.monOpen && !UI.monFixed))) {
    monOpen = false;
    document.getElementById("mon").className = "mon";
    document.getElementById("monb").className = "monbtn";
  } else if (!monOpen && UI.monitoring !== false && (UI.monOpen || UI.monFixed)) {
    toggleMon();
  }
}
// Add server: the same questions in the same order as its Settings (name, type, address, login)
var AS = null;
// Delete server: a confirmation popup over the Settings sheet
var delId = null;
function openDelServer(name){
  delId = st.cur;
  document.getElementById("dvt").textContent = "DELETE " + String(name || "SERVER").toUpperCase() + "?";
  document.getElementById("dvn").textContent = "Its settings, saved logins, buttons, stacks and layout are deleted from this phone. Your other servers stay. This can't be undone (unless you exported your settings).";
  ign = Date.now() + 350; // the tap that opened it doesn't press Delete
  document.getElementById("dv").className = "show";
}
function closeDelServer(){
  document.getElementById("dv").className = "";
}
function confirmDelServer(){
  if (Date.now() < ign || !delId) return;
  var id = delId;
  delId = null;
  closeDelServer();
  S.saved = true;
  closeSettings();
  location.href = "srv://go?d=" + encodeURIComponent(JSON.stringify({ act: "remove", id: id, dir: -1 }));
}
function openAddServer(){
  togglePmenu(false);
  AS = { name: "", platform: "none", host: "", scheme: "", port: "", u: "", p: "", err: "", busy: false };
  renderAddServer();
  document.getElementById("av").className = "show";
  setTimeout(function(){ var n = document.getElementById("asn"); if (n) n.focus(); }, 80);
}
function renderAddServer(){
  var m = document.getElementById("avm"), PL = st.platforms || {};
  m.innerHTML = "";
  var h = document.createElement("h2");
  h.textContent = "ADD SERVER";
  m.appendChild(h);
  var note = document.createElement("div");
  note.className = "cnote";
  note.textContent = "Each server has its own address, type, logins, Monitoring, buttons and layout. Swipe the server name to switch between them.";
  m.appendChild(note);
  var field = function(label, val, ph, type, set, id){
    var f = document.createElement("div");
    f.className = "f";
    var l = document.createElement("label");
    l.textContent = label;
    f.appendChild(l);
    var x = sInputEl(val, ph, null, type);
    if (id) x.i.id = id;
    x.i.oninput = function(){ set(x.i.value); };
    f.appendChild(x.w);
    m.appendChild(f);
    return x.i;
  };
  var hint = function(t){ var d = document.createElement("div"); d.className = "fh"; d.textContent = t; m.appendChild(d); };
  field("NAME", AS.name, "e.g. NAS", "text", function(v){ AS.name = v; }, "asn").maxLength = 20;
  var tl = document.createElement("div");
  tl.className = "f";
  tl.innerHTML = "<label>SERVER TYPE</label>";
  tl.appendChild(radioRow(typeOpts(PL), AS.platform, function(v){ AS.platform = v; AS.scheme = ""; AS.err = ""; renderAddServer(); }));
  m.appendChild(tl);
  hint(TYPE_HINT[AS.platform] || TYPE_HINT.none);
  serverAddrField(m, "SERVER ADDRESS", { PL: PL, platform: AS.platform, host: AS.host, scheme: AS.scheme, port: AS.port,
    onChange: function(a){ AS.host = a.host; AS.scheme = a.scheme; AS.port = a.port; } });
  var pl = PL[AS.platform];
  if (pl && pl.login) {
    field(pl.login.toUpperCase() + " USERNAME", AS.u, pl.userPh || "", "text", function(v){ AS.u = v; });
    field(pl.login.toUpperCase() + " PASSWORD", AS.p, "", "password", function(v){ AS.p = v; });
    hint("For Reboot / Shut down. Checked against the server before it's added, then stored in the iOS Keychain. Leave empty to be asked the first time.");
  } else if (AS.platform === "linux") {
    hint("After adding, its Settings open so you can set up the helper (one command over SSH).");
  }
  var er = document.createElement("div");
  er.className = "err";
  er.textContent = AS.err;
  m.appendChild(er);
  var bx = document.createElement("div");
  bx.className = "btns";
  var cb = document.createElement("button");
  cb.textContent = "Cancel";
  cb.onclick = closeAddServer;
  var ob = document.createElement("button");
  ob.className = "ok";
  ob.textContent = AS.busy ? "Checking..." : "Add";
  ob.onclick = submitAddServer;
  bx.appendChild(cb);
  bx.appendChild(ob);
  m.appendChild(bx);
}
function closeAddServer(){
  document.getElementById("av").className = "";
  if (document.activeElement) document.activeElement.blur();
  AS = null;
}
function submitAddServer(){
  if (Date.now() < ign || !AS || AS.busy) return;
  var fail = function(t){ AS.err = t; renderAddServer(); };
  if (AS.platform === "none") return fail("Pick the server type");
  if (!AS.host.trim()) return fail("Enter the server's IP or name");
  if ((AS.u.trim() && !AS.p) || (!AS.u.trim() && AS.p)) return fail("Enter both the username and the password, or neither");
  AS.err = "";
  AS.busy = true;
  renderAddServer();
  location.href = "srv://go?d=" + encodeURIComponent(JSON.stringify({ act: "add", name: AS.name.trim(), platform: AS.platform, host: AS.host.trim(), scheme: AS.scheme, port: AS.port, u: AS.u.trim(), p: AS.p, dir: 1 }));
}
// the answer to Add: added (the server is on screen now) or what went wrong (the popup stays open)
function addServerResult(r){
  if (!AS) return;
  if (r.ok) { closeAddServer(); return; }
  AS.busy = false;
  AS.err = r.error || "Couldn't add the server";
  renderAddServer();
}
function renderServices(p){
  st = p;
  applyUi(p.ui);
  if (p.title) document.getElementById("title").textContent = p.title;
  renderDots();
  gmap = {};
  (st.dash || []).forEach(function(x){ if (x.kind === "group") gmap[x.id] = x; });
  if (dragging) { pendingDraw = true; return; }
  draw();
}
function toggleMode(){
  mode = mode === "dash" ? "widget" : "dash";
  animDraw = true;
  draw();
}
function init(t, p){
  document.getElementById("title").textContent = t;
  animDraw = true;
  renderServices(p);
  if (UI.monOpen && !monOpen && UI.monitoring !== false) toggleMon();
  // startup screen only when it's turned on in Settings
  if (UI.splash) boot(); else skipSplash(true);
}
function showMenu(sv){
  if (sv.kind === "group") { openStack(sv); return; }
  cur = sv;
  document.getElementById("sbtn").textContent = sv.group ? "Change stack" : "Add to stack";
  delArmed = false;
  document.getElementById("mt").textContent = sv.name;
  document.getElementById("dbtn").textContent = "Delete";
  document.getElementById("mv").className = "show";
}
function closeMenu(){ document.getElementById("mv").className = ""; }
function menuEdit(){
  if (Date.now() < ign) return;
  var sv = cur;
  closeMenu();
  openAdd(sv);
}
function menuDel(){
  if (Date.now() < ign) return;
  if (!delArmed) {
    delArmed = true;
    document.getElementById("dbtn").textContent = "Tap again to delete";
    return;
  }
  closeMenu();
  location.href = "del://go?id=" + encodeURIComponent(cur.id);
}
var usePort = true;
function applyPort(){
  document.getElementById("sw").className = usePort ? "sw on" : "sw";
  document.getElementById("pf").style.display = usePort ? "" : "none";
  document.getElementById("fa").placeholder = usePort ? (st.host || "192.168.1.10") : "app.example.com";
}
var onW = true;
function applyW(){
  document.getElementById("sw2").className = onW ? "sw on" : "sw";
}
function toggleWidget(){
  onW = !onW;
  applyW();
}
function togglePort(){
  usePort = !usePort;
  applyPort();
}
function openAdd(sv){
  editId = sv ? sv.id : null;
  usePort = sv ? !!sv.port : true;
  applyPort();
  onW = sv ? sv.onWidget !== false : true;
  applyW();
  document.getElementById("at").textContent = sv ? "EDIT SERVICE" : "ADD SERVICE";
  document.getElementById("okb").textContent = sv ? "Save" : "Add";
  document.getElementById("fn").value = sv ? sv.name : "";
  document.getElementById("fs").value = sv && sv.scheme === "https" ? "https" : "http";
  document.getElementById("fa").value = sv ? sv.addr : "";
  document.getElementById("fp").value = sv && sv.port ? sv.port : "";
  document.getElementById("fl").value = sv && sv.logo ? sv.logo : "";
  document.getElementById("flh").textContent = "Empty = emoji. Auto guesses from the name.";
  previewLogo("fl", "flp");
  document.getElementById("wtg").style.display = sv && sv.group ? "none" : "";
  document.getElementById("er").textContent = "";
  document.getElementById("ov").className = "show";
  if (!sv) setTimeout(function(){ document.getElementById("fn").focus(); }, 80);
}
function closeAdd(){
  document.getElementById("ov").className = "";
  document.activeElement.blur();
}
function submitAdd(){
  if (Date.now() < ign) return;
  var n = document.getElementById("fn").value.trim();
  var a = document.getElementById("fa").value.trim();
  var sc = document.getElementById("fs").value;
  var p = usePort ? document.getElementById("fp").value.replace(/[^0-9]/g, "") : "";
  var lu = document.getElementById("fl").value.trim();
  if (!n || !a) { document.getElementById("er").textContent = "Enter a name and an address"; return; }
  if (lu && !/^https?:\\/\\//i.test(lu)) { document.getElementById("er").textContent = "Logo URL must start with http:// or https://"; return; }
  closeAdd();
  location.href = "add://go?d=" + encodeURIComponent(JSON.stringify({ id: editId, name: n, scheme: sc, addr: a, port: p, onWidget: onW, logo: lu }));
}
var lpT = {};
function previewLogo(uid, iid){
  clearTimeout(lpT[uid]);
  lpT[uid] = setTimeout(function(){
    var v = document.getElementById(uid).value.trim(), im = document.getElementById(iid);
    if (/^https?:\\/\\//i.test(v)) { if (im.getAttribute("src") !== v) { im.className = "lp"; im.src = v; } }
    else { im.className = "lp"; im.removeAttribute("src"); }
  }, 350);
}
// Guess a dashboard-icons logo from the name (e.g. "Uptime Kuma" -> uptime-kuma.png)
function guessLogo(nid, uid, iid, hid){
  var n = document.getElementById(nid).value.trim().toLowerCase();
  var h = document.getElementById(hid);
  if (!n) { h.textContent = "Type a name first."; return; }
  var slug = n.replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "");
  var im = document.getElementById(iid);
  var url = (st.cdn || "") + slug + ".png";
  h.textContent = "Checking " + slug + ".png ...";
  var t = new Image();
  t.onload = function(){ document.getElementById(uid).value = url; im.className = "lp"; im.src = url; h.textContent = "Found " + slug + ".png"; };
  t.onerror = function(){ h.textContent = "No logo named " + slug + ".png. Paste a URL instead."; };
  t.src = url;
}
// ---- Stacks: pick a stack for a button ----
var pickSv = null;
function menuStack(){
  if (Date.now() < ign) return;
  var sv = cur;
  closeMenu();
  openPick(sv);
}
function openPick(sv){
  pickSv = sv;
  var list = document.getElementById("pl");
  list.innerHTML = "";
  for (var id in gmap) {
    if (id === sv.group) continue;
    (function(g){
      var b = document.createElement("button");
      b.textContent = g.name;
      b.onclick = function(){ pickTo(g.id); };
      list.appendChild(b);
    })(gmap[id]);
  }
  document.getElementById("pvt").textContent = sv.group ? "MOVE TO STACK" : "ADD TO STACK";
  document.getElementById("prm").style.display = sv.group ? "" : "none";
  document.getElementById("pv").className = "show";
}
function closePick(){ document.getElementById("pv").className = ""; }
function pickTo(gid){
  if (Date.now() < ign) return;
  closePick();
  location.href = "grp://go?d=" + encodeURIComponent(JSON.stringify({ act: "move", id: pickSv.id, to: gid }));
}
function pickRemove(){
  if (Date.now() < ign) return;
  closePick();
  location.href = "grp://go?d=" + encodeURIComponent(JSON.stringify({ act: "move", id: pickSv.id, to: null }));
}
function pickNew(){
  if (Date.now() < ign) return;
  closePick();
  openStack(null, pickSv);
}
// ---- Stacks: create / edit / ungroup ----
var gEdit = null, gMember = null, gExp = false, gDelArmed = false;
function applyExp(){ document.getElementById("sw3").className = gExp ? "sw on" : "sw"; }
function toggleExp(){ gExp = !gExp; applyExp(); }
function openStack(g, member){
  gEdit = g ? g.id : null;
  gMember = member ? member.id : null;
  gExp = g ? !!g.expand : false;
  applyExp();
  gDelArmed = false;
  document.getElementById("gvt").textContent = g ? "EDIT STACK" : "NEW STACK";
  document.getElementById("gok").textContent = g ? "Save" : "Create";
  document.getElementById("gnm").value = g ? g.name : "";
  document.getElementById("gl").value = g && g.logo ? g.logo : "";
  document.getElementById("glh").textContent = "Empty = \ud83d\uddc2\ufe0f. Auto guesses from the name.";
  previewLogo("gl", "glp");
  document.getElementById("gdw").style.display = g ? "" : "none";
  document.getElementById("gdel").textContent = "Ungroup";
  document.getElementById("ger").textContent = "";
  document.getElementById("gv").className = "show";
  if (!g) setTimeout(function(){ document.getElementById("gnm").focus(); }, 80);
}
function closeStack(){
  document.getElementById("gv").className = "";
  document.activeElement.blur();
}
function saveStack(){
  if (Date.now() < ign) return;
  var n = document.getElementById("gnm").value.trim();
  var lu = document.getElementById("gl").value.trim();
  if (!n) { document.getElementById("ger").textContent = "Enter a name"; return; }
  if (lu && !/^https?:\\/\\//i.test(lu)) { document.getElementById("ger").textContent = "Logo URL must start with http:// or https://"; return; }
  closeStack();
  location.href = "grp://go?d=" + encodeURIComponent(JSON.stringify({ act: "save", gid: gEdit, member: gMember, name: n, logo: lu, expand: gExp }));
}
function ungroup(){
  if (Date.now() < ign) return;
  if (!gDelArmed) {
    gDelArmed = true;
    document.getElementById("gdel").textContent = "Tap again to ungroup";
    return;
  }
  closeStack();
  location.href = "grp://go?d=" + encodeURIComponent(JSON.stringify({ act: "del", gid: gEdit }));
}
// ---- Reboot / shut down (tap the server name) ----
var pAct = "restart", tT = null;
function togglePmenu(force){
  var t = document.getElementById("tw");
  var open = force != null ? force : !t.classList.contains("open");
  if (open) t.classList.add("open"); else t.classList.remove("open");
}
document.getElementById("tw").addEventListener("click", function(e){
  if (Date.now() < ign) return; // just renamed by holding the name
  if (!UI.power) return; // Reboot / Shut down turned off in Settings
  if (e.target.closest && e.target.closest(".pmenu")) return;
  togglePmenu();
});
document.addEventListener("touchstart", function(e){
  var t = document.getElementById("tw");
  if (t.classList.contains("open") && !t.contains(e.target)) togglePmenu(false);
}, { passive: true });
function openPower(a, err, needLogin){
  togglePmenu(false);
  pAct = a;
  var re = a === "restart";
  document.getElementById("cvt").textContent = re ? "REBOOT SERVER?" : "SHUT DOWN SERVER?";
  document.getElementById("cvn").textContent = re
    ? "Every service goes offline until the server is back up, usually a minute or two."
    : "Every service goes offline. You'll have to switch the server back on by hand.";
  document.getElementById("cok").textContent = re ? "Reboot" : "Shut down";
  // Linux uses the helper's key from Settings, so it never asks for a login here
  var showL = !!UI.login && (needLogin || !st.hasLogin);
  if (UI.login) {
    document.getElementById("cul").textContent = UI.login.toUpperCase() + " USERNAME";
    document.getElementById("cpl").textContent = UI.login.toUpperCase() + " PASSWORD";
    document.getElementById("cu").placeholder = UI.userPh || "";
  }
  document.getElementById("cvl").style.display = showL ? "flex" : "none";
  document.getElementById("cp").value = "";
  document.getElementById("cer").textContent = err || "";
  document.getElementById("cv").className = "show";
}
function closePower(){
  document.getElementById("cv").className = "";
  document.activeElement.blur();
}
function confirmPower(){
  if (Date.now() < ign) return;
  var u = "", p = "";
  if (document.getElementById("cvl").style.display !== "none") {
    u = document.getElementById("cu").value.trim();
    p = document.getElementById("cp").value;
    if (!u || !p) { document.getElementById("cer").textContent = "Enter your " + UI.login + " username and password"; return; }
  }
  closePower();
  toast(pAct === "restart" ? "Sending reboot..." : "Sending shutdown...", 15000);
  location.href = "pwr://go?d=" + encodeURIComponent(JSON.stringify({ a: pAct, u: u, p: p }));
}
function powerResult(r){
  st.hasLogin = r.hasLogin;
  if (r.auth) { toast("", 1); openPower(r.a, r.msg, true); return; }
  toast(r.msg, 5000);
}
function toast(m, ms){
  var t = document.getElementById("toast");
  t.textContent = m;
  t.className = m ? "toast show" : "toast";
  clearTimeout(tT);
  tT = setTimeout(function(){ t.className = "toast"; }, ms || 2500);
}
// ---- Display settings that apply right away (sent with every button update) ----
var UI = { cols: 2, power: true, platform: "zimaos", login: "ZimaOS", widgetMax: 24, monOpen: false };
function applyUi(u){
  if (!u) return;
  UI = u;
  applyLook(u.look, u);
  document.body.classList.toggle("c3", u.cols === 3);
  // Monitoring always open: the fold button is hidden and the panel stays open
  // Monitoring off: the panel is gone (and Scriptable stops asking Glances)
  document.body.classList.toggle("nomon", u.monitoring === false);
  document.body.classList.toggle("monfixed", !!u.monFixed);
  if (u.monFixed && !monOpen && u.monitoring !== false) toggleMon();
  var tw = document.getElementById("tw");
  tw.classList.toggle("nop", !u.power);
  if (!u.power) togglePmenu(false);
  var wn = document.getElementById("wnote");
  if (wn) wn.textContent = "The large widget fits " + u.widgetMax + " buttons. Dimmed ones will not show. Press, hold and drag to reorder, or drag a button between the two panels to show or hide it.";
  if (typeof markOverflow === "function") markOverflow();
}

// ---- Theme & look ----
var LOOKK = ["theme", "accent", "glow", "movingBg", "cardStyle", "corners", "font", "buttonSize", "style"];
function lookVars(look, U){
  var t = U.themes[look.theme] || U.themes.aurora;
  var a = U.accents[look.accent];
  var own = a && a.c;
  return {
    bg: t.bg, g1: t.g1, g2: t.g2,
    acc: own ? a.c : t.acc, ink: own ? a.ink : t.ink,
    panel: t.panel, panel2: t.panel2,
    glowop: t.noGlow ? 0 : U.glows[look.glow],
    glass: { subtle: 0.04, normal: 0.07, strong: 0.12 }[look.cardStyle] || 0.07
  };
}
function applyLook(look, U){
  if (!look || !U || !U.themes) return;
  var v = lookVars(look, U), r = document.documentElement.style;
  r.setProperty("--bg", v.bg);
  r.setProperty("--g1", v.g1);
  r.setProperty("--g2", v.g2);
  r.setProperty("--acc", v.acc);
  r.setProperty("--acc-ink", v.ink);
  r.setProperty("--panel", v.panel);
  r.setProperty("--panel2", v.panel2);
  r.setProperty("--glowop", String(v.glowop));
  r.setProperty("--glass", String(v.glass));
  var b = document.body.classList;
  b.toggle("noanim", !look.movingBg);
  b.toggle("sharp", look.corners === "sharp");
  b.toggle("frnd", look.font === "rounded");
  b.toggle("fserif", look.font === "serif");
  b.toggle("compact", look.buttonSize === "compact");
  b.toggle("glass", look.style === "glass");
}

// ---- Settings sheet (gear icon) ----
var S = {}, srq = 0, scb = {};
function sEl(id){ return document.getElementById(id); }
function sSend(d, cb){
  d.rq = ++srq;
  scb[d.rq] = cb;
  S.busy = true;
  sEl("sbody").classList.add("sbusy");
  location.href = "set://go?d=" + encodeURIComponent(JSON.stringify(d));
}
function setReply(rq, r){
  var cb = scb[rq];
  delete scb[rq];
  S.busy = false;
  sEl("sbody").classList.remove("sbusy");
  if (cb) cb(r || { ok: false, error: "No answer" });
}
function openSettings(){
  if (Date.now() < ign) return;
  togglePmenu(false);
  // the settings window of the server on screen
  var tt = sEl("sttl");
  tt.innerHTML = "SETTINGS<small></small>";
  tt.querySelector("small").textContent = document.getElementById("title").textContent;
  S = { busy: false };
  sEl("serr").textContent = "";
  sEl("sbody").innerHTML = '<div class="swait">Loading...</div>';
  sEl("ssave").style.display = "none";
  sEl("sv").className = "show";
  sSend({ op: "load" }, function(r){
    if (!r.ok) { sEl("sbody").innerHTML = ""; sEl("serr").textContent = r.error || "Couldn't load the settings"; return; }
    S.info = r;
    S.f = JSON.parse(JSON.stringify(r.s));
    S.gpass = "";
    S.clearGpass = false;
    S.title = document.getElementById("title").textContent;
    S.loginOpen = null;
    S.lu = "";
    S.lp = "";
    S.lerr = "";
    sEl("ssave").style.display = "";
    sRender();
  });
}
function closeSettings(){
  // a look that was only previewed goes back to the saved one
  if (S.previewed && !S.saved) applyLook(UI.look, UI);
  sEl("sv").className = "";
  if (document.activeElement) document.activeElement.blur();
}

// building blocks
function sSec(parent, title){
  var sec = document.createElement("div");
  sec.className = "ssec";
  var h = document.createElement("div");
  h.className = "sec";
  h.textContent = title;
  sec.appendChild(h);
  var card = document.createElement("div");
  card.className = "scard";
  sec.appendChild(card);
  parent.appendChild(sec);
  return card;
}
function sHint(parent, text){
  var d = document.createElement("div");
  d.className = "shint";
  d.textContent = text;
  parent.appendChild(d);
  return d;
}
function sLabel(parent, text){
  var f = document.createElement("div");
  f.className = "f";
  var l = document.createElement("label");
  l.textContent = text;
  f.appendChild(l);
  parent.appendChild(f);
  return f;
}
function sInputEl(val, ph, mode, type){
  var w = document.createElement("div");
  w.className = "in";
  var i = document.createElement("input");
  i.type = type || "text";
  i.value = val == null ? "" : String(val);
  i.placeholder = ph || "";
  i.setAttribute("autocapitalize", "none");
  i.setAttribute("autocorrect", "off");
  i.spellcheck = false;
  if (mode) i.inputMode = mode;
  w.appendChild(i);
  return { w: w, i: i };
}
// Address field: an http:// / https:// list plus the server's IP or domain (optionally :port and a path).
// o: { scheme (the usual one), ports: { http, https } (added when no port is typed), ph, host (for an empty field) }
// The full address (e.g. http://192.168.1.10:61208) is what's saved; the usual port is hidden while editing.
function sUrl(parent, label, key, o, hint){
  var f = sLabel(parent, label);
  var sel = document.createElement("select");
  ["http", "https"].forEach(function(x){
    var op = document.createElement("option");
    op.value = x;
    op.textContent = x + "://";
    sel.appendChild(op);
  });
  var chev = document.createElement("span");
  chev.className = "chev";
  chev.innerHTML = "&#9662;";
  var x = sInputEl("", o.ph || "", "url");
  var inp = x.i;
  var portOf = function(sc){ return o.ports && o.ports[sc]; };
  // the usual port is only added for IP addresses and local names; a public domain is usually behind a
  // reverse proxy on the standard http / https port
  var local = function(h){
    return /^[0-9.]+$/.test(h) || /^[[]/.test(h) || h.indexOf(".") < 0 || /[.](local|lan|home|internal|localdomain|home[.]arpa|ts[.]net)$/i.test(h);
  };
  var cur = String(S.f[key] || ""), m = cur.match(/^(https?):[/][/](.*)$/i);
  sel.value = m ? m[1].toLowerCase() : o.scheme;
  var rest = m ? m[2] : "";
  if (portOf(sel.value)) rest = rest.replace(new RegExp("^([^/:]+):" + portOf(sel.value) + "(?=/|$)"), "$1");
  inp.value = rest;
  function compose(){
    var v = inp.value.trim();
    var mm = v.match(/^(https?):[/][/](.*)$/i);
    // a full address was typed or pasted: take its http / https over
    if (mm) { sel.value = mm[1].toLowerCase(); v = mm[2].replace(/[/]+$/, ""); inp.value = v; }
    v = v.replace(/[/]+$/, "");
    var host = v;
    if (!host) {
      // empty: the usual address, unless the other http / https was picked
      if (sel.value === o.scheme || !o.host) { S.f[key] = ""; return; }
      host = o.host;
    }
    var hp = host.match(/^([^/:]+)(:[0-9]+)?(.*)$/);
    var port = hp && !hp[2] && portOf(sel.value) && local(hp[1]) ? ":" + portOf(sel.value) : "";
    S.f[key] = sel.value + "://" + (hp ? hp[1] + (hp[2] || "") + port + hp[3] : host);
  }
  inp.oninput = compose;
  sel.onchange = compose;
  x.w.insertBefore(chev, x.w.firstChild);
  x.w.insertBefore(sel, chev);
  f.appendChild(x.w);
  if (hint) sHint(parent, hint);
  return inp;
}
// ---- Shared by Settings and Add server: server types, the one server address field ----
var PLATFORM_ORDER = ["zimaos", "proxmox", "truenas", "unraid", "synology", "omv", "linux"];
var TYPE_HINT = {
  none: "Pick the system your server runs. It decides how Reboot / Shut down reach the server; until you pick one there is no Reboot / Shut down menu. Not sure? Debian, Ubuntu and similar are Linux.",
  zimaos: "ZimaOS: Reboot / Shut down use your ZimaOS login. Nothing to install.",
  proxmox: "Proxmox VE: Reboot / Shut down use the Proxmox API with your Proxmox login (e.g. root@pam). Nothing to install. Its self-signed certificate is accepted.",
  truenas: "TrueNAS: Reboot / Shut down use your TrueNAS login (the admin account, e.g. truenas_admin, or root on older versions). Nothing to install. With https, TrueNAS needs a certificate the iPhone trusts; plain http on your home network or VPN is fine.",
  unraid: "Unraid: Reboot / Shut down log in with your Unraid login (root) and send what Unraid's own Reboot / Shut down buttons send. Nothing to install.",
  synology: "Synology DSM: Reboot / Shut down use the DSM Web API with an administrator account. Nothing to install.",
  omv: "OpenMediaVault: Reboot / Shut down use OpenMediaVault's own API with an administrator account (admin by default). Nothing to install.",
  linux: "Linux (Debian, Ubuntu Server, Fedora ...): there's no web API for reboot, so -board uses a tiny helper service on the server, set up with one command over SSH from Settings. No login needed."
};
// a row of radio buttons (same look as in Settings)
function radioRow(opts, value, onPick){
  var row = document.createElement("div");
  row.className = "seg segw";
  row.setAttribute("role", "radiogroup");
  opts.forEach(function(o){
    var b = document.createElement("button");
    b.type = "button";
    b.setAttribute("role", "radio");
    b.setAttribute("aria-checked", value === o[0] ? "true" : "false");
    if (value === o[0]) b.className = "on";
    b.innerHTML = "<i></i><span></span>";
    b.querySelector("span").textContent = o[1];
    b.onclick = function(){ if (value !== o[0]) onPick(o[0]); };
    row.appendChild(b);
  });
  return row;
}
function typeOpts(PL){ return PLATFORM_ORDER.map(function(k){ return [k, PL[k].short || PL[k].name]; }); }
// the hint under the server address: which port is used when none is typed
function addrHint(PL, P){
  var pl = PL[P], ports = pl ? pl.ports : {};
  var usual = ports.http
    ? " For " + (pl.short || pl.name) + ", port " + (ports.http === ports.https ? ports.http : ports.http + " (http) or " + ports.https + " (https)") + (P === "linux" ? " (the helper)" : "") + " is used unless you type another."
    : "";
  return "Pick http or https, then type the IP or name, plus a port if it isn't the usual one (e.g. 192.168.1.10:90)." + usual + " LIVE / OFFLINE checks this address, Reboot / Shut down use it, and Glances is looked for on it. Use your VPN address to reach it away from home.";
}
// The one server address field: http / https list + IP or name with an optional port.
// o: { PL, platform, host, scheme ("" = the usual one), port, onChange({ host, scheme, port }) }
function serverAddrField(parent, label, o){
  var f = document.createElement("div");
  f.className = "f";
  var l = document.createElement("label");
  l.textContent = label;
  f.appendChild(l);
  var usual = (o.PL[o.platform] || {}).scheme || "http";
  var sel = document.createElement("select");
  ["http", "https"].forEach(function(x){
    var op = document.createElement("option");
    op.value = x;
    op.textContent = x + "://";
    sel.appendChild(op);
  });
  sel.value = o.scheme || usual;
  var chev = document.createElement("span");
  chev.className = "chev";
  chev.innerHTML = "&#9662;";
  var x = sInputEl((o.host || "") + (o.port ? ":" + o.port : ""), "192.168.1.10 or a name", "url");
  x.w.insertBefore(chev, x.w.firstChild);
  x.w.insertBefore(sel, chev);
  f.appendChild(x.w);
  parent.appendChild(f);
  function changed(){
    var v = x.i.value.trim(), m = v.match(/^(https?):[/][/](.*)$/i);
    // a full address was typed or pasted: its http / https goes into the list
    if (m) { sel.value = m[1].toLowerCase(); v = m[2]; x.i.value = v; }
    v = v.replace(/[/?#].*$/, "");
    var hp = v.match(/^([^:]+)(?::([0-9]{1,5}))?$/);
    o.onChange({ host: hp ? hp[1] : v, port: hp && hp[2] ? hp[2] : "", scheme: sel.value === usual ? "" : sel.value });
  }
  x.i.oninput = changed;
  sel.onchange = changed;
  return x.i;
}
function sText(parent, label, key, ph, mode, hint){
  var f = sLabel(parent, label), x = sInputEl(S.f[key], ph, mode);
  x.i.oninput = function(){ S.f[key] = x.i.value.trim(); };
  f.appendChild(x.w);
  if (hint) sHint(parent, hint);
  return x.i;
}
function sNum(parent, label, key, ph, hint){
  var f = sLabel(parent, label), x = sInputEl(S.f[key], ph, "numeric");
  x.i.oninput = function(){ S.f[key] = x.i.value.trim(); };
  f.appendChild(x.w);
  if (hint) sHint(parent, hint);
}
function sSelect(parent, label, key, opts, hint, asNumber){
  var f = sLabel(parent, label);
  var w = document.createElement("div");
  w.className = "in";
  var s = document.createElement("select");
  s.style.flex = "1";
  opts.forEach(function(o){
    var op = document.createElement("option");
    op.value = String(o[0]);
    op.textContent = o[1];
    s.appendChild(op);
  });
  s.value = String(S.f[key]);
  s.onchange = function(){ S.f[key] = asNumber ? Number(s.value) : s.value; sChanged(key); };
  w.appendChild(s);
  var c = document.createElement("span");
  c.className = "chev";
  c.innerHTML = "&#9662;";
  w.appendChild(c);
  f.appendChild(w);
  if (hint) sHint(parent, hint);
}
function sSwitch(on, onChange){
  var sw = document.createElement("div");
  sw.className = on ? "sw on" : "sw";
  sw.innerHTML = "<i></i>";
  sw.onclick = function(){ sw.classList.toggle("on"); onChange(sw.classList.contains("on")); };
  return sw;
}
function sToggle(parent, label, key, hint){
  var tg = document.createElement("div");
  tg.className = "tg";
  var l = document.createElement("label");
  l.textContent = label;
  tg.appendChild(l);
  tg.appendChild(sSwitch(!!S.f[key], function(v){ S.f[key] = v; sChanged(key); }));
  parent.appendChild(tg);
  if (hint) sHint(parent, hint);
}
// a row of radio buttons; after(v) runs when the choice changes
function sRadio(parent, label, key, opts, after){
  var f = sLabel(parent, label);
  var row = document.createElement("div");
  row.className = "seg";
  row.setAttribute("role", "radiogroup");
  opts.forEach(function(o){
    var b = document.createElement("button");
    b.type = "button";
    b.setAttribute("role", "radio");
    b.setAttribute("aria-checked", S.f[key] === o[0] ? "true" : "false");
    if (S.f[key] === o[0]) b.className = "on";
    b.innerHTML = "<i></i><span></span>";
    b.querySelector("span").textContent = o[1];
    b.onclick = function(){
      if (S.busy || S.f[key] === o[0]) return;
      S.f[key] = o[0];
      if (after) after(o[0]);
    };
    row.appendChild(b);
  });
  f.appendChild(row);
  return f;
}
// theme / look changes are previewed on the dashboard straight away
function sChanged(key){
  if (LOOKK.indexOf(key) < 0) return;
  S.previewed = true;
  var o = {};
  LOOKK.forEach(function(k){ o[k] = S.f[k]; });
  applyLook(o, UI);
}
function sSwatches(parent, label, key, items){
  var f = sLabel(parent, label);
  var row = document.createElement("div");
  row.className = "swl";
  f.appendChild(row);
  var nm = document.createElement("div");
  nm.className = "swn";
  f.appendChild(nm);
  function draw(){
    row.innerHTML = "";
    items.forEach(function(it){
      var b = document.createElement("button");
      b.type = "button";
      b.className = "swt" + (S.f[key] === it.v ? " on" : "");
      b.style.background = it.css;
      if (it.dashed) b.style.borderStyle = "dashed";
      b.setAttribute("aria-label", it.name);
      b.onclick = function(){ S.f[key] = it.v; draw(); sChanged(key); if (it.after) it.after(); };
      row.appendChild(b);
      if (S.f[key] === it.v) nm.textContent = it.name;
    });
  }
  draw();
  return { redraw: function(next){ items = next; draw(); } };
}
function sChip(parent, text, fn){
  var b = document.createElement("button");
  b.type = "button";
  b.className = "chip";
  b.textContent = text;
  b.onclick = function(){ if (!S.busy) fn(b); };
  parent.appendChild(b);
  return b;
}
function sRow(parent, title, sub){
  var r = document.createElement("div");
  r.className = "srow";
  var t = document.createElement("div");
  t.className = "st";
  var b = document.createElement("b");
  b.textContent = title;
  t.appendChild(b);
  if (sub) {
    var sp = document.createElement("span");
    sp.textContent = sub;
    t.appendChild(sp);
  }
  r.appendChild(t);
  parent.appendChild(r);
  return r;
}
function sSize(b){
  var u = ["B", "KB", "MB", "GB", "TB"], i = 0;
  while (b >= 1024 && i < 4) { b /= 1024; i++; }
  return (b >= 100 || i === 0 ? Math.round(b) : b.toFixed(1)) + " " + u[i];
}
// a button that needs a second tap
function sArmed(btn, label, armedLabel, fn){
  var armed = false;
  btn.textContent = label;
  btn.onclick = function(){
    if (S.busy) return;
    if (!armed) { armed = true; btn.textContent = armedLabel; return; }
    armed = false;
    btn.textContent = label;
    fn();
  };
}
function sMsg(r){
  if (!r.ok) { sEl("serr").textContent = r.error || "Something went wrong"; return false; }
  sEl("serr").textContent = "";
  if (r.msg) toast(r.msg, 3000);
  return true;
}

function sRender(){
  var b = sEl("sbody"), I = S.info, f = S.f;
  b.innerHTML = "";

  if (I.firstRun) {
    var wel = document.createElement("div");
    wel.className = "cnote";
    wel.textContent = "Welcome to -board. Enter your server's IP or name below, pick its server type, tap Test connection, then Save. After that, add your services with the + button.";
    b.appendChild(wel);
  }

  var dv0 = document.createElement("div");
  dv0.className = "sdiv";
  dv0.textContent = "THIS SERVER";
  b.appendChild(dv0);

  // ---------- SERVER ----------
  // the same order as Add server: name, type, address, login
  var c = sSec(b, "SERVER");
  var P = f.platform, hostPh = f.serverHost || I.host || "192.168.1.10", PL = I.platforms;
  var nmf = sLabel(c, "SERVER NAME"), nmx = sInputEl(S.title, "e.g. NAS", null);
  nmx.i.maxLength = 20;
  nmx.i.oninput = function(){ S.title = nmx.i.value; };
  nmf.appendChild(nmx.w);
  var tf = sLabel(c, "SERVER TYPE");
  tf.appendChild(radioRow(typeOpts(PL), P, function(v){ f.platform = v; sRender(); }));
  sHint(c, TYPE_HINT[P] || TYPE_HINT.none);
  serverAddrField(c, "SERVER ADDRESS", { PL: PL, platform: P, host: f.serverHost, scheme: f.serverScheme, port: f.serverPort,
    onChange: function(a){ f.serverHost = a.host; f.serverScheme = a.scheme; f.serverPort = a.port; } });
  sHint(c, addrHint(PL, P));
  // an older separate address for the server system on another host (still used until it's cleared)
  if (P !== "none" && f[PL[P].url]) {
    var ov = sRow(c, "Reboot / Shut down use " + f[PL[P].url], "A separate address from an older version of -board");
    sChip(ov, "Use the server address", function(){ f[PL[P].url] = ""; sRender(); });
  }
  if (P === "proxmox") {
    sText(c, "NODE NAME", "proxmoxNode", "Automatic (e.g. pve)", null, "Only needed in a cluster: the node to reboot, as shown in the Proxmox web UI.");
  } else if (P === "linux") {
    var hb = sChip(c, I.logins.linux ? "Copy setup command" : "Create key & copy setup command", function(){
      var k = "";
      try {
        var a = new Uint8Array(24);
        crypto.getRandomValues(a);
        for (var i = 0; i < a.length; i++) k += ("0" + a[i].toString(16)).slice(-2);
      } catch (e) { k = ""; }
      sSend({ op: "helper", s: S.f, key: k }, function(r){ if (sMsg(r)) { I.logins.linux = true; st.hasLogin = (UI.platform === "linux") || st.hasLogin; sRender(); } });
    });
    hb.style.alignSelf = "flex-start";
    sHint(c, "Then SSH into the server (ssh you@" + hostPh + "), paste and press Enter. It asks for your sudo password once, installs the helper as a system service and opens its port in ufw / firewalld if they're on. Run it again after changing the address or port.");
  }
  if (P !== "linux" && P !== "none") {
    var LN = PL[P].login;
    var openLogin = function(){ S.loginOpen = P; S.lu = ""; S.lp = ""; S.lerr = ""; sRender(); };
    if (I.logins[P]) {
      var zr = sRow(c, LN + " login saved", "Stored in the iOS Keychain");
      sChip(zr, "Change", openLogin);
      sChip(zr, "Forget", function(){
        sSend({ op: "forget", platform: P }, function(r){ if (sMsg(r)) { I.logins[P] = false; if (UI.platform === P) st.hasLogin = false; sRender(); } });
      });
    } else {
      var nr = sRow(c, "No " + LN + " login saved", "Enter it now, or you'll be asked the first time you use Reboot / Shut down");
      sChip(nr, "Enter login", openLogin);
    }
    // the login form (checked against the server before it's saved)
    if (S.loginOpen === P) {
      var lf = document.createElement("div");
      lf.className = "slogin";
      c.appendChild(lf);
      var uf = sLabel(lf, LN.toUpperCase() + " USERNAME"), ux = sInputEl(S.lu, PL[P].userPh || "", null);
      ux.i.setAttribute("autocomplete", "username");
      ux.i.oninput = function(){ S.lu = ux.i.value; };
      uf.appendChild(ux.w);
      var pwf = sLabel(lf, LN.toUpperCase() + " PASSWORD"), pwx = sInputEl(S.lp, "", null, "password");
      pwx.i.setAttribute("autocomplete", "current-password");
      pwx.i.oninput = function(){ S.lp = pwx.i.value; };
      pwf.appendChild(pwx.w);
      if (S.lerr) {
        var le = document.createElement("div");
        le.className = "sres";
        le.innerHTML = '<div class="bad"><i></i><span></span></div>';
        le.querySelector("span").textContent = S.lerr;
        lf.appendChild(le);
      }
      var lb = document.createElement("div");
      lb.className = "srow";
      lf.appendChild(lb);
      sChip(lb, "Cancel", function(){ S.loginOpen = null; S.lp = ""; sRender(); });
      sChip(lb, "Check & save login", function(){
        if (!S.lu.trim() || !S.lp) { S.lerr = "Enter a username and a password."; sRender(); return; }
        S.lerr = "";
        sSend({ op: "login", platform: P, u: S.lu, p: S.lp, s: S.f }, function(r){
          if (!r.ok) { S.lerr = r.error || "The login didn't work."; sRender(); return; }
          I.logins[P] = true;
          if (UI.platform === P) st.hasLogin = true;
          S.loginOpen = null;
          S.lp = "";
          toast(r.msg, 3500);
          sRender();
        });
      });
      sHint(lf, "Checked against the server at the address above (even before you save Settings), then stored in the iOS Keychain." + (P === "proxmox" ? " Typing root means root@pam." : ""));
      if (!S.lu) setTimeout(function(){ ux.i.focus(); }, 60);
    }
  } else if (P === "linux" && I.logins.linux) {
    var kr = sRow(c, "Helper key saved", "Stored in the iOS Keychain");
    sChip(kr, "Forget", function(){
      sSend({ op: "forget", platform: "linux" }, function(r){ if (sMsg(r)) { I.logins.linux = false; if (UI.platform === "linux") st.hasLogin = false; sRender(); } });
    });
  }
  sToggle(c, "SHOW REBOOT / SHUT DOWN", "showPower", "Off hides the arrow and menu next to the server name.");
  var res = document.createElement("div");
  res.className = "sres";
  var tb = sChip(c, "Test connection", function(){
    res.innerHTML = '<div><span>Testing...</span></div>';
    sSend({ op: "test", s: S.f, gpass: S.gpass }, function(r){
      res.innerHTML = "";
      if (!r.ok) { res.textContent = r.error || "Test failed"; return; }
      r.lines.forEach(function(l){
        var d = document.createElement("div");
        if (!l.ok) d.className = "bad";
        d.innerHTML = "<i></i><span></span>";
        d.querySelector("span").textContent = l.t;
        res.appendChild(d);
      });
    });
  });
  tb.style.alignSelf = "flex-start";
  c.appendChild(res);
  sHint(c, "Tests the server, Glances (below) and Reboot / Shut down with the values in this form, even before you save them.");

  // ---------- MONITORING ----------
  c = sSec(b, "MONITORING");
  var mt = document.createElement("div");
  mt.className = "tg";
  mt.innerHTML = "<label>MONITORING (GLANCES)</label>";
  mt.appendChild(sSwitch(!!f.monitoring, function(v){ f.monitoring = v; sRender(); }));
  c.appendChild(mt);
  sHint(c, f.monitoring
    ? "CPU, RAM, storage and network from Glances, in a fold-away panel on the dashboard and as gauges on the widget."
    : "Off: no Monitoring panel, no gauges on the widget (it shows your service buttons instead), and -board sends no requests to Glances. The LIVE / OFFLINE status still works.");
  if (f.monitoring) {
    var gh = f.serverHost || I.host;
    sUrl(c, "GLANCES ADDRESS", "baseUrl", { scheme: "http", ports: { http: 61208, https: 61208 }, host: gh, ph: gh ? gh + " (the server)" : "IP or name" }, "Leave empty for " + (gh ? "http://" + gh + ":61208" : "the server address on port 61208") + ". Otherwise pick http or https and type the IP or name; port 61208 is added for IP addresses and local names unless you type another. A public domain (behind a reverse proxy) gets no port added.");
    sSelect(c, "GLANCES API VERSION", "apiVersion", [[4, "4 (Glances 4.x)"], [3, "3 (Glances 3.x)"]], null, true);
    sText(c, "GLANCES USERNAME", "username", "Only if Glances has a password");
    var pf = sLabel(c, "GLANCES PASSWORD"), px = sInputEl(S.gpass, I.hasGpass ? "Saved. Leave empty to keep it" : "Optional", null, "password");
    px.i.oninput = function(){ S.gpass = px.i.value; };
    pf.appendChild(px.w);
    if (I.hasGpass) {
      var cg = sChip(c, "Remove saved Glances password", function(){
        S.clearGpass = !S.clearGpass;
        cg.textContent = S.clearGpass ? "Will be removed when you save (tap to undo)" : "Remove saved Glances password";
      });
      cg.style.alignSelf = "flex-start";
    }
    var dl = sLabel(c, "DRIVES IN THE STORAGE CARD");
    if (I.mounts && I.mounts.length) {
      var shown = I.mounts.slice();
      f.diskMounts.forEach(function(m){
        if (!shown.some(function(x){ return x.mnt === m; })) shown.push({ mnt: m, missing: true });
      });
      shown.forEach(function(m){
        var sub = m.missing ? "Not found in Glances right now" : sSize(m.size) + (m.dev ? " · " + m.dev : "") + (m.size ? " · " + Math.round(m.used / m.size * 100) + "% used" : "");
        var row = sRow(dl, m.mnt, sub);
        row.appendChild(sSwitch(f.diskMounts.indexOf(m.mnt) >= 0, function(on){
          f.diskMounts = f.diskMounts.filter(function(x){ return x !== m.mnt; });
          if (on) f.diskMounts.push(m.mnt);
        }));
      });
      sHint(c, "None picked = pick automatically. Paths are as the Glances container sees them.");
    } else {
      var dx = sInputEl(f.diskMounts.join(", "), "/, /mnt/data", null);
      dx.i.oninput = function(){ f.diskMounts = dx.i.value.split(",").map(function(x){ return x.trim(); }).filter(Boolean); };
      dl.appendChild(dx.w);
      sHint(c, "Couldn't reach Glances to list the drives, so type the paths, separated by commas. Empty = pick automatically.");
    }
    sNum(c, "MAX DRIVES WHEN PICKED AUTOMATICALLY", "maxDisks", "3");
    var ifo = [["", "Automatic (the busiest)"]];
    (I.ifaces || []).forEach(function(n){ ifo.push([n, n]); });
    if (f.networkInterface && !ifo.some(function(o){ return o[0] === f.networkInterface; })) ifo.push([f.networkInterface, f.networkInterface]);
    sSelect(c, "NETWORK INTERFACE", "networkInterface", ifo, I.ifaces ? null : "Couldn't reach Glances to list the interfaces.");
    sToggle(c, "OPEN MONITORING ON START", "monitoringOpen");
    sToggle(c, "KEEP MONITORING ALWAYS OPEN", "monitoringFixed", "Removes the Monitoring button, so the cards and network speed always show.");
    sNum(c, "DASHBOARD UPDATES EVERY (SECONDS)", "liveSeconds", "3", "Lower = smoother graphs, more battery while the dashboard is open.");
    var tl = sLabel(c, "TEMPERATURE COLOURS (°C)");
    var th = document.createElement("div");
    th.className = "trow th";
    th.innerHTML = "<span></span><span>ORANGE FROM</span><span>RED FROM</span>";
    tl.appendChild(th);
    [["cpu", "CPU"], ["nvme", "NVMe"], ["hdd", "HDD"]].forEach(function(k){
      var r = document.createElement("div");
      r.className = "trow";
      var lb = document.createElement("b");
      lb.textContent = k[1];
      r.appendChild(lb);
      ["warm", "hot"].forEach(function(w){
        var x = sInputEl(f.tempLimits[k[0]][w], "", "numeric");
        x.i.oninput = function(){ f.tempLimits[k[0]][w] = x.i.value.trim(); };
        r.appendChild(x.w);
      });
      tl.appendChild(r);
    });
  }

  // ---------- WIDGET ----------
  var dv1 = document.createElement("div");
  dv1.className = "sdiv";
  dv1.textContent = "SHARED BY ALL SERVERS";
  b.appendChild(dv1);

  c = sSec(b, "WIDGET");
  sHint(c, "Each widget shows one server: long-press the widget, Edit Widget, and type the server's name or number (1 = the first) under Parameter. Empty = the first server.");
  sNum(c, "REFRESH EVERY (MINUTES)", "refreshMinutes", "5", "iOS decides the exact moment, so this is the shortest gap, not a promise.");
  sText(c, "CORNER BUTTON LINK", "linkUrl", "https://... (empty = no button)", "url", "A small button in the widget corner, e.g. your ZimaOS or Proxmox page.");
  sToggle(c, "STATUS DOTS ON THE WIDGET", "widgetStatusDots", "Each widget refresh checks every button, which takes a couple of seconds.");
  sText(c, "WIDGET TITLE", "widgetTitle", "Same as the dashboard name");
  sSelect(c, "BUTTON ROWS", "widgetRows", [[1, "1 row (4 buttons)"], [2, "2 rows (8 buttons)"], [3, "3 rows (12 buttons)"], [4, "4 rows (16 buttons)"], [5, "5 rows (20 buttons)"], [6, "6 rows (24 buttons)"]], null, true);

  // ---------- BUTTONS ----------
  c = sSec(b, "BUTTONS");
  sToggle(c, "SHOW LOGOS", "useLogos", "Off shows the emoji instead.");
  sSelect(c, "DASHBOARD COLUMNS", "columns", [[2, "2 columns"], [3, "3 columns"]], null, true);
  sNum(c, "CHECK UP/DOWN EVERY (SECONDS)", "statusSeconds", "30");
  sNum(c, "COUNT AS DOWN AFTER (SECONDS)", "pingTimeout", "3", "How long a service may take to answer. The widget waits at most 3 seconds.");
  var rl = I.hasBuiltin || I.hidden.length ? sLabel(c, "DELETED BUTTONS") : null;
  if (!rl) {
    // no built-in buttons in this script, so nothing to restore
  } else if (I.hidden.length) {
    I.hidden.forEach(function(h){
      var row = sRow(rl, h.name);
      sChip(row, "Restore", function(){
        sSend({ op: "restore", id: h.id }, function(r){
          if (sMsg(r)) { I.hidden = I.hidden.filter(function(x){ return x.id !== h.id; }); sRender(); }
        });
      });
    });
    if (I.hidden.length > 1) {
      var ra = sChip(rl, "Restore all", function(){
        sSend({ op: "restore" }, function(r){ if (sMsg(r)) { I.hidden = []; sRender(); } });
      });
      ra.style.alignSelf = "flex-start";
    }
  } else {
    sRow(rl, "None", "Built-in buttons you delete show up here");
  }

  // ---------- THEME & LOOK ----------
  c = sSec(b, "THEME & LOOK");
  sRadio(c, "STYLE", "style", [["classic", "Classic"], ["glass", "Liquid Glass"]], function(){ sChanged("style"); sRender(); });
  sHint(c, f.style === "glass"
    ? "Liquid Glass: iOS 26 style glass cards with a bright rim, a third background glow, springy animation on every press, menu and sheet, and glass rings on the widget."
    : "Classic: the plain -board look. Switch to Liquid Glass for glass cards and animations.");
  sToggle(c, "STARTUP ANIMATION", "splash", "A short animated -board logo when the dashboard opens (tap it to skip). Works with either style.");
  var accRow = null;
  var accItems = function(){
    var ta = (UI.themes[f.theme] || UI.themes.aurora).acc;
    return Object.keys(UI.accents).map(function(k){
      var a = UI.accents[k];
      return { v: k, name: a.c ? a.name : "Same as theme", css: a.c || ta, dashed: !a.c };
    });
  };
  sSwatches(c, "THEME", "theme", Object.keys(UI.themes).map(function(k){
    var t = UI.themes[k];
    return {
      v: k,
      name: t.name,
      css: t.noGlow ? "#000" : "radial-gradient(circle at 28% 25%, rgb(" + t.g1 + ") 0%, rgba(" + t.g1 + ",0) 62%), radial-gradient(circle at 75% 82%, rgb(" + t.g2 + ") 0%, rgba(" + t.g2 + ",0) 66%), " + t.bg,
      after: function(){ if (accRow) accRow.redraw(accItems()); }
    };
  }));
  accRow = sSwatches(c, "ACCENT COLOUR", "accent", accItems());
  sHint(c, "Used for switches, Save buttons and the selected tab. Status colours (green / orange / red) stay the same.");
  sSelect(c, "BACKGROUND GLOW", "glow", [["off", "Off"], ["soft", "Soft"], ["normal", "Normal"], ["strong", "Strong"]]);
  sToggle(c, "MOVING BACKGROUND", "movingBg", "Off keeps the glow still (saves a little battery).");
  sSelect(c, "CARD STYLE", "cardStyle", [["subtle", "Subtle glass"], ["normal", "Normal glass"], ["strong", "Strong glass"]]);
  sSelect(c, "CORNERS", "corners", [["round", "Rounded"], ["sharp", "Sharper"]]);
  sSelect(c, "FONT", "font", [["system", "System (SF Pro)"], ["rounded", "Rounded (SF Pro Rounded)"], ["serif", "Serif (New York)"]]);
  sSelect(c, "BUTTON SIZE", "buttonSize", [["normal", "Normal"], ["compact", "Compact"]]);
  sHint(c, "Changes show on the dashboard behind this sheet as you pick them. Cancel puts the old look back. The widget background follows the theme and glow.");

  // ---------- DATA ----------
  c = sSec(b, "DATA");
  var bx = document.createElement("div");
  bx.className = "sbtns";
  c.appendChild(bx);
  var b1 = document.createElement("button");
  b1.textContent = "Clear logo cache";
  b1.onclick = function(){ if (!S.busy) sSend({ op: "clearlogos" }, sMsg); };
  bx.appendChild(b1);
  var b2 = document.createElement("button");
  b2.textContent = "Export settings";
  b2.onclick = function(){ if (!S.busy) sSend({ op: "export" }, sMsg); };
  bx.appendChild(b2);
  var b3 = document.createElement("button");
  sArmed(b3, "Import settings", "Tap again to replace everything", function(){
    sSend({ op: "import" }, function(r){ if (sMsg(r)) closeSettings(); });
  });
  bx.appendChild(b3);
  var b4 = document.createElement("button");
  b4.className = "rtxt";
  sArmed(b4, "Reset layout", "Tap again to reset the layout", function(){
    sSend({ op: "reset" }, sMsg);
  });
  bx.appendChild(b4);
  sHint(c, "Export copies all settings, buttons, stacks and layout to the clipboard (passwords stay on the phone). Import reads them back from the clipboard. Reset layout clears the button order and stacks but keeps your own buttons.");

  // ---------- DELETE SERVER (the very bottom, asks to confirm) ----------
  var dname = document.getElementById("title").textContent;
  c = sSec(b, "DELETE SERVER");
  var dbx = document.createElement("div");
  dbx.className = "sbtns";
  c.appendChild(dbx);
  var dbt = document.createElement("button");
  dbt.className = "del";
  dbt.textContent = "Delete " + dname;
  var only = (st.servers || []).length < 2;
  dbt.disabled = only;
  dbt.onclick = function(){ if (!S.busy && !only) openDelServer(dname); };
  dbx.appendChild(dbt);
  sHint(c, only
    ? "This is your only server, so it can't be deleted. Add another server first (swipe the server name left)."
    : "Deletes this server's settings, logins, buttons, stacks and layout from the phone. Your other servers stay. Asks to confirm.");
  if (!S.animated) { S.animated = true; restagger(b.children); }
}

function saveSettings(){
  if (Date.now() < ign || S.busy || !S.f) return;
  var f = S.f, n = function(v){ return /^\\d+$/.test(String(v)); };
  var nums = ["maxDisks", "liveSeconds", "refreshMinutes", "statusSeconds", "pingTimeout"];
  for (var i = 0; i < nums.length; i++) if (!n(f[nums[i]])) { sEl("serr").textContent = "Use whole numbers for the number fields"; return; }
  var tk = ["cpu", "nvme", "hdd"];
  for (i = 0; i < tk.length; i++) {
    var t = f.tempLimits[tk[i]];
    if (!n(t.warm) || !n(t.hot)) { sEl("serr").textContent = "Use whole numbers for the temperatures"; return; }
    if (Number(t.hot) <= Number(t.warm)) { sEl("serr").textContent = "Red has to start above orange (" + tk[i].toUpperCase() + ")"; return; }
  }
  if (!String(f.serverHost || "").trim()) { sEl("serr").textContent = "Enter your server's IP or name (Settings > Server)"; return; }
  if (f.baseUrl && !/^https?:\\/\\/[^\\/:]+/i.test(f.baseUrl)) { sEl("serr").textContent = "The Glances address must start with http:// or https://"; return; }
  var urls = [];
  for (i = 0; i < urls.length; i++) {
    if (f[urls[i][0]] && !/^https?:\\/\\//i.test(f[urls[i][0]])) { sEl("serr").textContent = "The " + urls[i][1] + " address must start with http:// or https://"; return; }
  }
  sSend({ op: "save", s: f, title: S.title, gpass: S.gpass, clearGpass: S.clearGpass }, function(r){
    if (sMsg(r)) { S.saved = true; closeSettings(); }
  });
}
// ================= Liquid Glass motion (only while the glass look is on) =================
function isGlass(){ return document.body.classList.contains("glass"); }
function restagger(list){
  if (!isGlass()) return;
  for (var i = 0; i < list.length; i++) {
    var e = list[i];
    e.classList.remove("enter");
    void e.offsetWidth;
    e.style.setProperty("--i", Math.min(i, 18));
    e.classList.add("enter");
  }
}
// entrance classes are removed when done (a finished transform would trap menus under other cards)
document.addEventListener("animationend", function(e){
  if (e.animationName === "rise" && e.target.classList) { e.target.classList.remove("enter", "intro"); }
}, true);
// FLIP: elements glide from where they were to where they end up
function flipEls(els, fn, opts){
  if (!isGlass()) { fn(); return; }
  els = els.filter(Boolean);
  var first = els.map(function(e){ return e.getBoundingClientRect(); });
  fn();
  els.forEach(function(e){ if (e._fa) { e._fa.cancel(); e._fa = null; } });
  var last = els.map(function(e){ return e.getBoundingClientRect(); });
  els.forEach(function(e, i){
    if (!e.isConnected || !e.animate) return;
    var a = first[i], l = last[i], dx = a.left - l.left, dy = a.top - l.top;
    var k0 = { transform: "translate(" + dx + "px," + dy + "px)" }, k1 = { transform: "translate(0,0)" };
    if (opts && opts.clip && (l.width > a.width + 1 || l.height > a.height + 1)) {
      var rad = getComputedStyle(e).borderTopLeftRadius || "0px";
      k0.clipPath = "inset(0px " + Math.max(0, l.width - a.width) + "px " + Math.max(0, l.height - a.height) + "px 0px round " + rad + ")";
      k1.clipPath = "inset(0px 0px 0px 0px round " + rad + ")";
    }
    if (Math.abs(dx) < 1 && Math.abs(dy) < 1 && !k0.clipPath) return;
    e._fa = e.animate([k0, k1], { duration: 620, easing: "cubic-bezier(.2,.9,.25,1.05)" });
    e._fa.onfinish = function(){ e._fa = null; };
  });
}
// drag reorder: neighbours slide out of the way; hit tests use their final spots
function sTop(){ return (document.scrollingElement || document.body).scrollTop; }
function moveFlip(fn){
  if (!isGlass()) { fn(); return; }
  var els = [].slice.call(document.querySelectorAll("#svc>*,#hid>*,.gbody>*"));
  var anchor = els.map(function(e){ var p = e.parentNode; return p && p.classList.contains("gbody") ? p.closest(".grp") : null; });
  var first = els.map(function(e){ return e.getBoundingClientRect(); });
  var firstA = anchor.map(function(a){ return a ? a.getBoundingClientRect() : null; });
  fn();
  els.forEach(function(e){ if (e._fa) { e._fa.cancel(); e._fa = null; } });
  var now = performance.now(), s0 = sTop();
  var last = els.map(function(e){ return e.getBoundingClientRect(); });
  els.forEach(function(e, i){
    var l = last[i], f = first[i];
    e._fr = { l: l.left, t: l.top, w: l.width, h: l.height, s: s0, end: now + 340 };
    var dx = f.left - l.left, dy = f.top - l.top;
    var na = e.parentNode && e.parentNode.classList.contains("gbody") ? e.parentNode.closest(".grp") : null;
    if (na && na === anchor[i] && firstA[i]) {
      var la = na.getBoundingClientRect();
      dx -= firstA[i].left - la.left;
      dy -= firstA[i].top - la.top;
    }
    if (Math.abs(dx) < 1 && Math.abs(dy) < 1) return;
    if (!e.animate) return;
    e._fa = e.animate([{ transform: "translate(" + dx + "px," + dy + "px)" }, { transform: "translate(0,0)" }], { duration: 340, easing: "cubic-bezier(.2,.9,.3,1)" });
  });
}
function rectOf(e){
  var r = e._fr;
  if (r && performance.now() < r.end) {
    var t = r.t - (sTop() - r.s);
    return { left: r.l, right: r.l + r.w, top: t, bottom: t + r.h };
  }
  return e.getBoundingClientRect();
}
// press: squish + a glint where the finger lands (the styles only exist in the glass look)
var PRESS = ".svc,.monbtn,.addbtn,.gear,.lay,.btns button,.sbtns button,.shd button,.chip,.pmenu button,.swt,.card,.live,.seg button";
var prEl = null, prX = 0, prY = 0;
function clearPress(){ if (prEl) { prEl.classList.remove("prs"); prEl = null; } }
document.addEventListener("touchstart", function(e){
  clearPress();
  if (!isGlass()) return;
  var el = e.target.closest && e.target.closest(PRESS);
  if (!el || el.classList.contains("ghost")) return;
  var t = e.touches[0], r = el.getBoundingClientRect();
  prX = t.clientX; prY = t.clientY;
  el.style.setProperty("--mx", (t.clientX - r.left) + "px");
  el.style.setProperty("--my", (t.clientY - r.top) + "px");
  el.classList.add("prs");
  prEl = el;
}, { passive: true });
document.addEventListener("touchmove", function(e){
  if (!prEl) return;
  var t = e.touches[0];
  if (Math.abs(t.clientX - prX) > 10 || Math.abs(t.clientY - prY) > 10) clearPress();
}, { passive: true });
document.addEventListener("touchend", function(){ setTimeout(clearPress, 70); }, { passive: true });
document.addEventListener("touchcancel", clearPress, { passive: true });
// startup screen (Settings > Theme & look > Startup animation)
var booted = false, splashDone = false;
function boot(){
  if (booted) return;
  booted = true;
  document.body.classList.add("boot");
  setTimeout(endSplash, 1350);
}
function endSplash(now){
  if (splashDone) return;
  splashDone = true;
  var sp = document.getElementById("splash");
  document.body.classList.add("ready");
  if (!sp) return;
  if (now) { sp.parentNode.removeChild(sp); return; }
  sp.classList.add("out");
  setTimeout(function(){ if (sp.parentNode) sp.parentNode.removeChild(sp); }, 950);
}
// now = true: no startup screen at all (turned off), removed without fading
function skipSplash(now){ booted = true; document.body.classList.add("boot"); endSplash(now === true); }
setTimeout(function(){ if (!booted) skipSplash(); }, 3000); // in case the data never arrives
</script></body></html>`;

// Changing the server on screen: everything fetched for the last one is dropped, then the page redraws
let GEN = 0;
async function showServer(wv, dir) {
  GEN++;
  fsCache = null;
  lastStats = null;
  expanded = null;
  statusDirty = true;
  lastStatus = 0;
  await wv.evaluateJavaScript(`switchServer(${JSON.stringify(getTitle())}, ${JSON.stringify(getPayload())}, ${Number(dir) || 0})`, false);
  refreshLogos(wv);
}

async function showLiveDashboard(openStackId) {
  const sleep = (ms) => new Promise((resolve) => Timer.schedule(ms, false, resolve));
  const wv = new WebView();

  // Service buttons use an open:// link; open it in an in-app browser
  // instead of navigating away from the dashboard.
  wv.shouldAllowRequest = (req) => {
    if (req.url.startsWith("add://")) {
      (async () => {
        try {
          const d = JSON.parse(decodeURIComponent(req.url.slice(req.url.indexOf("?d=") + 3)));
          const data = loadData();
          const item = makeCustom(d);
          let id = d.id;
          if (d.id && String(d.id).startsWith("b:")) {
            data.edits[d.id] = { name: item.name, port: item.port, url: item.url, logo: item.logo };
          } else if (d.id) {
            const i = data.custom.findIndex((c) => c.id === d.id);
            if (i >= 0) data.custom[i] = { ...data.custom[i], name: item.name, port: item.port, url: item.url, logo: item.logo };
          } else {
            item.id = "c:" + Date.now();
            id = item.id;
            data.custom.push(item);
          }
          data.widgetHidden = data.widgetHidden.filter((x) => x !== id);
          if (d.onWidget === false) data.widgetHidden.push(id);
          saveData(data);
          statusDirty = true;
          clearLogoFail(item.logo); // a re-saved logo gets a fresh download try
          await pushServices(wv);
          refreshLogos(wv);
        } catch (e) {}
      })();
      return false;
    }
    if (req.url.startsWith("del://")) {
      (async () => {
        try {
          const id = decodeURIComponent(req.url.slice(req.url.indexOf("?id=") + 4));
          const data = loadData();
          if (id.startsWith("b:")) {
            delete data.edits[id];
            if (!data.hidden.includes(id)) data.hidden.push(id);
          } else {
            data.custom = data.custom.filter((c) => c.id !== id);
          }
          data.groups.forEach((g) => (g.members = g.members.filter((x) => x !== id)));
          saveData(data);
          statusDirty = true;
          await pushServices(wv);
        } catch (e) {}
      })();
      return false;
    }
    if (req.url.startsWith("exp://")) {
      expanded = decodeURIComponent(req.url.slice(req.url.indexOf("?k=") + 3)) || null;
      if (expanded) {
        (async () => {
          try {
            await pushDetail(wv);
          } catch (e) {}
        })();
      }
      return false;
    }
    if (req.url.startsWith("ord://")) {
      try {
        const o = JSON.parse(decodeURIComponent(req.url.slice(req.url.indexOf("?o=") + 3)));
        const str = (x) => typeof x === "string";
        const data = loadData();
        if (Array.isArray(o)) {
          data.order = o.filter(str);
        } else if (o && Array.isArray(o.o)) {
          // o: top-level order, g: each stack's buttons (buttons can be dragged in and out of open stacks)
          data.order = o.o.filter(str);
          if (o.g && typeof o.g === "object") {
            for (const g of data.groups) if (Array.isArray(o.g[g.id])) g.members = o.g[g.id].filter(str);
          }
        }
        saveData(data);
        pushServices(wv).catch(() => {});
      } catch (e) {}
      return false;
    }
    if (req.url.startsWith("grp://")) {
      (async () => {
        try {
          stackAction(JSON.parse(decodeURIComponent(req.url.slice(req.url.indexOf("?d=") + 3))));
          statusDirty = true;
          await pushServices(wv);
          refreshLogos(wv);
        } catch (e) {}
      })();
      return false;
    }
    if (req.url.startsWith("wrd://")) {
      try {
        const o = JSON.parse(decodeURIComponent(req.url.slice(req.url.indexOf("?o=") + 3)));
        if (o && Array.isArray(o.s) && Array.isArray(o.h)) {
          const data = loadData();
          data.widgetOrder = o.s.filter((x) => typeof x === "string");
          data.widgetHidden = o.h.filter((x) => typeof x === "string");
          saveData(data);
          pushServices(wv).catch(() => {});
        }
      } catch (e) {}
      return false;
    }
    if (req.url.startsWith("ttl://")) {
      try {
        const t = decodeURIComponent(req.url.slice(req.url.indexOf("?t=") + 3)).trim().slice(0, 20);
        if (t) {
          const data = loadData();
          data.title = t;
          saveData(data);
        }
      } catch (e) {}
      return false;
    }
    if (req.url.startsWith("set://")) {
      (async () => {
        let d = {};
        try {
          d = JSON.parse(decodeURIComponent(req.url.slice(req.url.indexOf("?d=") + 3)));
        } catch (e) {}
        let r;
        try {
          r = await settingsOp(d);
        } catch (e) {
          r = { ok: false, error: "Something went wrong: " + ((e && e.message) || e) };
        }
        try {
          await wv.evaluateJavaScript(`setReply(${JSON.stringify(d.rq)}, ${JSON.stringify(r)})`, false);
          // anything that changes buttons, logos or layout is redrawn right away
          if (r.ok && d.op === "import") {
            await showServer(wv, 0);
          } else if (r.ok && ["save", "restore", "clearlogos", "reset"].includes(d.op)) {
            await pushServices(wv);
            refreshLogos(wv);
          }
        } catch (e) {}
      })();
      return false;
    }
    if (req.url.startsWith("pwr://")) {
      (async () => {
        let d = {};
        try {
          d = JSON.parse(decodeURIComponent(req.url.slice(req.url.indexOf("?d=") + 3)));
        } catch (e) {}
        let r;
        try {
          r = await handlePower(d);
        } catch (e) {
          r = { ok: false, msg: "Something went wrong: " + ((e && e.message) || e), a: d.a, hasLogin: hasLogin() };
        }
        try {
          await wv.evaluateJavaScript(`powerResult(${JSON.stringify(r)})`, false);
        } catch (e) {}
      })();
      return false;
    }
    // switch / add / remove a server (swipe the server name, the dots, or Settings)
    if (req.url.startsWith("srv://")) {
      (async () => {
        let d = {};
        try {
          d = JSON.parse(decodeURIComponent(req.url.slice(req.url.indexOf("?d=") + 3)));
        } catch (e) {}
        try {
          const before = listServers().map((x) => x.id);
          let ok = false;
          if (d.act === "switch") ok = setCurrentServer(d.id);
          else if (d.act === "add") {
            // the new server's address and type; the login (if given) is checked before the server is added
            const own = cleanSettings({
              ...SET, serverHost: d.host, serverScheme: d.scheme, serverPort: d.port, platform: d.platform || "none",
              proxmoxNode: "", baseUrl: "", username: "", diskMounts: [], networkInterface: "",
              ...Object.fromEntries(PLATFORM_KEYS.map((k) => [PLATFORMS[k].url, ""])),
            });
            const withLogin = !!(DRIVERS[own.platform] && d.u && d.p);
            if (withLogin) {
              const r = await DRIVERS[own.platform](null, d.u, d.p, platformBase(own.platform, own), own);
              if (!r.ok) {
                const error = (r.msg || "The login didn't work.") + (r.net ? " Check the address, or add the server without the login and enter it later." : "");
                await wv.evaluateJavaScript(`addServerResult(${JSON.stringify({ ok: false, error })})`, false);
                return;
              }
            }
            const id = addServer(d.name, { serverHost: own.serverHost, serverScheme: own.serverScheme, serverPort: own.serverPort, platform: own.platform });
            if (withLogin) {
              Keychain.set(loginKeys(own.platform, id).user, d.u);
              Keychain.set(loginKeys(own.platform, id).pass, d.p);
            }
            await wv.evaluateJavaScript(`addServerResult({ ok: true })`, false);
            ok = true;
            d.next = own.platform === "linux" ? "settings" : withLogin ? "Server added, login checked and saved" : "Server added";
          }
          else if (d.act === "remove") ok = removeServer(d.id);
          if (!ok) return;
          await showServer(wv, before.indexOf(CUR) < 0 ? 1 : d.dir || 0);
          // Linux still needs its helper set up: straight to its settings
          if (d.act === "add") {
            await wv.evaluateJavaScript(d.next === "settings" ? "setTimeout(openSettings, 450)" : `toast(${JSON.stringify(d.next)}, 3000)`, false);
          }
          if (d.act === "remove") await wv.evaluateJavaScript(`toast("Server deleted", 2500)`, false);
        } catch (e) {}
      })();
      return false;
    }
    if (req.url.startsWith("open://")) {
      Safari.open(decodeURIComponent(req.url.split("u=")[1]));
      return false;
    }
    return true;
  };

  // the chosen style and startup screen are set before the first paint (no flash of the other look)
  const bodyCls = [SET.style === "glass" ? "glass" : "", SET.splash ? "splashon" : ""].join(" ").trim();
  await wv.loadHTML(LIVE_HTML.replace("<body>", `<body class="${bodyCls}">`));
  await wv.evaluateJavaScript(
    `init(${JSON.stringify(getTitle())}, ${JSON.stringify(getPayload())})`,
    false
  );
  // opened from a stack pill on the widget: open that stack and scroll to it
  if (openStackId) await wv.evaluateJavaScript(`focusStack(${JSON.stringify(openStackId)})`, false);
  // download any logos not saved yet (first run, or new ones) without holding up the dashboard
  refreshLogos(wv);
  // first run: no server yet, so go straight to Settings
  // (after the startup screen, when it's on)
  if (!serverHostOf(SET)) await wv.evaluateJavaScript(`setTimeout(openSettings, ${SET.splash ? 1700 : 400})`, false);

  let open = true;
  const presented = wv.present(true).then(() => {
    open = false;
  });

  // LIVE / OFFLINE: is anything on the server answering? Checked in the background every REACH_SECONDS.
  let reachBusy = false;
  let lastReach = 0;
  let reachFor = "";
  while (open) {
    if (!serverHostOf(SET)) {
      await wv.evaluateJavaScript("offline(true)", false);
      await sleep(1000);
      continue;
    }
    // a changed address is checked straight away
    const rk = serverUrlOf(SET) + "|" + SET.platform + "|" + SET.monitoring + "|" + SET.baseUrl;
    if (!reachBusy && (rk !== reachFor || Date.now() - lastReach > REACH_SECONDS * 1000)) {
      reachBusy = true;
      reachFor = rk;
      const g = GEN;
      serverReachable(SET)
        .then((r) => (g === GEN ? wv.evaluateJavaScript(`setReach(${JSON.stringify(!!r)})`, false) : null))
        .catch(() => {})
        .then(() => {
          reachBusy = false;
          lastReach = Date.now();
        });
    }
    // Monitoring off: no requests to Glances at all
    if (SET.monitoring) {
      const g = GEN;
      try {
        const stats = await getStats({ fsMaxAge: 10000 });
        if (g === GEN) {
          lastStats = stats;
          await wv.evaluateJavaScript(`update(${JSON.stringify(stats)})`, false);
        }
      } catch (e) {
        if (g === GEN) await wv.evaluateJavaScript("offline()", false);
      }
      try {
        await pushDetail(wv);
      } catch (e) {}
    }
    if (!statusBusy && (statusDirty || Date.now() - lastStatus > SET.statusSeconds * 1000)) {
      statusBusy = true;
      statusDirty = false;
      const g = GEN;
      checkServices(getServices(false), SET.pingTimeout)
        .then((m) => (g === GEN ? wv.evaluateJavaScript(`setUp(${JSON.stringify(m)})`, false) : null))
        .catch(() => {})
        .then(() => {
          statusBusy = false;
          lastStatus = Date.now();
        });
    }
    await sleep(SET.liveSeconds * 1000);
  }
  await presented;
}

// ---------- Run ----------
if (config.runsInWidget) {
  Script.setWidget(await buildWidget());
} else if (PREVIEW_WIDGET_IN_APP) {
  await (await buildWidget()).presentMedium();
} else {
  await showLiveDashboard((args.queryParameters || {}).stack || null);
}
Script.complete();
