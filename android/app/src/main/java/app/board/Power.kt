package app.board

import android.content.Context
import android.util.Base64
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.withTimeoutOrNull
import okhttp3.Request
import okhttp3.Response
import okhttp3.WebSocket
import okhttp3.WebSocketListener
import org.json.JSONArray
import org.json.JSONObject
import java.net.URLEncoder

// ---------- Reboot / shut down ----------
// Every server type has a driver: run(action, user, pass, base, s). action "restart" / "off" sends the command;
// action null only checks the login (for Test connection).
data class PowerResult(val ok: Boolean, val msg: String = "", val auth: Boolean = false, val net: Boolean = false)

fun powerSent(action: String) =
    if (action == "off") "Shutdown sent. Switch the server on by hand to bring it back."
    else "Reboot sent. The server should be back in a minute or two."

fun formBody(vararg kv: Pair<String, Any>) = kv.joinToString("&") { URLEncoder.encode(it.first, "UTF-8") + "=" + URLEncoder.encode(it.second.toString(), "UTF-8") }

private fun isCancel(e: Exception) = e is kotlinx.coroutines.CancellationException

// After the command: a dropped connection is normal (the server is going down)
private suspend fun sendCommand(name: String, action: String, req: suspend () -> Resp): PowerResult {
    val r = try { req() } catch (e: Exception) { if (isCancel(e)) throw e; return PowerResult(true, powerSent(action)) }
    if (r.code == 401) return PowerResult(false, "$name refused the login. Enter it again.", auth = true)
    if (r.code >= 300) return PowerResult(false, "$name didn't accept the command (HTTP ${r.code}).")
    return PowerResult(true, powerSent(action))
}

private suspend fun jsonOrNull(f: suspend () -> Resp): Pair<Resp, JSONObject?>? = try {
    val r = f()
    r to (try { JSONObject(r.body) } catch (e: Exception) { null })
} catch (e: Exception) { if (isCancel(e)) throw e; null }

object Power {
    // --- ZimaOS ---
    suspend fun zima(action: String?, user: String, pass: String, base: String, s: Settings): PowerResult {
        val lg = jsonOrNull { Net.request("$base/v1/users/login", "POST", mapOf("Content-Type" to "application/json"), JSONObject().put("username", user).put("password", pass).toString()) }
        if (lg?.second == null) return PowerResult(false, "Can't reach ZimaOS at $base", net = true)
        val r = lg.second!!
        val token = r.optJSONObject("data")?.optJSONObject("token")?.optString("access_token").orEmpty()
        if (token.isEmpty()) return PowerResult(false, "Login failed: " + r.optString("message").ifEmpty { "wrong username or password" }, auth = true)
        if (action == null) return PowerResult(true, "saved login works")
        val res = sendCommand("ZimaOS", action) { Net.request("$base/v1/sys/state/${if (action == "off") "off" else "restart"}", "PUT", mapOf("Authorization" to token), "") }
        if (res.auth || res.msg.contains("HTTP 403")) return PowerResult(false, "ZimaOS refused the login. Enter it again.", auth = true)
        return res
    }

    // --- Proxmox VE ---
    fun pveUser(u0: String): String { val u = u0.trim(); return if (u.isNotEmpty() && !u.contains("@")) "$u@pam" else u }

