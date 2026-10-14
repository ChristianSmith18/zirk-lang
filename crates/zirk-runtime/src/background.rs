//! Runtime bookkeeping for application-owned background jobs.
//!
//! This module is deliberately independent from the executor and the C ABI.
//! It owns admission, job diagnostics, resilient error delivery, and the
//! bounded drain state machine; the executor integration supplies task IDs,
//! cancellation, and completion notifications.

use std::collections::{BTreeMap, VecDeque};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Instant;

/// Retains recent failures without letting a long-running application grow
/// diagnostics memory forever.
pub const MAX_BACKGROUND_DIAGNOSTICS: usize = 1024;

/// Opaque identity used by the executor integration to associate a scheduler
/// task with its supervisor entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BackgroundJobId(pub u64);

/// A failure from a detached job or from delivery of that failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackgroundFailure {
    pub job: BackgroundJobId,
    pub message: String,
}

/// Durable diagnostic emitted by the supervisor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackgroundDiagnostic {
    JobFailed(BackgroundFailure),
    ErrorHandlerFailed {
        job: BackgroundJobId,
        message: String,
    },
    RecursiveErrorDeliverySuppressed {
        job: BackgroundJobId,
        message: String,
    },
    DrainDeadlineExpired {
        unfinished: Vec<BackgroundJobId>,
    },
}

/// Why a job could not be admitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdmissionError {
    /// The supervisor's active background-job ceiling was reached.
    CapacityReached,
    /// The executor's separate active-task ceiling was reached first.
    TaskCapacityReached,
    Draining,
    DuplicateJob,
    IdExhausted,
}

/// Result of waiting for the supervisor to finish draining.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrainReport {
    pub unfinished: Vec<BackgroundJobId>,
    pub deadline_expired: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Lifecycle {
    Accepting,
    Draining,
    Stopped,
}

struct State {
    lifecycle: Lifecycle,
    jobs: BTreeMap<BackgroundJobId, ()>,
    diagnostics: VecDeque<BackgroundDiagnostic>,
}

/// Bounded owner for detached task bookkeeping.
///
/// The callback runs without the state lock held and may run concurrently on
/// different completion threads. A nested failure on the same thread is
/// recorded instead of invoking the callback recursively.
pub struct BackgroundSupervisor {
    capacity: usize,
    state: Mutex<State>,
    drained: Condvar,
    on_error: Arc<dyn Fn(&BackgroundFailure) + Send + Sync + 'static>,
}

impl BackgroundSupervisor {
    /// Creates a supervisor with finite admission capacity.
    pub fn new(
        capacity: usize,
        on_error: impl Fn(&BackgroundFailure) + Send + Sync + 'static,
    ) -> Self {
        assert!(capacity > 0, "background capacity must be nonzero");
        Self {
            capacity,
            state: Mutex::new(State {
                lifecycle: Lifecycle::Accepting,
                jobs: BTreeMap::new(),
                diagnostics: VecDeque::new(),
            }),
            drained: Condvar::new(),
            on_error: Arc::new(on_error),
        }
    }

    /// Reserves one background slot before the executor makes the task ready.
    pub fn admit(&self, job: BackgroundJobId) -> Result<(), AdmissionError> {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.lifecycle != Lifecycle::Accepting {
            return Err(AdmissionError::Draining);
        }
        if state.jobs.contains_key(&job) {
            return Err(AdmissionError::DuplicateJob);
        }
        if state.jobs.len() >= self.capacity {
            return Err(AdmissionError::CapacityReached);
        }
        state.jobs.insert(job, ());
        Ok(())
    }

    /// Marks a tracked job complete and optionally delivers its failure.
    /// Unknown or already-completed IDs are ignored, making completion
    /// notification idempotent across cancellation races.
    pub fn complete(&self, job: BackgroundJobId, failure: Option<String>) {
        self.complete_inner(job, failure, true);
    }

