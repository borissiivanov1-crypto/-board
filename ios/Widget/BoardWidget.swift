import AppIntents
import SwiftUI
import UIKit
import WidgetKit

// Edit Widget > Server: its name or number (1 = the first), like the Scriptable widget's Parameter. Empty = the first server.
struct BoardIntent: WidgetConfigurationIntent {
    static var title: LocalizedStringResource = "-board"
    static var description = IntentDescription("Shows one of your servers.")
    @Parameter(title: "Server (name or number)") var server: String?
}

struct Cell {
    let id: String, name: String, icon: String
    let logo: UIImage?
    let url: URL?
    let dot: Color?
}

struct BoardEntry: TimelineEntry {
    let date: Date
    var serverId = "s1"
    var title = "MY SERVER"
    var setup = false
    var reach = false
    var monitoring = true
    var stats: J?
    var cells: [Cell] = []
    var upCount = 0, total = 0
    var checked = false
    var linkUrl = ""
    var glass = false
    var look = WidgetLook(theme: "aurora", glow: "normal")
}

struct WidgetLook {
    let bg: Color, g1: Color, g2: Color, strength: Double
    init(theme: String, glow: String) {
        let t = app_theme(theme)
        bg = Color(hex: t.bg)
        g1 = Color(rgb: t.g1)
        g2 = Color(rgb: t.g2)
        strength = t.noGlow ? 0 : (GLOW_LEVELS[glow] ?? 0.5) / 0.5
    }
}

func app_theme(_ k: String) -> Theme { theme(k) }

extension Color {
    init(hex: String) {
        let v = UInt32(hex.replacingOccurrences(of: "#", with: ""), radix: 16) ?? 0
        self.init(red: Double((v >> 16) & 0xff) / 255, green: Double((v >> 8) & 0xff) / 255, blue: Double(v & 0xff) / 255)
    }
    init(rgb: String) {
        let p = rgb.split(separator: ",").map { Double($0.trimmingCharacters(in: .whitespaces)) ?? 0 }
        self.init(red: p[0] / 255, green: p[1] / 255, blue: p[2] / 255)
    }
}

let GREEN = Color(hex: "#34e0a1"), RED = Color(hex: "#ff5d73"), ORANGE = Color(hex: "#ffb454")

struct Provider: AppIntentTimelineProvider {
    func placeholder(in context: Context) -> BoardEntry { BoardEntry(date: Date(), reach: true, stats: ["cpu": 23, "ram": 61, "storage": ["percent": 48] as J, "net": ["down": 1_200_000, "up": 240_000] as J]) }

    func snapshot(for configuration: BoardIntent, in context: Context) async -> BoardEntry {
        context.isPreview ? placeholder(in: context) : await load(configuration, context.family)
    }

    func timeline(for configuration: BoardIntent, in context: Context) async -> Timeline<BoardEntry> {
        let e = await load(configuration, context.family)
        let s = Store.shared.settings(e.serverId)
        return Timeline(entries: [e], policy: .after(Date().addingTimeInterval(Double(int(s, "refreshMinutes")) * 60)))
    }

    private func load(_ cfg: BoardIntent, _ family: WidgetFamily) async -> BoardEntry {
        let store = Store.shared
        store.reload()
        let sid = store.resolveServer(cfg.server)
        let s = store.settings(sid)
        var e = BoardEntry(date: Date())
        e.serverId = sid
        e.title = str(s, "widgetTitle").isEmpty ? store.getTitle(sid) : str(s, "widgetTitle")
        e.monitoring = bool(s, "monitoring")
        e.linkUrl = str(s, "linkUrl")
        e.glass = str(s, "style") == "glass"
        e.look = WidgetLook(theme: str(s, "theme"), glow: str(s, "glow"))
        // not set up yet: ask to open the app
        if serverHostOf(s).isEmpty { e.setup = true; return e }
        let small = family == .systemSmall, large = family == .systemLarge
        let layout = store.getLayout(sid)
        // everything runs side by side: is the server reachable, the buttons' checks and (with Monitoring) the stats
        async let reachT = Net.serverReachable(s, layout.all)
        async let gridT: ([Cell], Int, Int)? = large || !e.monitoring ? prepareGrid(s, sid, layout, small ? 0 : large ? 99 : 8) : nil
        if e.monitoring {
            // the small widget only shows CPU and RAM
            e.stats = try? await Glances(s, serverId: sid).getStats(disks: !small, net: !small)
        }
        // Glances answering means the server is up
        e.reach = e.stats != nil ? true : !(await reachT).isEmpty
        if let g = await gridT { e.cells = g.0; e.upCount = g.1; e.total = g.2 }
        e.checked = bool(s, "widgetStatusDots")
        return e
    }

