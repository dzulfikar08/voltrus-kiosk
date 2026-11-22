# Changelog

All notable changes to Voltrus Kiosk. Versions follow [Semantic Versioning](https://semver.org).

## [0.1.0] - 2026-11-23

First release.

- Face attendance kiosk for Voltrus ERP:
  - Recognition runs offline with YuNet (detection), SFace int8 (recognition) and MiniFASNet-V2 (anti-spoofing) on ONNX Runtime.
  - Check-ins are signed with the device's Ed25519 key and queued while the server is unreachable.
- Setup screen with the Device UID and public key to register under HR → Kiosks.
- Models are downloaded from the Voltrus server and checked against pinned SHA-256 hashes.
- Auto, in and out modes. Indonesian and English.
- Updates install by themselves from signed GitHub Releases and take effect when nobody is at the camera.
- Command line: `identity`, `check PHOTO [in|out]`, `embed PHOTO`, `update`.
