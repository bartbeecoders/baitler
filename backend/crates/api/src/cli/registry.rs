//! In-memory registry of active runs.
//!
//! Three jobs: (1) enforce **one active run per owner** (a second concurrent run
//! is rejected with 409, so a runaway agent can't fork-bomb the single-user
//! host), (2) carry the cancellation signal from `POST /cli/runs/{id}/cancel`
//! to the in-flight run task, and (3) act as the **event hub** for a run: the
//! background task `publish`es every [`RunEvent`], and any number of SSE
//! connections `subscribe` — each gets an atomic replay of everything so far
//! plus a live tail. This is what lets a client navigate away mid-run and
//! re-attach later. State is process-local — runs don't survive a restart.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use serde_json::Value;
use tokio::sync::{broadcast, Notify};

/// Broadcast capacity per run. A subscriber that lags this far behind simply
/// skips ahead (`Lagged`); the replay buffer it got at subscribe time is intact.
const CHANNEL_CAPACITY: usize = 256;

#[derive(Default)]
pub struct RunRegistry {
    inner: Mutex<State>,
}

/// Per-run event fan-out: every wire-ready SSE payload published so far + a
/// live channel. Payloads are the serialized `type`-tagged JSON messages
/// (including the synthetic leading `run` and trailing `done`), so subscribers
/// forward them verbatim.
struct RunChannel {
    buffer: Vec<Value>,
    tx: broadcast::Sender<Value>,
}

#[derive(Default)]
struct State {
    /// Owners with an active run (the per-owner serialisation gate).
    active_owners: HashSet<String>,
    /// run_id → cancel signal for currently-streaming runs.
    cancels: HashMap<String, Arc<Notify>>,
    /// run_ids that were explicitly cancelled (so the finalizer records the
    /// right terminal status). Cleared by `finish`.
    cancelled: HashSet<String>,
    /// run_id → event buffer + live broadcast, dropped on `finish` (closing
    /// every subscriber's tail).
    channels: HashMap<String, RunChannel>,
}

impl RunRegistry {
    /// Reserve an active slot for `owner`'s `run_id`. Returns the cancel handle
    /// to hand to the runner, or `None` if the owner already has an active run.
    pub fn begin(&self, owner: &str, run_id: &str) -> Option<Arc<Notify>> {
        let mut s = self.inner.lock().expect("registry mutex");
        if s.active_owners.contains(owner) {
            return None;
        }
        let cancel = Arc::new(Notify::new());
        s.active_owners.insert(owner.to_string());
        s.cancels.insert(run_id.to_string(), cancel.clone());
        let (tx, _) = broadcast::channel(CHANNEL_CAPACITY);
        s.channels.insert(
            run_id.to_string(),
            RunChannel {
                buffer: Vec::new(),
                tx,
            },
        );
        Some(cancel)
    }

    /// Release the slot for a finished run. Idempotent. Dropping the channel
    /// closes every subscriber's live tail.
    pub fn finish(&self, owner: &str, run_id: &str) {
        let mut s = self.inner.lock().expect("registry mutex");
        s.active_owners.remove(owner);
        s.cancels.remove(run_id);
        s.cancelled.remove(run_id);
        s.channels.remove(run_id);
    }

    /// Signal cancellation of an active run. Returns `false` if the run isn't
    /// currently active (already finished or never existed).
    pub fn cancel(&self, run_id: &str) -> bool {
        let mut s = self.inner.lock().expect("registry mutex");
        match s.cancels.get(run_id).cloned() {
            Some(notify) => {
                s.cancelled.insert(run_id.to_string());
                notify.notify_one();
                true
            }
            None => false,
        }
    }

    /// Whether `run_id` was explicitly cancelled (read before `finish`).
    pub fn was_cancelled(&self, run_id: &str) -> bool {
        self.inner
            .lock()
            .expect("registry mutex")
            .cancelled
            .contains(run_id)
    }

    /// Record `payload` for late subscribers and fan it out to live ones. A
    /// send error (no live receivers) is fine — the buffer still serves replays.
    pub fn publish(&self, run_id: &str, payload: Value) {
        let mut s = self.inner.lock().expect("registry mutex");
        if let Some(ch) = s.channels.get_mut(run_id) {
            ch.buffer.push(payload.clone());
            let _ = ch.tx.send(payload);
        }
    }

    /// Subscribe to an active run: an atomic snapshot of every payload published
    /// so far plus a receiver for the live tail (no gap, no duplicates — both
    /// are taken under the same lock). `None` when the run is not active.
    pub fn subscribe(&self, run_id: &str) -> Option<(Vec<Value>, broadcast::Receiver<Value>)> {
        let s = self.inner.lock().expect("registry mutex");
        s.channels
            .get(run_id)
            .map(|ch| (ch.buffer.clone(), ch.tx.subscribe()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_active_run_per_owner() {
        let reg = RunRegistry::default();
        assert!(reg.begin("alice", "r1").is_some());
        // Second concurrent run for the same owner is rejected.
        assert!(reg.begin("alice", "r2").is_none());
        // A different owner is unaffected.
        assert!(reg.begin("bob", "r3").is_some());
        // After finishing, the owner can start again.
        reg.finish("alice", "r1");
        assert!(reg.begin("alice", "r4").is_some());
    }

    #[test]
    fn cancel_signals_only_active_runs() {
        let reg = RunRegistry::default();
        assert!(!reg.cancel("ghost"), "cancelling an unknown run is a no-op");
        reg.begin("alice", "r1");
        assert!(reg.cancel("r1"));
        assert!(reg.was_cancelled("r1"));
        reg.finish("alice", "r1");
        // The cancelled flag is cleared on finish.
        assert!(!reg.was_cancelled("r1"));
    }

    #[test]
    fn subscribe_replays_then_tails() {
        let reg = RunRegistry::default();
        reg.begin("alice", "r1");
        reg.publish(
            "r1",
            serde_json::json!({ "type": "assistant", "text": "first" }),
        );

        // A late subscriber sees the snapshot…
        let (replay, mut rx) = reg.subscribe("r1").expect("active run");
        assert_eq!(replay.len(), 1);

        // …and live events published after it subscribed.
        reg.publish(
            "r1",
            serde_json::json!({ "type": "assistant", "text": "second" }),
        );
        let live = rx.try_recv().expect("live event");
        assert_eq!(live["text"], "second");

        // finish() closes the tail and ends subscriptions.
        reg.finish("alice", "r1");
        assert!(rx.try_recv().is_err());
        assert!(reg.subscribe("r1").is_none(), "finished run is gone");
    }

    #[test]
    fn publish_without_subscribers_is_buffered() {
        let reg = RunRegistry::default();
        reg.begin("alice", "r1");
        // No subscriber yet — the broadcast send fails internally, but the
        // buffer must still serve a later replay (the re-attach case).
        reg.publish("r1", serde_json::json!({ "type": "error", "message": "x" }));
        let (replay, _rx) = reg.subscribe("r1").expect("active run");
        assert_eq!(replay.len(), 1);
    }
}
