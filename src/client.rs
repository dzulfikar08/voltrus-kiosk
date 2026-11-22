//! Signed requests to /api/hr/kiosk/* (src/hr/kiosk.ts). The kiosk signs
//!   `{deviceUid}.{ts}.{nonce}.{bodyHash}`
//! with its Ed25519 key, where bodyHash is base64(sha256(body)) or "" for an
//! empty body, and sends X-Device-Uid, X-Device-Ts (unix seconds),
//! X-Device-Nonce and X-Device-Sig (hex).

use crate::store::{Event, EventStatus, Gallery, Identity};
use anyhow::{Result, anyhow, bail};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as B64;
use ed25519_dalek::Signer as _;
use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Duration;

pub fn body_hash(body: &str) -> String {
    if body.is_empty() {
        String::new()
    } else {
        B64.encode(Sha256::digest(body.as_bytes()))
    }
}

pub fn signing_message(device_uid: &str, ts: &str, nonce: &str, body_hash: &str) -> String {
    format!("{device_uid}.{ts}.{nonce}.{body_hash}")
}

pub struct Client {
    base: parking_lot::Mutex<String>,
    identity: Identity,
    agent: ureq::Agent,
    /// Server clock minus ours, seconds, learnt from response Date headers,
    /// so a kiosk with a drifting clock still signs within the 5-minute window.
    skew: AtomicI64,
}

#[derive(Debug)]
pub enum CallError {
    /// No answer: offline, DNS, TLS, timeout. Keep the queue.
    Offline(String),
    /// The server answered with an error.
    Server(u16, String),
}

impl std::fmt::Display for CallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CallError::Offline(m) => write!(f, "offline: {m}"),
            CallError::Server(code, m) => write!(f, "{m} ({code})"),
        }
    }
}

impl std::error::Error for CallError {}

impl Client {
    pub fn new(base: &str, identity: Identity) -> Self {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(8))
            .timeout(Duration::from_secs(30))
            .build();
        Self {
            base: parking_lot::Mutex::new(base.trim_end_matches('/').to_string()),
            identity,
            agent,
            skew: AtomicI64::new(0),
        }
    }

    pub fn set_base(&self, base: &str) {
        *self.base.lock() = base.trim_end_matches('/').to_string();
    }

    pub fn base(&self) -> String {
        self.base.lock().clone()
    }

    /// Server time now (unix seconds), by our clock plus the learnt skew.
    pub fn now(&self) -> i64 {
        chrono::Utc::now().timestamp() + self.skew.load(Ordering::Relaxed)
    }

    pub fn now_iso(&self) -> String {
        chrono::DateTime::from_timestamp(self.now(), 0)
            .unwrap_or_else(chrono::Utc::now)
            .to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
    }

    fn learn_skew(&self, response: &ureq::Response) {
        if let Some(date) = response
            .header("date")
            .and_then(|d| chrono::DateTime::parse_from_rfc2822(d).ok())
        {
            self.skew.store(
                date.timestamp() - chrono::Utc::now().timestamp(),
                Ordering::Relaxed,
            );
        }
    }

    #[allow(clippy::result_large_err)] // ureq's own error type
    fn call_once(
        &self,
        method: &str,
        path: &str,
        body: &str,
    ) -> Result<ureq::Response, ureq::Error> {
        let ts = self.now().to_string();
        let mut nonce = [0u8; 16];
        rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut nonce);
        let nonce = hex::encode(nonce);
        let message = signing_message(&self.identity.device_uid, &ts, &nonce, &body_hash(body));
        let sig = hex::encode(self.identity.key.sign(message.as_bytes()).to_bytes());
        let request = self
            .agent
            .request(method, &format!("{}{path}", self.base()))
            .set("x-device-uid", &self.identity.device_uid)
            .set("x-device-ts", &ts)
            .set("x-device-nonce", &nonce)
            .set("x-device-sig", &sig);
        if body.is_empty() {
            request.call()
        } else {
            request
                .set("content-type", "application/json")
                .send_string(body)
        }
    }

    fn call(&self, method: &str, path: &str, body: &str) -> Result<serde_json::Value, CallError> {
        let mut retried = false;
        loop {
            match self.call_once(method, path, body) {
                Ok(response) => {
                    self.learn_skew(&response);
                    return response
                        .into_json()
                        .map_err(|e| CallError::Offline(e.to_string()));
                }
                Err(ureq::Error::Status(code, response)) => {
                    let before = self.skew.load(Ordering::Relaxed);
                    self.learn_skew(&response);
                    let message = response
                        .into_json::<serde_json::Value>()
                        .ok()
                        .and_then(|v| v.get("error").and_then(|e| e.as_str()).map(str::to_string))
                        .unwrap_or_else(|| format!("HTTP {code}"));
                    // A clock rejection with a newly learnt skew: sign again once.
                    if code == 401
                        && !retried
                        && self.skew.load(Ordering::Relaxed) != before
                        && message.contains("clock")
                    {
                        retried = true;
                        continue;
                    }
                    return Err(CallError::Server(code, message));
                }
                Err(e) => return Err(CallError::Offline(e.to_string())),
            }
        }
    }

    pub fn pull(&self) -> Result<Gallery> {
        let value = self.call("GET", "/api/hr/kiosk/pull", "")?;
        Ok(serde_json::from_value(value)?)
    }

    /// Push queued events; returns (local_id, status, error) per entry.
    /// `retry` answers come back as Queued.
    pub fn push(&self, events: &[Event]) -> Result<Vec<(String, EventStatus, Option<String>)>> {
        if events.is_empty() {
            return Ok(Vec::new());
        }
        let entries: Vec<_> = events
            .iter()
            .map(|e| serde_json::json!({ "local_id": e.local_id, "payload": e.payload }))
            .collect();
        let body = serde_json::to_string(&serde_json::json!({ "entries": entries }))?;
        let value = self.call("POST", "/api/hr/kiosk/push", &body)?;
        let results = value
            .get("results")
            .and_then(|r| r.as_array())
            .ok_or_else(|| anyhow!("push answer has no results"))?;
        let mut out = Vec::new();
        for r in results {
            let Some(local_id) = r.get("local_id").and_then(|v| v.as_str()) else {
                continue;
            };
            let status = match r.get("status").and_then(|v| v.as_str()) {
                Some("applied") => EventStatus::Applied,
                Some("duplicate") => EventStatus::Duplicate,
                Some("rejected") => EventStatus::Rejected,
                _ => EventStatus::Queued,
            };
            out.push((
                local_id.to_string(),
                status,
                r.get("error").and_then(|v| v.as_str()).map(str::to_string),
            ));
        }
        if out.is_empty() {
            bail!("push answer had no usable results");
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn body_hash_matches_the_server() {
        assert_eq!(body_hash(""), "");
        // base64(sha256("{}")), as crypto.subtle computes it in src/hr/kiosk.ts.
        assert_eq!(
            body_hash("{}"),
            "RBNvo1WzZ4oRRq0W9+hknpT7T8If536DEMBg9hyq/4o="
        );
    }

    #[test]
    fn message_layout() {
        assert_eq!(signing_message("vk-1", "100", "n", "h"), "vk-1.100.n.h");
    }
}
