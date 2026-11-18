import Foundation

/**
 The data file holds every server, in the same shape as the Scriptable version (so exports move between the two):
   { v: 2, current, settings (shared by all servers), servers: [ { id, title, settings (own), custom, edits, ... } ] }
 Functions work on one server at a time: CUR unless an id is given (the widget passes its own).
 */
final class Store {
    static let shared = Store()
    var CUR = "s1"
    private var dataRaw: String?
    private let lock = NSRecursiveLock()
    // Built-in buttons. Empty on purpose: add your own with the + button in the dashboard.
    let BUILTIN: [J] = []

    private var file: URL { Shared.dir.appendingPathComponent("board-data.json") }

    init() { CUR = str(readStore(), "current") }

    func reload() { lock.lock(); dataRaw = nil; lock.unlock() }

    private func readDataFile() -> String {
        if dataRaw == nil { dataRaw = (try? String(contentsOf: file, encoding: .utf8)) ?? "" }
        return dataRaw!
    }

    func readStore() -> J {
        lock.lock(); defer { lock.unlock() }
        var d: Any = parseJSON(readDataFile()) ?? J()
        if let a = d as? [Any] { d = ["custom": a] as J }
        var st = d as? J ?? J()
        if !(st["servers"] is [Any]) {
            let s = st["settings"] as? J ?? J()
            var own = J(), shared = J()
            for (k, v) in s { if SERVER_SETTING_KEYS.contains(k) { own[k] = v } else { shared[k] = v } }
            var s1: J = ["id": "s1", "title": st["title"] as? String ?? "", "settings": own]
            for k in SERVER_PARTS { if let v = st[k] { s1[k] = v } }
            st = ["v": 2, "current": "s1", "settings": shared, "servers": [s1]]
        }
        if !(st["settings"] is J) { st["settings"] = J() }
        var servers = objs(st["servers"]).filter { $0["id"] is String }
        if servers.isEmpty { servers = [["id": "s1", "title": "", "settings": J()]] }
        servers = servers.map { x in var x = x; if !(x["settings"] is J) { x["settings"] = J() }; return x }
        st["servers"] = servers
        if !servers.contains(where: { str($0, "id") == str(st, "current") }) { st["current"] = servers[0]["id"] }
        return st
    }

    func writeStore(_ st: J) {
        lock.lock(); defer { lock.unlock() }
        let raw = toJSON(st)
        try? raw.write(to: file, atomically: true, encoding: .utf8)
        dataRaw = raw
    }

    // An export (from this app or the Scriptable version) that holds every server
    func importRaw(_ raw: String) {
        lock.lock(); defer { lock.unlock() }
        dataRaw = raw
        let st = readStore()
        writeStore(st)
        CUR = str(st, "current")
    }

    private func curIndex(_ st: J, _ id: String? = nil) -> Int {
        let l = objs(st["servers"])
        let want = id ?? CUR
        return l.firstIndex { str($0, "id") == want } ?? l.firstIndex { str($0, "id") == str(st, "current") } ?? 0
    }

    private func titleAt(_ x: J, _ i: Int) -> String {
        let t = str(x, "title")
        return !t.isEmpty ? t : i == 0 ? CONFIG.title : "SERVER \(i + 1)"
    }

    // [{ id, title }] in order, with the name each one shows
    func listServers() -> [J] { objs(readStore()["servers"]).enumerated().map { ["id": str($1, "id"), "title": titleAt($1, $0)] } }

    func hasServer(_ id: String) -> Bool { objs(readStore()["servers"]).contains { str($0, "id") == id } }

