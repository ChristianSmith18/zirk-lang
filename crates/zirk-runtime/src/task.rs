//! The task control block and the registry that owns every live task.
//!
//! A [`TaskId`] is a generational handle: freeing a task bumps its slot's
//! generation, so an id kept after its scheduler task is gone is *detectably*
//! dead rather than aliasing whatever task reused the slot. This protects the
//! runtime's internal wait bookkeeping from stale scheduler ids.
//!
//! Design: `openspec/changes/fase-5-executor-core/design.md` D1–D2. The
//! `roots` field is this task's garbage-collection shadow-stack chain; group 7
//! rewires `crate::collector` to push/pop and walk it per task.

use std::any::Any;

use crate::collector::Frame;
use crate::context::TaskContext;

/// A generational reference to a task slot in the [`TaskRegistry`].
///
/// Packs to a `u64` (`generation << 32 | index`) for the `extern "C"` surface
/// generated code will use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TaskId {
    index: u32,
    generation: u32,
}

/// A generational reference to one structured-concurrency scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ScopeId {
    index: u32,
    generation: u32,
}

impl ScopeId {
    /// The packed one-word representation used at the runtime C ABI.
    pub fn to_bits(self) -> u64 {
        (u64::from(self.generation) << 32) | u64::from(self.index)
    }

    /// Inverse of [`to_bits`](Self::to_bits). Registry generation checks
    /// reject stale values before they can name a live scope.
    pub fn from_bits(bits: u64) -> Self {
        Self {
            index: bits as u32,
            generation: (bits >> 32) as u32,
        }
    }
}

/// Runtime bookkeeping for a lexical `concurrent` scope. Failures are kept as
/// task ids: their outcome remains owned by the task control block until the
/// scope has joined and propagated it.
#[derive(Debug, Default)]
pub struct ScopeControlBlock {
    pub branches: Vec<TaskId>,
    /// Timer jobs are owned by the lexical scope but do not participate in
    /// its join. Closing the scope requests their cancellation so a periodic
    /// timer cannot keep the block alive indefinitely.
    pub ambient_timers: Vec<TaskId>,
    pub cancel_requested: bool,
    pub primary_failure: Option<TaskId>,
    pub suppressed: Vec<TaskId>,
}

/// A generational slab of scope control blocks, parallel to [`TaskRegistry`].
#[derive(Default)]
pub struct ScopeRegistry {
    slots: Vec<ScopeSlot>,
    free: Vec<u32>,
}

enum ScopeSlot {
    Live {
        generation: u32,
        scope: ScopeControlBlock,
    },
    Free {
        generation: u32,
    },
}

impl ScopeRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self) -> ScopeId {
        if let Some(index) = self.free.pop() {
            let generation = match &self.slots[index as usize] {
                ScopeSlot::Free { generation } => *generation,
                ScopeSlot::Live { .. } => unreachable!("free scope slot was live"),
            };
            self.slots[index as usize] = ScopeSlot::Live {
                generation,
                scope: ScopeControlBlock::default(),
            };
            ScopeId { index, generation }
        } else {
            let index = self.slots.len() as u32;
            self.slots.push(ScopeSlot::Live {
                generation: 0,
                scope: ScopeControlBlock::default(),
            });
            ScopeId {
                index,
                generation: 0,
            }
        }
    }

    pub fn get(&self, id: ScopeId) -> Option<&ScopeControlBlock> {
        match self.slots.get(id.index as usize)? {
            ScopeSlot::Live { generation, scope } if *generation == id.generation => Some(scope),
            _ => None,
        }
    }

    pub fn get_mut(&mut self, id: ScopeId) -> Option<&mut ScopeControlBlock> {
        match self.slots.get_mut(id.index as usize)? {
            ScopeSlot::Live { generation, scope } if *generation == id.generation => Some(scope),
            _ => None,
        }
    }

    pub fn remove(&mut self, id: ScopeId) -> Option<ScopeControlBlock> {
        let slot = self.slots.get_mut(id.index as usize)?;
        match slot {
            ScopeSlot::Live { generation, .. } if *generation == id.generation => {
                let next_generation = generation.wrapping_add(1);
                let ScopeSlot::Live { scope, .. } = std::mem::replace(
                    slot,
                    ScopeSlot::Free {
                        generation: next_generation,
                    },
                ) else {
                    unreachable!()
                };
                self.free.push(id.index);
                Some(scope)
            }
            _ => None,
        }
    }
}

impl TaskId {
    /// The raw `u64` carried across the internal C ABI.
    pub fn to_bits(self) -> u64 {
        (u64::from(self.generation) << 32) | u64::from(self.index)
    }

    /// Inverse of [`to_bits`](Self::to_bits). Any `u64` is accepted; a value
    /// that never named a live task simply fails the registry's generation
    /// check on lookup.
    pub fn from_bits(bits: u64) -> Self {
        TaskId {
            index: bits as u32,
            generation: (bits >> 32) as u32,
        }
    }
}

