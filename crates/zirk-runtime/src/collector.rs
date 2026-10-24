//! Mark-sweep garbage collector (roadmap Phase 4e, `fase-4e-colector-mark-sweep`).
//!
//! `docs/decisions/ADR-003-memoria.md`'s "Cierre de la decisión" fixed the
//! strategy this module implements: non-moving mark-sweep, root enumeration
//! through a function-granularity shadow stack built by codegen
//! (`crates/zirk-codegen-llvm/src/emit.rs`), collection triggered
//! cooperatively inside [`crate::memory::zirk_rt_alloc`] itself.
//!
//! # Threads (`ADR-019`, `parallel-cpu-regions`)
//!
//! The cooperative executor is still single-threaded, but a `parallel` region
//! runs CPU work on a pool of OS worker threads that allocate onto the same
//! heap. Collection is therefore *thread-aware* when — and only when — such a
//! region is active (`mod heap` picks the running executor's published heap;
//! `worker_pool_is_multithreaded` gates the path): a threshold crossing raises
//! [`stw::REQUESTED`] instead of collecting inline. Every registered mutator
//! parks at its next [`safepoint_poll`] or enters the quiescent native-call
//! state, and the executor coordinates collection over every registered
//! shadow-frame chain plus its own task chains. Threads outside the executor's
//! published heap keep using their thread-local bootstrap heaps.
//!
//! # Object header
//!
//! Every object `zirk_rt_alloc` returns starts with three pointer-sized
//! words (design D1 — grown from the one word codegen has always written,
//! the dispatch descriptor):
//!
//! ```text
//!    [ descriptor | next | size (mark bit in the high bit) | field₁ | … ]
//! ```
//!
//! `descriptor` is written by codegen (`FunctionEmitter`'s `Alloc` case),
//! exactly as before this change. `next` and `size` are this module's own —
//! nothing outside the collector reads or writes them. `next` threads every
//! live allocation onto one intrusive singly-linked list, walked by sweep.
//! The mark bit lives in the otherwise-unused high bit of the size word:
//! valid `Layout`s are bounded by `isize::MAX`. Keeping `next` as a pointer
//! (rather than packing its address into an integer) preserves provenance.
//!
//! The descriptor itself carries one more table than it used to
//! (`crates/zirk-codegen-llvm/src/emit.rs`'s own descriptor-building loop,
//! design D6): after the existing method table / ancestor list / contract
//! tables, a count and then that many byte offsets — where, inside an
//! object of this class, a collector-managed reference field lives. Mark
//! reads this table to walk an object's own fields the same way
//! [`crate::memory::zirk_rt_check_cast`] already walks its ancestor list;
//! nothing about those existing reads changes, since this table sits
//! strictly after everything they already stop at.

use std::alloc::{Layout, dealloc};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::ffi::c_void;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

#[path = "gc_mutex.rs"]
pub(crate) mod gc_mutex;

/// Allocation membership is indexed only by allocator-issued addresses. A
/// compact integer mixer avoids paying the default randomized byte hasher on
/// every collector edge while HashSet still performs exact key equality.
#[derive(Default)]
pub(crate) struct AddressHasher(u64);

impl std::hash::Hasher for AddressHasher {
    fn finish(&self) -> u64 {
        let mut value = self.0;
        value ^= value >> 30;
        value = value.wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value ^= value >> 27;
        value = value.wrapping_mul(0x94d0_49bb_1331_11eb);
        value ^ (value >> 31)
    }

    fn write(&mut self, bytes: &[u8]) {
        let mut value = 0xcbf2_9ce4_8422_2325_u64;
        for byte in bytes {
            value = (value ^ u64::from(*byte)).wrapping_mul(0x100_0000_01b3);
        }
        self.0 = value;
    }

    fn write_usize(&mut self, value: usize) {
        self.0 = value as u64;
    }
}

pub(crate) type AllocationSet = HashSet<usize, std::hash::BuildHasherDefault<AddressHasher>>;

/// Every collector allocation is aligned to at least this many bytes,
/// regardless of what codegen actually requested for a given class'
/// fields — which is what lets [`sweep`] `dealloc` correctly using only the
/// `size` the header stores (design D1 names `size`, not alignment, as the
/// header's third word). The widest field type this compiler builds today
/// is `Float128`, so 16 bytes covers every alignment codegen can ask for; a
/// wider one later would need this constant raised, not a fourth header
/// word, since over-aligning the base allocation never perturbs the
/// *internal* field offsets LLVM computed independently.
const GC_ALIGN: usize = 16;

/// Word offset of `next` inside an object's header (design D1).
const NEXT_WORD: usize = 1;
/// Word offset of `size` inside an object's header (design D1).
const SIZE_WORD: usize = 2;

/// Byte size of the three-word header every collector allocation carries
/// (design D1) — every real, codegen-driven allocation is already at least
/// this large (`object_struct` always emits the header as its first three
/// fields), but [`crate::memory::zirk_rt_alloc`] clamps up to this floor
/// regardless: [`register`] unconditionally writes `next`/`size` into words
/// 1 and 2, so serving anything smaller would write past the allocation.
pub(crate) const HEADER_BYTES: usize = 3 * std::mem::size_of::<usize>();
/// The mark bit hidden in the size word's high bit. `Layout` rejects sizes
/// larger than `isize::MAX`, so this bit is clear for every valid allocation.
const MARK_BIT: usize = 1usize << (usize::BITS - 1);
const SIZE_MASK: usize = !MARK_BIT;

/// Raw descriptors used by dynamic array/list backing buffers. They are
/// metadata rather than collector objects, so sweep must release them with
/// the exact layout used at allocation time.
static DYNAMIC_DESCRIPTORS: OnceLock<Mutex<HashMap<usize, Layout>>> = OnceLock::new();

pub(crate) fn register_dynamic_descriptor(pointer: *mut c_void, layout: Layout) {
    DYNAMIC_DESCRIPTORS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .expect("dynamic descriptor registry poisoned")
        .insert(pointer as usize, layout);
}

pub(crate) unsafe fn release_dynamic_descriptor(pointer: *mut c_void) {
    if pointer.is_null() {
        return;
    }
    let layout = DYNAMIC_DESCRIPTORS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .expect("dynamic descriptor registry poisoned")
        .remove(&(pointer as usize));
    if let Some(layout) = layout {
        unsafe { dealloc(pointer as *mut u8, layout) };
    }
}

/// Word offset of a WeakCell's own single field — the target's address
/// (`fase-4e-weak`, design D1/D2) — right after the three-word header,
/// the same convention `crates/zirk-codegen-llvm/src/emit.rs`'s
/// `OBJECT_HEADER_FIELDS` uses for an ordinary object's own first field.
const WEAK_TARGET_WORD: usize = 3;

/// The WeakCell sentinel descriptor (`fase-4e-weak`, design D2): a fixed
/// static, not a per-class table like an ordinary object's own descriptor.
/// `crates/zirk-codegen-llvm/src/runtime.rs` declares an external global of
/// the same name and stores its address into a fresh WeakCell's header word
/// 0 (`InstKind::WeakFrom`'s own lowering) — this is the one real
/// definition, resolved at link time.
///
/// Its value is never read; only its address matters, as a sentinel every
/// WeakCell's descriptor word compares equal to and no real class descriptor
/// ever does (those are codegen-emitted per-class globals of their own).
#[unsafe(no_mangle)]
#[allow(non_upper_case_globals)]
pub static zirk_rt_weak_cell_descriptor: u8 = 0;

/// Set to a nonzero byte the first time a `Weak<T>` is ever allocated
/// (`fase-4e-weak`, design's own risk mitigation) — [`InstKind::WeakFrom`]'s
/// own lowering stores `1` into it directly (a plain store, no runtime
/// call), and [`clear_dead_weak_cells`] reads it before walking the
/// allocation list at all, so a program that never uses `Weak<T>` pays only
/// the one check per collection.
///
/// `static mut` rather than an ordinary immutable `static`, since generated
/// code writes to it directly — read and written only as a whole byte, never
/// referenced, so this is sound under the single-threaded execution model
/// this collector already assumes everywhere else (no `parallel`/`thread`
/// until Phase 5, `docs/decisions/ADR-003-memoria.md`'s own closing note).
#[unsafe(no_mangle)]
#[allow(non_upper_case_globals)]
pub static mut zirk_rt_weak_cell_ever_allocated: u8 = 0;

/// Descriptor for collector-managed `String`/`Char` objects.
///
/// The payload is a `ZirkString` (private to `crates/zirk-runtime/src/string.rs`).
/// Its optional backing handle is a strong GC edge after an indexed mutation;
/// inline bytes remain ordinary owned data rather than GC references.
#[unsafe(no_mangle)]
#[allow(non_upper_case_globals)]
pub static zirk_rt_string_descriptor: [usize; 5] = [
    0,
    0,
    0,
    1,
    HEADER_BYTES + crate::string::ZirkString::BACKING_OFFSET,
];

#[inline]
fn weak_cell_descriptor() -> *mut c_void {
    (&raw const zirk_rt_weak_cell_descriptor) as *mut c_void
}

#[inline]
pub(crate) fn string_descriptor() -> *mut c_void {
    zirk_rt_string_descriptor.as_ptr() as *mut c_void
}

#[inline]
fn weak_cell_ever_allocated() -> bool {
    unsafe { zirk_rt_weak_cell_ever_allocated != 0 }
}

#[inline]
unsafe fn get_weak_target(object: *mut c_void) -> *mut c_void {
    unsafe { *(header_word(object, WEAK_TARGET_WORD) as *mut *mut c_void) }
}

#[inline]
unsafe fn set_weak_target(object: *mut c_void, value: *mut c_void) {
    unsafe { *(header_word(object, WEAK_TARGET_WORD) as *mut *mut c_void) = value };
}

/// A function that visits every garbage-collection root in every live task's
/// frame chain, calling the supplied marker on each candidate object. The
/// executor registers one for the duration of a run (design D4).
pub(crate) type TaskRootWalker = fn(&mut dyn FnMut(*mut c_void));

pub(crate) struct Frame {
    /// Address of an array of root addresses (design D2): element `i` is
    /// the address of a reference-typed slot (or one of its own
    /// reference-typed fields) — not the reference's value, which is why
    /// [`mark`] dereferences each entry once more to reach the candidate
    /// object.
    pub(crate) roots: *mut *mut c_void,
    pub(crate) count: usize,
}

const MAX_GLOBAL_ROOTS: usize = 4_096;
static NEXT_GLOBAL_ROOT: AtomicU64 = AtomicU64::new(1);
static GLOBAL_ROOTS: OnceLock<Mutex<HashMap<u64, usize>>> = OnceLock::new();

fn global_roots() -> &'static Mutex<HashMap<u64, usize>> {
    GLOBAL_ROOTS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Retains a managed object independently of the current generated stack
