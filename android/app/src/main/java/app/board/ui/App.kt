package app.board.ui

import android.os.Build
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.spring
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.blur
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.zIndex
import app.board.BoardVM
import app.board.serverHostOf
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

@Composable
fun BoardApp(vm: BoardVM) {
    val look = lookOf(vm.lookPreview ?: vm.set)
    CompositionLocalProvider(LocalLook provides look) {
        var dlg by remember { mutableStateOf<Dlg?>(null) }
        var settings by remember { mutableStateOf(false) }
        var delName by remember { mutableStateOf<String?>(null) }
        var splash by rememberSaveable { mutableStateOf(vm.set.splash) }

        // first run: no server yet, so go straight to Settings (after the startup screen, when it's on)
        LaunchedEffect(Unit) {
            if (serverHostOf(vm.set).isEmpty()) { delay(if (vm.set.splash) 1700 else 400); settings = true }
        }
        // opened from a stack pill on the widget: open that stack
        LaunchedEffect(vm.focusStackReq) {
            val id = vm.focusStackReq ?: return@LaunchedEffect
            vm.mode = "dash"
            vm.openG[id] = true
            vm.focusStackReq = null
        }

        val blurOn = (dlg != null || settings || delName != null) && Build.VERSION.SDK_INT >= Build.VERSION_CODES.S
        Box(Modifier.fillMaxSize().background(look.bg)) {
            Box(Modifier.fillMaxSize().then(if (blurOn) Modifier.blur(8.dp) else Modifier)) {
                GlowBackground(look)
                Dashboard(vm, onSettings = { settings = true }, dlg = dlg, setDlg = { dlg = it })
            }
            when (val d = dlg) {
                is Dlg.AddService -> AddServiceDialog(vm, d.sv, { dlg = null }) { dlg = Dlg.ZimaPick }
                is Dlg.Menu -> MenuDialog(vm, d.sv, { dlg = null }, onStack = { dlg = Dlg.Pick(d.sv) }, onEdit = { dlg = Dlg.AddService(d.sv) }, onDelete = { dlg = null; vm.deleteService(d.sv.id) }) { dlg = Dlg.Logs(it) }
                is Dlg.Logs -> LogsDialog(vm, d.app) { dlg = null }
                Dlg.Updates -> UpdatesDialog(vm) { dlg = null }
                Dlg.ZimaPick -> ZimaPickDialog(vm) { dlg = null }
                is Dlg.Pick -> PickDialog(vm, d.sv, { dlg = null }) { dlg = Dlg.StackEdit(null, d.sv) }
                is Dlg.StackEdit -> StackDialog(vm, d.g, d.member) { dlg = null }
                Dlg.Rename -> RenameDialog(vm) { dlg = null }
                is Dlg.PowerConfirm -> androidx.compose.runtime.key(d) { PowerDialog(vm, d, { dlg = null }) { dlg = it } }
                Dlg.AddServer -> AddServerDialog(vm, { dlg = null }) { settings = true }
                null -> {}
            }
            if (settings) androidx.compose.runtime.key(vm.cur) {
                SettingsSheet(vm, { settings = false }) { delName = it }
            }
            delName?.let { name ->
                DelServerDialog(name, { delName = null }) {
                    delName = null
                    settings = false
                    vm.removeServer(vm.cur)
                }
            }
            Toast(vm.toast, Modifier.align(Alignment.BottomCenter).navigationBarsPadding().padding(bottom = 24.dp).zIndex(30f))
            if (splash) Splash { splash = false }
        }
    }
}

