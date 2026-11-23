//! The recognition loop: embed the newest frame, match it against the pulled
//! gallery with the server's own identify rule, and queue an
//! attendance event.

use crate::face::engine::Engine;
use crate::face::math::{ANTISPOOF_THRESH, FACE_MODEL, dot};
use crate::state::{CaptureHint, Mode, Seen, Shared, Tone};
use crate::store::{Event, EventStatus, EventType, Gallery, Payload};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// The same person within this window is the same scan (console kiosk: 60 s).
const COOLDOWN: Duration = Duration::from_secs(60);
/// A match must repeat on consecutive frames within this window before it counts.
const CONFIRM_WITHIN: Duration = Duration::from_secs(2);
/// Frame interval while a face is being identified (~7 per second).
const ACTIVE_PERIOD: Duration = Duration::from_millis(150);
/// Frame interval with nobody in front of the camera, or someone already
/// recorded: enough to notice the next person within half a second.
const IDLE_PERIOD: Duration = Duration::from_millis(450);
/// How long the camera must be empty before slowing down.
const IDLE_AFTER: Duration = Duration::from_secs(3);

#[derive(Debug, Clone, PartialEq)]
pub struct Match {
    pub employee_id: String,
    pub score: f32,
}

/// Best score per employee (max over their templates); a match needs the top
/// score at or above `threshold` and a lead of at least `margin` over the
/// next employee.
pub fn identify(gallery: &Gallery, embedding: &[f32]) -> Option<Match> {
    let mut best: HashMap<&str, f32> = HashMap::new();
    for face in &gallery.faces {
        if face.embedding.len() != embedding.len() {
            continue;
        }
        let score = dot(embedding, &face.embedding);
        let slot = best.entry(face.employee_id.as_str()).or_insert(f32::MIN);
        if score > *slot {
            *slot = score;
        }
    }
    let mut ranked: Vec<(&str, f32)> = best.into_iter().collect();
    ranked.sort_by(|a, b| b.1.total_cmp(&a.1));
    let top = ranked.first()?;
    if let Some(second) = ranked.get(1)
        && top.1 - second.1 < gallery.settings.face_margin
    {
        return None;
    }
    (top.1 >= gallery.settings.face_threshold).then(|| Match {
        employee_id: top.0.to_string(),
        score: top.1,
    })
}

/// Local midnight as an ISO UTC string (events are stamped in UTC).
fn start_of_today_utc() -> String {
    let local = chrono::Local::now()
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .expect("midnight");
    local
        .and_local_timezone(chrono::Local)
        .earliest()
        .map(|t| {
            t.with_timezone(&chrono::Utc)
                .to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
        })
        .unwrap_or_default()
}

fn event_type(shared: &Shared, employee_id: &str) -> EventType {
    match *shared.mode.lock() {
        Mode::In => EventType::In,
        Mode::Out => EventType::Out,
        Mode::Auto => {
            if shared
                .events
                .lock()
                .last_for(employee_id, &start_of_today_utc())
                .is_some()
            {
                EventType::Out
            } else {
                EventType::In
            }
        }
    }
}

/// Smallest face side (frame pixels) taken as an enrolment sample.
const ENROL_MIN_FACE: f32 = 110.0;
/// Time between enrolment samples, so the person can turn a little.
const ENROL_SPACING: Duration = Duration::from_millis(700);