/// frame. Long-lived native callbacks must release the returned handle on
/// every completion and cancellation path.
///
/// # Safety
/// `value` must be null or a pointer value previously issued by this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_global_root_add(value: *mut c_void) -> u64 {
    if value.is_null() {
        return 0;
    }
    let mut roots = global_roots()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if roots.len() >= MAX_GLOBAL_ROOTS {
        return 0;
    }
    let handle = NEXT_GLOBAL_ROOT.fetch_add(1, Ordering::Relaxed);
    if handle == 0 {
        return 0;
    }
    roots.insert(handle, value as usize);
    handle
}

/// Releases a root previously registered with [`zirk_rt_global_root_add`].
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_global_root_remove(handle: u64) -> bool {
    global_roots()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .remove(&handle)
        .is_some()
}

thread_local! {
    /// The **bootstrap** pushed-frame stack: the root chain used before task 0
    /// exists (a compiled `main` prologue can allocate before the executor has
    /// finished wiring itself) and whenever no task is running. Once the
    /// executor is driving a task, [`ACTIVE_FRAMES`] points at *that task's*
    /// chain instead (`fase-5-executor-core` design D4).
    ///
    /// A plain `Vec` rather than an intrusive list: `push_frame`/`pop_frame`
    /// already happen in perfect call/return (LIFO) order.
    static BOOTSTRAP_FRAMES: RefCell<Vec<Frame>> = const { RefCell::new(Vec::new()) };

    /// The running task's frame chain, or null to fall back to
    /// [`BOOTSTRAP_FRAMES`]. Set by the executor around every
    /// `TaskContext::resume` to a `*mut Vec<Frame>` living inside that task's
    /// (boxed, address-stable) control block.
    static ACTIVE_FRAMES: Cell<*mut Vec<Frame>> = const { Cell::new(std::ptr::null_mut()) };

    /// When the executor is running, a function that visits the roots of
    /// *every live task's* frame chain (design D4). `mark` calls it in addition
    /// to the bootstrap chain, exactly as it already calls
    /// [`crate::clone::mark_clone_roots`].
    static TASK_ROOT_WALKER: Cell<Option<TaskRootWalker>> =
        const { Cell::new(None) };

    /// Identity of a non-coordinator thread in the published heap's mutator
    /// registry. The coordinator is identified by TASK_ROOT_WALKER instead.
    static MUTATOR_ID: Cell<Option<u64>> = const { Cell::new(None) };
}

/// The non-moving heap (`ADR-003`).
///
/// # Which heap
///
/// Before `parallel-cpu-regions` every allocation and collection happened on
/// the one executor thread, so this state was `thread_local!`. The worker pool
/// now allocates from other OS threads while running a `parallel` region, and
/// those allocations must land on the *same* list the executor's collector
/// sweeps.
///
/// So there are two heaps and [`with_heap`] picks between them:
///
/// - a **published** heap owned by the running [`crate::executor::Executor`]
///   (a boxed, address-stable [`HeapState`] behind a `Mutex`, its pointer in
///   [`PUBLISHED_HEAP`]) — used whenever an executor is running, by the
///   executor thread *and* by every `parallel` worker thread;
/// - a per-thread **bootstrap** heap ([`BOOTSTRAP_HEAP`]) — used before any
///   executor starts and by unit tests that allocate without one.
///
/// `crate::executor::Executor::run_with_root` holds a process-wide mutex for
/// its whole run and publishes its heap for the duration, so at most one
/// executor heap is ever live and a stop-the-world collection over it only
/// races the executor's own parked workers (`ADR-019` D4) — never another
/// executor.
mod heap {
    use super::c_void;
    use std::cell::RefCell;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicPtr, Ordering};

    pub(super) struct HeapState {
        /// Head of the intrusive all-allocations list `next` threads through
        /// (design D1/D6).
        pub(super) head: *mut c_void,
        /// Sum of every live allocation's own `size` — compared against
        /// `threshold` before serving a new allocation (design D3).
        pub(super) live_bytes: usize,
        /// Bytes of live allocation allowed before a collection runs.
        pub(super) threshold: usize,
        /// Exact base addresses of current allocations. Membership is checked
        /// before reading a candidate object's header.
        pub(super) allocations: super::AllocationSet,
    }

    impl HeapState {
        fn new() -> Self {
            HeapState {
                head: std::ptr::null_mut(),
                live_bytes: 0,
                threshold: super::default_threshold().max(1),
                allocations: super::AllocationSet::default(),
            }
        }
    }

    /// A published executor heap. `Mutex` because several `parallel` worker
    /// threads register allocations onto it concurrently.
    pub(crate) struct PublishedHeap(pub(super) Mutex<HeapState>);
    impl PublishedHeap {
        pub(crate) fn new() -> Box<Self> {
            Box::new(PublishedHeap(Mutex::new(HeapState::new())))
        }
    }

    static PUBLISHED_HEAP: AtomicPtr<PublishedHeap> = AtomicPtr::new(std::ptr::null_mut());

    thread_local! {
        static BOOTSTRAP_HEAP: RefCell<HeapState> = RefCell::new(HeapState::new());
        /// Whether *this* thread is part of the running executor's world — the
        /// executor thread itself, and a `parallel` worker while it runs a
        /// chunk. Only such threads allocate onto the published heap; every
        /// other thread (bootstrap, and unrelated threads in the unit-test
        /// binary) keeps using its own bootstrap heap. This is what stops a
        /// published heap from hijacking allocations it has nothing to do with.
        static IN_EXECUTOR_WORLD: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    }

    /// Publishes `heap` as the current executor heap. Called by
    /// `Executor::run_with_root` under the process-wide GC-world mutex, so
    /// there is never a second publisher.
    pub(super) fn publish(heap: *mut PublishedHeap) {
        PUBLISHED_HEAP.store(heap, Ordering::Release);
    }

    pub(super) fn unpublish() {
        PUBLISHED_HEAP.store(std::ptr::null_mut(), Ordering::Release);
    }

    #[cfg(test)]
    pub(super) fn is_published_for_tests() -> bool {
        !PUBLISHED_HEAP.load(Ordering::Acquire).is_null()
    }

    /// Marks the calling thread as part of the executor's world for as long as
    /// the returned guard lives.
    pub(super) fn enter_world() -> WorldGuard {
        let previous = IN_EXECUTOR_WORLD.replace(true);
        WorldGuard(previous)
    }

    pub(crate) struct WorldGuard(bool);
    impl Drop for WorldGuard {
        fn drop(&mut self) {
            IN_EXECUTOR_WORLD.with(|c| c.set(self.0));
        }
    }

    pub(crate) fn in_executor_world() -> bool {
        IN_EXECUTOR_WORLD.with(std::cell::Cell::get)
    }

    /// Runs `f` against whichever heap is current: the published executor heap
    /// if this thread is in the executor's world, else this thread's bootstrap
    /// heap.
    pub(super) fn with_heap<R>(f: impl FnOnce(&mut HeapState) -> R) -> R {
        if in_executor_world() {
            let published = PUBLISHED_HEAP.load(Ordering::Acquire);
            debug_assert!(
                !published.is_null(),
                "in executor world with no published heap"
            );
            // SAFETY: the pointer targets a boxed `PublishedHeap` kept alive by
            // the running `Executor` for its whole run; a thread only has
            // `IN_EXECUTOR_WORLD` set while that run is in progress. Concurrent
            // access is serialized by the inner `Mutex`.
            let heap: &PublishedHeap = unsafe { &*published };
            f(&mut heap.0.lock().unwrap())
        } else {
            BOOTSTRAP_HEAP.with(|h| f(&mut h.borrow_mut()))
        }
    }
}

use heap::with_heap;
pub(crate) use heap::{WorldGuard, in_executor_world};

/// RAII guard: the calling thread is part of the running executor's world (uses
/// the published heap, may take the stop-the-world path) until it is dropped.
pub(crate) fn enter_executor_world() -> heap::WorldGuard {
    heap::enter_world()
}

/// The head of the intrusive all-allocations list, as a raw pointer.
#[cfg(test)]
fn all_objects_head() -> *mut c_void {
    with_heap(|h| h.head)
}

/// Current live-byte total.
#[cfg(test)]
fn live_bytes_total() -> usize {
    with_heap(|h| h.live_bytes)
}

/// The executor publishes its heap for the duration of a run — re-exported so
/// `crate::executor` can call it without reaching into `mod heap`.
pub(crate) fn publish_executor_heap(heap: *mut heap::PublishedHeap) {
    heap::publish(heap);
}
pub(crate) fn unpublish_executor_heap() {
    heap::unpublish();
}
pub(crate) use heap::PublishedHeap;

/// Frees every allocation still on `heap`'s list — the executor calls this on
/// its own heap right after unpublishing it, at the end of a run, so a program
/// does not leak its whole live set until process exit (`ADR-003`'s bound is
/// now met eagerly). No marking: at end of run nothing is a root any more.
pub(crate) fn free_all(heap: &mut heap::PublishedHeap) {
    let state = heap.0.get_mut().unwrap();
    let _ = sweep(state.head, &mut state.allocations);
    state.head = std::ptr::null_mut();
    state.live_bytes = 0;
    state.allocations.clear();
}

/// Test-only: free every object currently on the heap list (none is marked
/// outside a collection) and zero the totals.
#[cfg(test)]
fn heap_hard_reset() {
    with_heap(|heap| {
        let _ = sweep(heap.head, &mut heap.allocations);
        heap.head = std::ptr::null_mut();
        heap.live_bytes = 0;
        heap.allocations.clear();
    });
}

/// Test-only: set the current heap's collection threshold.
#[cfg(test)]
fn set_threshold_bytes(bytes: usize) {
    with_heap(|heap| heap.threshold = bytes.max(1));
}

/// 1 MiB: generous enough that an ordinary short-lived program never
/// collects at all, small enough that a genuinely long-running one bounds
/// its own memory rather than growing forever, the way `memory.rs`'s old
/// "never frees" comment used to require the operating system to do at
/// process exit.
const DEFAULT_THRESHOLD_BYTES: usize = 1 << 20;

fn default_threshold() -> usize {
    std::env::var("ZIRK_GC_THRESHOLD")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(DEFAULT_THRESHOLD_BYTES)
}

#[inline]
unsafe fn header_word(object: *mut c_void, word: usize) -> *mut usize {
    unsafe { (object as *mut usize).add(word) }
}

#[inline]
unsafe fn get_next(object: *mut c_void) -> *mut c_void {
    unsafe { *((object as *mut *mut c_void).add(NEXT_WORD)) }
}

#[inline]
unsafe fn set_next(object: *mut c_void, value: *mut c_void) {
    unsafe { *((object as *mut *mut c_void).add(NEXT_WORD)) = value };
}

#[inline]
unsafe fn get_size(object: *mut c_void) -> usize {
    unsafe { get_size_raw(object) & SIZE_MASK }
}

#[inline]
unsafe fn get_size_raw(object: *mut c_void) -> usize {
    unsafe { *header_word(object, SIZE_WORD) }
}

