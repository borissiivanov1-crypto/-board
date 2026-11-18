package app.board

import android.content.Context
import org.json.JSONArray
import org.json.JSONObject
import java.io.File

// ---------- Buttons and stacks ----------
interface Entry {
    val id: String
    val name: String
    val logo: String
    val icon: String
    val onWidget: Boolean
}

data class Service(
    override val id: String,
    override val name: String,
    val port: String,
    override val icon: String,
    override val logo: String,
    val url: String,
    val custom: Boolean,
    val scheme: String,
    val addr: String,
    val label: String,
    override val onWidget: Boolean,
    val group: String? = null,
    // the ZimaOS app this button was added from ("Pick from ZimaOS apps"); others are linked by port
    val zima: String = "",
) : Entry

data class Stack(
    override val id: String,
    override val name: String,
    override val logo: String,
    val expand: Boolean,
    val members: List<Service>,
    override val onWidget: Boolean,
    override val icon: String = STACK_ICON,
) : Entry

data class Group(var id: String, var name: String, var logo: String, var expand: Boolean, var members: MutableList<String>) {
    fun toJson() = JSONObject().put("id", id).put("name", name).put("logo", logo).put("expand", expand).put("members", JSONArray(members))
}

class BoardData(
    var title: String,
    var settings: JSONObject,
    var custom: MutableList<JSONObject>,
    var edits: JSONObject,
    var hidden: MutableList<String>,
    var order: MutableList<String>,
    var widgetOrder: MutableList<String>,
    var widgetHidden: MutableList<String>,
    var groups: MutableList<Group>,
)

data class Layout(val all: List<Service>, val groups: List<Stack>, val dash: List<Entry>, val widget: List<Entry>, val hidden: List<Entry>)

data class ServerRef(val id: String, val title: String)

fun isObj(x: Any?) = x is JSONObject
fun JSONArray?.strings(): MutableList<String> = if (this == null) mutableListOf() else (0 until length()).mapNotNull { opt(it) as? String }.toMutableList()

/**
 * The data file holds every server, in the same shape as the Scriptable version (so exports move between the two):
 *   { v: 2, current, settings (shared by all servers), servers: [ { id, title, settings (own), custom, edits, ... } ] }
 * Functions work on one server at a time: CUR unless an id is given (the widget passes its own).
 */
object Store {
    private lateinit var file: File
    private var dataRaw: String? = null
    @Volatile var CUR: String = "s1"

    // Built-in buttons. Empty on purpose: add your own with the + button in the dashboard.
    private val BUILTIN = emptyList<JSONObject>()

    fun init(ctx: Context) {
        if (::file.isInitialized) return
        file = File(ctx.filesDir, "board-data.json")
        CUR = readStore().optString("current", "s1")
        Secrets.init(ctx)
        Logos.init(ctx)
    }

    @Synchronized private fun readDataFile(): String {
        if (dataRaw == null) dataRaw = if (file.exists()) file.readText() else ""
        return dataRaw!!
    }

    @Synchronized fun readStore(): JSONObject {
        var d: Any = try {
            val raw = readDataFile()
            if (raw.isNotEmpty()) org.json.JSONTokener(raw).nextValue() else JSONObject()
        } catch (e: Exception) { JSONObject() }
        if (d is JSONArray) d = JSONObject().put("custom", d)
        if (d !is JSONObject) d = JSONObject()
        if (d.optJSONArray("servers") == null) {
            val st = d.optJSONObject("settings") ?: JSONObject()
            val own = JSONObject()
            val shared = JSONObject()
            for (k in st.keys()) (if (k in SERVER_SETTING_KEYS) own else shared).put(k, st.get(k))
            val s1 = JSONObject().put("id", "s1").put("title", d.opt("title") as? String ?: "").put("settings", own)
            for (k in SERVER_PARTS) if (d.has(k)) s1.put(k, d.get(k))
            d = JSONObject().put("v", 2).put("current", "s1").put("settings", shared).put("servers", JSONArray().put(s1))
        }
        if (d.optJSONObject("settings") == null) d.put("settings", JSONObject())
        val list = d.getJSONArray("servers")
        val keep = JSONArray()
        for (i in 0 until list.length()) {
            val x = list.opt(i)
            if (x is JSONObject && x.opt("id") is String) {
                if (x.optJSONObject("settings") == null) x.put("settings", JSONObject())
                keep.put(x)
            }
        }
        if (keep.length() == 0) keep.put(JSONObject().put("id", "s1").put("title", "").put("settings", JSONObject()))
        d.put("servers", keep)
        val cur = d.opt("current")
        if ((0 until keep.length()).none { keep.getJSONObject(it).getString("id") == cur }) d.put("current", keep.getJSONObject(0).getString("id"))
        return d
    }

