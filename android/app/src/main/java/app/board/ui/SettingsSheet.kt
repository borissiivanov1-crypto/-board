package app.board.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.board.ACCENTS
import app.board.BoardVM
import app.board.PLATFORMS
import app.board.SettingsInfo
import app.board.THEMES
import app.board.TEMP_DEFAULTS
import app.board.cleanSettings
import app.board.formatBytes
import kotlinx.coroutines.launch
import org.json.JSONArray
import org.json.JSONObject

private val LOOKK = listOf("theme", "accent", "glow", "movingBg", "cardStyle", "corners", "font", "buttonSize", "style")

@OptIn(ExperimentalLayoutApi::class)
@Composable
fun SettingsSheet(vm: BoardVM, onClose: () -> Unit, onDelete: (String) -> Unit) {
    val scope = rememberCoroutineScope()
    var info by remember { mutableStateOf<SettingsInfo?>(null) }
    val f = remember { JSONObject(vm.saved.toJson().toString()) }
    var ver by remember { mutableIntStateOf(0) }
    var title by remember { mutableStateOf(vm.title) }
    var gpass by remember { mutableStateOf("") }
    var clearGpass by remember { mutableStateOf(false) }
    var err by remember { mutableStateOf("") }
    var busy by remember { mutableStateOf(false) }
    var testLines by remember { mutableStateOf<List<Pair<Boolean, String>>?>(null) }
    var loginOpen by remember { mutableStateOf<String?>(null) }
    var lu by remember { mutableStateOf("") }
    var lp by remember { mutableStateOf("") }
    var lerr by remember { mutableStateOf("") }
    val logins = remember { mutableStateOf(mapOf<String, Boolean>()) }

    LaunchedEffect(Unit) {
        val r = vm.settingsLoad()
        info = r
        logins.value = r.logins
    }

    fun close() { vm.lookPreview = null; onClose() }
    fun touch(key: String) {
        ver++
        if (key in LOOKK) vm.lookPreview = cleanSettings(f)
    }
    fun str(k: String) = f.opt(k)?.let { if (it == JSONObject.NULL) "" else it.toString() } ?: ""
    fun bool(k: String) = f.optBoolean(k)
    fun msg(t: String) { err = ""; vm.showToast(t, 3000) }
    fun run(block: suspend () -> Unit) { if (busy) return; busy = true; scope.launch { try { block() } finally { busy = false } } }

    Modal(::close, wide = true, top = true) {
        // header: Cancel, title (with the server's name), Save
        Row(verticalAlignment = Alignment.CenterVertically) {
            SmallBtn("Cancel") { close() }
            Column(Modifier.weight(1f), horizontalAlignment = Alignment.CenterHorizontally) {
                T("SETTINGS", 17.sp, FontWeight.ExtraBold, spacing = 1.sp)
                Caps(vm.title, 10.sp, .55f)
            }
            if (info != null) SmallBtn("Save", true) {
                if (busy) return@SmallBtn
                val n = { k: String -> Regex("^\\d+$").matches(str(k)) }
                if (listOf("maxDisks", "liveSeconds", "refreshMinutes", "statusSeconds", "pingTimeout").any { !n(it) }) { err = "Use whole numbers for the number fields"; return@SmallBtn }
                val tl = f.optJSONObject("tempLimits") ?: JSONObject()
                for (k in TEMP_DEFAULTS.keys) {
                    val t = tl.optJSONObject(k) ?: JSONObject()
                    val w = t.opt("warm")?.toString() ?: ""; val h = t.opt("hot")?.toString() ?: ""
                    if (!Regex("^\\d+$").matches(w) || !Regex("^\\d+$").matches(h)) { err = "Use whole numbers for the temperatures"; return@SmallBtn }
                    if (h.toInt() <= w.toInt()) { err = "Red has to start above orange (${k.uppercase()})"; return@SmallBtn }
                }
                if (str("serverHost").trim().isEmpty()) { err = "Enter your server's IP or name (Settings > Server)"; return@SmallBtn }
                if (bool("failover") && str("failoverHost").trim().isEmpty()) { err = "Enter the failover server address, or switch Failover off"; return@SmallBtn }
                if (bool("failover") && str("failoverHost").trim().equals(str("serverHost").trim(), true)) { err = "The failover server address has to be different from the main server address"; return@SmallBtn }
                if (str("baseUrl").isNotEmpty() && !Regex("^https?://[^/:]+", RegexOption.IGNORE_CASE).containsMatchIn(str("baseUrl"))) { err = "The Glances address must start with http:// or https://"; return@SmallBtn }
                vm.settingsSave(f, title, gpass, clearGpass)
                vm.lookPreview = null
                vm.showToast("Settings saved", 2500)
                onClose()
            }
        }
        ErrText(err)
        val I = info
        if (I == null) {
            T("Loading...", 13.sp, alpha = .6f, align = TextAlign.Center, modifier = Modifier.fillMaxWidth().padding(vertical = 18.dp))
            return@Modal
        }
        // the form is a plain JSON object (as in the Scriptable version): a toggle or pick redraws the sheet
        androidx.compose.runtime.key(ver) {
        val first = ver == 0
        Column(verticalArrangement = Arrangement.spacedBy(20.dp)) {
            if (I.firstRun) Note("Welcome to -board. Enter your server's IP or name below, pick its server type, tap Test connection, then Save. After that, add your services with the + button.")
            Divider("THIS SERVER")

            // ---------- SERVER ----------
            val P = str("platform")
            Sec("SERVER", 0, first) {
                Labeled("SERVER NAME") { Field(title, { title = it }, "e.g. NAS", maxLength = 20) }
                Labeled("SERVER TYPE") { Seg(typeOpts(), P, wrap = true) { f.put("platform", it); touch("platform") } }
                Hint(TYPE_HINT[P] ?: TYPE_HINT["none"]!!)
                val fo = bool("failover")
                androidx.compose.runtime.key(P) {
                    ServerAddrField(if (fo) "MAIN SERVER ADDRESS" else "SERVER ADDRESS", P, str("serverHost"), str("serverScheme"), str("serverPort")) { h, pt, sc ->
                        f.put("serverHost", h); f.put("serverPort", pt); f.put("serverScheme", sc)
                    }
                }
                Hint(addrHint(P))
                ToggleSetting(f, "failover", "FAILOVER ADDRESS (OPTIONAL)") { touch(it) }
                if (fo) {
                    FailoverAddrField(str("failoverHost")) { f.put("failoverHost", it) }
                    Hint(FAILOVER_HINT)
                } else Hint("On: a second address for this server, used when the main server address doesn't answer.")
                val pl = PLATFORMS[P]
                // an older separate address for the server system on another host (still used until it's cleared)
                if (pl != null && str(pl.url).isNotEmpty()) {
                    SRow("Reboot / Shut down use ${str(pl.url)}", "A separate address from an older version of -board") {
                        Chip("Use the server address") { f.put(pl.url, ""); touch(pl.url) }
                    }
                }
                if (P == "proxmox") {
                    TextSetting(f, "proxmoxNode", "NODE NAME", "Automatic (e.g. pve)") { touch(it) }
                    Hint("Only needed in a cluster: the node to reboot, as shown in the Proxmox web UI.")
                } else if (P == "linux") {
                    Chip(if (logins.value["linux"] == true) "Copy setup command" else "Create key & copy setup command") {
                        msg(vm.settingsHelper(f)); logins.value = logins.value + ("linux" to true)
                    }
                    Hint("Then SSH into the server (ssh you@${str("serverHost").ifEmpty { I.host.ifEmpty { "192.168.1.10" } }}), paste and press Enter. It asks for your sudo password once, installs the helper as a system service and opens its port in ufw / firewalld if they're on. Run it again after changing the address or port.")
                }
                if (pl != null && P != "linux") {
                    val LN = pl.login!!
                    val openLogin = { loginOpen = P; lu = ""; lp = ""; lerr = "" }
                    if (logins.value[P] == true) {
                        SRow("$LN login saved", "Stored encrypted on this phone") {
                            Chip("Change") { openLogin() }
                            Spacer(Modifier.width(6.dp))
                            Chip("Forget") { msg(vm.settingsForget(P)); logins.value = logins.value + (P to false) }
                        }
                    } else {
                        SRow("No $LN login saved", "Enter it now, or you'll be asked the first time you use Reboot / Shut down") {
                            Chip("Enter login") { openLogin() }
                        }
                    }
                    if (loginOpen == P) {
                        val sh = RoundedCornerShape(14.dp)
                        Column(Modifier.fillMaxWidth().clip(sh).background(Color.White.copy(alpha = .04f)).border(1.dp, Color.White.copy(alpha = .1f), sh).padding(12.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                            Labeled("${LN.uppercase()} USERNAME") { Field(lu, { lu = it }, pl.userPh) }
                            Labeled("${LN.uppercase()} PASSWORD") { Field(lp, { lp = it }, password = true) }
                            if (lerr.isNotEmpty()) ResLine(false, lerr)
                            Row {
                                Chip("Cancel") { loginOpen = null; lp = "" }
                                Spacer(Modifier.width(8.dp))
                                Chip(if (busy) "Checking..." else "Check & save login") {
                                    if (lu.trim().isEmpty() || lp.isEmpty()) { lerr = "Enter a username and a password."; return@Chip }
                                    lerr = ""
                                    run {
                                        val (ok, m) = vm.settingsLogin(P, lu, lp, f)
                                        if (!ok) lerr = m else { logins.value = logins.value + (P to true); loginOpen = null; lp = ""; vm.showToast(m, 3500) }
                                    }
                                }
                            }
                            Hint("Checked against the server at the address above (even before you save Settings), then stored encrypted on this phone." + if (P == "proxmox") " Typing root means root@pam." else "")
                        }
                    }
                } else if (P == "linux" && logins.value["linux"] == true) {
                    SRow("Helper key saved", "Stored encrypted on this phone") {
                        Chip("Forget") { msg(vm.settingsForget("linux")); logins.value = logins.value + ("linux" to false) }
                    }
                }
                ToggleSetting(f, "showPower", "SHOW REBOOT / SHUT DOWN") { touch(it) }
                Hint("Off hides the arrow and menu next to the server name.")
                Chip(if (busy && testLines == listOf(true to "Testing...")) "Testing..." else "Test connection") {
                    testLines = listOf(true to "Testing...")
                    run { testLines = vm.settingsTest(f, gpass) }
                }
                testLines?.let { lines -> Column(verticalArrangement = Arrangement.spacedBy(6.dp)) { for ((ok, t) in lines) if (t == "Testing...") T(t, 12.sp) else ResLine(ok, t) } }
                Hint("Tests the server, Glances (below) and Reboot / Shut down with the values in this form, even before you save them.")
            }

            // ---------- MONITORING ----------
            Sec("MONITORING", 1, first) {
                ToggleSetting(f, "monitoring", "MONITORING (GLANCES)") { touch(it) }
                val mon = bool("monitoring")
                Hint(if (mon) "CPU, RAM, storage and network from Glances, in a fold-away panel on the dashboard and as gauges on the widget."
                else "Off: no Monitoring panel, no gauges on the widget (it shows your service buttons instead), and -board sends no requests to Glances. The LIVE / OFFLINE status still works.")
                if (mon) {
                    val gh = str("serverHost").ifEmpty { I.host }
                    GlancesUrlField(f, gh)
                    Hint("Leave empty for ${if (gh.isNotEmpty()) "http://$gh:61208" else "the server address on port 61208"}. Otherwise pick http or https and type the IP or name; port 61208 is added for IP addresses and local names unless you type another. A public domain (behind a reverse proxy) gets no port added.")
                    Labeled("GLANCES API VERSION") { Select(listOf("4" to "4 (Glances 4.x)", "3" to "3 (Glances 3.x)"), str("apiVersion")) { f.put("apiVersion", it.toInt()); touch("apiVersion") } }
                    TextSetting(f, "username", "GLANCES USERNAME", "Only if Glances has a password") { touch(it) }
                    Labeled("GLANCES PASSWORD") { Field(gpass, { gpass = it }, if (I.hasGpass) "Saved. Leave empty to keep it" else "Optional", password = true) }
                    if (I.hasGpass) Chip(if (clearGpass) "Will be removed when you save (tap to undo)" else "Remove saved Glances password") { clearGpass = !clearGpass }
                    Labeled("DRIVES IN THE STORAGE CARD") {
                        val chosen = (f.optJSONArray("diskMounts") ?: JSONArray()).let { a -> (0 until a.length()).map { a.optString(it) } }
                        if (!I.mounts.isNullOrEmpty()) {
                            val shown = I.mounts.map { Triple(it.mnt, it, false) } + chosen.filter { m -> I.mounts.none { it.mnt == m } }.map { Triple(it, null, true) }
                            for ((mnt, m, missing) in shown) {
                                val sub = if (missing) "Not found in Glances right now" else formatBytes(m!!.size) + (if (m.dev.isNotEmpty()) " · ${m.dev}" else "") + (if (m.size > 0) " · ${Math.round(m.used / m.size * 100)}% used" else "")
                                SRow(mnt, sub) {
                                    Switch(mnt in chosen) { on ->
                                        val l = chosen.filter { it != mnt }.toMutableList()
                                        if (on) l.add(mnt)
                                        f.put("diskMounts", JSONArray(l)); touch("diskMounts")
                                    }
                                }
                            }
                        } else {
                            var txt by remember { mutableStateOf(chosen.joinToString(", ")) }
                            Field(txt, { txt = it; f.put("diskMounts", JSONArray(it.split(",").map { x -> x.trim() }.filter { x -> x.isNotEmpty() })) }, "/, /mnt/data")
                        }
                    }
                    Hint(if (!I.mounts.isNullOrEmpty()) "None picked = pick automatically. Paths are as the Glances container sees them." else "Couldn't reach Glances to list the drives, so type the paths, separated by commas. Empty = pick automatically.")
                    NumSetting(f, "maxDisks", "MAX DRIVES WHEN PICKED AUTOMATICALLY", "3")
                    val ifo = mutableListOf("" to "Automatic (the busiest)")
                    I.ifaces?.forEach { ifo.add(it to it) }
                    if (str("networkInterface").isNotEmpty() && ifo.none { it.first == str("networkInterface") }) ifo.add(str("networkInterface") to str("networkInterface"))
                    Labeled("NETWORK INTERFACE") { Select(ifo, str("networkInterface")) { f.put("networkInterface", it); touch("networkInterface") } }
                    if (I.ifaces == null) Hint("Couldn't reach Glances to list the interfaces.")
                    ToggleSetting(f, "monitoringOpen", "OPEN MONITORING ON START") { touch(it) }
                    ToggleSetting(f, "monitoringFixed", "KEEP MONITORING ALWAYS OPEN") { touch(it) }
                    Hint("Removes the Monitoring button, so the cards and network speed always show.")
                    NumSetting(f, "liveSeconds", "DASHBOARD UPDATES EVERY (SECONDS)", "3")
                    Hint("Lower = smoother graphs, more battery while the dashboard is open.")
                    Labeled("TEMPERATURE COLOURS (°C)") {
                        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                            Spacer(Modifier.width(60.dp))
                            Caps("ORANGE FROM", 10.sp, .5f, Modifier.weight(1f), .12f)
                            Caps("RED FROM", 10.sp, .5f, Modifier.weight(1f), .12f)
                        }
                        val tl = f.optJSONObject("tempLimits") ?: JSONObject().also { f.put("tempLimits", it) }
                        for ((k, label) in listOf("cpu" to "CPU", "nvme" to "NVMe", "hdd" to "HDD")) {
                            val row = tl.optJSONObject(k) ?: JSONObject().also { tl.put(k, it) }
                            Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                                T(label, 13.sp, FontWeight.Bold, alpha = .8f, modifier = Modifier.width(60.dp).padding(start = 4.dp))
                                for (w in listOf("warm", "hot")) {
                                    var v by remember { mutableStateOf(row.opt(w)?.toString() ?: "") }
                                    Field(v, { v = it.trim(); row.put(w, v) }, keyboard = KeyboardType.Number, height = 40.dp, modifier = Modifier.weight(1f))
                                }
                            }
                        }
                    }
                }
            }

            Divider("SHARED BY ALL SERVERS")

            // ---------- WIDGET ----------
            Sec("WIDGET", 2, first) {
                Hint("Each widget shows one server: when you add it (or long-press it and pick Reconfigure) you choose the server. The first server is the default.")
                NumSetting(f, "refreshMinutes", "REFRESH EVERY (MINUTES)", "5")
                Hint("Android refreshes widgets at most every 15 minutes in the background, so smaller values count as 15. Opening -board refreshes them too.")
                TextSetting(f, "linkUrl", "CORNER BUTTON LINK", "https://... (empty = no button)", KeyboardType.Uri) { touch(it) }
                Hint("A small button in the widget corner, e.g. your ZimaOS or Proxmox page.")
                ToggleSetting(f, "widgetStatusDots", "STATUS DOTS ON THE WIDGET") { touch(it) }
                Hint("Each widget refresh checks every button, which takes a couple of seconds.")
                TextSetting(f, "widgetTitle", "WIDGET TITLE", "Same as the dashboard name") { touch(it) }
                Labeled("BUTTON ROWS") { Select((1..6).map { "$it" to "$it row${if (it > 1) "s" else ""} (${it * 4} buttons)" }, str("widgetRows")) { f.put("widgetRows", it.toInt()); touch("widgetRows") } }
            }

            // ---------- BUTTONS ----------
            Sec("BUTTONS", 3, first) {
                ToggleSetting(f, "useLogos", "SHOW LOGOS") { touch(it) }
                Hint("Off shows the emoji instead.")
                Labeled("DASHBOARD COLUMNS") { Select(listOf("2" to "2 columns", "3" to "3 columns"), str("columns")) { f.put("columns", it.toInt()); touch("columns") } }
                NumSetting(f, "statusSeconds", "CHECK UP/DOWN EVERY (SECONDS)", "30")
                NumSetting(f, "pingTimeout", "COUNT AS DOWN AFTER (SECONDS)", "3")
                Hint("How long a service may take to answer. The widget waits at most 3 seconds.")
                if (I.hidden.isNotEmpty()) Labeled("DELETED BUTTONS") {
                    for (h in I.hidden) SRow(h.removePrefix("b:")) { Chip("Restore") { msg(vm.settingsRestore(h)); info = I.copy(hidden = I.hidden - h) } }
                }
            }

            // ---------- THEME & LOOK ----------
            Sec("THEME & LOOK", 4, first) {
                Labeled("STYLE") { Seg(listOf("classic" to "Classic", "glass" to "Liquid Glass"), str("style")) { f.put("style", it); touch("style") } }
                Hint(if (str("style") == "glass") "Liquid Glass: iOS 26 style glass cards with a bright rim, a third background glow, springy animation on every press, menu and sheet, and glass rings on the widget."
                else "Classic: the plain -board look. Switch to Liquid Glass for glass cards and animations.")
                ToggleSetting(f, "splash", "STARTUP ANIMATION") { touch(it) }
                Hint("A short animated -board logo when the dashboard opens (tap it to skip). Works with either style.")
                Swatches("THEME", THEMES.map { (k, t) ->
                    Swatch(k, t.name, if (t.noGlow) Brush.linearGradient(listOf(Color.Black, Color.Black)) else Brush.radialGradient(listOf(rgb(t.g1), rgb(t.g1, 0f)), Offset(30f, 28f), 70f), hex(t.bg), Brush.radialGradient(listOf(rgb(t.g2), rgb(t.g2, 0f)), Offset(90f, 100f), 80f))
                }, str("theme")) { f.put("theme", it); touch("theme") }
                val ta = hex((THEMES[str("theme")] ?: THEMES["aurora"]!!).acc)
                Swatches("ACCENT COLOUR", ACCENTS.map { (k, a) ->
                    val c = if (a.c != null) hex(a.c) else ta
                    Swatch(k, if (a.c != null) a.name else "Same as theme", Brush.linearGradient(listOf(c, c)), c, null, dashed = a.c == null)
                }, str("accent")) { f.put("accent", it); touch("accent") }
                Hint("Used for switches, Save buttons and the selected tab. Status colours (green / orange / red) stay the same.")
                Labeled("BACKGROUND GLOW") { Select(listOf("off" to "Off", "soft" to "Soft", "normal" to "Normal", "strong" to "Strong"), str("glow")) { f.put("glow", it); touch("glow") } }
                ToggleSetting(f, "movingBg", "MOVING BACKGROUND") { touch(it) }
                Hint("Off keeps the glow still (saves a little battery).")
                Labeled("CARD STYLE") { Select(listOf("subtle" to "Subtle glass", "normal" to "Normal glass", "strong" to "Strong glass"), str("cardStyle")) { f.put("cardStyle", it); touch("cardStyle") } }
                Labeled("CORNERS") { Select(listOf("round" to "Rounded", "sharp" to "Sharper"), str("corners")) { f.put("corners", it); touch("corners") } }
                Labeled("FONT") { Select(listOf("system" to "System", "rounded" to "Sans", "serif" to "Serif"), str("font")) { f.put("font", it); touch("font") } }
                Labeled("BUTTON SIZE") { Select(listOf("normal" to "Normal", "compact" to "Compact"), str("buttonSize")) { f.put("buttonSize", it); touch("buttonSize") } }
                Hint("Changes show on the dashboard behind this sheet as you pick them. Cancel puts the old look back. The widget background follows the theme and glow.")
            }

            // ---------- DATA ----------
            Sec("DATA", 5, first) {
                Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    Btn("Clear logo cache", modifier = Modifier.fillMaxWidth()) { msg(vm.settingsClearLogos()) }
                    Btn("Export settings", modifier = Modifier.fillMaxWidth()) { msg(vm.settingsExport()) }
                    var armI by remember { mutableStateOf(false) }
                    Btn(if (armI) "Tap again to replace everything" else "Import settings", modifier = Modifier.fillMaxWidth()) {
                        if (!armI) { armI = true; return@Btn }
                        armI = false
                        val (ok, m) = vm.settingsImport()
                        if (!ok) err = m else { msg(m); close() }
                    }
                    var armR by remember { mutableStateOf(false) }
                    Btn(if (armR) "Tap again to reset the layout" else "Reset layout", BtnKind.RedText, Modifier.fillMaxWidth()) {
                        if (!armR) { armR = true; return@Btn }
                        armR = false
                        msg(vm.settingsReset())
                    }
                }
                Hint("Export copies all settings, buttons, stacks and layout to the clipboard (passwords stay on the phone). Import reads them back from the clipboard, including an export from the iPhone (Scriptable) version. Reset layout clears the button order and stacks but keeps your own buttons.")
            }

            // ---------- DELETE SERVER ----------
            Sec("DELETE SERVER", 6, first) {
                val only = vm.servers.size < 2
                Btn("Delete ${vm.title}", BtnKind.RedText, Modifier.fillMaxWidth(), enabled = !only) { onDelete(vm.title) }
                Hint(if (only) "This is your only server, so it can't be deleted. Add another server first (swipe the server name left)."
                else "Deletes this server's settings, logins, buttons, stacks and layout from the phone. Your other servers stay. Asks to confirm.")
            }
        }
        }
    }
}

