import Foundation

// Mounts that aren't real drives: system folders, container layers, Proxmox's config and LXC file systems
let SKIP_MNT = "^/(etc|run|dev|sys|proc|boot|snap|var/lib/(docker|lxcfs|containers)|var/snap)(/|$)"
let SKIP_FSTYPE = "^(overlay|tmpfs|squashfs|devtmpfs|ramfs|nsfs|autofs|efivarfs|fuse|fuse\\.lxcfs|fuse\\.gvfsd-fuse|fuse\\.portal|fusectl)$"
// Virtual network interfaces (Docker, Proxmox VM / container taps and firewall bridges, Kubernetes, LXD)
let SKIP_IFACE = "^(lo|docker|veth|br-|virbr|tap|fwbr|fwpr|fwln|vnet|cali|flannel|cni|kube|lxcbr|lxdbr)"

func isRealDrive(_ d: J) -> Bool { !test(str(d, "mnt_point"), SKIP_MNT) && !test(str(d, "fs_type"), SKIP_FSTYPE) }

// The same storage seen at several mount points counts once. ZFS datasets all share their pool's space.
func driveKey(_ device: String, _ fsType: String) -> String {
    fsType == "zfs" ? "zfs:" + (device.components(separatedBy: "/").first ?? "") : device
}

func n0(_ d: J, _ k: String) -> Double { num(d[k]) ?? 0 }

final class Glances {
    let s: J
    let serverId: String?
    private var fsCache: [J]?
    private var fsTime = 0.0

    init(_ s: J, serverId: String? = nil) { self.s = s; self.serverId = serverId }

    func request(_ path: String, pass: String? = nil, set: J? = nil) async throws -> Any {
        let st = set ?? s
        var headers = [String: String]()
        if !str(st, "username").isEmpty {
            let token = Data("\(str(st, "username")):\(pass ?? Logins.glancesPass(serverId))".utf8).base64EncodedString()
            headers["Authorization"] = "Basic \(token)"
        }
        let r = try await Net.request("\(glancesUrl(st))/api/\(int(st, "apiVersion"))/\(path)", headers: headers, timeout: 5)
        guard let j = r.json else { throw URLError(.cannotParseResponse) }
        if r.code >= 400 { throw URLError(.badServerResponse) }
        return j
    }

    func obj(_ p: String) async throws -> J { (try await request(p)) as? J ?? J() }
    func arr(_ p: String) async throws -> [J] { objs(try await request(p)) }

    // disks / net = false skip what a view doesn't show. fsMaxAge (ms) reuses the last drive list for that long.
    func getStats(disks: Bool = true, net: Bool = true, fsMaxAge: Double = 0) async throws -> J {
        let fsFresh = disks && fsMaxAge > 0 && fsCache != nil && nowMs() - fsTime < fsMaxAge
        async let quickT: J? = try? obj("quicklook")
        async let fsT: [J]? = disks && !fsFresh ? arr("fs") : nil
        async let netT: [J]? = net ? arr("network") : nil
        let quick = await quickT
        let fsGot = try await fsT
        let fs = disks ? (fsFresh ? fsCache! : (fsGot ?? [])) : []
        let netA = (try await netT) ?? []
        if disks && !fsFresh { fsCache = fs; fsTime = nowMs() }
        var cpu: Double, ram: Double
        if let q = quick, let c = num(q["cpu"]), let m = num(q["mem"]) {
            cpu = c; ram = m
        } else {
            // fallback if quicklook is unavailable on your Glances
            async let c = obj("cpu")
            async let m = obj("mem")
            cpu = n0(try await c, "total"); ram = n0(try await m, "percent")
        }
        let dl = disks ? pickDisks(fs) : []
        return ["cpu": cpu, "ram": ram, "disks": dl, "storage": sumStorage(dl) as Any? ?? NSNull(),
                "net": net ? pickNetwork(netA) : ["down": 0, "up": 0]]
    }

    func pickDisks(_ fs: [J]) -> [J] {
        let mounts = strs(s["diskMounts"])
        var list: [J]
        if !mounts.isEmpty {
            list = mounts.compactMap { m in fs.first { str($0, "mnt_point") == m } }
        } else {
            var seen = Set<String>()
            // biggest first, so a ZFS pool is shown by its largest dataset
            list = fs.filter(isRealDrive).sorted { n0($0, "size") > n0($1, "size") }
                .filter { seen.insert(driveKey(str($0, "device_name"), str($0, "fs_type"))).inserted }
        }
        return list.prefix(mounts.isEmpty ? int(s, "maxDisks") : 12).map { d -> J in
            let mnt = str(d, "mnt_point")
            return ["name": mnt == "/" ? "Root" : (mnt.split(separator: "/").last.map(String.init) ?? mnt),
                    "percent": n0(d, "percent"), "used": n0(d, "used"), "size": n0(d, "size"),
                    "free": num(d["free"]) as Any? ?? NSNull(), "mnt": mnt, "device": str(d, "device_name"), "fstype": str(d, "fs_type")]
        }
    }