    @Synchronized fun writeStore(st: JSONObject) {
        val raw = st.toString()
        file.writeText(raw)
        dataRaw = raw
    }

    @Synchronized fun importRaw(raw: String) {
        dataRaw = raw
        val st = readStore()
        writeStore(st)
        CUR = st.getString("current")
    }

    private fun servers(st: JSONObject): List<JSONObject> {
        val a = st.getJSONArray("servers")
        return (0 until a.length()).map { a.getJSONObject(it) }
    }

    private fun curServer(st: JSONObject, id: String = CUR): JSONObject {
        val l = servers(st)
        return l.find { it.getString("id") == id } ?: l.find { it.getString("id") == st.optString("current") } ?: l[0]
    }

    private fun titleAt(x: JSONObject, i: Int) = (x.opt("title") as? String)?.takeIf { it.isNotEmpty() } ?: if (i == 0) CONFIG.title else "SERVER ${i + 1}"

    fun listServers(): List<ServerRef> = servers(readStore()).mapIndexed { i, x -> ServerRef(x.getString("id"), titleAt(x, i)) }

    fun hasServer(id: String) = servers(readStore()).any { it.getString("id") == id }

    // A widget's server: its id, its name, or its number (1, 2 ...); else the first server
    fun resolveServer(want: String?): String {
        val l = servers(readStore())
        val w = want?.trim().orEmpty()
        if (w.isNotEmpty()) {
            val n = w.toIntOrNull()
            val hit = l.find { it.getString("id") == w }
                ?: (if (n != null && n >= 1 && n <= l.size) l[n - 1] else null)
                ?: l.withIndex().find { (i, x) -> ((x.opt("title") as? String)?.ifEmpty { null } ?: if (i == 0) CONFIG.title else "").equals(w, true) }?.value
            if (hit != null) return hit.getString("id")
        }
        return l[0].getString("id")
    }

    // Show another server (remembered for the next time the dashboard opens)
    @Synchronized fun setCurrentServer(id: String): Boolean {
        val st = readStore()
        if (servers(st).none { it.getString("id") == id }) return false
        CUR = id
        st.put("current", id)
        writeStore(st)
        return true
    }

    @Synchronized fun addServer(name: String, own: JSONObject): String {
        val st = readStore()
        val id = "s" + java.lang.Long.toString(System.currentTimeMillis(), 36)
        val n = st.getJSONArray("servers").length() + 1
        val settings = JSONObject().put("platform", "none")
        for (k in own.keys()) settings.put(k, own.get(k))
        st.getJSONArray("servers").put(JSONObject().put("id", id).put("title", name.trim().take(20).ifEmpty { "SERVER $n" }).put("settings", settings))
        writeStore(st)
        setCurrentServer(id)
        return id
    }

    // Removes a server, its buttons and layout, and the logins saved for it
    @Synchronized fun removeServer(id: String): Boolean {
        val st = readStore()
        val l = servers(st)
        if (l.size < 2) return false
        val i = l.indexOfFirst { it.getString("id") == id }
        if (i < 0) return false
        Secrets.forgetServerKeys(id)
        st.getJSONArray("servers").remove(i)
        val rest = servers(st)
        val next = rest[minOf(i, rest.size - 1)].getString("id")
        st.put("current", next)
        writeStore(st)
        CUR = next
        return true
    }