    /// Records a failed detached job after a managed source handler has
    /// already received it. This preserves diagnostics without invoking the
    /// native fallback callback a second time.
    pub fn complete_handled(&self, job: BackgroundJobId, failure: String) {
        self.complete_inner(job, Some(failure), false);
    }

    pub fn report_error_handler_failure(&self, job: BackgroundJobId, message: String) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        record_diagnostic(
            &mut state,
            BackgroundDiagnostic::ErrorHandlerFailed { job, message },
        );
    }

    fn complete_inner(&self, job: BackgroundJobId, failure: Option<String>, deliver: bool) {
        let tracked = {
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            let tracked = state.jobs.remove(&job).is_some();
            if tracked {
                if let Some(message) = failure.as_ref() {
                    record_diagnostic(
                        &mut state,
                        BackgroundDiagnostic::JobFailed(BackgroundFailure {
                            job,
                            message: message.clone(),
                        }),
                    );
                }
                if state.jobs.is_empty() {
                    self.drained.notify_all();
                }
            }
            tracked
        };

        if !tracked {
            return;
        }
        let Some(message) = failure else { return };
        if !deliver {
            return;
        }
        let error = BackgroundFailure { job, message };

        let supervisor_key = self as *const Self as usize;
        if ACTIVE_HANDLERS.with(|active| active.borrow().contains(&supervisor_key)) {
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            record_diagnostic(
                &mut state,
                BackgroundDiagnostic::RecursiveErrorDeliverySuppressed {
                    job,
                    message: error.message,
                },
            );
            return;
        }
        let _guard = HandlerGuard::enter(supervisor_key);
        if let Err(payload) = catch_unwind(AssertUnwindSafe(|| (self.on_error)(&error))) {
            let message = panic_message(payload.as_ref());
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            record_diagnostic(
                &mut state,
                BackgroundDiagnostic::ErrorHandlerFailed { job, message },
            );
        }
    }

    /// Stops admission and returns the stable set of jobs the executor should
    /// cancel. Repeated calls return the currently unfinished jobs.
    pub fn begin_drain(&self) -> Vec<BackgroundJobId> {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.lifecycle == Lifecycle::Accepting {
            state.lifecycle = Lifecycle::Draining;
        }
        state.jobs.keys().copied().collect()
    }

    /// Waits until jobs complete or `deadline` is reached. Expiry closes the
    /// supervisor and records each remaining job; it does not kill the work.
    pub fn drain_until(&self, deadline: Instant) -> DrainReport {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.lifecycle == Lifecycle::Accepting {
            state.lifecycle = Lifecycle::Draining;
        }
        loop {
            if state.jobs.is_empty() {
                state.lifecycle = Lifecycle::Stopped;
                return DrainReport {
                    unfinished: Vec::new(),
                    deadline_expired: false,
                };
            }
            let now = Instant::now();
            if now >= deadline {
                let unfinished = state.jobs.keys().copied().collect::<Vec<_>>();
                record_diagnostic(
                    &mut state,
                    BackgroundDiagnostic::DrainDeadlineExpired {
                        unfinished: unfinished.clone(),
                    },
                );
                state.lifecycle = Lifecycle::Stopped;
                return DrainReport {
                    unfinished,
                    deadline_expired: true,
                };
            }
            let timeout = deadline.saturating_duration_since(now);
            let (next, timed) = self
                .drained
                .wait_timeout(state, timeout)
                .unwrap_or_else(|e| e.into_inner());
            state = next;
            if timed.timed_out() && !state.jobs.is_empty() {
                // Re-check the monotonic deadline at the top of the loop.
            }
        }
    }

    /// Returns a snapshot of outstanding jobs in deterministic ID order.
    pub fn unfinished(&self) -> Vec<BackgroundJobId> {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .jobs
            .keys()
            .copied()
            .collect()
    }

    /// Returns a copy of diagnostics accumulated so far.
    pub fn diagnostics(&self) -> Vec<BackgroundDiagnostic> {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .diagnostics
            .iter()
            .cloned()
            .collect()
    }
}

