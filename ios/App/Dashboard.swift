import Foundation
import SafariServices
import UIKit
import WebKit
import WidgetKit

// The live dashboard: the Scriptable version's own page in a web view, answered by this controller
// exactly as the script answers it (add://, del://, set://, pwr:// ... requests and the update loop).
@MainActor
final class Dashboard: NSObject, WKNavigationDelegate {
    static let shared = Dashboard()
    let web: WKWebView
    private let store = Store.shared
    private var SET: J
    private var glances: Glances
    private var GEN = 0
    private var expanded: String?
    private var lastStats: J?
    private var statusBusy = false, statusDirty = false, lastStatus = 0.0
    private var reachBusy = false, lastReach = 0.0, reachFor = ""
    private var loop: Task<Void, Never>?
    private var logoBusy = false
    private var ready = false
    private var pendingStack: String?
    private let REACH_SECONDS = 10.0

    override init() {
        let c = WKWebViewConfiguration()
        c.websiteDataStore = .nonPersistent()
        web = WKWebView(frame: .zero, configuration: c)
        SET = store.settings()
        glances = Glances(SET)
        super.init()
        web.navigationDelegate = self
        web.isOpaque = false
        web.scrollView.contentInsetAdjustmentBehavior = .never
        web.scrollView.bounces = true
        applyBackground()
        load()
    }

    private func applyBackground() {
        let t = theme(str(SET, "theme"))
        web.backgroundColor = UIColor(hex: t.bg)
        web.scrollView.backgroundColor = UIColor(hex: t.bg)
    }

    // After Settings are saved (or imported, or another server is shown): use the new values right away
    private func applySettings() {
        SET = store.settings()
        glances = Glances(SET)
        applyBackground()
    }

    private func load() {
        guard let u = Bundle.main.url(forResource: "live", withExtension: "html"), var html = try? String(contentsOf: u, encoding: .utf8) else { return }
        // the chosen style and startup screen are set before the first paint (no flash of the other look)
        let cls = [str(SET, "style") == "glass" ? "glass" : "", bool(SET, "splash") ? "splashon" : ""].filter { !$0.isEmpty }.joined(separator: " ")
        html = html.replacingOccurrences(of: "<body>", with: "<body class=\"\(cls)\">")
        // a full-screen app has no Scriptable toolbar above the page
        html = html.replacingOccurrences(of: "</style>", with: "main{padding-top:calc(env(safe-area-inset-top,0px) + 14px)}</style>")
        web.loadHTMLString(html, baseURL: nil)
    }

    private func js(_ s: String) { web.evaluateJavaScript(s, completionHandler: nil) }

    private func refreshWidgets() { WidgetCenter.shared.reloadAllTimelines() }

    // ---------- What the page needs to draw the buttons ----------
    private func payload() -> J {
        let L = store.getLayout()
        var logos = J()
        if bool(SET, "useLogos") {
            for s in L.all + L.groups { if let b = Logos.base64(str(s, "logo")) { logos[str(s, "id")] = b } }
        }
        let pl = PLATFORMS[str(SET, "platform")]
        return [
            "dash": L.dash, "widget": L.widget, "hidden": L.hidden, "limits": SET["tempLimits"] ?? J(), "logos": logos, "cdn": LOGO_CDN,
            "hasLogin": pl != nil ? Logins.hasLogin(str(SET, "platform")) : false, "title": store.getTitle(), "host": serverHostOf(SET),
            "servers": store.listServers(), "cur": store.CUR, "platforms": platformsJSON,
            "ui": [
                "cols": SET["columns"] ?? 2, "power": bool(SET, "showPower") && pl != nil, "platform": str(SET, "platform"),
                "login": pl?.login as Any? ?? NSNull(), "userPh": pl?.userPh ?? "", "widgetMax": int(SET, "widgetRows") * 4,
                "monitoring": bool(SET, "monitoring"), "monOpen": bool(SET, "monitoringOpen"), "monFixed": bool(SET, "monitoringFixed"),
                "look": lookSettings(SET), "splash": bool(SET, "splash"), "themes": themesJSON, "accents": accentsJSON, "glows": glowsJSON,
            ] as J,
        ]
    }

