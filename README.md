# Voltrus Kiosk

The native face attendance kiosk for Voltrus ERP (M17, US-192). It is a Rust app
built with GPUI and gpui-kit. Put it on a mini PC with a webcam at the gate:

- **Recognition is offline.** It runs the console's three face models locally
  (YuNet detection, SFace int8 recognition, MiniFASNet-V2 anti-spoofing; see
  `../tasks/face-models.md`) against a copy of the face gallery pulled from the server.
- **Check-ins are queued.** Each check-in is stored locally and pushed to
  `/api/hr/kiosk/*`, signed with the device's Ed25519 key. Without network the
  queue grows and is pushed when the server answers again.

## Install and register

1. Build: `cargo build --release`. The binary is `target/release/voltrus-kiosk`.
   - ONNX Runtime is downloaded and linked at build time (the `ort` crate).
   - Linux needs the V4L2 headers and clang for the camera crate.
2. Run it. The first start opens **Setup**:
   - Enter the server address (e.g. `https://erp.company.com`).
   - Pick the camera number (0 is the first webcam) and the language (Indonesian or English).
3. In Voltrus, an OPS user opens **HR → Kiosks → New**. They enter a name, a
   location, and the **Device UID** and **public key** shown in Setup (both
   have a Copy button).
4. Press **Save and connect**. The kiosk downloads the three models from
   `<server>/models/`, checks each against its pinned SHA-256, pulls the
   employees and faces, and switches to the kiosk screen.

Employees enrol their face in the console (HR → Face, after consent). The
kiosk pulls new faces every 5 minutes.

## Using it

- **Auto** (default): a person's first scan of the day on this kiosk checks
  them in and later scans check them out. **Masuk/In** and **Pulang/Out** force
  one direction, for gates with separate entry and exit kiosks.
- **Matching rule:** the same as the server's (`src/hr/face.ts` identify).
  - The best score must reach `face_threshold` and lead the next employee by
    `face_margin`; both are HR settings.
  - The match must repeat on two frames in a row.
  - The face must pass the anti-spoofing check.
  - The same person within 60 seconds counts as one scan.
- **Shortcuts:** Ctrl+Shift+S opens Setup and F11 toggles full screen. There is
  no on-screen button for Setup, so passers-by can't open it.
- **Revoking:** a revoked device (HR → Kiosks → revoke) is refused at once.
  Its queue stays on the disk.

## Files

Everything is kept in `$VOLTRUS_KIOSK_HOME`, else the platform data folder
(`~/.local/share/voltrus-kiosk`, `%APPDATA%\voltrus-kiosk`,
`~/Library/Application Support/voltrus-kiosk`):

| File | What |
|---|---|
| `config.json` | server address, language, camera |
| `identity.json` | Device UID and the Ed25519 secret key (mode 0600; back it up or re-register) |
| `gallery.json` | last pull: employees, face templates, thresholds |
| `queue.json` | check-ins the server has not accepted yet |
| `log.json` | recent check-ins on screen; also what Auto uses |
| `models/` | the three ONNX models |

## Command line

```
voltrus-kiosk identity             # print the Device UID and public key
voltrus-kiosk check PHOTO [in|out] # pull, identify a photo, optionally send a check-in
voltrus-kiosk embed PHOTO          # embedding of the largest face, as JSON
VOLTRUS_KIOSK_FAKE_CAMERA=a.jpg:b.jpg voltrus-kiosk   # photos instead of a webcam
```

`check` tests an installation end to end without a camera.

## Checked

- **Unit tests** (`cargo test`): face math, alignment, NMS, crop, matching
  rule, body hash.
- **Against OpenCV** (`FaceDetectorYN` + `FaceRecognizerSF.alignCrop`/`feature`)
  on 40 LFW photos: the kiosk's embeddings agree at cosine ≥ 0.986 (mean 0.993).
- **Against a local worker:**
  - An unregistered device gets 401.
  - A registered one pulls, matches the right employee, and its check-ins arrive as `source: kiosk`.
  - Check-ins made while the server was down were queued and pushed when it came back.
- **Platforms:** built and run on Linux (Wayland) only. Windows (Media
  Foundation) and macOS (AVFoundation, camera permission prompt) have not been
  tried yet.
