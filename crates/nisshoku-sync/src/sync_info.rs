//! `info.json`: the sync target format version and encryption settings.

use serde_json::{Value, json};

use crate::e2ee::{Keyring, MasterKey};
use crate::error::{Result, SyncError};
use crate::storage::{
    INFO_FILE, LOCK_DIR, Precondition, RESOURCE_DIR, RemoteStorage, TEMP_DIR, item_id_from_path,
};

/// The sync target version this implementation reads and writes.
pub const SYNC_VERSION: i64 = 3;

#[derive(Debug, Clone)]
pub struct SyncInfo {
    raw: Value,
}

impl SyncInfo {
    pub fn new() -> Self {
        Self {
            raw: json!({
                "version": SYNC_VERSION,
                "e2ee": { "value": false, "updatedTime": 0 },
                "activeMasterKeyId": { "value": "", "updatedTime": 0 },
                "masterKeys": [],
                "ppk": { "value": null, "updatedTime": 0 },
                "appMinVersion": "0.0.0",
            }),
        }
    }

    pub fn parse(text: &str) -> Result<Self> {
        let raw: Value = serde_json::from_str(text)
            .map_err(|e| SyncError::IncompatibleTarget(format!("invalid {INFO_FILE}: {e}")))?;
        Ok(Self { raw })
    }

    pub fn version(&self) -> i64 {
        self.raw.get("version").and_then(Value::as_i64).unwrap_or(0)
    }

    pub fn encryption_enabled(&self) -> bool {
        self.raw
            .pointer("/e2ee/value")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    }

    pub fn master_keys(&self) -> Vec<Value> {
        self.raw
            .get("masterKeys")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
    }

    pub fn active_master_key_id(&self) -> Option<String> {
        self.raw
            .pointer("/activeMasterKeyId/value")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    }

    fn set_timed(&mut self, key: &str, value: Value) {
        self.raw[key] = json!({ "value": value, "updatedTime": nisshoku_core::time::now_ms() });
    }

    /// Decrypts the master keys that `password` opens. When encryption is
    /// enabled, the active key must be among them.
    pub fn keyring(&self, password: Option<&str>) -> Result<Keyring> {
        let keys: Vec<MasterKey> = match password {
            Some(pw) => self
                .master_keys()
                .iter()
                .filter_map(|entry| MasterKey::decrypt(entry, pw).ok())
                .collect(),
            None => Vec::new(),
        };
        if !self.encryption_enabled() {
            return Ok(Keyring::new(keys, None));
        }
        if password.is_none() {
            return Err(SyncError::EncryptionPasswordRequired);
        }
        let active = self.active_master_key_id();
        if !active
            .as_ref()
            .is_some_and(|id| keys.iter().any(|k| &k.id == id))
        {
            return Err(SyncError::WrongEncryptionPassword);
        }
        Ok(Keyring::new(keys, active))
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(&self.raw).expect("serializable")
    }
}

impl Default for SyncInfo {
    fn default() -> Self {
        Self::new()
    }
}

/// Reads the target's info, falling back to the `.sync/version.txt` file used
/// by older targets. Returns `None` if the target has neither.
pub async fn fetch_sync_info(storage: &dyn RemoteStorage) -> Result<Option<SyncInfo>> {
    if let Some(data) = storage.get(INFO_FILE).await? {
        return SyncInfo::parse(&String::from_utf8_lossy(&data)).map(Some);
    }
    if let Some(data) = storage.get(".sync/version.txt").await? {
        let version = String::from_utf8_lossy(&data)
            .trim()
            .parse::<i64>()
            .unwrap_or(0);
        return Ok(Some(SyncInfo {
            raw: json!({ "version": version }),
        }));
    }
    Ok(None)
}

