package app.board

import org.json.JSONArray
import org.json.JSONObject
import java.io.IOException
import java.net.URLEncoder

// ---------- ZimaOS integration (only for servers whose type is ZimaOS) ----------
// Apps: start / stop / restart, logs, updates and "Pick from ZimaOS apps" (CasaOS App Management API, /v2/app_management).
// System: CPU power draw and temperature (/v1/sys/utilization), drive health and temperature (/v1/disks).
// Everything uses the ZimaOS login saved for the server (Settings > Server > Enter login).

data class ZApp(
    val id: String,          // the compose app name, e.g. "jellyfin"
    val title: String,
    val icon: String,
    val port: String,
    val scheme: String,
    val host: String,        // empty = the server's own address
    val status: String,      // "running", "exited" ...
    val path: String,        // the web page path ("/" or "/web" ...)
    val appType: String,
    val storeId: String,
) {
    val running get() = status == "running"
    // only compose apps (what the App Store installs) can be started / stopped / updated through the API
    val controllable get() = appType.isEmpty() || appType == "v2app"
}

data class ZUpdate(val id: String, val title: String, val icon: String, val version: String)
data class ZDrive(val name: String, val model: String, val path: String, val type: String, val health: Boolean?, val temp: Double?)
data class ZSys(val watts: Double?, val cpuTemp: Double?)

class ZimaErr(msg: String, val auth: Boolean = false) : IOException(msg)

object Zima {
    const val NEED_LOGIN = "Enter your ZimaOS login first: Settings > Server > Enter login."
    private val tokens = HashMap<String, String>()
    // last CPU energy reading per server: (time in s, energy in µJ). Watts = energy difference / time difference.
    private val energy = HashMap<String, Pair<Double, Double>>()
    private var lastWatts = HashMap<String, Double>()

    fun base(s: Settings) = platformBase("zimaos", s)
    private fun enc(id: String) = URLEncoder.encode(id, "UTF-8").replace("+", "%20")
    private fun isCancel(e: Exception) = e is kotlinx.coroutines.CancellationException

    fun forget(id: String = Store.CUR) = synchronized(tokens) { tokens.keys.removeAll { it.startsWith("$id|") } }

    private suspend fun login(s: Settings, id: String): String {
        val key = id + "|" + base(s)
        synchronized(tokens) { tokens[key] }?.let { return it }
        if (!Secrets.hasLogin("zimaos", id)) throw ZimaErr(NEED_LOGIN, true)
        val (u, p) = Secrets.savedLogin("zimaos", id)
        val r = try {
            Net.request("${base(s)}/v1/users/login", "POST", mapOf("Content-Type" to "application/json"), JSONObject().put("username", u).put("password", p).toString(), timeout = 8.0)
        } catch (e: Exception) { if (isCancel(e)) throw e; throw ZimaErr("Can't reach ZimaOS at ${base(s)}") }
        val j = try { JSONObject(r.body) } catch (e: Exception) { throw ZimaErr("ZimaOS didn't answer the login (HTTP ${r.code})") }
        val t = j.optJSONObject("data")?.optJSONObject("token")?.optString("access_token").orEmpty()
        if (t.isEmpty()) throw ZimaErr("ZimaOS refused the saved login. Enter it again in Settings > Server.", true)
        synchronized(tokens) { tokens[key] = t }
        return t
    }

    // One API call with the saved login; an expired token is renewed once
    suspend fun call(s: Settings, path: String, method: String = "GET", body: String? = null, timeout: Double = 10.0, id: String = Store.CUR): Resp {
        repeat(2) { attempt ->
            val t = login(s, id)
            val r = try {
                Net.request(base(s) + path, method, mapOf("Authorization" to t, "Content-Type" to "application/json"), body, timeout = timeout)
            } catch (e: Exception) { if (isCancel(e)) throw e; throw ZimaErr("Can't reach ZimaOS at ${base(s)}") }
            if (r.code == 401 || r.code == 403) {
                forget(id)
                if (attempt == 0) return@repeat
                throw ZimaErr("ZimaOS refused the saved login. Enter it again in Settings > Server.", true)
            }
            return r
        }
        throw ZimaErr("ZimaOS refused the saved login.", true)
    }

