package app.board

import android.appwidget.AppWidgetManager
import android.content.Context
import android.content.Intent
import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.Paint
import android.graphics.RadialGradient
import android.graphics.RectF
import android.graphics.Shader
import android.graphics.Typeface
import android.net.Uri
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.DpSize
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.glance.GlanceId
import androidx.glance.GlanceModifier
import androidx.glance.Image
import androidx.glance.ImageProvider
import androidx.glance.LocalSize
import androidx.glance.action.ActionParameters
import androidx.glance.action.clickable
import androidx.glance.appwidget.GlanceAppWidget
import androidx.glance.appwidget.GlanceAppWidgetManager
import androidx.glance.appwidget.GlanceAppWidgetReceiver
import androidx.glance.appwidget.SizeMode
import androidx.glance.appwidget.action.actionStartActivity
import androidx.glance.appwidget.cornerRadius
import androidx.glance.appwidget.provideContent
import androidx.glance.appwidget.updateAll
import androidx.glance.appwidget.state.updateAppWidgetState
import androidx.glance.background
import androidx.glance.layout.Alignment
import androidx.glance.layout.Box
import androidx.glance.layout.Column
import androidx.glance.layout.ContentScale
import androidx.glance.layout.Row
import androidx.glance.layout.Spacer
import androidx.glance.layout.fillMaxSize
import androidx.glance.layout.fillMaxWidth
import androidx.glance.layout.height
import androidx.glance.layout.padding
import androidx.glance.layout.size
import androidx.glance.layout.width
import androidx.glance.text.FontWeight as GFontWeight
import androidx.glance.text.Text
import androidx.glance.text.TextAlign as GTextAlign
import androidx.glance.text.TextStyle
import androidx.glance.unit.ColorProvider
import androidx.work.CoroutineWorker
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.PeriodicWorkRequestBuilder
import androidx.work.WorkManager
import androidx.work.WorkerParameters
import app.board.ui.Caps
import app.board.ui.LocalLook
import app.board.ui.Modal
import app.board.ui.T
import app.board.ui.hex
import app.board.ui.lookOf
import kotlinx.coroutines.async
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.withTimeoutOrNull
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import java.util.concurrent.TimeUnit
import androidx.compose.foundation.layout.Column as CColumn
import androidx.compose.ui.Modifier as CModifier
import androidx.compose.ui.graphics.Color as CColor

// One cell on the large widget: a button (tap = open it) or a stack pill (tap = the dashboard with that stack open)
data class Cell(val id: String, val name: String, val icon: String, val logo: Bitmap?, val url: String?, val stack: String?, val dot: String?)

data class WidgetData(
    val serverId: String,
    val title: String,
    val setup: Boolean,
    val reach: Boolean,
    val stats: Stats?,
    val monitoring: Boolean,
    val cells: List<Cell>,
    val upCount: Int,
    val total: Int,
    val checked: Boolean,
    val linkUrl: String,
    val time: String,
    val bg: Bitmap,
    val glass: Boolean,
)

class BoardWidget : GlanceAppWidget() {
    companion object {
        val SMALL = DpSize(110.dp, 110.dp)
        val MEDIUM = DpSize(250.dp, 110.dp)
        val LARGE = DpSize(250.dp, 260.dp)

        fun prefs(ctx: Context) = ctx.getSharedPreferences("board-widgets", Context.MODE_PRIVATE)

        suspend fun refresh(ctx: Context) = BoardWidget().updateAll(ctx)

        fun schedule(ctx: Context, minutes: Int) {
            val req = PeriodicWorkRequestBuilder<WidgetWorker>(maxOf(15, minutes).toLong(), TimeUnit.MINUTES).build()
            WorkManager.getInstance(ctx).enqueueUniquePeriodicWork("board-widget", ExistingPeriodicWorkPolicy.UPDATE, req)
        }
    }

    override val sizeMode = SizeMode.Responsive(setOf(SMALL, MEDIUM, LARGE))

    override suspend fun provideGlance(context: Context, id: GlanceId) {
        Store.init(context)
        val wid = try { GlanceAppWidgetManager(context).getAppWidgetId(id) } catch (e: Exception) { 0 }
        val serverId = Store.resolveServer(prefs(context).getString("w$wid", null))
        val data = withTimeoutOrNull(28000) { load(context, serverId) } ?: load(context, serverId, offlineOnly = true)
        provideContent { Content(context, data) }
    }