    suspend fun pve(action: String?, user: String, pass: String, base: String, s: Settings): PowerResult {
        val lg = jsonOrNull { Net.request("$base/api2/json/access/ticket", "POST", mapOf("Content-Type" to "application/x-www-form-urlencoded"), formBody("username" to pveUser(user), "password" to pass)) }
        if (lg?.second == null) return PowerResult(false, "Can't reach Proxmox at $base", net = true)
        val d = lg.second!!.optJSONObject("data")
        if (d != null && d.has("NeedTFA") && d.opt("NeedTFA") != false && d.opt("NeedTFA") != 0) return PowerResult(false, "This account uses two-factor login. Use an account without it (e.g. a separate user with the Sys.PowerMgmt permission).", auth = true)
        val ticket = d?.optString("ticket").orEmpty()
        if (ticket.isEmpty()) return PowerResult(false, "Login failed: wrong username or password (e.g. root@pam)", auth = true)
        val auth = mapOf("Cookie" to "PVEAuthCookie=$ticket", "CSRFPreventionToken" to d!!.optString("CSRFPreventionToken"))
        // The node: the one named in Settings, the only one, or the one at the server address
        var node = s.proxmoxNode
        if (node.isEmpty()) {
            val nodes = try {
                JSONObject(Net.request("$base/api2/json/nodes", headers = auth).body).optJSONArray("data").objs().map { it.optString("node") }.filter { it.isNotEmpty() }
            } catch (e: Exception) { if (isCancel(e)) throw e; return PowerResult(false, "Couldn't list the Proxmox nodes") }
            if (nodes.size == 1) node = nodes[0]
            else if (nodes.isEmpty()) return PowerResult(false, "Proxmox didn't list any nodes. Enter the node name in Settings.")
            else {
                val host = serverHostOf(s).lowercase()
                var hit = nodes.find { it.lowercase() == host || host.startsWith(it.lowercase() + ".") }
                if (hit == null) try {
                    hit = JSONObject(Net.request("$base/api2/json/cluster/status", headers = auth).body).optJSONArray("data").objs()
                        .find { it.optString("type") == "node" && it.optString("ip") == host }?.optString("name")
                } catch (e: Exception) { if (isCancel(e)) throw e }
                node = hit ?: return PowerResult(false, "Several Proxmox nodes found (${nodes.joinToString(", ")}). Enter the node name in Settings.")
            }
        }
        if (action == null) return PowerResult(true, "saved login works (node $node)")
        val r = sendCommand("Proxmox", action) {
            Net.request("$base/api2/json/nodes/${URLEncoder.encode(node, "UTF-8")}/status", "POST", auth + ("Content-Type" to "application/x-www-form-urlencoded"), "command=" + if (action == "off") "shutdown" else "reboot")
        }
        if (r.msg.contains("HTTP 403")) return PowerResult(false, "This Proxmox user isn't allowed to power off node \"$node\" (needs Sys.PowerMgmt).")
        return r
    }

    // --- TrueNAS: JSON-RPC 2.0 over a WebSocket (25.04+), the REST API on older versions ---
    private data class WsResult(val ok: Boolean = false, val auth: Boolean = false, val error: String? = null, val timeout: Boolean = false, val closed: Boolean = false, val stage: String = "connect", val results: List<Any?> = emptyList())

    private suspend fun truenasWs(base: String, user: String, pass: String, calls: List<Triple<String, JSONArray, JSONArray?>>): WsResult {
        val url = base.replaceFirst(Regex("^http", RegexOption.IGNORE_CASE), "ws") + "/api/current"
        val done = CompletableDeferred<WsResult>()
        var stage = "connect"
        var id = 0
        val pending = HashMap<Int, (JSONObject) -> Unit>()
        val results = ArrayList<Any?>()
        lateinit var ws: WebSocket
        fun finish(r: WsResult) { if (done.complete(r.copy(stage = stage, results = results))) try { ws.close(1000, null) } catch (_: Exception) {} }
        fun call(method: String, params: JSONArray, cb: (JSONObject) -> Unit) {
            val i = ++id
            pending[i] = cb
            ws.send(JSONObject().put("jsonrpc", "2.0").put("id", i).put("method", method).put("params", params).toString())
        }
        fun msg(e: JSONObject?) = e?.optString("message")?.ifEmpty { null } ?: e?.optJSONObject("data")?.optString("reason")?.ifEmpty { null } ?: "error"
        val listener = object : WebSocketListener() {
            override fun onOpen(webSocket: WebSocket, response: Response) {
                stage = "login"
                call("auth.login", JSONArray().put(user).put(pass)) { m ->
                    if (m.has("error") && !m.isNull("error")) { finish(WsResult(error = msg(m.optJSONObject("error")))); return@call }
                    if (m.opt("result") != true) { finish(WsResult(auth = true)); return@call }
                    stage = "call"
                    var k = 0
                    fun next() {
                        if (k >= calls.size) { finish(WsResult(ok = true)); return }
                        val c = calls[k]
                        call(c.first, c.second) { r ->
                            if (r.has("error") && !r.isNull("error") && c.third != null) {
                                call(c.first, c.third!!) { r2 ->
                                    if (r2.has("error") && !r2.isNull("error")) { finish(WsResult(error = msg(r2.optJSONObject("error")))); return@call }
                                    results.add(r2.opt("result")); k++; next()
                                }
                                return@call
                            }
                            if (r.has("error") && !r.isNull("error")) { finish(WsResult(error = msg(r.optJSONObject("error")))); return@call }
                            results.add(r.opt("result")); k++; next()
                        }
                    }
                    next()
                }
            }
            override fun onMessage(webSocket: WebSocket, text: String) {
                val m = try { JSONObject(text) } catch (e: Exception) { return }
                pending.remove(m.optInt("id", -1))?.invoke(m)
            }
            override fun onFailure(webSocket: WebSocket, t: Throwable, response: Response?) { finish(WsResult(error = "connection")) }
            override fun onClosed(webSocket: WebSocket, code: Int, reason: String) { finish(WsResult(closed = true)) }
            override fun onClosing(webSocket: WebSocket, code: Int, reason: String) { finish(WsResult(closed = true)) }
        }
        ws = try { Net.client(12.0).newWebSocket(Request.Builder().url(url).build(), listener) } catch (e: Exception) { return WsResult(error = e.toString()) }
        return withTimeoutOrNull(12000) { done.await() } ?: run { finish(WsResult(timeout = true)); done.await() }
    }

