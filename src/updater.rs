//! In-app updates from GitHub Releases.
//!
//! Every release carries `latest.json` and `latest.json.sig`. The manifest
//! names each platform's binary with its SHA-256. The signature is Ed25519
//! over the manifest's exact bytes, made in the release workflow with the
//! `UPDATE_SIGNING_KEY` secret. The kiosk downloads
//! `releases/latest/download/latest.json`, checks the signature against
//! [`UPDATE_PUBLIC_KEY`], and if the version is newer it downloads its binary,
//! checks the hash, and swaps it over the running one. The new version starts
//! the next time nobody is in front of the camera.
//!
//! A compromised GitHub account alone cannot push an update: the binary must
//! be listed in a manifest signed with the release key.

use anyhow::{Context as _, Result, anyhow, bail};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as B64;
use ed25519_dalek::{Signature, Verifier as _, VerifyingKey};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const REPO: &str = "dzulfikar08/voltrus-kiosk";
/// Ed25519 public key (raw 32 bytes, base64) matching the release workflow's
/// UPDATE_SIGNING_KEY.
pub const UPDATE_PUBLIC_KEY: &str = "5LMc4SHW/gudzbsk47/K92PndTQAgsLxFTzx/JnW6kI=";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Release asset key for this build.
pub fn target() -> &'static str {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => "x86_64-linux",
        ("linux", "aarch64") => "aarch64-linux",
        ("windows", "x86_64") => "x86_64-windows",
        ("macos", _) => "universal-macos",
        _ => "unsupported",
    }
}

#[derive(Debug, Deserialize)]
pub struct Asset {
    pub name: String,
    pub sha256: String,
}

#[derive(Debug, Deserialize)]
pub struct Manifest {
    pub version: String,
    pub assets: HashMap<String, Asset>,
}

pub fn parse_semver(v: &str) -> Option<(u64, u64, u64)> {
    let core = v.trim().trim_start_matches('v').split(['-', '+']).next()?;
    let mut it = core.split('.').map(|p| p.parse::<u64>().ok());
    Some((it.next()??, it.next()??, it.next()??))
}

pub fn is_newer(candidate: &str, current: &str) -> bool {
    matches!((parse_semver(candidate), parse_semver(current)), (Some(a), Some(b)) if a > b)
}

/// Check `sig_b64` (base64 Ed25519 signature) over `bytes` with `key_b64`.
pub fn verify(bytes: &[u8], sig_b64: &str, key_b64: &str) -> Result<()> {
    let key: [u8; 32] = B64
        .decode(key_b64.trim())?
        .try_into()
        .map_err(|_| anyhow!("bad update public key"))?;
    let sig: [u8; 64] = B64
        .decode(sig_b64.trim())?
        .try_into()
        .map_err(|_| anyhow!("bad manifest signature length"))?;
    VerifyingKey::from_bytes(&key)?
        .verify(bytes, &Signature::from_bytes(&sig))
        .map_err(|_| anyhow!("update manifest signature does not verify"))
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout(Duration::from_secs(300))
        .user_agent(&format!("voltrus-kiosk/{VERSION}"))
        .build()
}

fn get(agent: &ureq::Agent, url: &str, limit: u64) -> Result<Vec<u8>> {
    let response = agent
        .get(url)
        .call()
        .with_context(|| format!("GET {url}"))?;
    let mut bytes = Vec::new();
    response.into_reader().take(limit).read_to_end(&mut bytes)?;
    Ok(bytes)
}

/// The newest signed manifest, if it is newer than this build.
pub fn check() -> Result<Option<Manifest>> {
    let agent = agent();
    let base = format!("https://github.com/{REPO}/releases/latest/download");
    let manifest = get(&agent, &format!("{base}/latest.json"), 1 << 20)?;
    let sig = String::from_utf8(get(&agent, &format!("{base}/latest.json.sig"), 4096)?)?;
    verify(&manifest, &sig, UPDATE_PUBLIC_KEY)?;
    let manifest: Manifest = serde_json::from_slice(&manifest).context("update manifest")?;
    Ok(is_newer(&manifest.version, VERSION).then_some(manifest))
}