    @Synchronized fun loadData(id: String = CUR): BoardData {
        val st = readStore()
        val sv = curServer(st, id)
        val merged = JSONObject(st.getJSONObject("settings").toString())
        val own = sv.getJSONObject("settings")
        for (k in own.keys()) merged.put(k, own.get(k))
        val customArr = sv.optJSONArray("custom") ?: JSONArray()
        val custom = (0 until customArr.length()).mapNotNull { i ->
            val c = customArr.optJSONObject(i) ?: return@mapNotNull null
            if (c.optString("id").isEmpty()) c.put("id", "c:legacy$i")
            c
        }.toMutableList()
        val groupsArr = sv.optJSONArray("groups") ?: JSONArray()
        val groups = (0 until groupsArr.length()).mapNotNull { i ->
            val g = groupsArr.optJSONObject(i) ?: return@mapNotNull null
            if (g.opt("id") !is String) return@mapNotNull null
            Group(g.getString("id"), g.optString("name", "Stack").ifEmpty { "Stack" }.take(24), g.optString("logo", ""), g.optBoolean("expand", false), g.optJSONArray("members").strings())
        }.toMutableList()
        return BoardData(
            title = sv.opt("title") as? String ?: "",
            settings = merged,
            custom = custom,
            edits = sv.optJSONObject("edits") ?: JSONObject(),
            hidden = sv.optJSONArray("hidden").strings(),
            order = sv.optJSONArray("order").strings(),
            widgetOrder = sv.optJSONArray("widgetOrder").strings(),
            widgetHidden = sv.optJSONArray("widgetHidden").strings(),
            groups = groups,
        )
    }

    // Saves the current server's part (and the shared settings) back into the file
    @Synchronized fun saveData(d: BoardData, id: String = CUR) {
        val st = readStore()
        val sv = curServer(st, id)
        sv.put("title", d.title)
        sv.put("custom", JSONArray(d.custom))
        sv.put("edits", d.edits)
        sv.put("hidden", JSONArray(d.hidden))
        sv.put("order", JSONArray(d.order))
        sv.put("widgetOrder", JSONArray(d.widgetOrder))
        sv.put("widgetHidden", JSONArray(d.widgetHidden))
        sv.put("groups", JSONArray(d.groups.map { it.toJson() }))
        val own = JSONObject()
        val shared = st.getJSONObject("settings")
        for (k in d.settings.keys()) {
            if (k in SERVER_SETTING_KEYS) own.put(k, d.settings.get(k)) else shared.put(k, d.settings.get(k))
        }
        sv.put("settings", own)
        writeStore(st)
    }

    fun settings(id: String = CUR) = cleanSettings(loadData(id).settings)

    fun getTitle(id: String = CUR): String {
        val st = readStore()
        val l = servers(st)
        val sv = curServer(st, id)
        return titleAt(sv, l.indexOf(sv))
    }

    // All buttons: built-in (minus deleted, with edits applied), then custom.
    fun getServices(forDashboard: Boolean, id: String = CUR): List<Service> {
        val d = loadData(id)
        val host = serverHostOf(cleanSettings(d.settings)).ifEmpty { "localhost" }
        val base = BUILTIN.map { s ->
            val sid = "b:" + s.optString("name")
            val o = JSONObject(s.toString()).put("id", sid)
                .put("url", "${if (s.optBoolean("https")) "https" else "http"}://$host:${s.optString("port")}${s.optString("path")}")
            d.edits.optJSONObject(sid)?.let { e -> for (k in e.keys()) o.put(k, e.get(k)) }
            o
        }.filter { it.getString("id") !in d.hidden }
        val list = (base + d.custom).map { s ->
            val url = s.optString("url")
            val u = splitUrl(url)
            val port = s.opt("port")?.let { if (it == JSONObject.NULL) "" else it.toString() } ?: ""
            Service(
                id = s.getString("id"), name = s.optString("name"), port = port, icon = s.optString("icon", "🔗").ifEmpty { "🔗" },
                logo = s.optString("logo", ""), url = url, custom = s.optBoolean("custom"),
                scheme = u.scheme, addr = u.host + u.path, label = if (port.isNotEmpty()) ":$port" else u.host,
                onWidget = s.getString("id") !in d.widgetHidden,
                zima = s.optString("zima", ""),
            )
        }
        return if (forDashboard) sortByOrder(list, d.order) else list
    }

