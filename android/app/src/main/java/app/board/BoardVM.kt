package app.board

import android.app.Application
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.async
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import org.json.JSONObject
import java.security.SecureRandom
import java.time.Instant

data class Hist(val t: List<Long> = emptyList(), val cpu: List<Double> = emptyList(), val ram: List<Double> = emptyList(), val disk: List<Double> = emptyList())
data class ToastMsg(val text: String, val ms: Long, val seq: Long = System.nanoTime())

// What the Settings sheet needs besides the settings themselves
data class SettingsInfo(
    val s: Settings,
    val host: String,
    val firstRun: Boolean,
    val mounts: List<Mount>?,
    val ifaces: List<String>?,
    val hidden: List<String>,
    val logins: Map<String, Boolean>,
    val hasGpass: Boolean,
)
data class Mount(val mnt: String, val dev: String, val size: Double, val used: Double)

const val HMAX = 100
const val REACH_SECONDS = 10

class BoardVM(app: Application) : AndroidViewModel(app) {
    private val ctx: Context get() = getApplication()

    // the settings as used right now (over the failover address while that's the one that answers)
    var set by mutableStateOf(Settings())
    // the settings as saved (the main server address): what Settings shows and saves
    var saved by mutableStateOf(Settings())
    // true while the server is reached over its failover address
    var onFailover by mutableStateOf(false)
    var title by mutableStateOf("")
    var layout by mutableStateOf(Layout(emptyList(), emptyList(), emptyList(), emptyList(), emptyList()))
    var servers by mutableStateOf(listOf<ServerRef>())
    var cur by mutableStateOf("")
    var logoVer by mutableIntStateOf(0)
    var hasLogin by mutableStateOf(false)

    var stats by mutableStateOf<Stats?>(null)
    var glancesOk by mutableStateOf<Boolean?>(null)
    var reach by mutableStateOf<Boolean?>(null)
    var hist by mutableStateOf(Hist())
    val details = mutableStateMapOf<String, Detail>()
    var expanded by mutableStateOf<String?>(null)
    var svcUp by mutableStateOf<Map<String, Boolean>>(emptyMap())
    var monOpen by mutableStateOf(false)
    val openG = mutableStateMapOf<String, Boolean>()
    var mode by mutableStateOf("dash")
    var toast by mutableStateOf<ToastMsg?>(null)
    var switchDir by mutableIntStateOf(0)
    var switchSeq by mutableIntStateOf(0)
    // the look shown while Settings previews a theme (null = the saved one)
    var lookPreview by mutableStateOf<Settings?>(null)
    var focusStackReq by mutableStateOf<String?>(null)

    // ZimaOS (only for servers of type ZimaOS with a saved login)
    var zApps by mutableStateOf<List<ZApp>?>(null)
    var zUpg by mutableStateOf<Set<String>>(emptySet())
    // installed apps the App Store has a newer version of (the "↑ N updates" chip)
    val zUpdates: List<ZApp> get() = if (!zimaReady) emptyList() else zApps.orEmpty().filter { zUpdatable(it) }
    fun zUpdatable(a: ZApp) = a.id.lowercase() in zUpg
    var zSys by mutableStateOf<ZSys?>(null)
    var zDrives by mutableStateOf<List<ZDrive>?>(null)
    private var zBusy = false
    private var zDrivesAt = 0L
    private var lastZApps = 0L
    val isZima get() = set.platform == "zimaos"
    val zimaReady get() = isZima && hasLogin

    private var glances = Glances(Settings())
    private var gen = 0
    private var loopJob: Job? = null
    private var statusDirty = true
    private var lastStatus = 0L
    private var reachBusy = false
    private var lastReach = 0L
    private var reachFor = ""
    private var statusBusy = false
    private var logoBusy = false

    init {
        Store.init(ctx)
        refreshAll()
        monOpen = set.monitoring && (set.monitoringOpen || set.monitoringFixed)
    }