/// Download this platform's binary from `manifest`, check its hash and put
/// it in place of the running executable. Returns the installed path.
pub fn install(manifest: &Manifest) -> Result<PathBuf> {
    let asset = manifest
        .assets
        .get(target())
        .ok_or_else(|| anyhow!("release {} has no build for {}", manifest.version, target()))?;
    let url = format!(
        "https://github.com/{REPO}/releases/download/v{}/{}",
        manifest.version, asset.name
    );
    let bytes = get(&agent(), &url, 512 << 20)?;
    if hex::encode(Sha256::digest(&bytes)) != asset.sha256.to_lowercase() {
        bail!(
            "{} does not match the SHA-256 in the signed manifest",
            asset.name
        );
    }
    let current = std::env::current_exe()?.canonicalize()?;
    replace(&current, &bytes)?;
    Ok(current)
}

/// Write beside the target (same filesystem, so the rename is atomic), then
/// swap. Unix renames over the running file; Windows moves the running .exe
/// aside first (it may be renamed but not overwritten).
fn replace(current: &Path, bytes: &[u8]) -> Result<()> {
    let dir = current
        .parent()
        .ok_or_else(|| anyhow!("executable has no folder"))?;
    let staged = dir.join(".voltrus-kiosk.new");
    std::fs::write(&staged, bytes).with_context(|| {
        format!(
            "cannot write to {}: install the kiosk in a folder it may write to",
            dir.display()
        )
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&staged, std::fs::Permissions::from_mode(0o755))?;
    }
    #[cfg(windows)]
    {
        let old = current.with_extension("old.exe");
        let _ = std::fs::remove_file(&old);
        std::fs::rename(current, &old)?;
    }
    std::fs::rename(&staged, current)?;
    Ok(())
}

/// Start the (new) executable with the same arguments and leave.
pub fn restart() -> ! {
    let exe = std::env::current_exe().expect("current exe");
    let args: Vec<String> = std::env::args().skip(1).collect();
    let _ = std::process::Command::new(exe).args(args).spawn();
    std::process::exit(0);
}

/// Leftover from a Windows update.
pub fn clean_up() {
    if let Ok(exe) = std::env::current_exe() {
        let _ = std::fs::remove_file(exe.with_extension("old.exe"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer as _, SigningKey};

    #[test]
    fn versions_compare_numerically() {
        assert!(is_newer("0.10.0", "0.9.9"));
        assert!(is_newer("v1.0.0", "0.99.0"));
        assert!(!is_newer("0.1.0", "0.1.0"));
        assert!(!is_newer("garbage", "0.1.0"));
    }

    #[test]
    fn signatures_are_checked() {
        let key = SigningKey::from_bytes(&[7u8; 32]);
        let public = B64.encode(key.verifying_key().to_bytes());
        let body = br#"{"version":"9.9.9","assets":{}}"#;
        let sig = B64.encode(key.sign(body).to_bytes());
        verify(body, &sig, &public).unwrap();
        assert!(verify(br#"{"version":"9.9.8","assets":{}}"#, &sig, &public).is_err());
    }

    /// Signed with the real release key by `openssl pkeyutl -sign -rawin`,
    /// exactly as the release workflow signs latest.json.
    #[test]
    fn openssl_signatures_verify_against_the_embedded_key() {
        verify(b"voltrus-kiosk update key check", "bPGMW9rqVxgXkxVoRFdTU03QZO8cbu9ugsU38azGjNWBfGzYWGTm0KpFp0dnxFnWMJiPUruqIqzwXtRUXGqeAA==", UPDATE_PUBLIC_KEY).unwrap();
    }

    #[test]
    fn the_embedded_key_parses() {
        let key: [u8; 32] = B64.decode(UPDATE_PUBLIC_KEY).unwrap().try_into().unwrap();
        VerifyingKey::from_bytes(&key).unwrap();
    }
}
