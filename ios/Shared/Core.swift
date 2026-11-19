import Foundation

// The Scriptable script works on plain JavaScript objects; this port keeps that shape ([String: Any]),
// so the data file, exports and the dashboard page stay exactly the same.
typealias J = [String: Any]

// ---------- JSON ----------
// An object whose keys keep their order when written (themes and accents are listed in this order in Settings)
struct OJ { var pairs: [(String, Any)] }

func isBool(_ v: Any?) -> Bool {
    guard let n = v as? NSNumber else { return false }
    return CFGetTypeID(n) == CFBooleanGetTypeID()
}

func isNum(_ v: Any?) -> Bool {
    guard let v = v, !(v is String), let n = v as? NSNumber else { return false }
    return CFGetTypeID(n) != CFBooleanGetTypeID()
}

func jsonQuote(_ s: String) -> String {
    var out = "\""
    out.reserveCapacity(s.utf16.count + 2)
    for u in s.unicodeScalars {
        switch u {
        case "\"": out += "\\\""
        case "\\": out += "\\\\"
        case "\n": out += "\\n"
        case "\r": out += "\\r"
        case "\t": out += "\\t"
        case "\u{2028}": out += "\\u2028"
        case "\u{2029}": out += "\\u2029"
        default:
            if u.value < 0x20 { out += String(format: "\\u%04x", u.value) } else { out.unicodeScalars.append(u) }
        }
    }
    return out + "\""
}

func numString(_ d: Double) -> String {
    if !d.isFinite { return "null" }
    if d == d.rounded(), abs(d) < 1e15 { return String(Int64(d)) }
    return String(d)
}

func toJSON(_ v: Any?, pretty: Bool = false, indent: String = "") -> String {
    let nl = pretty ? "\n" : ""
    let ind = pretty ? indent + "  " : ""
    let sep = pretty ? ": " : ":"
    guard let v = v else { return "null" }
    if v is NSNull { return "null" }
    if isBool(v) { return (v as! NSNumber).boolValue ? "true" : "false" }
    if let s = v as? String { return jsonQuote(s) }
    if let o = v as? OJ {
        if o.pairs.isEmpty { return "{}" }
        return "{" + nl + o.pairs.map { ind + jsonQuote($0.0) + sep + toJSON($0.1, pretty: pretty, indent: ind) }.joined(separator: "," + nl) + nl + indent + "}"
    }
    if let d = v as? J {
        if d.isEmpty { return "{}" }
        return "{" + nl + d.keys.sorted().map { ind + jsonQuote($0) + sep + toJSON(d[$0], pretty: pretty, indent: ind) }.joined(separator: "," + nl) + nl + indent + "}"
    }
    if let a = v as? [Any] {
        if a.isEmpty { return "[]" }
        return "[" + nl + a.map { ind + toJSON($0, pretty: pretty, indent: ind) }.joined(separator: "," + nl) + nl + indent + "]"
    }
    if let a = v as? [Any?] { return toJSON(a.map { $0 ?? NSNull() }, pretty: pretty, indent: indent) }
    if let n = v as? NSNumber { return numString(n.doubleValue) }
    return "null"
}

func parseJSON(_ s: String) -> Any? {
    guard let d = s.data(using: .utf8), !d.isEmpty else { return nil }
    return try? JSONSerialization.jsonObject(with: d, options: [.fragmentsAllowed])
}

func parseJSON(_ d: Data) -> Any? { try? JSONSerialization.jsonObject(with: d, options: [.fragmentsAllowed]) }

// ---------- Small JavaScript-style helpers ----------
func jsNumber(_ v: Any?) -> Double? {
    guard let v = v, !(v is NSNull) else { return nil }
    if isBool(v) { return (v as! NSNumber).boolValue ? 1 : 0 }
    if let s = v as? String { let t = s.trimmingCharacters(in: .whitespaces); return t.isEmpty ? 0 : Double(t) }
    if let n = v as? NSNumber { return n.doubleValue }
    return nil
}

func jsRound(_ x: Double) -> Double { (x + 0.5).rounded(.down) }

func num(_ v: Any?) -> Double? { isNum(v) ? (v as! NSNumber).doubleValue : nil }

func clampNum(_ v: Any?, _ lo: Int, _ hi: Int, _ def: Int) -> Int {
    guard let v = v, !(v is NSNull) else { return def }
    if let s = v as? String, s.isEmpty { return def }
    guard let n = jsNumber(v), n.isFinite else { return def }
    return Int(min(Double(hi), max(Double(lo), jsRound(n))))
}

