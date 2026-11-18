package app.board.ui

import androidx.compose.animation.core.Spring
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.spring
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.compositionLocalOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.composed
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import app.board.ACCENTS
import app.board.GLOW_LEVELS
import app.board.Settings
import app.board.THEMES

fun hex(h: String, alpha: Float = 1f): Color {
    val s = h.removePrefix("#")
    val v = s.toLong(16)
    return Color(((v shr 16) and 0xff).toInt(), ((v shr 8) and 0xff).toInt(), (v and 0xff).toInt()).copy(alpha = alpha)
}

fun rgb(triple: String, alpha: Float = 1f): Color {
    val p = triple.split(",").map { it.trim().toIntOrNull() ?: 0 }
    return Color(p[0], p[1], p[2]).copy(alpha = alpha)
}

// "rgba(30,38,64,.94)" or "#1e2640"
fun cssColor(c: String): Color {
    if (c.startsWith("#")) return hex(c)
    val p = c.substringAfter("(").substringBefore(")").split(",").map { it.trim() }
    return Color(p[0].toInt(), p[1].toInt(), p[2].toInt()).copy(alpha = p.getOrNull(3)?.toFloat() ?: 1f)
}

val GREEN = hex("#34e0a1")
val ORANGE = hex("#ffb454")
val RED = hex("#ff5d73")
val BLUE = hex("#5ac8fa")
val PEACH = hex("#ff9f6b")

fun pctColor(p: Double) = if (p >= 85) RED else if (p >= 60) ORANGE else GREEN

data class Look(
    val bg: Color,
    val g1: Color,
    val g2: Color,
    val acc: Color,
    val ink: Color,
    val panel: Color,
    val panel2: Color,
    val glowop: Float,
    val glass: Float,
    val movingBg: Boolean,
    val sharp: Boolean,
    val font: FontFamily,
    val compact: Boolean,
    val liquid: Boolean,
) {
    // corner radii (classic / sharp / Liquid Glass)
    fun card(): Dp = if (liquid) 30.dp else if (sharp) 12.dp else 24.dp
    fun svc(): Dp = if (liquid) 24.dp else if (sharp) 10.dp else 20.dp
    fun wide(): Dp = if (liquid) 12.dp else if (sharp) 6.dp else 10.dp
    fun modal(): Dp = if (liquid) 36.dp else if (sharp) 16.dp else 30.dp
    fun input(): Dp = if (liquid) 16.dp else if (sharp) 8.dp else 14.dp
    fun btn(): Dp = if (liquid) 16.dp else if (sharp) 8.dp else 14.dp
}

fun lookOf(s: Settings): Look {
    val t = THEMES[s.theme] ?: THEMES["aurora"]!!
    val a = ACCENTS[s.accent]
    val own = a?.c != null
    return Look(
        bg = hex(t.bg), g1 = rgb(t.g1), g2 = rgb(t.g2),
        acc = hex(if (own) a!!.c!! else t.acc), ink = hex(if (own) a!!.ink!! else t.ink),
        panel = cssColor(t.panel), panel2 = cssColor(t.panel2),
        glowop = if (t.noGlow) 0f else GLOW_LEVELS[s.glow] ?: 0.5f,
        glass = when (s.cardStyle) { "subtle" -> 0.04f; "strong" -> 0.12f; else -> 0.07f },
        movingBg = s.movingBg, sharp = s.corners == "sharp",
        font = when (s.font) { "serif" -> FontFamily.Serif; "rounded" -> FontFamily.SansSerif; else -> FontFamily.Default },
        compact = s.buttonSize == "compact", liquid = s.style == "glass",
    )
}

val LocalLook = compositionLocalOf { lookOf(Settings()) }

/**
 * Card / button surface. Classic: flat see-through fill with a hairline border.
 * Liquid Glass: top-lit gradient fill, a bright rim and a soft drop shadow.
 */
fun Modifier.glass(look: Look, shape: Shape, alphaMul: Float = 1f, pressed: Boolean = false): Modifier {
    val g = look.glass * alphaMul
    return if (!look.liquid) {
        this.background(Color.White.copy(alpha = if (pressed) 0.18f else g), shape)
            .border(1.dp, Color.White.copy(alpha = 0.1f), shape)
    } else {
        this.background(
                Brush.linearGradient(
                    0f to Color.White.copy(alpha = (g * 2.3f).coerceAtMost(1f)),
                    0.52f to Color.White.copy(alpha = g * 0.6f),
                    1f to Color.White.copy(alpha = g * 1.3f),
                    start = Offset(0f, 0f), end = Offset(400f, 1100f),
                ), shape,
            )
            .background(Color(0x10101830), shape)
            .border(
                1.dp,
                Brush.linearGradient(
                    0f to Color.White.copy(alpha = .62f), 0.28f to Color.White.copy(alpha = .14f), 0.48f to Color.White.copy(alpha = 0f),
                    0.68f to Color.White.copy(alpha = .06f), 1f to Color.White.copy(alpha = .34f),
                ), shape,
            )
    }
}

// Popups and sheets
fun Modifier.panel(look: Look, shape: Shape): Modifier =
    if (!look.liquid) this.background(look.panel, shape).border(1.dp, Color.White.copy(alpha = .14f), shape)
    else this.shadow(30.dp, shape).background(
        Brush.linearGradient(0f to Color.White.copy(alpha = .17f), .55f to Color.White.copy(alpha = .04f), 1f to Color.White.copy(alpha = .08f)), shape,
    ).background(look.panel.copy(alpha = .9f), shape).border(
        1.dp, Brush.linearGradient(0f to Color.White.copy(alpha = .62f), .35f to Color.White.copy(alpha = .1f), .55f to Color.Transparent, 1f to Color.White.copy(alpha = .4f)), shape,
    )

// Liquid Glass press: springy squish (classic: none)
fun Modifier.pressScale(look: Look, source: MutableInteractionSource, amount: Float = 0.94f): Modifier = composed {
    val pressed by source.collectIsPressedAsState()
    val sc by animateFloatAsState(if (look.liquid && pressed) amount else 1f, spring(dampingRatio = if (pressed) 0.8f else 0.45f, stiffness = Spring.StiffnessMediumLow), label = "press")
    this.graphicsLayer { scaleX = sc; scaleY = sc }
}

fun Modifier.squish(look: Look, pressed: Boolean, amount: Float = 0.92f): Modifier = composed {
    val sc by animateFloatAsState(if (look.liquid && pressed) amount else 1f, spring(dampingRatio = if (pressed) 0.8f else 0.45f, stiffness = Spring.StiffnessMediumLow), label = "squish")
    this.graphicsLayer { scaleX = sc; scaleY = sc }
}

fun shape(r: Dp) = RoundedCornerShape(r)

@Composable
fun rememberSource() = remember { MutableInteractionSource() }

// Draws a soft highlight where Liquid Glass is pressed
fun Modifier.glint(look: Look, pressed: Boolean): Modifier = if (!look.liquid || !pressed) this else this.drawWithContent {
    drawContent()
    drawRect(Brush.radialGradient(listOf(Color.White.copy(alpha = .28f), Color.Transparent), center = center, radius = size.maxDimension * .65f))
}