    private suspend fun load(ctx: Context, sid: String, offlineOnly: Boolean = false): WidgetData = coroutineScope {
        val s = Store.settings(sid)
        val title = s.widgetTitle.ifEmpty { Store.getTitle(sid) }
        val bg = background(s)
        val time = SimpleDateFormat("HH:mm", Locale.getDefault()).format(Date())
        if (serverHostOf(s).isEmpty() || offlineOnly) {
            return@coroutineScope WidgetData(sid, title, serverHostOf(s).isEmpty(), false, null, s.monitoring, emptyList(), 0, 0, false, s.linkUrl, time, bg, s.style == "glass")
        }
        val layout = Store.getLayout(sid)
        // everything runs side by side: is the server reachable, the buttons' checks and (with Monitoring) the stats
        val reachD = async { try { Net.serverReachable(s, layout.all) } catch (e: Exception) { "" } }
        val gridD = async {
            val list = Store.widgetCells(layout, s.widgetRows)
            val toCheck = list.flatMap { if (it is Stack) it.members else listOf(it as Service) }
            val statusD = async { if (s.widgetStatusDots) try { Net.checkServices(toCheck, minOf(s.pingTimeout, 3).toDouble()) } catch (e: Exception) { emptyMap() } else emptyMap() }
            if (s.useLogos) try { Logos.ensure(list, 4.0) } catch (_: Exception) {}
            val status = statusD.await()
            fun dot(e: Entry): String? {
                if (!s.widgetStatusDots) return null
                if (e !is Stack) return if (status[e.id] == true) "#34e0a1" else "#ff5d73"
                val up = e.members.count { status[it.id] == true }
                return if (up == e.members.size) "#34e0a1" else if (up == 0) "#ff5d73" else "#ffb454"
            }
            val cells = list.map { e ->
                Cell(e.id, e.name, e.icon, if (s.useLogos) Logos.cached(e.logo) else null, (e as? Service)?.url, if (e is Stack) e.id else null, dot(e))
            }
            Triple(cells, toCheck.count { status[it.id] == true }, toCheck.size)
        }
        val stats = if (s.monitoring) try { Glances(s, sid).getStats() } catch (e: Exception) { null } else null
        // Glances answering means the server is up, so there's no need to wait for the other checks then
        val reach = stats != null || reachD.await().isNotEmpty()
        val (cells, up, total) = gridD.await()
        WidgetData(sid, title, false, reach, stats, s.monitoring, cells, up, total, s.widgetStatusDots, s.linkUrl, time, bg, s.style == "glass")
    }