    fun <T : Entry> sortByOrder(list: List<T>, order: List<String>): List<T> {
        if (order.isEmpty()) return list
        fun idx(s: Entry) = order.indexOf(s.id).let { if (it < 0) Int.MAX_VALUE else it }
        return list.withIndex().sortedWith(compareBy({ idx(it.value) }, { it.index })).map { it.value }
    }

    // Top level = buttons not in a stack, plus stacks (each stack carries its member buttons).
    fun getLayout(id: String = CUR): Layout {
        val d = loadData(id)
        val all = getServices(false, id)
        val byId = all.associateBy { it.id }
        val inStack = HashSet<String>()
        val memberOf = HashMap<String, String>()
        val groups = d.groups.map { g ->
            val members = g.members.mapNotNull { byId[it] }.filter { it.id !in inStack }.distinctBy { it.id }
            for (s in members) { inStack.add(s.id); memberOf[s.id] = g.id }
            Stack(g.id, g.name, g.logo, g.expand, members.map { it.copy(group = g.id) }, g.id !in d.widgetHidden)
        }
        val allTagged = all.map { s -> memberOf[s.id]?.let { s.copy(group = it) } ?: s }
        val top: List<Entry> = allTagged.filter { it.id !in inStack } + groups
        return Layout(
            all = allTagged,
            groups = groups,
            dash = sortByOrder(top, d.order),
            widget = sortByOrder(top.filter { it.onWidget }, d.widgetOrder),
            hidden = top.filter { !it.onWidget },
        )
    }

    // What the large widget actually draws: stacks become one pill, or their members if "show members" is on
    fun widgetCells(layout: Layout, rows: Int): List<Entry> {
        val cells = ArrayList<Entry>()
        for (x in layout.widget) {
            if (x !is Stack) cells.add(x)
            else if (x.expand) cells.addAll(x.members)
            else if (x.members.isNotEmpty()) cells.add(x)
        }
        return cells.take(rows * 4)
    }

    // Create / edit / ungroup stacks and move buttons in or out of them
    data class StackAct(val act: String, val gid: String? = null, val member: String? = null, val name: String = "", val logo: String = "", val expand: Boolean = false, val id: String? = null, val to: String? = null)

    @Synchronized fun stackAction(a: StackAct) {
        val L = getLayout()
        val data = loadData()
        data.order = L.dash.map { it.id }.toMutableList()
        data.widgetOrder = L.widget.map { it.id }.toMutableList()
        fun strip(id: String) = data.groups.forEach { g -> g.members.removeAll { it == id } }
        fun holder(id: String) = data.groups.find { id in it.members }
        fun insertAt(list: List<String>, anchor: String?, ids: List<String>, after: Boolean): MutableList<String> {
            val out = list.filter { it !in ids }.toMutableList()
            var i = if (anchor != null) out.indexOf(anchor) else -1
            if (i < 0) return (out + ids).toMutableList()
            if (after) i++
            out.addAll(i, ids)
            return out
        }
        fun replaceIn(list: List<String>, oldId: String, ids: List<String>) = insertAt(list, oldId, ids, false).filter { it != oldId }.toMutableList()

        when (a.act) {
            "save" -> {
                val name = a.name.trim().take(24).ifEmpty { "Stack" }
                val logo = a.logo.trim()
                var g = data.groups.find { it.id == a.gid }
                if (g != null) {
                    g.name = name; g.logo = logo; g.expand = a.expand
                } else {
                    g = Group("g:" + System.currentTimeMillis(), name, logo, a.expand, mutableListOf())
                    if (a.member != null) {
                        val old = holder(a.member)
                        strip(a.member)
                        if (old != null) {
                            data.order = insertAt(data.order, old.id, listOf(g.id), true)
                            data.widgetOrder = insertAt(data.widgetOrder, old.id, listOf(g.id), true)
                        } else {
                            data.order = replaceIn(data.order, a.member, listOf(g.id))
                            data.widgetOrder = replaceIn(data.widgetOrder, a.member, listOf(g.id))
                        }
                        g.members.add(a.member)
                    } else {
                        data.order.add(g.id)
                        data.widgetOrder.add(g.id)
                    }
                    data.groups.add(g)
                }
                Logos.clearFail(logo)
            }
            "move" -> {
                val id = a.id ?: return
                val old = holder(id)
                strip(id)
                val to = a.to?.let { t -> data.groups.find { it.id == t } }
                if (to != null) {
                    to.members.add(id)
                    data.order.removeAll { it == id }
                    data.widgetOrder.removeAll { it == id }
                } else {
                    data.order = insertAt(data.order, old?.id, listOf(id), true)
                    data.widgetOrder = insertAt(data.widgetOrder, old?.id, listOf(id), true)
                }
            }
            "del" -> {
                val g = data.groups.find { it.id == a.gid }
                if (g != null) {
                    data.order = replaceIn(data.order, g.id, g.members)
                    data.widgetOrder = replaceIn(data.widgetOrder, g.id, g.members)
                    if (g.id in data.widgetHidden) {
                        data.widgetHidden = (data.widgetHidden.filter { it != g.id } + g.members).toMutableList()
                    }
                    data.groups.removeAll { it.id == g.id }
                }
            }
        }
        saveData(data)
    }

