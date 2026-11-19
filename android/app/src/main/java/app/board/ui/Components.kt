package app.board.ui

import androidx.activity.compose.BackHandler
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.CubicBezierEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.spring
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.systemBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.TextUnit
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.zIndex
import app.board.Entry
import app.board.Logos
import kotlinx.coroutines.delay

val SOFT = CubicBezierEasing(.22f, 1f, .36f, 1f)

@Composable
fun T(
    text: String,
    size: TextUnit = 14.sp,
    weight: FontWeight = FontWeight.Normal,
    color: Color = Color.White,
    alpha: Float = 1f,
    spacing: TextUnit = TextUnit.Unspecified,
    align: TextAlign? = null,
    maxLines: Int = Int.MAX_VALUE,
    modifier: Modifier = Modifier,
    lineHeight: TextUnit = TextUnit.Unspecified,
) {
    Text(
        text, modifier, color = color.copy(alpha = color.alpha * alpha), fontSize = size, fontWeight = weight, letterSpacing = spacing,
        textAlign = align, maxLines = maxLines, overflow = TextOverflow.Ellipsis, fontFamily = LocalLook.current.font, lineHeight = lineHeight,
    )
}

// Small capitals label used across the app (".sec", ".lbl", "f label")
@Composable
fun Caps(text: String, size: TextUnit = 12.sp, alpha: Float = .6f, modifier: Modifier = Modifier, spacing: Float = .14f) =
    T(text, size, FontWeight.Bold, alpha = alpha, spacing = (size.value * spacing).sp, modifier = modifier, maxLines = 1)

// ---------- Background: two drifting glows (a third in Liquid Glass) ----------
@Composable
fun GlowBackground(look: Look) {
    val inf = rememberInfiniteTransition(label = "drift")
    val t by inf.animateFloat(0f, 1f, infiniteRepeatable(tween(if (look.liquid) 18000 else 14000, easing = CubicBezierEasing(.42f, 0f, .58f, 1f)), RepeatMode.Reverse), label = "t")
    val t2 by inf.animateFloat(1f, 0f, infiniteRepeatable(tween(14000, delayMillis = 0, easing = CubicBezierEasing(.42f, 0f, .58f, 1f)), RepeatMode.Reverse), label = "t2")
    val t3 by inf.animateFloat(0f, 1f, infiniteRepeatable(tween(23000, easing = CubicBezierEasing(.42f, 0f, .58f, 1f)), RepeatMode.Reverse), label = "t3")
    val op by animateFloatAsState(look.glowop, tween(400), label = "glowop")
    Canvas(Modifier.fillMaxSize().background(look.bg)) {
        if (op <= 0f) return@Canvas
        val vmax = maxOf(size.width, size.height) / 100f
        val r = (if (look.liquid) 110 else 95) * vmax / 2
        fun blob(c: Color, cx: Float, cy: Float, k: Float, rad: Float, alpha: Float) {
            drawCircle(Brush.radialGradient(0f to c.copy(alpha = .75f * alpha), .68f to c.copy(alpha = 0f), center = Offset(cx, cy), radius = rad * k), radius = rad * k, center = Offset(cx, cy))
        }
        val a = if (look.movingBg) t else 0f
        val b = if (look.movingBg) t2 else 0f
        blob(look.g1, -35 * vmax + r + 14 * vmax * a, -40 * vmax + r + 10 * vmax * a, 1f + .2f * a, r, op)
        blob(look.g2, size.width + 40 * vmax - r + 14 * vmax * b, size.height + 45 * vmax - r + 10 * vmax * b, 1f + .2f * b, r, op)
        if (look.liquid) {
            val c = if (look.movingBg) t3 else .5f
            val r3 = 75 * vmax / 2
            blob(look.acc.copy(alpha = .45f), size.width * .15f + r3 - 12 * vmax + 26 * vmax * c, size.height * .35f + r3 - 8 * vmax + 24 * vmax * c, .85f + .3f * c, r3, op * .6f)
        }
    }
}