    suspend fun truenas(action: String?, user: String, pass: String, base: String, s: Settings): PowerResult {
        val method = if (action == "off") "system.shutdown" else "system.reboot"
        // 25.10+ needs a reason; earlier JSON-RPC versions take only options, hence the fallback
        val calls = if (action != null) listOf(Triple(method, JSONArray().put("Requested from the -board app"), JSONArray())) else listOf(Triple("system.info", JSONArray(), null))
        val w = truenasWs(base, user, pass, calls)
        if (w.ok) {
            if (action != null) return PowerResult(true, powerSent(action))
            val v = (w.results.firstOrNull() as? JSONObject)?.optString("version").orEmpty()
            return PowerResult(true, "saved login works" + if (v.isNotEmpty()) " ($v)" else "")
        }
        if (w.auth) return PowerResult(false, "TrueNAS refused the login (wrong username or password, or the account uses two-factor login).", auth = true)
        if (action != null && w.stage == "call" && (w.closed || w.timeout)) return PowerResult(true, powerSent(action))
        if (w.stage != "connect") return PowerResult(false, "TrueNAS: " + (w.error ?: "no answer"))

        // No JSON-RPC WebSocket: an older TrueNAS (24.10 and earlier, or CORE) with the REST API
        val auth = mapOf("Authorization" to "Basic " + Base64.encodeToString("$user:$pass".toByteArray(), Base64.NO_WRAP))
        if (action == null) {
            val r = jsonOrNull { Net.request("$base/api/v2.0/system/info", headers = auth) }
                ?: return PowerResult(false, "Can't reach TrueNAS at $base", net = true)
            if (r.first.code == 401) return PowerResult(false, "TrueNAS refused the login", auth = true)
            if (r.first.code != 200) return PowerResult(false, "TrueNAS answered HTTP ${r.first.code}")
            val v = r.second?.optString("version").orEmpty()
            return PowerResult(true, "saved login works" + if (v.isNotEmpty()) " ($v)" else "")
        }
        return sendCommand("TrueNAS", action) { Net.request("$base/api/v2.0/system/${if (action == "off") "shutdown" else "reboot"}", "POST", auth + ("Content-Type" to "application/json"), "{}") }
    }

