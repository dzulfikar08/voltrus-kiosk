//! State shared by the camera, recognizer and sync threads and the window.

use crate::face::math::Rgb;
use crate::store::{Event, Events, Gallery};
use gpui_kit::RenderImage;
use parking_lot::Mutex;
use std::sync::Arc;
use std::sync::mpsc::Sender;
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// First scan of the day checks in, later scans check out.
    Auto,
    In,
    Out,
}

/// What the recognizer says about the person in front of the camera.
#[derive(Debug, Clone, PartialEq)]
pub enum Seen {
    Nobody,
    /// A face, not (yet) a confident match.
    Unknown,
    /// MiniFASNet says this is not a live face (a photo or a screen).
    NotLive,
    /// Matched, but already recorded within the cooldown.
    Again {
        name: String,
    },
    /// Recorded just now.
    Recorded {
        event: Box<Event>,
    },
}

#[derive(Debug, Clone, Default)]
pub struct SyncStatus {
    pub online: bool,
    pub last_pull: Option<String>,
    pub last_push: Option<String>,
    pub error: Option<String>,
}

pub struct Shared {
    frame: Mutex<Option<(u64, Arc<Rgb>)>>,
    preview: Mutex<Option<Arc<RenderImage>>>,
    face: Mutex<Option<([f32; 4], bool)>>,
    camera_error: Mutex<Option<String>>,
    pub seen: Mutex<(Seen, Instant)>,
    pub mode: Mutex<Mode>,
    pub gallery: Mutex<Option<Gallery>>,
    pub events: Mutex<Events>,
    pub sync: Mutex<SyncStatus>,
    pub engine_error: Mutex<Option<String>>,
    /// Wakes the sync thread (a new event to push, or a pull request).
    pub wake: Mutex<Option<Sender<()>>>,
}

impl Shared {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            frame: Mutex::new(None),
            preview: Mutex::new(None),
            face: Mutex::new(None),
            camera_error: Mutex::new(None),
            seen: Mutex::new((Seen::Nobody, Instant::now())),
            mode: Mutex::new(Mode::Auto),
            gallery: Mutex::new(Gallery::load()),
            events: Mutex::new(Events::load()),
            sync: Mutex::new(SyncStatus::default()),
            engine_error: Mutex::new(None),
            wake: Mutex::new(None),
        })
    }

    pub fn put_frame(&self, frame: Arc<Rgb>, preview: Arc<RenderImage>) {
        let mut slot = self.frame.lock();
        let n = slot.as_ref().map_or(0, |(n, _)| n + 1);
        *slot = Some((n, frame));
        *self.preview.lock() = Some(preview);
    }

    /// The newest frame, if it is newer than `after`.
    pub fn frame_after(&self, after: Option<u64>) -> Option<(u64, Arc<Rgb>)> {
        let slot = self.frame.lock();
        match (&*slot, after) {
            (Some((n, _)), Some(a)) if *n == a => None,
            (Some((n, f)), _) => Some((*n, f.clone())),
            _ => None,
        }
    }

    pub fn take_preview(&self) -> Option<Arc<RenderImage>> {
        self.preview.lock().take()
    }

    pub fn face_box(&self) -> Option<([f32; 4], bool)> {
        *self.face.lock()
    }

    pub fn set_face_box(&self, face: Option<([f32; 4], bool)>) {
        *self.face.lock() = face;
    }

    pub fn camera_error(&self) -> Option<String> {
        self.camera_error.lock().clone()
    }

    pub fn set_camera_error(&self, e: Option<String>) {
        *self.camera_error.lock() = e;
    }

    pub fn set_seen(&self, seen: Seen) {
        let mut slot = self.seen.lock();
        // A fresh "recorded" stays on screen for a few seconds even if the
        // person walks off.
        if let Seen::Recorded { .. } = slot.0
            && slot.1.elapsed().as_secs() < 4
            && !matches!(seen, Seen::Recorded { .. })
        {
            return;
        }
        if slot.0 != seen {
            *slot = (seen, Instant::now());
        }
    }

    pub fn wake_sync(&self) {
        if let Some(tx) = self.wake.lock().as_ref() {
            let _ = tx.send(());
        }
    }
}
