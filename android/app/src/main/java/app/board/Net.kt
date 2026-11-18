package app.board

import android.content.Context
import android.content.SharedPreferences
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.Canvas
import android.graphics.Paint
import android.graphics.Rect
import androidx.security.crypto.EncryptedSharedPreferences
import androidx.security.crypto.MasterKey
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.async
import kotlinx.coroutines.awaitAll
import kotlinx.coroutines.cancelChildren
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.launch
import kotlinx.coroutines.suspendCancellableCoroutine
import kotlinx.coroutines.withContext
import okhttp3.Call
import okhttp3.Callback
import okhttp3.Headers
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import okhttp3.Response
import java.io.File
import java.io.IOException
import java.security.SecureRandom
import java.security.cert.X509Certificate
import java.util.concurrent.TimeUnit
import javax.net.ssl.SSLContext
import javax.net.ssl.TrustManager
import javax.net.ssl.X509TrustManager
import kotlin.coroutines.resume
import kotlin.coroutines.resumeWithException

// ---------- Logins (the iOS Keychain in the Scriptable version: encrypted storage on the phone) ----------
object Secrets {
    private var prefs: SharedPreferences? = null

    fun init(ctx: Context) {
        if (prefs != null) return
        prefs = try {
            val key = MasterKey.Builder(ctx).setKeyScheme(MasterKey.KeyScheme.AES256_GCM).build()
            EncryptedSharedPreferences.create(ctx, "board-keychain", key, EncryptedSharedPreferences.PrefKeyEncryptionScheme.AES256_SIV, EncryptedSharedPreferences.PrefValueEncryptionScheme.AES256_GCM)
        } catch (e: Exception) {
            ctx.getSharedPreferences("board-keychain-plain", Context.MODE_PRIVATE)
        }
    }

    fun has(k: String) = prefs?.contains(k) == true
    fun get(k: String): String? = prefs?.getString(k, null)
    fun set(k: String, v: String) { prefs?.edit()?.putString(k, v)?.apply() }
    fun remove(k: String) { prefs?.edit()?.remove(k)?.apply() }

    val LOGIN_KEYS = mapOf(
        "zimaos" to ("board-zima-user" to "board-zima-pass"),
        "proxmox" to ("board-pve-user" to "board-pve-pass"),
        "truenas" to ("board-truenas-user" to "board-truenas-pass"),
        "unraid" to ("board-unraid-user" to "board-unraid-pass"),
        "synology" to ("board-syno-user" to "board-syno-pass"),
        "omv" to ("board-omv-user" to "board-omv-pass"),
    )

    // Every server keeps its own logins. Server 1 ("s1") uses the names from before there were several servers.
    fun kname(base: String, id: String = Store.CUR) = if (id.isEmpty() || id == "s1") base else "$base@$id"
    fun loginKeys(platform: String, id: String = Store.CUR) = LOGIN_KEYS[platform]?.let { kname(it.first, id) to kname(it.second, id) }
    fun helperKey(id: String = Store.CUR) = kname("board-helper-key", id)
    fun gpassKey(id: String = Store.CUR) = kname("board-glances-pass", id)

    fun forgetServerKeys(id: String) {
        val names = mutableListOf(helperKey(id), gpassKey(id))
        for (p in LOGIN_KEYS.keys) loginKeys(p, id)!!.let { names.add(it.first); names.add(it.second) }
        names.forEach { remove(it) }
    }

    // Linux: "logged in" = the helper's secret key is saved
    fun hasLogin(platform: String, id: String = Store.CUR): Boolean {
        if (platform == "linux") return has(helperKey(id))
        val k = loginKeys(platform, id) ?: return false
        return has(k.first) && has(k.second)
    }

    fun forgetLogin(platform: String, id: String = Store.CUR) {
        if (platform == "linux") return remove(helperKey(id))
        loginKeys(platform, id)?.let { remove(it.first); remove(it.second) }
    }

    fun savedLogin(platform: String, id: String = Store.CUR): Pair<String, String> {
        val k = loginKeys(platform, id)!!
        return (get(k.first) ?: "") to (get(k.second) ?: "")
    }

    fun saveLogin(platform: String, user: String, pass: String, id: String = Store.CUR) {
        val k = loginKeys(platform, id) ?: return
        set(k.first, user); set(k.second, pass)
    }

    fun glancesPass(id: String = Store.CUR): String = get(gpassKey(id)) ?: ""
}

// ---------- HTTP ----------
class Resp(val code: Int, val body: String, val headers: Headers, val bytes: ByteArray?) {
    fun cookies(): String = headers.values("Set-Cookie").map { it.substringBefore(";") }.joinToString("; ")
    fun json(): Any = org.json.JSONTokener(body).nextValue().also { if (it !is org.json.JSONObject && it !is org.json.JSONArray) throw IOException("not JSON") }
}

