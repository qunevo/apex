//! Bounded background worker pool. Engine work runs on blocking threads while
//! the async side keeps the lease alive and watches for cancellation.
use crate::events::Events;
use apex_control::{
    engine::CancelToken,
    worker::{Worker, compute},
};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{sync::Notify, task::JoinHandle};

const IDLE_POLL: Duration = Duration::from_secs(2);

pub fn spawn(
    worker: Worker,
    count: usize,
    events: Events,
    wake: Arc<Notify>,
) -> Vec<JoinHandle<()>> {
    (0..count)
        .map(|i| {
            let mut worker = worker.clone();
            worker.id = format!("{}-{i}", worker.id);
            let (events, wake) = (events.clone(), wake.clone());
            tokio::spawn(async move {
                loop {
                    match process_one(&worker, &events).await {
                        Ok(true) => continue,
                        Ok(false) => {}
                        Err(e) => eprintln!("worker {}: {e}", worker.id),
                    }
                    tokio::select! {
                        _ = wake.notified() => {}
                        _ = tokio::time::sleep(IDLE_POLL) => {}
                    }
                }
            })
        })
        .collect()
}

/// Process one run if available. Returns whether a run was claimed.
pub async fn process_one(worker: &Worker, events: &Events) -> apex_control::Result<bool> {
    let Some(claimed) = worker.claim().await? else {
        return Ok(false);
    };
    let claimed = Arc::new(claimed);
    let (tenant, scenario) = (claimed.run.tenant, claimed.run.scenario.to_string());
    let notify = |kind: &'static str, id: String| {
        events.publish(tenant, kind, id, scenario.clone());
    };
    notify("run", claimed.run.id.to_string());

    let cancel = CancelToken::default();
    let phase = Arc::new(Mutex::new(String::from("claimed")));
    let job = {
        let (claimed, cancel, phase) = (claimed.clone(), cancel.clone(), phase.clone());
        tokio::task::spawn_blocking(move || {
            compute(&claimed, &cancel, &|p| {
                *phase.lock().unwrap() = p.to_string()
            })
        })
    };
    tokio::pin!(job);
    let tick = Duration::from_millis(500);
    let beat_every = (worker.lease / 3).to_std().unwrap_or(tick).max(tick);
    let mut last_beat = tokio::time::Instant::now();
    let mut reported = String::from("claimed");
    let completion = loop {
        tokio::select! {
            done = &mut job => {
                break done.map_err(|e| apex_control::Error::Store(format!("engine task: {e}")))?;
            }
            _ = tokio::time::sleep(tick) => {
                let current = phase.lock().unwrap().clone();
                let changed = current != reported;
                if changed || last_beat.elapsed() >= beat_every {
                    // Extends the lease and observes cancellation requests.
                    if !worker.heartbeat(&claimed, &current).await? {
                        cancel.cancel();
                    }
                    last_beat = tokio::time::Instant::now();
                }
                if changed {
                    reported = current;
                    notify("run", claimed.run.id.to_string());
                }
            }
        }
    };
    let result = match &completion {
        apex_control::store::Completion::Succeeded(r) => Some(r.id),
        _ => None,
    };
    let finished = worker.finish(&claimed, completion).await?;
    notify("run", finished.id.to_string());
    if let Some(id) = result {
        notify("result", id.to_string());
    }
    Ok(true)
}