// ---------- Ring gauge ----------
@Composable
fun Ring(pct: Double, modifier: Modifier, replayKey: Any? = null, labelSize: TextUnit = 30.sp) {
    val look = LocalLook.current
    val target = pct.coerceIn(0.0, 100.0).toFloat()
    val anim = remember(replayKey) { Animatable(0f) }
    LaunchedEffect(target, replayKey) { anim.animateTo(target, tween(if (look.liquid) 1400 else 1100, easing = SOFT)) }
    val num = remember(replayKey) { Animatable(0f) }
    LaunchedEffect(pct, replayKey) { num.animateTo(pct.toFloat(), tween(900, easing = SOFT)) }
    val c = pctColor(pct)
    Box(modifier, contentAlignment = Alignment.Center) {
        if (look.liquid) {
            Box(Modifier.fillMaxSize(.66f).clip(CircleShape).background(Brush.radialGradient(listOf(Color.White.copy(alpha = .16f), Color.White.copy(alpha = .02f)), center = Offset(30f, 25f), radius = 260f)).border(1.dp, Brush.verticalGradient(listOf(Color.White.copy(alpha = .28f), Color.Transparent)), CircleShape))
        }
        Canvas(Modifier.fillMaxSize()) {
            val sw = size.minDimension * 10f / 120f
            val rr = size.minDimension * 52f / 120f
            val tl = Offset(center.x - rr, center.y - rr)
            val sz = Size(rr * 2, rr * 2)
            drawCircle(Color.White.copy(alpha = if (look.liquid) .09f else .12f), rr, style = Stroke(sw))
            val sweep = 360f * anim.value / 100f
            if (sweep > 0.1f) {
                for ((w, a) in listOf(sw * 2.2f to .08f, sw * 1.6f to .14f)) drawArc(c.copy(alpha = a), -90f, sweep, false, tl, sz, style = Stroke(w, cap = StrokeCap.Round))
                drawArc(c, -90f, sweep, false, tl, sz, style = Stroke(sw, cap = StrokeCap.Round))
            }
        }
        Row(verticalAlignment = Alignment.Bottom) {
            T("${Math.round(num.value)}", labelSize, FontWeight.ExtraBold)
            T("%", (labelSize.value / 2).sp, FontWeight.Bold, alpha = .7f, modifier = Modifier.padding(bottom = (labelSize.value * .18f).dp))
        }
    }
}

// ---------- Switch ----------
@Composable
fun Switch(on: Boolean, onChange: (Boolean) -> Unit) {
    val look = LocalLook.current
    val src = rememberSource()
    val pressed by src.collectIsPressedAsState()
    val x by animateFloatAsState(if (on) 1f else 0f, if (look.liquid) spring(.55f, 400f) else tween(200), label = "sw")
    val knobW = if (look.liquid && pressed) 30.dp else 22.dp
    Box(
        Modifier.size(46.dp, 28.dp).clip(RoundedCornerShape(14.dp))
            .background(if (on) look.acc else if (look.liquid) Color(120, 120, 135).copy(alpha = .32f) else Color.White.copy(alpha = .18f))
            .clickable(src, null) { onChange(!on) },
    ) {
        Box(Modifier.offset(x = 3.dp + (46.dp - 6.dp - knobW) * x, y = 3.dp).size(knobW, 22.dp).clip(RoundedCornerShape(11.dp)).background(if (look.liquid && pressed) Color.White.copy(alpha = .6f) else Color.White))
    }
}

@Composable
fun ToggleRow(label: String, on: Boolean, onChange: (Boolean) -> Unit) {
    Row(Modifier.fillMaxWidth().padding(horizontal = 4.dp), verticalAlignment = Alignment.CenterVertically) {
        Caps(label, 11.sp, .6f, Modifier.weight(1f), .12f)
        Switch(on, onChange)
    }
}

// ---------- Radio row (".seg") ----------
@Composable
fun Seg(options: List<Pair<String, String>>, value: String, wrap: Boolean = false, onPick: (String) -> Unit) {
    val look = LocalLook.current
    val rows = if (wrap) options.chunked(3) else listOf(options)
    Column(
        Modifier.fillMaxWidth().glassOrFlat(look, RoundedCornerShape(if (look.liquid) 16.dp else 14.dp)).padding(3.dp),
        verticalArrangement = Arrangement.spacedBy(3.dp),
    ) {
        for (row in rows) Row(horizontalArrangement = Arrangement.spacedBy(3.dp)) {
            for ((k, name) in row) {
                val on = k == value
                val src = rememberSource()
                Row(
                    Modifier.weight(1f).height(38.dp).pressScale(look, src).clip(RoundedCornerShape(if (look.liquid) 13.dp else 11.dp))
                        .background(if (on) look.acc else Color.Transparent).clickable(src, null) { if (!on) onPick(k) },
                    horizontalArrangement = Arrangement.Center, verticalAlignment = Alignment.CenterVertically,
                ) {
                    Box(Modifier.size(14.dp).clip(CircleShape).border(if (on) 4.dp else 2.dp, if (on) look.ink else Color.White.copy(alpha = .5f), CircleShape).background(if (on) look.acc else Color.Transparent))
                    Spacer(Modifier.width(7.dp))
                    T(name, 14.sp, FontWeight.Bold, if (on) look.ink else Color.White, alpha = if (on) 1f else .75f, maxLines = 1)
                }
            }
            repeat(if (wrap) 3 - row.size else 0) { Spacer(Modifier.weight(1f)) }
        }
    }
}