    // --- Unraid ---
    suspend fun unraid(action: String?, user: String, pass: String, base: String, s: Settings): PowerResult {
        // 1. log in; the answer is a redirect that sets the session cookie, so don't follow it
        val lg = try {
            Net.request("$base/login", "POST", mapOf("Content-Type" to "application/x-www-form-urlencoded"), formBody("username" to user.ifEmpty { "root" }, "password" to pass), follow = false)
        } catch (e: Exception) { if (isCancel(e)) throw e; return PowerResult(false, "Can't reach Unraid at $base", net = true) }
        val loc = lg.headers["Location"].orEmpty()
        if (lg.code in 300..399 && Regex("^https?://", RegexOption.IGNORE_CASE).containsMatchIn(loc) && hostOf(loc) != hostOf(base)) {
            return PowerResult(false, "Unraid sends you on to ${loc.replace(Regex("/login.*$", RegexOption.IGNORE_CASE), "")}. Enter that as the server address in Settings.")
        }
        if (lg.code < 300 || lg.code >= 400) {
            return if (Regex("password", RegexOption.IGNORE_CASE).containsMatchIn(lg.body)) PowerResult(false, "Unraid refused the login (wrong username or password).", auth = true)
            else PowerResult(false, "Unraid answered HTTP ${lg.code}")
        }
        val cookie = lg.cookies()
        // 2. the csrf_token every Unraid page carries
        val html = try { Net.request("$base/Main", headers = mapOf("Cookie" to cookie)).body } catch (e: Exception) { if (isCancel(e)) throw e; "" }
        val m = Regex("csrf_token[\"'\\s]*[:=]\\s*[\"']([0-9A-Za-z]{8,})[\"']").find(html)
            ?: return if (Regex("name=[\"']?password", RegexOption.IGNORE_CASE).containsMatchIn(html)) PowerResult(false, "Unraid refused the login.", auth = true)
            else PowerResult(false, "Logged in to Unraid, but couldn't read its security token.")
        if (action == null) return PowerResult(true, "saved login works")
        // 3. what Unraid's own Reboot / Shut down buttons send
        return sendCommand("Unraid", action) {
            Net.request("$base/webGui/include/Boot.php", "POST", mapOf("Cookie" to cookie, "Content-Type" to "application/x-www-form-urlencoded"), formBody("cmd" to if (action == "off") "shutdown" else "reboot", "csrf_token" to m.groupValues[1]))
        }
    }

    // --- Synology DSM ---
    private val SYNO_ERRORS = mapOf(
        400 to "wrong username or password",
        401 to "this account is disabled",
        402 to "this account isn't allowed to log in",
        403 to "this account uses two-factor login. Use an administrator account without it",
        404 to "the two-factor code failed",
        406 to "two-factor login is enforced for this account",
        407 to "too many failed logins: this address is blocked for a while",
    )

    private suspend fun synoApi(base: String, cgi: String, vararg params: Pair<String, Any>) =
        Net.request("$base/webapi/$cgi", "POST", mapOf("Content-Type" to "application/x-www-form-urlencoded"), formBody(*params))

    suspend fun synology(action: String?, user: String, pass: String, base: String, s: Settings): PowerResult {
        val login = arrayOf<Pair<String, Any>>("api" to "SYNO.API.Auth", "method" to "login", "account" to user, "passwd" to pass, "session" to "board", "format" to "sid")
        var r = try {
            var j = JSONObject(synoApi(base, "entry.cgi", *login, "version" to 6).body)
            // DSM 6: the login API lives in auth.cgi
            if (!j.optBoolean("success") && j.optJSONObject("error")?.optInt("code") in listOf(102, 103, 104)) {
                j = JSONObject(synoApi(base, "auth.cgi", *login, "version" to 3).body)
            }
            j
        } catch (e: Exception) { if (isCancel(e)) throw e; return PowerResult(false, "Can't reach Synology DSM at $base", net = true) }
        val sid = r.optJSONObject("data")?.optString("sid").orEmpty()
        if (!r.optBoolean("success") || sid.isEmpty()) {
            val c = r.optJSONObject("error")?.optInt("code")
            return PowerResult(false, "Synology: " + (SYNO_ERRORS[c] ?: "login failed (error $c)"), auth = c in listOf(400, 401, 402, 403, 404, 406))
        }
        if (action == null) return PowerResult(true, "saved login works")
        val j = try {
            JSONObject(synoApi(base, "entry.cgi", "api" to "SYNO.Core.System", "version" to 1, "method" to if (action == "off") "shutdown" else "reboot", "_sid" to sid).body.ifEmpty { "{}" })
        } catch (e: Exception) { if (isCancel(e)) throw e; return PowerResult(true, powerSent(action)) }
        if (j.has("success") && !j.optBoolean("success")) {
            val c = j.optJSONObject("error")?.optInt("code")
            return PowerResult(false, if (c == 105) "This Synology account isn't an administrator." else "Synology didn't accept the command (error $c).")
        }
        return PowerResult(true, powerSent(action))
    }

