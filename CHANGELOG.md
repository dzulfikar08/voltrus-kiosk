# Changelog

All notable changes to Voltrus Kiosk. Versions follow [Semantic Versioning](https://semver.org).

## [0.2.0] - 2026-11-23

A redesigned look.

- New dark theme in the Voltrus colours, with the Voltrus mark, a larger clock and the date written out.
- The camera fills its frame. Rounded corner marks follow the face: white while scanning, green on a match, red for a photo or screen.
- The result card shows the person's initials, name, position and whether they checked in or out.
- The card now tells apart: no faces enrolled yet, recognising, face not recognised, and already recorded.
- Recent check-ins show initials, direction, time, and whether the server has accepted each one.
- Setup is two columns: the registration steps on the left, the form on the right. Device UID and public key copy with one click. Auto-update is a switch.

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
