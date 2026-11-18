package app.board.ui

import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.board.BoardVM
import app.board.LOGO_CDN
import app.board.Net
import app.board.PLATFORMS
import app.board.PLATFORM_KEYS
import app.board.Service
import app.board.Stack
import app.board.Store
import app.board.ZApp
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.ui.Alignment
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

val PLATFORM_ORDER = listOf("zimaos", "proxmox", "truenas", "unraid", "synology", "omv", "linux")
fun typeOpts() = PLATFORM_ORDER.map { it to PLATFORMS[it]!!.shortName }

val TYPE_HINT = mapOf(
    "none" to "Pick the system your server runs. It decides how Reboot / Shut down reach the server; until you pick one there is no Reboot / Shut down menu. Not sure? Debian, Ubuntu and similar are Linux.",
    "zimaos" to "ZimaOS: your ZimaOS login is used for Reboot / Shut down, the app controls (hold a button: Start / Stop / Restart / Logs / Update), app updates, adding apps from ZimaOS, CPU power draw and drive health. Nothing to install.",
    "proxmox" to "Proxmox VE: Reboot / Shut down use the Proxmox API with your Proxmox login (e.g. root@pam). Nothing to install. Its self-signed certificate is accepted.",
    "truenas" to "TrueNAS: Reboot / Shut down use your TrueNAS login (the admin account, e.g. truenas_admin, or root on older versions). Nothing to install. Self-signed https certificates are accepted; plain http on your home network or VPN is fine too.",
    "unraid" to "Unraid: Reboot / Shut down log in with your Unraid login (root) and send what Unraid's own Reboot / Shut down buttons send. Nothing to install.",
    "synology" to "Synology DSM: Reboot / Shut down use the DSM Web API with an administrator account. Nothing to install.",
    "omv" to "OpenMediaVault: Reboot / Shut down use OpenMediaVault's own API with an administrator account (admin by default). Nothing to install.",
    "linux" to "Linux (Debian, Ubuntu Server, Fedora ...): there's no web API for reboot, so -board uses a tiny helper service on the server, set up with one command over SSH from Settings. No login needed.",
)

// the hint under the server address: which port is used when none is typed
fun addrHint(P: String): String {
    val pl = PLATFORMS[P]
    val ports = pl?.ports ?: emptyMap()
    val usual = if (pl != null && ports["http"] != null)
        " For ${pl.shortName}, port " + (if (ports["http"] == ports["https"]) "${ports["http"]}" else "${ports["http"]} (http) or ${ports["https"]} (https)") + (if (P == "linux") " (the helper)" else "") + " is used unless you type another."
    else ""
    return "Pick http or https, then type the IP or name, plus a port if it isn't the usual one (e.g. 192.168.1.10:90).$usual LIVE / OFFLINE checks this address, Reboot / Shut down use it, and Glances is looked for on it. Use your VPN address to reach it away from home."
}

// The one server address field: http / https list + IP or name with an optional port.
@Composable
fun ServerAddrField(label: String, platform: String, host: String, scheme: String, port: String, onChange: (String, String, String) -> Unit) {
    val usual = PLATFORMS[platform]?.scheme ?: "http"
    var sel by remember(platform) { mutableStateOf(scheme.ifEmpty { usual }) }
    var text by remember { mutableStateOf(host + if (port.isNotEmpty()) ":$port" else "") }
    fun changed() {
        var v = text.trim()
        val m = Regex("^(https?)://(.*)$", RegexOption.IGNORE_CASE).find(v)
        if (m != null) { sel = m.groupValues[1].lowercase(); v = m.groupValues[2]; text = v }
        v = v.replace(Regex("[/?#].*$"), "")
        val hp = Regex("^([^:]+)(?::([0-9]{1,5}))?$").find(v)
        onChange(hp?.groupValues?.get(1) ?: v, hp?.groupValues?.get(2) ?: "", if (sel == usual) "" else sel)
    }
    Labeled(label) {
        Field(text, { text = it; changed() }, "192.168.1.10 or a name", keyboard = KeyboardType.Uri, leading = { SchemePick(sel) { sel = it; changed() } })
    }
}