    // After Settings are saved (or another server is shown): use the new values right away
    fun refreshAll() {
        saved = Store.settings()
        onFailover = failoverReady(saved) && Route.onFailover(Store.CUR)
        set = if (onFailover) viaFailover(saved) else saved
        title = Store.getTitle()
        servers = Store.listServers()
        cur = Store.CUR
        hasLogin = PLATFORMS.containsKey(set.platform) && Secrets.hasLogin(set.platform)
        glances = Glances(set)
        pushServices()
    }

    fun pushServices() {
        layout = Store.getLayout()
    }

    private fun refreshWidgets() {
        viewModelScope.launch { try { BoardWidget.refresh(ctx) } catch (_: Exception) {} }
    }

    fun showToast(t: String, ms: Long = 2500) { toast = ToastMsg(t, ms) }

    // ---------- the live loop: only while the dashboard is on screen ----------
    fun onResume() {
        if (loopJob?.isActive == true) return
        loopJob = viewModelScope.launch { loop() }
        refreshLogos()
    }

    fun onPause() {
        loopJob?.cancel()
        loopJob = null
        reachBusy = false
        statusBusy = false
    }

    private suspend fun loop() {
        while (true) {
            if (serverHostOf(set).isEmpty()) { delay(1000); continue }
            val g = gen
            // LIVE / OFFLINE: is anything on the server answering? A changed address is checked straight away.
            // With Failover on, this also picks the address to use: the main one, or the failover one if only it answers.
            val sv = saved
            val rk = serverUrlOf(sv) + "|" + sv.platform + "|" + sv.monitoring + "|" + sv.baseUrl + "|" + (if (failoverReady(sv)) sv.failoverHost else "")
            if (!reachBusy && (rk != reachFor || System.currentTimeMillis() - lastReach > REACH_SECONDS * 1000)) {
                reachBusy = true
                val first = rk != reachFor
                reachFor = rk
                val check: suspend () -> Unit = {
                    try {
                        val r = Net.resolveRoute(sv, Store.CUR)
                        if (g == gen) {
                            reach = r.answered.isNotEmpty()
                            if (r.answered.isNotEmpty()) applyRoute(r.failover)
                        }
                    } catch (e: Exception) { if (e is kotlinx.coroutines.CancellationException) { reachBusy = false; throw e } }
                    reachBusy = false
                    lastReach = System.currentTimeMillis()
                }
                // with Failover on, the first check decides the address before Glances and the buttons are asked
                if (first && failoverReady(sv)) check() else viewModelScope.launch { check() }
            }
            // Monitoring off: no requests to Glances at all
            if (set.monitoring) {
                try {
                    val st = glances.getStats(fsMaxAge = 10000)
                    if (g == gen) update(st)
                } catch (e: Exception) {
                    if (e is kotlinx.coroutines.CancellationException) throw e
                    if (g == gen) glancesOk = false
                }
                pushDetail()
            }
            if (zimaReady) zimaTick(g)
            if (!statusBusy && (statusDirty || System.currentTimeMillis() - lastStatus > set.statusSeconds * 1000L)) {
                statusBusy = true
                statusDirty = false
                val s = set
                viewModelScope.launch {
                    try {
                        val m = Net.checkServices(Store.getServices(false), s.pingTimeout.toDouble())
                        if (g == gen) svcUp = m
                    } catch (_: Exception) {}
                    statusBusy = false
                    lastStatus = System.currentTimeMillis()
                }
            }
            delay(set.liveSeconds * 1000L)
        }
    }

    // Switches between the main and the failover address: the buttons, Glances, ZimaOS and Reboot / Shut down follow
    private fun applyRoute(failover: Boolean) {
        val fo = failover && failoverReady(saved)
        if (fo == onFailover) return
        onFailover = fo
        set = if (fo) viaFailover(saved) else saved
        glances = Glances(set)
        Zima.forget(); lastZApps = 0
        statusDirty = true
        pushServices()
        showToast(if (fo) "Main address not answering: connected over the failover address (${saved.failoverHost})" else "Connected over the main address again", 3500)
        refreshWidgets()
    }