func str(_ d: J, _ k: String) -> String { d[k] as? String ?? "" }
func bool(_ d: J, _ k: String) -> Bool { isBool(d[k]) ? (d[k] as! NSNumber).boolValue : false }
func int(_ d: J, _ k: String) -> Int { Int(jsNumber(d[k]) ?? 0) }
func strs(_ v: Any?) -> [String] { (v as? [Any] ?? []).compactMap { $0 as? String } }
func objs(_ v: Any?) -> [J] { (v as? [Any] ?? []).compactMap { $0 as? J } }

// ---------- Regular expressions ----------
private var reCache: [String: NSRegularExpression] = [:]
private let reLock = NSLock()

func regex(_ p: String, _ i: Bool = false) -> NSRegularExpression {
    let key = (i ? "i:" : "c:") + p
    reLock.lock(); defer { reLock.unlock() }
    if let r = reCache[key] { return r }
    let r = try! NSRegularExpression(pattern: p, options: i ? [.caseInsensitive] : [])
    reCache[key] = r
    return r
}

// groups of the first match ("" for a group that didn't take part), or nil
func match(_ s: String, _ p: String, _ i: Bool = false) -> [String]? {
    let ns = s as NSString
    guard let m = regex(p, i).firstMatch(in: s, range: NSRange(location: 0, length: ns.length)) else { return nil }
    return (0..<m.numberOfRanges).map { m.range(at: $0).location == NSNotFound ? "" : ns.substring(with: m.range(at: $0)) }
}

func test(_ s: String, _ p: String, _ i: Bool = false) -> Bool {
    regex(p, i).firstMatch(in: s, range: NSRange(location: 0, length: (s as NSString).length)) != nil
}

// replaces every match ($1 ... in the template)
func replace(_ s: String, _ p: String, _ with: String, _ i: Bool = false) -> String {
    regex(p, i).stringByReplacingMatches(in: s, range: NSRange(location: 0, length: (s as NSString).length), withTemplate: with)
}

func replaceFirst(_ s: String, _ p: String, _ with: String, _ i: Bool = false) -> String {
    let ns = s as NSString
    guard let m = regex(p, i).firstMatch(in: s, range: NSRange(location: 0, length: ns.length)) else { return s }
    let rep = regex(p, i).replacementString(for: m, in: s, offset: 0, template: with)
    return ns.replacingCharacters(in: m.range, with: rep)
}

func trim(_ s: String) -> String { s.trimmingCharacters(in: .whitespacesAndNewlines) }
func clip(_ s: String, _ n: Int) -> String { String(s.prefix(n)) }

func encodeURIComponent(_ s: String) -> String {
    var allowed = CharacterSet.alphanumerics
    allowed.insert(charactersIn: "-_.!~*'()")
    return s.addingPercentEncoding(withAllowedCharacters: allowed) ?? s
}

func nowMs() -> Double { Date().timeIntervalSince1970 * 1000 }

// ---------- Where the data lives (shared by the app and its widget) ----------
enum Shared {
    // SideStore / AltStore rename the app group for your Apple ID and list the new name under ALTAppGroups
    static var appGroup: String {
        (Bundle.main.object(forInfoDictionaryKey: "ALTAppGroups") as? [String])?.first ?? "group.app.board"
    }

    static var dir: URL {
        if let u = FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: appGroup) { return u }
        return FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[0]
    }
}

// ---------- Logins (the iOS Keychain, shared with the widget through the app group) ----------
enum Keychain {
    private static func base(_ k: String) -> [String: Any] {
        [kSecClass as String: kSecClassGenericPassword, kSecAttrService as String: "board", kSecAttrAccount as String: k]
    }

    static func get(_ k: String) -> String? {
        var q = base(k)
        q[kSecReturnData as String] = true
        q[kSecMatchLimit as String] = kSecMatchLimitOne
        var out: AnyObject?
        guard SecItemCopyMatching(q as CFDictionary, &out) == errSecSuccess, let d = out as? Data else { return nil }
        return String(data: d, encoding: .utf8)
    }

    static func contains(_ k: String) -> Bool { get(k) != nil }

    static func set(_ k: String, _ v: String) {
        remove(k)
        var q = base(k)
        q[kSecValueData as String] = Data(v.utf8)
        q[kSecAttrAccessible as String] = kSecAttrAccessibleAfterFirstUnlock
        var withGroup = q
        withGroup[kSecAttrAccessGroup as String] = Shared.appGroup
        if SecItemAdd(withGroup as CFDictionary, nil) != errSecSuccess {
            SecItemAdd(q as CFDictionary, nil)
        }
    }

    static func remove(_ k: String) { SecItemDelete(base(k) as CFDictionary) }
}