fn record_diagnostic(state: &mut State, diagnostic: BackgroundDiagnostic) {
    if state.diagnostics.len() == MAX_BACKGROUND_DIAGNOSTICS {
        state.diagnostics.pop_front();
    }
    state.diagnostics.push_back(diagnostic);
}

thread_local! {
    static ACTIVE_HANDLERS: std::cell::RefCell<std::collections::HashSet<usize>> =
        std::cell::RefCell::new(std::collections::HashSet::new());
}

struct HandlerGuard(usize);

impl HandlerGuard {
    fn enter(key: usize) -> Self {
        ACTIVE_HANDLERS.with(|active| {
            let inserted = active.borrow_mut().insert(key);
            debug_assert!(inserted, "handler recursion checked before acquiring lock");
        });
        Self(key)
    }
}

impl Drop for HandlerGuard {
    fn drop(&mut self) {
        ACTIVE_HANDLERS.with(|active| {
            active.borrow_mut().remove(&self.0);
        });
    }
}

fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    payload
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| {
            payload
                .downcast_ref::<&'static str>()
                .map(|s| (*s).to_owned())
        })
        .unwrap_or_else(|| "background error handler panicked".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread;
    use std::time::Duration;

    #[test]
    fn admission_is_bounded_and_stops_at_drain() {
        let supervisor = BackgroundSupervisor::new(2, |_| {});
        assert_eq!(supervisor.admit(BackgroundJobId(1)), Ok(()));
        assert_eq!(supervisor.admit(BackgroundJobId(2)), Ok(()));
        assert_eq!(
            supervisor.admit(BackgroundJobId(3)),
            Err(AdmissionError::CapacityReached)
        );
        assert_eq!(
            supervisor.admit(BackgroundJobId(1)),
            Err(AdmissionError::DuplicateJob)
        );
        assert_eq!(
            supervisor.begin_drain(),
            vec![BackgroundJobId(1), BackgroundJobId(2)]
        );
        assert_eq!(
            supervisor.admit(BackgroundJobId(3)),
            Err(AdmissionError::Draining)
        );
    }

    #[test]
    fn diagnostic_history_is_bounded_and_keeps_the_most_recent_entries() {
        let supervisor = BackgroundSupervisor::new(1, |_| {});
        for id in 1..=(MAX_BACKGROUND_DIAGNOSTICS as u64 + 1) {
            let job = BackgroundJobId(id);
            supervisor.admit(job).unwrap();
            supervisor.complete(job, Some(format!("failure {id}")));
        }

        let diagnostics = supervisor.diagnostics();
        assert_eq!(diagnostics.len(), MAX_BACKGROUND_DIAGNOSTICS);
        assert!(
            diagnostics
                .iter()
                .all(|diagnostic| matches!(diagnostic, BackgroundDiagnostic::JobFailed(_)))
        );
        assert!(matches!(
            diagnostics.first(),
            Some(BackgroundDiagnostic::JobFailed(failure)) if failure.job == BackgroundJobId(2)
        ));
        assert!(matches!(
            diagnostics.last(),
            Some(BackgroundDiagnostic::JobFailed(failure))
                if failure.job == BackgroundJobId(MAX_BACKGROUND_DIAGNOSTICS as u64 + 1)
        ));
    }

    #[test]
    fn handler_panic_is_recorded_without_recursive_delivery() {
        let calls = Arc::new(AtomicUsize::new(0));
        let handler_calls = calls.clone();
        let supervisor = BackgroundSupervisor::new(2, move |_| {
            handler_calls.fetch_add(1, Ordering::SeqCst);
            panic!("handler failure");
        });
        supervisor.admit(BackgroundJobId(7)).unwrap();
        supervisor.complete(BackgroundJobId(7), Some("job failure".into()));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert!(
            supervisor
                .diagnostics()
                .contains(&BackgroundDiagnostic::ErrorHandlerFailed {
                    job: BackgroundJobId(7),
                    message: "handler failure".into(),
                })
        );
    }

    #[test]
    fn nested_failure_does_not_reenter_error_handler() {
        let supervisor_cell: Arc<Mutex<Option<Arc<BackgroundSupervisor>>>> =
            Arc::new(Mutex::new(None));
        let handler_calls = Arc::new(AtomicUsize::new(0));
        let cell = supervisor_cell.clone();
        let calls = handler_calls.clone();
        let supervisor = Arc::new(BackgroundSupervisor::new(2, move |_| {
            calls.fetch_add(1, Ordering::SeqCst);
            if let Some(supervisor) = cell.lock().unwrap().clone() {
                supervisor.admit(BackgroundJobId(2)).unwrap();
                supervisor.complete(BackgroundJobId(2), Some("nested failure".into()));
            }
        }));
        *supervisor_cell.lock().unwrap() = Some(supervisor.clone());
        supervisor.admit(BackgroundJobId(1)).unwrap();
        supervisor.complete(BackgroundJobId(1), Some("first failure".into()));
        assert_eq!(handler_calls.load(Ordering::SeqCst), 1);
        assert!(supervisor.diagnostics().contains(
            &BackgroundDiagnostic::RecursiveErrorDeliverySuppressed {
                job: BackgroundJobId(2),
                message: "nested failure".into(),
            }
        ));
    }

    #[test]
    fn recursive_failure_then_handler_panic_does_not_poison_later_delivery() {
        let supervisor_cell: Arc<Mutex<Option<Arc<BackgroundSupervisor>>>> =
            Arc::new(Mutex::new(None));
        let calls = Arc::new(AtomicUsize::new(0));
        let cell = supervisor_cell.clone();
        let handler_calls = calls.clone();
        let supervisor = Arc::new(BackgroundSupervisor::new(3, move |_| {
            let call = handler_calls.fetch_add(1, Ordering::SeqCst);
            if call == 0 {
                if let Some(supervisor) = cell.lock().unwrap().clone() {
                    let nested = BackgroundJobId(2);
                    supervisor.admit(nested).unwrap();
                    supervisor.complete(nested, Some("nested failure".into()));
                }
                panic!("first delivery panicked");
            }
        }));
        *supervisor_cell.lock().unwrap() = Some(supervisor.clone());

        for job in [BackgroundJobId(1), BackgroundJobId(3)] {
            supervisor.admit(job).unwrap();
            supervisor.complete(job, Some(format!("failure {}", job.0)));
        }

        assert_eq!(calls.load(Ordering::SeqCst), 2);
        let diagnostics = supervisor.diagnostics();
        assert!(
            diagnostics.contains(&BackgroundDiagnostic::RecursiveErrorDeliverySuppressed {
                job: BackgroundJobId(2),
                message: "nested failure".into(),
            })
        );
        assert!(
            diagnostics.contains(&BackgroundDiagnostic::ErrorHandlerFailed {
                job: BackgroundJobId(1),
                message: "first delivery panicked".into(),
            })
        );
        assert!(
            diagnostics.contains(&BackgroundDiagnostic::JobFailed(BackgroundFailure {
                job: BackgroundJobId(3),
                message: "failure 3".into(),
            }))
        );
    }

    #[test]
    fn drain_deadline_reports_unfinished_jobs_and_closes_admission() {
        let supervisor = BackgroundSupervisor::new(2, |_| {});
        supervisor.admit(BackgroundJobId(9)).unwrap();
        supervisor.begin_drain();
        let report = supervisor.drain_until(Instant::now() + Duration::from_millis(15));
        assert_eq!(report.unfinished, vec![BackgroundJobId(9)]);
        assert!(report.deadline_expired);
        assert_eq!(
            supervisor.admit(BackgroundJobId(10)),
            Err(AdmissionError::Draining)
        );
        assert!(
            supervisor
                .diagnostics()
                .contains(&BackgroundDiagnostic::DrainDeadlineExpired {
                    unfinished: vec![BackgroundJobId(9)]
                })
        );
    }

    #[test]
    fn drain_wakes_when_last_job_completes() {
        let supervisor = Arc::new(BackgroundSupervisor::new(1, |_| {}));
        supervisor.admit(BackgroundJobId(4)).unwrap();
        let worker_supervisor = supervisor.clone();
        let worker = thread::spawn(move || {
            thread::sleep(Duration::from_millis(10));
            worker_supervisor.complete(BackgroundJobId(4), None);
        });
        supervisor.begin_drain();
        let report = supervisor.drain_until(Instant::now() + Duration::from_secs(1));
        worker.join().unwrap();
        assert_eq!(
            report,
            DrainReport {
                unfinished: Vec::new(),
                deadline_expired: false
            }
        );
    }

    #[test]
    fn repeated_admission_completion_cycles_reuse_capacity() {
        const CAPACITY: usize = 4;
        const CYCLES: u64 = 128;
        let supervisor = BackgroundSupervisor::new(CAPACITY, |_| {});

        for cycle in 0..CYCLES {
            let jobs = (0..CAPACITY)
                .map(|slot| BackgroundJobId(cycle * CAPACITY as u64 + slot as u64))
                .collect::<Vec<_>>();
            for &job in &jobs {
                supervisor.admit(job).unwrap();
            }
            assert_eq!(supervisor.unfinished(), jobs);
            assert_eq!(
                supervisor.admit(BackgroundJobId(CYCLES * CAPACITY as u64)),
                Err(AdmissionError::CapacityReached)
            );

            for &job in &jobs {
                supervisor.complete(job, None);
                // Repeated completion is deliberately idempotent.
                supervisor.complete(job, Some("duplicate completion".into()));
            }
            assert!(supervisor.unfinished().is_empty());
            assert!(supervisor.diagnostics().is_empty());
        }
    }

    #[test]
    fn capacity_slot_is_reusable_until_drain_then_completion_finishes_drain() {
        let supervisor = BackgroundSupervisor::new(2, |_| {});
        let first = BackgroundJobId(1);
        let second = BackgroundJobId(2);
        let replacement = BackgroundJobId(3);

        supervisor.admit(first).unwrap();
        supervisor.admit(second).unwrap();
        assert_eq!(
            supervisor.admit(replacement),
            Err(AdmissionError::CapacityReached)
        );

        supervisor.complete(first, None);
        supervisor.admit(replacement).unwrap();
        assert_eq!(supervisor.begin_drain(), vec![second, replacement]);
        assert_eq!(
            supervisor.admit(BackgroundJobId(4)),
            Err(AdmissionError::Draining)
        );

        supervisor.complete(second, None);
        supervisor.complete(replacement, None);
        assert_eq!(
            supervisor.drain_until(Instant::now()),
            DrainReport {
                unfinished: Vec::new(),
                deadline_expired: false,
            }
        );
        assert_eq!(
            supervisor.admit(BackgroundJobId(5)),
            Err(AdmissionError::Draining)
        );
    }

    #[test]
    fn expired_deadline_reports_sorted_jobs_and_closes_admission_immediately() {
        let supervisor = BackgroundSupervisor::new(3, |_| {});
        let jobs = [BackgroundJobId(9), BackgroundJobId(2), BackgroundJobId(5)];
        for job in jobs {
            supervisor.admit(job).unwrap();
        }

        let report = supervisor.drain_until(Instant::now());
        let expected = vec![BackgroundJobId(2), BackgroundJobId(5), BackgroundJobId(9)];
        assert_eq!(
            report,
            DrainReport {
                unfinished: expected.clone(),
                deadline_expired: true,
            }
        );
        assert_eq!(supervisor.unfinished(), expected.clone());
        assert_eq!(
            supervisor.diagnostics(),
            vec![BackgroundDiagnostic::DrainDeadlineExpired {
                unfinished: expected
            }]
        );
        assert_eq!(
            supervisor.admit(BackgroundJobId(10)),
            Err(AdmissionError::Draining)
        );

        for job in jobs {
            supervisor.complete(job, None);
        }
        assert!(supervisor.unfinished().is_empty());
        assert_eq!(
            supervisor.admit(BackgroundJobId(11)),
            Err(AdmissionError::Draining)
        );
    }

    #[test]
    fn late_completion_after_expired_drain_is_recorded_without_reopening_admission() {
        let supervisor = BackgroundSupervisor::new(2, |_| {});
        let late = BackgroundJobId(8);
        supervisor.admit(late).unwrap();

        let report = supervisor.drain_until(Instant::now());
        assert_eq!(report.unfinished, vec![late]);
        assert!(report.deadline_expired);

        // Completion after timeout is legal and releases bookkeeping, but the
        // terminal drain state remains closed and the late failure is retained.
        supervisor.complete(late, Some("finished after grace".into()));
        assert!(supervisor.unfinished().is_empty());
        assert_eq!(
            supervisor.admit(BackgroundJobId(9)),
            Err(AdmissionError::Draining)
        );
        assert_eq!(
            supervisor.diagnostics(),
            vec![
                BackgroundDiagnostic::DrainDeadlineExpired {
                    unfinished: vec![late],
                },
                BackgroundDiagnostic::JobFailed(BackgroundFailure {
                    job: late,
                    message: "finished after grace".into(),
                }),
            ]
        );
    }

    #[test]
    fn empty_drain_succeeds_even_when_deadline_is_already_expired() {
        let supervisor = BackgroundSupervisor::new(1, |_| {});
        let report = supervisor.drain_until(Instant::now());

        assert_eq!(
            report,
            DrainReport {
                unfinished: Vec::new(),
                deadline_expired: false,
            }
        );
        assert!(supervisor.diagnostics().is_empty());
        assert_eq!(
            supervisor.admit(BackgroundJobId(1)),
            Err(AdmissionError::Draining)
        );
    }

    #[test]
    fn concurrent_failures_are_delivered_without_serialization_deadlock() {
        let first = BackgroundJobId(1);
        let second = BackgroundJobId(2);
        let (first_entered_tx, first_entered_rx) = std::sync::mpsc::sync_channel(1);
        let (second_entered_tx, second_entered_rx) = std::sync::mpsc::sync_channel(1);
        let (unblock_first_tx, unblock_first_rx) = std::sync::mpsc::sync_channel(1);
        let unblock_first_rx = Mutex::new(unblock_first_rx);
        let supervisor = Arc::new(BackgroundSupervisor::new(2, move |failure| {
            // The first callback waits for the second one to enter. This
            // deadlocks if complete() serializes callbacks while blocking.
            if failure.job == first {
                first_entered_tx.send(()).unwrap();
                unblock_first_rx
                    .lock()
                    .unwrap()
                    .recv_timeout(Duration::from_secs(2))
                    .unwrap();
            } else {
                second_entered_tx.send(()).unwrap();
                unblock_first_tx.send(()).unwrap();
            }
        }));
        supervisor.admit(first).unwrap();
        supervisor.admit(second).unwrap();

        let first_supervisor = Arc::clone(&supervisor);
        let first_thread = thread::spawn(move || {
            first_supervisor.complete(first, Some("first failed".into()));
        });
        first_entered_rx
            .recv_timeout(Duration::from_secs(2))
            .unwrap();

        let second_supervisor = Arc::clone(&supervisor);
        let (completed_tx, completed_rx) = std::sync::mpsc::sync_channel(1);
        let second_thread = thread::spawn(move || {
            second_supervisor.complete(second, Some("second failed".into()));
            completed_tx.send(()).unwrap();
        });
        let second_callback_entered = second_entered_rx
            .recv_timeout(Duration::from_secs(2))
            .is_ok();
        first_thread.join().unwrap();
        second_thread.join().unwrap();

        assert!(second_callback_entered);
        completed_rx.try_recv().unwrap();
        assert_eq!(
            supervisor
                .diagnostics()
                .iter()
                .filter(|d| matches!(d, BackgroundDiagnostic::JobFailed(_)))
                .count(),
            2
        );
    }
}