    func pickNetwork(_ net: [J]) -> J {
        func total(_ n: J) -> Double { (num(n["bytes_recv"]) ?? num(n["rx"]) ?? 0) + (num(n["bytes_sent"]) ?? num(n["tx"]) ?? 0) }
        let want = str(s, "networkInterface")
        var iface: J?
        if !want.isEmpty { iface = net.first(where: { str($0, "interface_name") == want }) }
        else { iface = net.filter({ !test(str($0, "interface_name"), SKIP_IFACE) }).max(by: { total($0) < total($1) }) }
        guard let i = iface else { return ["down": 0, "up": 0] }
        let dt = (num(i["time_since_update"]) ?? 1) == 0 ? 1 : (num(i["time_since_update"]) ?? 1)
        return ["down": num(i["bytes_recv_rate_per_sec"]) ?? n0(i, "rx") / dt, "up": num(i["bytes_sent_rate_per_sec"]) ?? n0(i, "tx") / dt]
    }

    // ---------- Details for the expanded CPU / RAM / storage cards ----------
    func getDetail(_ key: String, _ stats: J) async -> J? {
        if key == "cpu" {
            async let cpuT = try? obj("cpu")
            async let perT = try? arr("percpu")
            async let loadT = try? obj("load")
            async let procT = try? arr("processlist")
            async let sensT = try? arr("sensors")
            let cpu = await cpuT ?? J(), per = await perT ?? [], load = await loadT, procs = await procT ?? []
            let sens = await sensT ?? []
            let cores = Int(num(load?["cpucore"]) ?? 0) > 0 ? Int(num(load!["cpucore"])!) : max(per.count, 1)
            var agg = [String: Double](), order = [String]()
            for p in procs {
                let n = str(p, "name").isEmpty ? "?" : str(p, "name")
                if agg[n] == nil { order.append(n) }
                agg[n, default: 0] += n0(p, "cpu_percent")
            }
            let top = order.map { ["name": $0, "pct": agg[$0]! / Double(cores)] as J }.sorted { n0($0, "pct") > n0($1, "pct") }.prefix(5)
            let t = parseTemps(sens)
            let ct = cpuTemp(t)
            return ["kind": "cpu", "user": cpu["user"] ?? NSNull(), "system": cpu["system"] ?? NSNull(), "iowait": cpu["iowait"] ?? NSNull(),
                    "load": load.map { [n0($0, "min1"), n0($0, "min5"), n0($0, "min15")] as Any } ?? NSNull(),
                    "cores": per.map { n0($0, "total") }, "nCores": cores,
                    "temp": ct.0 as Any? ?? NSNull(), "hot": ct.1 as Any? ?? NSNull(), "nCoreTemps": (t["cores"] as! [Double]).count, "top": Array(top)]
        }
        if key == "ram" {
            guard let mem = try? await obj("mem") else { return nil }
            async let swapT = try? obj("memswap")
            async let procT = try? arr("processlist")
            let swap = await swapT, procs = await procT ?? []
            let total = n0(mem, "total") == 0 ? 1 : n0(mem, "total")
            func rss(_ p: J) -> Double {
                var r = 0.0
                if let a = p["memory_info"] as? [Any] { r = num(a.first) ?? 0 } else if let o = p["memory_info"] as? J { r = n0(o, "rss") }
                if r == 0, let mp = num(p["memory_percent"]) { r = mp / 100 * total }
                return r
            }
            var agg = [String: Double](), order = [String]()
            for p in procs {
                let n = str(p, "name").isEmpty ? "?" : str(p, "name")
                if agg[n] == nil { order.append(n) }
                agg[n, default: 0] += rss(p)
            }
            let top = order.map { ["name": $0, "bytes": agg[$0]!, "pct": agg[$0]! / total * 100] as J }.sorted { n0($0, "bytes") > n0($1, "bytes") }.prefix(5)
            return ["kind": "ram", "total": total, "used": n0(mem, "used"), "available": n0(mem, "available"), "cached": mem["cached"] ?? NSNull(),
                    "swapUsed": swap?["used"] ?? NSNull(), "swapTotal": swap?["total"] ?? NSNull(), "top": Array(top)]
        }
        if key == "disk" {
            let rows = (try? await arr("diskio")) ?? []
            let disks = objs(stats["disks"])
            let io: [Any] = disks.map { disk -> Any in
                let base = str(disk, "device").components(separatedBy: "/").last ?? ""
                guard let row = rows.first(where: { str($0, "disk_name") == base }) else { return NSNull() }
                let dt = n0(row, "time_since_update") == 0 ? 1 : n0(row, "time_since_update")
                return ["read": num(row["read_bytes_rate_per_sec"]) ?? n0(row, "read_bytes") / dt, "write": num(row["write_bytes_rate_per_sec"]) ?? n0(row, "write_bytes") / dt] as J
            }
            let t: J? = (try? await arr("sensors")).map(parseTemps)
            let temps: [Any] = disks.map { d -> Any in
                if let t = t, let x = driveTemp(d, t) { return x }
                return NSNull()
            }
            return ["kind": "disk", "io": io, "temps": temps, "nvme": t?["nvme"] ?? [Any](), "hdd": t?["hdd"] ?? [Any]()]
        }
        return nil
    }
}