    // ZimaOS apps and their updates: asked again once a minute
    private fun zimaTick(g: Int) {
        if (zBusy || System.currentTimeMillis() - lastZApps < 60 * 1000L) return
        viewModelScope.launch { refreshZima(g) }
    }

    private suspend fun refreshZima(g: Int = gen): String? {
        if (!zimaReady) return Zima.NEED_LOGIN
        if (zBusy) return null
        zBusy = true
        val s = set
        try {
            val a = viewModelScope.async { Zima.apps(s) }
            val u = viewModelScope.async { try { Zima.updates(s) } catch (e: Exception) { if (e is kotlinx.coroutines.CancellationException) throw e; emptySet() } }
            val apps = a.await(); val upg = u.await()
            if (g == gen) { zApps = apps; zUpg = upg }
            return null
        } catch (e: Exception) {
            if (e is kotlinx.coroutines.CancellationException) throw e
            return e.message ?: "ZimaOS didn't list its apps."
        } finally {
            zBusy = false
            lastZApps = System.currentTimeMillis()
        }
    }

    fun zAppFor(sv: Service): ZApp? = if (!zimaReady) null else zApps?.let { Zima.appFor(sv, it, serverHostOf(set)) }

    // Loads the app list now (the picker). Returns an error or null.
    suspend fun loadZApps(): String? {
        if (!isZima) return "These controls need the server type ZimaOS (Settings > Server)."
        if (!hasLogin) return Zima.NEED_LOGIN
        while (zBusy) delay(100)
        return refreshZima()
    }

    // Start / Stop / Restart / Update. Returns a message for the toast.
    suspend fun zAction(app: ZApp, action: String): String = try {
        if (action == "update") Zima.update(set, app.id) else Zima.setStatus(set, app.id, action)
        if (action != "update") delay(1500)
        statusDirty = true
        refreshZima()
        when (action) {
            "update" -> "${app.title}: update started. ZimaOS pulls the new version in the background."
            "start" -> "${app.title} started"
            "stop" -> "${app.title} stopped"
            else -> "${app.title} restarted"
        }
    } catch (e: Exception) {
        if (e is kotlinx.coroutines.CancellationException) throw e
        e.message ?: "ZimaOS didn't accept the command"
    }

    suspend fun zLogs(app: ZApp): Pair<Boolean, String> = try {
        true to Zima.logs(set, app.id, 300).ifEmpty { "(no log lines)" }
    } catch (e: Exception) {
        if (e is kotlinx.coroutines.CancellationException) throw e
        false to (e.message ?: "Couldn't load the logs")
    }

    fun addZimaApps(apps: List<ZApp>) {
        Store.addZimaApps(apps, serverHostOf(saved))
        statusDirty = true
        pushServices(); refreshLogos(); refreshWidgets()
    }

    private fun update(s: Stats) {
        stats = s
        glancesOk = true
        // a Glances answer also means the server is up
        if (reach == null) reach = true
        fun <T> List<T>.push(v: T) = (this + v).takeLast(HMAX)
        hist = Hist(hist.t.push(System.currentTimeMillis()), hist.cpu.push(s.cpu), hist.ram.push(s.ram), if (s.storage != null) hist.disk.push(s.storage.percent) else hist.disk)
    }

    private suspend fun pushDetail() {
        val key = expanded ?: return
        val st = stats ?: return
        if (!monOpen) return
        try {
            glances.getDetail(key, st)?.let { details[key] = it }
        } catch (e: Exception) { if (e is kotlinx.coroutines.CancellationException) throw e }
        if (zimaReady) try {
            if (key == "cpu") zSys = Zima.sys(set)
            if (key == "disk" && (zDrives == null || System.currentTimeMillis() - zDrivesAt > 60 * 1000L)) { zDrives = Zima.drives(set); zDrivesAt = System.currentTimeMillis() }
        } catch (e: Exception) { if (e is kotlinx.coroutines.CancellationException) throw e }
    }

