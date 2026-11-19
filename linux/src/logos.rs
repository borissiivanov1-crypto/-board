//! Service logos: downloaded once, shrunk to a small square PNG and kept in the cache folder.
//! A logo that fails to download is retried after LOGO_RETRY_HOURS.

use crate::config::{LOGO_RETRY_HOURS, LOGO_SIZE};
use image::{imageops::FilterType, DynamicImage, GenericImageView, ImageFormat, RgbaImage};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// The file name of a logo URL (same hash as the iOS version).
pub fn logo_key(url: &str) -> String {
    let mut h: u32 = 5381;
    let mut n = 0u32;
    for c in url.encode_utf16() {
        h = ((h as u64 * 33) as u32) ^ c as u32;
        n += 1;
    }
    format!("{}-{}", base36(h as u64), base36(n as u64))
}

fn base36(mut n: u64) -> String {
    const D: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut v = vec![];
    loop {
        v.push(D[(n % 36) as usize]);
        n /= 36;
        if n == 0 {
            break;
        }
    }
    v.reverse();
    String::from_utf8(v).unwrap()
}

pub struct Logos {
    dir: PathBuf,
}

/// Fit the logo inside a LOGO_SIZE square, keeping its shape and transparency.
pub fn shrink(img: &DynamicImage) -> RgbaImage {
    let (w0, h0) = img.dimensions();
    let scale = (LOGO_SIZE as f64 / w0.max(h0).max(1) as f64).min(1.0);
    let w = ((w0 as f64 * scale).round() as u32).max(1);
    let h = ((h0 as f64 * scale).round() as u32).max(1);
    let side = w.max(h);
    let small = img.resize_exact(w, h, FilterType::Lanczos3).to_rgba8();
    let mut out = RgbaImage::new(side, side);
    image::imageops::overlay(&mut out, &small, ((side - w) / 2) as i64, ((side - h) / 2) as i64);
    out
}

impl Logos {
    pub fn new(dir: impl AsRef<Path>) -> Logos {
        Logos { dir: dir.as_ref().to_path_buf() }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn path(&self, url: &str) -> PathBuf {
        self.dir.join(format!("{}.png", logo_key(url)))
    }

    fn fail_path(&self, url: &str) -> PathBuf {
        self.dir.join(format!("{}.png.fail", logo_key(url)))
    }

    /// The saved logo's PNG bytes.
    pub fn cached(&self, url: &str) -> Option<Vec<u8>> {
        if url.is_empty() {
            return None;
        }
        std::fs::read(self.path(url)).ok()
    }

    /// A re-saved logo gets a fresh download try.
    pub fn clear_fail(&self, url: &str) {
        if !url.is_empty() {
            let _ = std::fs::remove_file(self.fail_path(url));
        }
    }

    /// Saves an image (any format) as the logo of a URL.
    pub fn store(&self, url: &str, bytes: &[u8]) -> Result<(), String> {
        let img = image::load_from_memory(bytes).map_err(|e| e.to_string())?;
        let small = shrink(&img);
        std::fs::create_dir_all(&self.dir).map_err(|e| e.to_string())?;
        small.save_with_format(self.path(url), ImageFormat::Png).map_err(|e| e.to_string())?;
        let _ = std::fs::remove_file(self.fail_path(url));
        Ok(())
    }

    /// Downloads a logo if it isn't saved yet. Returns true only when something new was saved.
    pub fn fetch(&self, url: &str, timeout: f64, get: &dyn Fn(&str, f64) -> Result<Vec<u8>, String>) -> bool {
        let lower = url.to_lowercase();
        if !(lower.starts_with("http://") || lower.starts_with("https://")) {
            return false;
        }
        if self.path(url).exists() {
            return false;
        }
        let fail = self.fail_path(url);
        if let Ok(m) = std::fs::metadata(&fail).and_then(|m| m.modified()) {
            let age = SystemTime::now().duration_since(m).unwrap_or(Duration::ZERO);
            if age < Duration::from_secs(LOGO_RETRY_HOURS * 3600) {
                return false;
            }
        }
        match get(url, timeout).and_then(|b| self.store(url, &b)) {
            Ok(()) => true,
            Err(_) => {
                let _ = std::fs::create_dir_all(&self.dir);
                let _ = std::fs::write(&fail, chrono::Utc::now().timestamp_millis().to_string());
                false
            }
        }
    }

    /// Downloads every missing logo at once. True when anything new was saved.
    pub fn ensure(&self, urls: &[String], timeout: f64, get: &(dyn Fn(&str, f64) -> Result<Vec<u8>, String> + Sync)) -> bool {
        let set: HashSet<&String> = urls.iter().filter(|u| !u.is_empty()).collect();
        std::thread::scope(|sc| {
            let hs: Vec<_> = set.into_iter().map(|u| sc.spawn(move || self.fetch(u, timeout, get))).collect();
            hs.into_iter().map(|h| h.join().unwrap_or(false)).fold(false, |a, b| a || b)
        })
    }

    /// Removes saved logos that no button uses any more (after a logo URL was changed).
    pub fn prune(&self, in_use: &[String]) {
        let keep: HashSet<String> = in_use.iter().filter(|u| !u.is_empty()).map(|u| logo_key(u)).collect();
        if let Ok(rd) = std::fs::read_dir(&self.dir) {
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().to_string();
                let stem = name.split('.').next().unwrap_or("").to_string();
                if !keep.contains(&stem) {
                    let _ = std::fs::remove_file(e.path());
                }
            }
        }
    }

