package app.board

import org.json.JSONArray
import org.json.JSONObject
import kotlin.math.floor
import kotlin.math.max
import kotlin.math.min

// ---------- Default settings (the same starting values as CONFIG in the Scriptable script) ----------
object CONFIG {
    const val title = "MY SERVER"
    const val monitoring = true
    const val apiVersion = 4
    const val maxDisks = 3
    const val refreshMinutes = 5
    const val widgetStatusDots = true
    const val platform = "none"
}

// Server types (Settings > Server). Monitoring works the same on all of them (Glances);
// they only change how Reboot / Shut down reaches the server.
data class Platform(
    val key: String,
    val name: String,
    val short: String?,
    val login: String?,
    val url: String,
    val scheme: String,
    val ports: Map<String, Int>,
    val userPh: String = "",
) {
    val shortName get() = short ?: name
}

val PLATFORMS: LinkedHashMap<String, Platform> = linkedMapOf(
    "zimaos" to Platform("zimaos", "ZimaOS", null, "ZimaOS", "zimaUrl", "http", emptyMap()),
    "proxmox" to Platform("proxmox", "Proxmox VE", "Proxmox", "Proxmox", "proxmoxUrl", "https", mapOf("http" to 8006, "https" to 8006), "root@pam"),
    "truenas" to Platform("truenas", "TrueNAS", null, "TrueNAS", "truenasUrl", "http", emptyMap(), "truenas_admin"),
    "unraid" to Platform("unraid", "Unraid", null, "Unraid", "unraidUrl", "http", emptyMap(), "root"),
    "synology" to Platform("synology", "Synology DSM", "Synology", "Synology", "synologyUrl", "http", mapOf("http" to 5000, "https" to 5001)),
    "omv" to Platform("omv", "OpenMediaVault", "OMV", "OpenMediaVault", "omvUrl", "http", emptyMap(), "admin"),
    "linux" to Platform("linux", "Linux", null, null, "helperUrl", "http", mapOf("http" to 61209, "https" to 61209)),
)
val PLATFORM_KEYS = PLATFORMS.keys.toList()
val PLATFORM_URL_KEYS = PLATFORMS.values.map { it.url }

data class TempLimit(val warm: Int, val hot: Int)

// Temperatures (°C) where the number turns orange (warm) and red (hot) in the enlarged cards.
val TEMP_DEFAULTS = linkedMapOf("cpu" to TempLimit(70, 85), "nvme" to TempLimit(60, 70), "hdd" to TempLimit(45, 55))

// Settings that belong to one server (everything else is shared by all servers)
val SERVER_SETTING_KEYS = listOf(
    "serverHost", "serverScheme", "serverPort", "failover", "failoverHost", "monitoring", "baseUrl", "apiVersion", "username", "platform", "zimaUrl", "proxmoxUrl", "proxmoxNode",
    "truenasUrl", "unraidUrl", "synologyUrl", "omvUrl", "helperUrl", "showPower", "diskMounts", "maxDisks",
    "networkInterface", "monitoringOpen", "monitoringFixed", "tempLimits", "liveSeconds",
)
val SERVER_PARTS = listOf("custom", "edits", "hidden", "order", "widgetOrder", "widgetHidden", "groups")

// ---------- Themes ----------
data class ThemeDef(val name: String, val bg: String, val g1: String, val g2: String, val acc: String, val ink: String, val panel: String, val panel2: String, val noGlow: Boolean = false)

val THEMES = linkedMapOf(
    "aurora" to ThemeDef("Aurora", "#0b1220", "18,194,165", "106,92,255", "#34e0a1", "#06281d", "rgba(30,38,64,.94)", "#1e2640"),
    "ocean" to ThemeDef("Ocean", "#07111f", "0,132,255", "0,206,209", "#4fc3ff", "#04203a", "rgba(20,40,68,.94)", "#142844"),
    "sunset" to ThemeDef("Sunset", "#150d12", "255,122,69", "214,51,132", "#ff9f6b", "#3a1606", "rgba(54,30,40,.94)", "#361e28"),
    "forest" to ThemeDef("Forest", "#09140f", "46,160,67", "18,120,140", "#7ddc6f", "#0d2a0a", "rgba(24,44,36,.94)", "#182c24"),
    "rose" to ThemeDef("Rose", "#140b18", "255,92,160", "124,77,255", "#ff7eb6", "#3a0b22", "rgba(48,28,58,.94)", "#301c3a"),
    "graphite" to ThemeDef("Graphite", "#111317", "120,130,150", "70,80,100", "#d6dbe4", "#15181e", "rgba(36,38,44,.95)", "#24262c"),
    "oled" to ThemeDef("OLED black", "#000000", "0,0,0", "0,0,0", "#34e0a1", "#06281d", "rgba(22,22,26,.97)", "#16161a", noGlow = true),
)

