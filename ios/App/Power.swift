import Foundation

// ---------- Reboot / shut down ----------
// Every server type has a driver: run(action, user, pass, base, s). action "restart" / "off" sends the command;
// action nil only checks the login (for Test connection). It returns { ok, msg } or { ok: false, auth, net, msg }.
struct PowerResult { var ok: Bool; var msg: String = ""; var auth = false; var net = false }

func powerSent(_ action: String) -> String {
    action == "off" ? "Shutdown sent. Switch the server on by hand to bring it back." : "Reboot sent. The server should be back in a minute or two."
}

func formBody(_ kv: [(String, Any)]) -> String {
    kv.map { encodeURIComponent($0.0) + "=" + encodeURIComponent("\($0.1)") }.joined(separator: "&")
}

private let FORM = ["Content-Type": "application/x-www-form-urlencoded"]
private let JSONH = ["Content-Type": "application/json"]

enum Power {
    // After the command: a dropped connection is normal (the server is going down)
    static func sendCommand(_ name: String, _ action: String, _ req: () async throws -> Resp) async -> PowerResult {
        guard let r = try? await req() else { return PowerResult(ok: true, msg: powerSent(action)) }
        if r.code == 401 { return PowerResult(ok: false, msg: "\(name) refused the login. Enter it again.", auth: true) }
        if r.code >= 300 { return PowerResult(ok: false, msg: "\(name) didn't accept the command (HTTP \(r.code)).") }
        return PowerResult(ok: true, msg: powerSent(action))
    }

    // --- ZimaOS ---
    static func zima(_ action: String?, _ user: String, _ pass: String, _ base: String, _ s: J) async -> PowerResult {
        guard let r = try? await Net.request(base + "/v1/users/login", method: "POST", headers: JSONH, body: toJSON(["username": user, "password": pass])),
              let j = r.json as? J else { return PowerResult(ok: false, msg: "Can't reach ZimaOS at " + base, net: true) }
        let token = str(((j["data"] as? J)?["token"] as? J) ?? J(), "access_token")
        if token.isEmpty { return PowerResult(ok: false, msg: "Login failed: " + (str(j, "message").isEmpty ? "wrong username or password" : str(j, "message")), auth: true) }
        guard let action = action else { return PowerResult(ok: true, msg: "saved login works") }
        let r2 = await sendCommand("ZimaOS", action) { try await Net.request("\(base)/v1/sys/state/\(action == "off" ? "off" : "restart")", method: "PUT", headers: ["Authorization": token]) }
        if r2.auth || r2.msg.contains("HTTP 403") { return PowerResult(ok: false, msg: "ZimaOS refused the login. Enter it again.", auth: true) }
        return r2
    }

    // --- Proxmox VE ---
    // "root" -> "root@pam" (the Linux login realm Proxmox uses by default)
    static func pveUser(_ u0: String) -> String { let u = trim(u0); return !u.isEmpty && !u.contains("@") ? u + "@pam" : u }

