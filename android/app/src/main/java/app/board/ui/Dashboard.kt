package app.board.ui

import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Build
import androidx.browser.customtabs.CustomTabsIntent
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.spring
import androidx.compose.animation.core.tween
import androidx.compose.animation.expandVertically
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.shrinkVertically
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.ScrollState
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.gestures.detectHorizontalDragGestures
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.gestures.scrollBy
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.blur
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.PointerEventPass
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.layout.positionInRoot
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalViewConfiguration
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.unit.toSize
import androidx.compose.ui.zIndex
import app.board.BoardVM
import app.board.CpuDetail
import app.board.DiskDetail
import app.board.Entry
import app.board.PLATFORMS
import app.board.RamDetail
import app.board.Service
import app.board.Stack
import app.board.formatBytes
import app.board.serverHostOf
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlin.math.abs
import kotlin.math.roundToInt

fun openUrl(ctx: Context, url: String) {
    try {
        CustomTabsIntent.Builder().setShowTitle(true).build().launchUrl(ctx, Uri.parse(url))
    } catch (e: Exception) {
        try { ctx.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(url)).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)) } catch (_: Exception) {}
    }
}

// Which popup is open
sealed class Dlg {
    data class AddService(val sv: Service?) : Dlg()
    data class Menu(val sv: Service) : Dlg()
    data class Pick(val sv: Service) : Dlg()
    data class StackEdit(val g: Stack?, val member: Service?) : Dlg()
    object Rename : Dlg()
    data class PowerConfirm(val a: String, val err: String = "", val needLogin: Boolean = false) : Dlg()
    object AddServer : Dlg()
    // ZimaOS servers only
    data class Logs(val app: app.board.ZApp) : Dlg()
    object Updates : Dlg()
    object ZimaPick : Dlg()
}

// ---------- Drag and drop ----------
class DragState {
    val rects = HashMap<String, Rect>()
    val pressRects = HashMap<String, Rect>()
    val boxRects = HashMap<String, Rect>()
    var lists by mutableStateOf<Map<String, List<String>>>(emptyMap())
    var dragId by mutableStateOf<String?>(null)
    var pointer by mutableStateOf(Offset.Zero)
    var grab = Offset.Zero
    var ghostSize by mutableStateOf(androidx.compose.ui.geometry.Size.Zero)
    var pressedId by mutableStateOf<String?>(null)
    var moved = false
    var startSig = ""
    var cooldown = 0L
    fun sig() = lists.toString()
}