object Net {
    // Accepts self-signed certificates (most NAS systems use one out of the box)
    private val trustAll = object : X509TrustManager {
        override fun checkClientTrusted(chain: Array<out X509Certificate>?, authType: String?) {}
        override fun checkServerTrusted(chain: Array<out X509Certificate>?, authType: String?) {}
        override fun getAcceptedIssuers(): Array<X509Certificate> = arrayOf()
    }
    val base: OkHttpClient by lazy {
        val ssl = SSLContext.getInstance("TLS")
        ssl.init(null, arrayOf<TrustManager>(trustAll), SecureRandom())
        OkHttpClient.Builder()
            .sslSocketFactory(ssl.socketFactory, trustAll)
            .hostnameVerifier { _, _ -> true }
            .build()
    }

    fun client(timeout: Double, follow: Boolean = true): OkHttpClient {
        val ms = (timeout * 1000).toLong()
        return base.newBuilder().callTimeout(ms, TimeUnit.MILLISECONDS).connectTimeout(ms, TimeUnit.MILLISECONDS)
            .readTimeout(ms, TimeUnit.MILLISECONDS).followRedirects(follow).followSslRedirects(follow).build()
    }

    suspend fun request(
        url: String,
        method: String = "GET",
        headers: Map<String, String> = emptyMap(),
        body: String? = null,
        contentType: String? = null,
        timeout: Double = 8.0,
        follow: Boolean = true,
        binary: Boolean = false,
    ): Resp {
        val rb = Request.Builder().url(url)
        for ((k, v) in headers) rb.header(k, v)
        val ct = contentType ?: headers["Content-Type"] ?: "application/octet-stream"
        when (method) {
            "GET" -> rb.get()
            "HEAD" -> rb.head()
            else -> rb.method(method, (body ?: "").toRequestBody(ct.toMediaType()))
        }
        val call = client(timeout, follow).newCall(rb.build())
        return suspendCancellableCoroutine { cont ->
            cont.invokeOnCancellation { call.cancel() }
            call.enqueue(object : Callback {
                override fun onFailure(call: Call, e: IOException) { if (cont.isActive) cont.resumeWithException(e) }
                override fun onResponse(call: Call, response: Response) {
                    try {
                        response.use {
                            val bytes = if (binary) it.body?.bytes() else null
                            val text = if (binary || method == "HEAD") "" else it.body?.string() ?: ""
                            cont.resume(Resp(it.code, text, it.headers, bytes))
                        }
                    } catch (e: Exception) { if (cont.isActive) cont.resumeWithException(e) }
                }
            })
        }
    }

    suspend fun get(url: String, timeout: Double = 8.0) = request(url, timeout = timeout)

    // ---------- Service status (up/down dots) ----------
    // Any HTTP answer (even an error page) counts as up. Refused or timed-out connections count as down.
    private val LISTENING = Regex("certificate|secure connection|ssl|tls|handshake|unexpected status line|bad response|malformed|protocol|could not be parsed|end of stream", RegexOption.IGNORE_CASE)

    suspend fun pingUrl(url: String, timeout: Double): Boolean = try {
        request(url, "HEAD", timeout = timeout)
        true
    } catch (e: Exception) {
        // self-signed https or a non-web port (like SSH) still means something is listening
        if (e is kotlinx.coroutines.CancellationException) throw e
        LISTENING.containsMatchIn(e.toString())
    }

    suspend fun checkServices(list: List<Service>, timeout: Double): Map<String, Boolean> = coroutineScope {
        val byUrl = HashMap<String, kotlinx.coroutines.Deferred<Boolean>>()
        for (s in list) if (!byUrl.containsKey(s.url)) byUrl[s.url] = async { pingUrl(s.url, timeout) }
        list.associate { it.id to byUrl[it.url]!!.await() }
    }

    // One place on the server: a quick HEAD request; some servers turn HEAD away, so a refusal that isn't a
    // timeout gets one ordinary GET before the place counts as not answering
    suspend fun reachProbe(url: String, timeout: Double): Boolean {
        val t0 = System.currentTimeMillis()
        if (pingUrl(url, timeout)) return true
        if (System.currentTimeMillis() - t0 >= timeout * 1000 - 200) return false
        return try {
            request(url, timeout = timeout); true
        } catch (e: Exception) {
            if (e is kotlinx.coroutines.CancellationException) throw e
            LISTENING.containsMatchIn(e.toString())
        }
    }

