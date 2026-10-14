//! Cooperative FIFO read/write lock for tasks on one executor thread.
//!
//! The host mutex protects only ownership and queue bookkeeping. It is always
//! released before a task is parked or user code is called. Readers queued
//! before a writer may be admitted as a batch; once a writer is queued, later
//! readers cannot pass it.

use std::cell::UnsafeCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Mutex;
use std::thread::{self, ThreadId};

use crate::executor::{
    current_task, current_task_cancel_requested, in_task_context, suspend_current,
    task_cancel_requested, wake_task,
};
use crate::task::{TaskId, WaitReason};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RwLockError {
    OutsideTask,
    Cancelled,
    Reentrant,
}

#[derive(Default)]
struct State {
    readers: HashSet<TaskId>,
    writer: Option<TaskId>,
    /// Pending ownership reservations, selected for wake but not yet resumed.
    /// These are deliberately separate from active holders: cancellation of
    /// a reservation must not revoke a lock already held by a running task.
    handoff: HashMap<TaskId, WaitKind>,
    /// Mixed FIFO queue. A leading run of readers may be granted together.
    waiters: VecDeque<(TaskId, WaitKind)>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum WaitKind {
    Read,
    Write,
}

pub(crate) struct CooperativeRwLock<T> {
    state: Mutex<State>,
    value: UnsafeCell<T>,
    executor_thread: ThreadId,
}

// Executor tasks are serialized on the creating OS thread. The host mutex
// never protects or lends `value`; this lock rejects operations on other
// threads before touching ownership state or the UnsafeCell.
unsafe impl<T: Send> Send for CooperativeRwLock<T> {}
unsafe impl<T: Send> Sync for CooperativeRwLock<T> {}

impl<T> CooperativeRwLock<T> {
    pub(crate) fn new(value: T) -> Self {
        Self {
            state: Mutex::new(State::default()),
            value: UnsafeCell::new(value),
            executor_thread: thread::current().id(),
        }
    }

    pub(crate) fn with_read<R>(&self, f: impl FnOnce(&T) -> R) -> Result<R, RwLockError> {
        self.with_access(WaitKind::Read, || {
            // SAFETY: the executor is pinned to one OS thread, and every
            // writer waits until the complete reader set has released. The
            // closure cannot return a borrow tied to this temporary access.
            f(unsafe { &*self.value.get() })
        })
    }

    pub(crate) fn with_write<R>(&self, f: impl FnOnce(&mut T) -> R) -> Result<R, RwLockError> {
        self.with_access(WaitKind::Write, || {
            // SAFETY: a writer is the sole recorded holder, and its RAII
            // release runs on normal return or unwind before another task is
            // handed ownership.
            f(unsafe { &mut *self.value.get() })
        })
    }

    fn with_access<R>(&self, kind: WaitKind, f: impl FnOnce() -> R) -> Result<R, RwLockError> {
        if !in_task_context() || thread::current().id() != self.executor_thread {
            return Err(RwLockError::OutsideTask);
        }

        let me = current_task();
        loop {
            let (acquired, wake, error) = {
                let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());

                // Check for reentrancy before cancellation cleanup: the task
                // may already hold this lock while observing cancellation.
                if state.handoff.get(&me) == Some(&kind) {
                    if current_task_cancel_requested() {
                        Self::remove_waiter(&mut state, me);
                        let wake = Self::dispatch(&mut state);
                        (false, wake, Some(RwLockError::Cancelled))
                    } else {
                        state.handoff.remove(&me);
                        match kind {
                            WaitKind::Read => {
                                state.readers.insert(me);
                            }
                            WaitKind::Write => state.writer = Some(me),
                        }
                        (true, Vec::new(), None)
                    }
                } else if state.writer == Some(me) || state.readers.contains(&me) {
                    return Err(RwLockError::Reentrant);
                } else if current_task_cancel_requested() {
                    Self::remove_waiter(&mut state, me);
                    let wake = Self::dispatch(&mut state);
                    (false, wake, Some(RwLockError::Cancelled))
                } else if state.waiters.is_empty()
                    && state.handoff.is_empty()
                    && match kind {
                        WaitKind::Read => state.writer.is_none(),
                        WaitKind::Write => state.writer.is_none() && state.readers.is_empty(),
                    }
                {
                    match kind {
                        WaitKind::Read => {
                            state.readers.insert(me);
                        }
                        WaitKind::Write => state.writer = Some(me),
                    }
                    (true, Vec::new(), None)
                } else {
                    if !state.waiters.iter().any(|(task, _)| *task == me) {
                        state.waiters.push_back((me, kind));
                    }
                    (false, Vec::new(), None)
                }
            };

            for task in wake {
                wake_task(task);
            }
            if let Some(error) = error {
                return Err(error);
            }
            if acquired {
                let _guard = ReleaseGuard {
                    lock: self,
                    owner: me,
                    kind,
                };
                return Ok(f());
            }
            suspend_current(WaitReason::RwLock);
        }
    }

