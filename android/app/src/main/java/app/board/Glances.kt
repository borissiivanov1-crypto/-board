package app.board

import android.util.Base64
import kotlinx.coroutines.async
import kotlinx.coroutines.coroutineScope
import org.json.JSONArray
import org.json.JSONObject
import java.io.IOException

data class Disk(val name: String, val percent: Double, val used: Double, val size: Double, val free: Double?, val mnt: String, val device: String, val fstype: String)
data class Storage(val percent: Double, val used: Double, val size: Double, val free: Double, val count: Int)
data class NetRate(val down: Double, val up: Double)
data class Stats(val cpu: Double, val ram: Double, val disks: List<Disk>, val storage: Storage?, val net: NetRate)

data class TopRow(val name: String, val pct: Double, val bytes: Double = 0.0)
data class Temp(val label: String, val value: Double)
data class DriveTemp(val kind: String, val values: List<Double>)
data class Io(val read: Double, val write: Double)

sealed class Detail
data class CpuDetail(val user: Double?, val system: Double?, val iowait: Double?, val load: List<Double>?, val cores: List<Double>, val nCores: Int, val temp: Double?, val hot: Double?, val nCoreTemps: Int, val top: List<TopRow>) : Detail()
data class RamDetail(val total: Double, val used: Double, val available: Double, val cached: Double?, val swapUsed: Double?, val swapTotal: Double?, val top: List<TopRow>) : Detail()
data class DiskDetail(val io: List<Io?>, val temps: List<DriveTemp?>, val nvme: List<Temp>, val hdd: List<Temp>) : Detail()

// Mounts that aren't real drives: system folders, container layers, Proxmox's config and LXC file systems
val SKIP_MNT = Regex("^/(etc|run|dev|sys|proc|boot|snap|var/lib/(docker|lxcfs|containers)|var/snap)(/|$)")
val SKIP_FSTYPE = Regex("^(overlay|tmpfs|squashfs|devtmpfs|ramfs|nsfs|autofs|efivarfs|fuse|fuse\\.lxcfs|fuse\\.gvfsd-fuse|fuse\\.portal|fusectl)$")
// Virtual network interfaces (Docker, Proxmox VM / container taps and firewall bridges, Kubernetes, LXD)
val SKIP_IFACE = Regex("^(lo|docker|veth|br-|virbr|tap|fwbr|fwpr|fwln|vnet|cali|flannel|cni|kube|lxcbr|lxdbr)")

fun JSONObject.num(k: String): Double? = if (has(k) && !isNull(k)) opt(k).let { (it as? Number)?.toDouble() } else null
fun JSONArray?.objs(): List<JSONObject> = if (this == null) emptyList() else (0 until length()).mapNotNull { optJSONObject(it) }

fun isRealDrive(d: JSONObject) = !SKIP_MNT.containsMatchIn(d.optString("mnt_point")) && !SKIP_FSTYPE.containsMatchIn(d.optString("fs_type"))

// The same storage seen at several mount points counts once. ZFS datasets all share their pool's space.
fun driveKey(device: String, fsType: String) = if (fsType == "zfs") "zfs:" + device.split("/")[0] else device

class Glances(private val s: Settings, private val serverId: String = Store.CUR) {
    private var fsCache: JSONArray? = null
    private var fsTime = 0L

    suspend fun request(path: String, pass: String? = null, set: Settings = s): Any {
        val headers = HashMap<String, String>()
        if (set.username.isNotEmpty()) {
            val token = Base64.encodeToString("${set.username}:${pass ?: Secrets.glancesPass(serverId)}".toByteArray(), Base64.NO_WRAP)
            headers["Authorization"] = "Basic $token"
        }
        val r = Net.request("${glancesUrl(set)}/api/${set.apiVersion}/$path", headers = headers, timeout = 5.0)
        val j = r.json()
        if (r.code >= 400) throw IOException("Glances answered HTTP ${r.code}")
        return j
    }