/// `object`'s own header size word — [`crate::clone`]'s own read of "how
/// many bytes does this allocation occupy", the same word [`get_size`]
/// reads, exposed across the module boundary since the deep-clone
/// traversal needs it to size its own allocation identically (design D2).
///
/// # Safety
///
/// `object` must be a live, non-null allocation this collector produced.
pub(crate) unsafe fn object_size(object: *mut c_void) -> usize {
    unsafe { get_size(object) }
}

/// `object`'s own header descriptor word (word 0) — [`crate::clone`]'s own
/// read of "what class is this", needed both to copy into the fresh clone
/// and to find its [`gc_field_offsets`] table.
///
/// # Safety
///
/// `object` must be a live, non-null allocation this collector produced.
pub(crate) unsafe fn object_descriptor(object: *mut c_void) -> *mut c_void {
    unsafe { *(object as *mut *mut c_void) }
}

/// Checks exact-base membership without reading a candidate pointer's header.
/// Runtime handle ABIs use this before inspecting a managed object supplied by
/// generated code.
pub(crate) fn is_managed_allocation(object: *mut c_void) -> bool {
    !object.is_null() && with_heap(|heap| heap.allocations.contains(&(object as usize)))
}

#[inline]
unsafe fn set_size(object: *mut c_void, value: usize) {
    unsafe { *header_word(object, SIZE_WORD) = value & SIZE_MASK };
}

#[inline]
unsafe fn mark_object(object: *mut c_void) {
    unsafe { *header_word(object, SIZE_WORD) |= MARK_BIT };
}

#[inline]
fn is_marked(object: *mut c_void) -> bool {
    unsafe { get_size_raw(object) & MARK_BIT != 0 }
}

#[inline]
fn next_ptr(object: *mut c_void) -> *mut c_void {
    unsafe { get_next(object) }
}

/// Reads a class descriptor's own GC-field-offset table
/// (`crates/zirk-codegen-llvm/src/emit.rs`'s descriptor-building loop,
/// design D6): the same descriptor `zirk_rt_check_cast` already reads,
/// with one more section appended past everything it stops at —
/// `[ ... existing sections ... | gc_field_count | gc_field_offset* ]`.
///
/// # Safety
///
/// `descriptor` must be one this compiler emitted (or null, in which case
/// an empty slice is returned — a `Value`/`Enum` field that is itself a
/// contract-typed null has nothing to walk).
pub(crate) unsafe fn gc_field_offsets<'a>(descriptor: *const c_void) -> &'a [usize] {
    if descriptor.is_null() {
        return &[];
    }
    unsafe {
        let words = descriptor as *const usize;
        let ancestor_count = *words.add(1);
        let contract_count_slot = 2 + ancestor_count;
        let contract_count = *words.add(contract_count_slot);
        let gc_count_slot = contract_count_slot + 1 + contract_count * 2;
        let gc_count = *words.add(gc_count_slot);
        std::slice::from_raw_parts(words.add(gc_count_slot + 1), gc_count)
    }
}

/// Pushes this function activation's shadow-stack frame (design D2) —
/// codegen calls this once at function entry, after every reference-typed
/// slot has been zero-initialized (design D5).
///
/// # Safety
///
/// `roots` must be null (when `count` is 0) or point to an array of
/// `count` addresses, each themselves pointing at a reference-typed slot
/// (or field) that outlives this frame — exactly what codegen builds. Must
/// be paired with exactly one [`zirk_rt_pop_frame`] before this activation
/// returns.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_push_frame(roots: *mut *mut c_void, count: i64) {
    with_active_frames(|frames| {
        frames.push(Frame {
            roots,
            count: count as usize,
        });
    });
}

/// Pops the shadow-stack frame [`zirk_rt_push_frame`] pushed — codegen calls
/// this immediately before every `Terminator::Return` lowers to `ret`
/// (design D2).
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_pop_frame() {
    with_active_frames(|frames| {
        frames.pop();
    });
}

/// Runs `f` on whichever frame chain is active on this thread: the running
/// task's ([`ACTIVE_FRAMES`]), or [`BOOTSTRAP_FRAMES`] when no task is.
fn with_active_frames<R>(f: impl FnOnce(&mut Vec<Frame>) -> R) -> R {
    let active = ACTIVE_FRAMES.get();
    if active.is_null() {
        BOOTSTRAP_FRAMES.with(|frames| f(&mut frames.borrow_mut()))
    } else {
        // SAFETY: the executor points `ACTIVE_FRAMES` at a `Vec<Frame>` inside a
        // boxed, address-stable `TaskControlBlock`, and clears it before that
        // box can be dropped. Single-threaded, so there is no concurrent access.
        f(unsafe { &mut *active })
    }
}

/// The executor calls this around each `TaskContext::resume`: a `*mut Vec<Frame>`
/// into the running task's control block while it runs, null afterwards.
pub(crate) fn set_active_frames(frames: *mut Vec<Frame>) {
    ACTIVE_FRAMES.set(frames);
}

/// The executor registers a walker over every live task's frame chain for the
/// duration of its run, and clears it (null) on exit.
pub(crate) fn set_task_root_walker(walker: Option<TaskRootWalker>) {
    TASK_ROOT_WALKER.set(walker);
}