// ---------- Add / edit a service ----------
@Composable
fun AddServiceDialog(vm: BoardVM, sv: Service?, onClose: () -> Unit, onPickZima: () -> Unit = {}) {
    var name by remember { mutableStateOf(sv?.name ?: "") }
    var scheme by remember { mutableStateOf(if (sv?.scheme == "https") "https" else "http") }
    var addr by remember { mutableStateOf(sv?.addr ?: "") }
    var usePort by remember { mutableStateOf(if (sv != null) sv.port.isNotEmpty() else true) }
    var port by remember { mutableStateOf(sv?.port ?: "") }
    var logo by remember { mutableStateOf(sv?.logo ?: "") }
    var onW by remember { mutableStateOf(sv?.onWidget ?: true) }
    var err by remember { mutableStateOf("") }
    var hint by remember { mutableStateOf("Empty = emoji. Auto guesses from the name.") }
    val scope = rememberCoroutineScope()
    Modal(onClose) {
        ModalTitle(if (sv != null) "EDIT SERVICE" else "ADD SERVICE")
        if (sv == null && vm.zimaReady) Box(Modifier.fillMaxWidth(), contentAlignment = Alignment.Center) { Chip("Pick from ZimaOS apps") { onPickZima() } }
        Labeled("SERVICE NAME") { Field(name, { name = it }, "e.g. Jellyfin", maxLength = 24) }
        Labeled("ADDRESS") {
            Field(addr, { addr = it }, if (usePort) serverHostOrDefault(vm) else "app.example.com", keyboard = KeyboardType.Uri, leading = { SchemePick(scheme) { scheme = it } })
        }
        ToggleRow("USE PORT", usePort) { usePort = it }
        if (usePort) Labeled("PORT") { Field(port, { port = it.filter { c -> c.isDigit() } }, "8080", keyboard = KeyboardType.Number, maxLength = 5) }
        LogoField(logo, { logo = it }, hint, { hint = it }, name, scope)
        if (sv?.group == null) ToggleRow("SHOW ON WIDGET", onW) { onW = it }
        ErrText(err)
        Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            Btn("Cancel", modifier = Modifier.weight(1f)) { onClose() }
            Btn(if (sv != null) "Save" else "Add", BtnKind.Ok, Modifier.weight(1f)) {
                val n = name.trim(); val a = addr.trim(); val lu = logo.trim()
                if (n.isEmpty() || a.isEmpty()) { err = "Enter a name and an address"; return@Btn }
                if (lu.isNotEmpty() && !Regex("^https?://", RegexOption.IGNORE_CASE).containsMatchIn(lu)) { err = "Logo URL must start with http:// or https://"; return@Btn }
                onClose()
                vm.saveService(sv?.id, n, scheme, a, if (usePort) port else "", lu, onW)
            }
        }
    }
}

fun serverHostOrDefault(vm: BoardVM) = app.board.serverHostOf(vm.set).ifEmpty { "192.168.1.10" }

// Logo URL with a live preview and "Auto" (guesses a dashboard-icons logo from the name)
@Composable
fun LogoField(logo: String, setLogo: (String) -> Unit, hint: String, setHint: (String) -> Unit, name: String, scope: kotlinx.coroutines.CoroutineScope) {
    var preview by remember { mutableStateOf<ImageBitmap?>(null) }
    LaunchedEffect(logo) {
        delay(350)
        preview = null
        val v = logo.trim()
        if (Regex("^https?://", RegexOption.IGNORE_CASE).containsMatchIn(v)) preview = loadPreview(v)
    }
    Labeled("LOGO URL (OPTIONAL)") {
        Field(logo, setLogo, "https://.../logo.png", keyboard = KeyboardType.Uri, trailing = {
            preview?.let { Image(it, null, Modifier.padding(start = 8.dp).size(26.dp)) }
            Spacer(Modifier.width(8.dp))
            Chip("Auto") {
                val n = name.trim().lowercase()
                if (n.isEmpty()) { setHint("Type a name first."); return@Chip }
                val slug = n.replace(Regex("[^a-z0-9]+"), "-").trim('-')
                val url = "$LOGO_CDN$slug.png"
                setHint("Checking $slug.png ...")
                scope.launch {
                    if (app.board.Logos.exists(url)) { setLogo(url); setHint("Found $slug.png") }
                    else setHint("No logo named $slug.png. Paste a URL instead.")
                }
            }
        })
        T(hint, 11.sp, alpha = .45f, modifier = Modifier.padding(horizontal = 4.dp))
    }
}

suspend fun loadPreview(url: String): ImageBitmap? = withContext(Dispatchers.IO) {
    try {
        val r = Net.request(url, timeout = 8.0, binary = true)
        if (r.code >= 400 || r.bytes == null) null else android.graphics.BitmapFactory.decodeByteArray(r.bytes, 0, r.bytes.size)?.asImageBitmap()
    } catch (e: Exception) { null }
}

