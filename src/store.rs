//! Everything the kiosk keeps on disk, in its home directory
//! (`$VOLTRUS_KIOSK_HOME`, else the platform data dir + `voltrus-kiosk`):
//!   config.json    server URL, language, camera
//!   identity.json  device_uid and the Ed25519 secret key (0600)
//!   gallery.json   the last pull: employees, face templates, thresholds
//!   queue.json     recognised events not yet accepted by the server
//!   log.json       the last events shown on screen (auto in/out uses it)
//!   models/        the three ONNX models

use anyhow::{Context as _, Result};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as B64;
use ed25519_dalek::SigningKey;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::path::{Path, PathBuf};

pub fn home() -> PathBuf {
    if let Some(dir) = std::env::var_os("VOLTRUS_KIOSK_HOME") {
        return PathBuf::from(dir);
    }
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("voltrus-kiosk")
}

fn read_json<T: DeserializeOwned>(path: &Path) -> Option<T> {
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}

/// Write through a temp file and rename, so a power cut never leaves half a file.
fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(value)?)?;
    std::fs::rename(&tmp, path).with_context(|| format!("writing {}", path.display()))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    #[default]
    Id,
    En,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub server_url: String,
    #[serde(default)]
    pub lang: Lang,
    #[serde(default)]
    pub camera_index: u32,
    /// Install new releases by itself (on by default).
    #[serde(default = "yes")]
    pub auto_update: bool,
}

fn yes() -> bool {
    true
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server_url: String::new(),
            lang: Lang::Id,
            camera_index: 0,
            auto_update: true,
        }
    }
}

impl Config {
    pub fn load() -> Self {
        read_json(&home().join("config.json")).unwrap_or_default()
    }
    pub fn save(&self) -> Result<()> {
        write_json(&home().join("config.json"), self)
    }
    pub fn is_configured(&self) -> bool {
        !self.server_url.trim().is_empty()
    }
}

#[derive(Serialize, Deserialize)]
struct IdentityFile {
    device_uid: String,
    secret_key: String,
}

/// The device's identity: generated once, registered in HR → Kiosks.
#[derive(Clone)]
pub struct Identity {
    pub device_uid: String,
    pub key: SigningKey,
}