    static func pve(_ action: String?, _ user: String, _ pass: String, _ base: String, _ s: J) async -> PowerResult {
        guard let r = try? await Net.request(base + "/api2/json/access/ticket", method: "POST", headers: FORM, body: formBody([("username", pveUser(user)), ("password", pass)])),
              let j = r.json as? J else { return PowerResult(ok: false, msg: "Can't reach Proxmox at " + base, net: true) }
        let d = j["data"] as? J
        if let t = d?["NeedTFA"], !(t is NSNull), (jsNumber(t) ?? 0) != 0 {
            return PowerResult(ok: false, msg: "This account uses two-factor login. Use an account without it (e.g. a separate user with the Sys.PowerMgmt permission).", auth: true)
        }
        guard let d = d, !str(d, "ticket").isEmpty else { return PowerResult(ok: false, msg: "Login failed: wrong username or password (e.g. root@pam)", auth: true) }
        let auth = ["Cookie": "PVEAuthCookie=" + str(d, "ticket"), "CSRFPreventionToken": str(d, "CSRFPreventionToken")]
        // The node to reboot: the one named in Settings, the only one, or the one at the server address
        var node = str(s, "proxmoxNode")
        if node.isEmpty {
            guard let nr = try? await Net.request(base + "/api2/json/nodes", headers: auth), let nj = nr.json as? J else {
                return PowerResult(ok: false, msg: "Couldn't list the Proxmox nodes")
            }
            let nodes = objs(nj["data"]).map { str($0, "node") }.filter { !$0.isEmpty }
            if nodes.count == 1 { node = nodes[0] }
            else if nodes.isEmpty { return PowerResult(ok: false, msg: "Proxmox didn't list any nodes. Enter the node name in Settings.") }
            else {
                let host = serverHostOf(s).lowercased()
                var hit = nodes.first { $0.lowercased() == host || host.hasPrefix($0.lowercased() + ".") }
                if hit == nil, let cr = try? await Net.request(base + "/api2/json/cluster/status", headers: auth), let cj = cr.json as? J {
                    hit = objs(cj["data"]).first { str($0, "type") == "node" && str($0, "ip") == host }.map { str($0, "name") }
                }
                guard let h = hit else { return PowerResult(ok: false, msg: "Several Proxmox nodes found (\(nodes.joined(separator: ", "))). Enter the node name in Settings.") }
                node = h
            }
        }
        guard let action = action else { return PowerResult(ok: true, msg: "saved login works (node \(node))") }
        var h = auth
        h["Content-Type"] = FORM["Content-Type"]
        let r2 = await sendCommand("Proxmox", action) {
            try await Net.request("\(base)/api2/json/nodes/\(encodeURIComponent(node))/status", method: "POST", headers: h, body: "command=" + (action == "off" ? "shutdown" : "reboot"))
        }
        if r2.msg.contains("HTTP 403") { return PowerResult(ok: false, msg: "This Proxmox user isn't allowed to power off node \"\(node)\" (needs Sys.PowerMgmt).") }
        return r2
    }

    // --- TrueNAS: JSON-RPC 2.0 over a WebSocket (25.04+), the REST API on older versions ---
    struct WsResult: @unchecked Sendable { var ok = false; var auth = false; var error: String?; var timeout = false; var closed = false; var stage = "connect"; var results: [Any] = [] }

    static func truenasWs(_ base: String, _ user: String, _ pass: String, _ calls: [(String, [Any], [Any]?)]) async -> WsResult {
        guard let url = URL(string: replaceFirst(base, "^http", "ws", true) + "/api/current") else { return WsResult(error: "bad address") }
        let ws = Net.shared.session.webSocketTask(with: url)
        ws.resume()
        defer { ws.cancel(with: .normalClosure, reason: nil) }
        final class State: @unchecked Sendable { var stage = "connect"; var id = 0 }
        let st = State()
        func receiveText() async throws -> String {
            switch try await ws.receive() {
            case .string(let s): return s
            case .data(let d): return String(decoding: d, as: UTF8.self)
            @unknown default: return ""
            }
        }
        func call(_ method: String, _ params: [Any]) async throws -> J {
            st.id += 1
            let my = st.id
            try await ws.send(.string(toJSON(["jsonrpc": "2.0", "id": my, "method": method, "params": params] as J)))
            while true {
                if let j = parseJSON(try await receiveText()) as? J, int(j, "id") == my { return j }
            }
        }
        func msg(_ e: Any?) -> String {
            let e = e as? J ?? J()
            let m = str(e, "message")
            if !m.isEmpty { return m }
            let r = str(e["data"] as? J ?? J(), "reason")
            return r.isEmpty ? "error" : r
        }
        func hasError(_ j: J) -> Bool { j["error"] != nil && !(j["error"] is NSNull) }
        do {
            return try await withTimeout(12) {
                // the login only gets an answer once the connection is open
                let login = try await call("auth.login", [user, pass])
                st.stage = "login"
                if hasError(login) { return WsResult(error: msg(login["error"]), stage: st.stage) }
                if !bool(login, "result") { return WsResult(auth: true, stage: st.stage) }
                st.stage = "call"
                var results = [Any]()
                for c in calls {
                    var r = try await call(c.0, c.1)
                    if hasError(r), let fb = c.2 { r = try await call(c.0, fb) }
                    if hasError(r) { return WsResult(error: msg(r["error"]), stage: st.stage) }
                    results.append(r["result"] ?? NSNull())
                }
                return WsResult(ok: true, stage: st.stage, results: results)
            }
        } catch is TimeoutError {
            return WsResult(timeout: true, stage: st.stage)
        } catch {
            // after the login the server going down closes the connection
            return st.stage == "connect" ? WsResult(error: "connection", stage: st.stage) : WsResult(closed: true, stage: st.stage)
        }
    }