    // A widget's server: its id, its name, or its number (1, 2 ...); else the first server
    func resolveServer(_ want: String?) -> String {
        let l = objs(readStore()["servers"])
        let w = trim(want ?? "")
        if !w.isEmpty {
            if let hit = l.first(where: { str($0, "id") == w }) { return str(hit, "id") }
            if let n = Int(w), n >= 1, n <= l.count { return str(l[n - 1], "id") }
            for (i, x) in l.enumerated() {
                let t = str(x, "title").isEmpty ? (i == 0 ? CONFIG.title : "") : str(x, "title")
                if t.lowercased() == w.lowercased() { return str(x, "id") }
            }
        }
        return str(l[0], "id")
    }

    // Show another server (remembered for the next time the dashboard opens)
    @discardableResult func setCurrentServer(_ id: String) -> Bool {
        var st = readStore()
        guard objs(st["servers"]).contains(where: { str($0, "id") == id }) else { return false }
        CUR = id
        st["current"] = id
        writeStore(st)
        return true
    }

    // own: the new server's settings (address, type ...)
    func addServer(_ name: String, _ own: J) -> String {
        var st = readStore()
        let id = "s" + String(Int(nowMs()), radix: 36)
        var servers = objs(st["servers"])
        var settings: J = ["platform": "none"]
        for (k, v) in own { settings[k] = v }
        let t = clip(trim(name), 20)
        servers.append(["id": id, "title": t.isEmpty ? "SERVER \(servers.count + 1)" : t, "settings": settings])
        st["servers"] = servers
        writeStore(st)
        setCurrentServer(id)
        return id
    }

    // Removes a server, its buttons and layout, and the logins saved for it
    func removeServer(_ id: String) -> Bool {
        var st = readStore()
        var servers = objs(st["servers"])
        guard servers.count >= 2, let i = servers.firstIndex(where: { str($0, "id") == id }) else { return false }
        Logins.forgetServerKeys(id)
        servers.remove(at: i)
        let next = str(servers[min(i, servers.count - 1)], "id")
        st["servers"] = servers
        st["current"] = next
        writeStore(st)
        CUR = next
        return true
    }

    func loadData(_ id: String? = nil) -> J {
        let st = readStore()
        let sv = objs(st["servers"])[curIndex(st, id)]
        var settings = st["settings"] as? J ?? J()
        for (k, v) in sv["settings"] as? J ?? J() { settings[k] = v }
        var d: J = ["title": str(sv, "title"), "settings": settings]
        d["custom"] = objs(sv["custom"]).enumerated().map { (i, c) -> J in
            var c = c
            if str(c, "id").isEmpty { c["id"] = "c:legacy\(i)" }
            return c
        }
        d["edits"] = sv["edits"] as? J ?? J()
        d["hidden"] = strs(sv["hidden"])
        d["order"] = strs(sv["order"])
        d["widgetOrder"] = strs(sv["widgetOrder"])
        d["widgetHidden"] = strs(sv["widgetHidden"])
        // stacks: { id: "g:…", name, logo, expand (show members on the widget), members: [button ids] }
        d["groups"] = objs(sv["groups"]).filter { $0["id"] is String }.map { g -> J in
            let n = (g["name"] as? String).flatMap { $0.isEmpty ? nil : $0 } ?? "Stack"
            return ["id": str(g, "id"), "name": clip(n, 24), "logo": str(g, "logo"), "expand": bool(g, "expand"), "members": strs(g["members"])]
        }
        return d
    }

    // Saves the current server's part (and the shared settings) back into the file
    func saveData(_ d: J, _ id: String? = nil) {
        var st = readStore()
        var servers = objs(st["servers"])
        let i = curIndex(st, id)
        var sv = servers[i]
        if let t = d["title"] as? String { sv["title"] = t }
        for k in SERVER_PARTS { if let v = d[k] { sv[k] = v } }
        if let s = d["settings"] as? J {
            var own = J()
            var shared = st["settings"] as? J ?? J()
            for (k, v) in s { if SERVER_SETTING_KEYS.contains(k) { own[k] = v } else { shared[k] = v } }
            sv["settings"] = own
            st["settings"] = shared
        }
        servers[i] = sv
        st["servers"] = servers
        writeStore(st)
    }