@Composable
fun Dashboard(vm: BoardVM, onSettings: () -> Unit, dlg: Dlg?, setDlg: (Dlg?) -> Unit) {
    val look = LocalLook.current
    val ctx = LocalContext.current
    val set = vm.set
    val scroll = rememberScrollState()
    val drag = remember { DragState() }
    val scope = rememberCoroutineScope()
    val dens = LocalDensity.current
    val conf = LocalConfiguration.current
    val viewportH = with(dens) { conf.screenHeightDp.dp.toPx() }
    val L = vm.layout
    val wide = vm.mode == "widget"

    // entries by id, and the containers as the layout has them (or as the drag has rearranged them)
    val byId = remember(L) { HashMap<String, Entry>().apply { L.all.forEach { put(it.id, it) }; L.groups.forEach { put(it.id, it) } } }
    val baseLists = remember(L, wide) {
        if (wide) mapOf("svc" to L.widget.map { it.id }, "hid" to L.hidden.map { it.id })
        else mapOf("svc" to L.dash.map { it.id }) + L.groups.associate { g -> g.id to g.members.map { it.id } }
    }
    val lists = if (drag.dragId != null) drag.lists else baseLists

    fun openBodies() = if (wide) emptyList() else lists["svc"].orEmpty().filter { it.startsWith("g:") && vm.openG[it] == true }

    fun hitAt(p: Offset): String? {
        val order = ArrayList<String>()
        for (g in openBodies()) order.addAll(lists[g].orEmpty())
        order.addAll(lists["svc"].orEmpty())
        order.addAll(lists["hid"].orEmpty())
        return order.firstOrNull { drag.pressRects[it]?.contains(p) == true }
    }

    fun moveTo(id: String, box: String, index: Int?) {
        val m = drag.lists.mapValues { (_, v) -> v.filter { it != id } }.toMutableMap()
        val l = m[box].orEmpty().toMutableList()
        if (index == null || index > l.size) l.add(id) else l.add(index, id)
        m[box] = l
        drag.lists = m
        drag.cooldown = System.currentTimeMillis() + 140
    }

    fun reorder(p: Offset) {
        val id = drag.dragId ?: return
        if (System.currentTimeMillis() < drag.cooldown) return
        val isGrp = id.startsWith("g:")
        val boxes = if (wide) listOf("svc", "hid") else (if (!isGrp) openBodies() else emptyList()) + "svc"
        for (b in boxes) {
            val kids = drag.lists[b].orEmpty()
            for (c in kids) {
                if (c == id) continue
                if (!isGrp && c.startsWith("g:") && !wide && vm.openG[c] == true) {
                    val br = drag.boxRects[c]
                    if (br != null && p.x >= br.left && p.x <= br.right && p.y >= br.top - 8 && p.y <= br.bottom + 8) continue
                }
                val r = drag.rects[c] ?: continue
                if (r.contains(p)) {
                    val ci = kids.indexOf(c)
                    val myI = kids.indexOf(id)
                    val before = myI in 0 until ci
                    val rest = kids.filter { it != id }
                    val at = rest.indexOf(c) + if (before) 1 else 0
                    moveTo(id, b, at)
                    return
                }
            }
        }
        for (b in boxes) {
            val r = drag.boxRects[b] ?: continue
            if (p.x < r.left || p.x > r.right || p.y < r.top - 8 || p.y > r.bottom + 8) continue
            val others = drag.lists[b].orEmpty().filter { it != id }
            if (others.isEmpty()) {
                if (id !in drag.lists[b].orEmpty()) moveTo(id, b, null)
                return
            }
            val lastR = drag.rects[others.last()] ?: continue
            if (p.y > lastR.bottom && drag.lists[b].orEmpty().lastOrNull() != id) {
                moveTo(id, b, null)
                return
            }
        }
    }

    fun showMenu(id: String) {
        val e = byId[id] ?: return
        if (e is Stack) setDlg(Dlg.StackEdit(e, null)) else if (e is Service) setDlg(Dlg.Menu(e))
    }

    fun endDrag() {
        val id = drag.dragId ?: return
        if (!drag.moved) {
            drag.dragId = null
            showMenu(id)
            return
        }
        if (drag.sig() != drag.startSig) {
            val l = drag.lists
            if (wide) vm.commitWidget(l["svc"].orEmpty(), l["hid"].orEmpty())
            else vm.commitOrder(l["svc"].orEmpty(), l.filterKeys { it.startsWith("g:") })
        }
        drag.dragId = null
    }

    // edges scroll the page while dragging
    LaunchedEffect(drag.dragId) {
        while (drag.dragId != null) {
            val y = drag.pointer.y
            if (y > viewportH - with(dens) { 90.dp.toPx() }) scroll.scrollBy(with(dens) { 6.dp.toPx() })
            else if (y < with(dens) { 90.dp.toPx() }) scroll.scrollBy(-with(dens) { 6.dp.toPx() })
            reorder(drag.pointer)
            delay(16)
        }
    }

    var origin by remember { mutableStateOf(Offset.Zero) }
    val slop = LocalViewConfiguration.current.touchSlop

    Box(
        Modifier.fillMaxSize()
            .onGloballyPositioned { origin = it.positionInRoot() }
            .pointerInput(wide, L) {
                awaitEachGesture {
                    val down = awaitFirstDown(requireUnconsumed = false, pass = PointerEventPass.Initial)
                    val p0 = down.position + origin
                    val hit = hitAt(p0) ?: return@awaitEachGesture
                    drag.pressedId = hit
                    val res = withTimeoutOrNull(500L) {
                        while (true) {
                            val ev = awaitPointerEvent(PointerEventPass.Initial)
                            val ch = ev.changes.firstOrNull { it.id == down.id } ?: return@withTimeoutOrNull "cancel"
                            if (!ch.pressed) return@withTimeoutOrNull "tap"
                            if ((ch.position - down.position).getDistance() > slop) return@withTimeoutOrNull "scroll"
                        }
                        @Suppress("UNREACHABLE_CODE") "cancel"
                    }
                    if (res != null) {
                        drag.pressedId = null
                        if (res == "tap") {
                            val e = byId[hit]
                            if (e is Service) openUrl(ctx, e.url)
                            else if (e is Stack && !wide) vm.openG[e.id] = vm.openG[e.id] != true
                        }
                        return@awaitEachGesture
                    }
                    // held: lift the button (or the whole stack) and follow the finger
                    drag.pressedId = null
                    val r = drag.rects[hit] ?: return@awaitEachGesture
                    val pr = drag.pressRects[hit] ?: r
                    drag.lists = lists
                    drag.startSig = drag.sig()
                    drag.moved = false
                    drag.grab = p0 - pr.topLeft
                    drag.ghostSize = pr.size
                    drag.pointer = p0
                    drag.dragId = hit
                    while (true) {
                        val ev = awaitPointerEvent(PointerEventPass.Initial)
                        val ch = ev.changes.firstOrNull { it.id == down.id } ?: break
                        ev.changes.forEach { it.consume() }
                        if (!ch.pressed) break
                        val p = ch.position + origin
                        if (abs(p.x - p0.x) > 10 || abs(p.y - p0.y) > 10) drag.moved = true
                        drag.pointer = p
                        reorder(p)
                    }
                    endDrag()
                }
            },
    ) {
        Column(
            Modifier.fillMaxSize().verticalScroll(scroll).statusBarsPadding().navigationBarsPadding()
                .padding(start = 18.dp, end = 18.dp, top = 56.dp - 24.dp, bottom = 40.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            Header(vm, onSettings, setDlg)
            ServerDots(vm, setDlg)
            if (set.monitoring) Monitoring(vm)
            // section head
            Row(Modifier.fillMaxWidth().padding(horizontal = 4.dp).padding(top = 6.dp).enter(4, vm.switchSeq), verticalAlignment = Alignment.CenterVertically) {
                Caps(if (wide) "WIDGET LAYOUT" else "SERVICES", 12.sp, .6f, Modifier.weight(1f))
                // ZimaOS: app updates waiting in the App Store (hidden when there are none)
                val nUp = if (vm.zimaReady) vm.zUpdates.size else 0
                if (!wide && nUp > 0) {
                    LayButton("↑ $nUp ${if (nUp == 1) "update" else "updates"}", false) { setDlg(Dlg.Updates) }
                    Spacer(Modifier.width(8.dp))
                }
                LayButton(if (wide) "Done" else "Widget layout", wide) { vm.mode = if (wide) "dash" else "widget" }
            }
            ServicesGrid(vm, drag, lists, byId, wide, "svc")
            if (wide) {
                T(
                    "The large widget fits ${set.widgetRows * 4} buttons. Dimmed ones will not show. Press, hold and drag to reorder, or drag a button between the two panels to show or hide it.",
                    12.sp, alpha = .55f, modifier = Modifier.padding(horizontal = 4.dp), lineHeight = 17.sp,
                )
                Caps("NOT ON WIDGET", 12.sp, .6f, Modifier.padding(horizontal = 4.dp))
                ServicesGrid(vm, drag, lists, byId, true, "hid")
            } else {
                AddButton { setDlg(Dlg.AddService(null)) }
            }
        }
        // the lifted button follows the finger
        val dId = drag.dragId
        if (dId != null) {
            val e = byId[dId]
            if (e != null) {
                val sz = with(dens) { drag.ghostSize.width.toDp() } to with(dens) { drag.ghostSize.height.toDp() }
                Box(
                    Modifier.zIndex(20f).offset { IntOffset((drag.pointer.x - drag.grab.x - origin.x).roundToInt(), (drag.pointer.y - drag.grab.y - origin.y).roundToInt()) }
                        .size(sz.first, sz.second).graphicsLayer { val s = if (look.liquid) 1.1f else 1.07f; scaleX = s; scaleY = s; shadowElevation = 24f; shape = RoundedCornerShape(if (wide) look.wide() else look.svc()); clip = false },
                ) {
                    if (e is Stack && !wide) StackHeader(vm, e, lists[e.id]?.size ?: e.members.size, ghost = true)
                    else EntryButton(vm, e, wide, ghost = true, state = dotState(vm, e))
                }
            }
        }
    }
}

fun dotState(vm: BoardVM, e: Entry): String? {
    if (e is Stack) {
        var up = 0; var dn = 0
        for (m in e.members) when (vm.svcUp[m.id]) { true -> up++; false -> dn++; else -> {} }
        val n = e.members.size
        if (n == 0 || up + dn == 0) return null
        return if (up == n) "up" else if (dn == n) "dn" else if (dn > 0) "part" else null
    }
    return when (vm.svcUp[e.id]) { true -> "up"; false -> "dn"; else -> null }
}

@Composable
fun LayButton(text: String, on: Boolean, onClick: () -> Unit) {
    val look = LocalLook.current
    val src = rememberSource()
    Box(
        Modifier.pressScale(look, src).clip(RoundedCornerShape(99.dp))
            .then(if (on) Modifier.background(look.acc) else if (look.liquid) Modifier.glass(look, RoundedCornerShape(99.dp), 1.7f) else Modifier.background(Color.White.copy(alpha = .12f)).border(1.dp, Color.White.copy(alpha = .14f), RoundedCornerShape(99.dp)))
            .clickable(src, null) { onClick() }.padding(horizontal = if (look.liquid) 14.dp else 12.dp, vertical = if (look.liquid) 7.dp else 6.dp),
    ) { T(text, 12.sp, FontWeight.Bold, if (on) look.ink else Color.White) }
}

@Composable
fun AddButton(onClick: () -> Unit) {
    val look = LocalLook.current
    val src = rememberSource()
    val pressed by src.collectIsPressedAsState()
    val rot by animateFloatAsState(if (look.liquid && pressed) 90f else 0f, spring(.5f, 300f), label = "addrot")
    Box(Modifier.fillMaxWidth().padding(top = 2.dp).enter(6), contentAlignment = Alignment.Center) {
        val sh = if (look.liquid) CircleShape else RoundedCornerShape(if (look.sharp) 8.dp else 16.dp)
        Box(
            Modifier.size(if (look.liquid) 58.dp else 52.dp).pressScale(look, src, .88f).rotate(rot)
                .then(if (look.liquid) Modifier.glass(look, sh, 1.6f) else Modifier.background(Color.White.copy(alpha = if (pressed) .22f else .1f), sh).border(1.dp, Color.White.copy(alpha = .14f), sh))
                .clip(sh).clickable(src, null) { onClick() },
            contentAlignment = Alignment.Center,
        ) { T("+", if (look.liquid) 30.sp else 28.sp, FontWeight.Light) }
    }
}

// ---------- Header: server name (tap = Reboot / Shut down, hold = rename, swipe = other server), LIVE, gear ----------
@Composable
fun Header(vm: BoardVM, onSettings: () -> Unit, setDlg: (Dlg?) -> Unit) {
    val look = LocalLook.current
    val set = vm.set
    val power = set.showPower && PLATFORMS.containsKey(set.platform)
    var menu by remember { mutableStateOf(false) }
    val slide = remember(vm.switchSeq) { Animatable(if (vm.switchDir != 0) 1f else 0f) }
    LaunchedEffect(vm.switchSeq) { slide.animateTo(0f, tween(350, easing = SOFT)) }
    val dens = LocalDensity.current

    Row(Modifier.fillMaxWidth().zIndex(8f).enter(0), verticalAlignment = Alignment.CenterVertically) {
        Box(Modifier.weight(1f)) {
            Row(
                Modifier
                    .pointerInput(power) {
                        detectTapGestures(onTap = { if (power) menu = !menu }, onLongPress = { setDlg(Dlg.Rename) })
                    }
                    .pointerInput(vm.servers, vm.cur) {
                        var dx = 0f
                        detectHorizontalDragGestures(onDragStart = { dx = 0f }, onDragEnd = {
                            if (abs(dx) >= 45 * density) {
                                val l = vm.servers
                                val i = l.indexOfFirst { it.id == vm.cur }
                                if (dx < 0) { if (i >= 0 && i < l.size - 1) vm.switchServer(l[i + 1].id, 1) else setDlg(Dlg.AddServer) }
                                else if (i > 0) vm.switchServer(l[i - 1].id, -1)
                            }
                        }) { ch, d -> ch.consume(); dx += d }
                    },
                verticalAlignment = Alignment.CenterVertically,
            ) {
                T(
                    vm.title, if (look.liquid) 24.sp else 22.sp, FontWeight.ExtraBold, spacing = (if (look.liquid) 2.4 else 2.64).sp, maxLines = 1,
                    modifier = Modifier.weight(1f, fill = false).graphicsLayer {
                        translationX = slide.value * vm.switchDir * with(dens) { 44.dp.toPx() }
                        alpha = 1f - slide.value
                    },
                )
                if (power) {
                    val rot by animateFloatAsState(if (menu) 180f else 0f, if (look.liquid) spring(.5f, 300f) else tween(300, easing = SOFT), label = "chev")
                    Spacer(Modifier.width(8.dp))
                    Chevron(Modifier.size(14.dp).rotate(rot).alpha(.6f))
                }
            }
            if (menu) PowerMenu(onDismiss = { menu = false }) { a -> menu = false; setDlg(Dlg.PowerConfirm(a)) }
        }
        Spacer(Modifier.width(8.dp))
        LivePill(vm)
        Spacer(Modifier.width(8.dp))
        GearButton(onSettings)
    }
}

@Composable
fun Chevron(modifier: Modifier, color: Color = Color.White) {
    Canvas(modifier) {
        val p = Path().apply {
            moveTo(size.width * 6 / 24, size.height * 9 / 24); lineTo(size.width / 2, size.height * 15 / 24); lineTo(size.width * 18 / 24, size.height * 9 / 24)
        }
        drawPath(p, color, style = Stroke(size.width * 2.5f / 24, cap = StrokeCap.Round, join = StrokeJoin.Round))
    }
}

@Composable
fun PowerMenu(onDismiss: () -> Unit, onPick: (String) -> Unit) {
    val look = LocalLook.current
    val a = remember { Animatable(0f) }
    LaunchedEffect(Unit) { a.animateTo(1f, if (look.liquid) spring(.55f, 300f) else tween(200, easing = SOFT)) }
    androidx.compose.ui.window.Popup(offset = IntOffset(0, with(LocalDensity.current) { 40.dp.roundToPx() }), onDismissRequest = onDismiss, properties = androidx.compose.ui.window.PopupProperties(focusable = true)) {
        val sh = RoundedCornerShape(if (look.liquid) 22.dp else 18.dp)
        Column(
            Modifier.width(androidx.compose.foundation.layout.IntrinsicSize.Max).widthIn(min = 190.dp).graphicsLayer {
                val s = if (look.liquid) .35f + .65f * a.value else .9f + .1f * a.value
                scaleX = s; scaleY = s; alpha = a.value.coerceIn(0f, 1f)
                transformOrigin = androidx.compose.ui.graphics.TransformOrigin(0f, 0f)
            }.panel(look.copy(panel = look.panel2), sh).padding(6.dp),
            verticalArrangement = Arrangement.spacedBy(2.dp),
        ) {
            MenuItem("Reboot", Color.White, "restart") { onPick("restart") }
            MenuItem("Shut down", RED, "off") { onPick("off") }
        }
    }
}

@Composable
private fun MenuItem(text: String, color: Color, icon: String, onClick: () -> Unit) {
    val look = LocalLook.current
    val src = rememberSource()
    val pressed by src.collectIsPressedAsState()
    Row(
        Modifier.height(42.dp).fillMaxWidth().pressScale(look, src).clip(RoundedCornerShape(if (look.liquid) 14.dp else 12.dp))
            .background(if (pressed) Color.White.copy(alpha = .12f) else Color.Transparent).clickable(src, null) { onClick() }.padding(horizontal = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Canvas(Modifier.size(18.dp)) {
            val k = size.width / 24
            val st = Stroke(2.2f * k, cap = StrokeCap.Round, join = StrokeJoin.Round)
            if (icon == "restart") {
                drawArc(color, -45f, 315f, false, Offset(4 * k, 4 * k), androidx.compose.ui.geometry.Size(16 * k, 16 * k), style = st)
                drawPath(Path().apply { moveTo(20 * k, 4 * k); lineTo(20 * k, 9 * k); lineTo(15 * k, 9 * k) }, color, style = st)
            } else {
                drawLine(color, Offset(12 * k, 3 * k), Offset(12 * k, 12 * k), 2.2f * k, StrokeCap.Round)
                drawArc(color, -50f, 280f, false, Offset(4 * k, 5 * k), androidx.compose.ui.geometry.Size(16 * k, 16 * k), style = st)
            }
        }
        Spacer(Modifier.width(10.dp))
        T(text, 15.sp, FontWeight.Bold, color)
    }
}

@Composable
fun LivePill(vm: BoardVM) {
    val look = LocalLook.current
    val setup = serverHostOf(vm.set).isEmpty()
    val r = vm.reach
    val (text, c) = when {
        setup -> "SETUP" to Color.White.copy(alpha = .35f)
        r == null -> "..." to Color.White.copy(alpha = .35f)
        r -> "LIVE" to GREEN
        else -> "OFFLINE" to RED
    }
    val sh = RoundedCornerShape(99.dp)
    Row(
        Modifier.then(if (look.liquid) Modifier.glass(look, sh, 1.4f) else Modifier.background(Color.White.copy(alpha = .1f), sh))
            .padding(horizontal = if (look.liquid) 13.dp else 12.dp, vertical = if (look.liquid) 7.dp else 6.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        StatusDot(if (setup || r == null) null else if (r) "up" else "dn")
        Spacer(Modifier.width(8.dp))
        T(text, 12.sp, FontWeight.Bold, spacing = 1.2.sp)
    }
}

@Composable
fun GearButton(onClick: () -> Unit) {
    val look = LocalLook.current
    val src = rememberSource()
    val pressed by src.collectIsPressedAsState()
    val rot by animateFloatAsState(if (look.liquid && pressed) 75f else 0f, spring(.45f, 250f), label = "gear")
    val sz = if (look.liquid) 36.dp else 32.dp
    Box(
        Modifier.size(sz).pressScale(look, src).then(if (look.liquid) Modifier.glass(look, CircleShape, 1.4f) else Modifier.background(Color.White.copy(alpha = if (pressed) .2f else .1f), CircleShape))
            .clip(CircleShape).clickable(src, null) { onClick() },
        contentAlignment = Alignment.Center,
    ) {
        Canvas(Modifier.size(17.dp).rotate(rot).alpha(.85f)) {
            val k = size.width / 24
            val c = center
            val st = Stroke(2 * k, cap = StrokeCap.Round, join = StrokeJoin.Round)
            drawCircle(Color.White, 3 * k, c, style = st)
            // eight teeth around a ring
            drawCircle(Color.White, 7.2f * k, c, style = st)
            for (i in 0 until 8) {
                val ang = Math.toRadians(i * 45.0)
                val a = Offset(c.x + (7.2f * k) * Math.cos(ang).toFloat(), c.y + (7.2f * k) * Math.sin(ang).toFloat())
                val b = Offset(c.x + (10.6f * k) * Math.cos(ang).toFloat(), c.y + (10.6f * k) * Math.sin(ang).toFloat())
                drawLine(Color.White, a, b, 3.4f * k, StrokeCap.Round)
            }
        }
    }
}

// Dots under the name: one per server, + adds one
@Composable
fun ServerDots(vm: BoardVM, setDlg: (Dlg?) -> Unit) {
    val look = LocalLook.current
    Row(Modifier.padding(horizontal = 2.dp).offset(y = (-8).dp).height(16.dp).enter(1), horizontalArrangement = Arrangement.spacedBy(7.dp), verticalAlignment = Alignment.CenterVertically) {
        val l = vm.servers
        val curI = l.indexOfFirst { it.id == vm.cur }
        l.forEachIndexed { i, sv ->
            val on = sv.id == vm.cur
            val w by animateFloatAsState(if (on) 20f else 7f, tween(300), label = "dotw")
            Box(Modifier.size(w.dp, 7.dp).clip(RoundedCornerShape(4.dp)).background(if (on) look.acc else Color.White.copy(alpha = .28f)).clickable { if (!on) vm.switchServer(sv.id, if (i > curI) 1 else -1) })
        }
        T("+", 15.sp, FontWeight.SemiBold, alpha = .45f, modifier = Modifier.clickable { setDlg(Dlg.AddServer) }.padding(horizontal = 4.dp))
    }
}

// ---------- Monitoring panel ----------
@Composable
fun Monitoring(vm: BoardVM) {
    val look = LocalLook.current
    val set = vm.set
    var opens by remember { mutableIntStateOf(0) }
    LaunchedEffect(vm.monOpen) { if (vm.monOpen) opens++ }
    Column(Modifier.enter(2, vm.switchSeq)) {
        if (!set.monitoringFixed) {
            val src = rememberSource()
            val pressed by src.collectIsPressedAsState()
            val sh = RoundedCornerShape(if (look.liquid) 24.dp else if (look.sharp) 10.dp else 20.dp)
            val rot by animateFloatAsState(if (vm.monOpen) 180f else 0f, if (look.liquid) spring(.5f, 250f) else tween(350), label = "monchev")
            Row(
                Modifier.fillMaxWidth().pressScale(look, src, .965f).glass(look, sh, 1f, pressed).clip(sh).glint(look, pressed)
                    .clickable(src, null) { vm.toggleMon() }.padding(horizontal = 18.dp, vertical = 14.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                T("MONITORING", 13.sp, FontWeight.ExtraBold, spacing = 1.8.sp)
                T(if (vm.glancesOk == false) "NO DATA" else "", 10.sp, color = ORANGE, alpha = .9f, spacing = 1.sp, align = TextAlign.End, modifier = Modifier.weight(1f).padding(end = 10.dp))
                Chevron(Modifier.size(18.dp).rotate(rot).alpha(.8f))
            }
        }
        AnimatedVisibility(
            vm.monOpen || set.monitoringFixed,
            enter = expandVertically(tween(if (look.liquid) 600 else 400, easing = SOFT)) + fadeIn(tween(400)),
            exit = shrinkVertically(tween(400, easing = SOFT)) + fadeOut(tween(250)),
        ) {
            Column(Modifier.padding(top = if (set.monitoringFixed) 0.dp else 16.dp), verticalArrangement = Arrangement.spacedBy(16.dp)) {
                StatCards(vm, opens)
                NetCard(vm)
            }
        }
    }
}

@Composable
fun StatCards(vm: BoardVM, replay: Int) {
    val st = vm.stats
    val ids = buildList { add("cpu" to "CPU"); add("ram" to "RAM"); if (st?.storage != null) add("disk" to "STORAGE") }
    if (st == null) {
        // waiting for the first answer: the cards show 0
    }
    val exp = vm.expanded?.takeIf { e -> ids.any { it.first == e } }
    val rest = ids.filter { it.first != exp }
    Column(verticalArrangement = Arrangement.spacedBy(14.dp)) {
        if (exp != null) StatCard(vm, exp, ids.first { it.first == exp }.second, true, replay, 0, Modifier.fillMaxWidth())
        rest.chunked(2).forEachIndexed { ri, row ->
            Row(horizontalArrangement = Arrangement.spacedBy(14.dp)) {
                row.forEachIndexed { i, (id, label) -> StatCard(vm, id, label, false, replay, ri * 2 + i, Modifier.weight(1f)) }
                if (row.size == 1) Spacer(Modifier.weight(1f))
            }
        }
    }
}

@Composable
fun StatCard(vm: BoardVM, id: String, label: String, big: Boolean, replay: Int, index: Int, modifier: Modifier) {
    val look = LocalLook.current
    val st = vm.stats
    val pct = when (id) { "cpu" -> st?.cpu; "ram" -> st?.ram; else -> st?.storage?.percent } ?: 0.0
    val src = rememberSource()
    val pressed by src.collectIsPressedAsState()
    val sh = RoundedCornerShape(look.card())
    val w = LocalConfiguration.current.screenWidthDp
    Column(
        modifier.enter(index, replay).pressScale(look, src, if (big) .99f else .965f).glass(look, sh).clip(sh).glint(look, pressed)
            .clickable(src, null) { vm.toggleCard(id) }.padding(start = 12.dp, end = 12.dp, top = 18.dp, bottom = 14.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        val ring = if (big) minOf(w * .40f, 160f) else minOf(w * .34f, 150f)
        Ring(pct, Modifier.size(ring.dp), replay)
        Caps(label, 12.sp, .7f)
        if (big) Detail(vm, id)
    }
}

@Composable
fun NetCard(vm: BoardVM) {
    val look = LocalLook.current
    val sh = RoundedCornerShape(look.card())
    val n = vm.stats?.net
    Row(Modifier.fillMaxWidth().enter(3).glass(look, sh).padding(vertical = 18.dp, horizontal = 10.dp), horizontalArrangement = Arrangement.SpaceAround) {
        for ((arrow, c, v, l) in listOf(Quad("↓", BLUE, n?.down ?: 0.0, "DOWNLOAD"), Quad("↑", PEACH, n?.up ?: 0.0, "UPLOAD"))) {
            Column(horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(6.dp)) {
                T(arrow, 20.sp, color = c)
                T(formatBytes(v) + "/s", 20.sp, FontWeight.Bold)
                Caps(l, 11.sp, .65f)
            }
        }
    }
}

data class Quad<A, B, C, D>(val a: A, val b: B, val c: C, val d: D)

// ---------- Details of an enlarged card ----------
@Composable
fun Detail(vm: BoardVM, id: String) {
    val d = vm.details[id]
    val st = vm.stats ?: return
    val limits = vm.set.tempLimits
    val z = if (vm.zimaReady) vm.zSys else null
    val zd = if (vm.zimaReady) vm.zDrives else null
    fun tcol(t: Double, k: String): Color? { val l = limits[k] ?: return null; return if (t >= l.hot) RED else if (t >= l.warm) ORANGE else null }
    fun pc(v: Double?) = if (v == null) "-" else "${Math.round(v * 10) / 10.0}%"
    fun sz(b: Double) = formatBytes(b)
    Column(Modifier.fillMaxWidth().padding(start = 4.dp, end = 4.dp, top = 6.dp, bottom = 2.dp), verticalArrangement = Arrangement.spacedBy(16.dp)) {
        when (id) {
            "cpu" -> {
                val c = d as? CpuDetail
                if (c != null) {
                    val stats = ArrayList<Triple<String, String, Color?>>()
                    stats.add(Triple(pc(c.user), "USER", null)); stats.add(Triple(pc(c.system), "SYSTEM", null))
                    if (c.iowait != null) stats.add(Triple(pc(c.iowait), "IO WAIT", null))
                    c.load?.let { stats.add(Triple("%.2f".format(it[0]), "LOAD 1M", null)); stats.add(Triple("%.2f".format(it[1]), "LOAD 5M", null)); stats.add(Triple("%.2f".format(it[2]), "LOAD 15M", null)) }
                    stats.add(Triple("${c.nCores}", "CORES", null))
                    // ZimaOS: its CPU temperature when Glances has none, and the CPU power draw
                    (c.temp ?: z?.cpuTemp)?.let { stats.add(Triple("${Math.round(it)} °C", "TEMP", tcol(it, "cpu"))) }
                    z?.watts?.let { stats.add(Triple(if (it >= 100) "${Math.round(it)} W" else "%.1f W".format(it), "POWER", null)) }
                    if (c.hot != null && c.nCoreTemps > 1) stats.add(Triple("${Math.round(c.hot)} °C", "HOTTEST CORE", tcol(c.hot, "cpu")))
                    StatGrid(stats)
                    if (c.cores.isNotEmpty()) Column {
                        Dh("PER CORE")
                        Row(Modifier.fillMaxWidth().height(46.dp), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                            for (v in c.cores) Box(Modifier.weight(1f).fillMaxSize().clip(RoundedCornerShape(6.dp)).background(Color.White.copy(alpha = .08f)), contentAlignment = Alignment.BottomCenter) {
                                val h by animateFloatAsState((v / 100).toFloat().coerceIn(0f, 1f), tween(600), label = "core")
                                Box(Modifier.fillMaxWidth().fillMaxSize(h).clip(RoundedCornerShape(6.dp)).background(pctColor(v)))
                            }
                        }
                    }
                }
                Column { Dh("HISTORY"); Graph(vm.hist.cpu, vm.hist.t) }
                if (c != null) TopList("TOP CPU USERS", c.top.map { Triple(it.name, it.pct, "%.1f%%".format(it.pct)) })
            }
            "ram" -> {
                val r = d as? RamDetail
                if (r != null) {
                    BigVal(sz(r.used), "/ " + sz(r.total))
                    val stats = ArrayList<Triple<String, String, Color?>>()
                    stats.add(Triple(sz(r.available), "AVAILABLE", null))
                    r.cached?.let { stats.add(Triple(sz(it), "CACHED", null)) }
                    if (r.swapTotal != null && r.swapTotal > 0) stats.add(Triple(sz(r.swapUsed ?: 0.0) + " / " + sz(r.swapTotal), "SWAP", null))
                    StatGrid(stats)
                }
                Column { Dh("HISTORY"); Graph(vm.hist.ram, vm.hist.t) }
                if (r != null) TopList("TOP MEMORY USERS", r.top.map { Triple(it.name, it.bytes, sz(it.bytes) + " · " + "%.1f%%".format(it.pct)) })
            }
            else -> {
                val dd = d as? DiskDetail
                val sm = st.storage
                if (sm != null) {
                    BigVal(sz(sm.used), "/ ${sz(sm.size)} · ${sm.count} ${if (sm.count == 1) "drive" else "drives"}")
                    StatGrid(listOf(Triple(sz(sm.free), "FREE", null), Triple(pc(sm.percent), "USED", null)))
                }
                Column { Dh("HISTORY"); Graph(vm.hist.disk, vm.hist.t) }
                if (dd != null && (dd.nvme.isNotEmpty() || dd.hdd.isNotEmpty())) {
                    Column {
                        Dh("TEMPERATURES")
                        StatGrid(dd.nvme.mapIndexed { j, t -> Triple("${Math.round(t.value)} °C", "NVME ${j + 1}", tcol(t.value, "nvme")) } +
                            dd.hdd.mapIndexed { j, t -> Triple("${Math.round(t.value)} °C", Regex("(sd[a-z]+)", RegexOption.IGNORE_CASE).find(t.label)?.value?.uppercase() ?: "HDD ${j + 1}", tcol(t.value, "hdd")) })
                    }
                } else if (dd != null && zd.isNullOrEmpty()) {
                    Caps("NO DRIVE TEMPERATURES REPORTED BY GLANCES", 10.sp, .35f, Modifier.fillMaxWidth())
                }
                // ZimaOS: each drive's SMART health and temperature (no drivetemp module needed)
                if (!zd.isNullOrEmpty()) Column {
                    Dh("DRIVE HEALTH (ZIMAOS)")
                    for (x in zd) {
                        val k = if (x.path.contains("nvme") || x.name.startsWith("nvme")) "nvme" else if (x.type.equals("SSD", true)) "ssd" else "hdd"
                        val zs = ArrayList<Triple<String, String, Color?>>()
                        zs.add(Triple(if (x.health == false) "FAILING" else "OK", "SMART", if (x.health == false) RED else GREEN))
                        x.temp?.let { zs.add(Triple("${Math.round(it)} °C", "TEMP", tcol(it, if (k == "hdd") "hdd" else "nvme"))) }
                        zs.add(Triple(k.uppercase(), "TYPE", null))
                        Column(Modifier.fillMaxWidth().padding(bottom = 10.dp).clip(RoundedCornerShape(16.dp)).background(Color.White.copy(alpha = .05f)).padding(12.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                            Row(verticalAlignment = Alignment.Bottom) {
                                T(x.model.ifEmpty { x.name }, 15.sp, FontWeight.Bold, modifier = Modifier.weight(1f), maxLines = 1)
                                T(x.name + if (x.size > 0) " · ${sz(x.size)}" else "", 12.sp, alpha = .7f, maxLines = 1)
                            }
                            StatGrid(zs)
                        }
                    }
                }
                if (st.disks.isNotEmpty()) Column {
                    Dh("DRIVES")
                    st.disks.forEachIndexed { i, dk ->
                        val ds = ArrayList<Triple<String, String, Color?>>()
                        ds.add(Triple(sz(dk.free ?: (dk.size - dk.used)), "FREE", null))
                        ds.add(Triple(dk.device.ifEmpty { "-" }.split("/").last(), "DEVICE", null))
                        ds.add(Triple(dk.fstype.ifEmpty { "-" }, "TYPE", null))
                        dd?.io?.getOrNull(i)?.let { io -> ds.add(Triple(sz(io.read) + "/s", "READ", null)); ds.add(Triple(sz(io.write) + "/s", "WRITE", null)) }
                        val tp = dd?.temps?.getOrNull(i)
                        if (tp != null) {
                            val mx = tp.values.max()
                            ds.add(Triple("${Math.round(mx)} °C", if (tp.kind == "nvme") "NVME TEMP" else if (tp.values.size > 1) "HOTTEST HDD" else "HDD TEMP", tcol(mx, tp.kind)))
                        } else if (zd != null) app.board.Zima.driveFor(dk.device, zd)?.temp?.let { t ->
                            val k = if (dk.device.contains("nvme")) "nvme" else "hdd"
                            ds.add(Triple("${Math.round(t)} °C", "TEMP", tcol(t, k)))
                        }
                        Column(Modifier.fillMaxWidth().padding(bottom = 10.dp).clip(RoundedCornerShape(16.dp)).background(Color.White.copy(alpha = .05f)).padding(12.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                            Row(verticalAlignment = Alignment.Bottom) {
                                T(dk.name, 15.sp, FontWeight.Bold, modifier = Modifier.weight(1f), maxLines = 1)
                                T("${sz(dk.used)} / ${sz(dk.size)} · ${pc(dk.percent)}", 12.sp, alpha = .7f, maxLines = 1)
                            }
                            Bar(dk.percent / 100, pctColor(dk.percent), 8.dp)
                            StatGrid(ds)
                        }
                    }
                }
            }
        }
        Caps("TAP TO CLOSE", 10.sp, .35f, Modifier.fillMaxWidth())
    }
}

@Composable
private fun Dh(t: String) = Caps(t, 11.sp, .6f, Modifier.padding(start = 2.dp, end = 2.dp, bottom = 8.dp))

@Composable
private fun BigVal(a: String, b: String) {
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.Center, verticalAlignment = Alignment.Bottom) {
        T(a, 24.sp, FontWeight.ExtraBold)
        T(" $b", 14.sp, FontWeight.Bold, alpha = .6f, modifier = Modifier.padding(bottom = 3.dp))
    }
}

@Composable
private fun StatGrid(items: List<Triple<String, String, Color?>>) {
    val look = LocalLook.current
    BoxWithConstraints(Modifier.fillMaxWidth()) {
        val cols = maxOf(1, ((maxWidth.value + 8) / (92 + 8)).toInt())
        Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
            items.chunked(cols).forEach { row ->
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    for ((v, l, c) in row) {
                        Column(
                            Modifier.weight(1f).clip(RoundedCornerShape(if (look.liquid) 16.dp else 14.dp)).background(Color.White.copy(alpha = .06f)).padding(vertical = 10.dp, horizontal = 6.dp),
                            horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(4.dp),
                        ) {
                            T(v, 14.sp, FontWeight.Bold, c ?: Color.White, maxLines = 1)
                            Caps(l, 10.sp, .55f, spacing = .12f)
                        }
                    }
                    // the last row keeps the same widths (CSS auto-fit grid)
                    if (row.size < cols && items.size > cols) repeat(cols - row.size) { Spacer(Modifier.weight(1f)) }
                }
            }
        }
    }
}

@Composable
private fun Bar(frac: Double, c: Color, h: androidx.compose.ui.unit.Dp = 6.dp) {
    Box(Modifier.fillMaxWidth().height(h).clip(RoundedCornerShape(h / 2)).background(Color.White.copy(alpha = .1f))) {
        Box(Modifier.fillMaxWidth(frac.toFloat().coerceIn(0f, 1f)).height(h).clip(RoundedCornerShape(h / 2)).background(c))
    }
}

@Composable
private fun TopList(title: String, rows: List<Triple<String, Double, String>>) {
    if (rows.isEmpty()) return
    val mx = rows.maxOf { it.second }
    Column {
        Dh(title)
        for ((name, v, txt) in rows) {
            Row(Modifier.fillMaxWidth().padding(vertical = 5.dp, horizontal = 2.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                T(name, 13.sp, FontWeight.SemiBold, maxLines = 1, modifier = Modifier.fillMaxWidth(.36f))
                Box(Modifier.weight(1f)) { Bar(if (mx > 0) maxOf(3.0, v / mx * 100) / 100 else 0.0, GREEN) }
                T(txt, 12.sp, alpha = .85f, align = TextAlign.End, modifier = Modifier.widthIn(min = 84.dp), maxLines = 1)
            }
        }
    }
}

// History graph: the last 100 values, scaled to the highest
@Composable
fun Graph(v: List<Double>, tl: List<Long>) {
    Box(Modifier.fillMaxWidth().clip(RoundedCornerShape(16.dp)).background(Color.White.copy(alpha = .05f)).padding(10.dp)) {
        if (v.size < 2) {
            Box(Modifier.fillMaxWidth().height(80.dp), contentAlignment = Alignment.Center) { T("Collecting history...", 12.sp, alpha = .5f) }
            return@Box
        }
        val mx = v.max()
        val top = minOf(100.0, maxOf(20.0, Math.ceil(mx / 10) * 10 + 10))
        val c = pctColor(v.last())
        Column {
            Canvas(Modifier.fillMaxWidth().height(80.dp)) {
                val w = size.width; val h = size.height
                drawLine(Color.White.copy(alpha = .08f), Offset(0f, h / 2), Offset(w, h / 2), 1f, pathEffect = PathEffect.dashPathEffect(floatArrayOf(6f, 8f)))
                val line = Path()
                v.forEachIndexed { i, x ->
                    val px = i.toFloat() / (v.size - 1) * w
                    val py = h - (minOf(x, top) / top).toFloat() * (h - 8) - 4
                    if (i == 0) line.moveTo(px, py) else line.lineTo(px, py)
                }
                val fill = Path().apply { addPath(line); lineTo(w, h); lineTo(0f, h); close() }
                drawPath(fill, c.copy(alpha = .16f))
                drawPath(line, c, style = Stroke(2.dp.toPx(), join = StrokeJoin.Round))
            }
            val span = ((tl.lastOrNull() ?: 0L) - (tl.firstOrNull() ?: 0L)) / 1000
            Row(Modifier.fillMaxWidth().padding(top = 6.dp)) {
                T(if (span >= 90) "${Math.round(span / 60.0)} min ago" else "$span s ago", 10.sp, alpha = .5f, modifier = Modifier.weight(1f))
                T("scale 0-${top.toInt()}%", 10.sp, alpha = .5f, modifier = Modifier.weight(1f), align = TextAlign.Center)
                T("now", 10.sp, alpha = .5f, modifier = Modifier.weight(1f), align = TextAlign.End)
            }
        }
    }
}

// ---------- Service buttons ----------
@Composable
fun ServicesGrid(vm: BoardVM, drag: DragState, lists: Map<String, List<String>>, byId: Map<String, Entry>, wide: Boolean, box: String) {
    val look = LocalLook.current
    val set = vm.set
    val ids = lists[box].orEmpty()
    val cols = if (wide) 4 else set.columns
    val gap = if (wide) 6.dp else if (look.compact) 8.dp else if (cols == 3) 10.dp else 12.dp
    // in Widget layout: which buttons won't fit on the widget (counted in widget slots)
    val over = HashSet<String>()
    if (wide && box == "svc") {
        var used = 0
        for (id in ids) {
            val e = byId[id]
            val slots = if (e is Stack) (if (e.expand) e.members.size else if (e.members.isNotEmpty()) 1 else 0) else 1
            if (used >= set.widgetRows * 4 && slots > 0) over.add(id)
            used += slots
        }
    }
    val boxMod = Modifier.fillMaxWidth().onGloballyPositioned { drag.boxRects[box] = Rect(it.positionInRoot(), it.size.toSize()) }
    if (ids.isEmpty()) {
        val sh = RoundedCornerShape(if (box == "hid") 14.dp else 20.dp)
        Box(
            boxMod.heightIn(min = if (box == "hid") 46.dp else 90.dp).drawBehind {
                drawRoundRect(Color.White.copy(alpha = .16f), style = Stroke(1.dp.toPx(), pathEffect = PathEffect.dashPathEffect(floatArrayOf(8f, 6f))), cornerRadius = androidx.compose.ui.geometry.CornerRadius((if (box == "hid") 14.dp else 20.dp).toPx()))
            }.clip(sh).padding(16.dp),
            contentAlignment = Alignment.Center,
        ) {
            T(
                when { box == "hid" -> "Drag buttons here to hide them from the widget"; wide -> "Nothing on the widget yet. Drag buttons up from the panel below."; else -> "No buttons yet. Tap + below to add your first service." },
                if (box == "hid") 12.sp else 13.sp, alpha = .6f, align = TextAlign.Center,
            )
        }
        return
    }
    // stacks take a whole row; buttons fill rows of `cols`
    val rows = ArrayList<List<String>>()
    var cur = ArrayList<String>()
    for (id in ids) {
        if (!wide && id.startsWith("g:")) {
            if (cur.isNotEmpty()) { rows.add(cur); cur = ArrayList() }
            rows.add(listOf(id))
        } else {
            cur.add(id)
            if (cur.size == cols) { rows.add(cur); cur = ArrayList() }
        }
    }
    if (cur.isNotEmpty()) rows.add(cur)
    Column(
        boxMod.then(if (box == "hid") Modifier.drawBehind {
            drawRoundRect(Color.White.copy(alpha = .16f), style = Stroke(1.dp.toPx(), pathEffect = PathEffect.dashPathEffect(floatArrayOf(8f, 6f))), cornerRadius = androidx.compose.ui.geometry.CornerRadius(14.dp.toPx()))
        }.padding(6.dp) else Modifier),
        verticalArrangement = Arrangement.spacedBy(gap),
    ) {
        var idx = 0
        for (row in rows) {
            if (row.size == 1 && row[0].startsWith("g:") && !wide) {
                val g = byId[row[0]] as? Stack
                if (g != null) StackCard(vm, drag, g, lists[g.id].orEmpty().mapNotNull { byId[it] as? Service }, idx++)
                continue
            }
            Row(horizontalArrangement = Arrangement.spacedBy(gap)) {
                for (id in row) {
                    val e = byId[id] ?: continue
                    Box(
                        Modifier.weight(1f).enter(idx++, vm.switchSeq to vm.mode)
                            .onGloballyPositioned { val r = Rect(it.positionInRoot(), it.size.toSize()); drag.rects[id] = r; drag.pressRects[id] = r }
                            .alpha(if (drag.dragId == id) (if (look.liquid) .15f else .25f) else if (id in over) .4f else if (box == "hid") .7f else 1f)
                            .squish(look, drag.pressedId == id),
                    ) { EntryButton(vm, e, wide, state = dotState(vm, e)) }
                }
                repeat(cols - row.size) { Spacer(Modifier.weight(1f)) }
            }
        }
    }
}

@Composable
fun EntryButton(vm: BoardVM, e: Entry, wide: Boolean, ghost: Boolean = false, state: String?) {
    val look = LocalLook.current
    val set = vm.set
    val pressed = false
    if (wide) {
        val sh = RoundedCornerShape(look.wide())
        Row(
            Modifier.fillMaxWidth().height(34.dp).then(if (ghost) Modifier.background(Color(70, 84, 128).copy(alpha = .96f), sh) else Modifier.glass(look, sh, 1f, pressed))
                .padding(start = 6.dp, end = 15.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            EntryIcon(e, 14.dp, set.useLogos, vm.logoVer, 13.sp)
            Spacer(Modifier.width(4.dp))
            T(e.name + if (e is Stack && e.expand) " ×${e.members.size}" else "", 10.sp, FontWeight.Bold, maxLines = 1)
        }
        Box(Modifier.fillMaxWidth().height(34.dp).padding(end = 6.dp), contentAlignment = Alignment.CenterEnd) { StatusDot(state, 6.dp, glow = false) }
        return
    }
    val sh = RoundedCornerShape(look.svc())
    val c3 = set.columns == 3
    val pad = if (look.compact) 10.dp else if (c3) 12.dp else 14.dp
    Box(Modifier.fillMaxWidth()) {
        Column(
            Modifier.fillMaxWidth().then(if (ghost) Modifier.background(Color(70, 84, 128).copy(alpha = .96f), sh) else Modifier.glass(look, sh)).clip(sh)
                .padding(horizontal = if (look.compact) 12.dp else pad, vertical = pad),
            verticalArrangement = Arrangement.spacedBy(if (look.compact) 2.dp else 4.dp),
        ) {
            EntryIcon(e, if (look.compact) 22.dp else 26.dp, set.useLogos, vm.logoVer, if (look.compact) 18.sp else 22.sp)
            T(e.name, if (look.compact || c3) 13.sp else 15.sp, if (look.liquid) FontWeight.SemiBold else FontWeight.Bold, maxLines = 1)
            T((e as? Service)?.label ?: "", 11.sp, alpha = .55f, maxLines = 1)
        }
        Box(Modifier.align(Alignment.TopEnd).padding(pad)) { StatusDot(state) }
    }
}

// A stack in the dashboard: header (tap = open/close, hold = drag or menu) + its buttons
@Composable
fun StackCard(vm: BoardVM, drag: DragState, g: Stack, members: List<Service>, index: Int) {
    val look = LocalLook.current
    val open = vm.openG[g.id] == true
    val cols = vm.set.columns
    val gap = if (look.compact) 8.dp else if (cols == 3) 10.dp else 12.dp
    DisposableEffect(g.id) { onDispose { drag.boxRects.remove(g.id) } }
    Column(
        Modifier.fillMaxWidth().enter(index, vm.switchSeq)
            .onGloballyPositioned { drag.rects[g.id] = Rect(it.positionInRoot(), it.size.toSize()) }
            .alpha(if (drag.dragId == g.id) (if (look.liquid) .15f else .25f) else 1f),
    ) {
        Box(Modifier.onGloballyPositioned { drag.pressRects[g.id] = Rect(it.positionInRoot(), it.size.toSize()) }.squish(look, drag.pressedId == g.id)) {
            StackHeader(vm, g.copy(members = members), members.size)
        }
        AnimatedVisibility(open, enter = expandVertically(tween(if (look.liquid) 600 else 400, easing = SOFT)) + fadeIn(), exit = shrinkVertically(tween(400, easing = SOFT)) + fadeOut()) {
            Column(
                Modifier.fillMaxWidth().padding(top = 12.dp, bottom = if (look.liquid) 14.dp else 0.dp)
                    .onGloballyPositioned { drag.boxRects[g.id] = Rect(it.positionInRoot(), it.size.toSize()) },
                verticalArrangement = Arrangement.spacedBy(gap),
            ) {
                if (members.isEmpty()) Box(Modifier.fillMaxWidth().heightIn(min = 58.dp), contentAlignment = Alignment.Center) { T("Drag buttons here", 12.sp, alpha = .5f) }
                members.chunked(cols).forEachIndexed { ri, row ->
                    Row(horizontalArrangement = Arrangement.spacedBy(gap)) {
                        row.forEachIndexed { ci, s ->
                            Box(
                                Modifier.weight(1f).enter(ri * cols + ci, open)
                                    .onGloballyPositioned { val r = Rect(it.positionInRoot(), it.size.toSize()); drag.rects[s.id] = r; drag.pressRects[s.id] = r }
                                    .alpha(if (drag.dragId == s.id) .25f else 1f).squish(look, drag.pressedId == s.id),
                            ) { EntryButton(vm, s, false, state = dotState(vm, s)) }
                        }
                        repeat(cols - row.size) { Spacer(Modifier.weight(1f)) }
                    }
                }
            }
        }
    }
}

@Composable
fun StackHeader(vm: BoardVM, g: Stack, count: Int, ghost: Boolean = false) {
    val look = LocalLook.current
    val open = vm.openG[g.id] == true
    val sh = RoundedCornerShape(if (look.liquid) 24.dp else if (look.sharp) 10.dp else 20.dp)
    val rot by animateFloatAsState(if (open) 180f else 0f, if (look.liquid) spring(.5f, 250f) else tween(350), label = "gchev")
    Row(
        Modifier.fillMaxWidth().then(if (ghost) Modifier.background(Color(70, 84, 128).copy(alpha = .96f), sh) else Modifier.glass(look, sh)).clip(sh).padding(horizontal = 18.dp, vertical = 14.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Row(Modifier.weight(1f), verticalAlignment = Alignment.CenterVertically) {
            EntryIcon(g, 22.dp, vm.set.useLogos, vm.logoVer, 18.sp)
            Spacer(Modifier.width(10.dp))
            T(g.name, 13.sp, FontWeight.ExtraBold, spacing = 1.8.sp, maxLines = 1, modifier = Modifier.weight(1f, fill = false))
            Spacer(Modifier.width(10.dp))
            T("$count", 11.sp, FontWeight.Bold, alpha = .5f)
        }
        Spacer(Modifier.width(10.dp))
        StatusDot(dotState(vm, g))
        Spacer(Modifier.width(10.dp))
        Chevron(Modifier.size(18.dp).rotate(rot).alpha(.8f))
    }
}
