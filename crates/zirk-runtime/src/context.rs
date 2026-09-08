//! Task execution contexts — the single seam behind which task stack switching
//! lives.
//!
//! [`ADR-017`](../../../../docs/decisions/ADR-017-modelo-de-suspension.md) chose
//! **stackful coroutines**: a `task` owns its own stack and suspends by
//! switching back to the executor, with no `async fn`, no function coloring, and
//! no transformation of function bodies into state machines. This module is the
//! only place that knows how that switch is performed.
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

/// Default task stack size, in bytes. Fixed at 128 KiB and not user-configurable
/// in this phase (design decision 1.4). A per-`task` size and a hardware
/// guard-page scheme are tracked as follow-up.
pub const DEFAULT_TASK_STACK_BYTES: usize = 128 * 1024;

/// Result of resuming a [`TaskContext`].
#[derive(Debug, PartialEq, Eq)]
pub enum Run {
    /// The task suspended itself and may be resumed again.
    Suspended,
    /// The task body returned. The context is spent; do not resume it again.
    Finished,
}

// ---------------------------------------------------------------------------
// Native backend: corosensei
// ---------------------------------------------------------------------------

#[cfg(task_context_native)]
mod imp {
    use super::{DEFAULT_TASK_STACK_BYTES, Run};
    use corosensei::stack::{DefaultStack, MIN_STACK_SIZE};
    use corosensei::{Coroutine, CoroutineResult};

    type Yielder = corosensei::Yielder<(), ()>;

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
        }
    }

    /// A suspendable unit of execution with its own stack.
    pub struct TaskContext(Coroutine<(), (), (), DefaultStack>);

    impl TaskContext {
        /// Creates a task context that will run `body` on a fresh
        /// `stack_bytes`-byte stack (clamped up to the platform minimum). The
        /// body does not start until the first [`resume`](Self::resume).
        pub fn new<F>(stack_bytes: usize, body: F) -> Self
        where
            F: FnOnce(&Suspender<'_>) + Send + 'static,
        {
            let size = stack_bytes.max(MIN_STACK_SIZE);
            let stack = DefaultStack::new(size).expect("task stack allocation failed");
            let coroutine = Coroutine::with_stack(stack, move |yielder, ()| {
                body(&Suspender(yielder));
            });
            TaskContext(coroutine)
        }

        /// Runs the task until it next suspends or returns.
        pub fn resume(&mut self) -> Run {
            match self.0.resume(()) {
                CoroutineResult::Yield(()) => Run::Suspended,
                CoroutineResult::Return(()) => Run::Finished,
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
        F: FnOnce(&Suspender<'_>) + Send + 'static,
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
    use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
    use std::thread::JoinHandle;

    /// Sent from the executor thread into the task thread on every resume.
    struct Resume;
    /// Sent from the task thread back to the executor thread.
    enum Step {
        Suspended,
        Finished,
    }

    /// Panic payload used to unwind a still-suspended task when its
    /// [`TaskContext`] is dropped, mirroring `corosensei`'s drop-unwinds
    /// behavior so stack locals run their destructors.
    struct Shutdown;

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

    pub struct TaskContext {
        to_task: Option<SyncSender<Resume>>,
        from_task: Receiver<Step>,
        handle: Option<JoinHandle<()>>,
        finished: bool,
    }

    impl TaskContext {
        pub fn new<F>(stack_bytes: usize, body: F) -> Self
        where
            F: FnOnce(&Suspender<'_>) + Send + 'static,
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
                    let ran = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        body(&suspender);
                    }));
                    match ran {
                        Ok(()) => {
                            let _ = task_tx.send(Step::Finished);
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
                return Run::Finished;
            }
            match self.from_task.recv() {
                Ok(Step::Suspended) => Run::Suspended,
                Ok(Step::Finished) | Err(_) => {
                    self.finished = true;
                    Run::Finished
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
        F: FnOnce(&Suspender<'_>) + Send + 'static,
    {
        TaskContext::new(DEFAULT_TASK_STACK_BYTES, body)
    }
}

pub use imp::{Suspender, TaskContext, spawn_default};

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
        });
        assert_eq!(ctx.resume(), Run::Finished);
        assert!(ctx.is_finished());
        assert_eq!(ran.load(Ordering::SeqCst), 1);
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
        });
        let mut b = spawn_default(move |s| {
            for _ in 0..5 {
                bc.fetch_add(10, Ordering::SeqCst);
                s.suspend();
            }
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