    func settings(_ id: String? = nil) -> J { cleanSettings(loadData(id)["settings"]) }

    // Dashboard / widget name of the current server
    func getTitle(_ id: String? = nil) -> String {
        let st = readStore()
        let i = curIndex(st, id)
        return titleAt(objs(st["servers"])[i], i)
    }

    func sortByOrder(_ list: [J], _ order: [String]) -> [J] {
        if order.isEmpty { return list }
        func idx(_ s: J) -> Int { order.firstIndex(of: str(s, "id")) ?? Int.max }
        return list.enumerated().sorted { a, b in
            let ia = idx(a.element), ib = idx(b.element)
            return ia != ib ? ia < ib : a.offset < b.offset
        }.map { $0.element }
    }

    // All buttons: built-in (minus deleted, with edits applied), then custom.
    func getServices(_ forDashboard: Bool, _ id: String? = nil) -> [J] {
        let d = loadData(id)
        let host = serverHostOf(cleanSettings(d["settings"])).isEmpty ? "localhost" : serverHostOf(cleanSettings(d["settings"]))
        let hidden = strs(d["hidden"])
        let edits = d["edits"] as? J ?? J()
        let base: [J] = BUILTIN.map { s -> J in
            var o = s
            o["id"] = "b:" + str(s, "name")
            o["url"] = "\(bool(s, "https") ? "https" : "http")://\(host):\(o["port"].map { "\($0)" } ?? "")\(str(s, "path"))"
            if let e = edits[str(o, "id")] as? J { for (k, v) in e { o[k] = v } }
            return o
        }.filter { !hidden.contains(str($0, "id")) }
        let wh = strs(d["widgetHidden"])
        let list = (base + objs(d["custom"])).map { s -> J in
            var o = s
            let u = splitUrl(str(s, "url"))
            let port: String = s["port"].map { v in v is NSNull ? "" : (v as? String ?? (isNum(v) ? numString((v as! NSNumber).doubleValue) : "")) } ?? ""
            o["port"] = port
            o["icon"] = str(s, "icon").isEmpty ? "🔗" : str(s, "icon")
            o["logo"] = str(s, "logo")
            o["scheme"] = u.scheme
            o["addr"] = u.host + u.path
            o["label"] = port.isEmpty ? u.host : ":" + port
            o["onWidget"] = !wh.contains(str(s, "id"))
            return o
        }
        return forDashboard ? sortByOrder(list, strs(d["order"])) : list
    }

    struct Layout { var all: [J]; var groups: [J]; var dash: [J]; var widget: [J]; var hidden: [J] }

    // Top level = buttons not in a stack, plus stacks (each stack carries its member buttons).
    func getLayout(_ id: String? = nil) -> Layout {
        let d = loadData(id)
        var all = getServices(false, id)
        var byId = [String: Int]()
        for (i, s) in all.enumerated() { byId[str(s, "id")] = i }
        var inStack = Set<String>()
        let wh = strs(d["widgetHidden"])
        var groups = [J]()
        for g in objs(d["groups"]) {
            var members = [J]()
            for mid in strs(g["members"]) {
                guard let i = byId[mid], !inStack.contains(mid) else { continue }
                inStack.insert(mid)
                all[i]["group"] = str(g, "id")
                members.append(all[i])
            }
            groups.append(["id": str(g, "id"), "kind": "group", "name": str(g, "name"), "logo": str(g, "logo"), "icon": STACK_ICON,
                           "expand": bool(g, "expand"), "members": members, "onWidget": !wh.contains(str(g, "id"))])
        }
        let top = all.filter { !inStack.contains(str($0, "id")) } + groups
        return Layout(all: all, groups: groups, dash: sortByOrder(top, strs(d["order"])),
                      widget: sortByOrder(top.filter { bool($0, "onWidget") }, strs(d["widgetOrder"])),
                      hidden: top.filter { !bool($0, "onWidget") })
    }