    // status checks and any missing logo downloads run together (logos are only downloaded once)
    private func prepareGrid(_ s: J, _ sid: String, _ layout: Store.Layout, _ maxCells: Int) async -> ([Cell], Int, Int) {
        let list = Array(Store.shared.widgetCells(layout, int(s, "widgetRows")).prefix(maxCells == 0 ? 99 : maxCells))
        var toCheck = [J]()
        for c in list { if str(c, "kind") == "group" { toCheck += objs(c["members"]) } else { toCheck.append(c) } }
        let dots = bool(s, "widgetStatusDots")
        async let statusT: [String: Bool] = dots ? Net.checkServices(toCheck, Double(min(int(s, "pingTimeout"), 3))) : [:]
        if bool(s, "useLogos") { _ = await Logos.ensure(list, 4) }
        let status = await statusT
        // stack dot: green = all up, orange = some down, red = all down
        func dot(_ c: J) -> Color? {
            if !dots { return nil }
            if str(c, "kind") != "group" { return status[str(c, "id")] == true ? GREEN : RED }
            let m = objs(c["members"])
            let up = m.filter { status[str($0, "id")] == true }.count
            return up == m.count ? GREEN : up == 0 ? RED : ORANGE
        }
        let cells = list.map { c -> Cell in
            let url: URL? = str(c, "kind") == "group"
                ? URL(string: "board://open?server=\(encodeURIComponent(sid))&stack=\(encodeURIComponent(str(c, "id")))")
                : URL(string: str(c, "url"))
            return Cell(id: str(c, "id"), name: str(c, "name"), icon: str(c, "icon"), logo: bool(s, "useLogos") ? Logos.cached(str(c, "logo")) : nil, url: url, dot: dot(c))
        }
        return (cells, toCheck.filter { status[str($0, "id")] == true }.count, toCheck.count)
    }
}

// ---------- Drawing ----------
struct Ring: View {
    let pct: Double
    let glass: Bool
    var body: some View {
        let c = Color(hex: colorHexFor(pct))
        let p = max(0.01, min(pct, 100) / 100)
        ZStack {
            Circle().stroke(Color.white.opacity(0.12), lineWidth: 6)
            Circle().trim(from: 0, to: p).stroke(c.opacity(0.16), style: StrokeStyle(lineWidth: 9, lineCap: .round)).rotationEffect(.degrees(-90))
            Circle().trim(from: 0, to: p).stroke(c, style: StrokeStyle(lineWidth: 6, lineCap: .round)).rotationEffect(.degrees(-90))
            if glass {
                Circle().fill(Color.white.opacity(0.07)).overlay(Circle().stroke(Color.white.opacity(0.16), lineWidth: 0.5)).padding(7)
            }
            Text("\(Int(jsRound(pct)))%").font(.system(size: 13, weight: .bold, design: .rounded)).foregroundColor(.white)
        }
        .frame(width: 48, height: 48)
    }
}

// Widget pill / button background. Classic: flat see-through fill. Liquid Glass: top-lit gradient fill + a thin bright rim.
struct Glass: ViewModifier {
    let glass: Bool
    let radius: CGFloat
    var k: Double = 1
    var flat: Double = 0.1
    func body(content: Content) -> some View {
        if glass {
            content.background(
                RoundedRectangle(cornerRadius: radius + 1, style: .continuous)
                    .fill(LinearGradient(stops: [.init(color: .white.opacity(0.2 * k), location: 0), .init(color: .white.opacity(0.06 * k), location: 0.55), .init(color: .white.opacity(0.1 * k), location: 1)],
                                         startPoint: .top, endPoint: UnitPoint(x: 0.4, y: 1)))
                    .overlay(RoundedRectangle(cornerRadius: radius + 1, style: .continuous).stroke(Color.white.opacity(0.22 * k), lineWidth: 1))
            )
        } else {
            content.background(RoundedRectangle(cornerRadius: radius, style: .continuous).fill(Color.white.opacity(flat)))
        }
    }
}

