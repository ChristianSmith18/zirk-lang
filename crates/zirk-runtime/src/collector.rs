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

struct Frame {
    /// Address of an array of root addresses (design D2): element `i` is
    /// the address of a reference-typed slot (or one of its own
    /// reference-typed fields) — not the reference's value, which is why
    /// [`mark`] dereferences each entry once more to reach the candidate
    /// object.
    roots: *mut *mut c_void,
    count: usize,
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
unsafe fn gc_field_offsets<'a>(descriptor: *const c_void) -> &'a [usize] {
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
    let offsets = unsafe { gc_field_offsets(descriptor as *const c_void) };
    for &offset in offsets {
        let field_addr = (object as usize + offset) as *mut *mut c_void;
        let child = unsafe { *field_addr };
        mark_object(child);
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

    fn reset_state() {
        FRAMES.with(|f| f.borrow_mut().clear());
        // Sweeps away anything a previous test in this thread left behind,
        // without asserting on it — tests run single-threaded within one
        // process but share this thread's statics across `#[test]`s.
        sweep();
        ALL_OBJECTS.with(|c| c.set(std::ptr::null_mut()));
        LIVE_BYTES.with(|c| c.set(0));
    }

    #[test]
    fn an_unreachable_cycle_is_fully_reclaimed() {
        reset_state();
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
        reset_state();
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
        reset_state();
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
        reset_state();
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
}
