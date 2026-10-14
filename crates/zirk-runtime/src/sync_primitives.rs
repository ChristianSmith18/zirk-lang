//! Runtime-only cooperative synchronization primitives.
//!
//! This module implements private cooperative synchronization foundations.
//! Host mutexes protect queue bookkeeping and are never held while parking a
//! task or running caller code. Language and ABI integration remain gated.

use std::collections::{HashSet, VecDeque};
use std::marker::PhantomData;
use std::rc::Rc;
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::{self, ThreadId};

use crate::executor::{
    current_task, current_task_cancel_requested, in_task_context, suspend_current,
    task_cancel_requested, wake_task,
};
use crate::task::{TaskId, WaitReason};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SemaphoreError {
    InvalidBounds,
    OutsideTask,
    Cancelled,
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BarrierError {
    ZeroParticipants,
    OutsideTask,
    Cancelled,
    Broken,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BarrierWait {
    Leader,
    Follower,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OnceError {
    OutsideTask,
    Cancelled,
    Reentrant,
}

#[derive(Default)]
struct State {
    available: usize,
    waiters: VecDeque<TaskId>,
    /// Permits reserved for awakened tasks that have not resumed to consume
    /// their reservation yet. Reservations remain unavailable to newcomers.
    granted: HashSet<TaskId>,
    closed: bool,
}

struct Inner {
    state: Mutex<State>,
    maximum: usize,
    executor_thread: ThreadId,
}

/// A bounded, FIFO semaphore for cooperative tasks.
pub(crate) struct CooperativeSemaphore {
    inner: Arc<Inner>,
}

/// An acquired permit returns exactly one slot when dropped.
///
/// The `Rc` marker keeps a permit on its owning executor thread, so dropping it
/// can safely wake a task in that executor.
pub(crate) struct SemaphorePermit {
    inner: Arc<Inner>,
    _not_send: PhantomData<Rc<()>>,
}

impl CooperativeSemaphore {
    pub(crate) fn new(maximum: usize, initial: usize) -> Result<Self, SemaphoreError> {
        if initial > maximum {
            return Err(SemaphoreError::InvalidBounds);
        }
        Ok(Self {
            inner: Arc::new(Inner {
                state: Mutex::new(State {
                    available: initial,
                    ..State::default()
                }),
                maximum,
                executor_thread: thread::current().id(),
            }),
        })
    }

    /// Acquires one permit, waiting FIFO through the cooperative scheduler.
    pub(crate) fn acquire(&self) -> Result<SemaphorePermit, SemaphoreError> {
        if !self.on_executor_thread() {
            return Err(SemaphoreError::OutsideTask);
        }
        let me = current_task();
        loop {
            let (acquired, wake, error) = {
                let mut state = self.inner.state.lock().unwrap_or_else(|e| e.into_inner());
                if state.closed {
                    state.waiters.retain(|task| *task != me);
                    state.granted.remove(&me);
                    (false, Vec::new(), Some(SemaphoreError::Closed))
                } else if state.granted.remove(&me) {
                    if current_task_cancel_requested() {
                        let wake = Self::return_reserved_permit(&self.inner, &mut state);
                        (false, wake, Some(SemaphoreError::Cancelled))
                    } else {
                        (true, Vec::new(), None)
                    }
                } else if current_task_cancel_requested() {
                    state.waiters.retain(|task| *task != me);
                    let wake = Self::dispatch(&mut state);
                    (false, wake, Some(SemaphoreError::Cancelled))
                } else if state.waiters.is_empty() && state.available > 0 {
                    state.available -= 1;
                    (true, Vec::new(), None)
                } else {
                    if !state.waiters.contains(&me) {
                        state.waiters.push_back(me);
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
                return Ok(SemaphorePermit {
                    inner: Arc::clone(&self.inner),
                    _not_send: PhantomData,
                });
            }
            suspend_current(WaitReason::Semaphore);
        }
    }

    /// Attempts an immediate acquisition. It never parks or bypasses queued
    /// tasks.
    pub(crate) fn try_acquire(&self) -> Result<Option<SemaphorePermit>, SemaphoreError> {
        if !self.on_executor_thread() {
            return Err(SemaphoreError::OutsideTask);
        }
        if current_task_cancel_requested() {
            return Err(SemaphoreError::Cancelled);
        }
        let mut state = self.inner.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.closed {
            return Err(SemaphoreError::Closed);
        }
        if !state.waiters.is_empty() || !state.granted.is_empty() || state.available == 0 {
            return Ok(None);
        }
        state.available -= 1;
        Ok(Some(SemaphorePermit {
            inner: Arc::clone(&self.inner),
            _not_send: PhantomData,
        }))
    }

    /// Permanently closes admission and wakes queued and reserved waiters.
    pub(crate) fn close(&self) -> Result<(), SemaphoreError> {
        if !self.on_executor_thread() {
            return Err(SemaphoreError::OutsideTask);
        }
        let wake = {
            let mut state = self.inner.state.lock().unwrap_or_else(|e| e.into_inner());
            if state.closed {
                return Ok(());
            }
            state.closed = true;
            let mut wake = state.waiters.drain(..).collect::<Vec<_>>();
            wake.extend(state.granted.drain());
            wake
        };
        for task in wake {
            wake_task(task);
        }
        Ok(())
    }

    fn on_executor_thread(&self) -> bool {
        in_task_context() && thread::current().id() == self.inner.executor_thread
    }

    /// Reserves the available permit for the oldest live waiter before waking
    /// it, preventing a newly arriving task from barging.
    fn dispatch(state: &mut State) -> Vec<TaskId> {
        if state.closed || state.available == 0 {
            return Vec::new();
        }
        while let Some(task) = state.waiters.pop_front() {
            if task_cancel_requested(task) {
                continue;
            }
            state.available -= 1;
            state.granted.insert(task);
            return vec![task];
        }
        Vec::new()
    }

    fn return_reserved_permit(inner: &Inner, state: &mut State) -> Vec<TaskId> {
        if state.closed {
            return Vec::new();
        }
        state.available = (state.available + 1).min(inner.maximum);
        Self::dispatch(state)
    }

    fn release(inner: &Inner) {
        let wake = {
            let mut state = inner.state.lock().unwrap_or_else(|e| e.into_inner());
            if state.closed {
                return;
            }
            state.available = (state.available + 1).min(inner.maximum);
            Self::dispatch(&mut state)
        };
        for task in wake {
            wake_task(task);
        }
    }
}

impl Drop for SemaphorePermit {
    fn drop(&mut self) {
        CooperativeSemaphore::release(&self.inner);
    }
}

#[derive(Default)]
struct BarrierState {
    generation: u64,
    arrived: usize,
    waiters: VecDeque<TaskId>,
    broken: bool,
}

struct BarrierInner {
    state: Mutex<BarrierState>,
    participants: usize,
    executor_thread: ThreadId,
}

/// A fixed-party barrier for tasks on one cooperative executor.
pub(crate) struct CooperativeBarrier {
    inner: Arc<BarrierInner>,
}

impl CooperativeBarrier {
    pub(crate) fn new(participants: usize) -> Result<Self, BarrierError> {
        if participants == 0 {
            return Err(BarrierError::ZeroParticipants);
        }
        Ok(Self {
            inner: Arc::new(BarrierInner {
                state: Mutex::new(BarrierState::default()),
                participants,
                executor_thread: thread::current().id(),
            }),
        })
    }

    /// Arrives at the current generation and parks until it completes.
    /// The final arrival is the sole leader. Cancellation of a parked
    /// participant permanently breaks the barrier.
    pub(crate) fn wait(&self) -> Result<BarrierWait, BarrierError> {
        if !self.on_executor_thread() {
            return Err(BarrierError::OutsideTask);
        }
        let me = current_task();
        let (result, wake, waiting_generation) = {
            let mut state = self.inner.state.lock().unwrap_or_else(|e| e.into_inner());
            if state.broken {
                (Some(Err(BarrierError::Broken)), Vec::new(), None)
            } else if current_task_cancel_requested() {
                if state.waiters.contains(&me) {
                    state.broken = true;
                    state.arrived = 0;
                    let wake = state.waiters.drain(..).collect();
                    (Some(Err(BarrierError::Broken)), wake, None)
                } else {
                    // A cancelled task that has not joined this barrier
                    // generation must not poison participants who arrive
                    // later.
                    (Some(Err(BarrierError::Cancelled)), Vec::new(), None)
                }
            } else {
                // Cancellation is delivered cooperatively. Check queued
                // participants before admitting the final arrival so a
                // cancelled waiter cannot be silently counted as present.
                let cancelled_waiter = state
                    .waiters
                    .iter()
                    .any(|task| task_cancel_requested(*task));
                if cancelled_waiter {
                    state.broken = true;
                    state.arrived = 0;
                    let wake = state.waiters.drain(..).collect();
                    (Some(Err(BarrierError::Broken)), wake, None)
                } else {
                    let generation = state.generation;
                    state.arrived += 1;
                    if state.arrived == self.inner.participants {
                        state.arrived = 0;
                        state.generation = state.generation.wrapping_add(1);
                        let wake = state.waiters.drain(..).collect();
                        (Some(Ok(BarrierWait::Leader)), wake, None)
                    } else {
                        if !state.waiters.contains(&me) {
                            state.waiters.push_back(me);
                        }
                        (None, Vec::new(), Some(generation))
                    }
                }
            }
        };
        for task in wake {
            wake_task(task);
        }
        if let Some(result) = result {
            return result;
        }
        suspend_current(WaitReason::Barrier);
        let state = self.inner.state.lock().unwrap_or_else(|e| e.into_inner());
        let expected_generation = waiting_generation.expect("parked barrier generation");
        // Completion belongs to the generation this call joined. A later
        // generation may already be broken before this task resumes.
        if state.generation != expected_generation {
            return Ok(BarrierWait::Follower);
        }
        if state.broken {
            return Err(BarrierError::Broken);
        }
        if current_task_cancel_requested() {
            drop(state);
            let wake = {
                let mut state = self.inner.state.lock().unwrap_or_else(|e| e.into_inner());
                state.broken = true;
                state.arrived = 0;
                state.waiters.retain(|task| *task != me);
                state.waiters.drain(..).collect::<Vec<_>>()
            };
            for task in wake {
                wake_task(task);
            }
            return Err(BarrierError::Broken);
        }
        // A generation completed. Its waiters were drained and woken;
        // the post-wake state is either the next generation or broken.
        Ok(BarrierWait::Follower)
    }

    fn on_executor_thread(&self) -> bool {
        in_task_context() && thread::current().id() == self.inner.executor_thread
    }
}

#[derive(Default)]
struct OnceState {
    initializing: Option<TaskId>,
    waiters: VecDeque<TaskId>,
}

struct OnceInner<T> {
    state: Mutex<OnceState>,
    value: OnceLock<T>,
    executor_thread: ThreadId,
}

/// A retryable once cell whose value is exposed only to a scoped read closure.
pub(crate) struct CooperativeOnce<T> {
    inner: Arc<OnceInner<T>>,
}

impl<T: Send + Sync + 'static> CooperativeOnce<T> {
    pub(crate) fn new() -> Self {
        Self {
            inner: Arc::new(OnceInner {
                state: Mutex::new(OnceState::default()),
                value: OnceLock::new(),
                executor_thread: thread::current().id(),
            }),
        }
    }

    /// Initializes once and passes the published value to `read` without
    /// holding the host bookkeeping mutex. A panicking initializer leaves the
    /// cell empty and wakes the oldest waiter to retry.
    pub(crate) fn get_or_init<R>(
        &self,
        initialize: impl FnOnce() -> T,
        read: impl FnOnce(&T) -> R,
    ) -> Result<R, OnceError> {
        if !in_task_context() || thread::current().id() != self.inner.executor_thread {
            return Err(OnceError::OutsideTask);
        }
        let me = current_task();
        let mut initialize = Some(initialize);
        loop {
            enum Action {
                Read,
                Initialize,
                Wait,
                Error(OnceError, Option<TaskId>),
            }
            let action = {
                let mut state = self.inner.state.lock().unwrap_or_else(|e| e.into_inner());
                if current_task_cancel_requested() {
                    let was_head = state.waiters.front() == Some(&me);
                    state.waiters.retain(|task| *task != me);
                    let wake = if was_head && state.initializing.is_none() {
                        Self::next_live_waiter(&mut state)
                    } else {
                        None
                    };
                    Action::Error(OnceError::Cancelled, wake)
                } else if self.inner.value.get().is_some() {
                    state.waiters.retain(|task| *task != me);
                    Action::Read
                } else if state.initializing == Some(me) {
                    // A Once initializer may not recursively initialize itself.
                    // Treat this as a runtime contract error rather than deadlock.
                    Action::Error(OnceError::Reentrant, None)
                } else if state.initializing.is_none() {
                    if state.waiters.front().is_some_and(|task| *task != me) {
                        if !state.waiters.contains(&me) {
                            state.waiters.push_back(me);
                        }
                        Action::Wait
                    } else {
                        state.waiters.retain(|task| *task != me);
                        state.initializing = Some(me);
                        Action::Initialize
                    }
                } else {
                    if !state.waiters.contains(&me) {
                        state.waiters.push_back(me);
                    }
                    Action::Wait
                }
            };
            match action {
                Action::Read => {
                    let value = self.inner.value.get().expect("ready Once value");
                    return Ok(read(value));
                }
                Action::Initialize => {
                    let mut guard = OnceInitGuard {
                        inner: Arc::clone(&self.inner),
                        owner: me,
                        armed: true,
                    };
                    let value = initialize.take().expect("initializer runs once")();
                    let _ = self.inner.value.set(value);
                    let wake = {
                        let mut state = self.inner.state.lock().unwrap_or_else(|e| e.into_inner());
                        state.initializing = None;
                        guard.armed = false;
                        state.waiters.drain(..).collect::<Vec<_>>()
                    };
                    for task in wake {
                        wake_task(task);
                    }
                    let value = self.inner.value.get().expect("published Once value");
                    return Ok(read(value));
                }
                Action::Wait => suspend_current(WaitReason::Once),
                Action::Error(error, wake) => {
                    if let Some(task) = wake {
                        wake_task(task);
                    }
                    return Err(error);
                }
            }
        }
    }
}

struct OnceInitGuard<T> {
    inner: Arc<OnceInner<T>>,
    owner: TaskId,
    armed: bool,
}

impl<T> Drop for OnceInitGuard<T> {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        let wake = {
            let mut state = self.inner.state.lock().unwrap_or_else(|e| e.into_inner());
            if state.initializing != Some(self.owner) {
                return;
            }
            state.initializing = None;
            CooperativeOnce::<T>::next_live_waiter(&mut state)
        };
        if let Some(task) = wake {
            wake_task(task);
        }
    }
}

impl<T> CooperativeOnce<T> {
    /// Prunes cancelled queue heads and reserves the FIFO head by leaving it
    /// in place until it resumes and claims initialization.
    fn next_live_waiter(state: &mut OnceState) -> Option<TaskId> {
        while state
            .waiters
            .front()
            .is_some_and(|task| task_cancel_requested(*task))
        {
            state.waiters.pop_front();
        }
        state.waiters.front().copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::executor::{Executor, cancel_current, spawn, yield_now};
    use std::panic::{AssertUnwindSafe, catch_unwind};
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    fn run(body: impl FnOnce() -> usize + Send + 'static) -> usize {
        match Executor::new().run_with_root(body) {
            crate::task::TaskOutcome::Value(value) => value,
            crate::task::TaskOutcome::Panicked(payload) => std::panic::resume_unwind(payload),
        }
    }

    fn run_with_timeout(body: impl FnOnce() -> usize + Send + 'static) -> usize {
        let (done, result) = std::sync::mpsc::sync_channel(1);
        std::thread::spawn(move || {
            let _ = done.send(run(body));
        });
        result
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("cooperative synchronization stress exceeded five seconds")
    }

    fn wait_until_queued(semaphore: &CooperativeSemaphore, task: TaskId) {
        for _ in 0..32 {
            if semaphore
                .inner
                .state
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .waiters
                .contains(&task)
            {
                return;
            }
            yield_now();
        }
        panic!("task did not queue on semaphore");
    }

    fn wait_until_barrier_queued(barrier: &CooperativeBarrier, task: TaskId) {
        for _ in 0..32 {
            if barrier
                .inner
                .state
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .waiters
                .contains(&task)
            {
                return;
            }
            yield_now();
        }
        panic!("task did not queue on barrier");
    }

    fn wait_until_once_queued<T>(once: &CooperativeOnce<T>, task: TaskId) {
        for _ in 0..32 {
            if once
                .inner
                .state
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .waiters
                .contains(&task)
            {
                return;
            }
            yield_now();
        }
        panic!("task did not queue on once cell");
    }

    #[test]
    fn barrier_has_one_leader_per_generation() {
        const GENERATIONS: usize = 64;
        let barrier = Arc::new(CooperativeBarrier::new(4).unwrap());
        let leaders = Arc::new(AtomicUsize::new(0));
        run({
            let barrier = barrier.clone();
            let leaders = leaders.clone();
            move || {
                for _ in 0..3 {
                    let barrier = barrier.clone();
                    let leaders = leaders.clone();
                    spawn(move || {
                        for _ in 0..GENERATIONS {
                            if barrier.wait().unwrap() == BarrierWait::Leader {
                                leaders.fetch_add(1, Ordering::SeqCst);
                            }
                        }
                        0
                    });
                }
                for _ in 0..GENERATIONS {
                    if barrier.wait().unwrap() == BarrierWait::Leader {
                        leaders.fetch_add(1, Ordering::SeqCst);
                    }
                }
                0
            }
        });
        assert_eq!(leaders.load(Ordering::SeqCst), GENERATIONS);
    }

    #[test]
    fn barrier_cancellation_breaks_and_wakes_the_generation() {
        let barrier = Arc::new(CooperativeBarrier::new(3).unwrap());
        let outcomes = Arc::new(Mutex::new(Vec::new()));
        let observed = outcomes.clone();
        run(move || {
            let mut tasks = Vec::new();
            for _ in 0..2 {
                let barrier = barrier.clone();
                let observed = observed.clone();
                tasks.push(spawn(move || {
                    let outcome = barrier.wait();
                    observed.lock().unwrap().push(outcome);
                    0
                }));
            }
            for task in &tasks {
                wait_until_barrier_queued(&barrier, *task);
            }
            cancel_current(tasks[0]);
            yield_now();
            assert!(matches!(barrier.wait(), Err(BarrierError::Broken)));
            0
        });
        let outcomes = outcomes.lock().unwrap();
        assert_eq!(outcomes.len(), 2);
        assert!(
            outcomes
                .iter()
                .all(|outcome| *outcome == Err(BarrierError::Broken))
        );
    }

    #[test]
    fn barrier_repeated_cancellation_never_strands_a_generation_waiter() {
        const ROUNDS: usize = 32;
        let outcomes = Arc::new(Mutex::new(Vec::new()));
        let observed = outcomes.clone();
        run(move || {
            for _ in 0..ROUNDS {
                let barrier = Arc::new(CooperativeBarrier::new(3).unwrap());
                let mut tasks = Vec::new();
                for _ in 0..2 {
                    let barrier = barrier.clone();
                    let observed = observed.clone();
                    tasks.push(spawn(move || {
                        let outcome = barrier.wait();
                        observed.lock().unwrap().push(outcome);
                        0
                    }));
                }
                for task in &tasks {
                    wait_until_barrier_queued(&barrier, *task);
                }
                cancel_current(tasks[0]);
                yield_now();
                assert_eq!(barrier.wait(), Err(BarrierError::Broken));
                yield_now();
            }
            0
        });
        let outcomes = outcomes.lock().unwrap();
        assert_eq!(outcomes.len(), 2 * ROUNDS);
        assert!(
            outcomes
                .iter()
                .all(|outcome| *outcome == Err(BarrierError::Broken))
        );
    }

    #[test]
    fn completed_barrier_generation_keeps_its_result_if_a_later_one_breaks() {
        let barrier = Arc::new(CooperativeBarrier::new(2).unwrap());
        let results = Arc::new(Mutex::new(Vec::new()));
        let observed = results.clone();
        run(move || {
            let waiting_barrier = barrier.clone();
            let waiting_results = observed.clone();
            let waiter = spawn(move || {
                let outcome = waiting_barrier.wait();
                waiting_results.lock().unwrap().push(outcome);
                0
            });
            wait_until_barrier_queued(&barrier, waiter);
            {
                // Model the race where generation N completes, then N+1 is
                // broken before this already-released waiter gets scheduled.
                let mut state = barrier.inner.state.lock().unwrap();
                state.generation = state.generation.wrapping_add(1);
                state.arrived = 0;
                state.broken = true;
                state.waiters.clear();
            }
            wake_task(waiter);
            yield_now();
            0
        });
        assert_eq!(*results.lock().unwrap(), vec![Ok(BarrierWait::Follower)]);
    }

    #[test]
    fn barrier_rejects_zero_participants_and_outside_executor_calls() {
        assert!(matches!(
            CooperativeBarrier::new(0),
            Err(BarrierError::ZeroParticipants)
        ));
        let barrier = CooperativeBarrier::new(1).unwrap();
        assert_eq!(barrier.wait(), Err(BarrierError::OutsideTask));
    }

    #[test]
    fn pre_cancelled_nonparticipant_does_not_break_barrier() {
        let barrier = Arc::new(CooperativeBarrier::new(1).unwrap());
        let observed = Arc::new(Mutex::new(None));
        let result = observed.clone();
        run(move || {
            let child_barrier = barrier.clone();
            let child_result = result.clone();
            let child = spawn(move || {
                let outcome = child_barrier.wait();
                *child_result.lock().unwrap() = Some(outcome);
                0
            });
            cancel_current(child);
            assert_eq!(barrier.wait(), Ok(BarrierWait::Leader));
            0
        });
        assert_eq!(
            *observed.lock().unwrap(),
            Some(Err(BarrierError::Cancelled))
        );
    }

    #[test]
    fn once_publishes_one_value_to_concurrent_callers() {
        const CALLERS: usize = 64;
        let once = Arc::new(CooperativeOnce::<usize>::new());
        let calls = Arc::new(AtomicUsize::new(0));
        let observed = calls.clone();
        run(move || {
            for _ in 0..CALLERS {
                let once = once.clone();
                let calls = observed.clone();
                spawn(move || {
                    let value = once
                        .get_or_init(
                            || {
                                calls.fetch_add(1, Ordering::SeqCst);
                                yield_now();
                                73
                            },
                            |value| *value,
                        )
                        .unwrap();
                    assert_eq!(value, 73);
                    0
                });
            }
            yield_now();
            0
        });
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn once_initializer_panic_publishes_nothing_and_wakes_fifo_retry() {
        let once = Arc::new(CooperativeOnce::<usize>::new());
        let results = Arc::new(Mutex::new(Vec::new()));
        let allow_panic = Arc::new(AtomicBool::new(false));
        let observed = results.clone();
        run(move || {
            let first_once = once.clone();
            let first_allow_panic = allow_panic.clone();
            let first = spawn(move || {
                let panic = catch_unwind(AssertUnwindSafe(|| {
                    first_once.get_or_init(
                        || {
                            while !first_allow_panic.load(Ordering::SeqCst) {
                                yield_now();
                            }
                            panic!("initializer failed")
                        },
                        |value| *value,
                    )
                }));
                assert!(panic.is_err());
                0
            });
            for _ in 0..32 {
                if once.inner.state.lock().unwrap().initializing == Some(first) {
                    break;
                }
                yield_now();
            }
            let second_once = once.clone();
            let second_results = observed.clone();
            let second = spawn(move || {
                let value = second_once.get_or_init(|| 91, |value| *value).unwrap();
                second_results.lock().unwrap().push(value);
                0
            });
            wait_until_once_queued(&once, second);
            allow_panic.store(true, Ordering::SeqCst);
            yield_now();
            let _ = (first, second);
            0
        });
        assert_eq!(*results.lock().unwrap(), vec![91]);
    }

    #[test]
    fn once_waiting_cancellation_removes_only_that_waiter() {
        let once = Arc::new(CooperativeOnce::<usize>::new());
        let outcomes = Arc::new(Mutex::new(Vec::new()));
        let allow_finish = Arc::new(AtomicBool::new(false));
        let observed = outcomes.clone();
        run(move || {
            let first_once = once.clone();
            let first_allow_finish = allow_finish.clone();
            let first = spawn(move || {
                first_once
                    .get_or_init(
                        || {
                            while !first_allow_finish.load(Ordering::SeqCst) {
                                yield_now();
                            }
                            17
                        },
                        |value| *value,
                    )
                    .unwrap();
                0
            });
            for _ in 0..32 {
                if once.inner.state.lock().unwrap().initializing == Some(first) {
                    break;
                }
                yield_now();
            }
            let waiting_once = once.clone();
            let waiting_outcomes = observed.clone();
            let waiting = spawn(move || {
                let outcome = waiting_once.get_or_init(|| 99, |value| *value);
                waiting_outcomes.lock().unwrap().push(outcome);
                0
            });
            wait_until_once_queued(&once, waiting);
            cancel_current(waiting);
            allow_finish.store(true, Ordering::SeqCst);
            yield_now();
            assert!(!once.inner.state.lock().unwrap().waiters.contains(&waiting));
            0
        });
        assert_eq!(*outcomes.lock().unwrap(), vec![Err(OnceError::Cancelled)]);
    }

    #[test]
    fn once_panic_skips_cancelled_fifo_head_and_wakes_next_waiter() {
        let once = Arc::new(CooperativeOnce::<usize>::new());
        let outcomes = Arc::new(Mutex::new(Vec::new()));
        let head_id = Arc::new(Mutex::new(None));
        let allow_failure = Arc::new(AtomicBool::new(false));
        let observed = outcomes.clone();
        run(move || {
            let init_once = once.clone();
            let init_head = head_id.clone();
            let init_allowed = allow_failure.clone();
            let initializer = spawn(move || {
                let panic = catch_unwind(AssertUnwindSafe(|| {
                    init_once.get_or_init(
                        || {
                            while !init_allowed.load(Ordering::SeqCst) {
                                yield_now();
                            }
                            panic!("initializer failed")
                        },
                        |value| *value,
                    )
                }));
                assert!(panic.is_err());
                // The panic guard has selected and woken the FIFO head, but
                // the executor has not run it yet. Its cancellation must
                // transfer the reservation to the next waiter.
                cancel_current(init_head.lock().unwrap().unwrap());
                0
            });
            for _ in 0..32 {
                if once.inner.state.lock().unwrap().initializing == Some(initializer) {
                    break;
                }
                yield_now();
            }
            let first_once = once.clone();
            let first_outcomes = observed.clone();
            let first = spawn(move || {
                let outcome = first_once.get_or_init(|| 1, |value| *value);
                first_outcomes.lock().unwrap().push(outcome);
                0
            });
            *head_id.lock().unwrap() = Some(first);
            let second_once = once.clone();
            let second_outcomes = observed.clone();
            let second = spawn(move || {
                let outcome = second_once.get_or_init(|| 91, |value| *value);
                second_outcomes.lock().unwrap().push(outcome);
                0
            });
            wait_until_once_queued(&once, first);
            wait_until_once_queued(&once, second);
            allow_failure.store(true, Ordering::SeqCst);
            yield_now();
            0
        });
        let outcomes = outcomes.lock().unwrap();
        assert!(outcomes.contains(&Err(OnceError::Cancelled)));
        assert!(outcomes.contains(&Ok(91)));
        assert_eq!(once_value_for_test(&outcomes), Some(91));
    }

    fn once_value_for_test(outcomes: &[Result<usize, OnceError>]) -> Option<usize> {
        outcomes
            .iter()
            .find_map(|outcome| outcome.as_ref().ok().copied())
    }

    #[test]
    fn constructor_rejects_initial_count_above_fixed_maximum() {
        assert_eq!(
            CooperativeSemaphore::new(2, 3).err(),
            Some(SemaphoreError::InvalidBounds)
        );
        assert!(CooperativeSemaphore::new(0, 0).is_ok());
    }

    #[test]
    fn permit_drop_restores_one_slot_and_try_acquire_never_parks() {
        let semaphore = CooperativeSemaphore::new(1, 1).unwrap();
        run(move || {
            let permit = semaphore.try_acquire().unwrap().unwrap();
            assert!(semaphore.try_acquire().unwrap().is_none());
            drop(permit);
            assert!(semaphore.try_acquire().unwrap().is_some());
            0
        });
    }

    #[test]
    fn queued_tasks_receive_released_permits_in_fifo_order() {
        let semaphore = Arc::new(CooperativeSemaphore::new(1, 1).unwrap());
        let order = Arc::new(Mutex::new(Vec::new()));
        let observed = order.clone();
        run(move || {
            let held = semaphore.acquire().unwrap();
            let first_sem = semaphore.clone();
            let first_order = observed.clone();
            let first = spawn(move || {
                let _permit = first_sem.acquire().unwrap();
                first_order.lock().unwrap().push(1);
                0
            });
            wait_until_queued(&semaphore, first);
            let second_sem = semaphore.clone();
            let second_order = observed.clone();
            let second = spawn(move || {
                let _permit = second_sem.acquire().unwrap();
                second_order.lock().unwrap().push(2);
                0
            });
            wait_until_queued(&semaphore, second);
            drop(held);
            let _ = (first, second);
            0
        });
        assert_eq!(*order.lock().unwrap(), vec![1, 2]);
    }

    #[test]
    fn close_is_idempotent_wakes_every_waiter_and_is_terminal() {
        let semaphore = Arc::new(CooperativeSemaphore::new(1, 0).unwrap());
        let closed = Arc::new(AtomicUsize::new(0));
        let observed = closed.clone();
        run(move || {
            let mut tasks = Vec::new();
            for _ in 0..3 {
                let sem = semaphore.clone();
                let observed = observed.clone();
                tasks.push(spawn(move || {
                    if matches!(sem.acquire(), Err(SemaphoreError::Closed)) {
                        observed.fetch_add(1, Ordering::SeqCst);
                    }
                    0
                }));
            }
            for task in &tasks {
                wait_until_queued(&semaphore, *task);
            }
            semaphore.close().unwrap();
            semaphore.close().unwrap();
            assert!(matches!(
                semaphore.try_acquire(),
                Err(SemaphoreError::Closed)
            ));
            0
        });
        assert_eq!(closed.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn cancellation_after_grant_returns_reservation_without_losing_permit() {
        let semaphore = Arc::new(CooperativeSemaphore::new(1, 1).unwrap());
        let outcomes = Arc::new(Mutex::new(Vec::new()));
        let observed = outcomes.clone();
        let inspect = semaphore.clone();
        run(move || {
            let held = semaphore.acquire().unwrap();
            let cancelled_sem = semaphore.clone();
            let cancelled_result = observed.clone();
            let cancelled = spawn(move || {
                cancelled_result
                    .lock()
                    .unwrap()
                    .push(cancelled_sem.acquire().err());
                0
            });
            wait_until_queued(&semaphore, cancelled);
            drop(held); // grants the permit, but the executor has not run it yet
            assert!(
                semaphore
                    .inner
                    .state
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .granted
                    .contains(&cancelled)
            );
            crate::executor::cancel_current(cancelled);

            let next_sem = semaphore.clone();
            let next_result = observed.clone();
            let next = spawn(move || {
                match next_sem.acquire() {
                    Ok(_permit) => next_result.lock().unwrap().push(None),
                    Err(error) => next_result.lock().unwrap().push(Some(error)),
                }
                0
            });
            yield_now();
            let _ = (cancelled, next);
            0
        });
        assert!(
            outcomes
                .lock()
                .unwrap()
                .contains(&Some(SemaphoreError::Cancelled))
        );
        let state = inspect.inner.state.lock().unwrap();
        assert_eq!(state.available, 1);
        assert!(state.granted.is_empty());
        assert!(state.waiters.is_empty());
    }

    #[test]
    fn cancellation_while_queued_removes_waiter_and_preserves_fifo_progress() {
        let semaphore = Arc::new(CooperativeSemaphore::new(1, 1).unwrap());
        let outcomes = Arc::new(Mutex::new(Vec::new()));
        let observed = outcomes.clone();
        run(move || {
            let held = semaphore.acquire().unwrap();
            let cancelled_sem = semaphore.clone();
            let cancelled_result = observed.clone();
            let cancelled = spawn(move || {
                cancelled_result
                    .lock()
                    .unwrap()
                    .push(cancelled_sem.acquire().err());
                0
            });
            wait_until_queued(&semaphore, cancelled);

            let next_sem = semaphore.clone();
            let next_result = observed.clone();
            let next = spawn(move || {
                match next_sem.acquire() {
                    Ok(_permit) => next_result.lock().unwrap().push(None),
                    Err(error) => next_result.lock().unwrap().push(Some(error)),
                }
                0
            });
            wait_until_queued(&semaphore, next);
            crate::executor::cancel_current(cancelled);
            drop(held);
            let _ = (cancelled, next);
            0
        });
        let outcomes = outcomes.lock().unwrap();
        assert!(outcomes.contains(&Some(SemaphoreError::Cancelled)));
        assert!(outcomes.contains(&None));
    }

    #[test]
    fn dropping_a_permit_after_close_does_not_reopen_admission() {
        let semaphore = Arc::new(CooperativeSemaphore::new(1, 1).unwrap());
        run(move || {
            let permit = semaphore.acquire().unwrap();
            semaphore.close().unwrap();
            drop(permit);
            assert!(matches!(
                semaphore.try_acquire(),
                Err(SemaphoreError::Closed)
            ));
            0
        });
    }

    #[test]
    fn operations_reject_calls_outside_the_bound_executor_thread() {
        let semaphore = Arc::new(CooperativeSemaphore::new(1, 1).unwrap());
        assert_eq!(semaphore.acquire().err(), Some(SemaphoreError::OutsideTask));
        let other_thread = semaphore.clone();
        std::thread::spawn(move || {
            run(move || {
                assert_eq!(
                    other_thread.acquire().err(),
                    Some(SemaphoreError::OutsideTask)
                );
                assert_eq!(
                    other_thread.try_acquire().err(),
                    Some(SemaphoreError::OutsideTask)
                );
                assert_eq!(other_thread.close(), Err(SemaphoreError::OutsideTask));
                0
            });
        })
        .join()
        .unwrap();
    }

    #[test]
    fn bounded_sync_contention_cancellation_and_release_cycles_terminate() {
        // Repeat enough generations to exercise waiter removal, permit
        // handoff, and broken-barrier wakeup under sustained task churn.
        const ROUNDS: usize = 64;
        let semaphore = Arc::new(CooperativeSemaphore::new(1, 1).unwrap());
        let completed = Arc::new(AtomicUsize::new(0));
        let observed = completed.clone();
        let inspect_semaphore = semaphore.clone();
        let inspect_during_run = inspect_semaphore.clone();
        let barrier_results = Arc::new(Mutex::new(Vec::new()));
        let observed_barriers = barrier_results.clone();

        run(move || {
            for _ in 0..ROUNDS {
                // Hold the only permit while one task queues for cancellation
                // and a second queues behind it for the next release.
                let held = semaphore.acquire().unwrap();
                let cancelled_sem = semaphore.clone();
                let cancelled_count = observed.clone();
                let cancelled = spawn(move || {
                    let outcome = cancelled_sem.acquire();
                    if outcome.is_err() {
                        cancelled_count.fetch_add(1, Ordering::SeqCst);
                    }
                    drop(outcome);
                    0
                });
                wait_until_queued(&semaphore, cancelled);

                let next_sem = semaphore.clone();
                let next_count = observed.clone();
                let next = spawn(move || {
                    let permit = next_sem.acquire().expect("live waiter must acquire");
                    next_count.fetch_add(1, Ordering::SeqCst);
                    drop(permit);
                    0
                });
                wait_until_queued(&semaphore, next);
                cancel_current(cancelled);
                drop(held);
                yield_now();

                let state = inspect_during_run
                    .inner
                    .state
                    .lock()
                    .unwrap_or_else(|e| e.into_inner());
                assert_eq!(state.available + state.granted.len(), 1);
                assert!(state.waiters.is_empty());
                drop(state);

                let barrier = Arc::new(CooperativeBarrier::new(3).unwrap());
                let mut parties = Vec::new();
                for _ in 0..2 {
                    let barrier = barrier.clone();
                    let results = observed_barriers.clone();
                    parties.push(spawn(move || {
                        let outcome = barrier.wait();
                        results.lock().unwrap().push(outcome);
                        0
                    }));
                }
                for party in &parties {
                    wait_until_barrier_queued(&barrier, *party);
                }
                cancel_current(parties[0]);
                yield_now();
                assert_eq!(barrier.wait(), Err(BarrierError::Broken));
                let state = barrier
                    .inner
                    .state
                    .lock()
                    .unwrap_or_else(|e| e.into_inner());
                assert!(state.waiters.is_empty());
            }
            0
        });

        assert_eq!(completed.load(Ordering::SeqCst), 2 * ROUNDS);
        let barrier_results = barrier_results.lock().unwrap();
        assert_eq!(barrier_results.len(), 2 * ROUNDS);
        assert!(
            barrier_results
                .iter()
                .all(|result| *result == Err(BarrierError::Broken))
        );
        let state = inspect_semaphore
            .inner
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        assert_eq!(state.available, 1);
        assert!(state.granted.is_empty());
        assert!(state.waiters.is_empty());
    }

    #[test]
    fn bounded_sync_panic_cancel_and_wakeup_interactions_terminate() {
        // Keep panic, cancellation, Once retry, and semaphore handoff
        // interleaved over many independent primitive instances.
        const ROUNDS: usize = 32;
        let semaphore = Arc::new(CooperativeSemaphore::new(1, 1).unwrap());
        let once_results = Arc::new(Mutex::new(Vec::new()));
        let semaphore_results = Arc::new(AtomicUsize::new(0));
        let observed_once = once_results.clone();
        let observed_semaphore = semaphore_results.clone();

        run(move || {
            for round in 0..ROUNDS {
                let once = Arc::new(CooperativeOnce::<usize>::new());
                // A cancelled semaphore waiter must not prevent the next
                // queued task from acquiring the released permit.
                let held = semaphore.acquire().unwrap();
                let cancelled_sem = semaphore.clone();
                let cancelled = spawn(move || {
                    let result = cancelled_sem.acquire();
                    assert_eq!(result.err(), Some(SemaphoreError::Cancelled));
                    0
                });
                wait_until_queued(&semaphore, cancelled);
                let next_sem = semaphore.clone();
                let next_count = observed_semaphore.clone();
                let next = spawn(move || {
                    let permit = next_sem.acquire().expect("live waiter must be woken");
                    next_count.fetch_add(1, Ordering::SeqCst);
                    drop(permit);
                    0
                });
                wait_until_queued(&semaphore, next);
                cancel_current(cancelled);
                drop(held);

                // The first Once initializer is held until a waiter is
                // definitely queued. Its panic must leave the cell empty and
                // wake that waiter to publish the value.
                let allow_panic = Arc::new(AtomicBool::new(false));
                let initializer_once = once.clone();
                let initializer_gate = allow_panic.clone();
                let initializer = spawn(move || {
                    let panicked = catch_unwind(AssertUnwindSafe(|| {
                        initializer_once.get_or_init(
                            || {
                                while !initializer_gate.load(Ordering::SeqCst) {
                                    yield_now();
                                }
                                panic!("bounded sync stress initializer panic");
                            },
                            |value| *value,
                        )
                    }));
                    assert!(panicked.is_err());
                    0
                });
                for _ in 0..32 {
                    if once.inner.state.lock().unwrap().initializing == Some(initializer) {
                        break;
                    }
                    yield_now();
                }
                assert_eq!(
                    once.inner.state.lock().unwrap().initializing,
                    Some(initializer),
                    "initializer did not enter in round {round}"
                );

                // Queue one waiter that will be cancelled and a live waiter
                // behind it. Once panic recovery must skip the cancelled
                // head and transfer progress to the live waiter.
                let cancelled_once = once.clone();
                let cancelled_results = observed_once.clone();
                let cancelled_result = spawn(move || {
                    let result = cancelled_once.get_or_init(|| usize::MAX, |value| *value);
                    cancelled_results.lock().unwrap().push(result);
                    0
                });
                wait_until_once_queued(&once, cancelled_result);
                let live_once = once.clone();
                let live_results = observed_once.clone();
                let live = spawn(move || {
                    let result = live_once.get_or_init(|| round + 100, |value| *value);
                    live_results.lock().unwrap().push(result);
                    0
                });
                wait_until_once_queued(&once, live);
                cancel_current(cancelled_result);
                allow_panic.store(true, Ordering::SeqCst);
                yield_now();

                assert_eq!(once.get_or_init(|| 0, |value| *value), Ok(round + 100));
                assert_eq!(
                    semaphore
                        .inner
                        .state
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .available,
                    1,
                    "semaphore permit leaked in round {round}"
                );
                assert_eq!(
                    observed_once
                        .lock()
                        .unwrap()
                        .iter()
                        .filter(|result| **result == Err(OnceError::Cancelled))
                        .count(),
                    round + 1,
                    "cancelled Once waiter did not finish in round {round}"
                );
            }
            0
        });

        assert_eq!(semaphore_results.load(Ordering::SeqCst), ROUNDS);
        assert_eq!(
            *once_results.lock().unwrap(),
            (0..ROUNDS)
                .flat_map(|round| [Err(OnceError::Cancelled), Ok(round + 100)])
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn mixed_fifo_sync_contention_unwinds_and_conserves_waiters_and_permits() {
        use crate::sync_mutex::CooperativeMutex;
        use crate::sync_rwlock::CooperativeRwLock;

        const WORKERS: usize = 4;
        const ROUNDS: usize = 24;
        run_with_timeout(move || {
            let mutex = Arc::new(CooperativeMutex::new(0usize));
            let rwlock = Arc::new(CooperativeRwLock::new(0usize));
            let semaphore = Arc::new(CooperativeSemaphore::new(2, 2).unwrap());
            let barrier = Arc::new(CooperativeBarrier::new(WORKERS + 1).unwrap());
            let once = Arc::new(CooperativeOnce::<usize>::new());
            let completed = Arc::new(AtomicUsize::new(0));
            let recovered_semaphore_waiter = Arc::new(AtomicUsize::new(0));
            let cancelled_barrier_results = Arc::new(Mutex::new(Vec::new()));
            let cancelled_once_results = Arc::new(Mutex::new(Vec::new()));

            let initial_mutex_panic = catch_unwind(AssertUnwindSafe(|| {
                mutex
                    .with(|_| panic!("intentional mixed-stress mutex unwind"))
                    .unwrap();
            }));
            assert!(initial_mutex_panic.is_err());

            let initial_once_panic = catch_unwind(AssertUnwindSafe(|| {
                let _ = once.get_or_init(
                    || panic!("intentional mixed-stress Once unwind"),
                    |value| *value,
                );
            }));
            assert!(initial_once_panic.is_err());

            // Cancel a semaphore waiter while another task is queued
            // behind it. Releasing the held permit must still make
            // progress and restore the full permit count.
            let held = [
                semaphore.acquire().expect("first permit available"),
                semaphore.acquire().expect("second permit available"),
            ];
            let cancelled_semaphore = semaphore.clone();
            let cancelled_sem_waiter = spawn(move || {
                assert_eq!(
                    cancelled_semaphore.acquire().err(),
                    Some(SemaphoreError::Cancelled)
                );
                0
            });
            wait_until_queued(&semaphore, cancelled_sem_waiter);
            let live_semaphore = semaphore.clone();
            let recovered = recovered_semaphore_waiter.clone();
            let live_sem_waiter = spawn(move || {
                drop(
                    live_semaphore
                        .acquire()
                        .expect("live waiter receives permit"),
                );
                recovered.fetch_add(1, Ordering::SeqCst);
                0
            });
            wait_until_queued(&semaphore, live_sem_waiter);
            cancel_current(cancelled_sem_waiter);
            drop(held);

            // A cancelled barrier participant breaks the generation and
            // wakes its peer. All waits for queue registration are
            // bounded by the existing 32-yield helper.
            let cancel_barrier = Arc::new(CooperativeBarrier::new(3).unwrap());
            let cancelled_barrier = cancel_barrier.clone();
            let cancelled_results = cancelled_barrier_results.clone();
            let cancelled_barrier_waiter = spawn(move || {
                cancelled_results
                    .lock()
                    .unwrap()
                    .push(cancelled_barrier.wait());
                0
            });
            wait_until_barrier_queued(&cancel_barrier, cancelled_barrier_waiter);
            cancel_current(cancelled_barrier_waiter);
            yield_now();
            assert_eq!(cancel_barrier.wait(), Err(BarrierError::Broken));
            let cancelled_barrier_state = cancel_barrier.inner.state.lock().unwrap();
            assert_eq!(cancelled_barrier_state.arrived, 0);
            assert!(cancelled_barrier_state.waiters.is_empty());

            // Hold Once initialization in the root task while its waiter
            // queues, cancel that waiter, then publish. This avoids a
            // timing race and exercises waiter removal in the same
            // executor run as the contention stress below.
            let cancel_once = Arc::new(CooperativeOnce::<usize>::new());
            assert_eq!(
                cancel_once.get_or_init(
                    || {
                        let waiter_once = cancel_once.clone();
                        let waiter_results = cancelled_once_results.clone();
                        let waiter = spawn(move || {
                            waiter_results
                                .lock()
                                .unwrap()
                                .push(waiter_once.get_or_init(|| 99, |value| *value));
                            0
                        });
                        wait_until_once_queued(&cancel_once, waiter);
                        cancel_current(waiter);
                        7
                    },
                    |value| *value
                ),
                Ok(7)
            );
            yield_now();
            let cancelled_once_state = cancel_once.inner.state.lock().unwrap();
            assert_eq!(cancelled_once_state.initializing, None);
            assert!(cancelled_once_state.waiters.is_empty());
            drop(cancelled_once_state);

            for worker in 0..WORKERS {
                let mutex = mutex.clone();
                let rwlock = rwlock.clone();
                let semaphore = semaphore.clone();
                let barrier = barrier.clone();
                let once = once.clone();
                let completed = completed.clone();
                spawn(move || {
                    for _round in 0..ROUNDS {
                        let permit = semaphore.acquire().expect("permit admitted");
                        mutex
                            .with(|value| *value += 1)
                            .expect("mutex remains usable after unwind");
                        rwlock
                            .with_write(|value| *value += 1)
                            .expect("rwlock writer completes");
                        assert!(rwlock.with_read(|value| *value).is_ok());
                        assert_eq!(once.get_or_init(|| worker + 1, |value| *value), Ok(1));
                        drop(permit);
                        assert!(matches!(
                            barrier.wait(),
                            Ok(BarrierWait::Leader | BarrierWait::Follower)
                        ));
                        if worker == 0 {
                            completed.fetch_add(1, Ordering::SeqCst);
                        }
                    }
                    0
                });
            }

            for _ in 0..ROUNDS {
                assert!(matches!(
                    barrier.wait(),
                    Ok(BarrierWait::Leader | BarrierWait::Follower)
                ));
            }
            assert_eq!(mutex.with(|value| *value), Ok(WORKERS * ROUNDS));
            assert_eq!(rwlock.with_read(|value| *value), Ok(WORKERS * ROUNDS));
            assert_eq!(once.inner.value.get(), Some(&1));
            let semaphore_state = semaphore.inner.state.lock().unwrap();
            assert_eq!(semaphore_state.available, 2);
            assert!(semaphore_state.waiters.is_empty());
            assert!(semaphore_state.granted.is_empty());
            drop(semaphore_state);
            let barrier_state = barrier.inner.state.lock().unwrap();
            assert_eq!(barrier_state.arrived, 0);
            assert!(barrier_state.waiters.is_empty());
            assert!(!barrier_state.broken);
            drop(barrier_state);
            let once_state = once.inner.state.lock().unwrap();
            assert_eq!(once_state.initializing, None);
            assert!(once_state.waiters.is_empty());
            drop(once_state);

            for _ in 0..8 {
                yield_now();
            }
            assert_eq!(completed.load(Ordering::SeqCst), ROUNDS);
            assert_eq!(recovered_semaphore_waiter.load(Ordering::SeqCst), 1);
            assert_eq!(
                *cancelled_barrier_results.lock().unwrap(),
                vec![Err(BarrierError::Broken)]
            );
            assert_eq!(
                *cancelled_once_results.lock().unwrap(),
                vec![Err(OnceError::Cancelled)]
            );
            let semaphore_state = semaphore.inner.state.lock().unwrap();
            assert_eq!(semaphore_state.available, 2);
            assert!(semaphore_state.waiters.is_empty());
            assert!(semaphore_state.granted.is_empty());
            drop(semaphore_state);
            let barrier_state = barrier.inner.state.lock().unwrap();
            assert_eq!(barrier_state.arrived, 0);
            assert!(barrier_state.waiters.is_empty());
            assert!(!barrier_state.broken);
            drop(barrier_state);
            let once_state = once.inner.state.lock().unwrap();
            assert_eq!(once_state.initializing, None);
            assert!(once_state.waiters.is_empty());
            0
        });
    }
}