/// The stop-the-world safepoint protocol (`ADR-019` D4, `parallel-cpu-regions`
/// tasks 3.1–3.3), generalized to every registered mutator.
///
/// A collection in the published executor world is coordinated by the
/// executor. A gate-protected participant registry freezes admission/removal,
/// waits for each mutator to park or enter `InNative`, then scans every root
/// chain before reopening the gate. Bootstrap heaps still collect inline.
mod stw {
    use super::Frame;
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Condvar, Mutex};

    /// A stop-the-world collection has been requested and not yet completed.
    pub(super) static REQUESTED: AtomicBool = AtomicBool::new(false);
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    #[allow(dead_code)] // `InNative` is wired when the compiler enables managed FFI calls.
    enum ParticipantState {
        Running,
        Parked,
        InNative,
    }

    struct Participant {
        roots: *mut Vec<Frame>,
        state: ParticipantState,
    }
    // Only the coordinator reads a root pointer, after all participants have
    // become quiescent. GATE serializes registration, transitions, and removal.
    unsafe impl Send for Participant {}

    /// Admission and participant identity/state share one lock. Counts alone
    /// cannot provide the stable root-chain set required by mark/sweep.
    struct Gate {
        next_id: u64,
        participants: HashMap<u64, Participant>,
    }
    static GATE: std::sync::LazyLock<Mutex<Gate>> = std::sync::LazyLock::new(|| {
        Mutex::new(Gate {
            next_id: 1,
            participants: HashMap::new(),
        })
    });
    static PARK_CV: Condvar = Condvar::new();
    static ALL_PARKED_CV: Condvar = Condvar::new();

    /// Reopens the mutator gate even if mark/sweep unwinds. Otherwise parked,
    /// native-returning, and unregistering participants can wait forever.
    struct CollectionRequestReset;

    impl Drop for CollectionRequestReset {
        fn drop(&mut self) {
            let _gate = GATE.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            REQUESTED.store(false, Ordering::SeqCst);
            PARK_CV.notify_all();
            ALL_PARKED_CV.notify_all();
        }
    }

    fn participant_mut(gate: &mut Gate, id: u64) -> &mut Participant {
        gate.participants
            .get_mut(&id)
            .unwrap_or_else(|| panic!("GC mutator {id} is not registered"))
    }

    pub(super) fn register(roots: *mut Vec<Frame>) -> u64 {
        let mut gate = GATE.lock().unwrap();
        while REQUESTED.load(Ordering::SeqCst) {
            gate = PARK_CV.wait(gate).unwrap();
        }
        let id = gate.next_id;
        gate.next_id = gate.next_id.checked_add(1).expect("mutator id exhausted");
        let previous = gate.participants.insert(
            id,
            Participant {
                roots,
                state: ParticipantState::Running,
            },
        );
        debug_assert!(previous.is_none());
        id
    }

    pub(super) fn unregister(id: u64) {
        let mut gate = GATE.lock().unwrap();
        while REQUESTED.load(Ordering::SeqCst) {
            participant_mut(&mut gate, id).state = ParticipantState::Parked;
            ALL_PARKED_CV.notify_all();
            gate = PARK_CV.wait(gate).unwrap();
        }
        assert!(
            gate.participants.remove(&id).is_some(),
            "unregister called for unknown GC mutator {id}"
        );
    }

    /// Closes admission before waking the executor coordinator. The same
    /// mutex protects worker entry, so no participant can slip into the heap
    /// after the coordinator takes its parked-set snapshot.
    pub(super) fn request_collection() {
        let _gate = GATE.lock().unwrap();
        REQUESTED.store(true, Ordering::SeqCst);
        ALL_PARKED_CV.notify_all();
    }

    /// A worker parks here until the coordinator finishes the collection.
    pub(super) fn park(id: u64) {
        let mut generation = GATE.lock().unwrap();
        while REQUESTED.load(Ordering::SeqCst) {
            participant_mut(&mut generation, id).state = ParticipantState::Parked;
            ALL_PARKED_CV.notify_all();
            generation = PARK_CV.wait(generation).unwrap();
        }
        participant_mut(&mut generation, id).state = ParticipantState::Running;
    }

    #[allow(dead_code)] // Kept crate-private until FFI call sites carry the audited contract.
    pub(super) fn enter_native(id: u64) {
        let mut gate = GATE.lock().unwrap();
        loop {
            if REQUESTED.load(Ordering::SeqCst) {
                participant_mut(&mut gate, id).state = ParticipantState::Parked;
                ALL_PARKED_CV.notify_all();
                gate = PARK_CV.wait(gate).unwrap();
            } else {
                participant_mut(&mut gate, id).state = ParticipantState::InNative;
                return;
            }
        }
    }

    #[allow(dead_code)] // Kept crate-private until FFI call sites carry the audited contract.
    pub(super) fn resume_from_native(id: u64) {
        let mut gate = GATE.lock().unwrap();
        while REQUESTED.load(Ordering::SeqCst) {
            assert_eq!(
                participant_mut(&mut gate, id).state,
                ParticipantState::InNative,
                "only an InNative GC mutator may wait to resume"
            );
            gate = PARK_CV.wait(gate).unwrap();
        }
        assert_eq!(
            participant_mut(&mut gate, id).state,
            ParticipantState::InNative,
            "only an InNative GC mutator may resume"
        );
        participant_mut(&mut gate, id).state = ParticipantState::Running;
    }

    pub(super) fn has_participants() -> bool {
        !GATE.lock().unwrap().participants.is_empty()
    }

    #[cfg(test)]
    pub(super) fn participant_count_for_tests() -> usize {
        GATE.lock().unwrap().participants.len()
    }

    pub(super) fn participant_roots() -> Vec<*mut Vec<Frame>> {
        let gate = GATE.lock().unwrap();
        debug_assert!(REQUESTED.load(Ordering::SeqCst));
        debug_assert!(
            gate.participants
                .values()
                .all(|participant| participant.state != ParticipantState::Running)
        );
        gate.participants
            .values()
            .map(|participant| participant.roots)
            .collect()
    }

    /// The executor thread coordinates one collection: wait for every active
    /// worker to park, run `collect`, then release them.
    pub(super) fn coordinate(collect: impl FnOnce()) {
        let reset_request = CollectionRequestReset;
        let mut gate = GATE.lock().unwrap();
        while gate
            .participants
            .values()
            .any(|participant| participant.state == ParticipantState::Running)
        {
            gate = ALL_PARKED_CV.wait(gate).unwrap();
        }
        drop(gate);

        // Admission, resume from native code, and unregistration remain closed
        // until the request is cleared, so both the set and roots stay stable.
        collect();

        drop(reset_request);
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::collector::{
            PublishedHeap, enter_executor_world, free_all, is_live_for_tests,
            publish_executor_heap, unpublish_executor_heap,
        };
        use std::ffi::c_void;
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::{Barrier, mpsc};

        #[test]
        fn coordinator_waits_for_active_worker_and_releases_it_after_collection() {
            let _guard = crate::collector::test_support::test_guard();
            let _gc_world = crate::executor::lock_gc_world_for_test();
            let mutator = super::super::register_managed_mutator();
            request_collection();

            let (collected_tx, collected_rx) = mpsc::channel();
            let coordinator = std::thread::spawn(move || {
                coordinate(|| collected_tx.send(()).unwrap());
            });

            park(mutator.id);
            collected_rx
                .recv_timeout(std::time::Duration::from_secs(1))
                .expect("coordinator must collect after the worker parks");
            coordinator.join().unwrap();
            drop(mutator);

            let gate = GATE.lock().unwrap();
            assert!(gate.participants.is_empty());
            assert!(!REQUESTED.load(Ordering::SeqCst));
        }

        #[test]
        fn admission_waits_until_collection_reopens_the_gate() {
            let _guard = crate::collector::test_support::test_guard();
            let _gc_world = crate::executor::lock_gc_world_for_test();
            request_collection();
            let collection_finished = std::sync::Arc::new(AtomicBool::new(false));
            let thread_finished = std::sync::Arc::clone(&collection_finished);
            let start = std::sync::Arc::new(Barrier::new(2));
            let thread_start = std::sync::Arc::clone(&start);
            let entrant = std::thread::spawn(move || {
                thread_start.wait();
                let mutator = super::super::register_managed_mutator();
                let collection_was_finished = thread_finished.load(Ordering::SeqCst);
                drop(mutator);
                collection_was_finished
            });

            start.wait();
            coordinate(|| collection_finished.store(true, Ordering::SeqCst));
            assert!(entrant.join().unwrap());

            let gate = GATE.lock().unwrap();
            assert!(gate.participants.is_empty());
            assert!(!REQUESTED.load(Ordering::SeqCst));
        }

        #[test]
        fn blocked_native_mutator_keeps_stable_roots_visible_to_collection() {
            let _guard = crate::collector::test_support::test_guard();
            let _gc_world = crate::executor::lock_gc_world_for_test();
            let mutator = super::super::register_managed_mutator();
            let mutator_id = mutator.id;
            let native = mutator.enter_blocking_native();
            request_collection();

            let (collected_tx, collected_rx) = mpsc::channel();
            let coordinator = std::thread::spawn(move || {
                coordinate(|| {
                    let roots = participant_roots();
                    assert_eq!(roots.len(), 1);
                    assert!(!roots[0].is_null());
                    let gate = GATE.lock().unwrap();
                    assert_eq!(
                        gate.participants[&mutator_id].state,
                        ParticipantState::InNative
                    );
                    collected_tx.send(()).unwrap();
                });
            });
            collected_rx
                .recv_timeout(std::time::Duration::from_secs(1))
                .expect("collection must finish while the mutator remains in native code");
            coordinator.join().unwrap();

            drop(native);
            drop(mutator);
            let gate = GATE.lock().unwrap();
            assert!(gate.participants.is_empty());
            assert!(!REQUESTED.load(Ordering::SeqCst));
        }

        #[test]
        fn unregister_waits_until_collection_stops_scanning_its_roots() {
            let _guard = crate::collector::test_support::test_guard();
            let _gc_world = crate::executor::lock_gc_world_for_test();
            let mutator = super::super::register_managed_mutator();
            request_collection();
            let collection_finished = std::sync::Arc::new(AtomicBool::new(false));
            let thread_finished = std::sync::Arc::clone(&collection_finished);
            let coordinator = std::thread::spawn(move || {
                coordinate(|| thread_finished.store(true, Ordering::SeqCst));
            });

            // Drop publishes the participant as quiescent, then remains
            // blocked until the coordinator has completed its root walk.
            drop(mutator);
            coordinator.join().unwrap();
            assert!(collection_finished.load(Ordering::SeqCst));
            assert!(GATE.lock().unwrap().participants.is_empty());
        }

        #[test]
        fn native_reentry_waits_until_collection_reopens_admission() {
            let _guard = crate::collector::test_support::test_guard();
            let _gc_world = crate::executor::lock_gc_world_for_test();
            let mutator = super::super::register_managed_mutator();
            let native = mutator.enter_blocking_native();
            request_collection();
            let collection_finished = std::sync::Arc::new(AtomicBool::new(false));
            let thread_finished = std::sync::Arc::clone(&collection_finished);
            let coordinator = std::thread::spawn(move || {
                coordinate(|| thread_finished.store(true, Ordering::SeqCst));
            });

            drop(native);
            assert!(collection_finished.load(Ordering::SeqCst));
            coordinator.join().unwrap();
            drop(mutator);
            assert!(GATE.lock().unwrap().participants.is_empty());
        }

        #[test]
        fn collection_unwind_reopens_gate_and_allows_native_cleanup() {
            let _guard = crate::collector::test_support::test_guard();
            let _gc_world = crate::executor::lock_gc_world_for_test();
            let mutator = super::super::register_managed_mutator();
            let native = mutator.enter_blocking_native();
            request_collection();

            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                coordinate(|| panic!("injected collector unwind"));
            }));
            assert!(result.is_err());
            assert!(!REQUESTED.load(Ordering::SeqCst));

            // Cleanup after a failed collection must restore Running so future
            // stop-the-world requests and unregister can make progress.
            drop(native);
            drop(mutator);
            assert!(GATE.lock().unwrap().participants.is_empty());
        }

        #[test]
        fn collection_preserves_object_rooted_by_registered_native_thread() {
            let _guard = crate::collector::test_support::test_guard();
            let _gc_world = crate::executor::lock_gc_world_for_test();
            let mut heap = PublishedHeap::new();
            publish_executor_heap(&mut *heap as *mut _);
            let world = enter_executor_world();
            let descriptor = crate::collector::test_support::descriptor_with_fields_for_tests(&[]);
            let descriptor_addr = descriptor.as_ptr() as usize;
            let (ready_tx, ready_rx) = mpsc::channel();
            let (resume_tx, resume_rx) = mpsc::channel();
            let worker = std::thread::spawn(move || {
                let mutator = super::super::register_managed_mutator();
                let object = unsafe {
                    crate::collector::test_support::synthetic_object_for_tests(
                        descriptor_addr as *const c_void,
                        0,
                    )
                };
                let mut slot = object;
                let mut roots = [&mut slot as *mut *mut c_void as *mut c_void];
                unsafe { crate::collector::zirk_rt_push_frame(roots.as_mut_ptr(), 1) };
                let native = mutator.enter_blocking_native();
                ready_tx.send(object as usize).unwrap();
                resume_rx.recv().unwrap();
                drop(native);
                crate::collector::zirk_rt_pop_frame();
                drop(mutator);
                object as usize
            });

            let object_addr = ready_rx.recv().unwrap();
            request_collection();
            coordinate(crate::collector::collect_stw);
            resume_tx.send(()).unwrap();
            worker.join().unwrap();
            assert!(is_live_for_tests(object_addr as *mut c_void));

            unpublish_executor_heap();
            free_all(&mut heap);
            drop(world);
            assert!(GATE.lock().unwrap().participants.is_empty());
        }
    }
}

#[cfg(test)]
pub(crate) fn shutdown_snapshot_for_tests() -> (bool, usize) {
    (
        heap::is_published_for_tests(),
        stw::participant_count_for_tests(),
    )
}

/// Scoped registration for a non-coordinator thread that can access the
/// executor's published heap. Its shadow-frame chain remains registered until
/// it has stopped using the heap and any in-flight collection has completed.
pub(crate) struct MutatorRegistration {
    id: u64,
    _world: WorldGuard,
    // Registration is tied to TLS roots and executor-world state; moving the
    // guard to a different OS thread would unregister/clear the wrong chain.
    _thread_bound: std::marker::PhantomData<std::rc::Rc<()>>,
}

impl MutatorRegistration {
    /// Temporarily leave managed execution around a blocking native call.
    /// While this guard is live, the native call must not touch managed values
    /// or call back into Zirk/runtime code; the stable shadow frames remain
    /// visible to the collector.
    #[allow(dead_code)] // No source/IR path may use this until FFI effects are enforced.
    pub(crate) fn enter_blocking_native(&self) -> BlockingNativeGuard {
        enter_blocking_native(self.id)
    }
}

impl Drop for MutatorRegistration {
    fn drop(&mut self) {
        stw::unregister(self.id);
        MUTATOR_ID.set(None);
        BOOTSTRAP_FRAMES.with(|frames| frames.borrow_mut().clear());
    }
}

/// RAII transition for a native call that cannot execute compiler safepoints.
#[allow(dead_code)] // Constructed by the future managed-FFI lowering.
pub(crate) struct BlockingNativeGuard {
    mutator_id: u64,
}

impl Drop for BlockingNativeGuard {
    fn drop(&mut self) {
        stw::resume_from_native(self.mutator_id);
    }
}

fn enter_blocking_native(mutator_id: u64) -> BlockingNativeGuard {
    stw::enter_native(mutator_id);
    BlockingNativeGuard { mutator_id }
}

/// Enters the collector's stable native state for a blocking runtime wait on a
/// registered managed worker. The caller must not access managed values while
/// the guard is live; dropping it resumes only after any active collection.
pub(crate) fn enter_blocking_native_current() -> Option<BlockingNativeGuard> {
    MUTATOR_ID.get().map(enter_blocking_native)
}

/// Whether the current thread is a registered non-coordinator managed worker.
pub(crate) fn is_registered_managed_worker() -> bool {
    MUTATOR_ID.get().is_some()
}

