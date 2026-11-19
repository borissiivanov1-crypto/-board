//! Monitoring from Glances: CPU, RAM, drives, network, temperatures, and the details shown in the
//! enlarged cards. Every function takes an `api` callback (path -> JSON) so it can be tested offline.

use crate::settings::Settings;
use regex::Regex;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

pub type ApiResult = Result<Value, String>;

macro_rules! rx {
    ($pat:expr) => {{
        static CELL: OnceLock<Regex> = OnceLock::new();
        CELL.get_or_init(|| Regex::new($pat).unwrap())
    }};
}

/// Mounts that aren't real drives: system folders, container layers, Proxmox's config and LXC file systems.
pub fn skip_mnt(m: &str) -> bool {
    rx!(r"^/(etc|run|dev|sys|proc|boot|snap|var/lib/(docker|lxcfs|containers)|var/snap)(/|$)").is_match(m)
}
pub fn skip_fstype(t: &str) -> bool {
    rx!(r"^(overlay|tmpfs|squashfs|devtmpfs|ramfs|nsfs|autofs|efivarfs|fuse|fuse\.lxcfs|fuse\.gvfsd-fuse|fuse\.portal|fusectl)$").is_match(t)
}
/// Virtual network interfaces (Docker, Proxmox VM / container taps and firewall bridges, Kubernetes, LXD).
pub fn skip_iface(n: &str) -> bool {
    rx!(r"^(lo|docker|veth|br-|virbr|tap|fwbr|fwpr|fwln|vnet|cali|flannel|cni|kube|lxcbr|lxdbr)").is_match(n)
}

fn f(v: &Value, k: &str) -> Option<f64> {
    v.get(k).and_then(|x| x.as_f64())
}
fn s<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).and_then(|x| x.as_str()).unwrap_or("")
}
fn list(v: &Value) -> &[Value] {
    v.as_array().map(|a| a.as_slice()).unwrap_or(&[])
}

pub fn is_real_drive(d: &Value) -> bool {
    !skip_mnt(s(d, "mnt_point")) && !skip_fstype(s(d, "fs_type"))
}