    /// Settings > Data > Clear logo cache.
    pub fn clear_all(&self) {
        if let Ok(rd) = std::fs::read_dir(&self.dir) {
            for e in rd.flatten() {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn keys_match_the_ios_version() {
        // reference values computed with the original JavaScript
        assert_eq!(logo_key("https://cdn.jsdelivr.net/gh/homarr-labs/dashboard-icons/png/jellyfin.png"), "b8bvvt-20");
        assert_eq!(logo_key("a"), "3t1g-1");
        assert_eq!(logo_key("ü€x"), "36x8gd-3");
    }

    fn png(w: u32, h: u32) -> Vec<u8> {
        let img = DynamicImage::ImageRgba8(RgbaImage::from_pixel(w, h, image::Rgba([255, 0, 0, 255])));
        let mut out = Cursor::new(vec![]);
        img.write_to(&mut out, ImageFormat::Png).unwrap();
        out.into_inner()
    }

    #[test]
    fn shrink_keeps_shape() {
        let img = image::load_from_memory(&png(400, 200)).unwrap();
        let s = shrink(&img);
        assert_eq!(s.dimensions(), (96, 96));
        assert_eq!(s.get_pixel(48, 10)[3], 0, "transparent above a wide logo");
        assert_eq!(s.get_pixel(48, 48)[0], 255);
        let tiny = shrink(&image::load_from_memory(&png(10, 20)).unwrap());
        assert_eq!(tiny.dimensions(), (20, 20));
    }

    #[test]
    fn download_retry_and_prune() {
        let dir = tempfile::tempdir().unwrap();
        let l = Logos::new(dir.path().join("logos"));
        let ok = |_: &str, _: f64| Ok(png(50, 50));
        let bad = |_: &str, _: f64| Err::<Vec<u8>, String>("404".into());
        assert!(!l.fetch("ftp://x", 1.0, &ok));
        assert!(!l.fetch("https://e/x.png", 1.0, &bad));
        assert!(!l.fetch("https://e/x.png", 1.0, &ok), "a failure waits before trying again");
        l.clear_fail("https://e/x.png");
        assert!(l.fetch("https://e/x.png", 1.0, &ok));
        assert!(!l.fetch("https://e/x.png", 1.0, &ok), "already saved");
        assert!(l.cached("https://e/x.png").is_some());
        assert!(l.ensure(&["https://e/y.png".into(), "https://e/y.png".into(), String::new()], 1.0, &ok));
        l.prune(&["https://e/y.png".into()]);
        assert!(l.cached("https://e/x.png").is_none());
        assert!(l.cached("https://e/y.png").is_some());
        l.clear_all();
        assert!(l.cached("https://e/y.png").is_none());
    }
}