pub(crate) fn register_managed_mutator() -> MutatorRegistration {
    assert!(
        MUTATOR_ID.get().is_none(),
        "mutator already registered on this thread"
    );
    let world = enter_executor_world();
    let roots = BOOTSTRAP_FRAMES.with(|frames| frames.as_ptr());
    let id = stw::register(roots);
    MUTATOR_ID.set(Some(id));
    MutatorRegistration {
        id,
        _world: world,
        _thread_bound: std::marker::PhantomData,
    }
}

/// Drops shadow-frame entries only after generated code has fully unwound and
/// its stack slots are no longer valid. Call this before the next safepoint in
/// a native wrapper that catches an unwind from managed execution.
pub(crate) fn clear_unwound_mutator_roots() {
    assert!(
        MUTATOR_ID.get().is_some(),
        "mutator roots require registration"
    );
    BOOTSTRAP_FRAMES.with(|frames| frames.borrow_mut().clear());
}

/// Whether a multi-threaded `parallel` region is running right now — the
/// condition under which a threshold crossing defers to a safepoint instead of
/// collecting inline (`ADR-019` D4). False whenever the pool is size 1 or no
/// region is active.
fn executor_has_concurrent_mutators() -> bool {
    // Bootstrap heaps remain thread-local. Threads that join the published
    // heap use one registry regardless of whether they came from `parallel`
    // or a managed native-thread API.
    in_executor_world() && (crate::pool::pool_is_multithreaded() || stw::has_participants())
}

/// Whether this is the cooperative-executor thread (the only thread that may
/// coordinate a collection, since it alone can enumerate task roots).
fn on_executor_thread() -> bool {
    TASK_ROOT_WALKER.get().is_some()
}

/// Runs a collection if serving an allocation of `incoming` bytes would
/// cross the configured threshold (design D3) — called by
/// [`crate::memory::zirk_rt_alloc`] before it allocates anything.
///
/// While another registered mutator can access the published heap, this
/// raises [`stw::REQUESTED`] and cooperates at a [`safepoint_poll`] instead of
/// collecting inline: every participant must be quiescent and its roots
/// published before mark/sweep begins.
pub(crate) fn maybe_collect(incoming: usize) {
    let over_threshold = with_heap(|heap| heap.live_bytes + incoming > heap.threshold);
    if !over_threshold {
        return;
    }
    if executor_has_concurrent_mutators() {
        stw::request_collection();
        crate::executor::notify_pool_event();
        safepoint_poll();
    } else {
        collect();
    }
}

/// A safepoint: called at every `parallel` loop back-edge, after each region
/// chunk, and by the executor at its next scheduling turn. A registered
/// mutator parks; the executor thread coordinates the collection.
#[inline]
pub(crate) fn safepoint_poll() {
    if !stw::REQUESTED.load(std::sync::atomic::Ordering::SeqCst) {
        return;
    }
    if on_executor_thread() {
        stw::coordinate(collect_stw);
    } else {
        let id = MUTATOR_ID
            .get()
            .expect("only a registered executor mutator may park for stop-the-world");
        stw::park(id);
    }
}

/// Links a freshly allocated object onto the intrusive all-allocations
/// list and initializes its `next`/`size` header words (design D1) —
/// called by [`crate::memory::zirk_rt_alloc`] right after the underlying
/// allocation succeeds, before it stores the descriptor into word 0.
///
/// # Safety
///
/// `object` must be a fresh, zeroed allocation of at least `size` bytes,
/// not yet reachable from any root.
pub(crate) unsafe fn register(object: *mut c_void, size: usize) {
    with_heap(|heap| {
        let head = heap.head;
        unsafe {
            // Store the link as a pointer to retain allocator provenance.
            set_next(object, head);
            set_size(object, size);
        }
        heap.head = object;
        heap.live_bytes += size;
        heap.allocations.insert(object as usize);
    });
}

/// The alignment every collector allocation actually uses: always exactly
/// [`GC_ALIGN`], regardless of `requested` — not `requested.max(GC_ALIGN)`.
/// Design D1 stores only `size` in the header, not the alignment a
/// particular class asked for, so [`sweep`]'s `dealloc` has no way to
/// recover anything wider than a single fixed constant known at both ends;
/// pinning every allocation to that one constant (rather than letting it
/// float upward with `requested`) is what keeps allocation and
/// deallocation's `Layout`s provably identical without a fourth header
/// word. `GC_ALIGN` (16 bytes) already covers every alignment this
/// compiler's own types can demand (see its own doc comment) — the debug
/// assertion is what would catch it the day that stops being true, instead
/// of silently under-aligning.
pub(crate) fn allocation_align(requested: usize) -> usize {
    debug_assert!(
        requested <= GC_ALIGN,
        "a type needs {requested}-byte alignment, wider than the collector's fixed {GC_ALIGN}-byte floor"
    );
    GC_ALIGN
}

/// A single-threaded / bootstrap collection: only this thread runs Zirk code,
/// so only this thread's roots (and the clone-in-progress set) are walked.
fn collect() {
    collect_with(false);
}

/// A stop-the-world collection, run by the executor after every registered
/// mutator is parked or in the stable native-call state.
fn collect_stw() {
    collect_with(true);
}

fn collect_with(walk_mutator_chains: bool) {
    if std::env::var_os("ZIRK_GC_TRACE").is_some() {
        eprintln!("[zirk-gc] collection");
    }
    // One lock on the current heap for the whole collection. Every other
    // registered thread is parked or in the stable native-call state, so no
    // participant mutates the list or object headers during mark/sweep.
    with_heap(|heap| {
        mark(walk_mutator_chains, &heap.allocations);
        clear_dead_weak_cells(heap.head);
        let (survivors, live_bytes) = sweep(heap.head, &mut heap.allocations);
        heap.head = survivors;
        heap.live_bytes = live_bytes;
    });
}

fn mark(walk_mutator_chains: bool, allocations: &AllocationSet) {
    let pending = std::cell::RefCell::new(Vec::new());
    let mut enqueue = |object| enqueue_candidate(&pending, allocations, object);

    // The bootstrap chain (roots held before task 0, or with no task running).
    BOOTSTRAP_FRAMES.with(|frames| {
        for frame in frames.borrow().iter() {
            mark_frame_roots(frame, &mut enqueue);
        }
    });

    // Long-lived native callbacks have no live generated stack activation to
    // own their captures. Their explicit handles are bounded and removed by
    // the runtime when the callback completes or is canceled.
    {
        let roots = global_roots()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for value in roots.values() {
            enqueue(*value as *mut c_void);
        }
    }

    // Every registered non-coordinator mutator's chain. During stop-the-world
    // all are parked or inside an explicitly quiescent native call, and both
    // admission and unregistration remain closed.
    if walk_mutator_chains {
        for chain in stw::participant_roots() {
            let frames = unsafe { &*chain };
            for frame in frames.iter() {
                mark_frame_roots(frame, &mut enqueue);
            }
        }
    }

    // Every live task's chain (design D4) — including the one running right now
    // (whose chain `ACTIVE_FRAMES` also points at); marking is idempotent, so
    // visiting it twice is harmless.
    if let Some(walker) = TASK_ROOT_WALKER.get() {
        walker(&mut enqueue);
    }

    // `zirk-object-memory`'s own "Deep-clone traversal state is
    // collector-safe for its whole duration" requirement (roadmap Phase
    // 4e, `fase-4e-clone`, design D3): every destination object a
    // still-in-progress `clone()` call has produced so far is not yet
    // reachable from any ordinary shadow-stack root (only the *finished*
    // top-level result is, once its own instruction result is spilled) —
    // `crate::clone` tracks them itself and this is its own extra root
    // set, walked exactly like the shadow stack above.
    crate::clone::mark_clone_roots(&mut enqueue);
    let mut visited_exception_edges = HashSet::new();
    loop {
        loop {
            let object = { pending.borrow_mut().pop() };
            let Some(object) = object else { break };
            // Every worklist entry was membership-checked by enqueue_candidate.
            // The heap is stable for the duration of this stop-the-world/inline
            // collection, so no allocation can be freed between enqueue and
            // this header read; a second hash lookup here only duplicates the
            // exact-address safety check.
            if is_marked(object) {
                continue;
            }
            unsafe { mark_object(object) };

            let descriptor = unsafe { *(object as *mut *mut c_void) };

            // WeakCell is a live object but its target is deliberately weak.
            if std::ptr::eq(descriptor, weak_cell_descriptor()) {
                continue;
            }
            if std::ptr::eq(descriptor, crate::channel::channel_descriptor()) {
                unsafe { crate::channel::mark_queued(object, &mut enqueue) };
                continue;
            }
            if std::ptr::eq(descriptor, gc_mutex::descriptor()) {
                unsafe { gc_mutex::mark_payload(object, &mut enqueue) };
                continue;
            }
            if std::ptr::eq(descriptor, crate::map::map_descriptor()) {
                unsafe { crate::map::mark_entries(object, allocations, &mut enqueue) };
                continue;
            }
            if std::ptr::eq(descriptor, crate::object::object_descriptor()) {
                unsafe { crate::object::mark_properties(object, allocations, &mut enqueue) };
                continue;
            }
            if std::ptr::eq(descriptor, crate::object::dynamic_value_descriptor()) {
                unsafe { crate::object::mark_dynamic_value(object, allocations, &mut enqueue) };
                continue;
            }
            if std::ptr::eq(descriptor, crate::set::set_descriptor()) {
                unsafe { crate::set::mark_entries(object, allocations, &mut enqueue) };
                continue;
            }
            // Class instances use their fixed generated layout for declared
            // fields and an object sidecar for properties added through an
            // `Object` view. Trace that sidecar alongside the class fields.
            unsafe { crate::object::mark_properties(object, allocations, &mut enqueue) };
            let offsets = unsafe { gc_field_offsets(descriptor as *const c_void) };
            for &offset in offsets {
                let field_addr = (object as usize + offset) as *mut *mut c_void;
                let child = unsafe { *field_addr };
                enqueue(child);
            }
        }

        let mut added_exception_edge = false;
        crate::exceptions::visit_owned_edges(|owner, edge| {
            if allocations.contains(&(owner as usize))
                && is_marked(owner)
                && visited_exception_edges.insert((owner as usize, edge as usize))
            {
                enqueue_candidate(&pending, allocations, edge);
                added_exception_edge = true;
            }
        });
        if pending.borrow().is_empty() && !added_exception_edge {
            break;
        }
    }
}

fn mark_frame_roots(frame: &Frame, enqueue: &mut dyn FnMut(*mut c_void)) {
    for index in 0..frame.count {
        let root_addr = unsafe { *frame.roots.add(index) };
        if !root_addr.is_null() {
            let object = unsafe { *(root_addr as *mut *mut c_void) };
            enqueue(object);
        }
    }
}

fn enqueue_candidate(
    pending: &std::cell::RefCell<Vec<*mut c_void>>,
    allocations: &AllocationSet,
    object: *mut c_void,
) {
    if object.is_null() {
        return;
    }
    if !allocations.contains(&(object as usize)) {
        return;
    }
    let mut pending = pending.borrow_mut();
    if pending.try_reserve(1).is_err() {
        crate::failure::zirk_rt_allocation_failed();
    }
    pending.push(object);
}