    static func truenas(_ action: String?, _ user: String, _ pass: String, _ base: String, _ s: J) async -> PowerResult {
        let method = action == "off" ? "system.shutdown" : "system.reboot"
        // 25.10+ needs a reason; earlier JSON-RPC versions take only options, hence the fallback
        let calls: [(String, [Any], [Any]?)] = action != nil ? [(method, ["Requested from the -board app"], [])] : [("system.info", [], nil)]
        let w = await truenasWs(base, user, pass, calls)
        if w.ok {
            if let a = action { return PowerResult(ok: true, msg: powerSent(a)) }
            let v = str(w.results.first as? J ?? J(), "version")
            return PowerResult(ok: true, msg: "saved login works" + (v.isEmpty ? "" : " (\(v))"))
        }
        if w.auth { return PowerResult(ok: false, msg: "TrueNAS refused the login (wrong username or password, or the account uses two-factor login).", auth: true) }
        if let a = action, w.stage == "call", w.closed || w.timeout { return PowerResult(ok: true, msg: powerSent(a)) }
        if w.stage != "connect" { return PowerResult(ok: false, msg: "TrueNAS: " + (w.error ?? "no answer")) }

        // No JSON-RPC WebSocket: an older TrueNAS (24.10 and earlier, or CORE) with the REST API
        let auth = ["Authorization": "Basic " + Data("\(user):\(pass)".utf8).base64EncodedString()]
        guard let action = action else {
            guard let r = try? await Net.request(base + "/api/v2.0/system/info", headers: auth) else { return PowerResult(ok: false, msg: "Can't reach TrueNAS at " + base, net: true) }
            if r.code == 401 { return PowerResult(ok: false, msg: "TrueNAS refused the login", auth: true) }
            if r.code != 200 { return PowerResult(ok: false, msg: "TrueNAS answered HTTP \(r.code)") }
            let v = str(r.json as? J ?? J(), "version")
            return PowerResult(ok: true, msg: "saved login works" + (v.isEmpty ? "" : " (\(v))"))
        }
        var h = auth
        h["Content-Type"] = "application/json"
        return await sendCommand("TrueNAS", action) { try await Net.request("\(base)/api/v2.0/system/\(action == "off" ? "shutdown" : "reboot")", method: "POST", headers: h, body: "{}") }
    }