    private fun openApp(ctx: Context, server: String, stack: String? = null) = actionStartActivity(
        Intent(ctx, MainActivity::class.java)
            .setData(Uri.parse("board://open?server=" + Uri.encode(server) + (if (stack != null) "&stack=" + Uri.encode(stack) else "")))
            .putExtra("server", server).apply { if (stack != null) putExtra("stack", stack) }
            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_SINGLE_TOP),
    )

    private fun openUrl(url: String) = actionStartActivity(Intent(Intent.ACTION_VIEW, Uri.parse(url)).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))

    private fun white(a: Float = 1f) = ColorProvider(CColor.White.copy(alpha = a))
    private fun col(h: String) = ColorProvider(hex(h))

    @Composable
    private fun Content(ctx: Context, d: WidgetData) {
        val size = LocalSize.current
        val small = size.width < 200.dp
        val large = !small && size.height >= 220.dp
        Column(
            GlanceModifier.fillMaxSize().background(ImageProvider(d.bg), ContentScale.Crop).padding(horizontal = 14.dp, vertical = 12.dp).clickable(openApp(ctx, d.serverId)),
        ) {
            // not set up yet: ask to open the app
            if (d.setup) {
                Text(d.title, style = TextStyle(white(), 12.sp, GFontWeight.Bold))
                Spacer(GlanceModifier.defaultWeight())
                Text("Tap to set up -board:\nenter your server's address in Settings.", style = TextStyle(white(.75f), 12.sp))
                Spacer(GlanceModifier.defaultWeight())
                return@Column
            }
            // header: name, status pill, corner button
            Row(GlanceModifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                Text(d.title, style = TextStyle(white(), 12.sp, GFontWeight.Bold), maxLines = 1, modifier = GlanceModifier.defaultWeight())
                Row(pill(d.glass, 9.dp).padding(horizontal = 8.dp, vertical = 3.dp), verticalAlignment = Alignment.CenterVertically) {
                    Text("●", style = TextStyle(col(if (d.reach) "#34e0a1" else "#ff5d73"), 8.sp))
                    Spacer(GlanceModifier.width(4.dp))
                    Text(if (d.reach) d.time else "OFFLINE", style = TextStyle(white(), 10.sp, GFontWeight.Bold))
                }
                if (!small && d.linkUrl.isNotEmpty()) {
                    Spacer(GlanceModifier.width(6.dp))
                    Box(pill(d.glass, 12.dp, .14f).size(24.dp).clickable(openUrl(d.linkUrl)), contentAlignment = Alignment.Center) {
                        Text("↗", style = TextStyle(white(), 12.sp, GFontWeight.Bold))
                    }
                }
            }

            // Monitoring off: service buttons (medium / large) or a summary (small)
            if (!d.monitoring) {
                if (small) {
                    Spacer(GlanceModifier.defaultWeight())
                    Text(if (d.reach) "ONLINE" else "OFFLINE", style = TextStyle(col(if (d.reach) "#34e0a1" else "#ff5d73"), 20.sp, GFontWeight.Bold))
                    if (d.checked && d.total > 0) Text("${d.upCount} of ${d.total} services up", style = TextStyle(white(.75f), 11.sp, GFontWeight.Medium))
                    Spacer(GlanceModifier.defaultWeight())
                } else {
                    Spacer(GlanceModifier.height(10.dp))
                    val cells = if (large) d.cells else d.cells.take(8)
                    if (cells.isNotEmpty()) Grid(ctx, d, cells)
                    else Text(
                        if (d.reach) "Add service buttons in the dashboard with +." else "Can't reach the server.\nCheck the address in Settings, and your VPN if you're away from home.",
                        style = TextStyle(white(.7f), 11.sp),
                    )
                    Spacer(GlanceModifier.defaultWeight())
                }
                return@Column
            }

            val st = d.stats
            if (st == null) {
                Spacer(GlanceModifier.defaultWeight())
                Text(
                    if (d.reach) "The server is up, but Glances isn't answering.\nCheck the Glances address in Settings > Monitoring."
                    else "Can't reach the server.\nCheck the address in Settings, and your VPN if you're away from home.",
                    style = TextStyle(white(.7f), 11.sp),
                )
                Spacer(GlanceModifier.defaultWeight())
                if (large && d.cells.isNotEmpty()) Grid(ctx, d, d.cells)
                return@Column
            }

            Spacer(GlanceModifier.height(6.dp))
            // ring gauges
            val items = listOf("CPU" to st.cpu, "RAM" to st.ram) + (st.storage?.let { listOf("STORAGE" to it.percent) } ?: emptyList())
            val shown = if (small) items.take(2) else items
            Row(GlanceModifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                shown.forEachIndexed { i, (label, pct) ->
                    if (i > 0) Spacer(GlanceModifier.defaultWeight())
                    Column(horizontalAlignment = Alignment.CenterHorizontally) {
                        Image(ImageProvider(ringBitmap(pct, d.glass)), label, GlanceModifier.size(54.dp))
                        Spacer(GlanceModifier.height(2.dp))
                        Text(label, style = TextStyle(white(.7f), 9.sp, GFontWeight.Bold, textAlign = GTextAlign.Center))
                    }
                }
            }
            if (!small) {
                if (large) {
                    Spacer(GlanceModifier.height(10.dp))
                    Grid(ctx, d, d.cells)
                }
                Spacer(GlanceModifier.defaultWeight())
                Row(pill(d.glass, 9.dp, .08f).fillMaxWidth().padding(horizontal = 10.dp, vertical = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                    Text("↓ ", style = TextStyle(col("#5ac8fa"), 11.sp, GFontWeight.Bold))
                    Text("${formatBytes(st.net.down)}/s", style = TextStyle(white(), 11.sp, GFontWeight.Medium))
                    Spacer(GlanceModifier.defaultWeight())
                    Text("↑ ", style = TextStyle(col("#ff9f6b"), 11.sp, GFontWeight.Bold))
                    Text("${formatBytes(st.net.up)}/s", style = TextStyle(white(), 11.sp, GFontWeight.Medium))
                }
            }
        }
    }

    // Widget pill / button background. Classic: flat see-through fill. Liquid Glass: a brighter glass fill.
    private fun pill(glass: Boolean, r: androidx.compose.ui.unit.Dp, flat: Float = .1f) =
        GlanceModifier.background(ColorProvider(CColor.White.copy(alpha = if (glass) flat + .06f else flat))).cornerRadius(if (glass) r + 1.dp else r)

    // Service buttons grid for the widget (4 per row)
    @Composable
    private fun Grid(ctx: Context, d: WidgetData, cells: List<Cell>) {
        Column(GlanceModifier.fillMaxWidth()) {
            cells.chunked(4).forEachIndexed { ri, row ->
                Row(GlanceModifier.fillMaxWidth().padding(top = if (ri > 0) 5.dp else 0.dp)) {
                    for (j in 0 until 4) {
                        val c = row.getOrNull(j)
                        Box(GlanceModifier.defaultWeight().height(26.dp).padding(start = if (j > 0) 2.5.dp else 0.dp, end = if (j < 3) 2.5.dp else 0.dp)) {
                            if (c != null) {
                                val action = if (c.stack != null) openApp(ctx, d.serverId, c.stack) else openUrl(c.url ?: "")
                                Row(pill(d.glass, 8.dp).fillMaxSize().padding(start = 5.dp, end = 6.dp).clickable(action), verticalAlignment = Alignment.CenterVertically) {
                                    // logo / emoji in a fixed-size box, so every name starts at the same spot
                                    Box(GlanceModifier.size(13.dp), contentAlignment = Alignment.Center) {
                                        if (c.logo != null) Image(ImageProvider(c.logo), null, GlanceModifier.size(12.dp))
                                        else Text(c.icon, style = TextStyle(white(), 10.sp), maxLines = 1)
                                    }
                                    Spacer(GlanceModifier.width(3.dp))
                                    // the name takes the free space, which pushes the status dot to the right edge,
                                    // so the dots line up in a column on every row
                                    Text(c.name, GlanceModifier.defaultWeight(), style = TextStyle(white(), 9.sp, GFontWeight.Medium), maxLines = 1)
                                    if (c.dot != null) {
                                        Spacer(GlanceModifier.width(3.dp))
                                        Text("●", style = TextStyle(col(c.dot), 6.sp))
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

// ---------- Drawing ----------
// The widget background: the theme colour with its two glows
fun background(s: Settings): Bitmap {
    val t = THEMES[s.theme] ?: THEMES["aurora"]!!
    val strength = if (t.noGlow) 0f else (GLOW_LEVELS[s.glow] ?: .5f) / GLOW_LEVELS["normal"]!!
    val W = 660; val H = 310
    val b = Bitmap.createBitmap(W, H, Bitmap.Config.ARGB_8888)
    val c = Canvas(b)
    c.drawColor(hex(t.bg).toArgb())
    if (strength > 0) {
        fun glow(x: Float, y: Float, r: Float, rgb: String) {
            val col = app.board.ui.rgb(rgb)
            val p = Paint(Paint.ANTI_ALIAS_FLAG)
            p.shader = RadialGradient(x, y, r, intArrayOf(col.copy(alpha = (.33f * strength).coerceAtMost(1f)).toArgb(), col.copy(alpha = (.12f * strength).coerceAtMost(1f)).toArgb(), col.copy(alpha = 0f).toArgb()), floatArrayOf(0f, .5f, 1f), Shader.TileMode.CLAMP)
            c.drawCircle(x, y, r, p)
        }
        glow(90f, 20f, 280f, t.g1)
        glow(W - 40f, H + 20f, 320f, t.g2)
    }
    return b
}

// Ring gauge with a soft glow and the value in the middle (Liquid Glass: a glass lens in the middle)
fun ringBitmap(pct: Double, glass: Boolean, size: Int = 140): Bitmap {
    val b = Bitmap.createBitmap(size, size, Bitmap.Config.ARGB_8888)
    val c = Canvas(b)
    val cx = size / 2f
    val lw = size * .115f
    val r = size / 2f - lw * 1.1f
    val oval = RectF(cx - r, cx - r, cx + r, cx + r)
    val p = Paint(Paint.ANTI_ALIAS_FLAG).apply { style = Paint.Style.STROKE; strokeCap = Paint.Cap.ROUND }
    p.color = android.graphics.Color.argb(31, 255, 255, 255); p.strokeWidth = lw
    c.drawCircle(cx, cx, r, p)
    val color = hex(pctColorHex(pct))
    val sweep = 360f * maxOf(.01f, (minOf(pct, 100.0) / 100).toFloat())
    for ((w, a) in listOf(lw * 1.9f to .1f, lw * 1.45f to .16f, lw to 1f)) {
        p.color = color.copy(alpha = a).toArgb(); p.strokeWidth = w
        c.drawArc(oval, -90f, sweep, false, p)
    }
    if (glass) {
        val lr = r - lw * .95f
        c.drawCircle(cx, cx, lr, Paint(Paint.ANTI_ALIAS_FLAG).apply { this.color = android.graphics.Color.argb(18, 255, 255, 255) })
        c.drawCircle(cx, cx, lr, Paint(Paint.ANTI_ALIAS_FLAG).apply { style = Paint.Style.STROKE; strokeWidth = 1f; this.color = android.graphics.Color.argb(41, 255, 255, 255) })
    }
    val tp = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        this.color = android.graphics.Color.WHITE; textSize = size * .25f; textAlign = Paint.Align.CENTER
        typeface = Typeface.create(Typeface.DEFAULT, Typeface.BOLD)
    }
    c.drawText("${Math.round(pct)}%", cx, cx - (tp.descent() + tp.ascent()) / 2, tp)
    return b
}

class BoardWidgetReceiver : GlanceAppWidgetReceiver() {
    override val glanceAppWidget: GlanceAppWidget = BoardWidget()

    override fun onEnabled(context: Context) {
        super.onEnabled(context)
        Store.init(context)
        BoardWidget.schedule(context, Store.settings().refreshMinutes)
    }
}

class WidgetWorker(ctx: Context, params: WorkerParameters) : CoroutineWorker(ctx, params) {
    override suspend fun doWork(): Result {
        BoardWidget.refresh(applicationContext)
        return Result.success()
    }
}

// Picks the server a widget shows (the Parameter of the Scriptable widget)
class WidgetConfigActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val wid = intent?.extras?.getInt(AppWidgetManager.EXTRA_APPWIDGET_ID, AppWidgetManager.INVALID_APPWIDGET_ID) ?: AppWidgetManager.INVALID_APPWIDGET_ID
        setResult(RESULT_CANCELED, Intent().putExtra(AppWidgetManager.EXTRA_APPWIDGET_ID, wid))
        if (wid == AppWidgetManager.INVALID_APPWIDGET_ID) { finish(); return }
        Store.init(this)
        val servers = Store.listServers()
        val current = BoardWidget.prefs(this).getString("w$wid", null)
        fun pick(id: String) {
            BoardWidget.prefs(this).edit().putString("w$wid", id).apply()
            setResult(RESULT_OK, Intent().putExtra(AppWidgetManager.EXTRA_APPWIDGET_ID, wid))
            kotlinx.coroutines.MainScope().async {
                try {
                    val gid = GlanceAppWidgetManager(this@WidgetConfigActivity).getGlanceIdBy(wid)
                    BoardWidget().update(this@WidgetConfigActivity, gid)
                } catch (_: Exception) {}
                finish()
            }
        }
        setContent {
            CompositionLocalProvider(LocalLook provides lookOf(Store.settings())) {
                androidx.compose.foundation.layout.Box(CModifier.background(LocalLook.current.bg).statusBarsPadding()) {
                    Modal({ finish() }) {
                        app.board.ui.ModalTitle("WIDGET SERVER")
                        app.board.ui.Note("Which server should this widget show?")
                        CColumn(verticalArrangement = Arrangement.spacedBy(10.dp)) {
                            servers.forEachIndexed { i, s ->
                                val on = (current ?: servers[0].id) == s.id
                                app.board.ui.Btn("${i + 1}. ${s.title}", if (on) app.board.ui.BtnKind.Ok else app.board.ui.BtnKind.Plain, CModifier.fillMaxWidth()) { pick(s.id) }
                            }
                        }
                    }
                }
            }
        }
    }
}

@Suppress("unused") private fun unusedImports(p: ActionParameters?) = Unit