    fn remove_waiter(state: &mut State, task: TaskId) {
        state.waiters.retain(|(waiter, _)| *waiter != task);
        state.handoff.remove(&task);
    }

    fn release(&self, owner: TaskId, kind: WaitKind) {
        let wake = {
            let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
            match kind {
                WaitKind::Read => {
                    debug_assert!(state.readers.remove(&owner));
                }
                WaitKind::Write => {
                    debug_assert_eq!(state.writer, Some(owner));
                    state.writer = None;
                }
            }
            Self::dispatch(&mut state)
        };
        for task in wake {
            wake_task(task);
        }
    }

    /// Reserves compatible FIFO waiters before waking them, preventing barging
    /// while selected tasks are still suspended in the scheduler.
    fn dispatch(state: &mut State) -> Vec<TaskId> {
        let mut wake = Vec::new();
        if state.writer.is_some() || state.handoff.values().any(|kind| *kind == WaitKind::Write) {
            return wake;
        }

        while state
            .waiters
            .front()
            .is_some_and(|(task, _)| task_cancel_requested(*task))
        {
            let (cancelled, _) = state.waiters.pop_front().expect("front was present");
            state.handoff.remove(&cancelled);
        }

        match state.waiters.front().map(|(_, kind)| *kind) {
            Some(WaitKind::Read) => {
                // Admit only the FIFO reader prefix. A queued writer blocks
                // every reader behind it, while earlier readers share access.
                while let Some((task, WaitKind::Read)) = state.waiters.front().copied() {
                    state.waiters.pop_front();
                    if task_cancel_requested(task) {
                        continue;
                    }
                    state.handoff.insert(task, WaitKind::Read);
                    wake.push(task);
                }
            }
            Some(WaitKind::Write) if state.readers.is_empty() && state.handoff.is_empty() => {
                let (task, _) = state.waiters.pop_front().expect("front was present");
                if !task_cancel_requested(task) {
                    state.handoff.insert(task, WaitKind::Write);
                    wake.push(task);
                    // A cancellation can race only at a scheduler boundary;
                    // the reserved task will remove its reservation on resume.
                } else {
                    wake.extend(Self::dispatch(state));
                }
            }
            _ => {}
        }
        wake
    }
}

struct ReleaseGuard<'a, T> {
    lock: &'a CooperativeRwLock<T>,
    owner: TaskId,
    kind: WaitKind,
}