    // --- Unraid ---
    static func unraid(_ action: String?, _ user: String, _ pass: String, _ base: String, _ s: J) async -> PowerResult {
        // 1. log in; the answer is a redirect that sets the session cookie, so don't follow it
        guard let lg = try? await Net.request(base + "/login", method: "POST", headers: FORM, body: formBody([("username", user.isEmpty ? "root" : user), ("password", pass)]), follow: false) else {
            return PowerResult(ok: false, msg: "Can't reach Unraid at " + base, net: true)
        }
        let loc = lg.header("Location") ?? ""
        if lg.code >= 300 && lg.code < 400 && test(loc, "^https?://", true) && hostOf(loc) != hostOf(base) {
            return PowerResult(ok: false, msg: "Unraid sends you on to \(replace(loc, "/login.*$", "", true)). Enter that as the server address in Settings.")
        }
        if lg.code < 300 || lg.code >= 400 {
            return test(lg.text, "password", true) ? PowerResult(ok: false, msg: "Unraid refused the login (wrong username or password).", auth: true) : PowerResult(ok: false, msg: "Unraid answered HTTP \(lg.code)")
        }
        var cookie = lg.cookies
        if cookie.isEmpty { cookie = (lg.header("Set-Cookie") ?? "").components(separatedBy: ";")[0] }
        // 2. the csrf_token every Unraid page carries
        let html = (try? await Net.request(base + "/Main", headers: ["Cookie": cookie]))?.text ?? ""
        guard let m = match(html, "csrf_token[\"'\\s]*[:=]\\s*[\"']([0-9A-Za-z]{8,})[\"']") else {
            return test(html, "name=[\"']?password", true) ? PowerResult(ok: false, msg: "Unraid refused the login.", auth: true) : PowerResult(ok: false, msg: "Logged in to Unraid, but couldn't read its security token.")
        }
        guard let action = action else { return PowerResult(ok: true, msg: "saved login works") }
        // 3. what Unraid's own Reboot / Shut down buttons send
        return await sendCommand("Unraid", action) {
            try await Net.request(base + "/webGui/include/Boot.php", method: "POST", headers: ["Cookie": cookie, "Content-Type": FORM["Content-Type"]!],
                                  body: formBody([("cmd", action == "off" ? "shutdown" : "reboot"), ("csrf_token", m[1])]))
        }
    }

    // --- Synology DSM ---
    static let SYNO_ERRORS: [Int: String] = [
        400: "wrong username or password", 401: "this account is disabled", 402: "this account isn't allowed to log in",
        403: "this account uses two-factor login. Use an administrator account without it", 404: "the two-factor code failed",
        406: "two-factor login is enforced for this account", 407: "too many failed logins: this address is blocked for a while",
    ]

    static func synoApi(_ base: String, _ cgi: String, _ params: [(String, Any)]) async throws -> J {
        let r = try await Net.request("\(base)/webapi/\(cgi)", method: "POST", headers: FORM, body: formBody(params))
        return r.json as? J ?? J()
    }

    static func synology(_ action: String?, _ user: String, _ pass: String, _ base: String, _ s: J) async -> PowerResult {
        let login: [(String, Any)] = [("api", "SYNO.API.Auth"), ("method", "login"), ("account", user), ("passwd", pass), ("session", "board"), ("format", "sid")]
        var r: J
        do {
            r = try await synoApi(base, "entry.cgi", login + [("version", 6)])
            // DSM 6: the login API lives in auth.cgi
            if !bool(r, "success"), [102, 103, 104].contains(int(r["error"] as? J ?? J(), "code")) {
                r = try await synoApi(base, "auth.cgi", login + [("version", 3)])
            }
        } catch { return PowerResult(ok: false, msg: "Can't reach Synology DSM at " + base, net: true) }
        let sid = str(r["data"] as? J ?? J(), "sid")
        if !bool(r, "success") || sid.isEmpty {
            let c = int(r["error"] as? J ?? J(), "code")
            return PowerResult(ok: false, msg: "Synology: " + (SYNO_ERRORS[c] ?? "login failed (error \(c))"), auth: [400, 401, 402, 403, 404, 406].contains(c))
        }
        guard let action = action else { return PowerResult(ok: true, msg: "saved login works") }
        guard let j = try? await synoApi(base, "entry.cgi", [("api", "SYNO.Core.System"), ("version", 1), ("method", action == "off" ? "shutdown" : "reboot"), ("_sid", sid)]) else {
            return PowerResult(ok: true, msg: powerSent(action))
        }
        if isBool(j["success"]) && !bool(j, "success") {
            let c = int(j["error"] as? J ?? J(), "code")
            return PowerResult(ok: false, msg: c == 105 ? "This Synology account isn't an administrator." : "Synology didn't accept the command (error \(c)).")
        }
        return PowerResult(ok: true, msg: powerSent(action))
    }