    // The "data" part of ZimaOS's { message, data } answers
    private fun data(r: Resp): Any? {
        val j = try { r.json() } catch (e: Exception) {
            if (r.code >= 400) throw ZimaErr("ZimaOS answered HTTP ${r.code}")
            return r.body
        }
        if (j is JSONObject) {
            if (r.code >= 400) throw ZimaErr(j.optString("message").ifEmpty { "ZimaOS answered HTTP ${r.code}" })
            return if (j.has("data")) j.opt("data") else j
        }
        if (r.code >= 400) throw ZimaErr("ZimaOS answered HTTP ${r.code}")
        return j
    }

    // Titles are { en_us: "Jellyfin", ... } or a plain string
    fun locText(x: Any?): String = when (x) {
        is String -> x
        is JSONObject -> x.optString("en_us").ifEmpty { x.optString("en_US") }.ifEmpty {
            x.keys().asSequence().map { x.optString(it) }.firstOrNull { it.isNotEmpty() } ?: ""
        }
        else -> ""
    }

    fun parseApp(o: JSONObject): ZApp? {
        val id = o.optString("name").ifEmpty { o.optString("store_app_id") }
        if (id.isEmpty() || o.optString("app_type") == "link") return null
        val title = locText(o.opt("title")).ifEmpty { id.replaceFirstChar { it.uppercase() } }
        val port = (o.opt("port")?.takeIf { it != JSONObject.NULL }?.toString() ?: "").filter { it.isDigit() }.take(5)
        var path = o.optString("index").trim()
        if (path.isNotEmpty() && !path.startsWith("/")) path = "/$path"
        if (path == "/") path = ""
        return ZApp(
            id, title, o.optString("icon"), port,
            if (o.optString("scheme").equals("https", true)) "https" else "http",
            o.optString("hostname").trim(), o.optString("status").lowercase(), path,
            o.optString("app_type"), o.optString("store_app_id"),
        )
    }

    suspend fun apps(s: Settings, id: String = Store.CUR): List<ZApp> {
        val d = data(call(s, "/v2/app_management/web/appgrid", id = id))
        val arr = d as? JSONArray ?: (d as? JSONObject)?.optJSONArray("list") ?: JSONArray()
        return arr.objs().mapNotNull { parseApp(it) }.distinctBy { it.id }
    }

    // action: "start", "stop" or "restart"
    suspend fun setStatus(s: Settings, app: String, action: String) {
        data(call(s, "/v2/app_management/compose/${enc(app)}/status", "PUT", JSONObject.quote(action), timeout = 60.0))
    }

    private val ANSI = Regex("\u001B\\[[0-9;?]*[ -/]*[@-~]")

    suspend fun logs(s: Settings, app: String, lines: Int = 300): String {
        val d = data(call(s, "/v2/app_management/compose/${enc(app)}/logs?lines=$lines", timeout = 20.0))
        val text = (d as? String ?: d?.toString() ?: "").replace(ANSI, "").replace("\r", "")
        return text.trimEnd().lines().takeLast(lines).joinToString("\n")
    }

    suspend fun updates(s: Settings, id: String = Store.CUR): List<ZUpdate> {
        val d = data(call(s, "/v2/app_management/apps/upgradable", id = id))
        val arr = d as? JSONArray ?: JSONArray()
        return arr.objs().mapNotNull { o ->
            val sid = o.optString("store_app_id").ifEmpty { o.optString("name") }
            if (sid.isEmpty()) null
            else ZUpdate(sid, locText(o.opt("title")).ifEmpty { sid.replaceFirstChar { it.uppercase() } }, o.optString("icon"), o.optString("version"))
        }
    }

