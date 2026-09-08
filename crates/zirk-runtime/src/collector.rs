//! Mark-sweep garbage collector (roadmap Phase 4e, `fase-4e-colector-mark-sweep`).
//!
//! `docs/decisions/ADR-003-memoria.md`'s "Cierre de la decisión" fixed the
//! strategy this module implements: non-moving mark-sweep, root enumeration
//! through a function-granularity shadow stack built by codegen
//! (`crates/zirk-codegen-llvm/src/emit.rs`), collection triggered
//! cooperatively inside [`crate::memory::zirk_rt_alloc`] itself. No threads
//! exist yet (Phase 5), so "stop the world" needs nothing special: there is
//! nothing else running to stop.
//!
//! # Object header
//!
//! Every object `zirk_rt_alloc` returns starts with three pointer-sized
//! words (design D1 — grown from the one word codegen has always written,
//! the dispatch descriptor):
//!
//! ```text
//!    [ descriptor | next (mark bit in the low bit) | size | field₁ | … ]
//! ```
//!
//! `descriptor` is written by codegen (`FunctionEmitter`'s `Alloc` case),
//! exactly as before this change. `next` and `size` are this module's own —
//! nothing outside the collector reads or writes them. `next` threads every
//! live allocation onto one intrusive singly-linked list, walked by sweep;
//! its low bit doubles as the mark bit, which costs nothing extra since
//! every allocation this collector hands out is aligned to at least
//! [`GC_ALIGN`], so that bit is otherwise always zero.
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
use std::ffi::c_void;

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
/// The mark bit hidden in `next`'s low bit — free because every allocation
/// is [`GC_ALIGN`]-aligned, so a real `next` pointer's low bit is always
/// zero on its own.
const MARK_BIT: usize = 1;

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
/// The payload is a `ZirkString` (private to `crates/zirk-runtime/src/string.rs`);
/// `gc_field_count` is zero because the bytes following the header are owned
/// data, not GC references. This lets the collector reclaim string/char
/// objects exactly as it does ordinary class objects.
#[unsafe(no_mangle)]
#[allow(non_upper_case_globals)]
pub static zirk_rt_string_descriptor: [usize; 4] = [0, 0, 0, 0];

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

pub(crate) struct Frame {
    /// Address of an array of root addresses (design D2): element `i` is
    /// the address of a reference-typed slot (or one of its own
    /// reference-typed fields) — not the reference's value, which is why
    /// [`mark`] dereferences each entry once more to reach the candidate
    /// object.
    pub(crate) roots: *mut *mut c_void,
    pub(crate) count: usize,
}