    // --- OpenMediaVault ---
    static func omvRpc(_ base: String, _ body: J, _ cookie: String? = nil) async throws -> Resp {
        var h = JSONH
        if let c = cookie, !c.isEmpty { h["Cookie"] = c }
        return try await Net.request(base + "/rpc.php", method: "POST", headers: h, body: toJSON(body))
    }

    static func omv(_ action: String?, _ user: String, _ pass: String, _ base: String, _ s: J) async -> PowerResult {
        guard let lg = try? await omvRpc(base, ["service": "Session", "method": "login", "params": ["username": user, "password": pass], "options": NSNull()]) else {
            return PowerResult(ok: false, msg: "Can't reach OpenMediaVault at " + base, net: true)
        }
        let r = lg.json as? J ?? J()
        let resp = r["response"] as? J ?? J()
        if str(resp, "status") == "challengeRequired" { return PowerResult(ok: false, msg: "This OpenMediaVault account uses two-factor login. Use an administrator account without it.", auth: true) }
        if !(bool(resp, "authenticated") || str(resp, "status") == "authenticated") {
            let m = str(r["error"] as? J ?? J(), "message")
            return PowerResult(ok: false, msg: "OpenMediaVault: " + (m.isEmpty ? "wrong username or password" : m), auth: true)
        }
        var cookie = lg.cookies
        if cookie.isEmpty && !str(resp, "sessionid").isEmpty { cookie = "X-OPENMEDIAVAULT-SESSIONID=" + str(resp, "sessionid") }
        guard let action = action else { return PowerResult(ok: true, msg: "saved login works") }
        // a 1 second delay lets OpenMediaVault answer before it goes down
        guard let c = try? await omvRpc(base, ["service": "System", "method": action == "off" ? "shutdown" : "reboot", "params": ["delay": 1], "options": NSNull()], cookie),
              let j = c.json as? J else { return PowerResult(ok: true, msg: powerSent(action)) }
        if let e = j["error"] as? J {
            let m = str(e, "message").isEmpty ? "error" : str(e, "message")
            return PowerResult(ok: false, msg: "OpenMediaVault didn't accept the command: " + m, auth: test(m, "session|authenticat", true))
        }
        return PowerResult(ok: true, msg: powerSent(action))
    }

    static func hasDriver(_ P: String) -> Bool { Logins.KEYS[P] != nil }

    static func driver(_ P: String, _ action: String?, _ user: String, _ pass: String, _ base: String, _ s: J) async -> PowerResult {
        switch P {
        case "zimaos": return await zima(action, user, pass, base, s)
        case "proxmox": return await pve(action, user, pass, base, s)
        case "truenas": return await truenas(action, user, pass, base, s)
        case "unraid": return await unraid(action, user, pass, base, s)
        case "synology": return await synology(action, user, pass, base, s)
        case "omv": return await omv(action, user, pass, base, s)
        default: return PowerResult(ok: false, msg: "This server type doesn't use a login.")
        }
    }

    // --- Linux (any systemd distro) via the board-power helper ---
    static func helperBase(_ s: J) -> String { platformBase("linux", s) }
    static func helperPort(_ s: J) -> Int {
        let b = helperBase(s)
        if let m = match(b, "^https?://[^/:]+:(\\d+)", true) { return Int(m[1]) ?? HELPER_PORT }
        return test(b, "^https:", true) ? 443 : 80
    }

    // returns the HTTP status code, or 0 when the helper can't be reached
    static func helperCall(_ base: String, _ key: String, _ path: String, _ method: String) async -> Int {
        (try? await Net.request(base + path, method: method, headers: ["Authorization": "Bearer " + key]))?.code ?? 0
    }