fun Modifier.glassOrFlat(look: Look, shape: androidx.compose.ui.graphics.Shape) =
    if (look.liquid) this.glass(look, shape) else this.background(Color.White.copy(alpha = .09f), shape).border(1.dp, Color.White.copy(alpha = .12f), shape)

// ---------- Text input (".in") ----------
@Composable
fun Field(
    value: String,
    onChange: (String) -> Unit,
    placeholder: String = "",
    password: Boolean = false,
    keyboard: KeyboardType = KeyboardType.Text,
    maxLength: Int = 300,
    height: Dp = 44.dp,
    leading: (@Composable RowScope.() -> Unit)? = null,
    trailing: (@Composable RowScope.() -> Unit)? = null,
    modifier: Modifier = Modifier,
) {
    val look = LocalLook.current
    val sh = RoundedCornerShape(look.input())
    Row(
        modifier.fillMaxWidth().height(height).clip(sh)
            .background(Color.White.copy(alpha = if (look.liquid) .06f else .09f), sh)
            .border(1.dp, Color.White.copy(alpha = if (look.liquid) .14f else .12f), sh).padding(horizontal = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        leading?.invoke(this)
        Box(Modifier.weight(1f), contentAlignment = Alignment.CenterStart) {
            if (value.isEmpty()) T(placeholder, 16.sp, color = Color.White.copy(alpha = .3f), maxLines = 1)
            BasicTextField(
                value, { onChange(it.take(maxLength)) }, Modifier.fillMaxWidth(), singleLine = true,
                textStyle = TextStyle(color = Color.White, fontSize = 16.sp, fontFamily = look.font),
                cursorBrush = androidx.compose.ui.graphics.SolidColor(look.acc),
                visualTransformation = if (password) PasswordVisualTransformation() else VisualTransformation.None,
                keyboardOptions = KeyboardOptions(keyboardType = if (password) KeyboardType.Password else keyboard, autoCorrectEnabled = false, capitalization = androidx.compose.ui.text.input.KeyboardCapitalization.None),
            )
        }
        trailing?.invoke(this)
    }
}

@Composable
fun Labeled(label: String, content: @Composable ColumnScope.() -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(5.dp)) {
        Caps(label, 11.sp, .6f, Modifier.padding(start = 4.dp), .12f)
        content()
    }
}

// http:// / https:// list in front of an address
@Composable
fun SchemePick(value: String, onPick: (String) -> Unit) {
    var open by remember { mutableStateOf(false) }
    val look = LocalLook.current
    Box {
        Row(Modifier.clickable { open = true }, verticalAlignment = Alignment.CenterVertically) {
            T("$value://", 16.sp, alpha = .85f)
            T("▾", 11.sp, alpha = .5f, modifier = Modifier.padding(start = 3.dp, end = 8.dp))
        }
        DropdownMenu(open, { open = false }, Modifier.background(look.panel2)) {
            for (x in listOf("http", "https")) DropdownMenuItem({ T("$x://", 15.sp, FontWeight.Bold) }, { onPick(x); open = false })
        }
    }
}

