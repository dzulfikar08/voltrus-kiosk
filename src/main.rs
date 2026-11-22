//! Voltrus ERP face attendance kiosk (M17, US-192): a native GPUI app that
//! recognises employees offline with the console's three face models and
//! sends signed check-ins to /api/hr/kiosk/*.
//!
//!   voltrus-kiosk              run the kiosk
//!   voltrus-kiosk identity     print the Device UID and public key
//!   voltrus-kiosk embed IMG    print the embedding of the largest face (JSON)
//!   voltrus-kiosk check IMG [in|out]
//!                              pull from the server, identify the photo, and
//!                              with in|out send that check-in (install test)

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod camera;
mod client;
mod face;
mod i18n;
mod recognizer;
mod state;
mod store;
mod sync;
mod ui;

use gpui_kit::component::{Root, Theme, ThemeMode};
use gpui_kit::{
    AppContext as _, Bounds, KeyBinding, TitlebarOptions, WindowBounds, WindowOptions, point, px,
    size,
};
use std::sync::Arc;

fn embed_cli(path: &str) -> anyhow::Result<()> {
    let img = image::open(path)?.to_rgb8();
    let frame = face::math::Rgb {
        width: img.width(),
        height: img.height(),
        data: img.into_raw(),
    };
    let mut engine = face::engine::Engine::load(&store::home().join("models"))?;
    let out = engine.embed(&frame)?;
    let json = match out {
        None => serde_json::json!({ "face": null }),
        Some(e) => serde_json::json!({
            "model": face::math::FACE_MODEL,
            "bbox": e.face.bbox,
            "score": e.face.score,
            "kps": e.face.kps,
            "liveness": e.liveness,
            "embedding": e.embedding,
        }),
    };
    println!("{json}");
    Ok(())
}

/// The kiosk's whole path without camera or window: pull, identify, push.
fn check_cli(path: &str, record: Option<&str>) -> anyhow::Result<()> {
    let config = store::Config::load();
    anyhow::ensure!(
        config.is_configured(),
        "no server configured: run the kiosk once and fill in setup"
    );
    let identity = store::Identity::load_or_create()?;
    let client = client::Client::new(&config.server_url, identity);
    let models = store::home().join("models");
    face::models::ensure(&models, &config.server_url)?;
    let gallery = client.pull()?;
    println!(
        "pulled: {} employees, {} faces, threshold {}, margin {}",
        gallery.employees.len(),
        gallery.faces.len(),
        gallery.settings.face_threshold,
        gallery.settings.face_margin
    );
    let img = image::open(path)?.to_rgb8();
    let frame = face::math::Rgb {
        width: img.width(),
        height: img.height(),
        data: img.into_raw(),
    };
    let mut engine = face::engine::Engine::load(&models)?;
    let Some(found) = engine.embed(&frame)? else {
        anyhow::bail!("no face in {path}");
    };
    println!(
        "face score {:.3}, liveness {:?}",
        found.face.score, found.liveness
    );
    let Some(m) = recognizer::identify(&gallery, &found.embedding) else {
        anyhow::bail!("no confident match");
    };
    println!(
        "match: {} ({}) score {:.3}",
        gallery.name_of(&m.employee_id),
        m.employee_id,
        m.score
    );
    if let Some(kind) = record {
        let event_type = if kind == "out" {
            store::EventType::Out
        } else {
            store::EventType::In
        };
        let event = store::Event {
            local_id: format!("check-{}", client.now()),
            name: gallery.name_of(&m.employee_id),
            payload: store::Payload {
                employee_id: m.employee_id,
                event_type,
                ts: client.now_iso(),
                confidence: m.score,
            },
            status: store::EventStatus::Queued,
            error: None,
        };
        for (id, status, error) in client.push(&[event])? {
            println!("push {id}: {status:?} {}", error.unwrap_or_default());
        }
    }
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("--version" | "-V") => {
            println!(
                "voltrus-kiosk {} ({})",
                env!("CARGO_PKG_VERSION"),
                face::math::FACE_MODEL
            );
            return Ok(());
        }
        Some("identity") => {
            let id = store::Identity::load_or_create()?;
            println!(
                "Device UID: {}\nPublic key: {}",
                id.device_uid,
                id.public_key_b64()
            );
            return Ok(());
        }
        Some("embed") => return embed_cli(args.get(2).map(String::as_str).unwrap_or_default()),
        Some("check") => {
            return check_cli(
                args.get(2).map(String::as_str).unwrap_or_default(),
                args.get(3).map(String::as_str),
            );
        }
        _ => {}
    }

    let config = store::Config::load();
    let identity = store::Identity::load_or_create()?;
    let models = store::home().join("models");
    let shared = state::Shared::new();
    let client = Arc::new(client::Client::new(&config.server_url, identity.clone()));

    camera::spawn(shared.clone(), config.camera_index);
    {
        let client = client.clone();
        recognizer::spawn(shared.clone(), models.clone(), move || client.now_iso());
    }
    // Idles until setup gives it a server address.
    sync::spawn(shared.clone(), client.clone(), models.clone());

    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(move |cx| {
            gpui_kit::init(cx);
            Theme::change(ThemeMode::Dark, None, cx);
            cx.bind_keys([
                KeyBinding::new("ctrl-shift-s", ui::OpenSetup, None),
                KeyBinding::new("f11", ui::ToggleFullscreen, None),
            ]);
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: point(px(0.), px(0.)),
                    size: size(px(1280.), px(800.)),
                })),
                window_min_size: Some(size(px(1024.), px(640.))),
                titlebar: Some(TitlebarOptions {
                    title: Some("Voltrus Kiosk".into()),
                    ..Default::default()
                }),
                app_id: Some("voltrus-kiosk".into()),
                ..Default::default()
            };
            cx.spawn(async move |cx| {
                cx.open_window(options, |window, cx| {
                    let view = cx.new(|cx| {
                        ui::KioskApp::new(
                            shared.clone(),
                            client.clone(),
                            identity,
                            config,
                            models.clone(),
                            window,
                            cx,
                        )
                    });
                    cx.new(|cx| Root::new(view, window, cx))
                })
                .expect("open the kiosk window");
            })
            .detach();
        });
    Ok(())
}
