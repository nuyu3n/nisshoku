//! Passwords and tokens, kept in the operating system's credential store
//! (Windows Credential Manager, macOS Keychain, Secret Service on Linux)
//! rather than in the database.
//!
//! - Android has no such store reachable from Rust, so there they live in a
//!   file of the app's private `no_backup` directory (unreadable by other
//!   apps and left out of cloud backups).
//! - Linux desktops without a Secret Service (GNOME Keyring, KWallet…), such
//!   as some bare window managers, get the same kind of file in the profile,
//!   readable by the user alone.

pub const NEXTCLOUD_PASSWORD: &str = "nextcloud-app-password";
pub const GDRIVE_REFRESH_TOKEN: &str = "gdrive-refresh-token";
/// Of the user's own Google OAuth client, when the build has none.
pub const GDRIVE_CLIENT_SECRET: &str = "gdrive-client-secret";
pub const ONEDRIVE_REFRESH_TOKEN: &str = "onedrive-refresh-token";
pub const MASTER_PASSWORD: &str = "e2ee-master-password";

/// Secrets in a JSON file (Android, and Linux without a Secret Service).
#[cfg(any(target_os = "android", target_os = "linux"))]
mod file {
    use std::collections::BTreeMap;
    use std::path::PathBuf;
    use std::sync::{Mutex, OnceLock};

    static FILE: OnceLock<PathBuf> = OnceLock::new();
    static LOCK: Mutex<()> = Mutex::new(());

    pub fn init(path: PathBuf) {
        let _ = FILE.set(path);
    }

    fn read() -> BTreeMap<String, String> {
        FILE.get()
            .and_then(|f| std::fs::read(f).ok())
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    fn write(map: &BTreeMap<String, String>) -> Result<(), String> {
        let file = FILE.get().ok_or("secrets are not set up")?;
        let save = || -> std::io::Result<()> {
            std::fs::create_dir_all(file.parent().unwrap())?;
            let tmp = file.with_extension("tmp");
            std::fs::write(&tmp, serde_json::to_vec(map)?)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))?;
            }
            std::fs::rename(&tmp, file)
        };
        save().map_err(|e| format!("パスワードを保存できませんでした: {e}"))
    }

    pub fn get(name: &str) -> Option<String> {
        let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        read().remove(name)
    }

    pub fn set(name: &str, value: &str) -> Result<(), String> {
        let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut map = read();
        map.insert(name.into(), value.into());
        write(&map)
    }

    pub fn remove(name: &str) {
        let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut map = read();
        if map.remove(name).is_some() {
            let _ = write(&map);
        }
    }
}

/// The operating system's credential store.
#[cfg(not(target_os = "android"))]
mod keychain {
    const SERVICE: &str = "app.nisshoku.desktop";

    fn entry(name: &str) -> Result<keyring::Entry, String> {
        keyring::Entry::new(SERVICE, name).map_err(|e| e.to_string())
    }

    /// Windows keeps at most 2560 bytes (1280 UTF-16 units) per credential,
    /// less than some refresh tokens (Microsoft's): longer values are split
    /// over several entries, the first one saying how many.
    const CHUNK: usize = 1000;
    const PREFIX: &str = "nisshoku-chunks:";

    fn part(name: &str, i: usize) -> String {
        format!("{name}#{i}")
    }

    pub fn get(name: &str) -> Option<String> {
        let first = entry(name).ok()?.get_password().ok()?;
        let Some(n) = first.strip_prefix(PREFIX).and_then(|n| n.parse::<usize>().ok()) else {
            return Some(first);
        };
        let mut out = String::new();
        for i in 1..=n {
            out.push_str(&entry(&part(name, i)).ok()?.get_password().ok()?);
        }
        Some(out)
    }

    pub fn set(name: &str, value: &str) -> Result<(), String> {
        let failed = |e: keyring::Error| format!("パスワードを保存できませんでした: {e}");
        remove_parts(name);
        let chars: Vec<char> = value.chars().collect();
        if chars.len() <= CHUNK && !value.starts_with(PREFIX) {
            return entry(name)?.set_password(value).map_err(failed);
        }
        let parts: Vec<String> = chars.chunks(CHUNK).map(|c| c.iter().collect()).collect();
        for (i, p) in parts.iter().enumerate() {
            entry(&part(name, i + 1))?.set_password(p).map_err(failed)?;
        }
        entry(name)?
            .set_password(&format!("{PREFIX}{}", parts.len()))
            .map_err(failed)
    }

    fn remove_parts(name: &str) {
        let Some(n) = entry(name)
            .ok()
            .and_then(|e| e.get_password().ok())
            .and_then(|v| v.strip_prefix(PREFIX).and_then(|n| n.parse::<usize>().ok()))
        else {
            return;
        };
        for i in 1..=n {
            if let Ok(e) = entry(&part(name, i)) {
                let _ = e.delete_credential();
            }
        }
    }

    pub fn remove(name: &str) {
        remove_parts(name);
        if let Ok(e) = entry(name) {
            let _ = e.delete_credential();
        }
    }
}

/// `profile` is the folder of the notes (on Android, the app's data directory).
#[allow(unused_variables)]
pub fn init(profile: &std::path::Path) {
    #[cfg(target_os = "android")]
    file::init(profile.join("no_backup").join("secrets.json"));
    #[cfg(target_os = "linux")]
    file::init(profile.join("secrets.json"));
}

#[cfg(target_os = "android")]
pub use file::{get, remove, set};

#[cfg(not(any(target_os = "android", target_os = "linux")))]
pub use keychain::{get, remove, set};

// Linux: the Secret Service when the desktop has one, else the file.
#[cfg(target_os = "linux")]
pub fn get(name: &str) -> Option<String> {
    keychain::get(name).or_else(|| file::get(name))
}

#[cfg(target_os = "linux")]
pub fn set(name: &str, value: &str) -> Result<(), String> {
    match keychain::set(name, value) {
        Ok(()) => {
            file::remove(name);
            Ok(())
        }
        Err(e) => {
            tracing::warn!("no Secret Service, keeping {name} in a private file: {e}");
            file::set(name, value)
        }
    }
}

#[cfg(target_os = "linux")]
pub fn remove(name: &str) {
    keychain::remove(name);
    file::remove(name);
}