    private suspend fun obj(path: String) = request(path) as? JSONObject ?: JSONObject()
    private suspend fun arr(path: String) = request(path) as? JSONArray ?: JSONArray()
    private suspend fun <T> safe(def: T, f: suspend () -> T): T = try { f() } catch (e: Exception) { if (e is kotlinx.coroutines.CancellationException) throw e; def }

    // disks / net = false skip what a view doesn't show. fsMaxAge (ms) reuses the last drive list for that long.
    suspend fun getStats(disks: Boolean = true, net: Boolean = true, fsMaxAge: Long = 0): Stats = coroutineScope {
        val fsFresh = disks && fsMaxAge > 0 && fsCache != null && System.currentTimeMillis() - fsTime < fsMaxAge
        val quickD = async { safe<JSONObject?>(null) { obj("quicklook") } }
        val fsD = async { if (!disks) null else if (fsFresh) fsCache else arr("fs") }
        val netD = async { if (net) arr("network") else null }
        val quick = quickD.await()
        val fs = fsD.await()
        val netA = netD.await()
        if (disks && !fsFresh) { fsCache = fs; fsTime = System.currentTimeMillis() }
        val cpu: Double
        val ram: Double
        if (quick != null && quick.num("cpu") != null && quick.num("mem") != null) {
            cpu = quick.num("cpu")!!; ram = quick.num("mem")!!
        } else {
            val c = async { obj("cpu") }
            val m = async { obj("mem") }
            cpu = c.await().num("total") ?: 0.0
            ram = m.await().num("percent") ?: 0.0
        }
        val dl = if (disks) pickDisks(fs ?: JSONArray()) else emptyList()
        Stats(cpu, ram, dl, sumStorage(dl), if (net) pickNetwork(netA ?: JSONArray()) else NetRate(0.0, 0.0))
    }

    fun pickDisks(fs: JSONArray): List<Disk> {
        val all = fs.objs()
        val list = if (s.diskMounts.isNotEmpty()) {
            s.diskMounts.mapNotNull { m -> all.find { it.optString("mnt_point") == m } }
        } else {
            val seen = HashSet<String>()
            all.filter { isRealDrive(it) }.sortedByDescending { it.num("size") ?: 0.0 }
                .filter { seen.add(driveKey(it.optString("device_name"), it.optString("fs_type"))) }
        }
        return list.take(if (s.diskMounts.isNotEmpty()) 12 else s.maxDisks).map { d ->
            val mnt = d.optString("mnt_point")
            Disk(
                name = if (mnt == "/") "Root" else mnt.split("/").lastOrNull { it.isNotEmpty() } ?: mnt,
                percent = d.num("percent") ?: 0.0, used = d.num("used") ?: 0.0, size = d.num("size") ?: 0.0, free = d.num("free"),
                mnt = mnt, device = d.optString("device_name"), fstype = d.optString("fs_type"),
            )
        }
    }

    fun pickNetwork(net: JSONArray): NetRate {
        val all = net.objs()
        fun total(n: JSONObject) = (n.num("bytes_recv") ?: n.num("rx") ?: 0.0) + (n.num("bytes_sent") ?: n.num("tx") ?: 0.0)
        val iface = if (s.networkInterface.isNotEmpty()) all.find { it.optString("interface_name") == s.networkInterface }
        else all.filter { !SKIP_IFACE.containsMatchIn(it.optString("interface_name")) }.maxByOrNull { total(it) }
        iface ?: return NetRate(0.0, 0.0)
        val dt = (iface.num("time_since_update") ?: 1.0).let { if (it == 0.0) 1.0 else it }
        return NetRate(
            iface.num("bytes_recv_rate_per_sec") ?: ((iface.num("rx") ?: 0.0) / dt),
            iface.num("bytes_sent_rate_per_sec") ?: ((iface.num("tx") ?: 0.0) / dt),
        )
    }