// ---------- Hold and release a button: menu (plus Start / Stop / Restart / Logs / Update for a ZimaOS app) ----------
@Composable
fun MenuDialog(vm: BoardVM, sv: Service, onClose: () -> Unit, onStack: () -> Unit, onEdit: () -> Unit, onDelete: () -> Unit, onLogs: (ZApp) -> Unit) {
    var armed by remember { mutableStateOf(false) }
    var stopArmed by remember { mutableStateOf(false) }
    val app = vm.zAppFor(sv)
    fun run(action: String, busy: String) {
        val a = app ?: return
        onClose()
        vm.showToast("$busy ${a.title}...", 120000)
        vm.viewModelScope.launch { vm.showToast(vm.zAction(a, action), 4000) }
    }
    Modal(onClose) {
        ModalTitle(sv.name)
        if (app != null) {
            val upd = vm.zUpdatable(app)
            Note("ZimaOS app · " + (if (app.running) "running" else app.status.ifEmpty { "stopped" }) + if (upd) " · update available" else "")
            Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
                if (app.controllable) {
                    if (app.running) {
                        Btn(if (stopArmed) "Tap again to stop" else "Stop", BtnKind.RedText, Modifier.fillMaxWidth()) {
                            if (!stopArmed) stopArmed = true else run("stop", "Stopping")
                        }
                        Btn("Restart", modifier = Modifier.fillMaxWidth()) { run("restart", "Restarting") }
                    } else Btn("Start", BtnKind.GreenText, Modifier.fillMaxWidth()) { run("start", "Starting") }
                }
                Btn("Logs", modifier = Modifier.fillMaxWidth()) { onLogs(app) }
                if (upd && app.controllable) Btn("Update", BtnKind.GreenText, Modifier.fillMaxWidth()) { run("update", "Updating") }
            }
        }
        Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
            Btn(if (sv.group != null) "Change stack" else "Add to stack", modifier = Modifier.fillMaxWidth()) { onStack() }
            Btn("Edit", BtnKind.GreenText, Modifier.fillMaxWidth()) { onEdit() }
            Btn(if (armed) "Tap again to delete" else "Delete", BtnKind.RedText, Modifier.fillMaxWidth()) { if (!armed) armed = true else onDelete() }
            Btn("Cancel", modifier = Modifier.fillMaxWidth()) { onClose() }
        }
    }
}

// ---------- ZimaOS app logs: the last 300 lines ----------
@Composable
fun LogsDialog(vm: BoardVM, app: ZApp, onClose: () -> Unit) {
    var text by remember { mutableStateOf("Loading...") }
    var ok by remember { mutableStateOf(true) }
    var seq by remember { mutableIntStateOf(0) }
    val scroll = rememberScrollState()
    LaunchedEffect(seq) {
        text = "Loading..."
        val r = vm.zLogs(app)
        ok = r.first; text = r.second
        delay(50)
        scroll.scrollTo(scroll.maxValue)
    }
    Modal(onClose) {
        ModalTitle("${app.title.uppercase()} LOGS")
        Box(
            Modifier.fillMaxWidth().height(440.dp).clip(RoundedCornerShape(16.dp)).background(Color.Black.copy(alpha = .3f))
                .verticalScroll(scroll).padding(12.dp),
        ) {
            Text(
                text, color = if (ok) Color.White.copy(alpha = .88f) else RED, fontSize = 11.sp, lineHeight = 15.sp,
                fontFamily = FontFamily.Monospace,
            )
        }
        Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            Btn("Refresh", modifier = Modifier.weight(1f)) { seq++ }
            Btn("Close", BtnKind.Ok, Modifier.weight(1f)) { onClose() }
        }
    }
}