/// Nulls every live WeakCell's target field whose referent turns out to be
/// unreachable after [`mark`] — strictly before [`sweep`] frees that
/// referent's storage (`fase-4e-weak`, design D3): `.upgrade()`/`.is_alive`
/// must never observe reclaimed memory, by construction, not by a race
/// against `dealloc`.
///
/// Walks the same intrusive all-allocations list [`sweep`] does, once, but
/// skips the walk entirely when no `Weak<T>` has ever been allocated in the
/// running program (design's own risk mitigation) — a program that never
/// uses `Weak<T>` pays only [`weak_cell_ever_allocated`]'s own check.
fn clear_dead_weak_cells(list_head: *mut c_void) {
    if !weak_cell_ever_allocated() {
        return;
    }

    #[cfg(test)]
    tests::note_clear_dead_weak_cells_ran();

    let mut current = list_head;
    while !current.is_null() {
        let next_object = next_ptr(current);

        if is_marked(current) {
            let descriptor = unsafe { *(current as *mut *mut c_void) };
            if descriptor == weak_cell_descriptor() {
                let target = unsafe { get_weak_target(current) };
                if !target.is_null() && !is_marked(target) {
                    unsafe { set_weak_target(current, std::ptr::null_mut()) };
                }
            }
        }

        current = next_object;
    }
}

fn sweep(list_head: *mut c_void, allocations: &mut AllocationSet) -> (*mut c_void, usize) {
    let mut current = list_head;
    let mut survivors: *mut c_void = std::ptr::null_mut();
    let mut live_bytes = 0usize;

    while !current.is_null() {
        let next_object = next_ptr(current);

        if is_marked(current) {
            // Relinks `current` onto the survivors list being built, in the
            // process clearing its size-word mark bit for the next cycle.
            let size = unsafe { get_size(current) };
            unsafe { set_next(current, survivors) };
            unsafe { set_size(current, size) };
            survivors = current;
            live_bytes += size;
        } else {
            let size = unsafe { get_size(current) };
            let descriptor = unsafe { object_descriptor(current) };
            unsafe { release_dynamic_descriptor(descriptor) };
            crate::exceptions::owner_swept(current);
            crate::object::owner_swept(current);
            // A channel owns Rust collections after its three-word GC header.
            // Run their destructor before freeing the raw collector block.
            if std::ptr::eq(descriptor, crate::channel::channel_descriptor()) {
                unsafe { crate::channel::drop_body(current) };
            } else if std::ptr::eq(descriptor, gc_mutex::descriptor()) {
                unsafe { gc_mutex::drop_object(current) };
            } else if std::ptr::eq(descriptor, crate::list::list_descriptor()) {
                unsafe { crate::list::drop_body(current) };
            } else if std::ptr::eq(descriptor, crate::map::map_descriptor()) {
                unsafe { crate::map::drop_body(current) };
            } else if std::ptr::eq(descriptor, crate::object::object_descriptor()) {
                unsafe { crate::object::drop_object_body(current) };
            } else if std::ptr::eq(descriptor, crate::object::dynamic_value_descriptor()) {
                unsafe { crate::object::drop_dynamic_value_body(current) };
            } else if std::ptr::eq(descriptor, crate::set::set_descriptor()) {
                unsafe { crate::set::drop_body(current) };
            }
            let layout = Layout::from_size_align(size, GC_ALIGN)
                .expect("this is the exact layout `zirk_rt_alloc` used to allocate it");
            unsafe { dealloc(current as *mut u8, layout) };
            allocations.remove(&(current as usize));
        }

        current = next_object;
    }

    (survivors, live_bytes)
}

/// Test-only support for `crate::clone`'s own synthetic-object tests
/// (`fase-4e-clone`) — thin `pub(crate)` wrappers around this module's own
/// private test helpers below, since a sibling module's `#[cfg(test)]` code
/// cannot reach into `mod tests`' private items directly.
#[cfg(test)]
pub(crate) mod test_support {
    use super::*;
    use std::alloc::alloc_zeroed;

    /// The one process-wide lock every collector-touching test holds for its
    /// whole body. The heap list, the live-byte total and the WeakCell
    /// "ever allocated" flag are all process-global now (`ADR-019` made the
    /// heap process-wide), and libtest runs `#[test]`s across worker threads,
    /// so `collector`'s own tests and `crate::clone`'s synthetic-object tests
    /// must serialize against the *same* mutex.
    pub(crate) static TEST_GUARD: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[must_use = "hold the guard for the whole test body"]
    pub(crate) fn test_guard() -> std::sync::MutexGuard<'static, ()> {
        TEST_GUARD.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Reports whether any registered native mutator remains after a managed
    /// worker has joined and the runtime has shut its worker pool down.
    pub(crate) fn has_registered_participants_for_tests() -> bool {
        stw::has_participants()
    }

    /// See `mod tests`' own `synthetic_object` — identical shape, exposed
    /// across the module boundary. `extra_bytes` is in bytes, not fields.
    pub(crate) unsafe fn synthetic_object_for_tests(
        descriptor: *const c_void,
        extra_bytes: usize,
    ) -> *mut c_void {
        let size = HEADER_BYTES + extra_bytes;
        let align = allocation_align(std::mem::align_of::<usize>());
        let layout = Layout::from_size_align(size, align).unwrap();
        let object = unsafe { alloc_zeroed(layout) } as *mut c_void;
        unsafe { *(object as *mut *mut c_void) = descriptor as *mut c_void };
        unsafe { register(object, size) };
        object
    }

    /// See `mod tests`' own `descriptor_with_fields`.
    pub(crate) fn descriptor_with_fields_for_tests(field_offsets: &[usize]) -> Vec<usize> {
        let mut words = vec![0usize, 0, 0, field_offsets.len()];
        words.extend_from_slice(field_offsets);
        // Runtime object descriptors now append declared-field metadata
        // after the collector reference paths. These synthetic descriptors
        // have no language fields, so terminate them with a zero field count.
        words.push(0);
        words
    }

    /// Sets this thread's own collection threshold (bytes) — the same
    /// mechanism `ZIRK_GC_THRESHOLD` configures for a real compiled
    /// program, set directly here so a test can force a collection without
    /// depending on process environment variables.
    pub(crate) fn set_threshold_for_tests(bytes: usize) {
        set_threshold_bytes(bytes);
    }

    pub(crate) fn collect_for_tests() {
        collect();
    }

    pub(crate) fn is_live_for_tests(object: *mut c_void) -> bool {
        with_heap(|heap| heap.allocations.contains(&(object as usize)))
    }

    /// Resets every piece of this thread's own collector state to a fresh
    /// start — everything `mod tests`' own `reset_state` resets except the
    /// process-global WeakCell flag/guard, which `crate::clone`'s own tests
    /// never touch (they hold no descriptor `crate::collector::mark_object`
    /// would ever recognize as a WeakCell sentinel).
    pub(crate) fn reset_state_for_tests() {
        BOOTSTRAP_FRAMES.with(|f| f.borrow_mut().clear());
        ACTIVE_FRAMES.set(std::ptr::null_mut());
        global_roots()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clear();
        crate::object::reset_class_properties_for_tests();
        heap_hard_reset();
        set_threshold_bytes(default_threshold());
    }
}
#[cfg(test)]
pub(crate) use test_support::{
    collect_for_tests, descriptor_with_fields_for_tests, is_live_for_tests, reset_state_for_tests,
    set_threshold_for_tests, synthetic_object_for_tests,
};

#[cfg(test)]
mod tests {
    use super::*;
    use std::alloc::alloc_zeroed;