    // A button from the add / edit form
    fun makeCustom(name: String, schemeSel: String, addr0: String, port0: String, logo: String): JSONObject {
        var addr = addr0.trim()
        val m = Regex("^(https?)://", RegexOption.IGNORE_CASE).find(addr)
        val scheme = m?.groupValues?.get(1)?.lowercase() ?: if (schemeSel == "https") "https" else "http"
        addr = addr.replace(Regex("^https?://", RegexOption.IGNORE_CASE), "")
        var path = ""
        val slash = addr.indexOf('/')
        if (slash >= 0) { path = addr.substring(slash); addr = addr.substring(0, slash) }
        val port = port0.replace(Regex("[^0-9]"), "")
        return JSONObject().put("name", name.trim().take(24)).put("port", port).put("icon", "🔗").put("logo", logo.trim())
            .put("custom", true).put("url", "$scheme://$addr${if (port.isNotEmpty()) ":$port" else ""}$path")
    }

    fun saveService(editId: String?, name: String, scheme: String, addr: String, port: String, logo: String, onWidget: Boolean) {
        val data = loadData()
        val item = makeCustom(name, scheme, addr, port, logo)
        var id = editId
        if (editId != null && editId.startsWith("b:")) {
            data.edits.put(editId, JSONObject().put("name", item.get("name")).put("port", item.get("port")).put("url", item.get("url")).put("logo", item.get("logo")))
        } else if (editId != null) {
            val c = data.custom.find { it.optString("id") == editId }
            if (c != null) for (k in listOf("name", "port", "url", "logo")) c.put(k, item.get(k))
        } else {
            id = "c:" + System.currentTimeMillis()
            item.put("id", id)
            data.custom.add(item)
        }
        data.widgetHidden.removeAll { it == id }
        if (!onWidget && id != null) data.widgetHidden.add(id)
        saveData(data)
        Logos.clearFail(logo.trim())
    }

    // ZimaOS: add installed apps as buttons, with their real address and icon
    fun addZimaApps(apps: List<ZApp>, host: String) {
        if (apps.isEmpty()) return
        val data = loadData()
        var t = System.currentTimeMillis()
        for (a in apps) {
            val item = makeCustom(a.title, a.scheme, (a.host.ifEmpty { host }) + a.path, a.port, a.icon)
            item.put("id", "c:" + t++).put("zima", a.id)
            data.custom.add(item)
            Logos.clearFail(a.icon)
        }
        saveData(data)
    }

    fun deleteService(id: String) {
        val data = loadData()
        if (id.startsWith("b:")) {
            data.edits.remove(id)
            if (id !in data.hidden) data.hidden.add(id)
        } else data.custom.removeAll { it.optString("id") == id }
        data.groups.forEach { g -> g.members.removeAll { it == id } }
        saveData(data)
    }
}