extension View {
    func glass(_ g: Bool, _ r: CGFloat, k: Double = 1, flat: Double = 0.1) -> some View { modifier(Glass(glass: g, radius: r, k: k, flat: flat)) }
}

struct Background: View {
    let look: WidgetLook
    var body: some View {
        GeometryReader { geo in
            ZStack {
                look.bg
                if look.strength > 0 {
                    RadialGradient(colors: [look.g1.opacity(0.33 * look.strength), look.g1.opacity(0)], center: UnitPoint(x: 0.14, y: 0.06), startRadius: 0, endRadius: geo.size.width * 0.45)
                    RadialGradient(colors: [look.g2.opacity(0.33 * look.strength), look.g2.opacity(0)], center: UnitPoint(x: 0.94, y: 1.06), startRadius: 0, endRadius: geo.size.width * 0.5)
                }
            }
        }
    }
}

struct BoardWidgetView: View {
    let e: BoardEntry
    @Environment(\.widgetFamily) var family

    var small: Bool { family == .systemSmall }
    var large: Bool { family == .systemLarge }

    var body: some View {
        content
            .padding(EdgeInsets(top: 12, leading: 14, bottom: 12, trailing: 14))
            .widgetURL(URL(string: "board://open?server=\(encodeURIComponent(e.serverId))"))
            .containerBackground(for: .widget) { Background(look: e.look) }
    }

    @ViewBuilder var content: some View {
        if e.setup {
            VStack(alignment: .leading) {
                Text(e.title).font(.system(size: 12, weight: .heavy, design: .rounded)).foregroundColor(.white)
                Spacer()
                Text("Tap to set up -board:\nenter your server's address in Settings.").font(.system(size: 12)).foregroundColor(.white.opacity(0.75))
                Spacer()
            }.frame(maxWidth: .infinity, alignment: .leading)
        } else {
            VStack(alignment: .leading, spacing: 0) {
                header
                body2
            }
        }
    }

    var header: some View {
        HStack(spacing: 0) {
            Text(e.title).font(.system(size: 12, weight: .heavy, design: .rounded)).foregroundColor(.white).lineLimit(1)
            Spacer(minLength: 4)
            HStack(spacing: 4) {
                Text("●").font(.system(size: 8)).foregroundColor(e.reach ? GREEN : RED)
                Text(e.reach ? e.date.formatted(.dateTime.hour(.twoDigits(amPM: .omitted)).minute(.twoDigits)) : "OFFLINE")
                    .font(.system(size: 10, weight: .bold, design: .monospaced)).foregroundColor(.white)
            }
            .padding(.vertical, 3).padding(.horizontal, 8).glass(e.glass, 9)
            if !small && !e.linkUrl.isEmpty, let u = URL(string: e.linkUrl) {
                Link(destination: u) {
                    Image(systemName: "arrow.up.right").font(.system(size: 11, weight: .bold)).foregroundColor(.white)
                        .frame(width: 24, height: 24).glass(e.glass, 12, k: 1.3, flat: 0.14)
                }.padding(.leading, 6)
            }
        }
    }

    @ViewBuilder var body2: some View {
        if !e.monitoring {
            // Monitoring off: service buttons (medium / large) or a summary (small)
            if small {
                Spacer()
                Text(e.reach ? "ONLINE" : "OFFLINE").font(.system(size: 20, weight: .heavy, design: .rounded)).foregroundColor(e.reach ? GREEN : RED)
                if e.checked && e.total > 0 {
                    Text("\(e.upCount) of \(e.total) services up").font(.system(size: 11, weight: .medium)).foregroundColor(.white.opacity(0.75)).padding(.top, 2)
                }
                Spacer()
            } else {
                Spacer().frame(height: 10)
                if !e.cells.isEmpty { grid } else {
                    Text(e.reach ? "Add service buttons in the dashboard with +." : "Can't reach the server.\nCheck the address in Settings, and your VPN if you're away from home.")
                        .font(.system(size: 11)).foregroundColor(.white.opacity(0.7))
                }
                Spacer(minLength: 0)
            }
        } else if let st = e.stats {
            Spacer().frame(height: 6)
            gauges(st)
            if !small {
                if large { Spacer().frame(height: 10); grid }
                Spacer(minLength: 4)
                net(st)
            } else { Spacer(minLength: 0) }
        } else {
            Spacer()
            Text(e.reach ? "The server is up, but Glances isn't answering.\nCheck the Glances address in Settings > Monitoring."
                 : "Can't reach the server.\nCheck the address in Settings, and your VPN if you're away from home.")
                .font(.system(size: 11)).foregroundColor(.white.opacity(0.7))
            Spacer()
            if large && !e.cells.isEmpty { grid; Spacer() }
        }
    }

