//! Background sync: push the offline queue as soon as there is something to
//! push (and every 20 s while anything is left), pull the gallery every
//! 5 minutes.

use crate::client::{CallError, Client};
use crate::state::Shared;
use std::sync::Arc;
use std::sync::mpsc::{RecvTimeoutError, channel};
use std::time::{Duration, Instant};

const PUSH_EVERY: Duration = Duration::from_secs(20);
const PULL_EVERY: Duration = Duration::from_secs(300);
const BATCH: usize = 200;

fn describe(e: &anyhow::Error) -> (bool, String) {
    match e.downcast_ref::<CallError>() {
        Some(CallError::Offline(m)) => (false, m.clone()),
        Some(other) => (true, other.to_string()),
        None => (true, format!("{e:#}")),
    }
}

pub fn pull_once(shared: &Shared, client: &Client) -> anyhow::Result<()> {
    let result = client.pull();
    let mut status = shared.sync.lock();
    match result {
        Ok(gallery) => {
            gallery.save()?;
            *shared.gallery.lock() = Some(gallery);
            status.online = true;
            status.last_pull = Some(client.now_iso());
            status.error = None;
            Ok(())
        }
        Err(e) => {
            let (online, message) = describe(&e);
            status.online = online;
            status.error = Some(message);
            Err(e)
        }
    }
}

fn push_once(shared: &Shared, client: &Client) {
    loop {
        let batch: Vec<_> = shared
            .events
            .lock()
            .queue
            .iter()
            .take(BATCH)
            .cloned()
            .collect();
        if batch.is_empty() {
            return;
        }
        match client.push(&batch) {
            Ok(results) => {
                let progressed = results
                    .iter()
                    .any(|(_, s, _)| *s != crate::store::EventStatus::Queued);
                if let Err(e) = shared.events.lock().settle(&results) {
                    shared.sync.lock().error = Some(format!("{e:#}"));
                    return;
                }
                let mut status = shared.sync.lock();
                status.online = true;
                status.last_push = Some(client.now_iso());
                status.error = None;
                if !progressed {
                    return; // all `retry`: try again next round
                }
            }
            Err(e) => {
                let (online, message) = describe(&e);
                let mut status = shared.sync.lock();
                status.online = online;
                status.error = Some(message);
                return;
            }
        }
    }
}

pub fn spawn(shared: Arc<Shared>, client: Arc<Client>, models: std::path::PathBuf) {
    let (tx, rx) = channel::<()>();
    *shared.wake.lock() = Some(tx);
    std::thread::Builder::new()
        .name("sync".into())
        .spawn(move || {
            let mut last_pull: Option<Instant> = None;
            let mut models_ok = false;
            loop {
                if client.base().is_empty() {
                    match rx.recv_timeout(PUSH_EVERY) {
                        Ok(()) | Err(RecvTimeoutError::Timeout) => continue,
                        Err(RecvTimeoutError::Disconnected) => return,
                    }
                }
                if !models_ok {
                    match crate::face::models::ensure(&models, &client.base()) {
                        Ok(()) => models_ok = true,
                        Err(e) => shared.sync.lock().error = Some(format!("face models: {e:#}")),
                    }
                }
                if last_pull.is_none_or(|t| t.elapsed() >= PULL_EVERY) {
                    let _ = pull_once(&shared, &client);
                    last_pull = Some(Instant::now());
                }
                push_once(&shared, &client);
                match rx.recv_timeout(PUSH_EVERY) {
                    Ok(()) | Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => return,
                }
            }
        })
        .expect("sync thread");
}