/// Where a task is in its lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    /// Enqueued, not yet running.
    Ready,
    /// Currently executing (its stack is on top).
    Running,
    /// Parked at a safe point; [`TaskControlBlock::wait`] says why.
    Suspended,
    /// Body returned normally.
    Completed,
    /// Body let an unhandled throwable escape.
    Failed,
    /// Cancelled and cleaned. (Set by the structured-tasks change; the state
    /// exists here so the executor loop already handles it.)
    Cancelled,
}

impl TaskState {
    /// Whether no further progress is possible for this task.
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            TaskState::Completed | TaskState::Failed | TaskState::Cancelled
        )
    }
}

/// Why a suspended task is parked — what has to happen for it to become
/// [`TaskState::Ready`] again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaitReason {
    /// Not parked (a running or terminal task).
    None,
    /// Voluntarily yielded; re-enqueue at the back of the ready queue.
    Yielded,
    /// Blocked in `await` on another task; wakes when that task is terminal.
    AwaitingTask(TaskId),
    /// Blocked on a timer; wakes when it fires. The `u64` is the timer id
    /// (`crate::timer::TimerId`), stored untyped to keep this module leaf.
    Timer(u64),
    /// Blocked inside a `parallel` region: the branch submitted the region's
    /// chunks to the worker pool and yields the executor thread so other I/O
    /// branches keep running; it wakes when every chunk has joined
    /// (`ADR-019` D1, `parallel-cpu-regions`).
    Parallel,
}

/// What a finished task produced.
pub enum TaskOutcome {
    /// The body returned this machine word (0 for `Void`).
    Value(usize),
    /// The body panicked / an unhandled throwable escaped.
    Panicked(Box<dyn Any + Send>),
}

/// The default cancellation reason (`STRUCTURED_CONCURRENCY_SEMANTICS.md` §3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CancelReason {
    #[default]
    Cancelled,
    /// Cancelled because an `await ... timeout` expired.
    TimedOut,
}

/// Set once structured cleanup has run; a task's slot and stack are reclaimed
/// only after this is `Done` (trivially `Done` in this change — real cleanup
/// arrives with the language surface).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CleanupState {
    Pending,
    Done,
}

/// Everything the executor tracks for one task. Not `Send` — the executor and
/// every task run on one OS thread (`ADR-017`).
pub struct TaskControlBlock {
    pub state: TaskState,
    /// The stackful coroutine; owns the task's 128 KiB stack. `None` only for
    /// the instant the executor loop has it taken out to `resume()` it — this
    /// is what lets `suspend_current` / `await` reach back into the executor
    /// (through its raw-pointer thread-local) without a reentrant borrow.
    pub context: Option<TaskContext>,
    /// `Some` once the body has finished; taken exactly once by `await`.
    pub outcome: Option<TaskOutcome>,
    /// Set by `await` when it takes `outcome`; a second `await` is a fatal
    /// use-after-consume.
    pub result_consumed: bool,
    /// The single task blocked in `await` on this one, woken on completion.
    pub waiter: Option<TaskId>,
    pub wait: WaitReason,
    /// Cooperative-cancellation flag; read at safe points by the
    /// structured-tasks change.
    pub cancel_requested: bool,
    pub cancel_reason: Option<CancelReason>,
    /// `cancellation shield` nesting; delivery deferred while `> 0`.
    pub shield_depth: u32,
    /// This task's garbage-collection shadow-stack chain (design D2). Group 7
    /// makes `zirk_rt_push_frame` / `zirk_rt_pop_frame` and `collect()` use it.
    pub roots: Vec<Frame>,
    /// Capture block received from generated task code. It remains a root until
    /// the child has completed, including before its first body frame exists.
    pub capture_root: Option<*mut std::ffi::c_void>,
    pub cleanup_state: CleanupState,
    /// The structured scope that owns this branch, if any.
    pub parent: Option<ScopeId>,
}

impl TaskControlBlock {
    fn new(context: TaskContext, capture_root: Option<*mut std::ffi::c_void>) -> Self {
        TaskControlBlock {
            state: TaskState::Ready,
            context: Some(context),
            outcome: None,
            result_consumed: false,
            waiter: None,
            wait: WaitReason::None,
            cancel_requested: false,
            cancel_reason: None,
            shield_depth: 0,
            roots: Vec::new(),
            capture_root,
            cleanup_state: CleanupState::Done,
            parent: None,
        }
    }
}

/// A generational slab of task control blocks. The executor owns exactly one.
pub struct TaskRegistry {
    slots: Vec<Slot>,
    /// Indices of `Slot::Free` entries, ready to reuse.
    free: Vec<u32>,
    /// How many live tasks the slab holds.
    live: usize,
}

enum Slot {
    Live {
        generation: u32,
        tcb: Box<TaskControlBlock>,
    },
    Free {
        generation: u32,
    },
}

impl Default for TaskRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl TaskRegistry {
    pub fn new() -> Self {
        TaskRegistry {
            slots: Vec::new(),
            free: Vec::new(),
            live: 0,
        }
    }

    /// Number of live tasks.
    pub fn len(&self) -> usize {
        self.live
    }

    pub fn is_empty(&self) -> bool {
        self.live == 0
    }