/// Checks that the target can be synced with, initializing it if it is empty.
pub async fn prepare_target(storage: &dyn RemoteStorage) -> Result<SyncInfo> {
    let info = match fetch_sync_info(storage).await? {
        Some(info) => info,
        None => {
            let has_items = storage
                .list("")
                .await?
                .iter()
                .any(|s| !s.is_dir && item_id_from_path(&s.path).is_some());
            if has_items {
                return Err(SyncError::IncompatibleTarget(
                    "the target contains items but no version information".into(),
                ));
            }
            storage.mkdir("").await?;
            let info = SyncInfo::new();
            storage.put(INFO_FILE, info.to_json().as_bytes()).await?;
            info
        }
    };

    match info.version() {
        v if v < SYNC_VERSION => {
            return Err(SyncError::IncompatibleTarget(format!(
                "the target uses format version {v}; upgrade it by syncing once with a current Joplin client"
            )));
        }
        v if v > SYNC_VERSION => {
            return Err(SyncError::IncompatibleTarget(format!(
                "the target uses format version {v}, which is newer than this app supports ({SYNC_VERSION})"
            )));
        }
        _ => {}
    }
    for dir in [LOCK_DIR, TEMP_DIR, RESOURCE_DIR] {
        storage.mkdir(dir).await?;
    }
    Ok(info)
}

/// Applies `change` to `info.json` without overwriting concurrent changes.
async fn update_sync_info(
    storage: &dyn RemoteStorage,
    mut change: impl FnMut(&mut SyncInfo) -> Result<()>,
) -> Result<SyncInfo> {
    for _ in 0..5 {
        let (mut info, precondition) = match storage.get_versioned(INFO_FILE).await? {
            Some((data, version)) => (
                SyncInfo::parse(&String::from_utf8_lossy(&data))?,
                version.map_or(Precondition::None, Precondition::Version),
            ),
            None => (SyncInfo::new(), Precondition::Absent),
        };
        change(&mut info)?;
        if storage
            .put_if(INFO_FILE, info.to_json().as_bytes(), &precondition)
            .await?
        {
            return Ok(info);
        }
    }
    Err(SyncError::Remote(format!(
        "{INFO_FILE} kept changing; try again"
    )))
}

/// Turns on encryption for the target, reusing a master key the password
/// opens or generating a new one. Items are re-uploaded by the next sync.
pub async fn enable_encryption(storage: &dyn RemoteStorage, password: &str) -> Result<SyncInfo> {
    prepare_target(storage).await?;
    update_sync_info(storage, |info| {
        // Prefer the previously active key, then any key the password opens.
        let openable: Vec<String> = info
            .master_keys()
            .iter()
            .filter_map(|entry| MasterKey::decrypt(entry, password).ok())
            .map(|k| k.id)
            .collect();
        let existing = info
            .active_master_key_id()
            .filter(|id| openable.contains(id))
            .or_else(|| openable.first().cloned());
        let id = match existing {
            Some(id) => id,
            None => {
                let (key, entry) = MasterKey::generate(password, "app.nisshoku.desktop");
                let mut keys = info.master_keys();
                keys.push(entry);
                info.raw["masterKeys"] = Value::Array(keys);
                key.id
            }
        };
        info.set_timed("activeMasterKeyId", json!(id));
        info.set_timed("e2ee", json!(true));
        Ok(())
    })
    .await
}

/// Turns off encryption. Keys are kept so that encrypted data stays readable.
pub async fn disable_encryption(storage: &dyn RemoteStorage) -> Result<SyncInfo> {
    prepare_target(storage).await?;
    update_sync_info(storage, |info| {
        info.set_timed("e2ee", json!(false));
        Ok(())
    })
    .await
}

/// Re-encrypts the master keys that `old` opens with `new`.
pub async fn change_master_password(
    storage: &dyn RemoteStorage,
    old: &str,
    new: &str,
) -> Result<SyncInfo> {
    update_sync_info(storage, |info| {
        let mut changed = 0;
        let keys: Vec<Value> = info
            .master_keys()
            .into_iter()
            .map(|entry| match MasterKey::decrypt(&entry, old) {
                Ok(key) => {
                    changed += 1;
                    key.reencrypt(&entry, new)
                }
                Err(_) => Ok(entry),
            })
            .collect::<Result<_>>()?;
        if changed == 0 {
            return Err(SyncError::WrongEncryptionPassword);
        }
        info.raw["masterKeys"] = Value::Array(keys);
        Ok(())
    })
    .await
}
