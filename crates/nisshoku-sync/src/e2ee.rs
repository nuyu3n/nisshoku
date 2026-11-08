//! End-to-end encryption, compatible with Joplin's.
//!
//! Encrypted data is a text container ("JED01"):
//!
//! ```text
//! JED  01  <metadata length: 6 hex>  <method: 2 hex><master key id: 32 hex>
//! then chunks:  <length: 6 hex> <chunk data>
//! ```
//!
//! Chunks of the native methods are JSON `{"salt","iv","ct"}` (base64):
//! AES-256-GCM (96-bit IV, 128-bit tag appended to `ct`, no associated data)
//! under a key derived with PBKDF2-HMAC-SHA512 from a per-chunk salt.
//!
//! | method | what | key material | PBKDF2 rounds | plaintext |
//! |---|---|---|---|---|
//! | KeyV1 (8) | master keys | master password | 220 000 | raw key bytes |
//! | FileV1 (9) | attachment data | master key (hex) | 3 | raw bytes |
//! | StringV1 (10) | items | master key (hex) | 3 | UTF-16LE text |
//!
//! Older Joplin versions used SJCL (AES-CCM, PBKDF2-HMAC-SHA256, parameters
//! stored in each chunk's JSON). Those methods (1–7) can be read, not written.
//!
//! Master keys are 256 random bytes, used as their hex string. They are
//! stored in `info.json`, encrypted with the master password.

use std::collections::HashMap;
use std::path::Path;

use aes_gcm::aead::{Aead, KeyInit};
use base64::Engine;
use serde_json::{Value, json};

use crate::error::{Result, SyncError};

const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

pub const METHOD_KEY_V1: u8 = 8;
pub const METHOD_FILE_V1: u8 = 9;
pub const METHOD_STRING_V1: u8 = 10;

const KEY_ROUNDS: u32 = 220_000;
const DATA_ROUNDS: u32 = 3;
/// Plaintext bytes per chunk. Text chunks never split a UTF-16 surrogate pair.
const CHUNK_BYTES: usize = 64 * 1024;

fn err(msg: impl Into<String>) -> SyncError {
    SyncError::Item(msg.into())
}