// A dropdown that looks like an input (".in select")
@Composable
fun Select(options: List<Pair<String, String>>, value: String, onPick: (String) -> Unit) {
    var open by remember { mutableStateOf(false) }
    val look = LocalLook.current
    val sh = RoundedCornerShape(look.input())
    Box {
        Row(
            Modifier.fillMaxWidth().height(44.dp).clip(sh).background(Color.White.copy(alpha = if (look.liquid) .06f else .09f)).border(1.dp, Color.White.copy(alpha = .12f), sh)
                .clickable { open = true }.padding(horizontal = 12.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            T(options.find { it.first == value }?.second ?: value, 16.sp, alpha = .85f, modifier = Modifier.weight(1f), maxLines = 1)
            T("▾", 11.sp, alpha = .5f)
        }
        DropdownMenu(open, { open = false }, Modifier.background(look.panel2)) {
            for ((k, n) in options) DropdownMenuItem({ T(n, 15.sp, if (k == value) FontWeight.Bold else FontWeight.Normal, if (k == value) look.acc else Color.White) }, { onPick(k); open = false })
        }
    }
}

// ---------- Buttons ----------
enum class BtnKind { Plain, Ok, Del, GreenText, RedText }

@Composable
fun Btn(text: String, kind: BtnKind = BtnKind.Plain, modifier: Modifier = Modifier, enabled: Boolean = true, height: Dp = 44.dp, onClick: () -> Unit) {
    val look = LocalLook.current
    val src = rememberSource()
    val pressed by src.collectIsPressedAsState()
    val sh = RoundedCornerShape(look.btn())
    val h = if (look.liquid) maxOf(height, 48.dp) else height
    val bgMod = when (kind) {
        BtnKind.Ok -> Modifier.background(if (look.liquid) Brush.verticalGradient(0f to Color.White.copy(alpha = .42f), .58f to Color.Transparent) else Brush.verticalGradient(listOf(Color.Transparent, Color.Transparent)), sh).background(look.acc.copy(alpha = if (look.liquid) .85f else 1f), sh)
        BtnKind.Del -> Modifier.background(if (look.liquid) Brush.verticalGradient(0f to Color.White.copy(alpha = .38f), .58f to Color.Transparent) else Brush.verticalGradient(listOf(Color.Transparent, Color.Transparent)), sh).background(RED.copy(alpha = if (look.liquid) .85f else 1f), sh)
        else -> if (look.liquid) Modifier.glass(look, sh, 1.6f) else Modifier.background(Color.White.copy(alpha = if (pressed) .2f else .12f), sh)
    }
    Box(
        modifier.height(h).pressScale(look, src).then(bgMod).clip(sh).glint(look, pressed).alpha(if (enabled) 1f else .4f)
            .clickable(src, null, enabled = enabled) { onClick() },
        contentAlignment = Alignment.Center,
    ) {
        T(
            text, 15.sp, FontWeight.Bold,
            color = when (kind) { BtnKind.Ok -> look.ink; BtnKind.Del -> Color.White; BtnKind.GreenText -> look.acc; BtnKind.RedText -> RED; else -> Color.White },
            maxLines = 1, modifier = Modifier.padding(horizontal = 12.dp),
        )
    }
}

@Composable
fun Chip(text: String, modifier: Modifier = Modifier, onClick: () -> Unit) {
    val look = LocalLook.current
    val src = rememberSource()
    val pressed by src.collectIsPressedAsState()
    Box(
        modifier.pressScale(look, src).clip(RoundedCornerShape(99.dp))
            .then(if (look.liquid) Modifier.glass(look, RoundedCornerShape(99.dp), 1.8f) else Modifier.background(Color.White.copy(alpha = if (pressed) .26f else .14f)))
            .clickable(src, null) { onClick() }.padding(horizontal = 10.dp, vertical = 5.dp),
    ) { T(text, 12.sp, FontWeight.Bold, maxLines = 1) }
}

@Composable
fun Hint(text: String, modifier: Modifier = Modifier) = T(text, 11.sp, alpha = .45f, modifier = modifier.padding(horizontal = 4.dp), lineHeight = 15.sp)

@Composable
fun ErrText(text: String) = T(text, 12.sp, color = RED, align = TextAlign.Center, modifier = Modifier.fillMaxWidth().height(if (text.isEmpty()) 14.dp else androidx.compose.ui.unit.Dp.Unspecified))

// ---------- Modal popup (".modal" over a blurred, dimmed dashboard) ----------
@Composable
fun Modal(onDismiss: () -> Unit, wide: Boolean = false, top: Boolean = false, content: @Composable ColumnScope.() -> Unit) {
    val look = LocalLook.current
    BackHandler { onDismiss() }
    val enter = remember { Animatable(0f) }
    LaunchedEffect(Unit) { if (look.liquid) enter.animateTo(1f, spring(.62f, 300f)) else enter.animateTo(1f, tween(220, easing = SOFT)) }
    val scrim = remember { MutableInteractionSource() }
    Box(
        Modifier.fillMaxSize().zIndex(10f).background(Color(5, 10, 20).copy(alpha = if (look.liquid) .28f + .1f * enter.value else .55f * enter.value))
            .clickable(scrim, null) { onDismiss() }.systemBarsPadding().imePadding(),
        contentAlignment = if (top) Alignment.TopCenter else Alignment.Center,
    ) {
        val sh = RoundedCornerShape(look.modal())
        Column(
            Modifier.padding(horizontal = if (wide) 12.dp else 20.dp, vertical = if (top) 16.dp else 20.dp)
                .widthIn(max = if (wide) 460.dp else 340.dp).fillMaxWidth()
                .graphicsLayer {
                    val e = enter.value
                    val s0 = if (look.liquid) (if (top) .96f else .84f) else .9f
                    scaleX = s0 + (1 - s0) * e; scaleY = scaleX
                    alpha = e.coerceIn(0f, 1f)
                    translationY = if (look.liquid) (1 - e) * (if (top) 90.dp.toPx() else 18.dp.toPx()) else 0f
                }
                .panel(look, sh).clip(sh)
                .clickable(remember { MutableInteractionSource() }, null) {}
                .then(if (top) Modifier.verticalScroll(rememberScrollState()) else Modifier.verticalScroll(rememberScrollState()))
                .padding(start = 18.dp, end = 18.dp, top = if (wide) 14.dp else 22.dp, bottom = 18.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
            content = content,
        )
    }
}

@Composable
fun ModalTitle(text: String) = T(text, 17.sp, FontWeight.ExtraBold, spacing = 1.sp, align = TextAlign.Center, modifier = Modifier.fillMaxWidth())

@Composable
fun Note(text: String) = T(text, 13.sp, alpha = .65f, align = TextAlign.Center, modifier = Modifier.fillMaxWidth().padding(horizontal = 4.dp), lineHeight = 18.sp)

// ---------- Logo or emoji ----------
@Composable
fun EntryIcon(e: Entry, size: Dp, useLogos: Boolean, logoVer: Int, emojiSize: TextUnit) {
    val bmp = remember(e.logo, logoVer, useLogos) { if (useLogos) Logos.cached(e.logo)?.asImageBitmap() else null }
    if (bmp != null) Image(bmp, null, Modifier.size(size))
    else Text(e.icon, fontSize = emojiSize)
}

// Status dot (".sd"): null = unknown, "up", "dn", "part"
@Composable
fun StatusDot(state: String?, size: Dp = 8.dp, glow: Boolean = true) {
    val look = LocalLook.current
    val c = when (state) { "up" -> GREEN; "dn" -> RED; "part" -> ORANGE; else -> Color.White.copy(alpha = .25f) }
    Box(contentAlignment = Alignment.Center) {
        if (state != null && glow) Box(Modifier.size(size * (if (look.liquid) 2.6f else 1.8f)).background(Brush.radialGradient(listOf(c.copy(alpha = .55f), Color.Transparent)), CircleShape))
        Box(Modifier.size(size).clip(CircleShape).background(c))
    }
}

// ---------- Toast ----------
@Composable
fun Toast(msg: app.board.ToastMsg?, modifier: Modifier) {
    val look = LocalLook.current
    var shown by remember { mutableStateOf(false) }
    var text by remember { mutableStateOf("") }
    LaunchedEffect(msg) {
        if (msg == null) return@LaunchedEffect
        text = msg.text
        shown = msg.text.isNotEmpty()
        delay(msg.ms)
        shown = false
    }
    val a by animateFloatAsState(if (shown) 1f else 0f, if (look.liquid) spring(.6f, 300f) else tween(250), label = "toast")
    if (a > 0.01f) {
        val sh = RoundedCornerShape(if (look.liquid) 22.dp else 18.dp)
        Box(
            modifier.graphicsLayer { alpha = a.coerceIn(0f, 1f); translationY = (1 - a) * 40f; val s = if (look.liquid) .85f + .15f * a else 1f; scaleX = s; scaleY = s }
                .widthIn(max = 340.dp).panel(look.copy(panel = Color(30, 38, 64).copy(alpha = .96f)), sh).padding(horizontal = 16.dp, vertical = 10.dp),
        ) { T(text, 13.sp, FontWeight.SemiBold, align = TextAlign.Center) }
    }
}

// Liquid Glass entrance: rises into place with a spring, one after another
@Composable
fun Modifier.enter(index: Int, key: Any? = Unit): Modifier {
    val look = LocalLook.current
    if (!look.liquid) return this
    val a = remember(key) { Animatable(0f) }
    LaunchedEffect(key) {
        delay(index.coerceAtMost(18) * 32L)
        a.animateTo(1f, spring(.55f, 220f))
    }
    val d = LocalDensity.current
    return this.graphicsLayer {
        val v = a.value
        alpha = v.coerceIn(0f, 1f)
        translationY = (1 - v) * with(d) { 16.dp.toPx() }
        val s = .92f + .08f * v
        scaleX = s; scaleY = s
    }
}

@Composable
fun Centered(content: @Composable () -> Unit) = BoxWithConstraints(Modifier.fillMaxWidth(), contentAlignment = Alignment.Center) { content() }