data class AccentDef(val name: String, val c: String?, val ink: String?)

val ACCENTS = linkedMapOf(
    "" to AccentDef("Theme", null, null),
    "green" to AccentDef("Green", "#34e0a1", "#06281d"),
    "blue" to AccentDef("Blue", "#4fc3ff", "#04203a"),
    "purple" to AccentDef("Purple", "#a78bfa", "#1e1240"),
    "orange" to AccentDef("Orange", "#ff9f6b", "#3a1606"),
    "pink" to AccentDef("Pink", "#ff7eb6", "#3a0b22"),
    "white" to AccentDef("White", "#e8ecf3", "#15181e"),
)
val GLOW_LEVELS = linkedMapOf("off" to 0f, "soft" to 0.3f, "normal" to 0.5f, "strong" to 0.75f)

const val LOGO_CDN = "https://cdn.jsdelivr.net/gh/homarr-labs/dashboard-icons/png/"
const val STACK_ICON = "🗂️"
const val HELPER_PORT = 61209

// ---------- Settings ----------
data class Settings(
    val serverHost: String = "",
    val serverScheme: String = "",
    val serverPort: String = "",
    // Failover: a second address for the same server (e.g. its LAN IP), tried when the main address doesn't answer
    val failover: Boolean = false,
    val failoverHost: String = "",
    val monitoring: Boolean = CONFIG.monitoring,
    val baseUrl: String = "",
    val apiVersion: Int = CONFIG.apiVersion,
    val username: String = "",
    val platform: String = CONFIG.platform,
    val urls: Map<String, String> = PLATFORM_URL_KEYS.associateWith { "" },
    val proxmoxNode: String = "",
    val showPower: Boolean = true,
    val diskMounts: List<String> = emptyList(),
    val maxDisks: Int = CONFIG.maxDisks,
    val networkInterface: String = "",
    val monitoringOpen: Boolean = false,
    val monitoringFixed: Boolean = false,
    val tempLimits: Map<String, TempLimit> = TEMP_DEFAULTS,
    val liveSeconds: Int = 3,
    val refreshMinutes: Int = CONFIG.refreshMinutes,
    val linkUrl: String = "",
    val widgetStatusDots: Boolean = CONFIG.widgetStatusDots,
    val widgetTitle: String = "",
    val widgetRows: Int = 6,
    val useLogos: Boolean = true,
    val columns: Int = 2,
    val statusSeconds: Int = 30,
    val pingTimeout: Int = 3,
    val theme: String = "aurora",
    val accent: String = "",
    val glow: String = "normal",
    val movingBg: Boolean = true,
    val cardStyle: String = "normal",
    val corners: String = "round",
    val font: String = "system",
    val buttonSize: String = "normal",
    val style: String = "classic",
    val splash: Boolean = false,
) {
    fun url(key: String) = urls[key] ?: ""

    fun toJson(): JSONObject {
        val o = JSONObject()
        o.put("serverHost", serverHost); o.put("serverScheme", serverScheme); o.put("serverPort", serverPort)
        o.put("failover", failover); o.put("failoverHost", failoverHost)
        o.put("monitoring", monitoring); o.put("baseUrl", baseUrl); o.put("apiVersion", apiVersion)
        o.put("username", username); o.put("platform", platform)
        for (k in PLATFORM_URL_KEYS) o.put(k, url(k))
        o.put("proxmoxNode", proxmoxNode); o.put("showPower", showPower)
        o.put("diskMounts", JSONArray(diskMounts)); o.put("maxDisks", maxDisks)
        o.put("networkInterface", networkInterface); o.put("monitoringOpen", monitoringOpen); o.put("monitoringFixed", monitoringFixed)
        val t = JSONObject()
        for ((k, v) in tempLimits) t.put(k, JSONObject().put("warm", v.warm).put("hot", v.hot))
        o.put("tempLimits", t)
        o.put("liveSeconds", liveSeconds); o.put("refreshMinutes", refreshMinutes); o.put("linkUrl", linkUrl)
        o.put("widgetStatusDots", widgetStatusDots); o.put("widgetTitle", widgetTitle); o.put("widgetRows", widgetRows)
        o.put("useLogos", useLogos); o.put("columns", columns); o.put("statusSeconds", statusSeconds); o.put("pingTimeout", pingTimeout)
        o.put("theme", theme); o.put("accent", accent); o.put("glow", glow); o.put("movingBg", movingBg)
        o.put("cardStyle", cardStyle); o.put("corners", corners); o.put("font", font); o.put("buttonSize", buttonSize)
        o.put("style", style); o.put("splash", splash)
        return o
    }
}

