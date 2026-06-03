//! In-memory registry of active runs.
//!
//! Two jobs: (1) enforce **one active run per owner** (a second concurrent run
//! is rejected with 409, so a runaway agent can't fork-bomb the single-user
//! host), and (2) carry the cancellation signal from `POST /cli/runs/{id}/cancel`
//! to the in-flight streaming task. State is process-local — runs don't survive
//! a restart, which is fine for a synchronous, streamed model.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use tokio::sync::Notify;

#[derive(Default)]
pub struct RunRegistry {
    inner: Mutex<State>,
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
        Some(cancel)
    }

    /// Release the slot for a finished run. Idempotent.
    pub fn finish(&self, owner: &str, run_id: &str) {
        let mut s = self.inner.lock().expect("registry mutex");
        s.active_owners.remove(owner);
        s.cancels.remove(run_id);
        s.cancelled.remove(run_id);
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
}