    // ---------- Details for the expanded CPU / RAM / storage cards ----------
    suspend fun getDetail(key: String, stats: Stats): Detail? = coroutineScope {
        when (key) {
            "cpu" -> {
                val cpuD = async { safe(JSONObject()) { obj("cpu") } }
                val perD = async { safe(JSONArray()) { arr("percpu") } }
                val loadD = async { safe<JSONObject?>(null) { obj("load") } }
                val procD = async { safe(JSONArray()) { arr("processlist") } }
                val sensD = async { safe(JSONArray()) { arr("sensors") } }
                val cpu = cpuD.await(); val per = perD.await().objs(); val load = loadD.await()
                val cores = (load?.num("cpucore")?.toInt()?.takeIf { it > 0 }) ?: per.size.takeIf { it > 0 } ?: 1
                val agg = LinkedHashMap<String, Double>()
                for (p in procD.await().objs()) {
                    val n = p.optString("name", "?").ifEmpty { "?" }
                    agg[n] = (agg[n] ?: 0.0) + (p.num("cpu_percent") ?: 0.0)
                }
                val top = agg.map { TopRow(it.key, it.value / cores) }.sortedByDescending { it.pct }.take(5)
                val t = parseTemps(sensD.await())
                val ct = cpuTemp(t)
                CpuDetail(cpu.num("user"), cpu.num("system"), cpu.num("iowait"),
                    load?.let { listOf(it.num("min1") ?: 0.0, it.num("min5") ?: 0.0, it.num("min15") ?: 0.0) },
                    per.map { it.num("total") ?: 0.0 }, cores, ct.first, ct.second, t.cores.size, top)
            }
            "ram" -> {
                val memD = async { obj("mem") }
                val swapD = async { safe<JSONObject?>(null) { obj("memswap") } }
                val procD = async { safe(JSONArray()) { arr("processlist") } }
                val mem = memD.await(); val swap = swapD.await()
                val total = mem.num("total") ?: 1.0
                fun rssOf(p: JSONObject): Double {
                    val mi = p.opt("memory_info")
                    var r = when (mi) {
                        is JSONArray -> (mi.opt(0) as? Number)?.toDouble() ?: 0.0
                        is JSONObject -> mi.num("rss") ?: 0.0
                        else -> 0.0
                    }
                    if (r == 0.0) p.num("memory_percent")?.let { r = it / 100 * total }
                    return r
                }
                val agg = LinkedHashMap<String, Double>()
                for (p in procD.await().objs()) {
                    val n = p.optString("name", "?").ifEmpty { "?" }
                    agg[n] = (agg[n] ?: 0.0) + rssOf(p)
                }
                val top = agg.map { TopRow(it.key, it.value / total * 100, it.value) }.sortedByDescending { it.bytes }.take(5)
                RamDetail(total, mem.num("used") ?: 0.0, mem.num("available") ?: 0.0, mem.num("cached"), swap?.num("used"), swap?.num("total"), top)
            }
            "disk" -> {
                val rows = safe(JSONArray()) { arr("diskio") }.objs()
                val io = stats.disks.map { disk ->
                    val base = disk.device.split("/").last()
                    val row = rows.find { it.optString("disk_name") == base } ?: return@map null
                    val dt = (row.num("time_since_update") ?: 1.0).let { if (it == 0.0) 1.0 else it }
                    Io(row.num("read_bytes_rate_per_sec") ?: ((row.num("read_bytes") ?: 0.0) / dt), row.num("write_bytes_rate_per_sec") ?: ((row.num("write_bytes") ?: 0.0) / dt))
                }
                val t = safe<Temps?>(null) { parseTemps(arr("sensors")) }
                DiskDetail(io, stats.disks.map { if (t != null) driveTemp(it, t) else null }, t?.nvme ?: emptyList(), t?.hdd ?: emptyList())
            }
            else -> null
        }
    }

    // Settings: the drive and network lists
    suspend fun mounts(): List<Triple<String, Double, Double>>? = safe(null) {
        arr("fs").objs().filter { it.optString("mnt_point").isNotEmpty() && isRealDrive(it) }.map { Triple(it.optString("mnt_point") + "|" + it.optString("device_name").split("/").last(), it.num("size") ?: 0.0, it.num("used") ?: 0.0) }
    }

