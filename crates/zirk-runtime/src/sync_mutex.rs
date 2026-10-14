//! FIFO mutex shared by cooperative executor tasks and registered native
//! workers. Task waiters park through the scheduler; only registered native
//! workers may block an OS thread while waiting.

use std::cell::UnsafeCell;
use std::collections::VecDeque;
use std::sync::{Condvar, Mutex};
use std::thread::{self, ThreadId};

use crate::executor::{
    current_task, current_task_cancel_requested, in_task_context, notify_pool_event,
    park_current_for_pool_event_if_unchanged, pool_event_epoch,
};
use crate::task::TaskId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MutexError {
    OutsideTask,
    Cancelled,
    Reentrant,
    WrongOwner,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Participant {
    Task(TaskId, ThreadId),
    Worker(ThreadId),
}

#[derive(Default)]
struct State {
    owner: Option<Participant>,
    /// FIFO head selected by unlock but not yet resumed to consume ownership.
    handoff: Option<Participant>,
    waiters: VecDeque<Participant>,
    #[cfg(test)]
    native_waiters: usize,
}

pub(crate) struct CooperativeMutex<T> {
    state: Mutex<State>,
    worker_ready: Condvar,
    value: UnsafeCell<T>,
}

// `state` establishes exclusive ownership across every participant before
// `value` is accessed. Executor tasks additionally remain non-preemptive except
// at scheduler suspension points; the OS mutex is never held across either a
// suspension or user code.
unsafe impl<T: Send> Send for CooperativeMutex<T> {}
unsafe impl<T: Send> Sync for CooperativeMutex<T> {}

impl<T> CooperativeMutex<T> {
    pub(crate) fn new(value: T) -> Self {
        Self {
            state: Mutex::new(State::default()),
            worker_ready: Condvar::new(),
            value: UnsafeCell::new(value),
        }
    }

    /// Acquires for the current task or a registered managed native worker.
    /// Tasks suspend through the executor; registered workers wait on a
    /// condition variable. Unregistered native threads are rejected.
    pub(crate) fn acquire(&self) -> Result<(), MutexError> {
        let participant = if in_task_context() {
            Participant::Task(current_task(), thread::current().id())
        } else if crate::collector::is_registered_managed_worker() {
            Participant::Worker(thread::current().id())
        } else {
            return Err(MutexError::OutsideTask);
        };
        let is_task = matches!(participant, Participant::Task(..));
        loop {
            let cancelled = is_task && current_task_cancel_requested();
            // Capture before inspecting the queue: a concurrent worker unlock
            // increments this epoch and closes the register-after-wake race.
            let event_epoch = is_task.then(pool_event_epoch);
            let (acquired, wake, error, wait_for_worker) = {
                let mut state = lock_state(&self.state);
                // A cancelled owner can call acquire again before its outer
                // scope returns. It must not be treated as a waiter while its
                // mutable access is still live.
                if state.owner == Some(participant) {
                    (false, None, Some(MutexError::Reentrant), false)
                } else if cancelled {
                    let was_front = state.waiters.front() == Some(&participant);
                    state.waiters.retain(|waiter| *waiter != participant);
                    let wake = if state.handoff == Some(participant) {
                        state.handoff = None;
                        Self::next_waiter(&mut state)
                    } else if was_front && state.owner.is_none() {
                        Self::next_waiter(&mut state)
                    } else {
                        None
                    };
                    (false, wake, Some(MutexError::Cancelled), false)
                } else {
                    // A waiter only consumes a slot when explicitly selected
                    // by unlock. This prevents newcomers from barging.
                    let acquired = if state.handoff == Some(participant) {
                        state.handoff = None;
                        state.owner = Some(participant);
                        true
                    } else if state.owner.is_none()
                        && state.handoff.is_none()
                        && state
                            .waiters
                            .front()
                            .is_none_or(|waiter| *waiter == participant)
                    {
                        if state.waiters.front() == Some(&participant) {
                            state.waiters.pop_front();
                        }
                        state.owner = Some(participant);
                        true
                    } else {
                        if !state.waiters.contains(&participant) {
                            state.waiters.push_back(participant);
                        }
                        false
                    };
                    (acquired, None, None, !is_task && !acquired)
                }
            };
            if let Some(next) = wake {
                self.wake(next);
            }
            if let Some(error) = error {
                return Err(error);
            }

            if acquired {
                return Ok(());
            }
            if wait_for_worker {
                let mut state = lock_state(&self.state);
                loop {
                    if state.handoff == Some(participant) {
                        break;
                    }
                    // Managed workers must be visible as InNative while
                    // blocked: they cannot poll GC safepoints inside condvar
                    // wait. The guard is dropped before this method retries
                    // ownership or exposes the protected payload.
                    let native = crate::collector::enter_blocking_native_current()
                        .expect("managed worker wait requires collector registration");
                    #[cfg(test)]
                    {
                        state.native_waiters += 1;
                    }
                    state = self
                        .worker_ready
                        .wait(state)
                        .unwrap_or_else(|error| error.into_inner());
                    #[cfg(test)]
                    {
                        state.native_waiters -= 1;
                    }
                    let handed_off = state.handoff == Some(participant);
                    drop(native);
                    if handed_off {
                        break;
                    }
                }
            } else {
                let epoch = event_epoch.expect("task waiter has a pool-event epoch");
                park_current_for_pool_event_if_unchanged(epoch);
            }
        }
    }

    /// Releases ownership held by the current task or registered worker.
    pub(crate) fn release(&self) -> Result<(), MutexError> {
        let owner = current_participant()?;
        let owns_mutex = {
            let state = lock_state(&self.state);
            state.owner == Some(owner)
        };
        if !owns_mutex {
            return Err(MutexError::WrongOwner);
        }
        self.unlock(owner);
        Ok(())
    }

    pub(crate) fn with<R>(&self, f: impl FnOnce(&mut T) -> R) -> Result<R, MutexError> {
        self.acquire()?;
        let _guard = UnlockGuard {
            mutex: self,
            owner: current_participant()?,
        };
        // SAFETY: acquire established exclusive ownership for this task or
        // worker. The guard releases ownership on normal return or unwind.
        Ok(f(unsafe { &mut *self.value.get() }))
    }

    fn unlock(&self, owner: Participant) {
        let wake = {
            let mut state = lock_state(&self.state);
            debug_assert_eq!(state.owner, Some(owner));
            state.owner = None;
            Self::next_waiter(&mut state)
        };
        if let Some(next) = wake {
            self.wake(next);
        }
    }

    fn wake(&self, participant: Participant) {
        match participant {
            // The executor owns its task registry on its own thread. The
            // pool-event bridge safely wakes parked tasks from native workers;
            // it also handles task-to-task handoff without stale queue entries.
            Participant::Task(_, _) => notify_pool_event(),
            Participant::Worker(_) => self.worker_ready.notify_all(),
        }
    }

    /// Selects and reserves the next waiter for FIFO handoff.
    fn next_waiter(state: &mut State) -> Option<Participant> {
        if let Some(next) = state.waiters.pop_front() {
            // The releasing participant may be a native worker, which cannot
            // inspect executor-owned TaskIds. A cancelled task is woken via
            // the pool-event bridge, observes cancellation on its own
            // executor thread, and relinquishes this reservation there.
            state.handoff = Some(next);
            return Some(next);
        }
        None
    }
}

struct UnlockGuard<'a, T> {
    mutex: &'a CooperativeMutex<T>,
    owner: Participant,
}