// ---------- ZimaOS app updates (the "↑ 2 updates" chip) ----------
@Composable
fun UpdatesDialog(vm: BoardVM, onClose: () -> Unit) {
    val state = remember { mutableStateMapOf<String, String>() }
    // the list as it was when the popup opened (an app stays listed with its result after updating)
    val list = remember { vm.zUpdates }
    Modal(onClose) {
        ModalTitle("APP UPDATES")
        Note("Newer versions in the ZimaOS App Store. Updating pulls the new image and recreates the app; its data stays.")
        Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
            for (a in list) Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                AppIcon(a.icon)
                Spacer(Modifier.width(12.dp))
                Column(Modifier.weight(1f)) {
                    T(a.title, 15.sp, FontWeight.Bold, maxLines = 1)
                    T("Update available", 11.sp, alpha = .5f, maxLines = 1)
                }
                Spacer(Modifier.width(8.dp))
                val st = state[a.id]
                Chip(st ?: "Update") {
                    if (st != null) return@Chip
                    state[a.id] = "Updating..."
                    vm.viewModelScope.launch {
                        val r = vm.zAction(a, "update")
                        val ok = r.contains("update started")
                        state[a.id] = if (ok) "Started" else "Failed"
                        if (!ok) vm.showToast(r, 6000)
                    }
                }
            }
        }
        Btn("Close", modifier = Modifier.fillMaxWidth()) { onClose() }
    }
}

// ---------- Add apps from ZimaOS (from the + popup) ----------
@Composable
fun ZimaPickDialog(vm: BoardVM, onClose: () -> Unit) {
    var err by remember { mutableStateOf<String?>(null) }
    var loading by remember { mutableStateOf(true) }
    val picked = remember { mutableStateMapOf<String, Boolean>() }
    LaunchedEffect(Unit) { err = vm.loadZApps(); loading = false }
    Modal(onClose) {
        ModalTitle("ADD FROM ZIMAOS")
        Note("Your installed ZimaOS apps. Pick the ones to add as buttons; they get the app's address, logo and the Start / Stop / Logs controls.")
        val apps = vm.zApps.orEmpty()
        val taken = vm.layout.all.mapNotNull { vm.zAppFor(it)?.id }.toSet()
        // Add says how many are picked
        when {
            loading -> Note("Loading your apps...")
            err != null -> ErrText(err!!)
            apps.isEmpty() -> Note("No apps installed on ZimaOS.")
            else -> Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                for (a in apps) {
                    val has = a.id in taken
                    Row(Modifier.fillMaxWidth().alpha(if (has) .4f else 1f), verticalAlignment = Alignment.CenterVertically) {
                        AppIcon(a.icon)
                        Spacer(Modifier.width(12.dp))
                        Column(Modifier.weight(1f)) {
                            T(a.title, 15.sp, FontWeight.Bold, maxLines = 1)
                            T(if (has) "Already a button" else (if (a.port.isNotEmpty()) "port ${a.port} · " else "") + a.status, 11.sp, alpha = .55f, maxLines = 1)
                        }
                        if (!has) Switch(picked[a.id] == true) { picked[a.id] = it }
                    }
                }
            }
        }
        Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            Btn("Cancel", modifier = Modifier.weight(1f)) { onClose() }
            val n = picked.count { it.value }
            Btn(if (n > 0) "Add $n" else "Add", BtnKind.Ok, Modifier.weight(1f)) {
                val sel = apps.filter { picked[it.id] == true && it.id !in taken }
                onClose()
                vm.addZimaApps(sel)
                vm.showToast("${sel.size} app${if (sel.size == 1) "" else "s"} added from ZimaOS", 3500)
            }
        }
    }
}

// An app's icon from ZimaOS (or a blank square while it loads / if it has none)
@Composable
fun AppIcon(url: String) {
    var bmp by remember(url) { mutableStateOf<ImageBitmap?>(app.board.Logos.cached(url)?.asImageBitmap()) }
    LaunchedEffect(url) { if (bmp == null && Regex("^https?://", RegexOption.IGNORE_CASE).containsMatchIn(url)) bmp = loadPreview(url) }
    Box(Modifier.size(36.dp).clip(RoundedCornerShape(8.dp)).background(Color.White.copy(alpha = .06f)), contentAlignment = Alignment.Center) {
        bmp?.let { Image(it, null, Modifier.size(32.dp)) }
    }
}

// ---------- Pick a stack for a button ----------
@Composable
fun PickDialog(vm: BoardVM, sv: Service, onClose: () -> Unit, onNew: () -> Unit) {
    Modal(onClose) {
        ModalTitle(if (sv.group != null) "MOVE TO STACK" else "ADD TO STACK")
        val stacks = vm.layout.groups.filter { it.id != sv.group }
        if (stacks.isNotEmpty()) Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
            for (g in stacks) Btn(g.name, modifier = Modifier.fillMaxWidth()) { onClose(); vm.stackAction(Store.StackAct("move", id = sv.id, to = g.id)) }
        }
        Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
            Btn("New stack", BtnKind.Ok, Modifier.fillMaxWidth()) { onNew() }
            if (sv.group != null) Btn("Remove from stack", BtnKind.Del, Modifier.fillMaxWidth()) { onClose(); vm.stackAction(Store.StackAct("move", id = sv.id, to = null)) }
            Btn("Cancel", modifier = Modifier.fillMaxWidth()) { onClose() }
        }
    }
}

