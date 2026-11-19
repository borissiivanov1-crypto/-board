//! Saved logins, the Linux helper's key and the Glances password.
//! Stored in the GNOME keyring (Secret Service). Without a keyring (e.g. a minimal desktop), they go
//! to a private file readable only by you (~/.local/share/board/secrets.json, mode 600).
//! The names are the same as the iOS version's Keychain names.

use serde_json::{Map, Value};
use std::path::PathBuf;
use std::sync::Mutex;

pub trait Backend: Send + Sync {
    fn get(&self, k: &str) -> Option<String>;
    fn set(&self, k: &str, v: &str) -> Result<(), String>;
    fn remove(&self, k: &str);
}

/// The GNOME keyring.
pub struct Keyring;

const SERVICE: &str = "board";

impl Backend for Keyring {
    fn get(&self, k: &str) -> Option<String> {
        keyring::Entry::new(SERVICE, k).ok()?.get_password().ok()
    }
    fn set(&self, k: &str, v: &str) -> Result<(), String> {
        keyring::Entry::new(SERVICE, k).and_then(|e| e.set_password(v)).map_err(|e| e.to_string())
    }
    fn remove(&self, k: &str) {
        if let Ok(e) = keyring::Entry::new(SERVICE, k) {
            let _ = e.delete_credential();
        }
    }
}

/// A private JSON file (mode 600).
pub struct FileStore {
    path: PathBuf,
    lock: Mutex<()>,
}

impl FileStore {
    pub fn new(path: PathBuf) -> FileStore {
        FileStore { path, lock: Mutex::new(()) }
    }
    fn read(&self) -> Map<String, Value> {
        std::fs::read_to_string(&self.path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
    }
    fn write(&self, m: &Map<String, Value>) -> Result<(), String> {
        if let Some(d) = self.path.parent() {
            std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
        }
        let raw = serde_json::to_string(m).unwrap();
        #[cfg(unix)]
        {
            use std::io::Write;
            use std::os::unix::fs::OpenOptionsExt;
            let mut f = std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(&self.path).map_err(|e| e.to_string())?;
            f.write_all(raw.as_bytes()).map_err(|e| e.to_string())?;
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&self.path, std::fs::Permissions::from_mode(0o600));
            Ok(())
        }
        #[cfg(not(unix))]
        std::fs::write(&self.path, raw).map_err(|e| e.to_string())
    }
}

impl Backend for FileStore {
    fn get(&self, k: &str) -> Option<String> {
        let _g = self.lock.lock().unwrap();
        self.read().get(k).and_then(|v| v.as_str()).map(String::from)
    }
    fn set(&self, k: &str, v: &str) -> Result<(), String> {
        let _g = self.lock.lock().unwrap();
        let mut m = self.read();
        m.insert(k.into(), Value::String(v.into()));
        self.write(&m)
    }
    fn remove(&self, k: &str) {
        let _g = self.lock.lock().unwrap();
        let mut m = self.read();
        if m.remove(k).is_some() {
            let _ = self.write(&m);
        }
    }
}

/// Keychain names for each server type's login.
fn login_names(platform: &str) -> Option<(&'static str, &'static str)> {
    Some(match platform {
        "zimaos" => ("board-zima-user", "board-zima-pass"),
        "proxmox" => ("board-pve-user", "board-pve-pass"),
        "truenas" => ("board-truenas-user", "board-truenas-pass"),
        "unraid" => ("board-unraid-user", "board-unraid-pass"),
        "synology" => ("board-syno-user", "board-syno-pass"),
        "omv" => ("board-omv-user", "board-omv-pass"),
        _ => return None,
    })
}
const LOGIN_PLATFORMS: [&str; 6] = ["zimaos", "proxmox", "truenas", "unraid", "synology", "omv"];
const HELPER_KEY_NAME: &str = "board-helper-key";
const GLANCES_PASS_NAME: &str = "board-glances-pass";

/// Every server keeps its own logins. Server 1 ("s1") uses the plain names.
pub fn kname(base: &str, id: &str) -> String {
    if id.is_empty() || id == "s1" {
        base.to_string()
    } else {
        format!("{base}@{id}")
    }
}

pub struct Secrets {
    backend: Box<dyn Backend>,
    /// "keyring" or "file"
    pub kind: &'static str,
}

impl Secrets {
    pub fn new(backend: Box<dyn Backend>, kind: &'static str) -> Secrets {
        Secrets { backend, kind }
    }

    /// The keyring when it works, otherwise the private file. BOARD_SECRETS=file forces the file.
    pub fn open(file: PathBuf) -> Secrets {
        let forced = std::env::var("BOARD_SECRETS").map(|v| v == "file").unwrap_or(false);
        if !forced {
            let probe = keyring::Entry::new(SERVICE, "board-probe").map(|e| e.get_password());
            if matches!(probe, Ok(Ok(_)) | Ok(Err(keyring::Error::NoEntry))) {
                return Secrets::new(Box::new(Keyring), "keyring");
            }
        }
        Secrets::new(Box::new(FileStore::new(file)), "file")
    }