    func gauges(_ st: J) -> some View {
        var items: [(String, Double)] = [("CPU", n0(st, "cpu")), ("RAM", n0(st, "ram"))]
        if let s = st["storage"] as? J { items.append(("STORAGE", n0(s, "percent"))) }
        let shown = small ? Array(items.prefix(2)) : items
        return HStack(spacing: 0) {
            ForEach(Array(shown.enumerated()), id: \.offset) { i, it in
                if i > 0 { Spacer(minLength: 0) }
                VStack(spacing: 2) {
                    Ring(pct: it.1, glass: e.glass)
                    Text(it.0).font(.system(size: 9, weight: .bold, design: .rounded)).foregroundColor(.white.opacity(0.7))
                }
            }
        }
    }

    func net(_ st: J) -> some View {
        let n = st["net"] as? J ?? J()
        return HStack(spacing: 4) {
            Image(systemName: "arrow.down").font(.system(size: 10, weight: .bold)).foregroundColor(Color(hex: "#5ac8fa"))
            Text("\(formatBytes(n0(n, "down")))/s").font(.system(size: 11, weight: .medium, design: .monospaced)).foregroundColor(.white)
            Spacer()
            Image(systemName: "arrow.up").font(.system(size: 10, weight: .bold)).foregroundColor(Color(hex: "#ff9f6b"))
            Text("\(formatBytes(n0(n, "up")))/s").font(.system(size: 11, weight: .medium, design: .monospaced)).foregroundColor(.white)
        }
        .padding(.vertical, 4).padding(.horizontal, 10).glass(e.glass, 9, k: 0.85, flat: 0.08)
    }

    // Service buttons grid (tap = open the service; a stack opens the dashboard with that stack open)
    var grid: some View {
        let cells = large ? e.cells : Array(e.cells.prefix(8))
        let rows = stride(from: 0, to: cells.count, by: 4).map { Array(cells[$0..<min($0 + 4, cells.count)]) }
        return VStack(spacing: 5) {
            ForEach(0..<rows.count, id: \.self) { r in
                HStack(spacing: 5) {
                    ForEach(0..<4, id: \.self) { j in
                        if j < rows[r].count { pill(rows[r][j]) } else { Color.clear.frame(maxWidth: .infinity, maxHeight: 26) }
                    }
                }
            }
        }
    }

    @ViewBuilder func pill(_ c: Cell) -> some View {
        let inner = HStack(spacing: 2) {
            if let d = c.dot { Text("●").font(.system(size: 6)).foregroundColor(d) }
            if let l = c.logo { Image(uiImage: l).resizable().scaledToFit().frame(width: 12, height: 12) }
            else { Text(c.icon).font(.system(size: 10)) }
            Text(c.name).font(.system(size: 9, weight: .semibold)).foregroundColor(.white).lineLimit(1).minimumScaleFactor(0.6)
            Spacer(minLength: 0)
        }
        .padding(.horizontal, 4)
        .frame(maxWidth: .infinity, minHeight: 26, maxHeight: 26)
        .glass(e.glass, 8)
        if let u = c.url { Link(destination: u) { inner } } else { inner }
    }
}

struct BoardWidget: Widget {
    var body: some WidgetConfiguration {
        AppIntentConfiguration(kind: "BoardWidget", intent: BoardIntent.self, provider: Provider()) { BoardWidgetView(e: $0) }
            .configurationDisplayName("-board")
            .description("Live server gauges and service buttons.")
            .supportedFamilies([.systemSmall, .systemMedium, .systemLarge])
            .contentMarginsDisabled()
    }
}

@main
struct BoardWidgets: WidgetBundle {
    var body: some Widget { BoardWidget() }
}