// ---------- Create / edit / ungroup a stack ----------
@Composable
fun StackDialog(vm: BoardVM, g: Stack?, member: Service?, onClose: () -> Unit) {
    var name by remember { mutableStateOf(g?.name ?: "") }
    var logo by remember { mutableStateOf(g?.logo ?: "") }
    var exp by remember { mutableStateOf(g?.expand ?: false) }
    var err by remember { mutableStateOf("") }
    var hint by remember { mutableStateOf("Empty = 🗂️. Auto guesses from the name.") }
    var armed by remember { mutableStateOf(false) }
    val scope = rememberCoroutineScope()
    Modal(onClose) {
        ModalTitle(if (g != null) "EDIT STACK" else "NEW STACK")
        Labeled("STACK NAME") { Field(name, { name = it }, "e.g. Media", maxLength = 24) }
        LogoField(logo, { logo = it }, hint, { hint = it }, name, scope)
        ToggleRow("SHOW MEMBERS ON WIDGET", exp) { exp = it }
        ErrText(err)
        Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            Btn("Cancel", modifier = Modifier.weight(1f)) { onClose() }
            Btn(if (g != null) "Save" else "Create", BtnKind.Ok, Modifier.weight(1f)) {
                val n = name.trim(); val lu = logo.trim()
                if (n.isEmpty()) { err = "Enter a name"; return@Btn }
                if (lu.isNotEmpty() && !Regex("^https?://", RegexOption.IGNORE_CASE).containsMatchIn(lu)) { err = "Logo URL must start with http:// or https://"; return@Btn }
                onClose()
                vm.stackAction(Store.StackAct("save", gid = g?.id, member = member?.id, name = n, logo = lu, expand = exp))
            }
        }
        if (g != null) Btn(if (armed) "Tap again to ungroup" else "Ungroup", BtnKind.Del, Modifier.fillMaxWidth()) {
            if (!armed) armed = true else { onClose(); vm.stackAction(Store.StackAct("del", gid = g.id)) }
        }
    }
}

// ---------- Rename (hold the server name) ----------
@Composable
fun RenameDialog(vm: BoardVM, onClose: () -> Unit) {
    var v by remember { mutableStateOf(vm.title) }
    var err by remember { mutableStateOf("") }
    Modal(onClose) {
        ModalTitle("RENAME")
        Labeled("DASHBOARD NAME") { Field(v, { v = it }, maxLength = 20) }
        ErrText(err)
        Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            Btn("Cancel", modifier = Modifier.weight(1f)) { onClose() }
            Btn("Save", BtnKind.Ok, Modifier.weight(1f)) {
                if (v.trim().isEmpty()) { err = "Enter a name"; return@Btn }
                onClose(); vm.rename(v)
            }
        }
    }
}

// ---------- Reboot / shut down: confirm (and the login, the first time) ----------
@Composable
fun PowerDialog(vm: BoardVM, d: Dlg.PowerConfirm, onClose: () -> Unit, reopen: (Dlg.PowerConfirm) -> Unit) {
    val re = d.a == "restart"
    val pl = PLATFORMS[vm.set.platform]
    val login = pl?.login
    val showL = login != null && (d.needLogin || !vm.hasLogin)
    var u by remember { mutableStateOf("") }
    var p by remember { mutableStateOf("") }
    var err by remember(d) { mutableStateOf(d.err) }
    val scope = rememberCoroutineScope()
    Modal(onClose) {
        ModalTitle(if (re) "REBOOT SERVER?" else "SHUT DOWN SERVER?")
        Note(if (re) "Every service goes offline until the server is back up, usually a minute or two." else "Every service goes offline. You'll have to switch the server back on by hand.")
        if (showL) {
            Labeled("${login!!.uppercase()} USERNAME") { Field(u, { u = it }, pl.userPh) }
            Labeled("${login.uppercase()} PASSWORD") { Field(p, { p = it }, password = true) }
            T("Asked once, then saved on this phone (encrypted).", 11.sp, alpha = .45f, modifier = Modifier.padding(horizontal = 4.dp))
        }
        ErrText(err)
        Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            Btn("Cancel", modifier = Modifier.weight(1f)) { onClose() }
            Btn(if (re) "Reboot" else "Shut down", BtnKind.Del, Modifier.weight(1f)) {
                if (showL && (u.trim().isEmpty() || p.isEmpty())) { err = "Enter your $login username and password"; return@Btn }
                onClose()
                vm.showToast(if (re) "Sending reboot..." else "Sending shutdown...", 15000)
                // runs on after this popup closes
                vm.viewModelScope.launch {
                    val r = vm.power(d.a, if (showL) u.trim() else "", if (showL) p else "")
                    if (r.r.auth) { vm.showToast("", 1); reopen(Dlg.PowerConfirm(r.a, r.r.msg, true)) }
                    else vm.showToast(r.r.msg, 5000)
                }
            }
        }
    }
}

