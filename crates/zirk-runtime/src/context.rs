//! Task execution contexts — the single seam behind which task stack switching
//! lives.
//!
//! [`ADR-017`](../../../../docs/decisions/ADR-017-modelo-de-suspension.md) chose
//! **stackful coroutines**: an internal scheduler task owns its own stack and
//! suspends by switching back to the executor, with no transformation of
//! function bodies into state machines. This module is the only place that
//! knows how that switch is performed.
//!
//! # Backends
//!
//! * **Native** (`task_context_native`, set by `build.rs`): on the targets the
//!   toolchain supports for running the executor — macOS aarch64, Linux x86_64,
//!   Linux aarch64, Windows x86_64 (`ADR-004` addendum "portability C") — the
//!   switch is provided by [`corosensei`], whose default stack also carries an
//!   OS guard page and whose trap API can later back `StackOverflowError`.
//! * **Fallback**: any other host (a contributor's machine `corosensei` does not
//!   cover) gets a thread-backed implementation with the *same* API and the
//!   *same* cooperative "only one side runs at a time" semantics, so
//!   `cargo test` builds and passes everywhere. It is never linked into a
//!   shipped binary for a supported target.
//!
//! Choosing `corosensei` over hand-written per-triple assembly is the
//! resolution of task 1.6 of the `fase-5-async-core` plan: it covers exactly the
//! four supported triples, is `no_std`-capable and dual MIT/Apache, and keeps
//! the register save/restore out of hand-audited assembly in this repository.
//!
//! # Suspending without a `&Suspender`
//!
//! A task body receives a `&Suspender` and can suspend directly. Generated code
//! and runtime functions that must suspend from deep in a call stack (a future
//! `zirk_rt_task_await`, channel operations) hold no such reference, so this
//! module also exposes [`suspend_current`]: it suspends the running task through
//! a thread-local pointer to its suspend handle, armed at body entry and
//! re-armed after every resume. Because the executor is single-threaded and a
//! task only ever resumes from inside its own last suspend call, whenever any
//! task is running that pointer refers to *its* handle.

/// Default scheduler-task stack size, in bytes. Fixed at 128 KiB and not
/// user-configurable in this phase (design decision 1.4). A per-task size and
/// a hardware guard-page scheme are tracked as follow-up.
pub const DEFAULT_TASK_STACK_BYTES: usize = 128 * 1024;

/// Result of resuming a [`TaskContext`].
#[derive(Debug, PartialEq, Eq)]
pub enum Run {
    /// The task suspended itself and may be resumed again.
    Suspended,
    /// The task body returned the carried value. The context is spent; do not
    /// resume it again. The value is the body's `usize` result — a machine word
    /// or a pointer, whatever generated code will eventually hand back.
    Finished(usize),
}

// ---------------------------------------------------------------------------
// Native backend: corosensei
// ---------------------------------------------------------------------------

#[cfg(task_context_native)]
mod imp {
    use super::{DEFAULT_TASK_STACK_BYTES, Run};
    use corosensei::stack::{DefaultStack, MIN_STACK_SIZE};
    use corosensei::{Coroutine, CoroutineResult};
    use std::cell::Cell;

    type Yielder = corosensei::Yielder<(), ()>;

    thread_local! {
        /// The running task's yielder, or null when no task body is on the
        /// stack. corosensei keeps a `Yielder` at a fixed address for the whole
        /// body execution (its own `Yielder::suspend` holds `&self` across the
        /// switch), so this pointer stays valid between body entry and return.
        static ARMED: Cell<*const Yielder> = const { Cell::new(std::ptr::null()) };
    }