/// One frame of an enrolment: take it as a sample if exactly one live,
/// large enough face is in view and it is not a copy of the last sample.
fn capture_step(shared: &Shared, engine: &mut Engine, frame: &crate::face::math::Rgb) {
    shared.set_seen(Seen::Matching);
    let faces = match engine.detect(frame) {
        Ok(f) => f,
        Err(e) => {
            *shared.engine_error.lock() = Some(format!("{e:#}"));
            return;
        }
    };
    let hint = |h: CaptureHint| {
        if let Some(c) = shared.capture.lock().as_mut() {
            c.hint = h;
        }
    };
    let face = match faces.as_slice() {
        [] => {
            shared.set_face_box(None);
            return hint(CaptureHint::NoFace);
        }
        [one] => one.clone(),
        _ => {
            shared.set_face_box(None);
            return hint(CaptureHint::TooMany);
        }
    };
    let side = (face.bbox[2] - face.bbox[0]).min(face.bbox[3] - face.bbox[1]);
    if side < ENROL_MIN_FACE {
        shared.set_face_box(Some((face.bbox, Tone::Neutral)));
        return hint(CaptureHint::TooSmall);
    }
    if engine
        .liveness(frame, &face)
        .is_some_and(|p| p < ANTISPOOF_THRESH)
    {
        shared.set_face_box(Some((face.bbox, Tone::Reject)));
        return hint(CaptureHint::NotLive);
    }
    let ready =
        shared.capture.lock().as_ref().is_some_and(|c| {
            !c.done() && c.last_taken.is_none_or(|t| t.elapsed() >= ENROL_SPACING)
        });
    if !ready {
        shared.set_face_box(Some((face.bbox, Tone::Neutral)));
        return hint(CaptureHint::Hold);
    }
    let embedding = match engine.embed_face(frame, &face) {
        Ok(e) => e,
        Err(e) => {
            *shared.engine_error.lock() = Some(format!("{e:#}"));
            return;
        }
    };
    let mut slot = shared.capture.lock();
    let Some(capture) = slot.as_mut() else { return };
    // A sample identical to the last one adds nothing.
    if capture
        .samples
        .last()
        .is_some_and(|last| dot(last, &embedding) > 0.995)
    {
        capture.hint = CaptureHint::Hold;
        return;
    }
    capture.samples.push(embedding);
    capture.last_taken = Some(Instant::now());
    capture.hint = CaptureHint::Taken;
    shared.set_face_box(Some((face.bbox, Tone::Match)));
}