fn random<const N: usize>() -> [u8; N] {
    let mut out = [0u8; N];
    getrandom::fill(&mut out).expect("system random generator");
    out
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

// -------------------------------------------------------------------------
// Native methods (AES-256-GCM)

fn derive_gcm(material: &[u8], salt: &[u8], rounds: u32) -> aes_gcm::Aes256Gcm {
    let mut key = [0u8; 32];
    pbkdf2::pbkdf2_hmac::<sha2::Sha512>(material, salt, rounds, &mut key);
    aes_gcm::Aes256Gcm::new_from_slice(&key).expect("32-byte key")
}

fn gcm_encrypt(material: &[u8], rounds: u32, plaintext: &[u8]) -> String {
    let salt = random::<32>();
    let iv = random::<12>();
    let ct = derive_gcm(material, &salt, rounds)
        .encrypt(&aes_gcm::Nonce::from(iv), plaintext)
        .expect("in-memory encryption");
    json!({ "salt": B64.encode(salt), "iv": B64.encode(iv), "ct": B64.encode(ct) }).to_string()
}

fn gcm_decrypt(material: &[u8], rounds: u32, chunk: &str) -> Result<Vec<u8>> {
    let v: Value = serde_json::from_str(chunk).map_err(|_| err("invalid encrypted chunk"))?;
    let field = |name: &str| -> Result<Vec<u8>> {
        v.get(name)
            .and_then(Value::as_str)
            .and_then(|s| B64.decode(s).ok())
            .ok_or_else(|| err(format!("encrypted chunk without {name}")))
    };
    let (salt, iv, ct) = (field("salt")?, field("iv")?, field("ct")?);
    let nonce = aes_gcm::Nonce::try_from(&iv[..]).map_err(|_| err("invalid IV"))?;
    derive_gcm(material, &salt, rounds)
        .decrypt(&nonce, &ct[..])
        .map_err(|_| err("decryption failed (wrong key or corrupted data)"))
}

// -------------------------------------------------------------------------
// Legacy SJCL (AES-CCM), decryption only

fn sjcl_decrypt(password: &[u8], chunk: &str) -> Result<Vec<u8>> {
    use ccm::aead::AeadInOut;
    use ccm::consts::{U8, U11, U12, U13, U16};

    let v: Value = serde_json::from_str(chunk).map_err(|_| err("invalid SJCL data"))?;
    if v.get("mode").and_then(Value::as_str) != Some("ccm") {
        return Err(err("unsupported SJCL mode"));
    }
    let b64 = |name: &str| -> Result<Vec<u8>> {
        v.get(name)
            .and_then(Value::as_str)
            .and_then(|s| B64.decode(s).ok())
            .ok_or_else(|| err(format!("SJCL data without {name}")))
    };
    let num = |name: &str, default: u64| v.get(name).and_then(Value::as_u64).unwrap_or(default);
    let (salt, iv, mut buf) = (b64("salt")?, b64("iv")?, b64("ct")?);
    let (iter, ks, ts) = (
        num("iter", 10_000) as u32,
        num("ks", 128) as usize,
        num("ts", 64) as usize,
    );
    let adata = v
        .get("adata")
        .and_then(Value::as_str)
        .map(|s| B64.decode(s).unwrap_or_default())
        .unwrap_or_default();

    let mut key = vec![0u8; ks / 8];
    pbkdf2::pbkdf2_hmac::<sha2::Sha256>(password, &salt, iter, &mut key);

    // SJCL shortens the 16-byte IV according to the message length.
    let tag_len = ts / 8;
    let plain_len = buf
        .len()
        .checked_sub(tag_len)
        .ok_or_else(|| err("SJCL data too short"))?;
    let mut l = 2;
    while l < 4 && (plain_len >> (8 * l)) != 0 {
        l += 1;
    }
    let nonce = iv.get(..15 - l).ok_or_else(|| err("SJCL IV too short"))?;
    let (msg, tag) = buf.split_at_mut(plain_len);
    let tag = tag.to_vec();

    macro_rules! run {
        ($aes:ty, $tag:ty, $nonce:ty) => {{
            let cipher =
                ccm::Ccm::<$aes, $tag, $nonce>::new_from_slice(&key).map_err(|_| err("bad key"))?;
            let nonce = ccm::Nonce::<$nonce>::try_from(nonce).map_err(|_| err("bad nonce"))?;
            let tag = ccm::Tag::<$tag>::try_from(&tag[..]).map_err(|_| err("bad tag"))?;
            cipher
                .decrypt_inout_detached(&nonce, &adata, msg.into(), &tag)
                .map_err(|_| err("decryption failed (wrong key or corrupted data)"))?;
        }};
    }
    macro_rules! by_nonce {
        ($aes:ty, $tag:ty) => {
            match 15 - l {
                13 => run!($aes, $tag, U13),
                12 => run!($aes, $tag, U12),
                _ => run!($aes, $tag, U11),
            }
        };
    }
    match (ks, ts) {
        (128, 64) => by_nonce!(aes::Aes128, U8),
        (256, 64) => by_nonce!(aes::Aes256, U8),
        (128, 128) => by_nonce!(aes::Aes128, U16),
        (256, 128) => by_nonce!(aes::Aes256, U16),
        _ => return Err(err(format!("unsupported SJCL parameters ks={ks} ts={ts}"))),
    }
    buf.truncate(plain_len);
    Ok(buf)
}

/// JavaScript's `unescape()`: `%XX` is a Latin-1 character, `%uXXXX` a UTF-16
/// code unit.
fn js_unescape(s: &str) -> Result<String> {
    let bytes = s.as_bytes();
    let mut units: Vec<u16> = Vec::with_capacity(s.len());
    let hex = |b: &[u8]| {
        std::str::from_utf8(b)
            .ok()
            .and_then(|h| u16::from_str_radix(h, 16).ok())
    };
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            if bytes.get(i + 1) == Some(&b'u') {
                if let Some(u) = bytes.get(i + 2..i + 6).and_then(hex) {
                    units.push(u);
                    i += 6;
                    continue;
                }
            } else if let Some(u) = bytes.get(i + 1..i + 3).and_then(hex) {
                units.push(u);
                i += 3;
                continue;
            }
        }
        // Not an escape: copy the character as is.
        let ch = s[i..].chars().next().expect("in bounds");
        let mut buf = [0u16; 2];
        units.extend_from_slice(ch.encode_utf16(&mut buf));
        i += ch.len_utf8();
    }
    String::from_utf16(&units).map_err(|_| err("invalid escaped text"))
}

fn is_sjcl(method: u8) -> bool {
    (1..=7).contains(&method) && method != 6
}

// -------------------------------------------------------------------------
// Container

struct Container<'a> {
    method: u8,
    master_key_id: &'a str,
    chunks: Vec<&'a str>,
}