val DEFAULTS = Settings()

fun jsNumber(v: Any?): Double? = when (v) {
    null, JSONObject.NULL -> null
    is Number -> v.toDouble()
    is Boolean -> if (v) 1.0 else 0.0
    is String -> if (v.isBlank()) 0.0 else v.trim().toDoubleOrNull()
    else -> null
}

fun jsRound(x: Double) = floor(x + 0.5)

fun clampNum(v: Any?, lo: Int, hi: Int, def: Int): Int {
    if (v == null || v == JSONObject.NULL || v == "") return def
    val n = jsNumber(v) ?: return def
    if (n.isNaN() || n.isInfinite()) return def
    return min(hi.toDouble(), max(lo.toDouble(), jsRound(n))).toInt()
}

fun hostOf(url: String): String {
    val m = Regex("^https?://([^:/]+)", RegexOption.IGNORE_CASE).find(url)
    return m?.groupValues?.get(1) ?: "localhost"
}

fun cleanSettings(raw: JSONObject?): Settings {
    val r = raw ?: JSONObject()
    fun strOf(k: String, def: String, max: Int = 300): String {
        val v = r.opt(k)
        return if (v is String) v.trim().take(max) else def
    }
    fun boolOf(k: String, def: Boolean): Boolean = (r.opt(k) as? Boolean) ?: def
    fun pick(k: String, allowed: Collection<String>, def: String): String {
        val v = r.opt(k)
        return if (v is String && v in allowed) v else def
    }

    var baseUrl = strOf("baseUrl", DEFAULTS.baseUrl)
    if (!Regex("^https?://[^/:]+", RegexOption.IGNORE_CASE).containsMatchIn(baseUrl)) baseUrl = ""
    baseUrl = baseUrl.replace(Regex("/+$"), "")

    var serverHost = strOf("serverHost", DEFAULTS.serverHost, 200)
    var serverScheme = r.opt("serverScheme").let { if (it == "http" || it == "https") it as String else "" }
    var serverPort = r.opt("serverPort").let { v ->
        val s = if (v == null || v == JSONObject.NULL || v == false || v == "" || v == 0) "" else v.toString()
        if (Regex("^\\d{1,5}$").matches(s)) s else ""
    }
    val typed = Regex("^(https?)://", RegexOption.IGNORE_CASE).find(serverHost)
    if (typed != null && serverScheme.isEmpty()) serverScheme = typed.groupValues[1].lowercase()
    serverHost = serverHost.replace(Regex("^[a-z]+://", RegexOption.IGNORE_CASE), "").replace(Regex("[/?#].*$"), "")
    val hp = Regex("^([^\\[\\]:]+):(\\d+)$").find(serverHost)
    if (hp != null) {
        serverHost = hp.groupValues[1]
        if (serverPort.isEmpty()) serverPort = hp.groupValues[2]
    }
    if (serverHost.isEmpty() && baseUrl.isNotEmpty()) serverHost = hostOf(baseUrl)
    // the failover address is only an IP or name: it uses the main address's http / https and port
    val failoverHost = strOf("failoverHost", "", 200).replace(Regex("^[a-z]+://", RegexOption.IGNORE_CASE), "")
        .replace(Regex("[/?#].*$"), "").replace(Regex("^([^\\[\\]:]+):\\d+$"), "$1")

    val monitoring = boolOf("monitoring", DEFAULTS.monitoring)
    val apiVersion = clampNum(r.opt("apiVersion"), 3, 4, DEFAULTS.apiVersion)
    val username = strOf("username", DEFAULTS.username, 100)
    val rp = r.opt("platform")
    val platform = when {
        rp is String && PLATFORMS.containsKey(rp) -> rp
        rp == "none" || rp == "auto" -> "none"
        r.opt("showPower") is Boolean -> "zimaos"
        PLATFORMS.containsKey(DEFAULTS.platform) -> DEFAULTS.platform
        else -> "none"
    }
    val urls = LinkedHashMap<String, String>()
    for (k in PLATFORM_URL_KEYS) {
        var u = strOf(k, "")
        if (u.isNotEmpty() && !Regex("^https?://", RegexOption.IGNORE_CASE).containsMatchIn(u)) u = ""
        urls[k] = u.replace(Regex("/+$"), "")
    }
    // An older separate address for the server system on the same host becomes part of the server address
    val PLo = PLATFORMS[platform]
    if (PLo != null && urls[PLo.url]!!.isNotEmpty() && serverHost.isNotEmpty() && hostOf(urls[PLo.url]!!).lowercase() == serverHost.lowercase()) {
        val m = Regex("^(https?)://[^/:]+(?::(\\d+))?", RegexOption.IGNORE_CASE).find(urls[PLo.url]!!)
        if (m != null && serverScheme.isEmpty() && serverPort.isEmpty()) {
            val sc = m.groupValues[1].lowercase()
            serverScheme = if (sc == PLo.scheme) "" else sc
            val port = m.groupValues[2]
            serverPort = if (port.isNotEmpty() && port.toInt() != (PLo.ports[sc] ?: 0)) port else ""
        }
        urls[PLo.url] = ""
    }
    val proxmoxNode = strOf("proxmoxNode", "", 64).replace(Regex("[^A-Za-z0-9._-]"), "")
    val showPower = boolOf("showPower", DEFAULTS.showPower)
    val dm = r.opt("diskMounts")
    val diskMounts = if (dm is JSONArray) (0 until dm.length()).mapNotNull { dm.opt(it) as? String }.filter { it.isNotEmpty() }.take(12) else DEFAULTS.diskMounts
    val maxDisks = clampNum(r.opt("maxDisks"), 1, 12, DEFAULTS.maxDisks)
    val networkInterface = strOf("networkInterface", "", 60)
    val tl = r.optJSONObject("tempLimits") ?: JSONObject()
    val temps = LinkedHashMap<String, TempLimit>()
    for ((k, d) in TEMP_DEFAULTS) {
        val x = tl.optJSONObject(k) ?: JSONObject()
        val warm = clampNum(x.opt("warm"), 20, 120, d.warm)
        temps[k] = TempLimit(warm, max(clampNum(x.opt("hot"), 21, 130, d.hot), warm + 1))
    }
    return Settings(
        serverHost = serverHost, serverScheme = serverScheme, serverPort = serverPort,
        failover = boolOf("failover", DEFAULTS.failover), failoverHost = failoverHost,
        monitoring = monitoring, baseUrl = baseUrl, apiVersion = apiVersion, username = username, platform = platform,
        urls = urls, proxmoxNode = proxmoxNode, showPower = showPower, diskMounts = diskMounts, maxDisks = maxDisks,
        networkInterface = networkInterface,
        monitoringOpen = boolOf("monitoringOpen", DEFAULTS.monitoringOpen),
        monitoringFixed = boolOf("monitoringFixed", DEFAULTS.monitoringFixed),
        tempLimits = temps,
        liveSeconds = clampNum(r.opt("liveSeconds"), 1, 30, DEFAULTS.liveSeconds),
        refreshMinutes = clampNum(r.opt("refreshMinutes"), 1, 120, DEFAULTS.refreshMinutes),
        linkUrl = strOf("linkUrl", DEFAULTS.linkUrl),
        widgetStatusDots = boolOf("widgetStatusDots", DEFAULTS.widgetStatusDots),
        widgetTitle = strOf("widgetTitle", DEFAULTS.widgetTitle, 20),
        widgetRows = clampNum(r.opt("widgetRows"), 1, 6, DEFAULTS.widgetRows),
        useLogos = boolOf("useLogos", DEFAULTS.useLogos),
        columns = clampNum(r.opt("columns"), 2, 3, DEFAULTS.columns),
        statusSeconds = clampNum(r.opt("statusSeconds"), 10, 600, DEFAULTS.statusSeconds),
        pingTimeout = clampNum(r.opt("pingTimeout"), 1, 15, DEFAULTS.pingTimeout),
        theme = pick("theme", THEMES.keys, DEFAULTS.theme),
        accent = pick("accent", ACCENTS.keys, DEFAULTS.accent),
        glow = pick("glow", GLOW_LEVELS.keys, DEFAULTS.glow),
        movingBg = boolOf("movingBg", DEFAULTS.movingBg),
        cardStyle = pick("cardStyle", listOf("subtle", "normal", "strong"), DEFAULTS.cardStyle),
        corners = pick("corners", listOf("round", "sharp"), DEFAULTS.corners),
        font = pick("font", listOf("system", "rounded", "serif"), DEFAULTS.font),
        buttonSize = pick("buttonSize", listOf("normal", "compact"), DEFAULTS.buttonSize),
        style = pick("style", listOf("classic", "glass"), DEFAULTS.style),
        splash = boolOf("splash", DEFAULTS.splash),
    )
}