pub fn spawn(shared: Arc<Shared>, models: PathBuf, now_iso: impl Fn() -> String + Send + 'static) {
    std::thread::Builder::new()
        .name("recognizer".into())
        .spawn(move || {
            let mut engine = loop {
                match Engine::load(&models) {
                    Ok(e) => {
                        *shared.engine_error.lock() = None;
                        break e;
                    }
                    Err(e) => {
                        *shared.engine_error.lock() = Some(format!("{e:#}"));
                        std::thread::sleep(Duration::from_secs(5));
                    }
                }
            };
            if !engine.has_antispoof() {
                *shared.engine_error.lock() = Some("antispoof.onnx missing: liveness is off".into());
            }
            let mut last_frame = None;
            let mut pending: Option<(String, Instant)> = None;
            let mut recorded: HashMap<String, Instant> = HashMap::new();
            loop {
                let Some((n, frame)) = shared.frame_after(last_frame) else {
                    std::thread::sleep(Duration::from_millis(40));
                    continue;
                };
                last_frame = Some(n);
                let started = Instant::now();
                if shared.capture.lock().is_some() {
                    pending = None;
                    capture_step(&shared, &mut engine, &frame);
                    let spent = started.elapsed();
                    if spent < ACTIVE_PERIOD {
                        std::thread::sleep(ACTIVE_PERIOD - spent);
                    }
                    continue;
                }
                let result = engine.embed(&frame);
                let gallery = shared.gallery.lock().clone();
                match (result, gallery) {
                    (Ok(Some(found)), Some(gallery)) if gallery.settings.face_model == FACE_MODEL => {
                        if found.liveness.is_some_and(|p| p < ANTISPOOF_THRESH) {
                            shared.set_face_box(Some((found.face.bbox, Tone::Reject)));
                            shared.set_seen(Seen::NotLive);
                            pending = None;
                        } else if let Some(m) = identify(&gallery, &found.embedding) {
                            shared.set_face_box(Some((found.face.bbox, Tone::Match)));
                            let name = gallery.name_of(&m.employee_id);
                            let confirmed = matches!(&pending, Some((id, at)) if *id == m.employee_id && at.elapsed() < CONFIRM_WITHIN);
                            if recorded.get(&m.employee_id).is_some_and(|at| at.elapsed() < COOLDOWN) {
                                shared.set_seen(Seen::Again { name });
                            } else if confirmed {
                                pending = None;
                                recorded.insert(m.employee_id.clone(), Instant::now());
                                let mut id = [0u8; 8];
                                rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut id);
                                let event = Event {
                                    local_id: hex::encode(id),
                                    name,
                                    payload: Payload {
                                        event_type: event_type(&shared, &m.employee_id),
                                        employee_id: m.employee_id,
                                        ts: now_iso(),
                                        confidence: (m.score * 1000.0).round() / 1000.0,
                                    },
                                    status: EventStatus::Queued,
                                    error: None,
                                };
                                if let Err(e) = shared.events.lock().record(event.clone()) {
                                    *shared.engine_error.lock() = Some(format!("could not save the event: {e:#}"));
                                }
                                shared.set_seen(Seen::Recorded { event: Box::new(event) });
                                shared.wake_sync();
                            } else {
                                pending = Some((m.employee_id, Instant::now()));
                                shared.set_seen(Seen::Matching);
                            }
                        } else {
                            shared.set_face_box(Some((found.face.bbox, Tone::Neutral)));
                            shared.set_seen(Seen::Unknown);
                            pending = None;
                        }
                    }
                    (Ok(Some(found)), _) => {
                        shared.set_face_box(Some((found.face.bbox, Tone::Neutral)));
                        shared.set_seen(Seen::Unknown);
                    }
                    (Ok(None), _) => {
                        shared.set_face_box(None);
                        shared.set_seen(Seen::Nobody);
                        pending = None;
                    }
                    (Err(e), _) => {
                        *shared.engine_error.lock() = Some(format!("{e:#}"));
                    }
                }
                // Leave the CPU some room: full speed only while someone is
                // being identified, slower when the camera is empty or the
                // person was just recorded.
                let period = {
                    let (seen, since) = &*shared.seen.lock();
                    match seen {
                        Seen::Nobody if since.elapsed() > IDLE_AFTER => IDLE_PERIOD,
                        Seen::Again { .. } => IDLE_PERIOD,
                        _ => ACTIVE_PERIOD,
                    }
                };
                let spent = started.elapsed();
                if spent < period {
                    std::thread::sleep(period - spent);
                }
            }
        })
        .expect("recognizer thread");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::{Face, PullSettings};

    fn gallery(faces: Vec<(&str, Vec<f32>)>) -> Gallery {
        Gallery {
            server_time: String::new(),
            device: Default::default(),
            company: None,
            employees: vec![],
            faces: faces
                .into_iter()
                .map(|(id, e)| Face {
                    employee_id: id.into(),
                    embedding: e,
                })
                .collect(),
            settings: PullSettings {
                face_model: FACE_MODEL.into(),
                face_threshold: 0.45,
                face_margin: 0.08,
            },
        }
    }

    #[test]
    fn matches_the_best_employee_over_all_their_templates() {
        let g = gallery(vec![
            ("a", vec![1.0, 0.0]),
            ("a", vec![0.6, 0.8]),
            ("b", vec![0.0, 1.0]),
        ]);
        let m = identify(&g, &[0.6, 0.8]).unwrap();
        assert_eq!(m.employee_id, "a");
    }

    #[test]
    fn refuses_ambiguous_and_weak_matches() {
        let g = gallery(vec![("a", vec![1.0, 0.0]), ("b", vec![0.96, 0.28])]);
        assert_eq!(identify(&g, &[1.0, 0.0]), None, "lead under the margin");
        let g = gallery(vec![("a", vec![1.0, 0.0])]);
        assert_eq!(identify(&g, &[0.4, 0.916]), None, "under the threshold");
    }
}