// ---------- Add server: name, type, address, login (the same order as its Settings) ----------
@Composable
fun AddServerDialog(vm: BoardVM, onClose: () -> Unit, openSettings: () -> Unit) {
    var name by remember { mutableStateOf("") }
    var platform by remember { mutableStateOf("none") }
    var host by remember { mutableStateOf("") }
    var scheme by remember { mutableStateOf("") }
    var port by remember { mutableStateOf("") }
    var u by remember { mutableStateOf("") }
    var p by remember { mutableStateOf("") }
    var err by remember { mutableStateOf("") }
    var busy by remember { mutableStateOf(false) }
    val scope = rememberCoroutineScope()
    Modal(onClose) {
        ModalTitle("ADD SERVER")
        Note("Each server has its own address, type, logins, Monitoring, buttons and layout. Swipe the server name to switch between them.")
        Labeled("NAME") { Field(name, { name = it }, "e.g. NAS", maxLength = 20) }
        Labeled("SERVER TYPE") { Seg(typeOpts(), platform, wrap = true) { platform = it; scheme = ""; err = "" } }
        Hint(TYPE_HINT[platform] ?: TYPE_HINT["none"]!!)
        androidx.compose.runtime.key(platform) {
            ServerAddrField("SERVER ADDRESS", platform, host, scheme, port) { h, pt, sc -> host = h; port = pt; scheme = sc }
        }
        val pl = PLATFORMS[platform]
        if (pl?.login != null) {
            Labeled("${pl.login.uppercase()} USERNAME") { Field(u, { u = it }, pl.userPh) }
            Labeled("${pl.login.uppercase()} PASSWORD") { Field(p, { p = it }, password = true) }
            Hint("For Reboot / Shut down. Checked against the server before it's added, then stored encrypted on this phone. Leave empty to be asked the first time.")
        } else if (platform == "linux") {
            Hint("After adding, its Settings open so you can set up the helper (one command over SSH).")
        }
        ErrText(err)
        Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            Btn("Cancel", modifier = Modifier.weight(1f)) { onClose() }
            Btn(if (busy) "Checking..." else "Add", BtnKind.Ok, Modifier.weight(1f)) {
                if (busy) return@Btn
                if (platform == "none") { err = "Pick the server type"; return@Btn }
                if (host.trim().isEmpty()) { err = "Enter the server's IP or name"; return@Btn }
                if ((u.trim().isNotEmpty() && p.isEmpty()) || (u.trim().isEmpty() && p.isNotEmpty())) { err = "Enter both the username and the password, or neither"; return@Btn }
                err = ""; busy = true
                scope.launch {
                    val e = vm.addServer(name.trim(), platform, host.trim(), scheme, port, u.trim(), p)
                    busy = false
                    if (e != null) err = e
                    else { onClose(); if (platform == "linux") { delay(450); openSettings() } }
                }
            }
        }
    }
}

// ---------- Delete server (from the bottom of Settings) ----------
@Composable
fun DelServerDialog(name: String, onClose: () -> Unit, onDelete: () -> Unit) {
    Modal(onClose) {
        ModalTitle("DELETE ${name.uppercase()}?")
        Note("Its settings, saved logins, buttons, stacks and layout are deleted from this phone. Your other servers stay. This can't be undone (unless you exported your settings).")
        Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            Btn("Cancel", modifier = Modifier.weight(1f)) { onClose() }
            Btn("Delete", BtnKind.Del, Modifier.weight(1f)) { onDelete() }
        }
    }
}

@Suppress("unused") private val keepKeys = PLATFORM_KEYS
