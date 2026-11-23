//! Webcam capture on its own thread. Keeps the latest frame for the
//! recognizer and a mirrored BGRA preview (with the face box drawn in) for
//! the window.

use crate::face::math::Rgb;
use crate::state::{Shared, Tone};
use gpui_kit::RenderImage;
use nokhwa::Camera;
use nokhwa::pixel_format::RgbFormat;
use nokhwa::utils::{
    CameraFormat, CameraIndex, FrameFormat, RequestedFormat, RequestedFormatType, Resolution,
};
use std::sync::Arc;
use std::time::Duration;

fn open(index: u32) -> Result<Camera, nokhwa::NokhwaError> {
    let wanted = RequestedFormat::new::<RgbFormat>(RequestedFormatType::Closest(
        CameraFormat::new(Resolution::new(1280, 720), FrameFormat::MJPEG, 30),
    ));
    let mut camera = match Camera::new(CameraIndex::Index(index), wanted) {
        Ok(c) => c,
        Err(_) => Camera::new(
            CameraIndex::Index(index),
            RequestedFormat::new::<RgbFormat>(RequestedFormatType::AbsoluteHighestFrameRate),
        )?,
    };
    camera.open_stream()?;
    Ok(camera)
}

/// `VOLTRUS_KIOSK_FAKE_CAMERA=a.jpg:b.jpg` shows those photos in turn
/// (4 s each) instead of a webcam: for demos and for testing without one.
fn fake_camera(shared: &Shared, paths: &str) -> ! {
    let frames: Vec<Rgb> = paths
        .split(':')
        .filter_map(|p| image::open(p).ok())
        .map(|i| {
            let i = i.to_rgb8();
            Rgb {
                width: i.width(),
                height: i.height(),
                data: i.into_raw(),
            }
        })
        .collect();
    loop {
        if frames.is_empty() {
            shared.set_camera_error(Some(format!(
                "VOLTRUS_KIOSK_FAKE_CAMERA: no readable image in {paths}"
            )));
            std::thread::sleep(Duration::from_secs(5));
            continue;
        }
        for frame in &frames {
            for _ in 0..60 {
                shared.put_frame(Arc::new(frame.clone()), preview(frame, shared.face_box()));
                std::thread::sleep(Duration::from_millis(66));
            }
        }
    }
}

/// Runs for the life of the process; reopens the camera when it fails.
pub fn spawn(shared: Arc<Shared>, index: u32) {
    std::thread::Builder::new()
        .name("camera".into())
        .spawn(move || {
            if let Ok(paths) = std::env::var("VOLTRUS_KIOSK_FAKE_CAMERA") {
                fake_camera(&shared, &paths);
            }
            loop {
                let mut camera = match open(index) {
                    Ok(c) => {
                        shared.set_camera_error(None);
                        c
                    }
                    Err(e) => {
                        shared.set_camera_error(Some(e.to_string()));
                        std::thread::sleep(Duration::from_secs(3));
                        continue;
                    }
                };
                loop {
                    let frame = match camera.frame().and_then(|b| b.decode_image::<RgbFormat>()) {
                        Ok(img) => Rgb {
                            width: img.width(),
                            height: img.height(),
                            data: img.into_raw(),
                        },
                        Err(e) => {
                            shared.set_camera_error(Some(e.to_string()));
                            break;
                        }
                    };
                    let preview = preview(&frame, shared.face_box());
                    shared.put_frame(Arc::new(frame), preview);
                }
                let _ = camera.stop_stream();
                std::thread::sleep(Duration::from_secs(1));
            }
        })
        .expect("camera thread");
}