    // --- OpenMediaVault ---
    private suspend fun omvRpc(base: String, body: JSONObject, cookie: String? = null) =
        Net.request("$base/rpc.php", "POST", mapOf("Content-Type" to "application/json") + (if (!cookie.isNullOrEmpty()) mapOf("Cookie" to cookie) else emptyMap()), body.toString())

    suspend fun omv(action: String?, user: String, pass: String, base: String, s: Settings): PowerResult {
        val lg = try {
            omvRpc(base, JSONObject().put("service", "Session").put("method", "login").put("params", JSONObject().put("username", user).put("password", pass)).put("options", JSONObject.NULL))
        } catch (e: Exception) { if (isCancel(e)) throw e; return PowerResult(false, "Can't reach OpenMediaVault at $base", net = true) }
        val r = try { JSONObject(lg.body.ifEmpty { "{}" }) } catch (e: Exception) { return PowerResult(false, "Can't reach OpenMediaVault at $base", net = true) }
        val resp = r.optJSONObject("response") ?: JSONObject()
        if (resp.optString("status") == "challengeRequired") return PowerResult(false, "This OpenMediaVault account uses two-factor login. Use an administrator account without it.", auth = true)
        if (!(resp.opt("authenticated") == true || resp.optString("status") == "authenticated")) {
            return PowerResult(false, "OpenMediaVault: " + (r.optJSONObject("error")?.optString("message")?.ifEmpty { null } ?: "wrong username or password"), auth = true)
        }
        val cookie = lg.cookies().ifEmpty { resp.optString("sessionid").let { if (it.isNotEmpty()) "X-OPENMEDIAVAULT-SESSIONID=$it" else "" } }
        if (action == null) return PowerResult(true, "saved login works")
        // a 1 second delay lets OpenMediaVault answer before it goes down
        val j = try {
            JSONObject(omvRpc(base, JSONObject().put("service", "System").put("method", if (action == "off") "shutdown" else "reboot").put("params", JSONObject().put("delay", 1)).put("options", JSONObject.NULL), cookie).body.ifEmpty { "{}" })
        } catch (e: Exception) { if (isCancel(e)) throw e; return PowerResult(true, powerSent(action)) }
        val err = j.optJSONObject("error")
        if (err != null) {
            val m = err.optString("message").ifEmpty { "error" }
            return PowerResult(false, "OpenMediaVault didn't accept the command: $m", auth = Regex("session|authenticat", RegexOption.IGNORE_CASE).containsMatchIn(m))
        }
        return PowerResult(true, powerSent(action))
    }

    suspend fun driver(P: String, action: String?, user: String, pass: String, base: String, s: Settings): PowerResult = when (P) {
        "zimaos" -> zima(action, user, pass, base, s)
        "proxmox" -> pve(action, user, pass, base, s)
        "truenas" -> truenas(action, user, pass, base, s)
        "unraid" -> unraid(action, user, pass, base, s)
        "synology" -> synology(action, user, pass, base, s)
        "omv" -> omv(action, user, pass, base, s)
        else -> PowerResult(false, "This server type doesn't use a login.")
    }

    fun hasDriver(P: String) = P in Secrets.LOGIN_KEYS

    // --- Linux (any systemd distro) via the board-power helper ---
    fun helperBase(s: Settings) = platformBase("linux", s)
    fun helperPort(s: Settings): Int {
        val b = helperBase(s)
        return Regex("^https?://[^/:]+:(\\d+)", RegexOption.IGNORE_CASE).find(b)?.groupValues?.get(1)?.toInt() ?: if (b.startsWith("https", true)) 443 else 80
    }