/// The same storage seen at several mount points counts once. ZFS datasets share their pool's space,
/// so they count once per pool.
pub fn drive_key(device: &str, fs_type: &str) -> String {
    if fs_type == "zfs" {
        format!("zfs:{}", device.split('/').next().unwrap_or(""))
    } else {
        device.to_string()
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Disk {
    pub name: String,
    pub percent: f64,
    pub used: f64,
    pub size: f64,
    pub free: Option<f64>,
    pub mnt: String,
    pub device: String,
    pub fstype: String,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Storage {
    pub percent: f64,
    pub used: f64,
    pub size: f64,
    pub free: f64,
    pub count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Net {
    pub down: f64,
    pub up: f64,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Stats {
    pub cpu: f64,
    pub ram: f64,
    pub disks: Vec<Disk>,
    pub storage: Option<Storage>,
    pub net: Net,
}

pub fn pick_disks(fs: &Value, set: &Settings) -> Vec<Disk> {
    let all = list(fs);
    let chosen: Vec<&Value> = if !set.disk_mounts.is_empty() {
        set.disk_mounts.iter().filter_map(|m| all.iter().find(|d| s(d, "mnt_point") == m)).take(12).collect()
    } else {
        let mut v: Vec<&Value> = all.iter().filter(|d| is_real_drive(d)).collect();
        // biggest first, so a ZFS pool is shown by its largest dataset
        v.sort_by(|a, b| f(b, "size").unwrap_or(0.0).partial_cmp(&f(a, "size").unwrap_or(0.0)).unwrap_or(std::cmp::Ordering::Equal));
        let mut seen = HashSet::new();
        v.into_iter().filter(|d| seen.insert(drive_key(s(d, "device_name"), s(d, "fs_type")))).take(set.max_disks.max(1) as usize).collect()
    };
    chosen
        .into_iter()
        .map(|d| {
            let mnt = s(d, "mnt_point").to_string();
            Disk {
                name: if mnt == "/" { "Root".into() } else { mnt.split('/').filter(|x| !x.is_empty()).last().unwrap_or(&mnt).to_string() },
                percent: f(d, "percent").unwrap_or(0.0),
                used: f(d, "used").unwrap_or(0.0),
                size: f(d, "size").unwrap_or(0.0),
                free: f(d, "free"),
                mnt,
                device: s(d, "device_name").to_string(),
                fstype: s(d, "fs_type").to_string(),
            }
        })
        .collect()
}

/// Total across all drives (drives that are the same filesystem are only counted once).
pub fn sum_storage(disks: &[Disk]) -> Option<Storage> {
    if disks.is_empty() {
        return None;
    }
    let mut seen = HashSet::new();
    let (mut used, mut size, mut free) = (0.0, 0.0, 0.0);
    for d in disks {
        let k = if d.fstype == "zfs" { drive_key(&d.device, "zfs") } else { format!("{}|{}|{}", d.device, d.size, d.used) };
        if !seen.insert(k) {
            continue;
        }
        used += d.used;
        size += d.size;
        free += d.free.unwrap_or(d.size - d.used);
    }
    Some(Storage { percent: if size > 0.0 { used / size * 100.0 } else { 0.0 }, used, size, free, count: seen.len() })
}

pub fn pick_network(net: &Value, set: &Settings) -> Net {
    let total = |n: &Value| f(n, "bytes_recv").or(f(n, "rx")).unwrap_or(0.0) + f(n, "bytes_sent").or(f(n, "tx")).unwrap_or(0.0);
    let all = list(net);
    let iface = if !set.network_interface.is_empty() {
        all.iter().find(|n| s(n, "interface_name") == set.network_interface)
    } else {
        let mut v: Vec<&Value> = all.iter().filter(|n| !skip_iface(s(n, "interface_name"))).collect();
        v.sort_by(|a, b| total(b).partial_cmp(&total(a)).unwrap_or(std::cmp::Ordering::Equal));
        v.into_iter().next()
    };
    let Some(i) = iface else { return Net::default() };
    let dt = f(i, "time_since_update").filter(|x| *x != 0.0).unwrap_or(1.0);
    Net {
        down: f(i, "bytes_recv_rate_per_sec").unwrap_or(f(i, "rx").unwrap_or(0.0) / dt),
        up: f(i, "bytes_sent_rate_per_sec").unwrap_or(f(i, "tx").unwrap_or(0.0) / dt),
    }
}

/// Drive lists are reused for a while (drive usage changes slowly).
#[derive(Default)]
pub struct FsCache(Mutex<Option<(Instant, Value)>>);

impl FsCache {
    pub fn clear(&self) {
        *self.0.lock().unwrap() = None;
    }
}

/// CPU, RAM, drives and network. disks / net = false skip what a view doesn't show.
pub fn get_stats(api: &(dyn Fn(&str) -> ApiResult + Sync), set: &Settings, disks: bool, net: bool, cache: Option<(&FsCache, Duration)>) -> Result<Stats, String> {
    let fresh = cache.and_then(|(c, age)| c.0.lock().unwrap().as_ref().filter(|(t, _)| t.elapsed() < age).map(|(_, v)| v.clone()));
    // quicklook returns CPU and RAM together
    let (quick, fs, netv) = std::thread::scope(|sc| {
        let q = sc.spawn(|| api("quicklook").ok());
        let fsh = if disks && fresh.is_none() { Some(sc.spawn(|| api("fs"))) } else { None };
        let nh = if net { Some(sc.spawn(|| api("network"))) } else { None };
        (q.join().unwrap(), fsh.map(|h| h.join().unwrap()), nh.map(|h| h.join().unwrap()))
    });
    let fs = match (disks, fresh) {
        (false, _) => Value::Null,
        (true, Some(v)) => v,
        (true, None) => {
            let v = fs.unwrap()?;
            if let Some((c, _)) = cache {
                *c.0.lock().unwrap() = Some((Instant::now(), v.clone()));
            }
            v
        }
    };
    let netv = match netv {
        Some(r) => Some(r?),
        None => None,
    };
    let (cpu, ram) = match quick.as_ref().and_then(|q| Some((f(q, "cpu")?, f(q, "mem")?))) {
        Some(x) => x,
        None => {
            // fallback if quicklook is unavailable on your Glances
            let c = api("cpu")?;
            let m = api("mem")?;
            (f(&c, "total").unwrap_or(0.0), f(&m, "percent").unwrap_or(0.0))
        }
    };
    let dl = if disks { pick_disks(&fs, set) } else { vec![] };
    Ok(Stats {
        cpu,
        ram,
        storage: sum_storage(&dl),
        disks: dl,
        net: netv.map(|n| pick_network(&n, set)).unwrap_or_default(),
    })
}

// ---------- Temperatures (from the Glances sensors plugin) ----------

#[derive(Debug, Clone, PartialEq)]
pub struct Sensor {
    pub label: String,
    pub value: f64,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Temps {
    pub pkg: Option<f64>,
    pub cores: Vec<f64>,
    pub nvme: Vec<Sensor>,
    pub hdd: Vec<Sensor>,
    pub other: Vec<f64>,
}

pub fn parse_temps(sens: &Value) -> Temps {
    let all: Vec<&Value> = list(sens)
        .iter()
        .filter(|x| x.get("value").map(|v| v.is_number()).unwrap_or(false) && (s(x, "unit") == "C" || rx!(r"(?i)temp").is_match(s(x, "type"))))
        .collect();
    let lb = |x: &Value| s(x, "label").to_string();
    let is_hdd = |x: &Value| s(x, "type") == "temperature_hdd" || rx!(r"(?i)^(drivetemp|hddtemp)|^/dev/sd|^sd[a-z]").is_match(&lb(x));
    let is_nvme = |x: &Value| rx!(r"(?i)composite").is_match(&lb(x)) || rx!(r"(?i)^nvme").is_match(&lb(x));
    let sensor = |x: &&Value| Sensor { label: lb(x), value: f(x, "value").unwrap() };
    let rest: Vec<&Value> = all.iter().copied().filter(|x| !is_hdd(x) && !is_nvme(x)).collect();
    let pkg = rest.iter().find(|x| rx!(r"(?i)package|tdie|tctl|^cpu").is_match(&lb(x)));
    Temps {
        pkg: pkg.and_then(|x| f(x, "value")),
        cores: rest.iter().filter(|x| rx!(r"(?i)^core").is_match(&lb(x))).filter_map(|x| f(x, "value")).collect(),
        nvme: all.iter().filter(|x| !is_hdd(x) && is_nvme(x)).map(sensor).collect(),
        hdd: all.iter().filter(|x| is_hdd(x)).map(sensor).collect(),
        other: rest.iter().filter(|x| !rx!(r"(?i)^sensor").is_match(&lb(x))).filter_map(|x| f(x, "value")).collect(),
    }
}

fn max_of(v: &[f64]) -> Option<f64> {
    v.iter().copied().fold(None, |m, x| Some(m.map_or(x, |m: f64| m.max(x))))
}

/// (package temperature, hottest core, core temperatures)
pub fn cpu_temp(t: &Temps) -> (Option<f64>, Option<f64>, usize) {
    let hottest = max_of(&t.cores);
    let pkg = t.pkg.or(hottest).or(max_of(&t.other));
    (pkg, hottest, t.cores.len())
}

#[derive(Debug, Clone, PartialEq)]
pub struct DriveTemp {
    /// "nvme" or "hdd"
    pub kind: &'static str,
    pub values: Vec<f64>,
}

/// Match a mounted drive to its sensor(s) by device name (nvme0n1 = first NVMe sensor, md0 = all HDD sensors).
pub fn drive_temp(disk: &Disk, t: &Temps) -> Option<DriveTemp> {
    let dev = disk.device.rsplit('/').next().unwrap_or("");
    if let Some(m) = rx!(r"^nvme(\d+)n").captures(dev) {
        let i: usize = m[1].parse().ok()?;
        return t.nvme.get(i).map(|x| DriveTemp { kind: "nvme", values: vec![x.value] });
    }
    if let Some(m) = rx!(r"^sd([a-z])").captures(dev) {
        let i = (m[1].as_bytes()[0] - b'a') as usize;
        return t.hdd.get(i).map(|x| DriveTemp { kind: "hdd", values: vec![x.value] });
    }
    if dev.starts_with("md") && !t.hdd.is_empty() {
        return Some(DriveTemp { kind: "hdd", values: t.hdd.iter().map(|x| x.value).collect() });
    }
    None
}

// ---------- Details for the enlarged CPU / RAM / storage cards ----------

#[derive(Debug, Clone, PartialEq)]
pub struct TopRow {
    pub name: String,
    pub pct: f64,
    pub bytes: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CpuDetail {
    pub user: Option<f64>,
    pub system: Option<f64>,
    pub iowait: Option<f64>,
    pub load: Option<[f64; 3]>,
    pub cores: Vec<f64>,
    pub n_cores: f64,
    pub temp: Option<f64>,
    pub hot: Option<f64>,
    pub n_core_temps: usize,
    pub top: Vec<TopRow>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RamDetail {
    pub total: f64,
    pub used: f64,
    pub available: f64,
    pub cached: Option<f64>,
    pub swap_used: Option<f64>,
    pub swap_total: Option<f64>,
    pub top: Vec<TopRow>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Io {
    pub read: f64,
    pub write: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DiskDetail {
    pub io: Vec<Option<Io>>,
    pub temps: Vec<Option<DriveTemp>>,
    pub nvme: Vec<Sensor>,
    pub hdd: Vec<Sensor>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Detail {
    Cpu(CpuDetail),
    Ram(RamDetail),
    Disk(DiskDetail),
}

fn par<A: Send, B: Send, F1: FnOnce() -> A + Send, F2: FnOnce() -> B + Send>(a: F1, b: F2) -> (A, B) {
    std::thread::scope(|sc| {
        let h = sc.spawn(a);
        let y = b();
        (h.join().unwrap(), y)
    })
}

/// Group processes by name and keep the top five.
fn top_by<F: Fn(&Value) -> f64>(procs: &Value, val: F) -> Vec<(String, f64)> {
    let mut agg: Vec<(String, f64)> = vec![];
    let mut idx: HashMap<String, usize> = HashMap::new();
    for p in list(procs) {
        let n = match s(p, "name") {
            "" => "?".to_string(),
            x => x.to_string(),
        };
        let v = val(p);
        match idx.get(&n) {
            Some(i) => agg[*i].1 += v,
            None => {
                idx.insert(n.clone(), agg.len());
                agg.push((n, v));
            }
        }
    }
    agg.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    agg.truncate(5);
    agg
}

pub fn get_detail(api: &(dyn Fn(&str) -> ApiResult + Sync), key: &str, stats: &Stats) -> Result<Detail, String> {
    match key {
        "cpu" => {
            let ((cpu, per), ((load, procs), sens)) = par(
                || par(|| api("cpu").unwrap_or(Value::Null), || api("percpu").unwrap_or(Value::Null)),
                || par(|| par(|| api("load").ok(), || api("processlist").unwrap_or(Value::Null)), || api("sensors").unwrap_or(Value::Null)),
            );
            let per_n = list(&per).len();
            let cores = load.as_ref().and_then(|l| f(l, "cpucore")).filter(|c| *c != 0.0).unwrap_or(if per_n > 0 { per_n as f64 } else { 1.0 });
            let top = top_by(&procs, |p| f(p, "cpu_percent").unwrap_or(0.0))
                .into_iter()
                .map(|(name, v)| TopRow { name, pct: v / cores, bytes: 0.0 })
                .collect();
            let (temp, hot, n) = cpu_temp(&parse_temps(&sens));
            Ok(Detail::Cpu(CpuDetail {
                user: f(&cpu, "user"),
                system: f(&cpu, "system"),
                iowait: f(&cpu, "iowait"),
                load: load.as_ref().and_then(|l| Some([f(l, "min1")?, f(l, "min5")?, f(l, "min15")?])),
                cores: list(&per).iter().map(|c| f(c, "total").unwrap_or(0.0)).collect(),
                n_cores: cores,
                temp,
                hot,
                n_core_temps: n,
                top,
            }))
        }
        "ram" => {
            let (mem, (swap, procs)) = par(|| api("mem"), || par(|| api("memswap").ok(), || api("processlist").unwrap_or(Value::Null)));
            let mem = mem?;
            let total = f(&mem, "total").unwrap_or(0.0);
            let rss = |p: &Value| {
                let mut r = match p.get("memory_info") {
                    Some(Value::Array(a)) => a.first().and_then(|x| x.as_f64()).unwrap_or(0.0),
                    Some(o @ Value::Object(_)) => f(o, "rss").unwrap_or(0.0),
                    _ => 0.0,
                };
                if r == 0.0 {
                    if let Some(mp) = f(p, "memory_percent").filter(|x| *x != 0.0) {
                        r = mp / 100.0 * total;
                    }
                }
                r
            };
            let top = top_by(&procs, rss)
                .into_iter()
                .map(|(name, b)| TopRow { name, pct: if total > 0.0 { b / total * 100.0 } else { 0.0 }, bytes: b })
                .collect();
            Ok(Detail::Ram(RamDetail {
                total,
                used: f(&mem, "used").unwrap_or(0.0),
                available: f(&mem, "available").unwrap_or(0.0),
                cached: f(&mem, "cached"),
                swap_used: swap.as_ref().and_then(|w| f(w, "used")),
                swap_total: swap.as_ref().and_then(|w| f(w, "total")),
                top,
            }))
        }
        _ => {
            let (rows, sens) = par(|| api("diskio").unwrap_or(Value::Null), || api("sensors").ok());
            let io = stats
                .disks
                .iter()
                .map(|d| {
                    let base = d.device.rsplit('/').next().unwrap_or("");
                    let row = list(&rows).iter().find(|x| s(x, "disk_name") == base)?;
                    let dt = f(row, "time_since_update").filter(|x| *x != 0.0).unwrap_or(1.0);
                    Some(Io {
                        read: f(row, "read_bytes_rate_per_sec").unwrap_or(f(row, "read_bytes").unwrap_or(0.0) / dt),
                        write: f(row, "write_bytes_rate_per_sec").unwrap_or(f(row, "write_bytes").unwrap_or(0.0) / dt),
                    })
                })
                .collect();
            let t = sens.as_ref().map(parse_temps);
            Ok(Detail::Disk(DiskDetail {
                io,
                temps: stats.disks.iter().map(|d| t.as_ref().and_then(|t| drive_temp(d, t))).collect(),
                nvme: t.as_ref().map(|t| t.nvme.clone()).unwrap_or_default(),
                hdd: t.as_ref().map(|t| t.hdd.clone()).unwrap_or_default(),
            }))
        }
    }
}

/// Drives and interfaces for the Settings lists: (mount, size, used, device) and interface names.
pub fn mounts_for_settings(fs: &Value) -> Vec<(String, f64, f64, String)> {
    list(fs)
        .iter()
        .filter(|x| !s(x, "mnt_point").is_empty() && is_real_drive(x))
        .map(|x| (s(x, "mnt_point").to_string(), f(x, "size").unwrap_or(0.0), f(x, "used").unwrap_or(0.0), s(x, "device_name").rsplit('/').next().unwrap_or("").to_string()))
        .collect()
}

pub fn ifaces_for_settings(net: &Value) -> Vec<String> {
    list(net).iter().map(|n| s(n, "interface_name").to_string()).filter(|n| !n.is_empty() && !skip_iface(n)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::clean_settings;
    use serde_json::json;

    fn fs() -> Value {
        json!([
            { "mnt_point": "/", "device_name": "/dev/nvme0n1p2", "fs_type": "ext4", "size": 500.0, "used": 100.0, "free": 400.0, "percent": 20.0 },
            { "mnt_point": "/etc/hosts", "device_name": "/dev/nvme0n1p2", "fs_type": "ext4", "size": 500.0, "used": 100.0, "percent": 20.0 },
            { "mnt_point": "/rpool/data", "device_name": "rpool/data", "fs_type": "zfs", "size": 2000.0, "used": 500.0, "percent": 25.0 },
            { "mnt_point": "/rpool/ROOT", "device_name": "rpool/ROOT/pve-1", "fs_type": "zfs", "size": 1900.0, "used": 400.0, "percent": 21.0 },
            { "mnt_point": "/mnt/media", "device_name": "/dev/sda1", "fs_type": "xfs", "size": 4000.0, "used": 3600.0, "percent": 90.0 },
            { "mnt_point": "/var/lib/docker/overlay2/x", "device_name": "overlay", "fs_type": "overlay", "size": 9.0, "used": 1.0, "percent": 1.0 },
        ])
    }

    #[test]
    fn drives_are_picked_and_deduplicated() {
        let set = clean_settings(&json!({}));
        let d = pick_disks(&fs(), &set);
        assert_eq!(d.iter().map(|x| x.name.as_str()).collect::<Vec<_>>(), vec!["media", "data", "Root"]);
        let st = sum_storage(&d).unwrap();
        assert_eq!(st.count, 3);
        assert_eq!(st.size, 6500.0);
        assert_eq!(st.used, 4200.0);
        assert_eq!(st.free, 400.0 + 1500.0 + 400.0);
        let set = clean_settings(&json!({ "maxDisks": 1 }));
        assert_eq!(pick_disks(&fs(), &set).len(), 1);
        let set = clean_settings(&json!({ "diskMounts": ["/", "/missing", "/etc/hosts"] }));
        let d = pick_disks(&fs(), &set);
        assert_eq!(d.len(), 2);
        assert_eq!(sum_storage(&d).unwrap().count, 1, "the same drive at two mounts counts once");
        assert!(sum_storage(&[]).is_none());
    }

    #[test]
    fn network_picks_the_busiest_real_interface() {
        let net = json!([
            { "interface_name": "lo", "bytes_recv": 9e12, "bytes_sent": 9e12, "bytes_recv_rate_per_sec": 1.0, "bytes_sent_rate_per_sec": 1.0 },
            { "interface_name": "eth0", "bytes_recv": 100.0, "bytes_sent": 50.0, "bytes_recv_rate_per_sec": 2048.0, "bytes_sent_rate_per_sec": 512.0 },
            { "interface_name": "veth1", "bytes_recv": 1e9, "bytes_sent": 1e9 },
            { "interface_name": "wlan0", "rx": 300.0, "tx": 30.0, "time_since_update": 3.0 }
        ]);
        let set = clean_settings(&json!({}));
        assert_eq!(pick_network(&net, &set), Net { down: 100.0, up: 10.0 });
        let set = clean_settings(&json!({ "networkInterface": "eth0" }));
        assert_eq!(pick_network(&net, &set), Net { down: 2048.0, up: 512.0 });
        let set = clean_settings(&json!({ "networkInterface": "nope" }));
        assert_eq!(pick_network(&net, &set), Net::default());
        assert_eq!(ifaces_for_settings(&net), vec!["eth0", "wlan0"]);
    }

    fn sensors() -> Value {
        json!([
            { "label": "Package id 0", "value": 55.0, "unit": "C", "type": "temperature_core" },
            { "label": "Core 0", "value": 50.0, "unit": "C" },
            { "label": "Core 1", "value": 61.0, "unit": "C" },
            { "label": "Composite", "value": 40.0, "unit": "C" },
            { "label": "nvme1", "value": 42.0, "unit": "C" },
            { "label": "sda", "value": 35.0, "unit": "C", "type": "temperature_hdd" },
            { "label": "/dev/sdb", "value": 37.0, "unit": "C" },
            { "label": "fan1", "value": 1200.0, "unit": "R", "type": "fan_speed" },
            { "label": "sensor 1", "value": 20.0, "unit": "C" },
            { "label": "acpitz 1", "value": 27.0, "unit": "C" }
        ])
    }

    #[test]
    fn temperatures() {
        let t = parse_temps(&sensors());
        assert_eq!(t.pkg, Some(55.0));
        assert_eq!(t.cores, vec![50.0, 61.0]);
        assert_eq!(t.nvme.len(), 2);
        assert_eq!(t.hdd.len(), 2);
        assert_eq!(cpu_temp(&t), (Some(55.0), Some(61.0), 2));
        let disk = |d: &str| Disk { device: d.into(), ..Default::default() };
        assert_eq!(drive_temp(&disk("/dev/nvme1n1"), &t).unwrap().values, vec![42.0]);
        assert_eq!(drive_temp(&disk("/dev/sdb1"), &t).unwrap().kind, "hdd");
        assert_eq!(drive_temp(&disk("/dev/md0"), &t).unwrap().values, vec![35.0, 37.0]);
        assert!(drive_temp(&disk("/dev/sdz"), &t).is_none());
        let only_other = parse_temps(&json!([{ "label": "acpitz", "value": 30.0, "unit": "C" }, { "label": "sensor 9", "value": 99.0, "unit": "C" }]));
        assert_eq!(cpu_temp(&only_other).0, Some(30.0));
    }

    fn fake(path: &str) -> ApiResult {
        match path {
            "quicklook" => Ok(json!({ "cpu": 12.5, "mem": 40.0 })),
            "fs" => Ok(fs()),
            "network" => Ok(json!([{ "interface_name": "eth0", "bytes_recv_rate_per_sec": 10.0, "bytes_sent_rate_per_sec": 20.0 }])),
            "cpu" => Ok(json!({ "total": 12.5, "user": 8.0, "system": 3.0, "iowait": 0.5 })),
            "percpu" => Ok(json!([{ "total": 10.0 }, { "total": 15.0 }])),
            "load" => Ok(json!({ "min1": 0.5, "min5": 0.4, "min15": 0.3, "cpucore": 2 })),
            "processlist" => Ok(json!([
                { "name": "python", "cpu_percent": 30.0, "memory_info": { "rss": 100.0 } },
                { "name": "python", "cpu_percent": 10.0, "memory_info": [50.0, 0] },
                { "name": "nginx", "cpu_percent": 4.0, "memory_percent": 10.0 },
                { "cpu_percent": 1.0 }
            ])),
            "sensors" => Ok(sensors()),
            "mem" => Ok(json!({ "total": 1000.0, "percent": 40.0, "used": 400.0, "available": 600.0, "cached": 100.0 })),
            "memswap" => Ok(json!({ "used": 1.0, "total": 2.0 })),
            "diskio" => Ok(json!([{ "disk_name": "sda1", "read_bytes_rate_per_sec": 5.0, "write_bytes_rate_per_sec": 6.0 }])),
            _ => Err("404".into()),
        }
    }

    #[test]
    fn stats_and_details() {
        let set = clean_settings(&json!({}));
        let cache = FsCache::default();
        let st = get_stats(&fake, &set, true, true, Some((&cache, Duration::from_secs(10)))).unwrap();
        assert_eq!((st.cpu, st.ram), (12.5, 40.0));
        assert_eq!(st.net, Net { down: 10.0, up: 20.0 });
        assert_eq!(st.disks.len(), 3);
        // the drive list is reused while it's fresh
        let no_fs = |p: &str| if p == "fs" { Err("down".to_string()) } else { fake(p) };
        assert!(get_stats(&no_fs, &set, true, true, Some((&cache, Duration::from_secs(10)))).is_ok());
        cache.clear();
        assert!(get_stats(&no_fs, &set, true, true, Some((&cache, Duration::from_secs(10)))).is_err());
        // quicklook missing: cpu + mem instead
        let no_quick = |p: &str| if p == "quicklook" { Err("x".to_string()) } else { fake(p) };
        let st2 = get_stats(&no_quick, &set, false, false, None).unwrap();
        assert_eq!((st2.cpu, st2.ram), (12.5, 40.0));

        let Detail::Cpu(c) = get_detail(&fake, "cpu", &st).unwrap() else { panic!() };
        assert_eq!(c.n_cores, 2.0);
        assert_eq!(c.top[0], TopRow { name: "python".into(), pct: 20.0, bytes: 0.0 });
        assert_eq!(c.top.last().unwrap().name, "?");
        assert_eq!(c.load, Some([0.5, 0.4, 0.3]));
        assert_eq!(c.temp, Some(55.0));
        let Detail::Ram(r) = get_detail(&fake, "ram", &st).unwrap() else { panic!() };
        assert_eq!(r.top[0].name, "python");
        assert_eq!(r.top[0].bytes, 150.0);
        assert_eq!(r.top[1].bytes, 100.0, "memory_percent of the total when there's no rss");
        assert_eq!(r.swap_total, Some(2.0));
        let Detail::Disk(d) = get_detail(&fake, "disk", &st).unwrap() else { panic!() };
        assert_eq!(d.io[0], Some(Io { read: 5.0, write: 6.0 }));
        assert_eq!(d.io[1], None);
        assert_eq!(d.temps[0].as_ref().unwrap().kind, "hdd");
    }

    #[test]
    fn settings_lists() {
        let m = mounts_for_settings(&fs());
        assert_eq!(m.len(), 4);
        assert_eq!(m[0].3, "nvme0n1p2");
    }
}