fn parse_container(text: &str) -> Result<Container<'_>> {
    let bad = || err("invalid encrypted data");
    if !text.starts_with("JED") {
        return Err(bad());
    }
    let version = text.get(3..5).ok_or_else(bad)?;
    if version != "01" {
        return Err(err(format!(
            "unsupported encryption format version {version}"
        )));
    }
    let meta_len =
        usize::from_str_radix(text.get(5..11).ok_or_else(bad)?, 16).map_err(|_| bad())?;
    let meta = text.get(11..11 + meta_len).ok_or_else(bad)?;
    let method = u8::from_str_radix(meta.get(..2).ok_or_else(bad)?, 16).map_err(|_| bad())?;
    let master_key_id = meta.get(2..34).ok_or_else(bad)?;
    let mut rest = &text[11 + meta_len..];
    let mut chunks = Vec::new();
    while !rest.trim_end().is_empty() {
        let len = usize::from_str_radix(rest.get(..6).ok_or_else(bad)?, 16).map_err(|_| bad())?;
        chunks.push(rest.get(6..6 + len).ok_or_else(bad)?);
        rest = &rest[6 + len..];
    }
    Ok(Container {
        method,
        master_key_id,
        chunks,
    })
}

fn write_header(out: &mut String, method: u8, master_key_id: &str) {
    let meta = format!("{method:02x}{master_key_id}");
    out.push_str(&format!("JED01{:06x}{meta}", meta.len()));
}

fn write_chunk(out: &mut String, chunk: &str) {
    out.push_str(&format!("{:06x}{chunk}", chunk.len()));
}

/// The master key ID in an encrypted container's header.
pub fn container_master_key_id(text: &str) -> Option<String> {
    parse_container(text)
        .ok()
        .map(|c| c.master_key_id.to_string())
}

// -------------------------------------------------------------------------
// Master keys

#[derive(Clone)]
pub struct MasterKey {
    pub id: String,
    /// The key as Joplin uses it: hex of its 256 bytes.
    secret: String,
}

impl std::fmt::Debug for MasterKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MasterKey({})", self.id)
    }
}

impl MasterKey {
    /// Generates a new key and its `info.json` entry (encrypted with `password`).
    pub fn generate(password: &str, source_application: &str) -> (MasterKey, Value) {
        let bytes = random::<256>();
        let id = nisshoku_core::ids::new_id();
        let now = nisshoku_core::time::now_ms();
        let content = gcm_encrypt(password.as_bytes(), KEY_ROUNDS, &bytes);
        let entry = json!({
            "id": id,
            "created_time": now,
            "updated_time": now,
            "source_application": source_application,
            "encryption_method": METHOD_KEY_V1,
            "checksum": "",
            "content": content,
            "hasBeenUsed": false,
        });
        (
            MasterKey {
                id,
                secret: hex(&bytes),
            },
            entry,
        )
    }

    /// Decrypts an `info.json` master key entry.
    pub fn decrypt(entry: &Value, password: &str) -> Result<MasterKey> {
        let id = entry
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let method = entry
            .get("encryption_method")
            .and_then(Value::as_u64)
            .unwrap_or(0) as u8;
        let content = entry
            .get("content")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let wrong = |_| SyncError::WrongEncryptionPassword;
        let secret = match method {
            METHOD_KEY_V1 => {
                hex(&gcm_decrypt(password.as_bytes(), KEY_ROUNDS, content).map_err(wrong)?)
            }
            m if is_sjcl(m) => {
                String::from_utf8(sjcl_decrypt(password.as_bytes(), content).map_err(wrong)?)
                    .map_err(|_| SyncError::WrongEncryptionPassword)?
            }
            m => {
                return Err(SyncError::IncompatibleTarget(format!(
                    "unsupported master key method {m}"
                )));
            }
        };
        Ok(MasterKey { id, secret })
    }

    /// Re-encrypts this key with a new password (for a password change).
    pub fn reencrypt(&self, entry: &Value, password: &str) -> Result<Value> {
        let bytes: Vec<u8> = (0..self.secret.len() / 2)
            .map(|i| u8::from_str_radix(&self.secret[2 * i..2 * i + 2], 16))
            .collect::<std::result::Result<_, _>>()
            .map_err(|_| err("master key is not hex"))?;
        let mut entry = entry.clone();
        entry["content"] = Value::String(gcm_encrypt(password.as_bytes(), KEY_ROUNDS, &bytes));
        entry["encryption_method"] = json!(METHOD_KEY_V1);
        entry["checksum"] = json!("");
        entry["updated_time"] = json!(nisshoku_core::time::now_ms());
        Ok(entry)
    }
}