    // What the large widget actually draws: stacks become one pill, or their members if "show members" is on
    func widgetCells(_ layout: Layout, _ rows: Int) -> [J] {
        var cells = [J]()
        for x in layout.widget {
            if str(x, "kind") != "group" { cells.append(x) }
            else if bool(x, "expand") { cells.append(contentsOf: objs(x["members"])) }
            else if !objs(x["members"]).isEmpty { cells.append(x) }
        }
        return Array(cells.prefix(rows * 4))
    }

    // Create / edit / ungroup stacks and move buttons in or out of them
    func stackAction(_ a: J) {
        let L = getLayout()
        var data = loadData()
        var order = L.dash.map { str($0, "id") }
        var widgetOrder = L.widget.map { str($0, "id") }
        var groups = objs(data["groups"])
        var widgetHidden = strs(data["widgetHidden"])
        func strip(_ id: String) { for i in groups.indices { groups[i]["members"] = strs(groups[i]["members"]).filter { $0 != id } } }
        func holder(_ id: String) -> J? { groups.first { strs($0["members"]).contains(id) } }
        func insertAt(_ list: [String], _ anchor: String?, _ ids: [String], _ after: Bool) -> [String] {
            var out = list.filter { !ids.contains($0) }
            guard let anchor = anchor, var i = out.firstIndex(of: anchor) else { return out + ids }
            if after { i += 1 }
            out.insert(contentsOf: ids, at: i)
            return out
        }
        func replaceIn(_ list: [String], _ oldId: String, _ ids: [String]) -> [String] { insertAt(list, oldId, ids, false).filter { $0 != oldId } }

        let act = str(a, "act")
        if act == "save" {
            let n = clip(trim(str(a, "name")), 24)
            let name = n.isEmpty ? "Stack" : n
            let logo = trim(str(a, "logo"))
            if let gi = groups.firstIndex(where: { str($0, "id") == str(a, "gid") && !str(a, "gid").isEmpty }) {
                groups[gi]["name"] = name; groups[gi]["logo"] = logo; groups[gi]["expand"] = bool(a, "expand")
            } else {
                var g: J = ["id": "g:\(Int(nowMs()))", "name": name, "logo": logo, "expand": bool(a, "expand"), "members": [String]()]
                let gid = str(g, "id")
                if let member = a["member"] as? String, !member.isEmpty {
                    let old = holder(member)
                    strip(member)
                    if let old = old {
                        order = insertAt(order, str(old, "id"), [gid], true)
                        widgetOrder = insertAt(widgetOrder, str(old, "id"), [gid], true)
                    } else {
                        order = replaceIn(order, member, [gid])
                        widgetOrder = replaceIn(widgetOrder, member, [gid])
                    }
                    g["members"] = [member]
                } else {
                    order.append(gid)
                    widgetOrder.append(gid)
                }
                groups.append(g)
            }
            Logos.clearFail(logo)
        } else if act == "move" {
            let id = str(a, "id")
            let old = holder(id)
            strip(id)
            if let to = a["to"] as? String, let ti = groups.firstIndex(where: { str($0, "id") == to }) {
                groups[ti]["members"] = strs(groups[ti]["members"]) + [id]
                order = order.filter { $0 != id }
                widgetOrder = widgetOrder.filter { $0 != id }
            } else {
                // out of the stack: placed right after it
                order = insertAt(order, old.map { str($0, "id") }, [id], true)
                widgetOrder = insertAt(widgetOrder, old.map { str($0, "id") }, [id], true)
            }
        } else if act == "del" {
            // ungroup: the buttons go back where the stack was
            if let g = groups.first(where: { str($0, "id") == str(a, "gid") }) {
                let gid = str(g, "id"), members = strs(g["members"])
                order = replaceIn(order, gid, members)
                widgetOrder = replaceIn(widgetOrder, gid, members)
                if widgetHidden.contains(gid) { widgetHidden = widgetHidden.filter { $0 != gid } + members }
                groups = groups.filter { str($0, "id") != gid }
            }
        }
        data["order"] = order
        data["widgetOrder"] = widgetOrder
        data["groups"] = groups
        data["widgetHidden"] = widgetHidden
        saveData(data)
    }