thread_local! {
    /// The pushed-frame stack (design D2), one entry per live function
    /// activation that declared at least one collector-managed local — a
    /// plain `Vec` rather than an intrusive list threaded through the
    /// native call stack: `push_frame`/`pop_frame` already happen in
    /// perfect call/return (LIFO) order, so a `Vec` gives the identical
    /// stack discipline design D2 asks for with far less unsafe plumbing.
    static FRAMES: RefCell<Vec<Frame>> = const { RefCell::new(Vec::new()) };

    /// Head of the intrusive all-allocations list `next` threads through
    /// (design D1/D6) — every object ever handed out by
    /// [`crate::memory::zirk_rt_alloc`] and not yet swept.
    static ALL_OBJECTS: Cell<*mut c_void> = const { Cell::new(std::ptr::null_mut()) };

    /// Sum of every live allocation's own `size` — what [`maybe_collect`]
    /// compares against [`THRESHOLD`] before serving a new allocation
    /// (design D3).
    static LIVE_BYTES: Cell<usize> = const { Cell::new(0) };

    /// Bytes of live allocation allowed before a collection runs (design
    /// D3). Configurable through `ZIRK_GC_THRESHOLD` (bytes) so a test can
    /// force frequent collections without needing a program that actually
    /// allocates a default-sized threshold's worth of memory — read once,
    /// lazily, the first time it is needed.
    static THRESHOLD: Cell<usize> = Cell::new(default_threshold());
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
unsafe fn get_next_raw(object: *mut c_void) -> usize {
    unsafe { *header_word(object, NEXT_WORD) }
}

#[inline]
unsafe fn set_next_raw(object: *mut c_void, value: usize) {
    unsafe { *header_word(object, NEXT_WORD) = value };
}

#[inline]
unsafe fn get_size(object: *mut c_void) -> usize {
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

#[inline]
unsafe fn set_size(object: *mut c_void, value: usize) {
    unsafe { *header_word(object, SIZE_WORD) = value };
}

#[inline]
fn is_marked(next_raw: usize) -> bool {
    next_raw & MARK_BIT != 0
}

#[inline]
fn next_ptr(next_raw: usize) -> *mut c_void {
    (next_raw & !MARK_BIT) as *mut c_void
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
    FRAMES.with(|frames| {
        frames.borrow_mut().push(Frame {
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
    FRAMES.with(|frames| {
        frames.borrow_mut().pop();
    });
}

/// Runs a collection if serving an allocation of `incoming` bytes would
/// cross the configured threshold (design D3) — called by
/// [`crate::memory::zirk_rt_alloc`] before it allocates anything.
pub(crate) fn maybe_collect(incoming: usize) {
    let live = LIVE_BYTES.with(Cell::get);
    let threshold = THRESHOLD.with(Cell::get);
    if live + incoming > threshold {
        collect();
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
    let head = ALL_OBJECTS.with(Cell::get);
    unsafe {
        // `head` is itself either null or `GC_ALIGN`-aligned, so its low
        // bit is already the clear mark bit this fresh object needs.
        set_next_raw(object, head as usize);
        set_size(object, size);
    }
    ALL_OBJECTS.with(|cell| cell.set(object));
    LIVE_BYTES.with(|cell| cell.set(cell.get() + size));
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

fn collect() {
    mark();
    clear_dead_weak_cells();
    sweep();
}

fn mark() {
    FRAMES.with(|frames| {
        for frame in frames.borrow().iter() {
            for index in 0..frame.count {
                let root_addr = unsafe { *frame.roots.add(index) };
                if root_addr.is_null() {
                    continue;
                }
                let object = unsafe { *(root_addr as *mut *mut c_void) };
                mark_object(object);
            }
        }
    });

    // `zirk-object-memory`'s own "Deep-clone traversal state is
    // collector-safe for its whole duration" requirement (roadmap Phase
    // 4e, `fase-4e-clone`, design D3): every destination object a
    // still-in-progress `clone()` call has produced so far is not yet
    // reachable from any ordinary shadow-stack root (only the *finished*
    // top-level result is, once its own instruction result is spilled) —
    // `crate::clone` tracks them itself and this is its own extra root
    // set, walked exactly like the shadow stack above.
    crate::clone::mark_clone_roots(mark_object);
}

fn mark_object(object: *mut c_void) {
    if object.is_null() {
        return;
    }
    let next_raw = unsafe { get_next_raw(object) };
    if is_marked(next_raw) {
        return;
    }
    unsafe { set_next_raw(object, next_raw | MARK_BIT) };

    let descriptor = unsafe { *(object as *mut *mut c_void) };

    // A WeakCell is marked as reachable exactly like any other object right
    // above — but its own single field is never traced as a strong edge
    // (`fase-4e-weak`, design D2): that is what lets [`clear_dead_weak_cells`]
    // observe, between this pass and [`sweep`], whether its target survived
    // on its own merits.
    if std::ptr::eq(descriptor, weak_cell_descriptor()) {
        return;
    }

    let offsets = unsafe { gc_field_offsets(descriptor as *const c_void) };
    for &offset in offsets {
        let field_addr = (object as usize + offset) as *mut *mut c_void;
        let child = unsafe { *field_addr };
        mark_object(child);
    }
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
fn clear_dead_weak_cells() {
    if !weak_cell_ever_allocated() {
        return;
    }

    #[cfg(test)]
    tests::note_clear_dead_weak_cells_ran();

    let mut current = ALL_OBJECTS.with(Cell::get);
    while !current.is_null() {
        let next_raw = unsafe { get_next_raw(current) };

        if is_marked(next_raw) {
            let descriptor = unsafe { *(current as *mut *mut c_void) };
            if descriptor == weak_cell_descriptor() {
                let target = unsafe { get_weak_target(current) };
                if !target.is_null() {
                    let target_next_raw = unsafe { get_next_raw(target) };
                    if !is_marked(target_next_raw) {
                        unsafe { set_weak_target(current, std::ptr::null_mut()) };
                    }
                }
            }
        }

        current = next_ptr(next_raw);
    }
}

fn sweep() {
    let mut current = ALL_OBJECTS.with(Cell::get);
    let mut survivors: *mut c_void = std::ptr::null_mut();
    let mut live_bytes = 0usize;

    while !current.is_null() {
        let next_raw = unsafe { get_next_raw(current) };
        let next_object = next_ptr(next_raw);

        if is_marked(next_raw) {
            // Relinks `current` onto the survivors list being built, in the
            // process clearing its mark bit for the next cycle: `survivors`
            // is itself always a plain, unmarked pointer.
            unsafe { set_next_raw(current, survivors as usize) };
            survivors = current;
            live_bytes += unsafe { get_size(current) };
        } else {
            let size = unsafe { get_size(current) };
            let layout = Layout::from_size_align(size, GC_ALIGN)
                .expect("this is the exact layout `zirk_rt_alloc` used to allocate it");
            unsafe { dealloc(current as *mut u8, layout) };
        }

        current = next_object;
    }

    ALL_OBJECTS.with(|cell| cell.set(survivors));
    LIVE_BYTES.with(|cell| cell.set(live_bytes));
}

/// Test-only support for `crate::clone`'s own synthetic-object tests
/// (`fase-4e-clone`) — thin `pub(crate)` wrappers around this module's own
/// private test helpers below, since a sibling module's `#[cfg(test)]` code
/// cannot reach into `mod tests`' private items directly.
#[cfg(test)]
pub(crate) mod test_support {
    use super::*;
    use std::alloc::alloc_zeroed;

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
        words
    }

    /// Sets this thread's own collection threshold (bytes) — the same
    /// mechanism `ZIRK_GC_THRESHOLD` configures for a real compiled
    /// program, set directly here so a test can force a collection without
    /// depending on process environment variables.
    pub(crate) fn set_threshold_for_tests(bytes: usize) {
        THRESHOLD.with(|c| c.set(bytes));
    }

    /// Resets every piece of this thread's own collector state to a fresh
    /// start — everything `mod tests`' own `reset_state` resets except the
    /// process-global WeakCell flag/guard, which `crate::clone`'s own tests
    /// never touch (they hold no descriptor `crate::collector::mark_object`
    /// would ever recognize as a WeakCell sentinel).
    pub(crate) fn reset_state_for_tests() {
        FRAMES.with(|f| f.borrow_mut().clear());
        sweep();
        ALL_OBJECTS.with(|c| c.set(std::ptr::null_mut()));
        LIVE_BYTES.with(|c| c.set(0));
        THRESHOLD.with(|c| c.set(default_threshold()));
    }
}
#[cfg(test)]
pub(crate) use test_support::{
    descriptor_with_fields_for_tests, reset_state_for_tests, set_threshold_for_tests,
    synthetic_object_for_tests,
};

#[cfg(test)]
mod tests {
    use super::*;
    use std::alloc::alloc_zeroed;

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
    static TEST_GUARD: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[must_use = "the guard must stay bound for the whole test body, not just this call"]
    fn reset_state() -> std::sync::MutexGuard<'static, ()> {
        let guard = TEST_GUARD
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        FRAMES.with(|f| f.borrow_mut().clear());
        // Sweeps away anything a previous test in this thread left behind,
        // without asserting on it — tests run single-threaded within one
        // process but share this thread's statics across `#[test]`s.
        sweep();
        ALL_OBJECTS.with(|c| c.set(std::ptr::null_mut()));
        LIVE_BYTES.with(|c| c.set(0));
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

        assert!(LIVE_BYTES.with(Cell::get) > 0);
        collect();
        assert_eq!(
            LIVE_BYTES.with(Cell::get),
            0,
            "a cycle with no root must be fully collected"
        );
        assert!(ALL_OBJECTS.with(Cell::get).is_null());
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
            LIVE_BYTES.with(Cell::get),
            unsafe { get_size(object) },
            "a rooted object must survive collection"
        );
        zirk_rt_pop_frame();

        collect();
        assert_eq!(
            LIVE_BYTES.with(Cell::get),
            0,
            "once its frame pops, an unrooted object is reclaimed on the next collection"
        );
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
                LIVE_BYTES.with(Cell::get),
                get_size(parent) + get_size(leaf),
                "a field reached transitively through a root must survive too"
            );
            zirk_rt_pop_frame();
        }
    }

    #[test]
    fn the_threshold_trigger_fires_at_the_configured_point_and_not_before() {
        let _guard = reset_state();
        THRESHOLD.with(|c| c.set(16));

        // Below the threshold: no collection, nothing to reclaim yet since
        // nothing is unreachable.
        maybe_collect(8);
        assert!(ALL_OBJECTS.with(Cell::get).is_null());

        let descriptor = descriptor_with_fields(&[]);
        unsafe { synthetic_object(descriptor.as_ptr() as *const c_void, 0) };
        let live_before = LIVE_BYTES.with(Cell::get);
        assert!(live_before > 0);

        // An allocation request that would cross the threshold triggers a
        // collection first — the unrooted object above is unreachable, so
        // it is reclaimed before the (never actually performed, this test
        // only checks the trigger) allocation would be served.
        maybe_collect(1024);
        assert_eq!(
            LIVE_BYTES.with(Cell::get),
            0,
            "crossing the threshold must trigger a collection"
        );

        THRESHOLD.with(|c| c.set(default_threshold()));
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
                LIVE_BYTES.with(Cell::get),
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

            assert!(LIVE_BYTES.with(Cell::get) > 0);
            collect();
            assert_eq!(
                LIVE_BYTES.with(Cell::get),
                0,
                "an unrooted WeakCell is an ordinary allocation once nothing reaches it"
            );
            assert!(ALL_OBJECTS.with(Cell::get).is_null());
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