/// The decrypted master keys available for a sync, and which one encrypts.
#[derive(Debug, Clone, Default)]
pub struct Keyring {
    keys: HashMap<String, MasterKey>,
    active: Option<String>,
}

impl Keyring {
    pub fn new(keys: Vec<MasterKey>, active: Option<String>) -> Self {
        Self {
            keys: keys.into_iter().map(|k| (k.id.clone(), k)).collect(),
            active,
        }
    }

    pub fn active(&self) -> Option<&MasterKey> {
        self.active.as_ref().and_then(|id| self.keys.get(id))
    }

    fn key_for(&self, container: &Container<'_>) -> Result<&MasterKey> {
        self.keys.get(container.master_key_id).ok_or_else(|| {
            err(format!(
                "data is encrypted with master key {}, which is not available",
                container.master_key_id
            ))
        })
    }

    pub fn encrypt_string(&self, text: &str) -> Result<String> {
        let key = self.active().ok_or_else(|| err("no active master key"))?;
        let units: Vec<u16> = text.encode_utf16().collect();
        let mut out = String::new();
        write_header(&mut out, METHOD_STRING_V1, &key.id);
        let per_chunk = CHUNK_BYTES / 2;
        let mut start = 0;
        while start < units.len() {
            let mut end = (start + per_chunk).min(units.len());
            if end < units.len() && (0xD800..0xDC00).contains(&units[end - 1]) {
                end -= 1; // keep surrogate pairs together
            }
            let bytes: Vec<u8> = units[start..end]
                .iter()
                .flat_map(|u| u.to_le_bytes())
                .collect();
            write_chunk(
                &mut out,
                &gcm_encrypt(key.secret.as_bytes(), DATA_ROUNDS, &bytes),
            );
            start = end;
        }
        Ok(out)
    }

    pub fn decrypt_string(&self, text: &str) -> Result<String> {
        let c = parse_container(text)?;
        let key = self.key_for(&c)?;
        match c.method {
            METHOD_STRING_V1 => {
                let mut bytes = Vec::new();
                for chunk in &c.chunks {
                    bytes.extend(gcm_decrypt(key.secret.as_bytes(), DATA_ROUNDS, chunk)?);
                }
                if bytes.len() % 2 != 0 {
                    return Err(err("invalid UTF-16 data"));
                }
                let units: Vec<u16> = bytes
                    .chunks(2)
                    .map(|p| u16::from_le_bytes([p[0], p[1]]))
                    .collect();
                String::from_utf16(&units).map_err(|_| err("invalid UTF-16 data"))
            }
            // Legacy text was passed through JavaScript's `escape()` first.
            m if is_sjcl(m) => {
                let mut escaped = String::new();
                for chunk in &c.chunks {
                    let plain = sjcl_decrypt(key.secret.as_bytes(), chunk)?;
                    escaped.push_str(
                        &String::from_utf8(plain).map_err(|_| err("invalid UTF-8 data"))?,
                    );
                }
                js_unescape(&escaped)
            }
            m => Err(err(format!("unsupported encryption method {m} for text"))),
        }
    }

    pub async fn encrypt_file(&self, src: &Path, dest: &Path) -> Result<()> {
        let key = self
            .active()
            .ok_or_else(|| err("no active master key"))?
            .clone();
        let (src, dest) = (src.to_path_buf(), dest.to_path_buf());
        tokio::task::spawn_blocking(move || -> Result<()> {
            use std::io::{Read, Write};
            let mut input = std::fs::File::open(&src)?;
            let mut output = std::io::BufWriter::new(std::fs::File::create(&dest)?);
            let mut header = String::new();
            write_header(&mut header, METHOD_FILE_V1, &key.id);
            output.write_all(header.as_bytes())?;
            let mut buf = vec![0u8; CHUNK_BYTES];
            loop {
                let mut filled = 0;
                while filled < buf.len() {
                    let n = input.read(&mut buf[filled..])?;
                    if n == 0 {
                        break;
                    }
                    filled += n;
                }
                if filled == 0 {
                    break;
                }
                let mut chunk = String::new();
                write_chunk(
                    &mut chunk,
                    &gcm_encrypt(key.secret.as_bytes(), DATA_ROUNDS, &buf[..filled]),
                );
                output.write_all(chunk.as_bytes())?;
                if filled < buf.len() {
                    break;
                }
            }
            output.flush()?;
            Ok(())
        })
        .await
        .map_err(|e| err(format!("encryption task failed: {e}")))?
    }