fn lock_state(state: &Mutex<State>) -> std::sync::MutexGuard<'_, State> {
    state.lock().unwrap_or_else(|error| error.into_inner())
}

fn current_participant() -> Result<Participant, MutexError> {
    if in_task_context() {
        Ok(Participant::Task(current_task(), thread::current().id()))
    } else if crate::collector::is_registered_managed_worker() {
        Ok(Participant::Worker(thread::current().id()))
    } else {
        Err(MutexError::OutsideTask)
    }
}

impl<T> Drop for UnlockGuard<'_, T> {
    fn drop(&mut self) {
        self.mutex.unlock(self.owner);
    }
}

#[cfg(test)]
mod tests {
    include!("sync_mutex_bench.rs");

    use super::*;
    use crate::executor::{Executor, spawn};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

    fn run(body: impl FnOnce() -> usize + Send + 'static) -> usize {
        match Executor::new().run_with_root(body) {
            crate::task::TaskOutcome::Value(value) => value,
            crate::task::TaskOutcome::Panicked(payload) => std::panic::resume_unwind(payload),
        }
    }

    #[test]
    fn outside_task_is_rejected() {
        let mutex = CooperativeMutex::new(0);
        assert_eq!(mutex.with(|value| *value), Err(MutexError::OutsideTask));
        assert_eq!(mutex.acquire(), Err(MutexError::OutsideTask));
        assert_eq!(mutex.release(), Err(MutexError::OutsideTask));
    }

