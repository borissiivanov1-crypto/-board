import Foundation
import UIKit

// ---------- Service logos (downloaded once, shrunk, stored on the phone) ----------
enum Logos {
    static let SIZE: CGFloat = 96
    static let RETRY_HOURS = 12.0

    static var dir: URL {
        let d = Shared.dir.appendingPathComponent("board-logos")
        try? FileManager.default.createDirectory(at: d, withIntermediateDirectories: true)
        return d
    }

    // The same names as the Scriptable version
    static func key(_ url: String) -> String {
        var h: UInt32 = 5381
        for c in url.utf16 { h = (h &* 33) ^ UInt32(c) }
        return String(h, radix: 36) + "-" + String(url.utf16.count, radix: 36)
    }

    static func path(_ url: String) -> URL { dir.appendingPathComponent(key(url) + ".png") }

    static func cached(_ url: String) -> UIImage? {
        if url.isEmpty { return nil }
        return UIImage(contentsOfFile: path(url).path)
    }

    private static var b64: [String: String] = [:]
    private static let lock = NSLock()

    static func base64(_ url: String) -> String? {
        if url.isEmpty { return nil }
        lock.lock(); defer { lock.unlock() }
        if let b = b64[url] { return b }
        guard let d = try? Data(contentsOf: path(url)) else { return nil }
        let s = d.base64EncodedString()
        b64[url] = s
        return s
    }

    static func clearFail(_ url: String) {
        if url.isEmpty { return }
        try? FileManager.default.removeItem(at: URL(fileURLWithPath: path(url).path + ".fail"))
    }

    // Fit the logo inside a SIZE square, keeping its shape and transparency
    static func shrink(_ img: UIImage) -> Data? {
        let w0 = img.size.width, h0 = img.size.height
        guard w0 > 0, h0 > 0 else { return nil }
        let scale = min(1, SIZE / max(w0, h0))
        let w = max(1, (w0 * scale).rounded()), h = max(1, (h0 * scale).rounded())
        let side = max(w, h)
        let f = UIGraphicsImageRendererFormat()
        f.scale = 1
        f.opaque = false
        return UIGraphicsImageRenderer(size: CGSize(width: side, height: side), format: f).pngData { _ in
            img.draw(in: CGRect(x: (side - w) / 2, y: (side - h) / 2, width: w, height: h))
        }
    }

    // Downloads a logo if it isn't saved yet. Returns true only when something new was saved.
    static func fetch(_ url: String, _ timeout: Double) async -> Bool {
        guard test(url, "^https?://", true) else { return false }
        let p = path(url)
        let fail = URL(fileURLWithPath: p.path + ".fail")
        let fm = FileManager.default
        if fm.fileExists(atPath: p.path) { return false }
        if let a = try? fm.attributesOfItem(atPath: fail.path), let m = a[.modificationDate] as? Date,
           Date().timeIntervalSince(m) < RETRY_HOURS * 3600 { return false }
        do {
            let r = try await Net.request(url, timeout: timeout)
            guard r.code < 400, let img = UIImage(data: r.data), let png = shrink(img) else { throw URLError(.cannotDecodeContentData) }
            try png.write(to: p)
            try? fm.removeItem(at: fail)
            return true
        } catch {
            try? String(Int(nowMs())).write(to: fail, atomically: true, encoding: .utf8)
            return false
        }
    }

    static func ensure(_ list: [J], _ timeout: Double) async -> Bool {
        var urls = [String]()
        for s in list { let u = str(s, "logo"); if !u.isEmpty && !urls.contains(u) { urls.append(u) } }
        return await withTaskGroup(of: Bool.self) { g -> Bool in
            for u in urls { g.addTask { await fetch(u, timeout) } }
            var any = false
            for await r in g where r { any = true }
            return any
        }
    }

    // Removes saved logos that no button uses any more (of any server)
    static func prune() {
        var keep = Set<String>()
        for sv in Store.shared.listServers() {
            let L = Store.shared.getLayout(str(sv, "id"))
            for s in L.all + L.groups where !str(s, "logo").isEmpty { keep.insert(key(str(s, "logo"))) }
        }
        for f in (try? FileManager.default.contentsOfDirectory(atPath: dir.path)) ?? [] {
            if !keep.contains(f.components(separatedBy: ".")[0]) { try? FileManager.default.removeItem(at: dir.appendingPathComponent(f)) }
        }
    }

    static func clearAll() {
        for f in (try? FileManager.default.contentsOfDirectory(atPath: dir.path)) ?? [] { try? FileManager.default.removeItem(at: dir.appendingPathComponent(f)) }
        lock.lock(); b64 = [:]; lock.unlock()
    }
}