    pub async fn decrypt_file(&self, src: &Path, dest: &Path) -> Result<()> {
        let keyring = self.clone();
        let (src, dest) = (src.to_path_buf(), dest.to_path_buf());
        tokio::task::spawn_blocking(move || -> Result<()> {
            use std::io::Write;
            let text =
                std::fs::read_to_string(&src).map_err(|_| err("encrypted file is not text"))?;
            let c = parse_container(&text)?;
            let key = keyring.key_for(&c)?;
            let mut output = std::io::BufWriter::new(std::fs::File::create(&dest)?);
            for chunk in &c.chunks {
                let plain = match c.method {
                    METHOD_FILE_V1 => gcm_decrypt(key.secret.as_bytes(), DATA_ROUNDS, chunk)?,
                    // Legacy file chunks hold base64 text.
                    m if is_sjcl(m) => {
                        let b64 = sjcl_decrypt(key.secret.as_bytes(), chunk)?;
                        B64.decode(b64)
                            .map_err(|_| err("invalid legacy file data"))?
                    }
                    m => return Err(err(format!("unsupported encryption method {m} for files"))),
                };
                output.write_all(&plain)?;
            }
            output.flush()?;
            Ok(())
        })
        .await
        .map_err(|e| err(format!("decryption task failed: {e}")))?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keyring() -> Keyring {
        let (key, _) = MasterKey::generate("pw", "test");
        let id = key.id.clone();
        Keyring::new(vec![key], Some(id))
    }

    #[test]
    fn master_key_round_trip() {
        let (key, entry) = MasterKey::generate("correct horse", "test");
        let again = MasterKey::decrypt(&entry, "correct horse").unwrap();
        assert_eq!(again.secret, key.secret);
        assert_eq!(again.secret.len(), 512);
        assert!(matches!(
            MasterKey::decrypt(&entry, "wrong"),
            Err(SyncError::WrongEncryptionPassword)
        ));

        let changed = key.reencrypt(&entry, "new pw").unwrap();
        assert_eq!(
            MasterKey::decrypt(&changed, "new pw").unwrap().secret,
            key.secret
        );
    }

    #[test]
    fn strings_round_trip_across_chunks() {
        let k = keyring();
        for text in [
            "",
            "short",
            "日本語 🌒 emoji",
            &"🌒".repeat(40_000),
            &"x".repeat(100_000),
        ] {
            let enc = k.encrypt_string(text).unwrap();
            assert!(enc.starts_with("JED01"));
            assert_eq!(k.decrypt_string(&enc).unwrap(), text);
        }
        let enc = k.encrypt_string(&"🌒".repeat(40_000)).unwrap();
        assert!(parse_container(&enc).unwrap().chunks.len() > 1);
    }

    #[test]
    fn tampering_is_detected() {
        let k = keyring();
        let enc = k.encrypt_string("secret").unwrap();
        let tampered = enc.replacen("\"ct\":\"", "\"ct\":\"AAAA", 1);
        assert!(k.decrypt_string(&tampered).is_err());
        assert!(keyring().decrypt_string(&enc).is_err(), "other key");
    }

    #[test]
    fn unescapes_like_javascript() {
        assert_eq!(
            js_unescape("%u65E7%u5F62%0Aemoji%20%uD83C%uDF18%20100%25 a%zz%u12").unwrap(),
            "旧形
emoji 🌘 100% a%zz%u12"
        );
    }

    #[tokio::test]
    async fn files_round_trip() {
        let k = keyring();
        let dir = tempfile::tempdir().unwrap();
        for size in [0usize, 5, CHUNK_BYTES, CHUNK_BYTES * 2 + 7] {
            let data: Vec<u8> = (0..size).map(|i| (i % 251) as u8).collect();
            let (src, enc, out) = (
                dir.path().join("a"),
                dir.path().join("b"),
                dir.path().join("c"),
            );
            std::fs::write(&src, &data).unwrap();
            k.encrypt_file(&src, &enc).await.unwrap();
            k.decrypt_file(&enc, &out).await.unwrap();
            assert_eq!(std::fs::read(&out).unwrap(), data);
        }
    }

    /// The private key (a JWK) in an `info.json` written by Joplin 3.7.1 (SJCL4, AES-CCM,
    /// 256-bit key), encrypted with the master password "test-master-pw".
    #[test]
    fn decrypts_sjcl_written_by_joplin() {
        let chunk = include_str!("../tests/data/joplin_sjcl4_ppk.json");
        let plain = sjcl_decrypt(b"test-master-pw", chunk.trim()).unwrap();
        let text = String::from_utf8(plain).unwrap();
        assert!(
            text.contains("\"kty\":\"RSA\""),
            "{}",
            &text[..text.len().min(80)]
        );
    }
}