/// Mirrored (a mirror is what people expect to see), BGRA, at most 960 wide,
/// with corner marks around the last face.
fn preview(frame: &Rgb, face: Option<([f32; 4], Tone)>) -> Arc<RenderImage> {
    let step = frame.width.div_ceil(960).max(1);
    let (w, h) = (frame.width / step, frame.height / step);
    let mut bgra = vec![255u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let sx = (frame.width - 1 - x * step) as usize;
            let s = ((y * step) as usize * frame.width as usize + sx) * 3;
            let d = ((y * w + x) * 4) as usize;
            bgra[d] = frame.data[s + 2];
            bgra[d + 1] = frame.data[s + 1];
            bgra[d + 2] = frame.data[s];
        }
    }
    if let Some((b, tone)) = face {
        // BGR. White while looking, Voltrus green on a match, red for a spoof.
        let color: [u8; 3] = match tone {
            Tone::Neutral => [245, 245, 245],
            Tone::Match => [118, 230, 0],
            Tone::Reject => [104, 84, 255],
        };
        let to = |v: f32| (v / step as f32).round() as i64;
        // Pad the detector box a little so the corners frame the whole face.
        let (bw, bh) = (to(b[2] - b[0]), to(b[3] - b[1]));
        let pad = (bw.max(bh) / 10).max(4);
        let x1 = (w as i64 - 1) - to(b[2]) - pad;
        let x2 = (w as i64 - 1) - to(b[0]) + pad;
        let (y1, y2) = (to(b[1]) - pad, to(b[3]) + pad);
        corners(&mut bgra, w, h, (x1, y1, x2, y2), color);
    }
    let buffer = image::RgbaImage::from_raw(w, h, bgra).expect("preview buffer size");
    Arc::new(RenderImage::new([image::Frame::new(buffer)]))
}

/// Four rounded corner brackets around `(x1, y1, x2, y2)`, drawn as
/// anti-aliased strokes.
fn corners(
    bgra: &mut [u8],
    w: u32,
    h: u32,
    (x1, y1, x2, y2): (i64, i64, i64, i64),
    color: [u8; 3],
) {
    let side = (x2 - x1).min(y2 - y1).max(8) as f32;
    let arm = side * 0.22;
    let radius = side * 0.07;
    let half = (w as f32 / 320.0).max(1.6);
    let (fx1, fy1, fx2, fy2) = (x1 as f32, y1 as f32, x2 as f32, y2 as f32);
    // Distance from (px, py) to a corner bracket opening towards (sx, sy).
    let bracket = |px: f32, py: f32, cx: f32, cy: f32, sx: f32, sy: f32| -> f32 {
        // Local coordinates: the bracket runs along +u and +v from the corner.
        let u = (px - cx) * sx;
        let v = (py - cy) * sy;
        let cu = u.clamp(radius, arm);
        let cv = v.clamp(radius, arm);
        let seg_u = (u - cu).hypot(v);
        let seg_v = u.hypot(v - cv);
        let arc = if u <= radius && v <= radius {
            ((radius - u).hypot(radius - v) - radius).abs()
        } else {
            f32::MAX
        };
        seg_u.min(seg_v).min(arc)
    };
    let reach = (half + 2.0) as i64;
    let boxes = [
        (fx1, fy1, 1.0, 1.0),
        (fx2, fy1, -1.0, 1.0),
        (fx1, fy2, 1.0, -1.0),
        (fx2, fy2, -1.0, -1.0),
    ];
    for (cx, cy, sx, sy) in boxes {
        let (ax, ay) = (cx as i64, cy as i64);
        let span = arm as i64 + reach;
        let (xa, xb) = if sx > 0.0 {
            (ax - reach, ax + span)
        } else {
            (ax - span, ax + reach)
        };
        let (ya, yb) = if sy > 0.0 {
            (ay - reach, ay + span)
        } else {
            (ay - span, ay + reach)
        };
        for y in ya.max(0)..=yb.min(h as i64 - 1) {
            for x in xa.max(0)..=xb.min(w as i64 - 1) {
                let d = bracket(x as f32 + 0.5, y as f32 + 0.5, cx, cy, sx, sy);
                let a = (half + 0.5 - d).clamp(0.0, 1.0);
                if a > 0.0 {
                    let i = ((y as u32 * w + x as u32) * 4) as usize;
                    for c in 0..3 {
                        let old = bgra[i + c] as f32;
                        bgra[i + c] = (old + (color[c] as f32 - old) * a).round() as u8;
                    }
                }
            }
        }
    }
}