    #[test]
    fn invalid_gc_candidate_is_rejected_before_header_access() {
        let _guard = test_support::test_guard();
        reset_state_for_tests();
        let mut invalid = 0x1234usize as *mut c_void;
        let mut roots = [&mut invalid as *mut *mut c_void as *mut c_void];
        unsafe { zirk_rt_push_frame(roots.as_mut_ptr(), 1) };
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(collect));
        zirk_rt_pop_frame();
        assert!(
            result.is_ok(),
            "invalid candidates must be skipped without dereferencing them"
        );
        reset_state_for_tests();
    }

    #[test]
    fn stale_gc_candidate_is_rejected_after_its_allocation_is_swept() {
        let _guard = test_support::test_guard();
        reset_state_for_tests();
        let descriptor = descriptor_with_fields(&[]);
        let stale = unsafe { synthetic_object(descriptor.as_ptr().cast(), 0) };
        collect();
        assert!(!test_support::is_live_for_tests(stale));
        let mut candidate = stale;
        let mut roots = [&mut candidate as *mut *mut c_void as *mut c_void];
        unsafe { zirk_rt_push_frame(roots.as_mut_ptr(), 1) };
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(collect));
        zirk_rt_pop_frame();
        assert!(
            result.is_ok(),
            "stale candidates must be skipped without dereferencing them"
        );
        reset_state_for_tests();
    }

    #[test]
    fn explicit_global_root_retains_a_managed_value_until_removed() {
        let _guard = test_support::test_guard();
        reset_state_for_tests();
        let descriptor = descriptor_with_fields(&[]);
        let object = unsafe { synthetic_object(descriptor.as_ptr().cast(), 0) };
        let handle = unsafe { zirk_rt_global_root_add(object) };
        assert_ne!(handle, 0);

        collect();
        assert!(test_support::is_live_for_tests(object));
        assert!(zirk_rt_global_root_remove(handle));
        assert!(!zirk_rt_global_root_remove(handle));
        collect();
        assert!(!test_support::is_live_for_tests(object));
        reset_state_for_tests();
    }

    #[test]
    fn iterative_marking_handles_a_deep_reference_chain() {
        let _guard = test_support::test_guard();
        reset_state_for_tests();
        const DEPTH: usize = 25_000;
        let descriptor = descriptor_with_fields(&[HEADER_BYTES]);
        let mut objects = Vec::with_capacity(DEPTH);
        for _ in 0..DEPTH {
            objects.push(unsafe {
                synthetic_object(descriptor.as_ptr().cast(), std::mem::size_of::<usize>())
            });
        }
        for pair in objects.windows(2) {
            unsafe { *((pair[0] as *mut u8).add(HEADER_BYTES) as *mut *mut c_void) = pair[1] };
        }
        let root = objects[0];
        let mut root_slot = root;
        let mut roots = [&mut root_slot as *mut *mut c_void as *mut c_void];
        unsafe { zirk_rt_push_frame(roots.as_mut_ptr(), 1) };
        collect();
        assert!(
            objects
                .iter()
                .all(|&object| test_support::is_live_for_tests(object))
        );
        zirk_rt_pop_frame();
        collect();
        assert!(
            objects
                .iter()
                .all(|&object| !test_support::is_live_for_tests(object))
        );
        reset_state_for_tests();
    }

    #[test]
    fn adversarial_mixed_managed_graph_survives_churn_and_sweeps_unreachable_graphs() {
        let _guard = reset_state();
        const WORD: usize = std::mem::size_of::<usize>();
        // Avoid an incidental collection while assembling deliberately
        // unrooted test objects. Once the shadow frame is installed, explicit
        // collections exercise precisely the graph under test.
        test_support::set_threshold_for_tests(usize::MAX / 2);

        let key = crate::string::alloc_owned("mixed-map-key");
        let mapped = crate::string::alloc_owned("mixed-map-value");
        let set_value = crate::string::alloc_owned("mixed-set-value");
        let array_value = crate::string::alloc_owned("mixed-array-value");
        let list_value = crate::string::alloc_owned("mixed-list-value");
        let callable_descriptor = descriptor_with_fields(&[HEADER_BYTES]);
        let callable = unsafe {
            crate::zirk_rt_alloc_callable(callable_descriptor.as_ptr().cast(), WORD as u64)
        };
        unsafe {
            *((callable as *mut u8).add(HEADER_BYTES) as *mut usize) = array_value as usize;
        }
        // The cached trace is reachable only through the exception side table
        // while this owner remains reachable through the Set.
        let exception =
            unsafe { synthetic_object(crate::array::zirk_rt_array_descriptor.as_ptr().cast(), 0) };
        let exception_trace = unsafe { crate::exceptions::zirk_rt_stack_trace(exception) };
        let offsets = [0usize];
        let map = unsafe {
            crate::map::zirk_rt_map_new_with_layout(offsets.as_ptr(), 1, offsets.as_ptr(), 1, false)
        };
        crate::map::zirk_rt_map_set(map, key as i64, mapped as i64);
        let set = unsafe { crate::set::zirk_rt_set_new_with_layout(offsets.as_ptr(), 1, false) };
        crate::set::zirk_rt_set_add(set, set_value as i64);

        // A two-word element models a flattened aggregate/enum payload whose
        // second field is a managed reference. Arrays and lists must expand
        // that relative offset for every stored element.
        let element_size = 2 * WORD;
        let flat_offsets = [WORD];
        let array = unsafe {
            crate::array::zirk_rt_array_new_with_layout(
                2,
                element_size,
                WORD,
                flat_offsets.as_ptr(),
                flat_offsets.len(),
            )
        };
        let list = unsafe {
            crate::list::zirk_rt_list_new_with_layout(
                element_size,
                WORD,
                flat_offsets.as_ptr(),
                flat_offsets.len(),
            )
        };
        for (index, value) in [callable, map].into_iter().enumerate() {
            let slot =
                unsafe { crate::array::zirk_rt_array_element(array, index as i64, element_size) };
            unsafe {
                (slot as *mut usize).write(index + 100);
                (slot as *mut usize).add(1).write(value as usize);
                crate::list::zirk_rt_list_add(list, slot, element_size, WORD, false);
            }
        }

        // Enum-like flattened payload: descriptor offsets point to the
        // reference-bearing variant field after its scalar tag.
        let enum_descriptor = descriptor_with_fields(&[HEADER_BYTES + WORD]);
        let enum_value = unsafe { synthetic_object(enum_descriptor.as_ptr().cast(), 2 * WORD) };
        unsafe {
            *((enum_value as *mut u8).add(HEADER_BYTES) as *mut usize) = 1;
            *((enum_value as *mut u8).add(HEADER_BYTES + WORD) as *mut usize) = list_value as usize;
        }
        // Close a real cross-container cycle through the flattened payload:
        // Map -> Array -> Map. The Map also reaches the enum-like object,
        // which reaches a String; Set and List add independent edge layouts.
        crate::map::zirk_rt_map_set(map, enum_value as i64, array as i64);
        crate::set::zirk_rt_set_add(set, enum_value as i64);
        crate::set::zirk_rt_set_add(set, exception as i64);

        let mut roots_values = [map, set, list];
        let mut roots: Vec<*mut c_void> = roots_values
            .iter_mut()
            .map(|root| (root as *mut *mut c_void).cast())
            .collect();
        unsafe { zirk_rt_push_frame(roots.as_mut_ptr(), roots.len() as i64) };

        for round in 0..8 {
            test_support::collect_for_tests();
            for object in [
                key,
                mapped,
                set_value,
                array_value,
                callable,
                list_value,
                exception,
                exception_trace,
                map,
                set,
                array,
                list,
                enum_value,
            ] {
                assert!(
                    test_support::is_live_for_tests(object),
                    "reachable allocation {object:p} was reclaimed on collection {round}"
                );
            }
            // Allocation churn between collections pressures the threshold
            // and ensures dead objects do not accumulate as false survivors.
            let mut dead = Vec::with_capacity(64);
            for i in 0..64 {
                dead.push(crate::string::alloc_owned(&format!("churn-{round}-{i}")));
            }
            test_support::collect_for_tests();
            assert!(
                dead.iter()
                    .all(|object| !test_support::is_live_for_tests(*object)),
                "unreachable churn allocations survived collection {round}"
            );
        }

        // Build a second, disconnected mixed subgraph while the reachable
        // graph remains rooted, then prove every node in it is swept.
        let dead_text = crate::string::alloc_owned("unreachable-leaf");
        let dead_array = unsafe {
            crate::array::zirk_rt_array_new_with_layout(
                1,
                element_size,
                WORD,
                flat_offsets.as_ptr(),
                flat_offsets.len(),
            )
        };
        let dead_slot = unsafe { crate::array::zirk_rt_array_element(dead_array, 0, element_size) };
        unsafe {
            (dead_slot as *mut usize).write(0);
            (dead_slot as *mut usize).add(1).write(dead_text as usize);
        }
        let dead_list = unsafe {
            crate::list::zirk_rt_list_new_with_layout(
                element_size,
                WORD,
                flat_offsets.as_ptr(),
                flat_offsets.len(),
            )
        };
        unsafe {
            crate::list::zirk_rt_list_add(dead_list, dead_slot, element_size, WORD, false);
        }
        let dead_map = crate::map::zirk_rt_map_new();
        crate::map::zirk_rt_map_set(dead_map, dead_array as i64, dead_list as i64);
        let dead_set = crate::set::zirk_rt_set_new();
        crate::set::zirk_rt_set_add(dead_set, dead_map as i64);
        let dead_exception =
            unsafe { synthetic_object(crate::array::zirk_rt_array_descriptor.as_ptr().cast(), 0) };
        let dead_exception_trace =
            unsafe { crate::exceptions::zirk_rt_stack_trace(dead_exception) };
        let dead_enum = unsafe { synthetic_object(enum_descriptor.as_ptr().cast(), 2 * WORD) };
        unsafe {
            *((dead_enum as *mut u8).add(HEADER_BYTES + WORD) as *mut usize) = dead_set as usize;
        }
        test_support::collect_for_tests();
        for object in [
            dead_text,
            dead_array,
            dead_list,
            dead_map,
            dead_set,
            dead_enum,
            dead_exception,
            dead_exception_trace,
        ] {
            assert!(
                !test_support::is_live_for_tests(object),
                "unreachable node {object:p} survived"
            );
        }
        for object in [
            key,
            mapped,
            set_value,
            array_value,
            callable,
            list_value,
            exception,
            exception_trace,
            map,
            set,
            array,
            list,
            enum_value,
        ] {
            assert!(
                test_support::is_live_for_tests(object),
                "rooted graph lost {object:p}"
            );
        }

        zirk_rt_pop_frame();
        test_support::collect_for_tests();
        assert!(
            [
                map,
                set,
                array,
                list,
                enum_value,
                key,
                mapped,
                set_value,
                array_value,
                list_value,
                callable,
                exception,
                exception_trace
            ]
            .iter()
            .all(|object| !test_support::is_live_for_tests(*object)),
            "the mixed graph should be reclaimable after removing its roots"
        );
    }

    /// Builds a synthetic object with a caller-chosen descriptor and no
    /// fields beyond the header — enough to exercise the collector without
    /// going through codegen at all.
    unsafe fn synthetic_object(descriptor: *const c_void, extra_bytes: usize) -> *mut c_void {
        let size = HEADER_BYTES + extra_bytes;
        let align = allocation_align(std::mem::align_of::<usize>());
        let layout = Layout::from_size_align(size, align).unwrap();
        let object = unsafe { alloc_zeroed(layout) } as *mut c_void;
        unsafe { *(object as *mut *mut c_void) = descriptor as *mut c_void };
        unsafe { register(object, size) };
        object
    }

    /// A descriptor whose only content this test cares about is the
    /// GC-field-offset table: `[ 0 (method table) | 0 (ancestors) | 0
    /// (contracts) | field_count | field_offset* ]` — the exact shape
    /// `gc_field_offsets` reads, with every earlier section empty so
    /// nothing but the collector ever looks at it.
    fn descriptor_with_fields(field_offsets: &[usize]) -> Vec<usize> {
        let mut words = vec![0usize, 0, 0, field_offsets.len()];
        words.extend_from_slice(field_offsets);
        words
    }

    thread_local! {
        /// How many times [`clear_dead_weak_cells`] actually ran its walk
        /// (not skipped by the "ever allocated" check) — `fase-4e-weak`
        /// task 4.3's own instrumentation hook, confirming the skip is real
        /// rather than assumed.
        static CLEAR_DEAD_WEAK_CELLS_RUNS: Cell<usize> = const { Cell::new(0) };
    }

    pub(super) fn note_clear_dead_weak_cells_ran() {
        CLEAR_DEAD_WEAK_CELLS_RUNS.with(|c| c.set(c.get() + 1));
    }

    /// Serializes every collector test (`fase-4e-weak`): unlike every other
    /// piece of state this module resets between tests, the WeakCell "ever
    /// allocated" flag ([`zirk_rt_weak_cell_ever_allocated`]) is a real
    /// process-global, not `thread_local!` — correctly so, since it must
    /// reflect one running *program's* history, not one test worker
    /// thread's. Libtest runs `#[test]`s across several worker threads by
    /// default, so without this, one thread's [`reset_state`] clearing the
    /// flag to `0` could race a different thread's in-flight `collect()`
    /// that still needed it `true`. [`reset_state`]'s returned guard must be
    /// held for the whole test body, not just its own call.
    #[must_use = "the guard must stay bound for the whole test body, not just this call"]
    fn reset_state() -> std::sync::MutexGuard<'static, ()> {
        let guard = test_support::test_guard();
        BOOTSTRAP_FRAMES.with(|f| f.borrow_mut().clear());
        ACTIVE_FRAMES.set(std::ptr::null_mut());
        // Frees anything a previous test left behind (the heap list is
        // process-global now — `TEST_GUARD` serializes collector tests so this
        // is safe) and zeroes the totals.
        heap_hard_reset();
        unsafe { zirk_rt_weak_cell_ever_allocated = 0 };
        CLEAR_DEAD_WEAK_CELLS_RUNS.with(|c| c.set(0));
        guard
    }

    #[test]
    fn an_unreachable_cycle_is_fully_reclaimed() {
        let _guard = reset_state();
        // Two objects, each holding the other at header-relative offset 24
        // (right after the 3-word header) — a cycle with no root pointing
        // into it at all.
        let descriptor = descriptor_with_fields(&[24]);
        unsafe {
            let a = synthetic_object(descriptor.as_ptr() as *const c_void, 8);
            let b = synthetic_object(descriptor.as_ptr() as *const c_void, 8);
            *((a as usize + 24) as *mut *mut c_void) = b;
            *((b as usize + 24) as *mut *mut c_void) = a;
        }

        assert!(live_bytes_total() > 0);
        collect();
        assert_eq!(
            live_bytes_total(),
            0,
            "a cycle with no root must be fully collected"
        );
        assert!(all_objects_head().is_null());
    }

    #[test]
    fn an_object_reachable_only_through_a_pushed_frame_survives() {
        let _guard = reset_state();
        let descriptor = descriptor_with_fields(&[]);
        let object = unsafe { synthetic_object(descriptor.as_ptr() as *const c_void, 0) };

        let mut root_slot: *mut c_void = object;
        let mut roots: [*mut c_void; 1] = [(&mut root_slot as *mut *mut c_void) as *mut c_void];

        unsafe { zirk_rt_push_frame(roots.as_mut_ptr(), 1) };
        collect();
        assert_eq!(
            live_bytes_total(),
            unsafe { get_size(object) },
            "a rooted object must survive collection"
        );
        zirk_rt_pop_frame();

        collect();
        assert_eq!(
            live_bytes_total(),
            0,
            "once its frame pops, an unrooted object is reclaimed on the next collection"
        );
    }

    #[test]
    fn a_suspended_tasks_roots_survive_a_collection_from_another_task() {
        // The highest-severity case for `fase-5-executor-core` group 7: a
        // reference held only by a *suspended* task, in a frame pushed while it
        // ran, must not be swept by a collection another task triggers.
        let _guard = reset_state();
        use crate::executor;
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};

        let checked = Arc::new(AtomicUsize::new(0));
        let done = Arc::clone(&checked);

        let object_size = executor::Executor::new().run_with_root(move || {
            // A sibling that forces a collection while the root is parked.
            executor::spawn(|| {
                collect();
                0
            });

            let descriptor = descriptor_with_fields(&[]);
            let object = unsafe { synthetic_object(descriptor.as_ptr() as *const c_void, 0) };
            let size = unsafe { get_size(object) };

            // Root the object through a pushed frame — routed to *this task's*
            // chain by the executor.
            let mut root_slot: *mut c_void = object;
            let mut roots: [*mut c_void; 1] = [(&mut root_slot as *mut *mut c_void) as *mut c_void];
            unsafe { zirk_rt_push_frame(roots.as_mut_ptr(), 1) };

            executor::yield_now(); // the sibling runs `collect()` here

            assert_eq!(
                live_bytes_total(),
                size,
                "the suspended task's rooted object was swept"
            );
            // Header word 0 (the descriptor) is intact -> not overwritten by a
            // reuse of freed memory.
            assert_eq!(
                unsafe { *(root_slot as *const *const c_void) },
                descriptor.as_ptr() as *const c_void,
                "the object header was clobbered"
            );
            zirk_rt_pop_frame();
            done.store(1, Ordering::SeqCst);
            size
        });

        let object_size = match object_size {
            crate::task::TaskOutcome::Value(v) => v,
            crate::task::TaskOutcome::Panicked(p) => std::panic::resume_unwind(p),
        };
        assert!(object_size > 0);
        assert_eq!(checked.load(Ordering::SeqCst), 1);

        // Frame popped, object now unrooted: the next collection reclaims it.
        collect();
        assert_eq!(live_bytes_total(), 0);
    }

    #[test]
    fn a_field_reached_transitively_through_a_root_survives() {
        let _guard = reset_state();
        let leaf_descriptor = descriptor_with_fields(&[]);
        let parent_descriptor = descriptor_with_fields(&[24]);

        unsafe {
            let leaf = synthetic_object(leaf_descriptor.as_ptr() as *const c_void, 0);
            let parent = synthetic_object(parent_descriptor.as_ptr() as *const c_void, 8);
            *((parent as usize + 24) as *mut *mut c_void) = leaf;

            let mut root_slot: *mut c_void = parent;
            let mut roots: [*mut c_void; 1] = [(&mut root_slot as *mut *mut c_void) as *mut c_void];
            zirk_rt_push_frame(roots.as_mut_ptr(), 1);

            collect();
            assert_eq!(
                live_bytes_total(),
                get_size(parent) + get_size(leaf),
                "a field reached transitively through a root must survive too"
            );
            zirk_rt_pop_frame();
        }
    }

    #[test]
    fn the_threshold_trigger_fires_at_the_configured_point_and_not_before() {
        let _guard = reset_state();
        set_threshold_bytes(16);

        // Below the threshold: no collection, nothing to reclaim yet since
        // nothing is unreachable.
        maybe_collect(8);
        assert!(all_objects_head().is_null());

        let descriptor = descriptor_with_fields(&[]);
        unsafe { synthetic_object(descriptor.as_ptr() as *const c_void, 0) };
        let live_before = live_bytes_total();
        assert!(live_before > 0);

        // An allocation request that would cross the threshold triggers a
        // collection first — the unrooted object above is unreachable, so
        // it is reclaimed before the (never actually performed, this test
        // only checks the trigger) allocation would be served.
        maybe_collect(1024);
        assert_eq!(
            live_bytes_total(),
            0,
            "crossing the threshold must trigger a collection"
        );

        set_threshold_bytes(default_threshold());
    }

    // --- Weak<T> (`fase-4e-weak`) -------------------------------------------

    /// Builds a synthetic WeakCell: the sentinel descriptor, one extra
    /// field's worth of storage, its target field set to `target` — the
    /// exact shape `InstKind::WeakFrom`'s own lowering builds, without going
    /// through codegen at all. Sets [`zirk_rt_weak_cell_ever_allocated`]
    /// itself, the same thing real codegen does right after allocating —
    /// callers of this helper never need to set it separately.
    unsafe fn synthetic_weak_cell(target: *mut c_void) -> *mut c_void {
        unsafe {
            let cell = synthetic_object(weak_cell_descriptor() as *const c_void, 8);
            set_weak_target(cell, target);
            zirk_rt_weak_cell_ever_allocated = 1;
            cell
        }
    }

    #[test]
    fn a_weak_cells_target_that_survives_collection_keeps_being_observed() {
        let _guard = reset_state();
        let leaf_descriptor = descriptor_with_fields(&[]);
        unsafe {
            let leaf = synthetic_object(leaf_descriptor.as_ptr() as *const c_void, 0);
            let cell = synthetic_weak_cell(leaf);

            let mut leaf_slot: *mut c_void = leaf;
            let mut cell_slot: *mut c_void = cell;
            let mut roots: [*mut c_void; 2] = [
                (&mut leaf_slot as *mut *mut c_void) as *mut c_void,
                (&mut cell_slot as *mut *mut c_void) as *mut c_void,
            ];
            zirk_rt_push_frame(roots.as_mut_ptr(), 2);

            collect();
            assert_eq!(
                get_weak_target(cell),
                leaf,
                "a referent kept alive by its own root must not have its weak target cleared"
            );
            zirk_rt_pop_frame();
        }
    }

    #[test]
    fn a_weak_cells_target_that_is_collected_has_its_field_nulled_not_dangling() {
        let _guard = reset_state();
        let leaf_descriptor = descriptor_with_fields(&[]);
        unsafe {
            // `leaf` has no root of its own — the WeakCell is the only
            // thing that ever points at it, and a WeakCell's own field is
            // never a strong edge (design D2), so `leaf` must not survive.
            let leaf = synthetic_object(leaf_descriptor.as_ptr() as *const c_void, 0);
            let cell = synthetic_weak_cell(leaf);

            let mut cell_slot: *mut c_void = cell;
            let mut roots: [*mut c_void; 1] = [(&mut cell_slot as *mut *mut c_void) as *mut c_void];
            zirk_rt_push_frame(roots.as_mut_ptr(), 1);

            collect();
            assert!(
                get_weak_target(cell).is_null(),
                "an unreachable referent's weak target must be cleared before its storage is \
                 reclaimed, never left dangling"
            );
            assert_eq!(
                live_bytes_total(),
                get_size(cell),
                "only the WeakCell itself should remain live; its collected referent's bytes \
                 must be gone from the live total"
            );
            zirk_rt_pop_frame();
        }
    }

    #[test]
    fn an_unreachable_weak_cell_is_collected_like_any_other_object() {
        let _guard = reset_state();
        unsafe {
            let _cell = synthetic_weak_cell(std::ptr::null_mut());

            assert!(live_bytes_total() > 0);
            collect();
            assert_eq!(
                live_bytes_total(),
                0,
                "an unrooted WeakCell is an ordinary allocation once nothing reaches it"
            );
            assert!(all_objects_head().is_null());
        }
    }

    #[test]
    fn a_weak_cells_target_field_is_never_traced_as_a_strong_root() {
        let _guard = reset_state();
        let leaf_descriptor = descriptor_with_fields(&[]);
        unsafe {
            // The *only* path to `leaf` is through the WeakCell's own
            // field — no other root reaches it at all.
            let leaf = synthetic_object(leaf_descriptor.as_ptr() as *const c_void, 0);
            let cell = synthetic_weak_cell(leaf);

            let mut cell_slot: *mut c_void = cell;
            let mut roots: [*mut c_void; 1] = [(&mut cell_slot as *mut *mut c_void) as *mut c_void];
            zirk_rt_push_frame(roots.as_mut_ptr(), 1);

            collect();
            assert!(
                get_weak_target(cell).is_null(),
                "a WeakCell must not keep its referent alive merely by pointing at it"
            );
            zirk_rt_pop_frame();
        }
    }

    #[test]
    fn clear_dead_weak_cells_runs_only_once_a_weak_cell_has_been_allocated() {
        let _guard = reset_state();
        // An ordinary object graph — no `Weak<T>` anywhere.
        let descriptor = descriptor_with_fields(&[]);
        unsafe { synthetic_object(descriptor.as_ptr() as *const c_void, 0) };

        collect();
        assert_eq!(
            CLEAR_DEAD_WEAK_CELLS_RUNS.with(Cell::get),
            0,
            "the weak-clearing pass must be skipped entirely when no Weak<T> was ever allocated"
        );

        unsafe { zirk_rt_weak_cell_ever_allocated = 1 };
        collect();
        assert_eq!(
            CLEAR_DEAD_WEAK_CELLS_RUNS.with(Cell::get),
            1,
            "the weak-clearing pass must run once a Weak<T> has been allocated"
        );
    }
}