// Total across all drives (drives that are the same filesystem are only counted once)
func sumStorage(_ disks: [J]) -> J? {
    if disks.isEmpty { return nil }
    var seen = Set<String>()
    var used = 0.0, size = 0.0, free = 0.0
    for d in disks {
        let k = str(d, "fstype") == "zfs" ? driveKey(str(d, "device"), "zfs") : "\(str(d, "device"))|\(n0(d, "size"))|\(n0(d, "used"))"
        if !seen.insert(k).inserted { continue }
        used += n0(d, "used"); size += n0(d, "size"); free += num(d["free"]) ?? (n0(d, "size") - n0(d, "used"))
    }
    return ["percent": size > 0 ? used / size * 100 : 0, "used": used, "size": size, "free": free, "count": seen.count]
}

// ---------- Temperatures (from the Glances sensors plugin) ----------
func parseTemps(_ sens: [J]) -> J {
    let all = sens.filter { isNum($0["value"]) && (str($0, "unit") == "C" || test(str($0, "type"), "temp", true)) }
    func lb(_ x: J) -> String { str(x, "label") }
    func isHdd(_ x: J) -> Bool { str(x, "type") == "temperature_hdd" || test(lb(x), "^(drivetemp|hddtemp)|^/dev/sd|^sd[a-z]", true) }
    func isNvme(_ x: J) -> Bool { test(lb(x), "composite", true) || test(lb(x), "^nvme", true) }
    let hdd = all.filter(isHdd)
    let nvme = all.filter { !isHdd($0) && isNvme($0) }
    let rest = all.filter { !isHdd($0) && !isNvme($0) }
    let pkg = rest.first { test(lb($0), "package|tdie|tctl|^cpu", true) }
    return [
        "pkg": pkg.map { n0($0, "value") as Any } ?? NSNull(),
        "cores": rest.filter { test(lb($0), "^core", true) }.map { n0($0, "value") },
        "nvme": nvme.map { ["label": lb($0), "value": n0($0, "value")] as J },
        "hdd": hdd.map { ["label": lb($0), "value": n0($0, "value")] as J },
        "other": rest.filter { !test(lb($0), "^sensor", true) }.map { n0($0, "value") },
    ]
}

// (package temperature, hottest core)
func cpuTemp(_ t: J) -> (Double?, Double?) {
    let cores = t["cores"] as? [Double] ?? []
    let hottest = cores.max()
    return (num(t["pkg"]) ?? hottest ?? (t["other"] as? [Double] ?? []).max(), hottest)
}

// Match a mounted drive to its temperature sensor(s) by device name (nvme0n1 = first NVMe sensor, md0 = all HDD sensors)
func driveTemp(_ disk: J, _ t: J) -> J? {
    let dev = str(disk, "device").components(separatedBy: "/").last ?? ""
    let nvme = objs(t["nvme"]), hdd = objs(t["hdd"])
    if let m = match(dev, "^nvme(\\d+)n") {
        let i = Int(m[1]) ?? 99
        return i < nvme.count ? ["kind": "nvme", "values": [n0(nvme[i], "value")]] : nil
    }
    if let m = match(dev, "^sd([a-z])") {
        let i = Int(m[1].unicodeScalars.first!.value) - 97
        return i < hdd.count ? ["kind": "hdd", "values": [n0(hdd[i], "value")]] : nil
    }
    if dev.hasPrefix("md") && !hdd.isEmpty { return ["kind": "hdd", "values": hdd.map { n0($0, "value") }] }
    return nil
}