@Composable
private fun SmallBtn(text: String, ok: Boolean = false, onClick: () -> Unit) {
    val look = LocalLook.current
    val src = rememberSource()
    val sh = RoundedCornerShape(12.dp)
    Box(
        Modifier.height(36.dp).pressScale(look, src).clip(sh)
            .then(if (ok) Modifier.background(look.acc) else if (look.liquid) Modifier.glass(look, sh, 1.6f) else Modifier.background(Color.White.copy(alpha = .12f)))
            .clickable(src, null) { onClick() }.padding(horizontal = 14.dp),
        contentAlignment = Alignment.Center,
    ) { T(text, 15.sp, FontWeight.Bold, if (ok) look.ink else Color.White) }
}

@Composable
private fun Divider(t: String) = T(t, 11.sp, FontWeight.ExtraBold, alpha = .5f, spacing = 1.8.sp, align = TextAlign.Center, modifier = Modifier.fillMaxWidth().padding(top = 6.dp))

@Composable
private fun Sec(title: String, index: Int, animate: Boolean, content: @Composable ColumnScope.() -> Unit) {
    val look = LocalLook.current
    Column(if (animate) Modifier.enter(index) else Modifier, verticalArrangement = Arrangement.spacedBy(10.dp)) {
        Caps(title, 12.sp, .6f, Modifier.padding(horizontal = 4.dp))
        val sh = RoundedCornerShape(if (look.sharp) 10.dp else 18.dp)
        Column(
            Modifier.fillMaxWidth().then(if (look.liquid) Modifier.glass(look, sh, .8f) else Modifier.background(Color.White.copy(alpha = .05f), sh).border(1.dp, Color.White.copy(alpha = .08f), sh)).padding(14.dp),
            verticalArrangement = Arrangement.spacedBy(14.dp),
            content = content,
        )
    }
}