    fun toggleCard(id: String) {
        expanded = if (expanded == id) null else id
        if (expanded != null) viewModelScope.launch { pushDetail() }
    }

    fun toggleMon() {
        if (monOpen && set.monitoringFixed) return
        monOpen = !monOpen
        if (monOpen && expanded != null) viewModelScope.launch { pushDetail() }
    }

    // ---------- logos ----------
    fun refreshLogos() {
        if (logoBusy || !set.useLogos) return
        logoBusy = true
        viewModelScope.launch {
            try {
                val L = layout
                if (Logos.ensure(L.all + L.groups, 8.0)) logoVer++
                pruneLogos()
            } catch (_: Exception) {}
            logoBusy = false
        }
    }

    private fun pruneLogos() {
        val keep = ArrayList<String>()
        val prev = Store.CUR
        for (sv in Store.listServers()) {
            val L = Store.getLayout(sv.id)
            keep.addAll(L.all.map { it.logo }); keep.addAll(L.groups.map { it.logo })
        }
        Store.CUR = prev
        Logos.prune(keep)
    }

    // ---------- buttons and stacks ----------
    fun saveService(editId: String?, name: String, scheme: String, addr: String, port: String, logo: String, onWidget: Boolean, addr2: String = "") {
        Store.saveService(editId, name, scheme, addr, port, logo, onWidget, addr2)
        statusDirty = true
        pushServices(); refreshLogos(); refreshWidgets()
    }

    fun deleteService(id: String) {
        Store.deleteService(id)
        statusDirty = true
        pushServices(); refreshWidgets()
    }

    fun stackAction(a: Store.StackAct) {
        Store.stackAction(a)
        statusDirty = true
        pushServices(); refreshLogos(); refreshWidgets()
    }

    // o: top-level order, g: each stack's buttons
    fun commitOrder(top: List<String>, bodies: Map<String, List<String>>) {
        val data = Store.loadData()
        data.order = top.toMutableList()
        for (g in data.groups) bodies[g.id]?.let { g.members = it.toMutableList() }
        Store.saveData(data)
        pushServices(); refreshWidgets()
    }

    fun commitWidget(shown: List<String>, hidden: List<String>) {
        val data = Store.loadData()
        data.widgetOrder = shown.toMutableList()
        data.widgetHidden = hidden.toMutableList()
        Store.saveData(data)
        pushServices(); refreshWidgets()
    }

    fun rename(t: String) {
        val v = t.trim().take(20)
        if (v.isEmpty()) return
        val data = Store.loadData()
        data.title = v
        Store.saveData(data)
        title = v
        servers = Store.listServers()
        refreshWidgets()
    }

    // ---------- servers ----------
    private fun showServer(dir: Int) {
        gen++
        stats = null; glancesOk = null; reach = null; hist = Hist(); details.clear(); expanded = null
        svcUp = emptyMap(); openG.clear(); mode = "dash"
        zApps = null; zUpg = emptySet(); zSys = null; zDrives = null; lastZApps = 0; Zima.forgetSys()
        statusDirty = true; lastStatus = 0; reachFor = ""
        switchDir = dir; switchSeq++
        refreshAll()
        monOpen = set.monitoring && (set.monitoringOpen || set.monitoringFixed)
        refreshLogos()
    }

    fun switchServer(id: String, dir: Int) {
        if (id == Store.CUR) return
        if (Store.setCurrentServer(id)) showServer(dir)
    }