    suspend fun ifaces(): List<String>? = safe(null) {
        arr("network").objs().map { it.optString("interface_name") }.filter { it.isNotEmpty() && !SKIP_IFACE.containsMatchIn(it) }
    }
}

// Total across all drives (drives that are the same filesystem are only counted once)
fun sumStorage(disks: List<Disk>): Storage? {
    if (disks.isEmpty()) return null
    val seen = HashSet<String>()
    var used = 0.0; var size = 0.0; var free = 0.0
    for (d in disks) {
        val k = if (d.fstype == "zfs") driveKey(d.device, "zfs") else "${d.device}|${d.size}|${d.used}"
        if (!seen.add(k)) continue
        used += d.used; size += d.size; free += d.free ?: (d.size - d.used)
    }
    return Storage(if (size > 0) used / size * 100 else 0.0, used, size, free, seen.size)
}

// ---------- Temperatures (from the Glances sensors plugin) ----------
data class Temps(val pkg: Double?, val cores: List<Double>, val nvme: List<Temp>, val hdd: List<Temp>, val other: List<Double>)

fun parseTemps(sens: JSONArray): Temps {
    val all = sens.objs().filter { x -> x.opt("value") is Number && (x.optString("unit") == "C" || Regex("temp", RegexOption.IGNORE_CASE).containsMatchIn(x.optString("type"))) }
    fun lb(x: JSONObject) = x.optString("label")
    fun isHdd(x: JSONObject) = x.optString("type") == "temperature_hdd" || Regex("^(drivetemp|hddtemp)|^/dev/sd|^sd[a-z]", RegexOption.IGNORE_CASE).containsMatchIn(lb(x))
    fun isNvme(x: JSONObject) = Regex("composite", RegexOption.IGNORE_CASE).containsMatchIn(lb(x)) || Regex("^nvme", RegexOption.IGNORE_CASE).containsMatchIn(lb(x))
    fun v(x: JSONObject) = (x.opt("value") as Number).toDouble()
    val hdd = all.filter { isHdd(it) }
    val nvme = all.filter { !isHdd(it) && isNvme(it) }
    val rest = all.filter { !isHdd(it) && !isNvme(it) }
    val pkg = rest.find { Regex("package|tdie|tctl|^cpu", RegexOption.IGNORE_CASE).containsMatchIn(lb(it)) }
    return Temps(
        pkg?.let { v(it) },
        rest.filter { Regex("^core", RegexOption.IGNORE_CASE).containsMatchIn(lb(it)) }.map { v(it) },
        nvme.map { Temp(lb(it), v(it)) },
        hdd.map { Temp(lb(it), v(it)) },
        rest.filter { !Regex("^sensor", RegexOption.IGNORE_CASE).containsMatchIn(lb(it)) }.map { v(it) },
    )
}

// (package temperature, hottest core)
fun cpuTemp(t: Temps): Pair<Double?, Double?> {
    val hottest = t.cores.maxOrNull()
    return (t.pkg ?: hottest ?: t.other.maxOrNull()) to hottest
}

// Match a mounted drive to its temperature sensor(s) by device name
fun driveTemp(disk: Disk, t: Temps): DriveTemp? {
    val dev = disk.device.split("/").last()
    Regex("^nvme(\\d+)n").find(dev)?.let { m -> return t.nvme.getOrNull(m.groupValues[1].toInt())?.let { DriveTemp("nvme", listOf(it.value)) } }
    Regex("^sd([a-z])").find(dev)?.let { m -> return t.hdd.getOrNull(m.groupValues[1][0] - 'a')?.let { DriveTemp("hdd", listOf(it.value)) } }
    if (dev.startsWith("md") && t.hdd.isNotEmpty()) return DriveTemp("hdd", t.hdd.map { it.value })
    return null
}