@Composable
private fun SRow(title: String, sub: String? = null, trailing: @Composable () -> Unit = {}) {
    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
        Column(Modifier.weight(1f)) {
            T(title, 14.sp, FontWeight.Bold, maxLines = 1)
            if (sub != null) T(sub, 11.sp, alpha = .5f, modifier = Modifier.padding(top = 2.dp))
        }
        Spacer(Modifier.width(10.dp))
        trailing()
    }
}

@Composable
private fun ResLine(ok: Boolean, t: String) {
    Row(verticalAlignment = Alignment.Top) {
        Box(Modifier.padding(top = 4.dp).size(8.dp).clip(CircleShape).background(if (ok) GREEN else RED))
        Spacer(Modifier.width(8.dp))
        T(t, 12.sp, lineHeight = 16.sp)
    }
}

@Composable
private fun TextSetting(f: JSONObject, key: String, label: String, ph: String, kb: KeyboardType = KeyboardType.Text, onTouch: (String) -> Unit) {
    var v by remember { mutableStateOf(f.optString(key)) }
    Labeled(label) { Field(v, { v = it; f.put(key, it.trim()) }, ph, keyboard = kb) }
}

@Composable
private fun NumSetting(f: JSONObject, key: String, label: String, ph: String) {
    var v by remember { mutableStateOf(f.opt(key)?.toString() ?: "") }
    Labeled(label) { Field(v, { v = it.trim(); f.put(key, v) }, ph, keyboard = KeyboardType.Number) }
}