    // Pulls the app's newest images from the App Store (ZimaOS carries on in the background)
    suspend fun update(s: Settings, app: String) {
        data(call(s, "/v2/app_management/compose/${enc(app)}", "PATCH", "", timeout = 60.0))
    }

    private fun numOf(x: Any?): Double? = when (x) {
        is Number -> x.toDouble()
        is String -> x.trim().toDoubleOrNull()
        else -> null
    }

    // CPU power: ZimaOS gives the CPU's energy counter (Intel RAPL, µJ); two readings make watts,
    // the same figure ZimaOS shows. Also its CPU temperature.
    suspend fun sys(s: Settings, id: String = Store.CUR): ZSys {
        val d = data(call(s, "/v1/sys/utilization", id = id)) as? JSONObject ?: return ZSys(null, null)
        val cpu = d.optJSONObject("cpu") ?: return ZSys(null, null)
        val temp = numOf(cpu.opt("temperature"))?.takeIf { it > 0 && it < 150 }
        val pw = cpu.opt("power")
        var watts: Double? = null
        val key = id + "|" + base(s)
        if (pw is JSONObject) {
            val e = numOf(pw.opt("value"))
            val t = numOf(pw.opt("timestamp")) ?: (System.currentTimeMillis() / 1000.0)
            if (e != null && e > 0) {
                val prev = energy[key]
                if (prev != null && t > prev.first && e >= prev.second) {
                    watts = (e - prev.second) / 1_000_000.0 / (t - prev.first)
                    lastWatts[key] = watts
                } else if (prev != null && t == prev.first) watts = lastWatts[key]
                energy[key] = t to e
            }
        } else numOf(pw)?.let { watts = it }
        return ZSys(watts?.takeIf { it >= 0 && it < 2000 }, temp)
    }

    // Drives with ZimaOS's health check (smartctl) and temperature
    suspend fun drives(s: Settings, id: String = Store.CUR): List<ZDrive> {
        val d = data(call(s, "/v1/disks", id = id))
        val arr = when (d) {
            is JSONArray -> d
            is JSONObject -> d.optJSONArray("disks") ?: d.optJSONArray("list") ?: JSONArray()
            else -> JSONArray()
        }
        return arr.objs().map { o ->
            val h = o.opt("health")
            val health = when {
                h is Boolean -> h
                h is String && h.lowercase() in listOf("true", "ok", "passed", "healthy", "good") -> true
                h is String && h.lowercase() in listOf("false", "failed", "failing", "bad") -> false
                else -> null
            }
            val path = o.optString("path")
            ZDrive(
                o.optString("name").ifEmpty { path.substringAfterLast("/") }, o.optString("model").trim(), path,
                o.optString("disk_type"), health, numOf(o.opt("temperature"))?.takeIf { it > 0 && it < 150 },
            )
        }.filter { it.name.isNotEmpty() || it.path.isNotEmpty() }
    }

    // Which ZimaOS app a button points at: the app it was added from, else the same port on the server
    fun appFor(sv: Service, apps: List<ZApp>, serverHost: String): ZApp? {
        if (sv.zima.isNotEmpty()) apps.find { it.id == sv.zima }?.let { return it }
        val u = splitUrl(sv.url)
        val port = u.port.ifEmpty { sv.port }
        if (port.isEmpty()) return null
        val h = u.host.lowercase()
        return apps.find { a ->
            a.port == port && (h == serverHost.lowercase() || h == "localhost" || (a.host.isNotEmpty() && h == a.host.lowercase()))
        }
    }

    // A mounted drive (e.g. /dev/sda1, /dev/nvme0n1p2) to the ZimaOS drive it's on (/dev/sda, /dev/nvme0n1)
    fun driveFor(device: String, drives: List<ZDrive>): ZDrive? {
        val dev = device.substringAfterLast("/")
        if (dev.isEmpty()) return null
        return drives.filter { it.path.substringAfterLast("/").let { n -> n.isNotEmpty() && dev.startsWith(n) } }
            .maxByOrNull { it.path.length }
    }
}
