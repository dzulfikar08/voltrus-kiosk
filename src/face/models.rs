//! Model files: the same three the console serves at /models (pinned by
//! SHA-256 here and in the console's model download script). The kiosk downloads
//! them from its server on first start and checks every byte.

use anyhow::{Context as _, Result, bail};
use sha2::{Digest, Sha256};
use std::io::Read as _;
use std::path::Path;

/// Bump with the console's MODEL_VERSION (faceEngine.ts) so caches agree.
pub const MODEL_VERSION: &str = "3";

pub const MODELS: [(&str, &str); 3] = [
    (
        "face_det.onnx",
        "ebafce4e3c118d6554634be5c27ab333b4c047a9a8c3faf1d7cf93101c22f0f0",
    ),
    (
        "face_rec.onnx",
        "2b0e941e6f16cc048c20aee0c8e31f569118f65d702914540f7bfdc14048d78a",
    ),
    (
        "antispoof.onnx",
        "d7b3cd9ba8a7ceb13baa8c4720902e27ca3112eff52f926c08804af6b6eecc7b",
    ),
];

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn is_valid(path: &Path, sum: &str) -> bool {
    std::fs::read(path)
        .map(|b| sha256_hex(&b) == sum)
        .unwrap_or(false)
}

/// Fetch any missing or altered model from `<server>/models/<file>`.
pub fn ensure(dir: &Path, server: &str) -> Result<()> {
    std::fs::create_dir_all(dir)?;
    for (file, sum) in MODELS {
        let path = dir.join(file);
        if is_valid(&path, sum) {
            continue;
        }
        let url = format!(
            "{}/models/{file}?v={MODEL_VERSION}",
            server.trim_end_matches('/')
        );
        let response = ureq::get(&url)
            .call()
            .with_context(|| format!("downloading {url}"))?;
        let mut bytes = Vec::new();
        response
            .into_reader()
            .take(64 * 1024 * 1024)
            .read_to_end(&mut bytes)?;
        if sha256_hex(&bytes) != sum {
            bail!("{file} from {url} does not match its pinned SHA-256");
        }
        let part = path.with_extension("part");
        std::fs::write(&part, &bytes)?;
        std::fs::rename(&part, &path)?;
    }
    Ok(())
}