    #[test]
    fn explicit_acquire_release_hands_off_to_waiter() {
        let mutex = Arc::new(CooperativeMutex::new(()));
        let entered = Arc::new(AtomicBool::new(false));
        let run_mutex = mutex.clone();
        let run_entered = entered.clone();
        run(move || {
            let waiter_mutex = run_mutex.clone();
            let waiter_entered = run_entered.clone();
            spawn(move || {
                waiter_mutex.acquire().unwrap();
                waiter_entered.store(true, Ordering::SeqCst);
                waiter_mutex.release().unwrap();
                0
            });

            run_mutex.acquire().unwrap();
            crate::executor::yield_now();
            assert!(!run_entered.load(Ordering::SeqCst));
            run_mutex.release().unwrap();
            for _ in 0..8 {
                if run_entered.load(Ordering::SeqCst) {
                    break;
                }
                crate::executor::yield_now();
            }
            assert!(run_entered.load(Ordering::SeqCst));
            0
        });
    }

    #[test]
    fn explicit_release_rejects_non_owner_and_reentrant_acquire() {
        let mutex = Arc::new(CooperativeMutex::new(()));
        let run_mutex = mutex.clone();
        run(move || {
            let other_task_mutex = run_mutex.clone();
            let other = spawn(move || {
                assert_eq!(other_task_mutex.release(), Err(MutexError::WrongOwner));
                0
            });
            run_mutex.acquire().unwrap();
            assert_eq!(run_mutex.acquire(), Err(MutexError::Reentrant));
            crate::executor::yield_now();
            assert_eq!(run_mutex.release(), Ok(()));
            let _ = other;
            0
        });
    }

    #[test]
    fn tasks_on_another_executor_thread_have_distinct_ownership() {
        let mutex = Arc::new(CooperativeMutex::new(0));
        let other_mutex = mutex.clone();
        let other_thread = std::thread::spawn(move || {
            run(move || {
                other_mutex.with(|value| *value = 1).unwrap();
                0
            })
        });
        other_thread.join().unwrap();
        assert_eq!(
            run({
                let mutex = mutex.clone();
                move || mutex.with(|value| *value).unwrap()
            }),
            1
        );
    }

    #[test]
    fn registered_worker_waits_without_blocking_executor_and_panic_releases() {
        let _collector_guard = crate::collector::test_support::test_guard();
        let mutex = Arc::new(CooperativeMutex::new(0));
        let started = Arc::new(AtomicBool::new(false));
        let done = Arc::new(AtomicBool::new(false));
        let worker_result = Arc::new(Mutex::new(None));
        let run_mutex = mutex.clone();
        let run_started = started.clone();
        let run_done = done.clone();
        let result = worker_result.clone();

        run(move || {
            run_mutex.acquire().unwrap();
            let worker_mutex = run_mutex.clone();
            let worker_started = run_started.clone();
            let worker_done = run_done.clone();
            let worker_result = result.clone();
            let worker = std::thread::spawn(move || {
                let _registration = crate::collector::register_managed_mutator();
                worker_started.store(true, Ordering::Release);
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    worker_mutex.with(|value| {
                        *value = 1;
                        panic!("expected managed worker panic");
                    })
                }));
                *worker_result.lock().unwrap() = Some(result.is_err());
                worker_mutex.with(|value| *value += 1).unwrap();
                worker_done.store(true, Ordering::Release);
            });

            // The worker is contending while this task retains ownership. A
            // scheduler yield must still run another task, proving no OS
            // mutex is held across the worker wait.
            while !run_started.load(Ordering::Acquire) {
                crate::executor::yield_now();
            }
            // Seeing this counter while acquiring `state` proves the worker
            // entered its Condvar wait (which atomically released `state`)
            // after entering the collector's InNative state.
            for _ in 0..10_000 {
                if run_mutex.state.lock().unwrap().native_waiters == 1 {
                    break;
                }
                thread::yield_now();
            }
            assert_eq!(run_mutex.state.lock().unwrap().native_waiters, 1);
            assert!(!run_done.load(Ordering::Acquire));

            // Force the executor's stop-the-world path while the worker is
            // blocked in Condvar wait and the task still owns the mutex.
            crate::collector::test_support::set_threshold_for_tests(1);
            crate::collector::maybe_collect(2);
            assert_eq!(run_mutex.state.lock().unwrap().native_waiters, 1);