// The server's IP or name ("" = not set up yet)
fun serverHostOf(s: Settings) = s.serverHost.ifEmpty { if (s.baseUrl.isNotEmpty()) hostOf(s.baseUrl) else "" }

// ---------- Failover (main server address + failover server address) ----------
// Failover is in use only when it's switched on and a failover address is entered
fun failoverReady(s: Settings) = s.failover && s.failoverHost.isNotEmpty() && !s.failoverHost.equals(serverHostOf(s), true)

// An address with its host swapped (only when it is on the host `from`)
fun swapHost(url: String, from: String, to: String): String {
    if (url.isEmpty() || from.isEmpty() || to.isEmpty()) return url
    val m = Regex("^(https?://)([^/:]+)(.*)$", RegexOption.IGNORE_CASE).find(url) ?: return url
    return if (m.groupValues[2].equals(from, true)) m.groupValues[1] + to + m.groupValues[3] else url
}

// The same settings, reached over the failover address: the server address, and the Glances and
// Reboot / Shut down addresses that point at the main address, use the failover address instead
fun viaFailover(s: Settings): Settings {
    if (!failoverReady(s)) return s
    val main = serverHostOf(s)
    val to = s.failoverHost
    return s.copy(serverHost = to, baseUrl = swapHost(s.baseUrl, main, to), urls = s.urls.mapValues { swapHost(it.value, main, to) })
}