    // Add server: the login (if given) is checked before the server is added. Returns an error or null.
    // failoverHost: the failover server address ("" = Failover off)
    suspend fun addServer(name: String, platform: String, host: String, scheme: String, port: String, u: String, p: String, failoverHost: String = ""): String? {
        val raw = saved.toJson().put("serverHost", host).put("serverScheme", scheme).put("serverPort", port).put("platform", platform)
            .put("failover", failoverHost.isNotEmpty()).put("failoverHost", failoverHost)
            .put("proxmoxNode", "").put("baseUrl", "").put("username", "").put("diskMounts", org.json.JSONArray()).put("networkInterface", "")
        for (k in PLATFORM_URL_KEYS) raw.put(k, "")
        val own = cleanSettings(raw)
        val withLogin = Power.hasDriver(own.platform) && u.isNotEmpty() && p.isNotEmpty()
        if (withLogin) {
            var r = Power.driver(own.platform, null, u, p, platformBase(own.platform, own), own)
            // the main address can't be reached: check the login over the failover address
            if (!r.ok && r.net && failoverReady(own)) {
                val fo = viaFailover(own)
                val r2 = Power.driver(own.platform, null, u, p, platformBase(own.platform, fo), fo)
                if (r2.ok || !r2.net) r = r2
            }
            if (!r.ok) return r.msg.ifEmpty { "The login didn't work." } + if (r.net) " Check the address, or add the server without the login and enter it later." else ""
        }
        val before = servers.map { it.id }
        val id = Store.addServer(name, JSONObject().put("serverHost", own.serverHost).put("serverScheme", own.serverScheme).put("serverPort", own.serverPort).put("platform", own.platform)
            .put("failover", own.failover).put("failoverHost", own.failoverHost))
        if (withLogin) Secrets.saveLogin(own.platform, u, p, id)
        showServer(if (id in before) 0 else 1)
        if (own.platform != "linux") showToast(if (withLogin) "Server added, login checked and saved" else "Server added", 3000)
        refreshWidgets()
        return null
    }

    fun removeServer(id: String) {
        if (Store.removeServer(id)) {
            showServer(-1)
            showToast("Server deleted", 2500)
            refreshWidgets()
        }
    }

    // ---------- Reboot / shut down ----------
    suspend fun power(a: String, u: String, p: String): Power.HandleResult {
        val r = try { Power.handlePower(a, u, p, set) } catch (e: Exception) {
            Power.HandleResult(PowerResult(false, "Something went wrong: ${e.message ?: e}"), a, Secrets.hasLogin(set.platform))
        }
        hasLogin = r.hasLogin
        return r
    }

    // ---------- Settings sheet ----------
    suspend fun settingsLoad(): SettingsInfo {
        var mounts: List<Mount>? = null
        var ifaces: List<String>? = null
        if (set.monitoring && serverHostOf(set).isNotEmpty()) {
            val m = viewModelScope.async { glances.mounts() }
            val i = viewModelScope.async { glances.ifaces() }
            mounts = m.await()?.map { (k, size, used) -> Mount(k.substringBefore("|"), k.substringAfter("|"), size, used) }
            ifaces = i.await()
        }
        return SettingsInfo(saved, serverHostOf(saved), serverHostOf(saved).isEmpty(), mounts, ifaces,
            Store.loadData().hidden, PLATFORM_KEYS.associateWith { Secrets.hasLogin(it) }, Secrets.has(Secrets.gpassKey()))
    }

    fun settingsSave(form: JSONObject, newTitle: String, gpass: String, clearGpass: Boolean) {
        val data = Store.loadData()
        data.settings = cleanSettings(form).toJson()
        if (newTitle.trim().isNotEmpty()) data.title = newTitle.trim().take(20)
        Store.saveData(data)
        if (clearGpass) Secrets.remove(Secrets.gpassKey())
        if (gpass.isNotEmpty()) Secrets.set(Secrets.gpassKey(), gpass)
        statusDirty = true
        refreshAll()
        if (!isZima) { zApps = null; zUpg = emptySet(); zSys = null; zDrives = null }
        Zima.forget(); lastZApps = 0
        if (set.monitoringFixed && set.monitoring) monOpen = true
        if (!set.monitoring) monOpen = false
        refreshLogos(); refreshWidgets()
        BoardWidget.schedule(ctx, set.refreshMinutes)
    }