impl<T> Drop for ReleaseGuard<'_, T> {
    fn drop(&mut self) {
        self.lock.release(self.owner, self.kind);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::executor::{Executor, cancel_current, spawn, yield_now};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    fn run(body: impl FnOnce() -> usize + Send + 'static) -> usize {
        match Executor::new().run_with_root(body) {
            crate::task::TaskOutcome::Value(value) => value,
            crate::task::TaskOutcome::Panicked(payload) => std::panic::resume_unwind(payload),
        }
    }

    fn wait_until_queued(lock: &CooperativeRwLock<impl Sized>, task: TaskId) {
        for _ in 0..32 {
            if lock
                .state
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .waiters
                .iter()
                .any(|(queued, _)| *queued == task)
            {
                return;
            }
            yield_now();
        }
        panic!("task did not queue on CooperativeRwLock");
    }

    #[test]
    fn outside_task_is_rejected() {
        let lock = CooperativeRwLock::new(7);
        assert_eq!(
            lock.with_read(|value| *value),
            Err(RwLockError::OutsideTask)
        );
        assert_eq!(
            lock.with_write(|value| *value),
            Err(RwLockError::OutsideTask)
        );
    }

    #[test]
    fn executor_on_another_os_thread_is_rejected() {
        let lock = Arc::new(CooperativeRwLock::new(7));
        let thread = std::thread::spawn(move || {
            let lock = lock.clone();
            run(move || {
                assert_eq!(
                    lock.with_read(|value| *value),
                    Err(RwLockError::OutsideTask)
                );
                assert_eq!(
                    lock.with_write(|value| *value),
                    Err(RwLockError::OutsideTask)
                );
                0
            });
        });
        thread.join().unwrap();
    }

    #[test]
    fn multiple_readers_overlap_while_writer_waits_for_all_of_them() {
        let lock = Arc::new(CooperativeRwLock::new(11));
        let active = Arc::new(AtomicUsize::new(0));
        let max_active = Arc::new(AtomicUsize::new(0));
        let writer_saw_readers = Arc::new(AtomicBool::new(false));
        let run_lock = lock.clone();
        let run_active = active.clone();
        let run_max = max_active.clone();
        let run_writer_check = writer_saw_readers.clone();

        run(move || {
            run_lock
                .with_read(|_| {
                    for _ in 0..2 {
                        let reader_lock = run_lock.clone();
                        let reader_active = run_active.clone();
                        let reader_max = run_max.clone();
                        spawn(move || {
                            reader_lock
                                .with_read(|value| {
                                    assert_eq!(*value, 11);
                                    let now = reader_active.fetch_add(1, Ordering::SeqCst) + 1;
                                    reader_max.fetch_max(now, Ordering::SeqCst);
                                    yield_now();
                                    reader_active.fetch_sub(1, Ordering::SeqCst);
                                })
                                .unwrap();
                            0
                        });
                    }
                    // Reader-compatible tasks enter concurrently and suspend
                    // while retaining their read leases.
                    yield_now();
                    assert_eq!(run_active.load(Ordering::SeqCst), 2);
                    let writer_lock = run_lock.clone();
                    let writer_active = run_active.clone();
                    let writer_check = run_writer_check.clone();
                    let writer = spawn(move || {
                        writer_lock
                            .with_write(|value| {
                                writer_check.store(
                                    writer_active.load(Ordering::SeqCst) != 0,
                                    Ordering::SeqCst,
                                );
                                *value += 1;
                            })
                            .unwrap();
                        0
                    });
                    wait_until_queued(&run_lock, writer);
                    yield_now();
                })
                .unwrap();
            0
        });

        assert_eq!(max_active.load(Ordering::SeqCst), 2);
        assert!(!writer_saw_readers.load(Ordering::SeqCst));
        assert_eq!(
            lock.with_read(|value| *value),
            Err(RwLockError::OutsideTask)
        );
    }

    #[test]
    fn queued_readers_before_writer_are_admitted_together_and_later_readers_follow_writer() {
        let lock = Arc::new(CooperativeRwLock::new(()));
        let events = Arc::new(Mutex::new(Vec::new()));
        let run_lock = lock.clone();
        let run_events = events.clone();
        run(move || {
            run_lock
                .with_write(|_| {
                    let mut readers = Vec::new();
                    for label in ["reader-1", "reader-2"] {
                        let reader_lock = run_lock.clone();
                        let reader_events = run_events.clone();
                        readers.push(spawn(move || {
                            reader_lock
                                .with_read(|_| {
                                    reader_events.lock().unwrap().push(format!("{label}-enter"));
                                    yield_now();
                                    reader_events.lock().unwrap().push(format!("{label}-exit"));
                                })
                                .unwrap();
                            0
                        }));
                    }
                    for reader in readers {
                        wait_until_queued(&run_lock, reader);
                    }
                    let writer_lock = run_lock.clone();
                    let writer_events = run_events.clone();
                    let writer = spawn(move || {
                        writer_lock
                            .with_write(|_| writer_events.lock().unwrap().push("writer".into()))
                            .unwrap();
                        0
                    });
                    wait_until_queued(&run_lock, writer);
                    let last_lock = run_lock.clone();
                    let last_events = run_events.clone();
                    let last = spawn(move || {
                        last_lock
                            .with_read(|_| last_events.lock().unwrap().push("reader-3".into()))
                            .unwrap();
                        0
                    });
                    wait_until_queued(&run_lock, last);
                    yield_now();
                })
                .unwrap();
            0
        });
        assert_eq!(
            *events.lock().unwrap(),
            vec![
                "reader-1-enter",
                "reader-2-enter",
                "reader-1-exit",
                "reader-2-exit",
                "writer",
                "reader-3"
            ]
        );
    }

    #[test]
    fn parked_lock_waiter_does_not_block_unrelated_executor_progress() {
        let lock = Arc::new(CooperativeRwLock::new(()));
        let progressed = Arc::new(AtomicBool::new(false));
        let run_lock = lock.clone();
        let run_progressed = progressed.clone();
        run(move || {
            run_lock
                .with_write(|_| {
                    let waiting_lock = run_lock.clone();
                    spawn(move || {
                        waiting_lock.with_read(|_| ()).unwrap();
                        0
                    });
                    let flag = run_progressed.clone();
                    spawn(move || {
                        flag.store(true, Ordering::SeqCst);
                        0
                    });
                    yield_now();
                    assert!(run_progressed.load(Ordering::SeqCst));
                })
                .unwrap();
            0
        });
        assert!(progressed.load(Ordering::SeqCst));
    }

    #[test]
    fn cancellation_removes_queued_reader_without_stranding_writer() {
        let lock = Arc::new(CooperativeRwLock::new(()));
        let writer_entered = Arc::new(AtomicBool::new(false));
        let run_lock = lock.clone();
        let run_entered = writer_entered.clone();
        run(move || {
            run_lock
                .with_write(|_| {
                    let cancelled_lock = run_lock.clone();
                    let reader = spawn(move || {
                        assert_eq!(
                            cancelled_lock.with_read(|_| ()),
                            Err(RwLockError::Cancelled)
                        );
                        0
                    });
                    wait_until_queued(&run_lock, reader);
                    let writer_lock = run_lock.clone();
                    let entered = run_entered.clone();
                    let writer = spawn(move || {
                        writer_lock
                            .with_write(|_| entered.store(true, Ordering::SeqCst))
                            .unwrap();
                        0
                    });
                    wait_until_queued(&run_lock, writer);
                    cancel_current(reader);
                    yield_now();
                })
                .unwrap();
            0
        });
        assert!(writer_entered.load(Ordering::SeqCst));
    }

    #[test]
    fn cancellation_removes_queued_writer_and_allows_following_reader() {
        let lock = Arc::new(CooperativeRwLock::new(()));
        let reader_entered = Arc::new(AtomicBool::new(false));
        let run_lock = lock.clone();
        let run_entered = reader_entered.clone();
        run(move || {
            run_lock
                .with_read(|_| {
                    let cancelled_lock = run_lock.clone();
                    let writer = spawn(move || {
                        assert_eq!(
                            cancelled_lock.with_write(|_| ()),
                            Err(RwLockError::Cancelled)
                        );
                        0
                    });
                    wait_until_queued(&run_lock, writer);
                    let reader_lock = run_lock.clone();
                    let entered = run_entered.clone();
                    let reader = spawn(move || {
                        reader_lock
                            .with_read(|_| entered.store(true, Ordering::SeqCst))
                            .unwrap();
                        0
                    });
                    wait_until_queued(&run_lock, reader);
                    cancel_current(writer);
                    yield_now();
                })
                .unwrap();
            0
        });
        assert!(reader_entered.load(Ordering::SeqCst));
    }

    #[test]
    fn cancellation_of_handoff_recipient_forwards_reservation() {
        let lock = Arc::new(CooperativeRwLock::new(()));
        let entered = Arc::new(AtomicBool::new(false));
        let run_lock = lock.clone();
        let run_entered = entered.clone();
        run(move || {
            let cancelled_lock = run_lock.clone();
            let cancelled = spawn(move || {
                assert_eq!(
                    cancelled_lock.with_write(|_| ()),
                    Err(RwLockError::Cancelled)
                );
                0
            });
            let following_lock = run_lock.clone();
            let following_entered = run_entered.clone();
            let following = spawn(move || {
                following_lock
                    .with_read(|_| following_entered.store(true, Ordering::SeqCst))
                    .unwrap();
                0
            });
            run_lock.with_write(|_| yield_now()).unwrap();
            cancel_current(cancelled);
            yield_now();
            let _ = following;
            0
        });
        assert!(entered.load(Ordering::SeqCst));
    }

    #[test]
    fn cancellation_of_reader_handoff_forwards_reservation() {
        let lock = Arc::new(CooperativeRwLock::new(()));
        let entered = Arc::new(AtomicBool::new(false));
        let run_lock = lock.clone();
        let run_entered = entered.clone();
        run(move || {
            let cancelled = run_lock
                .with_write(|_| {
                    let cancelled_lock = run_lock.clone();
                    let cancelled = spawn(move || {
                        assert_eq!(
                            cancelled_lock.with_read(|_| ()),
                            Err(RwLockError::Cancelled)
                        );
                        0
                    });
                    wait_until_queued(&run_lock, cancelled);
                    let following_lock = run_lock.clone();
                    let following_entered = run_entered.clone();
                    let following = spawn(move || {
                        following_lock
                            .with_write(|_| following_entered.store(true, Ordering::SeqCst))
                            .unwrap();
                        0
                    });
                    wait_until_queued(&run_lock, following);
                    cancelled
                })
                .unwrap();
            // The unlock above reserves the queued reader and makes it ready.
            // Cancel before that task gets its next scheduler turn.
            cancel_current(cancelled);
            yield_now();
            0
        });
        assert!(entered.load(Ordering::SeqCst));
    }

    #[test]
    fn panic_releases_read_and_write_guards() {
        for write in [false, true] {
            let lock = Arc::new(CooperativeRwLock::new(0));
            let completed = Arc::new(AtomicBool::new(false));
            let run_lock = lock.clone();
            let run_completed = completed.clone();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                run(move || {
                    let follower_lock = run_lock.clone();
                    let follower_completed = run_completed.clone();
                    spawn(move || {
                        follower_lock
                            .with_write(|_| follower_completed.store(true, Ordering::SeqCst))
                            .unwrap();
                        0
                    });
                    if write {
                        run_lock
                            .with_write(|_| {
                                yield_now();
                                panic!("intentional lock test panic");
                            })
                            .unwrap();
                    } else {
                        run_lock
                            .with_read(|_| {
                                yield_now();
                                panic!("intentional lock test panic");
                            })
                            .unwrap();
                    }
                    0
                })
            }));
            assert!(result.is_err());
            assert!(completed.load(Ordering::SeqCst));

            // Unwind releases either guard kind without poisoning the lock;
            // the next task can acquire write access and observe the value.
            let recovered = lock.clone();
            run(move || {
                recovered.with_write(|value| *value += 1).unwrap();
                assert_eq!(recovered.with_read(|value| *value), Ok(1));
                0
            });
        }
    }

    #[test]
    fn nested_read_or_write_acquisition_is_rejected_as_reentrant() {
        let lock = CooperativeRwLock::new(3);
        run(move || {
            lock.with_read(|_| {
                assert_eq!(lock.with_read(|_| ()), Err(RwLockError::Reentrant));
                assert_eq!(lock.with_write(|_| ()), Err(RwLockError::Reentrant));
            })
            .unwrap();
            lock.with_write(|_| {
                assert_eq!(lock.with_read(|_| ()), Err(RwLockError::Reentrant));
                assert_eq!(lock.with_write(|_| ()), Err(RwLockError::Reentrant));
            })
            .unwrap();
            0
        });
    }

    #[test]
    fn cancelled_active_reader_reentrancy_does_not_release_outer_lease() {
        let lock = Arc::new(CooperativeRwLock::new(()));
        let run_lock = lock.clone();
        run(move || {
            let child_lock = run_lock.clone();
            let child = spawn(move || {
                child_lock
                    .with_read(|_| {
                        cancel_current(current_task());
                        assert_eq!(child_lock.with_read(|_| ()), Err(RwLockError::Reentrant));
                        assert_eq!(child_lock.with_write(|_| ()), Err(RwLockError::Reentrant));
                    })
                    .unwrap();
                0
            });
            let cancelled = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                crate::executor::await_task(child)
            }));
            assert!(cancelled.is_err());
            run_lock.with_write(|_| ()).unwrap();
            0
        });
    }

    #[test]
    fn contended_rwlock_writers_preserve_fifo_across_many_rounds() {
        const ROUNDS: usize = 24;
        const WRITERS: usize = 6;
        let lock = Arc::new(CooperativeRwLock::new(()));
        let observed = Arc::new(Mutex::new(Vec::new()));
        let finished = Arc::new(AtomicUsize::new(0));
        let result = observed.clone();
        let done = finished.clone();

        run(move || {
            for round in 0..ROUNDS {
                let run_lock = lock.clone();
                let run_result = result.clone();
                let mut writers = Vec::with_capacity(WRITERS);
                run_lock
                    .with_write(|_| {
                        for index in 0..WRITERS {
                            let writer_lock = run_lock.clone();
                            let writer_result = run_result.clone();
                            let writer_done = done.clone();
                            writers.push(spawn(move || {
                                writer_lock
                                    .with_write(|_| {
                                        writer_result.lock().unwrap().push((round, index));
                                    })
                                    .unwrap();
                                writer_done.fetch_add(1, Ordering::SeqCst);
                                0
                            }));
                        }
                        for writer in &writers {
                            wait_until_queued(&run_lock, *writer);
                        }
                    })
                    .unwrap();
                let target = (round + 1) * WRITERS;
                for _ in 0..WRITERS * 8 {
                    if done.load(Ordering::SeqCst) == target {
                        break;
                    }
                    yield_now();
                }
                assert_eq!(done.load(Ordering::SeqCst), target);

                // A fresh reader must enter after the stressed FIFO batch;
                // this also detects stale handoff reservations or leaked writers.
                run_lock.with_read(|_| ()).unwrap();
            }
            0
        });

        let actual = observed.lock().unwrap().clone();
        let expected = (0..ROUNDS)
            .flat_map(|round| (0..WRITERS).map(move |index| (round, index)))
            .collect::<Vec<_>>();
        assert_eq!(actual, expected);
    }
}