    /// Where logins are kept, for the hints in Settings.
    pub fn place(&self) -> &'static str {
        if self.kind == "keyring" {
            "Stored in the GNOME keyring"
        } else {
            "Stored in a private file (no keyring found)"
        }
    }

    pub fn get(&self, k: &str) -> Option<String> {
        self.backend.get(k)
    }
    pub fn has(&self, k: &str) -> bool {
        self.backend.get(k).is_some()
    }
    pub fn set(&self, k: &str, v: &str) -> Result<(), String> {
        self.backend.set(k, v)
    }
    pub fn remove(&self, k: &str) {
        self.backend.remove(k)
    }

    pub fn login_keys(platform: &str, id: &str) -> Option<(String, String)> {
        login_names(platform).map(|(u, p)| (kname(u, id), kname(p, id)))
    }
    pub fn helper_key(id: &str) -> String {
        kname(HELPER_KEY_NAME, id)
    }
    pub fn gpass_key(id: &str) -> String {
        kname(GLANCES_PASS_NAME, id)
    }

    /// Linux: "logged in" = the helper's secret key is saved.
    pub fn has_login(&self, platform: &str, id: &str) -> bool {
        if platform == "linux" {
            return self.has(&Self::helper_key(id));
        }
        match Self::login_keys(platform, id) {
            Some((u, p)) => self.has(&u) && self.has(&p),
            None => false,
        }
    }

    pub fn forget_login(&self, platform: &str, id: &str) {
        if platform == "linux" {
            return self.remove(&Self::helper_key(id));
        }
        if let Some((u, p)) = Self::login_keys(platform, id) {
            self.remove(&u);
            self.remove(&p);
        }
    }

    pub fn saved_login(&self, platform: &str, id: &str) -> Option<(String, String)> {
        let (u, p) = Self::login_keys(platform, id)?;
        Some((self.get(&u)?, self.get(&p)?))
    }

    pub fn save_login(&self, platform: &str, id: &str, user: &str, pass: &str) -> Result<(), String> {
        let (u, p) = Self::login_keys(platform, id).ok_or("no login for this server type")?;
        self.set(&u, user)?;
        self.set(&p, pass)
    }

    pub fn glances_pass(&self, id: &str) -> String {
        // CONFIG.password when none is saved
        self.get(&Self::gpass_key(id)).unwrap_or_else(|| crate::config::start().password.clone())
    }

    /// Everything saved for one server (when it's removed).
    pub fn forget_server(&self, id: &str) {
        self.remove(&Self::helper_key(id));
        self.remove(&Self::gpass_key(id));
        for p in LOGIN_PLATFORMS {
            self.forget_login(p, id);
        }
    }
}

/// In-memory secrets (tests).
#[derive(Default)]
pub struct Memory(pub Mutex<std::collections::HashMap<String, String>>);

impl Backend for Memory {
    fn get(&self, k: &str) -> Option<String> {
        self.0.lock().unwrap().get(k).cloned()
    }
    fn set(&self, k: &str, v: &str) -> Result<(), String> {
        self.0.lock().unwrap().insert(k.into(), v.into());
        Ok(())
    }
    fn remove(&self, k: &str) {
        self.0.lock().unwrap().remove(k);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_follow_the_ios_keychain() {
        assert_eq!(Secrets::login_keys("proxmox", "s1").unwrap(), ("board-pve-user".into(), "board-pve-pass".into()));
        assert_eq!(Secrets::login_keys("synology", "sabc").unwrap().0, "board-syno-user@sabc");
        assert_eq!(Secrets::helper_key("s2"), "board-helper-key@s2");
        assert!(Secrets::login_keys("linux", "s1").is_none());
    }

    #[test]
    fn logins_per_server() {
        let s = Secrets::new(Box::<Memory>::default(), "file");
        s.save_login("omv", "s1", "admin", "pw").unwrap();
        s.save_login("omv", "s2", "admin2", "pw2").unwrap();
        s.set(&Secrets::helper_key("s2"), "abc").unwrap();
        assert!(s.has_login("omv", "s1"));
        assert!(s.has_login("linux", "s2"));
        assert!(!s.has_login("linux", "s1"));
        assert_eq!(s.saved_login("omv", "s2").unwrap(), ("admin2".into(), "pw2".into()));
        s.forget_server("s2");
        assert!(!s.has_login("omv", "s2"));
        assert!(!s.has_login("linux", "s2"));
        assert!(s.has_login("omv", "s1"));
        s.forget_login("omv", "s1");
        assert!(!s.has_login("omv", "s1"));
    }

    #[test]
    fn private_file() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("x/secrets.json");
        let f = FileStore::new(p.clone());
        f.set("a", "1").unwrap();
        assert_eq!(f.get("a").as_deref(), Some("1"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(&p).unwrap().permissions().mode() & 0o777, 0o600);
        }
        f.remove("a");
        assert!(f.get("a").is_none());
    }
}
