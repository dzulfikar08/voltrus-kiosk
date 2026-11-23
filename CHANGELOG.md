# Changelog

All notable changes to Voltrus Kiosk. Versions follow [Semantic Versioning](https://semver.org).

## [0.3.1] - 2026-11-23

- Setup has a close button (✕) in its corner, and Esc leaves Setup, the PIN pad and enrolment.
- Setup says why **Enrol a face** is greyed out: the kiosk has no admin PIN yet.
- The green gradient in Setup, the result card's tint and the camera image no longer spill past their rounded corners.

## [0.3.0] - 2026-11-23

Enrol faces at the kiosk.

- **Enrol a face** from the kiosk: pick the employee, they read and accept the consent notice on screen (UU PDP), and the kiosk takes five samples while asking them to turn their head a little. The samples go straight to Voltrus and every kiosk recognises the person from its next pull; this one at once. Needs Voltrus ERP with db/0104.
- **Admin PIN:** HR sets one per kiosk in HR → Kiosks. The kiosk then asks for it before opening its settings or enrolling, and the server checks it again for every enrolment (five wrong PINs lock enrolment for 15 minutes).
- A settings button on the kiosk screen. Ctrl+Shift+S still works.
- macOS: a proper menu bar with Settings (⌘,), Hide (⌘H), Full Screen (⌃⌘F) and Quit (⌘Q). Closing the window quits on every platform; Ctrl+Q quits on Windows and Linux.
- The Voltrus icon on the app: in the macOS app bundle and the Windows .exe.
- Releases include `voltrus-kiosk-X-universal-macos-app.zip`, a ready-to-run **Voltrus Kiosk.app** for first installs on a Mac.

## [0.2.1] - 2026-11-23

Uses much less CPU.

- ONNX Runtime no longer busy-waits between frames.
- The camera decodes 15 frames a second instead of 30.
- With nobody in front of the camera, or right after a check-in, the kiosk checks about twice a second instead of seven times.

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