    private func pushServices() { js("renderServices(\(toJSON(payload())))") }

    private func refreshLogos() {
        if logoBusy || !bool(SET, "useLogos") { return }
        logoBusy = true
        Task {
            let L = store.getLayout()
            if await Logos.ensure(L.all + L.groups, 8) { pushServices() }
            Logos.prune()
            logoBusy = false
        }
    }

    // Changing the server on screen: everything fetched for the last one is dropped, then the page redraws
    private func showServer(_ dir: Int) {
        GEN += 1
        lastStats = nil
        expanded = nil
        statusDirty = true
        lastStatus = 0
        reachFor = ""
        applySettings()
        js("switchServer(\(jsonQuote(store.getTitle())), \(toJSON(payload())), \(dir))")
        refreshLogos()
    }

    // ---------- First load ----------
    func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
        if ready { return }
        ready = true
        js("init(\(jsonQuote(store.getTitle())), \(toJSON(payload())))")
        if let s = pendingStack { js("focusStack(\(jsonQuote(s)))"); pendingStack = nil }
        refreshLogos()
        // first run: no server yet, so go straight to Settings (after the startup screen, when it's on)
        if serverHostOf(SET).isEmpty { js("setTimeout(openSettings, \(bool(SET, "splash") ? 1700 : 400))") }
        start()
    }

    // board://open?server=...&stack=... from the widget
    func openURL(_ url: URL) {
        let q = URLComponents(url: url, resolvingAgainstBaseURL: false)?.queryItems ?? []
        let server = q.first { $0.name == "server" }?.value
        let stack = q.first { $0.name == "stack" }?.value
        if let s = server, s != store.CUR, store.hasServer(s), store.setCurrentServer(s) { if ready { showServer(0) } else { applySettings() } }
        if let st = stack { if ready { js("focusStack(\(jsonQuote(st)))") } else { pendingStack = st } }
    }

    // ---------- The live loop: only while the dashboard is on screen ----------
    func start() {
        guard ready, loop == nil else { return }
        store.reload()
        loop = Task { [weak self] in
            while !Task.isCancelled {
                guard let self = self else { return }
                await self.tick()
                try? await Task.sleep(nanoseconds: UInt64(max(1, int(self.SET, "liveSeconds"))) * 1_000_000_000)
            }
        }
    }

    func stop() {
        loop?.cancel()
        loop = nil
        reachBusy = false
        statusBusy = false
    }

    private func tick() async {
        if serverHostOf(SET).isEmpty { js("offline(true)"); return }
        let g = GEN
        // LIVE / OFFLINE: is anything on the server answering? A changed address is checked straight away.
        let rk = serverUrlOf(SET) + "|" + str(SET, "platform") + "|" + String(bool(SET, "monitoring")) + "|" + str(SET, "baseUrl")
        if !reachBusy && (rk != reachFor || nowMs() - lastReach > REACH_SECONDS * 1000) {
            reachBusy = true
            reachFor = rk
            let s = SET, services = store.getServices(false)
            Task {
                let r = await Net.serverReachable(s, services)
                if g == self.GEN { self.js("setReach(\(r.isEmpty ? "false" : "true"))") }
                self.reachBusy = false
                self.lastReach = nowMs()
            }
        }
        // Monitoring off: no requests to Glances at all
        if bool(SET, "monitoring") {
            do {
                let stats = try await glances.getStats(fsMaxAge: 10000)
                if g == GEN { lastStats = stats; js("update(\(toJSON(stats)))") }
            } catch {
                if g == GEN { js("offline()") }
            }
            await pushDetail()
        }
        if !statusBusy && (statusDirty || nowMs() - lastStatus > Double(int(SET, "statusSeconds")) * 1000) {
            statusBusy = true
            statusDirty = false
            let services = store.getServices(false), t = Double(int(SET, "pingTimeout"))
            Task {
                let m = await Net.checkServices(services, t)
                if g == self.GEN { self.js("setUp(\(toJSON(m)))") }
                self.statusBusy = false
                self.lastStatus = nowMs()
            }
        }
    }

    private func pushDetail() async {
        guard let key = expanded, let st = lastStats else { return }
        if let d = await glances.getDetail(key, st) { js("detail(\(jsonQuote(key)), \(toJSON(d)))") }
    }

    // ---------- Requests from the page ----------
    func webView(_ webView: WKWebView, decidePolicyFor navigationAction: WKNavigationAction) async -> WKNavigationActionPolicy {
        guard let url = navigationAction.request.url, let scheme = url.scheme?.lowercased(),
              ["add", "del", "exp", "ord", "grp", "wrd", "ttl", "set", "pwr", "srv", "open"].contains(scheme) else {
            return .allow
        }
        let raw = url.absoluteString
        func arg(_ name: String) -> String {
            guard let r = raw.range(of: "?\(name)=") else { return "" }
            let v = String(raw[r.upperBound...])
            return v.removingPercentEncoding ?? v
        }
        handle(scheme, arg)
        return .cancel
    }

    private func handle(_ scheme: String, _ arg: (String) -> String) {
        switch scheme {
        case "add":
            guard let d = parseJSON(arg("d")) as? J else { return }
            var data = store.loadData()
            var item = store.makeCustom(d)
            var id = d["id"] as? String
            var custom = objs(data["custom"])
            if let i = id, i.hasPrefix("b:") {
                var edits = data["edits"] as? J ?? J()
                edits[i] = ["name": item["name"]!, "port": item["port"]!, "url": item["url"]!, "logo": item["logo"]!]
                data["edits"] = edits
            } else if let i = id, let at = custom.firstIndex(where: { str($0, "id") == i }) {
                for k in ["name", "port", "url", "logo"] { custom[at][k] = item[k] }
            } else {
                id = "c:\(Int(nowMs()))"
                item["id"] = id
                custom.append(item)
            }
            data["custom"] = custom
            var wh = strs(data["widgetHidden"]).filter { $0 != id }
            if isBool(d["onWidget"]) && !bool(d, "onWidget"), let i = id { wh.append(i) }
            data["widgetHidden"] = wh
            store.saveData(data)
            statusDirty = true
            Logos.clearFail(str(item, "logo"))
            pushServices(); refreshLogos(); refreshWidgets()
        case "del":
            let id = arg("id")
            var data = store.loadData()
            if id.hasPrefix("b:") {
                var edits = data["edits"] as? J ?? J()
                edits[id] = nil
                data["edits"] = edits
                var h = strs(data["hidden"])
                if !h.contains(id) { h.append(id) }
                data["hidden"] = h
            } else {
                data["custom"] = objs(data["custom"]).filter { str($0, "id") != id }
            }
            data["groups"] = objs(data["groups"]).map { g -> J in var g = g; g["members"] = strs(g["members"]).filter { $0 != id }; return g }
            store.saveData(data)
            statusDirty = true
            pushServices(); refreshWidgets()
        case "exp":
            let k = arg("k")
            expanded = k.isEmpty ? nil : k
            if expanded != nil { Task { await pushDetail() } }
        case "ord":
            guard let o = parseJSON(arg("o")) else { return }
            var data = store.loadData()
            if let a = o as? [Any] { data["order"] = a.compactMap { $0 as? String } }
            else if let oo = o as? J, oo["o"] is [Any] {
                // o: top-level order, g: each stack's buttons (buttons can be dragged in and out of open stacks)
                data["order"] = strs(oo["o"])
                if let gm = oo["g"] as? J {
                    data["groups"] = objs(data["groups"]).map { g -> J in
                        var g = g
                        if let m = gm[str(g, "id")] as? [Any] { g["members"] = m.compactMap { $0 as? String } }
                        return g
                    }
                }
            }
            store.saveData(data)
            pushServices(); refreshWidgets()
        case "grp":
            guard let a = parseJSON(arg("d")) as? J else { return }
            store.stackAction(a)
            statusDirty = true
            pushServices(); refreshLogos(); refreshWidgets()
        case "wrd":
            guard let o = parseJSON(arg("o")) as? J, o["s"] is [Any], o["h"] is [Any] else { return }
            var data = store.loadData()
            data["widgetOrder"] = strs(o["s"])
            data["widgetHidden"] = strs(o["h"])
            store.saveData(data)
            pushServices(); refreshWidgets()
        case "ttl":
            let t = clip(trim(arg("t")), 20)
            if !t.isEmpty { var data = store.loadData(); data["title"] = t; store.saveData(data); refreshWidgets() }
        case "set":
            let d = parseJSON(arg("d")) as? J ?? J()
            Task {
                let r = await self.settingsOp(d)
                self.js("setReply(\(toJSON(d["rq"] ?? NSNull())), \(toJSON(r)))")
                // anything that changes buttons, logos or layout is redrawn right away
                if bool(r, "ok") && str(d, "op") == "import" { self.showServer(0); self.refreshWidgets() }
                else if bool(r, "ok") && ["save", "restore", "clearlogos", "reset"].contains(str(d, "op")) {
                    self.pushServices(); self.refreshLogos(); self.refreshWidgets()
                }
            }
        case "pwr":
            let d = parseJSON(arg("d")) as? J ?? J()
            let s = SET
            Task {
                let r = await Power.handlePower(d, s)
                self.js("powerResult(\(toJSON(r)))")
            }
        case "srv":
            let d = parseJSON(arg("d")) as? J ?? J()
            Task { await self.serverOp(d) }
        case "open":
            let u = arg("u")
            if let url = URL(string: u), ["http", "https"].contains(url.scheme?.lowercased() ?? "") { present(SFSafariViewController(url: url)) }
        default: break
        }
    }

    private func present(_ vc: UIViewController) {
        var top = web.window?.rootViewController
        while let p = top?.presentedViewController { top = p }
        top?.present(vc, animated: true)
    }

    // switch / add / remove a server (swipe the server name, the dots, or Settings)
    private func serverOp(_ d: J) async {
        let before = store.listServers().map { str($0, "id") }
        var ok = false
        var next = ""
        switch str(d, "act") {
        case "switch":
            ok = store.setCurrentServer(str(d, "id"))
        case "add":
            // the new server's address and type; the login (if given) is checked before the server is added
            var raw = SET
            raw["serverHost"] = str(d, "host"); raw["serverScheme"] = str(d, "scheme"); raw["serverPort"] = str(d, "port")
            raw["platform"] = str(d, "platform").isEmpty ? "none" : str(d, "platform")
            raw["proxmoxNode"] = ""; raw["baseUrl"] = ""; raw["username"] = ""; raw["diskMounts"] = [String](); raw["networkInterface"] = ""
            for k in PLATFORM_KEYS { raw[PLATFORMS[k]!.url] = "" }
            let own = cleanSettings(raw)
            let P = str(own, "platform")
            let withLogin = Power.hasDriver(P) && !str(d, "u").isEmpty && !str(d, "p").isEmpty
            if withLogin {
                let r = await Power.driver(P, nil, str(d, "u"), str(d, "p"), platformBase(P, own), own)
                if !r.ok {
                    let error = (r.msg.isEmpty ? "The login didn't work." : r.msg) + (r.net ? " Check the address, or add the server without the login and enter it later." : "")
                    js("addServerResult(\(toJSON(["ok": false, "error": error] as J)))")
                    return
                }
            }
            let id = store.addServer(str(d, "name"), ["serverHost": own["serverHost"]!, "serverScheme": own["serverScheme"]!, "serverPort": own["serverPort"]!, "platform": P])
            if withLogin { Logins.save(P, str(d, "u"), str(d, "p"), id) }
            js("addServerResult({ ok: true })")
            ok = true
            next = P == "linux" ? "settings" : withLogin ? "Server added, login checked and saved" : "Server added"
        case "remove":
            ok = store.removeServer(str(d, "id"))
        default: break
        }
        if !ok { return }
        showServer(before.contains(store.CUR) ? int(d, "dir") : 1)
        refreshWidgets()
        if str(d, "act") == "add" { js(next == "settings" ? "setTimeout(openSettings, 450)" : "toast(\(jsonQuote(next)), 3000)") }
        if str(d, "act") == "remove" { js("toast(\"Server deleted\", 2500)") }
    }

    // ---------- Settings sheet (gear icon): load / save / test / maintenance ----------
    private func settingsOp(_ d: J) async -> J {
        var data = store.loadData()
        let op = str(d, "op")
        if op == "load" {
            // the drive and network lists come from Glances (only asked when Monitoring is on)
            var mounts: Any = NSNull(), ifaces: Any = NSNull()
            if bool(SET, "monitoring") && !serverHostOf(SET).isEmpty {
                async let fsT = try? glances.arr("fs")
                async let netT = try? glances.arr("network")
                if let fs = await fsT {
                    mounts = fs.filter { !str($0, "mnt_point").isEmpty && isRealDrive($0) }.map {
                        ["mnt": str($0, "mnt_point"), "size": n0($0, "size"), "used": n0($0, "used"), "dev": str($0, "device_name").components(separatedBy: "/").last ?? ""] as J
                    }
                }
                if let net = await netT { ifaces = net.map { str($0, "interface_name") }.filter { !$0.isEmpty && !test($0, SKIP_IFACE) } }
            }
            var logins = J()
            for k in PLATFORM_KEYS { logins[k] = Logins.hasLogin(k) }
            return ["ok": true, "s": SET, "defaults": DEFAULTS, "host": serverHostOf(SET), "hasBuiltin": !store.BUILTIN.isEmpty,
                    "firstRun": serverHostOf(SET).isEmpty, "mounts": mounts, "ifaces": ifaces,
                    "hidden": strs(data["hidden"]).map { ["id": $0, "name": replace($0, "^b:", "")] as J },
                    "logins": logins, "platforms": platformsJSON, "helperPort": HELPER_PORT, "hasGpass": Keychain.contains(Logins.gpassKey())]
        }
        if op == "save" {
            data["settings"] = cleanSettings(d["s"])
            let t = trim(str(d, "title"))
            if !t.isEmpty { data["title"] = clip(t, 20) }
            store.saveData(data)
            if bool(d, "clearGpass") { Keychain.remove(Logins.gpassKey()) }
            if !str(d, "gpass").isEmpty { Keychain.set(Logins.gpassKey(), str(d, "gpass")) }
            applySettings()
            statusDirty = true
            return ["ok": true, "msg": "Settings saved"]
        }
        if op == "test" {
            // tests the values in the form, saved or not
            let s = cleanSettings(d["s"])
            var lines = [J]()
            if serverHostOf(s).isEmpty { return ["ok": true, "lines": [["ok": false, "t": "Enter your server's IP or name first, e.g. 192.168.1.10"] as J]] }
            let reach = await Net.serverReachable(s, store.getServices(false))
            lines.append(!reach.isEmpty ? ["ok": true, "t": "Server: reachable (\(reach) answered)"]
                         : ["ok": false, "t": "Server: nothing answered at \(serverHostOf(s)) (check the address, and your VPN if you're away from home)"])
            if bool(s, "monitoring") {
                let t0 = nowMs()
                let g = Glances(s)
                let pass = str(d, "gpass").isEmpty ? nil : str(d, "gpass")
                do {
                    let q = try await g.request("quicklook", pass: pass) as? J
                    let name = str(q ?? J(), "cpu_name")
                    lines.append(["ok": true, "t": "Glances: OK (\(Int(nowMs() - t0)) ms\(name.isEmpty ? "" : ", " + name))"])
                } catch {
                    // maybe it's the other Glances API version (distro packages are often Glances 3.x)
                    let other = int(s, "apiVersion") == 4 ? 3 : 4
                    var s2 = s
                    s2["apiVersion"] = other
                    let otherOk = (try? await g.request("quicklook", pass: pass, set: s2)) != nil
                    lines.append(otherOk ? ["ok": false, "t": "Glances: answers with API version \(other), not \(int(s, "apiVersion")). Pick \(other) under GLANCES API VERSION."]
                                 : ["ok": false, "t": "Glances: no answer at \(glancesUrl(s)) (check the address and that Glances runs with -w, or turn Monitoring off)"])
                }
            }
            lines.append(await Power.testPower(s))
            return ["ok": true, "lines": lines]
        }
        if op == "login" {
            // Settings > Server > Enter login: check the login against the server (address from the form), then save it
            let P = str(d, "platform")
            if !Power.hasDriver(P) { return ["ok": false, "error": "This server type doesn't use a login."] }
            let user = trim(str(d, "u")), pass = str(d, "p")
            if user.isEmpty || pass.isEmpty { return ["ok": false, "error": "Enter a username and a password."] }
            let s = cleanSettings(d["s"])
            let base = platformBase(P, s)
            let r = await Power.driver(P, nil, user, pass, base, s)
            if !r.ok {
                let why = r.msg.isEmpty ? "no answer at " + base : r.msg
                return ["ok": false, "error": r.net ? why + ". The login wasn't saved: try again when the server can be reached." : why]
            }
            Logins.save(P, user, pass)
            let extra = match(r.msg, "\\(([^)]*)\\)\\s*$")?[1]
            return ["ok": true, "msg": "\(PLATFORMS[P]!.login ?? P) login checked and saved" + (extra.map { " (\($0))" } ?? "")]
        }
        if op == "forget" {
            let P = PLATFORMS[str(d, "platform")] != nil ? str(d, "platform") : str(SET, "platform")
            Logins.forget(P)
            return ["ok": true, "msg": P == "linux" ? "Helper key forgotten" : (PLATFORMS[P]?.login ?? P) + " login forgotten"]
        }
        if op == "helper" {
            // Linux power helper: keep the saved key (or save the new one made in the page) and copy the setup command
            let s = cleanSettings(d["s"])
            if !Logins.hasLogin("linux") {
                let key = str(d, "key")
                if !test(key, "^[a-f0-9]{32,128}$") { return ["ok": false, "error": "Couldn't make a key. Try again."] }
                Keychain.set(Logins.helperKey(), key)
            }
            UIPasteboard.general.string = Power.helperSetupScript(Keychain.get(Logins.helperKey()) ?? "", Power.helperPort(s))
            return ["ok": true, "msg": "Setup command copied. Paste it into an SSH session on the server.", "hasKey": true]
        }
        if op == "restore" {
            let id = str(d, "id")
            data["hidden"] = id.isEmpty ? [String]() : strs(data["hidden"]).filter { $0 != id }
            store.saveData(data)
            statusDirty = true
            return ["ok": true, "msg": id.isEmpty ? "All buttons restored" : "Button restored"]
        }
        if op == "clearlogos" {
            Logos.clearAll()
            return ["ok": true, "msg": "Logo cache cleared. Logos download again now."]
        }
        if op == "export" {
            // every server, its buttons and layout, and the shared settings
            UIPasteboard.general.string = toJSON(OJ(pairs: [("board", 2), ("exported", ISO8601DateFormatter().string(from: Date())), ("data", store.readStore())]), pretty: true)
            return ["ok": true, "msg": "All servers' settings copied to the clipboard. Passwords are not included."]
        }
        if op == "import" {
            guard let obj = parseJSON(UIPasteboard.general.string ?? "") as? J, obj["board"] != nil, let imp = obj["data"] as? J else {
                return ["ok": false, "error": "The clipboard doesn't contain exported settings."]
            }
            if imp["servers"] is [Any] {
                // every server (an export from this version or the Scriptable / Android ones): replaces all of them
                store.importRaw(toJSON(imp))
            } else {
                // an export from before there were several servers: goes into the server shown now
                var merged = imp
                var s = store.loadData()["settings"] as? J ?? J()
                for (k, v) in imp["settings"] as? J ?? J() { s[k] = v }
                merged["settings"] = s
                store.saveData(merged)
            }
            applySettings()
            statusDirty = true
            return ["ok": true, "msg": "Settings imported", "reload": true]
        }
        if op == "reset" {
            data["order"] = [String](); data["widgetOrder"] = [String](); data["widgetHidden"] = [String](); data["groups"] = [J]()
            store.saveData(data)
            return ["ok": true, "msg": "Layout reset. Your own buttons are kept."]
        }
        return ["ok": false, "error": "Unknown request"]
    }
}

extension UIColor {
    convenience init(hex: String) {
        let v = UInt32(hex.replacingOccurrences(of: "#", with: ""), radix: 16) ?? 0
        self.init(red: CGFloat((v >> 16) & 0xff) / 255, green: CGFloat((v >> 8) & 0xff) / 255, blue: CGFloat(v & 0xff) / 255, alpha: 1)
    }
}