    // ---------- Is the server reachable? (the LIVE / OFFLINE status) ----------
    // Asks the usual places on the server at once; returns what answered first, or "" for none.
    suspend fun serverReachable(s: Settings, services: List<Service>, timeout: Double = 4.0): String {
        val host = serverHostOf(s)
        if (host.isEmpty()) return ""
        val urls = LinkedHashSet<String>()
        urls.add(serverUrlOf(s) + "/"); urls.add("http://$host/"); urls.add("https://$host/")
        if (PLATFORMS.containsKey(s.platform)) urls.add(platformBase(s.platform, s) + "/")
        if (s.monitoring) urls.add(glancesUrl(s) + "/")
        for (sv in services) if (hostOf(sv.url) == host && urls.size < 12) urls.add(sv.url)
        fun label(u: String) = (Regex("^https?://[^/]+", RegexOption.IGNORE_CASE).find(u)?.value ?: u).replace(Regex("^https?://", RegexOption.IGNORE_CASE), "")
        return coroutineScope {
            val result = CompletableDeferred<String>()
            val jobs = urls.map { u -> async { if (reachProbe(u, timeout)) result.complete(label(u)) } }
            launch { jobs.awaitAll(); result.complete("") }
            val r = result.await()
            coroutineContext.cancelChildren()
            r
        }
    }
}

// ---------- Service logos (downloaded once, shrunk, stored on the phone) ----------
object Logos {
    const val LOGO_SIZE = 96
    const val LOGO_RETRY_HOURS = 12
    private lateinit var dir: File
    private val mem = HashMap<String, Bitmap>()

    fun init(ctx: Context) {
        dir = File(ctx.filesDir, "board-logos").apply { mkdirs() }
    }

    fun key(url: String): String {
        var h = 5381L
        for (c in url) h = (((h * 33) and 0xFFFFFFFFL) xor c.code.toLong()) and 0xFFFFFFFFL
        return java.lang.Long.toString(h, 36) + "-" + Integer.toString(url.length, 36)
    }

    private fun path(url: String) = File(dir, key(url) + ".png")

    @Synchronized fun cached(url: String?): Bitmap? {
        if (url.isNullOrEmpty()) return null
        mem[url]?.let { return it }
        val f = path(url)
        if (!f.exists()) return null
        return BitmapFactory.decodeFile(f.path)?.also { mem[url] = it }
    }

    fun clearFail(url: String) {
        if (url.isEmpty()) return
        File(path(url).path + ".fail").delete()
    }

    // Fit the logo inside a LOGO_SIZE square, keeping its shape and transparency
    private fun shrink(img: Bitmap): Bitmap {
        val scale = minOf(1f, LOGO_SIZE.toFloat() / maxOf(img.width, img.height))
        val w = maxOf(1, Math.round(img.width * scale))
        val h = maxOf(1, Math.round(img.height * scale))
        val side = maxOf(w, h)
        val out = Bitmap.createBitmap(side, side, Bitmap.Config.ARGB_8888)
        Canvas(out).drawBitmap(img, null, Rect((side - w) / 2, (side - h) / 2, (side - w) / 2 + w, (side - h) / 2 + h), Paint(Paint.FILTER_BITMAP_FLAG))
        return out
    }

    // Downloads a logo if it isn't saved yet. Returns true only when something new was saved.
    suspend fun fetch(url: String, timeout: Double): Boolean = withContext(Dispatchers.IO) {
        if (!Regex("^https?://", RegexOption.IGNORE_CASE).containsMatchIn(url)) return@withContext false
        val p = path(url)
        val fail = File(p.path + ".fail")
        if (p.exists()) return@withContext false
        if (fail.exists() && System.currentTimeMillis() - fail.lastModified() < LOGO_RETRY_HOURS * 3600 * 1000L) return@withContext false
        try {
            val r = Net.request(url, timeout = timeout, binary = true)
            if (r.code >= 400) throw IOException("HTTP ${r.code}")
            val img = BitmapFactory.decodeByteArray(r.bytes, 0, r.bytes!!.size) ?: throw IOException("not an image")
            p.outputStream().use { shrink(img).compress(Bitmap.CompressFormat.PNG, 100, it) }
            fail.delete()
            true
        } catch (e: Exception) {
            if (e is kotlinx.coroutines.CancellationException) throw e
            try { fail.writeText(System.currentTimeMillis().toString()) } catch (_: Exception) {}
            false
        }
    }

    suspend fun ensure(list: List<Entry>, timeout: Double): Boolean = coroutineScope {
        list.map { it.logo }.filter { it.isNotEmpty() }.distinct().map { async { fetch(it, timeout) } }.awaitAll().any { it }
    }

    // Removes saved logos that no button uses any more
    fun prune(keepUrls: Collection<String>) {
        val keep = keepUrls.filter { it.isNotEmpty() }.map { key(it) }.toSet()
        dir.listFiles()?.forEach { f -> if (f.name.substringBefore('.') !in keep) f.delete() }
    }

    @Synchronized fun clearAll() {
        dir.listFiles()?.forEach { it.delete() }
        mem.clear()
    }

    // Does a logo exist at this URL? ("Auto" in the add / edit form)
    suspend fun exists(url: String): Boolean = try {
        val r = Net.request(url, timeout = 8.0, binary = true)
        r.code < 400 && r.bytes != null && BitmapFactory.decodeByteArray(r.bytes, 0, r.bytes.size) != null
    } catch (e: Exception) { false }
}