@Composable
private fun ToggleSetting(f: JSONObject, key: String, label: String, onTouch: (String) -> Unit) {
    ToggleRow(label, f.optBoolean(key)) { f.put(key, it); onTouch(key) }
}

data class Swatch(val v: String, val name: String, val top: Brush, val base: Color, val second: Brush?, val dashed: Boolean = false)

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun Swatches(label: String, items: List<Swatch>, value: String, onPick: (String) -> Unit) {
    val look = LocalLook.current
    Labeled(label) {
        FlowRow(horizontalArrangement = Arrangement.spacedBy(10.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
            for (it in items) {
                val on = it.v == value
                val src = rememberSource()
                Box(
                    Modifier.size(44.dp).pressScale(look, src, .85f).clip(CircleShape).background(it.base).background(it.top).then(if (it.second != null) Modifier.background(it.second) else Modifier)
                        .border(if (on) 3.dp else 2.dp, if (on) Color.White else Color.White.copy(alpha = if (it.dashed) .5f else .15f), CircleShape)
                        .clickable(src, null) { onPick(it.v) },
                )
            }
        }
        T(items.find { it.v == value }?.name ?: "", 12.sp, alpha = .6f, modifier = Modifier.padding(start = 4.dp, top = 2.dp))
    }
}

// Glances address: http / https list plus IP or name; port 61208 is added for IP addresses and local names
@Composable
private fun GlancesUrlField(f: JSONObject, host: String) {
    val portOf = 61208
    val cur0 = f.optString("baseUrl")
    val m0 = Regex("^(https?)://(.*)$", RegexOption.IGNORE_CASE).find(cur0)
    var sel by remember { mutableStateOf(m0?.groupValues?.get(1)?.lowercase() ?: "http") }
    var text by remember { mutableStateOf((m0?.groupValues?.get(2) ?: "").replace(Regex("^([^/:]+):$portOf(?=/|$)"), "$1")) }
    fun local(h: String) = Regex("^[0-9.]+$").matches(h) || h.startsWith("[") || !h.contains(".") || Regex("[.](local|lan|home|internal|localdomain|home[.]arpa|ts[.]net)$", RegexOption.IGNORE_CASE).containsMatchIn(h)
    fun compose() {
        var v = text.trim()
        val mm = Regex("^(https?)://(.*)$", RegexOption.IGNORE_CASE).find(v)
        if (mm != null) { sel = mm.groupValues[1].lowercase(); v = mm.groupValues[2].replace(Regex("/+$"), ""); text = v }
        v = v.replace(Regex("/+$"), "")
        var h = v
        if (h.isEmpty()) {
            if (sel == "http" || host.isEmpty()) { f.put("baseUrl", ""); return }
            h = host
        }
        val hp = Regex("^([^/:]+)(:[0-9]+)?(.*)$").find(h)
        val port = if (hp != null && hp.groupValues[2].isEmpty() && local(hp.groupValues[1])) ":$portOf" else ""
        f.put("baseUrl", "$sel://" + if (hp != null) hp.groupValues[1] + hp.groupValues[2] + port + hp.groupValues[3] else h)
    }
    Labeled("GLANCES ADDRESS") {
        Field(text, { text = it; compose() }, if (host.isNotEmpty()) "$host (the server)" else "IP or name", keyboard = KeyboardType.Uri, leading = { SchemePick(sel) { sel = it; compose() } })
    }
}
