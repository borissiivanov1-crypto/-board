import Foundation

struct Resp: @unchecked Sendable {
    let code: Int
    let data: Data
    let http: HTTPURLResponse?
    var text: String { String(decoding: data, as: UTF8.self) }
    var json: Any? { parseJSON(data) }
    func header(_ name: String) -> String? { http?.value(forHTTPHeaderField: name) }
    // "name=value; ..." from the cookies the response set
    var cookies: String {
        guard let h = http, let url = h.url, let f = h.allHeaderFields as? [String: String] else { return "" }
        return HTTPCookie.cookies(withResponseHeaderFields: f, for: url).map { "\($0.name)=\($0.value)" }.joined(separator: "; ")
    }
}

struct TimeoutError: Error {}

func withTimeout<T: Sendable>(_ seconds: Double, _ op: @escaping () async throws -> T) async throws -> T {
    try await withThrowingTaskGroup(of: T.self) { g in
        g.addTask { try await op() }
        g.addTask {
            try await Task.sleep(nanoseconds: UInt64(seconds * 1_000_000_000))
            throw TimeoutError()
        }
        let r = try await g.next()!
        g.cancelAll()
        return r
    }
}

final class Net: NSObject, URLSessionDelegate, URLSessionTaskDelegate {
    static let shared = Net()

    private func config() -> URLSessionConfiguration {
        let c = URLSessionConfiguration.ephemeral
        c.httpShouldSetCookies = false
        c.httpCookieAcceptPolicy = .never
        c.requestCachePolicy = .reloadIgnoringLocalCacheData
        c.waitsForConnectivity = false
        return c
    }

    lazy var session = URLSession(configuration: config(), delegate: self, delegateQueue: nil)
    lazy var noRedirect = URLSession(configuration: config(), delegate: self, delegateQueue: nil)

    // Accepts self-signed certificates (most NAS systems use one out of the box)
    func urlSession(_ session: URLSession, didReceive challenge: URLAuthenticationChallenge, completionHandler: @escaping (URLSession.AuthChallengeDisposition, URLCredential?) -> Void) {
        if challenge.protectionSpace.authenticationMethod == NSURLAuthenticationMethodServerTrust, let t = challenge.protectionSpace.serverTrust {
            completionHandler(.useCredential, URLCredential(trust: t))
        } else {
            completionHandler(.performDefaultHandling, nil)
        }
    }

    func urlSession(_ session: URLSession, task: URLSessionTask, didReceive challenge: URLAuthenticationChallenge, completionHandler: @escaping (URLSession.AuthChallengeDisposition, URLCredential?) -> Void) {
        urlSession(session, didReceive: challenge, completionHandler: completionHandler)
    }

    func urlSession(_ session: URLSession, task: URLSessionTask, willPerformHTTPRedirection response: HTTPURLResponse, newRequest request: URLRequest, completionHandler: @escaping (URLRequest?) -> Void) {
        completionHandler(session === noRedirect ? nil : request)
    }

    static func request(_ url: String, method: String = "GET", headers: [String: String] = [:], body: String? = nil, timeout: Double = 8, follow: Bool = true) async throws -> Resp {
        guard let u = URL(string: url), u.scheme != nil, u.host != nil else { throw URLError(.badURL) }
        var r = URLRequest(url: u, timeoutInterval: timeout)
        r.httpMethod = method
        for (k, v) in headers { r.setValue(v, forHTTPHeaderField: k) }
        if let b = body { r.httpBody = Data(b.utf8) }
        let s = follow ? shared.session : shared.noRedirect
        return try await withTimeout(timeout + 0.5) {
            let (d, resp) = try await s.data(for: r)
            let h = resp as? HTTPURLResponse
            return Resp(code: h?.statusCode ?? 0, data: d, http: h)
        }
    }

    // ---------- Service status (up/down dots) ----------
    // Any HTTP answer (even an error page) counts as up. Refused or timed-out connections count as down.
    // A certificate problem or a non-web port (like SSH) still means something is listening.
    static func listening(_ e: Error) -> Bool {
        let code = (e as NSError).code
        let listeningCodes = [NSURLErrorBadServerResponse, NSURLErrorCannotParseResponse, NSURLErrorZeroByteResource, NSURLErrorCannotDecodeRawData,
                              NSURLErrorSecureConnectionFailed, NSURLErrorServerCertificateHasBadDate, NSURLErrorServerCertificateUntrusted,
                              NSURLErrorServerCertificateHasUnknownRoot, NSURLErrorServerCertificateNotYetValid, NSURLErrorClientCertificateRejected,
                              NSURLErrorClientCertificateRequired, NSURLErrorHTTPTooManyRedirects]
        return (e as NSError).domain == NSURLErrorDomain && listeningCodes.contains(code)
    }

    static func pingUrl(_ url: String, _ timeout: Double) async -> Bool {
        do { _ = try await request(url, method: "HEAD", timeout: timeout); return true } catch { return listening(error) }
    }

    static func checkServices(_ list: [J], _ timeout: Double) async -> [String: Bool] {
        var urls = [String]()
        for s in list where !urls.contains(str(s, "url")) { urls.append(str(s, "url")) }
        let byUrl = await withTaskGroup(of: (String, Bool).self) { g -> [String: Bool] in
            for u in urls { g.addTask { (u, await pingUrl(u, timeout)) } }
            var m = [String: Bool]()
            for await (u, ok) in g { m[u] = ok }
            return m
        }
        var out = [String: Bool]()
        for s in list { out[str(s, "id")] = byUrl[str(s, "url")] ?? false }
        return out
    }

    // One place on the server: a quick HEAD request; some servers turn HEAD away, so a refusal that isn't a
    // timeout gets one ordinary GET before the place counts as not answering
    static func reachProbe(_ url: String, _ timeout: Double) async -> Bool {
        let t0 = nowMs()
        if await pingUrl(url, timeout) { return true }
        if nowMs() - t0 >= timeout * 1000 - 200 { return false }
        do { _ = try await request(url, timeout: timeout); return true } catch { return listening(error) }
    }

    // ---------- Is the server reachable? (the LIVE / OFFLINE status) ----------
    // Asks the usual places on the server at once; returns what answered first, or "" for none.
    static func serverReachable(_ s: J, _ services: [J], timeout: Double = 4) async -> String {
        let host = serverHostOf(s)
        if host.isEmpty { return "" }
        var urls = [String]()
        func add(_ u: String) { if !urls.contains(u) { urls.append(u) } }
        add(serverUrlOf(s) + "/"); add("http://\(host)/"); add("https://\(host)/")
        if PLATFORMS[str(s, "platform")] != nil { add(platformBase(str(s, "platform"), s) + "/") }
        if bool(s, "monitoring") { add(glancesUrl(s) + "/") }
        for sv in services where hostOf(str(sv, "url")) == host && urls.count < 12 { add(str(sv, "url")) }
        func label(_ u: String) -> String { replace(match(u, "^https?://[^/]+", true)?[0] ?? u, "^https?://", "", true) }
        return await withTaskGroup(of: String.self) { g -> String in
            for u in urls { g.addTask { await reachProbe(u, timeout) ? label(u) : "" } }
            for await r in g where !r.isEmpty { g.cancelAll(); return r }
            return ""
        }
    }
}