    // A button from the add / edit form
    func makeCustom(_ d: J) -> J {
        var addr = trim(str(d, "addr"))
        let m = match(addr, "^(https?)://", true)
        let scheme = m?[1].lowercased() ?? (str(d, "scheme") == "https" ? "https" : "http")
        addr = replace(addr, "^https?://", "", true)
        var path = ""
        if let slash = addr.firstIndex(of: "/") { path = String(addr[slash...]); addr = String(addr[..<slash]) }
        let raw = d["port"].map { v -> String in v as? String ?? (isNum(v) ? numString((v as! NSNumber).doubleValue) : "") } ?? ""
        let port = replace(raw, "[^0-9]", "")
        return ["name": clip(trim(str(d, "name")), 24), "port": port, "icon": "🔗", "logo": trim(str(d, "logo")), "custom": true,
                "url": "\(scheme)://\(addr)\(port.isEmpty ? "" : ":" + port)\(path)"]
    }
}

// ---------- Saved logins: every server keeps its own. Server 1 ("s1") uses the names from before there were several servers. ----------
enum Logins {
    static let KEYS: [String: (String, String)] = [
        "zimaos": ("board-zima-user", "board-zima-pass"), "proxmox": ("board-pve-user", "board-pve-pass"),
        "truenas": ("board-truenas-user", "board-truenas-pass"), "unraid": ("board-unraid-user", "board-unraid-pass"),
        "synology": ("board-syno-user", "board-syno-pass"), "omv": ("board-omv-user", "board-omv-pass"),
    ]
    static func kname(_ base: String, _ id: String? = nil) -> String {
        let i = id ?? Store.shared.CUR
        return i.isEmpty || i == "s1" ? base : "\(base)@\(i)"
    }
    static func loginKeys(_ p: String, _ id: String? = nil) -> (String, String)? { KEYS[p].map { (kname($0.0, id), kname($0.1, id)) } }
    static func helperKey(_ id: String? = nil) -> String { kname("board-helper-key", id) }
    static func gpassKey(_ id: String? = nil) -> String { kname("board-glances-pass", id) }

    static func forgetServerKeys(_ id: String) {
        var names = [helperKey(id), gpassKey(id)]
        for p in KEYS.keys { let k = loginKeys(p, id)!; names += [k.0, k.1] }
        names.forEach { Keychain.remove($0) }
    }

    // Linux: "logged in" = the helper's secret key is saved
    static func hasLogin(_ p: String, _ id: String? = nil) -> Bool {
        if p == "linux" { return Keychain.contains(helperKey(id)) }
        guard let k = loginKeys(p, id) else { return false }
        return Keychain.contains(k.0) && Keychain.contains(k.1)
    }

    static func forget(_ p: String) {
        if p == "linux" { Keychain.remove(helperKey()); return }
        if let k = loginKeys(p) { Keychain.remove(k.0); Keychain.remove(k.1) }
    }

    static func saved(_ p: String) -> (String, String) {
        let k = loginKeys(p)!
        return (Keychain.get(k.0) ?? "", Keychain.get(k.1) ?? "")
    }

    static func save(_ p: String, _ u: String, _ pw: String, _ id: String? = nil) {
        guard let k = loginKeys(p, id) else { return }
        Keychain.set(k.0, u); Keychain.set(k.1, pw)
    }

    static func glancesPass(_ id: String? = nil) -> String { Keychain.get(gpassKey(id)) ?? "" }
}