    /// Registers a fresh task and returns its id.
    pub fn insert(&mut self, context: TaskContext) -> TaskId {
        self.insert_with_capture_root(context, None)
    }

    /// Registers a generated task whose callable capture block is a GC root.
    pub fn insert_with_capture_root(
        &mut self,
        context: TaskContext,
        capture_root: Option<*mut std::ffi::c_void>,
    ) -> TaskId {
        let tcb = Box::new(TaskControlBlock::new(context, capture_root));
        self.live += 1;
        if let Some(index) = self.free.pop() {
            let generation = match &self.slots[index as usize] {
                Slot::Free { generation } => *generation,
                Slot::Live { .. } => unreachable!("free list pointed at a live slot"),
            };
            self.slots[index as usize] = Slot::Live { generation, tcb };
            TaskId { index, generation }
        } else {
            let index = self.slots.len() as u32;
            self.slots.push(Slot::Live { generation: 0, tcb });
            TaskId {
                index,
                generation: 0,
            }
        }
    }

    /// The control block for `id`, or `None` if the id is stale or out of range.
    pub fn get(&self, id: TaskId) -> Option<&TaskControlBlock> {
        match self.slots.get(id.index as usize)? {
            Slot::Live { generation, tcb } if *generation == id.generation => Some(tcb),
            _ => None,
        }
    }

    /// Mutable [`get`](Self::get).
    pub fn get_mut(&mut self, id: TaskId) -> Option<&mut TaskControlBlock> {
        match self.slots.get_mut(id.index as usize)? {
            Slot::Live { generation, tcb } if *generation == id.generation => Some(tcb),
            _ => None,
        }
    }

    /// Frees `id`'s slot (dropping the `TaskContext`, so its stack is
    /// reclaimed) and bumps the slot generation so `id` is now stale.
    /// Returns the freed control block for any last inspection.
    pub fn remove(&mut self, id: TaskId) -> Option<Box<TaskControlBlock>> {
        let slot = self.slots.get_mut(id.index as usize)?;
        match slot {
            Slot::Live { generation, .. } if *generation == id.generation => {
                let next_generation = generation.wrapping_add(1);
                let Slot::Live { tcb, .. } = std::mem::replace(
                    slot,
                    Slot::Free {
                        generation: next_generation,
                    },
                ) else {
                    unreachable!()
                };
                self.free.push(id.index);
                self.live -= 1;
                Some(tcb)
            }
            _ => None,
        }
    }

    /// Every live task id, in slot order. Used by the collector to walk every
    /// task's root chain and by the executor's terminal-state scan.
    pub fn live_ids(&self) -> impl Iterator<Item = TaskId> + '_ {
        self.slots
            .iter()
            .enumerate()
            .filter_map(|(index, slot)| match slot {
                Slot::Live { generation, .. } => Some(TaskId {
                    index: index as u32,
                    generation: *generation,
                }),
                Slot::Free { .. } => None,
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::spawn_default;

    fn dummy() -> TaskContext {
        spawn_default(|_s| 0)
    }

    #[test]
    fn task_id_round_trips_through_bits() {
        let id = TaskId {
            index: 0xABCD,
            generation: 0x1234,
        };
        assert_eq!(TaskId::from_bits(id.to_bits()), id);
    }

    #[test]
    fn insert_get_remove() {
        let mut reg = TaskRegistry::new();
        assert!(reg.is_empty());
        let a = reg.insert(dummy());
        let b = reg.insert(dummy());
        assert_eq!(reg.len(), 2);
        assert!(reg.get(a).is_some());
        reg.get_mut(a).unwrap().state = TaskState::Running;
        assert_eq!(reg.get(a).unwrap().state, TaskState::Running);

        let removed = reg.remove(a);
        assert!(removed.is_some());
        assert_eq!(reg.len(), 1);
        assert!(reg.get(a).is_none(), "a stale id must not resolve");
        assert!(reg.get(b).is_some());
    }

    #[test]
    fn a_reused_slot_rejects_the_old_id() {
        let mut reg = TaskRegistry::new();
        let old = reg.insert(dummy());
        reg.remove(old);
        let new = reg.insert(dummy()); // same slot index, bumped generation
        assert_eq!(old.index_for_test(), new.index_for_test());
        assert_ne!(old, new);
        assert!(
            reg.get(old).is_none(),
            "the reused slot must reject the old id"
        );
        assert!(reg.get(new).is_some());
    }

    #[test]
    fn live_ids_skips_freed_slots() {
        let mut reg = TaskRegistry::new();
        let a = reg.insert(dummy());
        let b = reg.insert(dummy());
        let c = reg.insert(dummy());
        reg.remove(b);
        let live: Vec<_> = reg.live_ids().collect();
        assert_eq!(live, vec![a, c]);
    }

    #[test]
    fn a_reused_scope_slot_rejects_the_old_id() {
        let mut scopes = ScopeRegistry::new();
        let old = scopes.insert();
        scopes.remove(old);
        let new = scopes.insert();
        assert!(scopes.get(old).is_none());
        assert!(scopes.get(new).is_some());
    }

    impl TaskId {
        fn index_for_test(self) -> u32 {
            self.index
        }
    }
}