    // returns the HTTP status code, or 0 when the helper can't be reached
    suspend fun helperCall(base: String, key: String, path: String, method: String): Int = try {
        Net.request(base + path, method, mapOf("Authorization" to "Bearer $key"), if (method == "POST") "" else null).code
    } catch (e: Exception) { if (isCancel(e)) throw e; 0 }

    suspend fun helperPower(action: String, s: Settings): PowerResult {
        if (!Secrets.hasLogin("linux")) return PowerResult(false, "Set up the power helper first: Settings > Server > Copy setup command.")
        val base = helperBase(s)
        val code = helperCall(base, Secrets.get(Secrets.helperKey()) ?: "", if (action == "off") "/shutdown" else "/reboot", "POST")
        if (code == 0) return PowerResult(false, "Can't reach the power helper at $base. Is it set up on the server?")
        if (code == 401) return PowerResult(false, "The helper refused the key. Copy the setup command from Settings and run it again.")
        if (code >= 300) return PowerResult(false, "The helper didn't accept the command (HTTP $code).")
        return PowerResult(true, powerSent(action))
    }

    // What "Copy setup command" puts on the clipboard: paste it into an SSH session on the server.
    fun helperSetupScript(ctx: Context, key: String, port: Int): String {
        val py = ctx.assets.open("helper.py").bufferedReader().readText()
        return ctx.assets.open("helper_setup.sh").bufferedReader().readText()
            .replace("{{HELPER_PY}}", py).replace("{{KEY}}", key).replace("{{PORT}}", port.toString())
    }

    // u/p only when typed in the popup; otherwise the saved login is used
    data class HandleResult(val r: PowerResult, val a: String, val hasLogin: Boolean)

    suspend fun handlePower(a0: String, u: String, p: String, s: Settings): HandleResult {
        val a = if (a0 == "off") "off" else "restart"
        val P = s.platform
        val r: PowerResult = if (!PLATFORMS.containsKey(P)) {
            PowerResult(false, "Pick your server type first: Settings > Server > Server type.")
        } else if (P == "linux") {
            helperPower(a, s)
        } else {
            var user = u; var pass = p; var saved = false
            if ((user.isEmpty() || pass.isEmpty()) && Secrets.hasLogin(P)) {
                Secrets.savedLogin(P).let { user = it.first; pass = it.second }
                saved = true
            }
            val res = if (user.isEmpty() || pass.isEmpty()) PowerResult(false, "", auth = true) else driver(P, a, user, pass, platformBase(P, s), s)
            if (res.auth && saved) Secrets.forgetLogin(P)
            if (!res.auth && !res.net && !saved && user.isNotEmpty() && pass.isNotEmpty()) Secrets.saveLogin(P, user, pass)
            res
        }
        return HandleResult(r, a, Secrets.hasLogin(P))
    }

    // One line for "Test connection" about the Reboot / Shut down side of the chosen server type
    suspend fun testPower(s: Settings): Pair<Boolean, String> {
        val P = s.platform
        val pl = PLATFORMS[P] ?: return false to "Server type: not picked yet, so Reboot / Shut down is off. Pick it under SERVER TYPE."
        val base = platformBase(P, s)
        if (P == "linux") {
            if (!Secrets.hasLogin("linux")) return false to "Power helper: not set up yet (tap Copy setup command)"
            val code = helperCall(base, Secrets.get(Secrets.helperKey()) ?: "", "/health", "GET")
            if (code == 200) return true to "Power helper: OK at $base"
            if (code == 401) return false to "Power helper: answers, but refused the key. Run the setup command again."
            return false to "Power helper: no answer at $base" + if (code != 0) " (HTTP $code)" else ""
        }
        if (Secrets.hasLogin(P)) {
            val (user, pass) = Secrets.savedLogin(P)
            val r = driver(P, null, user, pass, base, s)
            if (r.ok) return true to "${pl.name}: OK, ${r.msg}"
            return false to "${pl.name}: " + (if (r.auth) "the saved login was refused. " else "") + r.msg.ifEmpty { "no answer at $base" }
        }
        return try {
            Net.request(base, timeout = 6.0)
            true to "${pl.name}: reachable at $base (no login saved yet)"
        } catch (e: Exception) { if (isCancel(e)) throw e; false to "${pl.name}: no answer at $base" }
    }
}