    // tests the values in the form, saved or not
    suspend fun settingsTest(form: JSONObject, gpass: String): List<Pair<Boolean, String>> {
        var s = cleanSettings(form)
        val lines = ArrayList<Pair<Boolean, String>>()
        if (serverHostOf(s).isEmpty()) return listOf(false to "Enter your server's IP or name first, e.g. 192.168.1.10")
        val reachR = Net.serverReachable(s, Store.getServices(false, via = false))
        lines.add(if (reachR.isNotEmpty()) true to "Main server address: reachable ($reachR answered)"
        else false to "Main server address: nothing answered at ${serverHostOf(s)} (check the address, and your VPN if you're away from home)")
        // Failover: the failover address is tested too; with the main address down, the other tests use it
        if (s.failover && s.failoverHost.isEmpty()) lines.add(false to "Failover: enter the failover server address, or switch Failover off")
        else if (failoverReady(s)) {
            val fo = viaFailover(s)
            val foR = Net.serverReachable(fo, Store.getServices(false, via = true))
            lines.add(if (foR.isNotEmpty()) true to "Failover server address: reachable ($foR answered)"
            else false to "Failover server address: nothing answered at ${s.failoverHost} (fine if you're away from that network)")
            if (reachR.isEmpty() && foR.isNotEmpty()) s = fo
        }
        if (s.monitoring) {
            val t0 = System.currentTimeMillis()
            val g = Glances(s)
            val pass = gpass.ifEmpty { null }
            try {
                val q = g.request("quicklook", pass) as? JSONObject
                val cpuName = q?.optString("cpu_name").orEmpty()
                lines.add(true to "Glances: OK (${System.currentTimeMillis() - t0} ms${if (cpuName.isNotEmpty()) ", $cpuName" else ""})")
            } catch (e: Exception) {
                val other = if (s.apiVersion == 4) 3 else 4
                val otherOk = try { g.request("quicklook", pass, s.copy(apiVersion = other)); true } catch (_: Exception) { false }
                lines.add(if (otherOk) false to "Glances: answers with API version $other, not ${s.apiVersion}. Pick $other under GLANCES API VERSION."
                else false to "Glances: no answer at ${glancesUrl(s)} (check the address and that Glances runs with -w, or turn Monitoring off)")
            }
        }
        lines.add(Power.testPower(s))
        return lines
    }

    // Settings > Server > Enter login: check the login against the server, then save it. Returns (ok, message)
    suspend fun settingsLogin(P: String, u: String, p: String, form: JSONObject): Pair<Boolean, String> {
        if (!Power.hasDriver(P)) return false to "This server type doesn't use a login."
        val user = u.trim()
        if (user.isEmpty() || p.isEmpty()) return false to "Enter a username and a password."
        val s0 = cleanSettings(form)
        // checked over the address the server is reached over right now (main or failover)
        val s = if (onFailover && failoverReady(s0) && serverHostOf(s0).equals(serverHostOf(saved), true)) viaFailover(s0) else s0
        val base = platformBase(P, s)
        val r = Power.driver(P, null, user, p, base, s)
        if (!r.ok) {
            val why = r.msg.ifEmpty { "no answer at $base" }
            return false to if (r.net) "$why. The login wasn't saved: try again when the server can be reached." else why
        }
        Secrets.saveLogin(P, user, p)
        if (P == "zimaos") { Zima.forget(); lastZApps = 0 }
        if (P == set.platform) hasLogin = true
        val extra = Regex("\\(([^)]*)\\)\\s*$").find(r.msg)?.groupValues?.get(1)
        return true to "${PLATFORMS[P]!!.login} login checked and saved" + if (extra != null) " ($extra)" else ""
    }

