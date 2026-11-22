//! The update thread: check 30 s after start and every 6 hours (or when
//! asked), install a newer signed release, then restart once nobody has
//! been at the camera for a minute.

use crate::state::{Shared, UpdateState};
use crate::updater;
use std::sync::Arc;
use std::sync::mpsc::{RecvTimeoutError, channel};
use std::time::Duration;

const EVERY: Duration = Duration::from_secs(6 * 3600);
const IDLE_SECS: u64 = 60;

pub fn spawn(shared: Arc<Shared>, auto: impl Fn() -> bool + Send + 'static) {
    let (tx, rx) = channel::<()>();
    *shared.update_now.lock() = Some(tx);
    std::thread::Builder::new()
        .name("updater".into())
        .spawn(move || {
            updater::clean_up();
            let mut wait = Duration::from_secs(30);
            let mut asked = false;
            loop {
                if let UpdateState::Ready(_) = &*shared.update.lock() {
                    if shared.idle_for(IDLE_SECS) {
                        updater::restart();
                    }
                    match rx.recv_timeout(Duration::from_secs(5)) {
                        Ok(()) => updater::restart(), // "Restart now"
                        Err(RecvTimeoutError::Timeout) => continue,
                        Err(RecvTimeoutError::Disconnected) => return,
                    }
                }
                match rx.recv_timeout(wait) {
                    Ok(()) => asked = true,
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => return,
                }
                wait = EVERY;
                if !asked && !auto() {
                    continue;
                }
                asked = false;
                *shared.update.lock() = UpdateState::Checking;
                let next = match updater::check() {
                    Ok(None) => UpdateState::UpToDate,
                    Ok(Some(manifest)) => match updater::install(&manifest) {
                        Ok(_) => UpdateState::Ready(manifest.version),
                        Err(e) => UpdateState::Failed(format!("{e:#}")),
                    },
                    Err(e) => UpdateState::Failed(format!("{e:#}")),
                };
                *shared.update.lock() = next;
            }
        })
        .expect("updater thread");
}

pub fn ask(shared: &Shared) {
    if let Some(tx) = shared.update_now.lock().as_ref() {
        let _ = tx.send(());
    }
}