    static func helperPower(_ action: String, _ s: J) async -> PowerResult {
        if !Logins.hasLogin("linux") { return PowerResult(ok: false, msg: "Set up the power helper first: Settings > Server > Copy setup command.") }
        let base = helperBase(s)
        let code = await helperCall(base, Keychain.get(Logins.helperKey()) ?? "", action == "off" ? "/shutdown" : "/reboot", "POST")
        if code == 0 { return PowerResult(ok: false, msg: "Can't reach the power helper at \(base). Is it set up on the server?") }
        if code == 401 { return PowerResult(ok: false, msg: "The helper refused the key. Copy the setup command from Settings and run it again.") }
        if code >= 300 { return PowerResult(ok: false, msg: "The helper didn't accept the command (HTTP \(code)).") }
        return PowerResult(ok: true, msg: powerSent(action))
    }

    // What "Copy setup command" puts on the clipboard: paste it into an SSH session on the server.
    static func helperSetupScript(_ key: String, _ port: Int) -> String {
        func res(_ n: String, _ e: String) -> String {
            Bundle.main.url(forResource: n, withExtension: e).flatMap { try? String(contentsOf: $0, encoding: .utf8) } ?? ""
        }
        return res("helper_setup", "sh").replacingOccurrences(of: "{{HELPER_PY}}", with: res("helper", "py"))
            .replacingOccurrences(of: "{{KEY}}", with: key).replacingOccurrences(of: "{{PORT}}", with: String(port))
    }

    // d: { a: "restart" | "off", u, p } (u/p only when typed in the popup; otherwise the saved login is used)
    static func handlePower(_ d: J, _ s: J) async -> J {
        let a = str(d, "a") == "off" ? "off" : "restart"
        let P = str(s, "platform")
        var r: PowerResult
        if PLATFORMS[P] == nil {
            r = PowerResult(ok: false, msg: "Pick your server type first: Settings > Server > Server type.")
        } else if P == "linux" {
            r = await helperPower(a, s)
        } else {
            var user = str(d, "u"), pass = str(d, "p"), saved = false
            if (user.isEmpty || pass.isEmpty) && Logins.hasLogin(P) { (user, pass) = Logins.saved(P); saved = true }
            r = user.isEmpty || pass.isEmpty ? PowerResult(ok: false, msg: "", auth: true) : await driver(P, a, user, pass, platformBase(P, s), s)
            if r.auth && saved { Logins.forget(P) }
            if !r.auth && !r.net && !saved && !user.isEmpty && !pass.isEmpty { Logins.save(P, user, pass) }
        }
        return ["ok": r.ok, "msg": r.msg, "auth": r.auth, "a": a, "hasLogin": Logins.hasLogin(P)]
    }

    // One line for "Test connection" about the Reboot / Shut down side of the chosen server type
    static func testPower(_ s: J) async -> J {
        let P = str(s, "platform")
        guard let pl = PLATFORMS[P] else { return ["ok": false, "t": "Server type: not picked yet, so Reboot / Shut down is off. Pick it under SERVER TYPE."] }
        let base = platformBase(P, s)
        if P == "linux" {
            if !Logins.hasLogin("linux") { return ["ok": false, "t": "Power helper: not set up yet (tap Copy setup command)"] }
            let code = await helperCall(base, Keychain.get(Logins.helperKey()) ?? "", "/health", "GET")
            if code == 200 { return ["ok": true, "t": "Power helper: OK at " + base] }
            if code == 401 { return ["ok": false, "t": "Power helper: answers, but refused the key. Run the setup command again."] }
            return ["ok": false, "t": "Power helper: no answer at " + base + (code != 0 ? " (HTTP \(code))" : "")]
        }
        if Logins.hasLogin(P) {
            let (user, pass) = Logins.saved(P)
            let r = await driver(P, nil, user, pass, base, s)
            if r.ok { return ["ok": true, "t": "\(pl.name): OK, \(r.msg)"] }
            return ["ok": false, "t": "\(pl.name): " + (r.auth ? "the saved login was refused. " : "") + (r.msg.isEmpty ? "no answer at " + base : r.msg)]
        }
        if (try? await Net.request(base, timeout: 6)) != nil { return ["ok": true, "t": "\(pl.name): reachable at \(base) (no login saved yet)"] }
        return ["ok": false, "t": "\(pl.name): no answer at \(base)"]
    }
}