    fun settingsForget(P: String): String {
        Secrets.forgetLogin(P)
        if (P == "zimaos") { Zima.forget(); zApps = null; zUpg = emptySet() }
        if (P == set.platform) hasLogin = false
        return if (P == "linux") "Helper key forgotten" else PLATFORMS[P]!!.login + " login forgotten"
    }

    // Linux power helper: keep the saved key (or make one) and copy the setup command
    fun settingsHelper(form: JSONObject): String {
        val s = cleanSettings(form)
        if (!Secrets.hasLogin("linux")) {
            val a = ByteArray(24)
            SecureRandom().nextBytes(a)
            Secrets.set(Secrets.helperKey(), a.joinToString("") { "%02x".format(it) })
        }
        if (set.platform == "linux") hasLogin = true
        copy(Power.helperSetupScript(ctx, Secrets.get(Secrets.helperKey())!!, Power.helperPort(s)))
        return "Setup command copied. Paste it into an SSH session on the server."
    }

    fun settingsRestore(id: String?): String {
        val data = Store.loadData()
        data.hidden = if (id != null) data.hidden.filter { it != id }.toMutableList() else mutableListOf()
        Store.saveData(data)
        statusDirty = true
        pushServices()
        return if (id != null) "Button restored" else "All buttons restored"
    }

    fun settingsClearLogos(): String {
        Logos.clearAll()
        logoVer++
        refreshLogos()
        return "Logo cache cleared. Logos download again now."
    }

    fun settingsExport(): String {
        val o = JSONObject().put("board", 2).put("exported", Instant.now().toString()).put("data", Store.readStore())
        copy(o.toString(2))
        return "All servers' settings copied to the clipboard. Passwords are not included."
    }

    // Returns (ok, message)
    fun settingsImport(): Pair<Boolean, String> {
        val cm = ctx.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
        val text = cm.primaryClip?.takeIf { it.itemCount > 0 }?.getItemAt(0)?.coerceToText(ctx)?.toString().orEmpty()
        val obj = try { JSONObject(text) } catch (e: Exception) { return false to "The clipboard doesn't contain exported settings." }
        val imp = if (obj.has("board") && obj.opt("data") is JSONObject) obj.getJSONObject("data") else return false to "The clipboard doesn't contain exported settings."
        if (imp.optJSONArray("servers") != null) {
            // every server (an export from this version, or from the Scriptable app): replaces all of them
            Store.importRaw(imp.toString())
        } else {
            // an export from before there were several servers: goes into the server shown now
            val data = Store.loadData()
            val merged = data.settings
            imp.optJSONObject("settings")?.let { s -> for (k in s.keys()) merged.put(k, s.get(k)) }
            val st = Store.readStore()
            val servers = st.getJSONArray("servers")
            for (i in 0 until servers.length()) {
                val sv = servers.getJSONObject(i)
                if (sv.getString("id") == Store.CUR) {
                    if (imp.opt("title") is String) sv.put("title", imp.getString("title"))
                    for (k in SERVER_PARTS) if (imp.has(k)) sv.put(k, imp.get(k))
                }
            }
            Store.writeStore(st)
            data.settings = merged
            val again = Store.loadData()
            again.settings = merged
            Store.saveData(again)
        }
        showServer(0)
        refreshWidgets()
        return true to "Settings imported"
    }

    fun settingsReset(): String {
        val data = Store.loadData()
        data.order = mutableListOf(); data.widgetOrder = mutableListOf(); data.widgetHidden = mutableListOf(); data.groups = mutableListOf()
        Store.saveData(data)
        pushServices(); refreshWidgets()
        return "Layout reset. Your own buttons are kept."
    }

    private fun copy(t: String) {
        val cm = ctx.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
        cm.setPrimaryClip(ClipData.newPlainText("-board", t))
    }
}
