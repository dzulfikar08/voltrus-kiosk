# Voltrus Kiosk

A face attendance kiosk for [Voltrus ERP](https://voltrus.id). It is a native
desktop app written in Rust with [GPUI](https://gpui.rs) and
[gpui-kit](https://gpui-kit.com). Put it on a mini PC with a webcam at the gate:

- **Recognition runs on the kiosk.** It needs no network to recognise a face.
  It uses three ONNX models on ONNX Runtime and matches against a copy of the
  employees' face templates pulled from the Voltrus server.
- **Check-ins are signed and queued.** Each one is saved locally and pushed to
  the server signed with the device's Ed25519 key. While the server can't be
  reached, the queue grows; it is pushed when the server answers again.
- **It updates itself** from this repository's signed GitHub Releases.

Licensed under the [GNU AGPL v3](LICENSE) or later.

## Face models

All three models are licensed for commercial use. The kiosk downloads them
from the Voltrus server it connects to (`<server>/models/`) and checks each
against a SHA-256 pinned in `src/face/models.rs`.

| File | Model | Licence |
|---|---|---|
| `face_det.onnx` | YuNet 2026may face detection (OpenCV Zoo) | MIT |
| `face_rec.onnx` | SFace 2021dec int8 face recognition, 128-dim (OpenCV Zoo) | Apache-2.0 |
| `antispoof.onnx` | MiniFASNet-V2 anti-spoofing (minivision Silent-Face) | Apache-2.0 |

The preprocessing is a line-for-line port of the Voltrus console's browser
engine. The detector input is downscaled to a long side of at most 640 and
padded to a multiple of 32. Faces are aligned with a 5-point similarity
transform onto the SFace template. Embeddings are L2-normalised and tagged
`sface-2021dec-int8`.

On 40 LFW photos the kiosk's embeddings agree with OpenCV's
`FaceRecognizerSF` at cosine ≥ 0.986 (mean 0.993).

## Install and register

1. **Download** the binary for your platform from
   [Releases](https://github.com/dzulfikar08/voltrus-kiosk/releases/latest).
   - Put it in a folder the kiosk's user can write to, because updates replace
     it in place.
   - Linux needs glibc 2.39+ (Ubuntu 24.04, Debian 13 or later).
   - The Windows and macOS builds are not code-signed.
2. **Run it.** The first start opens **Setup**:
   - Enter the server address (e.g. `https://erp.company.com`).
   - Pick the camera number (0 is the first webcam) and the language
     (Indonesian or English).
3. **Register it in Voltrus.** An OPS user opens **HR → Kiosks → New** and
   enters a name, a location, and the **Device UID** and **public key** shown in
   Setup (both have a Copy button).
4. **Save and connect.** The kiosk downloads the models, pulls the employees and
   faces, and switches to the kiosk screen. Employees enrol their faces in the
   Voltrus console, and the kiosk pulls new faces every 5 minutes.

## Using it

- **Modes:**
  - **Auto** (default): a person's first scan of the day on this kiosk checks
    them in; later scans check them out.
  - **Masuk/In** and **Pulang/Out** force one direction, for separate entry and
    exit gates.
- **What counts as a match** (the server's own rule):
  - The best score must reach the company's face threshold and lead the next
    employee by the margin. Both are HR settings.
  - The match must repeat on two frames in a row.
  - The face must pass the anti-spoofing check.
  - The same person within 60 seconds counts as one scan.
- **Shortcuts:** Ctrl+Shift+S opens Setup and F11 toggles full screen. Setup
  has no on-screen button, so passers-by can't open it.
- **Revoking:** a device revoked in HR → Kiosks is refused immediately.

## Updates

The kiosk checks 30 seconds after it starts and then every 6 hours.

1. It downloads `latest.json` and `latest.json.sig` from the newest release.
2. It checks the Ed25519 signature against the public key compiled into
   `src/updater.rs`.
3. If the version is newer, it downloads this platform's binary, checks its
   SHA-256 against the signed manifest, and swaps it over the running
   executable.
4. It restarts once nobody has been at the camera for a minute.

You can also check, restart now, or turn auto-update off in Setup, or run
`voltrus-kiosk update` from a shell.

A binary is installed only if it is listed in a manifest signed with the
release key. Control of the GitHub account alone is therefore not enough to
push code to kiosks.

### Making a release

1. Bump `version` in `Cargo.toml`.
2. Add a `## [X.Y.Z]` section to `CHANGELOG.md`.
3. Tag and push:

   ```
   git tag -a vX.Y.Z -m "Voltrus Kiosk X.Y.Z" && git push origin vX.Y.Z
   ```

The [release workflow](.github/workflows/release.yml) then:

- builds Linux x86_64 and aarch64, Windows x64, and macOS universal;
- writes `SHA256SUMS` and `latest.json`;
- signs the manifest with the `UPDATE_SIGNING_KEY` secret, an Ed25519 private
  key in PEM form;
- publishes the release.

Installed kiosks pick it up within 6 hours.

## Files

Everything is kept in `$VOLTRUS_KIOSK_HOME`, else in the platform's data folder:
`~/.local/share/voltrus-kiosk`, `%APPDATA%\voltrus-kiosk`, or
`~/Library/Application Support/voltrus-kiosk`.

| File | What |
|---|---|
| `config.json` | server address, language, camera, auto-update |
| `identity.json` | Device UID and the Ed25519 secret key (mode 0600; back it up, or register the device again) |
| `gallery.json` | last pull: employees, face templates, thresholds |
| `queue.json` | check-ins the server has not accepted yet |
| `log.json` | recent check-ins on screen; Auto mode reads it |
| `models/` | the three ONNX models |

## Command line

```
voltrus-kiosk --version
voltrus-kiosk identity             # print the Device UID and public key
voltrus-kiosk update               # install the newest release now
voltrus-kiosk check PHOTO [in|out] # pull, identify a photo, optionally send a check-in
voltrus-kiosk embed PHOTO          # embedding of the largest face, as JSON
VOLTRUS_KIOSK_FAKE_CAMERA=a.jpg:b.jpg voltrus-kiosk   # photos instead of a webcam
```

`check` tests an installation end to end without a camera.

## Building

```
cargo build --release
cargo test
```

- ONNX Runtime is downloaded and linked at build time (the `ort` crate).
- Linux also needs GPUI's libraries and clang; `scripts/ci-deps.sh` installs
  them on Ubuntu.

## Server protocol

The protocol is defined by Voltrus ERP. Each request carries four headers:

| Header | Value |
|---|---|
| `X-Device-Uid` | the Device UID |
| `X-Device-Ts` | unix seconds |
| `X-Device-Nonce` | a single-use random value |
| `X-Device-Sig` | hex Ed25519 signature of `{uid}.{ts}.{nonce}.{bodyHash}` |

`bodyHash` is base64(SHA-256(body)), or empty for a GET.

| Endpoint | Purpose |
|---|---|
| `GET /api/hr/kiosk/pull` | employees, face templates (`model` must be `sface-2021dec-int8`), threshold and margin |
| `POST /api/hr/kiosk/push` | `{entries: [{local_id, payload: {employee_id, event_type: in\|out, ts, confidence}}]}`; each entry is answered `applied`, `duplicate`, `rejected` or `retry` |