    /// The handle a running task uses to switch back to the executor.
    ///
    /// A task never constructs one: it receives a `&Suspender` as the single
    /// argument of the body passed to [`TaskContext::new`].
    pub struct Suspender<'a>(&'a Yielder);

    impl Suspender<'_> {
        /// Switches control back to whoever last called [`TaskContext::resume`].
        /// Returns when the context is resumed again.
        #[inline]
        pub fn suspend(&self) {
            self.0.suspend(());
            // Another task may have run while we were parked; re-point ARMED
            // at us now that we are the one executing again.
            ARMED.set(self.0 as *const Yielder);
        }
    }

    /// Suspends the currently running task. Panics if no task is running.
    pub fn suspend_current() {
        let yielder = ARMED.get();
        assert!(!yielder.is_null(), "suspend_current() with no task running");
        // SAFETY: ARMED is non-null only while a task body is on the stack, and
        // that body's `Yielder` lives at a fixed address for its whole run.
        unsafe { (*yielder).suspend(()) };
        ARMED.set(yielder);
    }

    /// A suspendable unit of execution with its own stack.
    pub struct TaskContext(Coroutine<(), (), usize, DefaultStack>);

    impl TaskContext {
        /// Creates a task context that will run `body` on a fresh
        /// `stack_bytes`-byte stack (clamped up to the platform minimum). The
        /// body does not start until the first [`resume`](Self::resume); its
        /// `usize` result is delivered by the final [`Run::Finished`].
        pub fn new<F>(stack_bytes: usize, body: F) -> Self
        where
            F: FnOnce(&Suspender<'_>) -> usize + Send + 'static,
        {
            let size = stack_bytes.max(MIN_STACK_SIZE);
            let stack = DefaultStack::new(size).expect("task stack allocation failed");
            let coroutine = Coroutine::with_stack(stack, move |yielder, ()| {
                let previous = ARMED.replace(yielder as *const Yielder);
                let out = body(&Suspender(yielder));
                ARMED.set(previous);
                out
            });
            TaskContext(coroutine)
        }

        /// Runs the task until it next suspends or returns.
        pub fn resume(&mut self) -> Run {
            match self.0.resume(()) {
                CoroutineResult::Yield(()) => Run::Suspended,
                CoroutineResult::Return(value) => Run::Finished(value),
            }
        }

        /// Whether the body has returned.
        pub fn is_finished(&self) -> bool {
            self.0.done()
        }
    }

    /// Creates a context with the default stack size.
    pub fn spawn_default<F>(body: F) -> TaskContext
    where
        F: FnOnce(&Suspender<'_>) -> usize + Send + 'static,
    {
        TaskContext::new(DEFAULT_TASK_STACK_BYTES, body)
    }
}

// ---------------------------------------------------------------------------
// Fallback backend: one cooperatively-scheduled OS thread per context
// ---------------------------------------------------------------------------

#[cfg(not(task_context_native))]
mod imp {
    use super::{DEFAULT_TASK_STACK_BYTES, Run};
    use std::cell::Cell;
    use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
    use std::thread::JoinHandle;

    /// Sent from the executor thread into the task thread on every resume.
    struct Resume;
    /// Sent from the task thread back to the executor thread.
    enum Step {
        Suspended,
        Finished(usize),
    }

    /// Panic payload used to unwind a still-suspended task when its
    /// [`TaskContext`] is dropped, mirroring `corosensei`'s drop-unwinds
    /// behavior so stack locals run their destructors.
    struct Shutdown;

    thread_local! {
        /// The running task's suspend handle. In this backend each task is its
        /// own OS thread, so a plain `thread_local` is already per-task; the
        /// pointer targets the `Suspender` local in that thread's body frame.
        static ARMED: Cell<*const Suspender<'static>> = const { Cell::new(std::ptr::null()) };
    }

    pub struct Suspender<'a> {
        to_executor: &'a SyncSender<Step>,
        from_executor: &'a Receiver<Resume>,
    }

    impl Suspender<'_> {
        #[inline]
        pub fn suspend(&self) {
            // Hand the executor the "suspended" step, then block until resumed.
            let _ = self.to_executor.send(Step::Suspended);
            if self.from_executor.recv().is_err() {
                // The TaskContext was dropped: unwind this task's stack.
                std::panic::resume_unwind(Box::new(Shutdown));
            }
        }
    }

    /// Suspends the currently running task. Panics if no task is running.
    pub fn suspend_current() {
        let handle = ARMED.get();
        assert!(!handle.is_null(), "suspend_current() with no task running");
        // SAFETY: ARMED is non-null only while this thread's body frame (which
        // owns the `Suspender`) is live.
        unsafe { (*handle).suspend() };
    }

    pub struct TaskContext {
        to_task: Option<SyncSender<Resume>>,
        from_task: Receiver<Step>,
        handle: Option<JoinHandle<()>>,
        finished: bool,
    }

    impl TaskContext {
        pub fn new<F>(stack_bytes: usize, body: F) -> Self
        where
            F: FnOnce(&Suspender<'_>) -> usize + Send + 'static,
        {
            // Rendezvous channels: each side blocks until the other is ready,
            // which is what makes the handoff strictly one-runner-at-a-time.
            let (to_task, task_rx) = sync_channel::<Resume>(0);
            let (task_tx, from_task) = sync_channel::<Step>(0);

            let stack_size = stack_bytes.max(DEFAULT_TASK_STACK_BYTES);
            let handle = std::thread::Builder::new()
                .name("zirk-task-fallback".into())
                .stack_size(stack_size)
                .spawn(move || {
                    // Do not start the body until the first resume().
                    if task_rx.recv().is_err() {
                        return;
                    }
                    let suspender = Suspender {
                        to_executor: &task_tx,
                        from_executor: &task_rx,
                    };
                    // SAFETY: `suspender` outlives every `suspend_current` call
                    // this thread makes (they all happen inside `body`).
                    ARMED.set(&suspender as *const Suspender<'_> as *const Suspender<'static>);
                    let ran =
                        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| body(&suspender)));
                    ARMED.set(std::ptr::null());
                    match ran {
                        Ok(value) => {
                            let _ = task_tx.send(Step::Finished(value));
                        }
                        Err(payload) if payload.is::<Shutdown>() => {
                            // Dropped while suspended: exit quietly.
                        }
                        Err(payload) => std::panic::resume_unwind(payload),
                    }
                })
                .expect("failed to spawn fallback task thread");

            TaskContext {
                to_task: Some(to_task),
                from_task,
                handle: Some(handle),
                finished: false,
            }
        }

        pub fn resume(&mut self) -> Run {
            assert!(!self.finished, "resumed a finished task context");
            let sender = self
                .to_task
                .as_ref()
                .expect("task context already torn down");
            if sender.send(Resume).is_err() {
                self.finished = true;
                return Run::Finished(0);
            }
            match self.from_task.recv() {
                Ok(Step::Suspended) => Run::Suspended,
                Ok(Step::Finished(value)) => {
                    self.finished = true;
                    Run::Finished(value)
                }
                Err(_) => {
                    self.finished = true;
                    Run::Finished(0)
                }
            }
        }

        pub fn is_finished(&self) -> bool {
            self.finished
        }
    }

    impl Drop for TaskContext {
        fn drop(&mut self) {
            // Closing the resume channel makes a suspended task unwind.
            drop(self.to_task.take());
            if let Some(handle) = self.handle.take() {
                let _ = handle.join();
            }
        }
    }

    pub fn spawn_default<F>(body: F) -> TaskContext
    where
        F: FnOnce(&Suspender<'_>) -> usize + Send + 'static,
    {
        TaskContext::new(DEFAULT_TASK_STACK_BYTES, body)
    }
}