// Which address each server is reached over right now (true = the failover address). Kept in memory:
// worked out again by every LIVE / OFFLINE check, in the app and in the widget.
object Route {
    private val fo = java.util.concurrent.ConcurrentHashMap<String, Boolean>()
    fun onFailover(id: String) = fo[id] == true
    fun set(id: String, failover: Boolean) { fo[id] = failover }
    fun forget(id: String) { fo.remove(id) }
}

// The settings as used right now: over the failover address when that's the one that answers
fun effective(s: Settings, id: String) = if (failoverReady(s) && Route.onFailover(id)) viaFailover(s) else s

// Where Glances answers: the address in Settings, or the server on Glances' usual port 61208
fun glancesUrl(s: Settings): String {
    val h = serverHostOf(s)
    return s.baseUrl.ifEmpty { if (h.isNotEmpty()) "http://$h:61208" else "" }
}

// The server address as a full address: its http / https and port, or the usual ones for the server type
fun serverUrlOf(s: Settings, P: String = s.platform): String {
    val pl = PLATFORMS[P]
    val scheme = s.serverScheme.ifEmpty { pl?.scheme ?: "http" }
    val port = s.serverPort.ifEmpty { pl?.ports?.get(scheme)?.toString() ?: "" }
    return "$scheme://${serverHostOf(s).ifEmpty { "localhost" }}${if (port.isNotEmpty()) ":$port" else ""}"
}

// The address Reboot / Shut down uses for a server type
fun platformBase(P: String, s: Settings): String {
    val pl = PLATFORMS[P]!!
    if (s.url(pl.url).isNotEmpty()) return s.url(pl.url).replace(Regex("/+$"), "")
    return serverUrlOf(s, P)
}

data class SplitUrl(val scheme: String, val host: String, val port: String, val path: String)

fun splitUrl(url: String): SplitUrl {
    val m = Regex("^(https?)://([^/:]+)(?::(\\d+))?(.*)$", RegexOption.IGNORE_CASE).find(url)
    return if (m != null) SplitUrl(m.groupValues[1].lowercase(), m.groupValues[2], m.groupValues[3], m.groupValues[4])
    else SplitUrl("http", url, "", "")
}

fun formatBytes(b0: Double): String {
    val units = listOf("B", "KB", "MB", "GB", "TB")
    var b = b0
    var i = 0
    while (b >= 1024 && i < units.size - 1) { b /= 1024; i++ }
    return "${if (b >= 100 || i == 0) jsRound(b).toLong().toString() else String.format(java.util.Locale.US, "%.1f", b)} ${units[i]}"
}

fun pctColorHex(pct: Double) = if (pct >= 85) "#ff5d73" else if (pct >= 60) "#ffb454" else "#34e0a1"
