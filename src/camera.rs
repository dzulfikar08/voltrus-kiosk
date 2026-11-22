//! Webcam capture on its own thread. Keeps the latest frame for the
//! recognizer and a mirrored BGRA preview (with the face box drawn in) for
//! the window.

use crate::face::math::Rgb;
use crate::state::Shared;
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
/// with the last face box outlined.
fn preview(frame: &Rgb, face: Option<([f32; 4], bool)>) -> Arc<RenderImage> {
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
    if let Some((b, matched)) = face {
        let color: [u8; 3] = if matched {
            [94, 197, 34]
        } else {
            [11, 158, 245]
        }; // BGR: green / amber
        let to = |v: f32| (v / step as f32).round() as i64;
        let (x1, x2) = ((w as i64 - 1) - to(b[2]), (w as i64 - 1) - to(b[0]));
        let (y1, y2) = (to(b[1]), to(b[3]));
        let thickness = (w as i64 / 240).max(2);
        let mut put = |x: i64, y: i64| {
            if x >= 0 && y >= 0 && x < w as i64 && y < h as i64 {
                let d = ((y as u32 * w + x as u32) * 4) as usize;
                bgra[d..d + 3].copy_from_slice(&color);
            }
        };
        for t in 0..thickness {
            for x in x1..=x2 {
                put(x, y1 + t);
                put(x, y2 - t);
            }
            for y in y1..=y2 {
                put(x1 + t, y);
                put(x2 - t, y);
            }
        }
    }
    let buffer = image::RgbaImage::from_raw(w, h, bgra).expect("preview buffer size");
    Arc::new(RenderImage::new([image::Frame::new(buffer)]))
}