// ---------- startup screen (Settings > Theme & look > Startup animation) ----------
@Composable
fun Splash(onDone: () -> Unit) {
    val look = LocalLook.current
    val glow = remember { Animatable(0f) }
    val orb = remember { Animatable(0f) }
    val arc = remember { Animatable(0f) }
    val b = remember { Animatable(0f) }
    val name = remember { Animatable(0f) }
    val sweep = remember { Animatable(0f) }
    val out = remember { Animatable(0f) }
    var leaving by remember { mutableStateOf(false) }
    LaunchedEffect(Unit) {
        launch { glow.animateTo(1f, tween(2400, easing = SOFT)) }
        launch { orb.animateTo(1f, spring(.55f, 180f)) }
        launch { delay(200); arc.animateTo(1f, tween(1000, easing = SOFT)) }
        launch { delay(400); b.animateTo(1f, spring(.5f, 260f)) }
        launch { delay(450); name.animateTo(1f, tween(1000, easing = SOFT)) }
        launch { delay(550); sweep.animateTo(1f, tween(1100, easing = SOFT)) }
        delay(1350)
        leaving = true
        out.animateTo(1f, tween(800, easing = SOFT))
        onDone()
    }
    val scope = androidx.compose.runtime.rememberCoroutineScope()
    Box(
        Modifier.fillMaxSize().zIndex(100f)
            .graphicsLayer { alpha = 1f - out.value; val s = 1f + .25f * out.value; scaleX = s; scaleY = s }
            .then(if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S && out.value > 0f) Modifier.blur((16 * out.value).dp) else Modifier)
            .background(look.bg)
            .clickable(remember { MutableInteractionSource() }, null) { if (!leaving) { leaving = true; scope.launch { out.animateTo(1f, tween(600, easing = SOFT)); onDone() } } },
        contentAlignment = Alignment.Center,
    ) {
        Canvas(Modifier.fillMaxSize()) {
            val vmax = maxOf(size.width, size.height) / 100
            val r = 60 * vmax * (.4f + .6f * glow.value)
            val a = (look.glowop * 1.3f).coerceAtMost(1f) * glow.value
            drawCircle(Brush.radialGradient(0f to look.g1.copy(alpha = .7f * a), .6f to look.g1.copy(alpha = 0f), center = Offset(-50 * vmax + 60 * vmax, -60 * vmax + 60 * vmax), radius = r), r, Offset(10 * vmax, 0f))
            drawCircle(Brush.radialGradient(0f to look.g2.copy(alpha = .7f * a), .6f to look.g2.copy(alpha = 0f), center = Offset(size.width - 10 * vmax, size.height), radius = r), r, Offset(size.width - 10 * vmax, size.height))
        }
        Column(horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(26.dp)) {
            val sh = RoundedCornerShape((42 + 27 * (1 - orb.value.coerceIn(0f, 1f))).dp)
            Box(
                Modifier.size(138.dp).graphicsLayer { val s = .2f + .8f * orb.value; scaleX = s; scaleY = s; rotationZ = -25f * (1 - orb.value); alpha = orb.value.coerceIn(0f, 1f) }
                    .clip(sh).background(Brush.linearGradient(0f to Color.White.copy(alpha = .22f), .55f to Color.White.copy(alpha = .04f), 1f to Color.White.copy(alpha = .1f)))
                    .border(1.2.dp, Brush.linearGradient(0f to Color.White.copy(alpha = .8f), .35f to Color.White.copy(alpha = .1f), .55f to Color.Transparent, 1f to Color.White.copy(alpha = .45f)), sh),
                contentAlignment = Alignment.Center,
            ) {
                Canvas(Modifier.fillMaxSize().padding(13.dp)) {
                    val k = size.width / 120
                    val rr = 48 * k
                    val tl = Offset(center.x - rr, center.y - rr)
                    drawCircle(Color.White.copy(alpha = .12f), rr, style = Stroke(8 * k))
                    val sw = 360f * (1 - 52f / 301.6f) * arc.value
                    drawArc(look.acc.copy(alpha = .3f), -90f, sw, false, tl, Size(rr * 2, rr * 2), style = Stroke(14 * k, cap = StrokeCap.Round))
                    drawArc(look.acc, -90f, sw, false, tl, Size(rr * 2, rr * 2), style = Stroke(8 * k, cap = StrokeCap.Round))
                }
                // the light sweeping across the glass
                Canvas(Modifier.fillMaxSize()) {
                    val x = -0.8f * size.width + 2.3f * size.width * sweep.value
                    drawRect(Brush.horizontalGradient(listOf(Color.Transparent, Color.White.copy(alpha = .35f), Color.Transparent), startX = x, endX = x + size.width * .6f))
                }
                T("-b", 40.sp, FontWeight.Black, spacing = (-2).sp, modifier = Modifier.graphicsLayer { val s = .4f + .6f * b.value; scaleX = s; scaleY = s; alpha = b.value.coerceIn(0f, 1f) })
            }
            T(
                "-BOARD", 14.sp, FontWeight.ExtraBold, alpha = .85f * name.value, spacing = (14 * (.34f + (1.1f - .34f) * (1 - name.value))).sp,
                modifier = Modifier.then(if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S && name.value < 1f) Modifier.blur((8 * (1 - name.value)).dp) else Modifier),
            )
        }
    }
}