impl Identity {
    pub fn load_or_create() -> Result<Self> {
        let path = home().join("identity.json");
        if let Some(file) = read_json::<IdentityFile>(&path) {
            let bytes: [u8; 32] = B64
                .decode(&file.secret_key)?
                .try_into()
                .map_err(|_| anyhow::anyhow!("identity.json: bad key length"))?;
            return Ok(Self {
                device_uid: file.device_uid,
                key: SigningKey::from_bytes(&bytes),
            });
        }
        let key = SigningKey::generate(&mut rand::rngs::OsRng);
        let mut uid = [0u8; 6];
        rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut uid);
        let identity = Self {
            device_uid: format!("vk-{}", hex::encode(uid)),
            key,
        };
        write_json(
            &path,
            &IdentityFile {
                device_uid: identity.device_uid.clone(),
                secret_key: B64.encode(identity.key.to_bytes()),
            },
        )?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
        }
        Ok(identity)
    }

    /// The public key as the server stores it: base64 of the 32 raw bytes.
    pub fn public_key_b64(&self) -> String {
        B64.encode(self.key.verifying_key().to_bytes())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Employee {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub position: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Face {
    pub employee_id: String,
    pub embedding: Vec<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullSettings {
    pub face_model: String,
    pub face_threshold: f32,
    pub face_margin: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DeviceInfo {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    /// hex(sha256("voltrus-kiosk-pin:{id}:{pin}")) when HR set an admin PIN.
    #[serde(default)]
    pub admin_pin_hash: Option<String>,
    #[serde(default)]
    pub location: Option<String>,
}

/// GET /api/hr/kiosk/pull, kept as the offline gallery.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Gallery {
    pub server_time: String,
    #[serde(default)]
    pub device: DeviceInfo,
    #[serde(default)]
    pub company: Option<String>,
    pub employees: Vec<Employee>,
    pub faces: Vec<Face>,
    pub settings: PullSettings,
}

impl Gallery {
    pub fn load() -> Option<Self> {
        read_json(&home().join("gallery.json"))
    }
    pub fn save(&self) -> Result<()> {
        write_json(&home().join("gallery.json"), self)
    }
    /// Whether `pin` is this kiosk's admin PIN (true when none is set).
    pub fn pin_matches(&self, pin: &str) -> bool {
        match (&self.device.id, &self.device.admin_pin_hash) {
            (_, None) => true,
            (Some(id), Some(hash)) => {
                use sha2::{Digest, Sha256};
                let digest = Sha256::digest(format!("voltrus-kiosk-pin:{id}:{pin}").as_bytes());
                hex::encode(digest).eq_ignore_ascii_case(hash)
            }
            (None, Some(_)) => false,
        }
    }

    pub fn templates_of(&self, employee_id: &str) -> usize {
        self.faces
            .iter()
            .filter(|f| f.employee_id == employee_id)
            .count()
    }

    pub fn name_of(&self, employee_id: &str) -> String {
        self.employees
            .iter()
            .find(|e| e.id == employee_id)
            .map(|e| e.name.clone())
            .unwrap_or_else(|| employee_id.to_string())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EventType {
    In,
    Out,
}

/// The body the server's applyEvent reads.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Payload {
    pub employee_id: String,
    pub event_type: EventType,
    pub ts: String,
    pub confidence: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EventStatus {
    Queued,
    Applied,
    Duplicate,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Event {
    pub local_id: String,
    pub name: String,
    pub payload: Payload,
    pub status: EventStatus,
    #[serde(default)]
    pub error: Option<String>,
}

const LOG_KEEP: usize = 300;

/// The offline queue and the on-screen log. Both are small (a gate's day),
/// so plain JSON files rewritten on change are enough.
pub struct Events {
    pub queue: Vec<Event>,
    pub log: Vec<Event>,
}

impl Events {
    pub fn load() -> Self {
        Self {
            queue: read_json(&home().join("queue.json")).unwrap_or_default(),
            log: read_json(&home().join("log.json")).unwrap_or_default(),
        }
    }

    fn save(&self) -> Result<()> {
        write_json(&home().join("queue.json"), &self.queue)?;
        write_json(&home().join("log.json"), &self.log)
    }

    pub fn record(&mut self, event: Event) -> Result<()> {
        self.queue.push(event.clone());
        self.log.push(event);
        if self.log.len() > LOG_KEEP {
            let drop = self.log.len() - LOG_KEEP;
            self.log.drain(..drop);
        }
        self.save()
    }

    /// Server answers for queued entries: accepted ones leave the queue,
    /// `retry` ones stay.
    pub fn settle(&mut self, results: &[(String, EventStatus, Option<String>)]) -> Result<()> {
        for (local_id, status, error) in results {
            if *status == EventStatus::Queued {
                continue;
            }
            self.queue.retain(|e| &e.local_id != local_id);
            if let Some(e) = self.log.iter_mut().find(|e| &e.local_id == local_id) {
                e.status = *status;
                e.error = error.clone();
            }
        }
        self.save()
    }

    /// The employee's last event on this kiosk since `since` (ISO, UTC).
    pub fn last_for(&self, employee_id: &str, since: &str) -> Option<&Event> {
        self.log
            .iter()
            .rev()
            .find(|e| e.payload.employee_id == employee_id && e.payload.ts.as_str() >= since)
    }
}

#[cfg(test)]
mod pin_tests {
    use super::*;

    #[test]
    fn pin_hash_matches_the_server() {
        let mut g: Gallery = serde_json::from_str(
            r#"{"server_time":"","employees":[],"faces":[],"settings":{"face_model":"m","face_threshold":0.45,"face_margin":0.08}}"#,
        )
        .unwrap();
        assert!(g.pin_matches("anything"), "no PIN set");
        g.device.id = Some("dev-1".into());
        // hex(sha256("voltrus-kiosk-pin:dev-1:482913"))
        g.device.admin_pin_hash =
            Some("ba00e1167dd7268176b5cc7868d6a6816be6fe926232f84bdc1038e4e8fd5975".into());
        assert!(g.pin_matches("482913"));
        assert!(!g.pin_matches("000000"));
    }
}