pub use imp::{Suspender, TaskContext, spawn_default, suspend_current};

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn a_context_runs_to_completion_without_suspending() {
        let ran = Arc::new(AtomicUsize::new(0));
        let ran2 = Arc::clone(&ran);
        let mut ctx = spawn_default(move |_suspender| {
            ran2.fetch_add(1, Ordering::SeqCst);
            0
        });
        assert_eq!(ctx.resume(), Run::Finished(0));
        assert!(ctx.is_finished());
        assert_eq!(ran.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn a_body_result_reaches_resume() {
        let mut ctx = spawn_default(|s| {
            s.suspend();
            0xC0FFEE
        });
        assert_eq!(ctx.resume(), Run::Suspended);
        assert_eq!(ctx.resume(), Run::Finished(0xC0FFEE));
    }

    #[test]
    fn executor_and_task_ping_pong_preserves_task_local_state() {
        // The executor increments on even turns, the task on odd turns, each
        // reading the other's writes across the stack switch. If any
        // callee-saved register or the task's stack were clobbered by the
        // switch, `local` or `shared` would not hold the expected values.
        const ROUNDS: usize = 1000;

        let shared = Arc::new(AtomicUsize::new(0));
        let task_shared = Arc::clone(&shared);

        let mut ctx = spawn_default(move |suspender| {
            // `local` lives entirely on the task stack across every suspend.
            let mut local: usize = 0;
            for _ in 0..ROUNDS {
                local += 1;
                task_shared.fetch_add(1, Ordering::SeqCst);
                suspender.suspend();
            }
            assert_eq!(local, ROUNDS);
            local
        });

        let mut turns = 0;
        while ctx.resume() == Run::Suspended {
            turns += 1;
            shared.fetch_add(1, Ordering::SeqCst);
        }

        assert_eq!(turns, ROUNDS);
        // ROUNDS task increments + ROUNDS executor increments.
        assert_eq!(shared.load(Ordering::SeqCst), ROUNDS * 2);
    }

    #[test]
    fn suspend_current_round_trips_like_the_suspender() {
        let mut ctx = spawn_default(|_s| {
            // Never touch the `&Suspender`; suspend through the thread-local.
            suspend_current();
            suspend_current();
            7
        });
        assert_eq!(ctx.resume(), Run::Suspended);
        assert_eq!(ctx.resume(), Run::Suspended);
        assert_eq!(ctx.resume(), Run::Finished(7));
    }

    #[test]
    fn three_tasks_suspending_via_suspend_current_interleave() {
        // Each body only ever calls `suspend_current()`. If the thread-local
        // yielder were not re-armed per task, the wrong task would resume.
        fn make(id: usize, hits: Arc<AtomicUsize>) -> TaskContext {
            spawn_default(move |_s| {
                for _ in 0..id {
                    hits.fetch_add(id, Ordering::SeqCst);
                    suspend_current();
                }
                id
            })
        }
        let h = Arc::new(AtomicUsize::new(0));
        let mut tasks = [
            make(1, Arc::clone(&h)),
            make(2, Arc::clone(&h)),
            make(3, Arc::clone(&h)),
        ];
        let mut done = [None; 3];
        while done.iter().any(Option::is_none) {
            for (i, t) in tasks.iter_mut().enumerate() {
                if done[i].is_none()
                    && let Run::Finished(v) = t.resume()
                {
                    done[i] = Some(v);
                }
            }
        }
        assert_eq!(done, [Some(1), Some(2), Some(3)]);
        // 1*1 + 2*2 + 3*3
        assert_eq!(h.load(Ordering::SeqCst), 14);
    }

    #[test]
    fn multiple_contexts_interleave_independently() {
        let mut a_hits = 0;
        let mut b_hits = 0;

        let a_counter = Arc::new(AtomicUsize::new(0));
        let b_counter = Arc::new(AtomicUsize::new(0));
        let (ac, bc) = (Arc::clone(&a_counter), Arc::clone(&b_counter));

        let mut a = spawn_default(move |s| {
            for _ in 0..3 {
                ac.fetch_add(1, Ordering::SeqCst);
                s.suspend();
            }
            0
        });
        let mut b = spawn_default(move |s| {
            for _ in 0..5 {
                bc.fetch_add(10, Ordering::SeqCst);
                s.suspend();
            }
            0
        });

        while !a.is_finished() || !b.is_finished() {
            if !a.is_finished() && a.resume() == Run::Suspended {
                a_hits += 1;
            }
            if !b.is_finished() && b.resume() == Run::Suspended {
                b_hits += 1;
            }
        }

        assert_eq!(a_hits, 3);
        assert_eq!(b_hits, 5);
        assert_eq!(a_counter.load(Ordering::SeqCst), 3);
        assert_eq!(b_counter.load(Ordering::SeqCst), 50);
    }

    #[test]
    fn dropping_a_suspended_context_unwinds_its_stack() {
        // A guard whose Drop bumps a counter proves the task stack was unwound
        // when the context is dropped mid-suspend (native: corosensei
        // force-unwind; fallback: the Shutdown panic).
        struct Guard(Arc<AtomicUsize>);
        impl Drop for Guard {
            fn drop(&mut self) {
                self.0.fetch_add(1, Ordering::SeqCst);
            }
        }

        let dropped = Arc::new(AtomicUsize::new(0));
        let flag = Arc::clone(&dropped);
        let mut ctx = spawn_default(move |s| {
            let _g = Guard(flag);
            loop {
                s.suspend();
            }
        });

        assert_eq!(ctx.resume(), Run::Suspended);
        drop(ctx);
        assert_eq!(
            dropped.load(Ordering::SeqCst),
            1,
            "suspended task stack was not unwound on drop"
        );
    }
}