            let progressed = Arc::new(AtomicBool::new(false));
            let task_progressed = progressed.clone();
            spawn(move || {
                task_progressed.store(true, Ordering::Release);
                0
            });
            crate::executor::yield_now();
            assert!(progressed.load(Ordering::Acquire));
            run_mutex.release().unwrap();
            while !run_done.load(Ordering::Acquire) {
                crate::executor::yield_now();
            }
            worker.join().unwrap();
            0
        });

        assert_eq!(*worker_result.lock().unwrap(), Some(true));
        assert_eq!(
            run({
                let mutex = mutex.clone();
                move || mutex.with(|value| *value).unwrap()
            }),
            2
        );
    }

    #[test]
    fn mixed_task_and_registered_worker_waiters_keep_fifo_order() {
        let mutex = Arc::new(CooperativeMutex::new(()));
        let order = Arc::new(Mutex::new(Vec::new()));
        let worker_done = Arc::new(AtomicBool::new(false));
        let run_mutex = mutex.clone();
        let run_order = order.clone();
        let run_worker_done = worker_done.clone();

        run(move || {
            run_mutex.acquire().unwrap();
            let worker_mutex = run_mutex.clone();
            let worker_order = run_order.clone();
            let worker_done = run_worker_done.clone();
            let worker = std::thread::spawn(move || {
                let _registration = crate::collector::register_managed_mutator();
                worker_mutex
                    .with(|_| worker_order.lock().unwrap().push("worker"))
                    .unwrap();
                worker_done.store(true, Ordering::Release);
            });

            // Ensure the worker is first in the FIFO before enqueueing the
            // executor task behind it.
            for _ in 0..1000 {
                if run_mutex
                    .state
                    .lock()
                    .unwrap()
                    .waiters
                    .iter()
                    .any(|waiter| matches!(waiter, Participant::Worker(_)))
                {
                    break;
                }
                crate::executor::yield_now();
            }
            assert!(
                run_mutex
                    .state
                    .lock()
                    .unwrap()
                    .waiters
                    .iter()
                    .any(|waiter| matches!(waiter, Participant::Worker(_)),)
            );

            let task_mutex = run_mutex.clone();
            let task_order = run_order.clone();
            spawn(move || {
                task_mutex
                    .with(|_| task_order.lock().unwrap().push("task"))
                    .unwrap();
                0
            });
            for _ in 0..100 {
                if run_mutex
                    .state
                    .lock()
                    .unwrap()
                    .waiters
                    .iter()
                    .any(|waiter| matches!(waiter, Participant::Task(..)))
                {
                    break;
                }
                crate::executor::yield_now();
            }
            assert!(
                run_mutex
                    .state
                    .lock()
                    .unwrap()
                    .waiters
                    .iter()
                    .any(|waiter| matches!(waiter, Participant::Task(..)),)
            );
            run_mutex.release().unwrap();

            for _ in 0..1000 {
                if run_worker_done.load(Ordering::Acquire) && run_order.lock().unwrap().len() == 2 {
                    break;
                }
                crate::executor::yield_now();
            }
            assert!(run_worker_done.load(Ordering::Acquire));
            assert_eq!(run_order.lock().unwrap().len(), 2);
            worker.join().unwrap();
            0
        });

        assert_eq!(*order.lock().unwrap(), vec!["worker", "task"]);
    }

    #[test]
    fn contended_waiters_enter_in_fifo_order() {
        let mutex = Arc::new(CooperativeMutex::new(Vec::<usize>::new()));
        let observed = Arc::new(Mutex::new(Vec::new()));
        let result = observed.clone();
        run(move || {
            let first_lock = mutex.clone();
            let first_result = result.clone();
            let first = spawn(move || {
                first_lock
                    .with(|_| first_result.lock().unwrap().push(1))
                    .unwrap();
                0
            });
            let second_lock = mutex.clone();
            let second_result = result.clone();
            let second = spawn(move || {
                second_lock
                    .with(|_| second_result.lock().unwrap().push(2))
                    .unwrap();
                0
            });
            let third_lock = mutex.clone();
            let third_result = result.clone();
            let third = spawn(move || {
                third_lock
                    .with(|_| third_result.lock().unwrap().push(3))
                    .unwrap();
                0
            });
            // Root enters first and yields while retaining the lock, allowing
            // children to queue in spawn order before it releases.
            mutex
                .with(|v| {
                    v.push(0);
                    crate::executor::yield_now();
                })
                .unwrap();
            let _ = (first, second, third);
            0
        });
        assert_eq!(*observed.lock().unwrap(), vec![1, 2, 3]);
    }

    #[test]
    fn unrelated_task_progresses_while_mutex_waiter_is_parked() {
        let mutex = Arc::new(CooperativeMutex::new(()));
        let progressed = Arc::new(AtomicBool::new(false));
        let result = progressed.clone();
        let lock_for_child = mutex.clone();
        run(move || {
            // The root holds the mutex while the child attempts acquisition;
            // a second child must still run before the root releases it.
            let waiting_lock = lock_for_child.clone();
            spawn(move || {
                waiting_lock.with(|_| ()).unwrap();
                0
            });
            let flag = result.clone();
            spawn(move || {
                flag.store(true, Ordering::SeqCst);
                0
            });
            mutex
                .with(|_| {
                    crate::executor::yield_now();
                    assert!(result.load(Ordering::SeqCst));
                })
                .unwrap();
            0
        });
    }

    #[test]
    fn cancellation_of_handoff_recipient_does_not_strand_next_waiter() {
        let mutex = Arc::new(CooperativeMutex::new(()));
        let completed = Arc::new(AtomicBool::new(false));
        let done = completed.clone();
        let run_mutex = mutex.clone();
        run(move || {
            let cancelled_mutex = run_mutex.clone();
            let cancelled = spawn(move || {
                let result = cancelled_mutex.with(|_| ());
                assert_eq!(result, Err(MutexError::Cancelled));
                0
            });
            let next_mutex = run_mutex.clone();
            spawn(move || {
                next_mutex
                    .with(|_| done.store(true, Ordering::SeqCst))
                    .unwrap();
                0
            });
            run_mutex.with(|_| crate::executor::yield_now()).unwrap();
            // Unlock has reserved the FIFO head and made it ready. Cancel it
            // before its next turn so it must relinquish that reservation.
            crate::executor::cancel_current(cancelled);
            crate::executor::yield_now();
            0
        });
        assert!(completed.load(Ordering::SeqCst));
    }

    #[test]
    fn explicit_acquire_removes_cancelled_waiter_without_stranding_next_waiter() {
        let mutex = Arc::new(CooperativeMutex::new(()));
        let cancelled_result = Arc::new(Mutex::new(None));
        let completed = Arc::new(AtomicBool::new(false));
        let run_mutex = mutex.clone();
        let run_result = cancelled_result.clone();
        let run_completed = completed.clone();
        run(move || {
            run_mutex.acquire().unwrap();
            let waiting_mutex = run_mutex.clone();
            let waiting_result = run_result.clone();
            let cancelled = spawn(move || {
                *waiting_result.lock().unwrap() = Some(waiting_mutex.acquire());
                0
            });
            let next_mutex = run_mutex.clone();
            let next_completed = run_completed.clone();
            spawn(move || {
                next_mutex.acquire().unwrap();
                next_completed.store(true, Ordering::SeqCst);
                next_mutex.release().unwrap();
                0
            });

            // Let both children run in spawn order: the first parks in the
            // mutex queue, and the second queues behind it.
            crate::executor::yield_now();
            crate::executor::yield_now();
            crate::executor::cancel_current(cancelled);
            run_mutex.release().unwrap();
            crate::executor::yield_now();
            0
        });
        assert_eq!(
            *cancelled_result.lock().unwrap(),
            Some(Err(MutexError::Cancelled))
        );
        assert!(completed.load(Ordering::SeqCst));
    }

    #[test]
    fn panic_unlocks_mutex_for_next_task() {
        let mutex = Arc::new(CooperativeMutex::new(0));
        let done = Arc::new(AtomicBool::new(false));
        let run_mutex = mutex.clone();
        let result = done.clone();
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            run(move || {
                let following_mutex = run_mutex.clone();
                spawn(move || {
                    following_mutex
                        .with(|_| result.store(true, Ordering::SeqCst))
                        .unwrap();
                    0
                });
                run_mutex
                    .with(|_| {
                        crate::executor::yield_now();
                        panic!("expected test panic");
                    })
                    .unwrap();
                0
            })
        }));
        assert!(outcome.is_err());
        assert!(done.load(Ordering::SeqCst));

        // The private cooperative mutex has no poisoning state: after unwind
        // releases the guard, a later task can acquire and update it.
        let recovered = mutex.clone();
        run(move || {
            recovered.with(|value| *value = 1).unwrap();
            assert_eq!(recovered.with(|value| *value), Ok(1));
            0
        });
    }

    #[test]
    fn reentrant_acquisition_returns_explicit_error() {
        let mutex = CooperativeMutex::new(0);
        run(move || {
            mutex
                .with(|_| assert_eq!(mutex.with(|_| ()), Err(MutexError::Reentrant)))
                .unwrap();
            0
        });
    }

    #[test]
    fn cancellation_of_active_owner_cannot_release_its_live_mutable_view() {
        let mutex = Arc::new(CooperativeMutex::new(0));
        let owner_bits = Arc::new(AtomicU64::new(0));
        let waiter_started = Arc::new(AtomicBool::new(false));
        let waiter_entered = Arc::new(AtomicBool::new(false));
        let run_mutex = mutex.clone();
        let run_owner_bits = owner_bits.clone();
        let run_waiter_started = waiter_started.clone();
        let run_waiter_entered = waiter_entered.clone();

        run(move || {
            spawn(move || {
                run_owner_bits.store(current_task().to_bits(), Ordering::SeqCst);
                let waiter_mutex = run_mutex.clone();
                let waiter_started = run_waiter_started.clone();
                let waiter_entered = run_waiter_entered.clone();
                spawn(move || {
                    waiter_started.store(true, Ordering::SeqCst);
                    waiter_mutex
                        .with(|value| {
                            assert_eq!(*value, 2);
                            waiter_entered.store(true, Ordering::SeqCst);
                        })
                        .unwrap();
                    0
                });

                let cancel_owner_bits = run_owner_bits.clone();
                let cancel_waiter_started = run_waiter_started.clone();
                spawn(move || {
                    while !cancel_waiter_started.load(Ordering::SeqCst) {
                        crate::executor::yield_now();
                    }
                    crate::executor::cancel_current(TaskId::from_bits(
                        cancel_owner_bits.load(Ordering::SeqCst),
                    ));
                    0
                });

                run_mutex
                    .with(|value| {
                        *value = 1;
                        crate::executor::yield_now();
                        assert_eq!(run_mutex.with(|_| ()), Err(MutexError::Reentrant));
                        crate::executor::yield_now();
                        assert!(
                            !run_waiter_entered.load(Ordering::SeqCst),
                            "cancellation must not release the active owner's mutable view"
                        );
                        *value = 2;
                    })
                    .unwrap();
                0
            });
            0
        });

        assert!(waiter_entered.load(Ordering::SeqCst));
        run(move || {
            assert_eq!(mutex.with(|value| *value), Ok(2));
            0
        });
    }

    #[test]
    fn contended_mutex_handoff_preserves_fifo_across_many_rounds() {
        const ROUNDS: usize = 32;
        const WAITERS: usize = 8;
        let mutex = Arc::new(CooperativeMutex::new(()));
        let observed = Arc::new(Mutex::new(Vec::new()));
        let finished = Arc::new(AtomicU64::new(0));
        let result = observed.clone();
        let done = finished.clone();

        run(move || {
            for round in 0..ROUNDS {
                let lock = mutex.clone();
                let order = result.clone();
                let mut tasks = Vec::with_capacity(WAITERS);
                lock.with(|_| {
                    for index in 0..WAITERS {
                        let waiter_lock = lock.clone();
                        let waiter_order = order.clone();
                        let waiter_done = done.clone();
                        tasks.push(spawn(move || {
                            waiter_lock
                                .with(|_| {
                                    waiter_order.lock().unwrap().push((round, index));
                                })
                                .unwrap();
                            waiter_done.fetch_add(1, Ordering::SeqCst);
                            0
                        }));
                    }
                    for task in &tasks {
                        for _ in 0..32 {
                            if lock.state.lock().unwrap().waiters.iter().any(
                                |waiter| matches!(waiter, Participant::Task(id, _) if id == task),
                            ) {
                                break;
                            }
                            crate::executor::yield_now();
                        }
                        assert!(lock.state.lock().unwrap().waiters.iter().any(
                            |waiter| matches!(waiter, Participant::Task(id, _) if id == task)
                        ));
                    }
                })
                .unwrap();

                // Every queued waiter must finish before the next round.
                let target = ((round + 1) * WAITERS) as u64;
                for _ in 0..WAITERS * 8 {
                    if done.load(Ordering::SeqCst) == target {
                        break;
                    }
                    crate::executor::yield_now();
                }
                assert_eq!(done.load(Ordering::SeqCst), target);
            }
            0
        });

        let actual = observed.lock().unwrap().clone();
        let expected = (0..ROUNDS)
            .flat_map(|round| (0..WAITERS).map(move |index| (round, index)))
            .collect::<Vec<_>>();
        assert_eq!(actual, expected);
    }
}
