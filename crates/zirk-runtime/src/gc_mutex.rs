//! Internal storage for a GC-managed mutex object.
//!
//! The allocation contains the collector header, Rust-owned mutex metadata,
//! and the inline payload; no process-global handle table is needed. The C ABI
//! source `Mutex.with` lowering calls the synchronous callback ABI below.

use std::alloc::Layout;
use std::ffi::c_void;

use crate::collector::HEADER_BYTES;
use crate::sync_mutex::{CooperativeMutex, MutexError};

/// Unique sentinel recognized by the collector's tracing and sweep hooks.
static GC_MUTEX_DESCRIPTOR: u8 = 0;

#[inline]
pub(crate) fn descriptor() -> *mut c_void {
    (&raw const GC_MUTEX_DESCRIPTOR) as *mut c_void
}

/// Rust-owned metadata placed between the GC header and aligned payload.
///
/// `reference_offsets` are byte offsets within the payload, not the complete
/// object. The collector adds the computed payload offset while tracing.
struct RootFrameGuard;

impl Drop for RootFrameGuard {
    fn drop(&mut self) {
        crate::collector::zirk_rt_pop_frame();
    }
}

struct GcMutexBody {
    #[allow(dead_code)] // used by the internal acquire/release runtime tests
    lock: CooperativeMutex<()>,
    reference_offsets: Vec<usize>,
    drop_payload: unsafe fn(*mut u8),
    payload_offset: usize,
    payload_size: usize,
}

const STATUS_OK: u32 = 0;
const STATUS_INVALID_ARGUMENT: u32 = 1;
const STATUS_INVALID_HANDLE: u32 = 2;
const STATUS_CANCELLED: u32 = 3;
const STATUS_INTERNAL_PANIC: u32 = 4;
const MAX_PAYLOAD_BYTES: usize = 16 * 1024 * 1024;
const MAX_REFERENCE_OFFSETS: usize = 4096;

type MutexCallback = unsafe extern "C-unwind" fn(*mut c_void, *mut c_void);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GcMutexError {
    InvalidHandle,
    Lock(MutexError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AllocateError {
    InvalidArgument,
    Allocation,
}

#[inline]
unsafe fn body(object: *mut c_void) -> *mut GcMutexBody {
    // The body begins immediately after the fixed three-word collector header.
    unsafe { (object as *mut u8).add(HEADER_BYTES).cast::<GcMutexBody>() }
}

struct LockGuard(*const CooperativeMutex<()>);

impl Drop for LockGuard {
    fn drop(&mut self) {
        // SAFETY: the handle remains rooted by the generated caller for the
        // entire synchronous callback, and acquire established ownership on
        // this executor task/registered worker.
        if unsafe { &*self.0 }.release().is_err() {
            crate::failure::fatal("internal error: GC mutex callback lost lock ownership");
        }
    }
}

/// Allocates an internal GC mutex object and moves `payload` into the same
/// collector allocation. Each reference offset must point to an initialized
/// managed-reference slot in `T`.
///
/// The payload references are temporarily registered as roots across the GC
/// allocation call, which may collect before returning the new object.
///
/// # Panics
///
/// Panics if the payload alignment exceeds the collector's fixed alignment or
/// if a reference offset does not identify a complete pointer-sized field.
#[allow(dead_code)] // intentionally internal until source/ABI integration
pub(crate) fn allocate<T>(mut payload: T, reference_offsets: &[usize]) -> *mut c_void {
    let payload_layout = Layout::new::<T>();
    let pointer_layout = Layout::new::<*mut c_void>();
    assert!(
        payload_layout.align() <= crate::collector::allocation_align(payload_layout.align()),
        "GC mutex payload alignment exceeds the collector alignment"
    );
    for &offset in reference_offsets {
        assert!(
            offset
                .checked_add(pointer_layout.size())
                .is_some_and(|end| end <= payload_layout.size())
                && offset.is_multiple_of(pointer_layout.align()),
            "GC mutex reference offset is outside or misaligned within its payload"
        );
    }

    // Clone metadata before entering the collector: if this allocation fails,
    // no partially initialized GC object is left for sweep to inspect.
    let offsets = reference_offsets.to_vec();
    let body_layout = Layout::new::<GcMutexBody>();
    let body_end = HEADER_BYTES
        .checked_add(body_layout.size())
        .expect("GC mutex body end overflow");
    // Compute alignment against the allocation base, not against the body:
    // HEADER_BYTES is 24 on 64-bit targets, so the body starts at offset 8
    // modulo 16 even though the allocation base itself is 16-byte aligned.
    let payload_align = payload_layout.align();
    let payload_offset = body_end
        .checked_add(payload_align - 1)
        .map(|value| value & !(payload_align - 1))
        .expect("GC mutex payload alignment overflow");
    let allocation_size = payload_offset
        .checked_add(payload_layout.size())
        .expect("GC mutex allocation size overflow");
    let allocation_align = body_layout.align().max(payload_align);
    Layout::from_size_align(allocation_size, allocation_align)
        .expect("invalid GC mutex allocation layout");
    assert!(
        payload_offset.is_multiple_of(payload_layout.align()),
        "GC mutex payload layout lost its required alignment"
    );

    // Keep managed pointers in the by-value input alive if zirk_rt_alloc
    // triggers collection before the object can be initialized.
    let mut roots: Vec<*mut c_void> = reference_offsets
        .iter()
        .map(|&offset| unsafe { (std::ptr::addr_of_mut!(payload).cast::<u8>().add(offset)).cast() })
        .collect();
    unsafe { crate::collector::zirk_rt_push_frame(roots.as_mut_ptr(), roots.len() as i64) };
    let _root_frame = RootFrameGuard;

    let object = unsafe {
        crate::memory::zirk_rt_alloc(
            allocation_size,
            payload_layout.align().max(body_layout.align()),
        )
    };
    unsafe { *(object as *mut *mut c_void) = descriptor() };
    unsafe {
        body(object).write(GcMutexBody {
            lock: CooperativeMutex::new(()),
            reference_offsets: offsets,
            drop_payload: drop_payload::<T>,
            payload_offset,
            payload_size: payload_layout.size(),
        });
        (object as *mut u8)
            .add(payload_offset)
            .cast::<T>()
            .write(payload);
    }
    drop(_root_frame);
    object
}

/// Dynamic counterpart of `allocate` for generated-code ABI callers. All
/// input validation and native staging allocations happen before entering GC.
unsafe fn allocate_dynamic(
    payload: *const u8,
    payload_size: usize,
    payload_align: usize,
    offsets_ptr: *const usize,
    offset_count: usize,
) -> Result<*mut c_void, AllocateError> {
    if payload.is_null()
        || payload_size > MAX_PAYLOAD_BYTES
        || payload_align == 0
        || !payload_align.is_power_of_two()
        || payload_align > crate::collector::allocation_align(1)
        || !(payload as usize).is_multiple_of(payload_align)
        || (payload as usize).checked_add(payload_size).is_none()
        || offset_count > MAX_REFERENCE_OFFSETS
        || offset_count
            .checked_mul(std::mem::size_of::<usize>())
            .is_none()
        || (offset_count != 0 && offsets_ptr.is_null())
        || (offset_count != 0
            && (offsets_ptr as usize)
                .checked_add(offset_count * std::mem::size_of::<usize>())
                .is_none())
        || (!offsets_ptr.is_null()
            && !(offsets_ptr as usize).is_multiple_of(std::mem::align_of::<usize>()))
    {
        return Err(AllocateError::InvalidArgument);
    }
    if offset_count > payload_size / std::mem::size_of::<*mut c_void>() {
        return Err(AllocateError::InvalidArgument);
    }
    // SAFETY: caller guarantees readable `payload_size` bytes and, when count
    // is nonzero, `offset_count` readable aligned offsets. Bounds are capped.
    let offsets_ptr = if offset_count == 0 {
        std::ptr::NonNull::<usize>::dangling().as_ptr()
    } else {
        offsets_ptr
    };
    let offsets = unsafe { std::slice::from_raw_parts(offsets_ptr, offset_count) };
    let mut staged_offsets = Vec::new();
    staged_offsets
        .try_reserve_exact(offset_count)
        .map_err(|_| AllocateError::Allocation)?;
    for &offset in offsets {
        if offset
            .checked_add(std::mem::size_of::<*mut c_void>())
            .is_none_or(|end| end > payload_size)
            || !offset.is_multiple_of(std::mem::align_of::<*mut c_void>())
        {
            return Err(AllocateError::InvalidArgument);
        }
        staged_offsets.push(offset);
    }

    let word_size = std::mem::size_of::<usize>();
    let staged_word_count = payload_size
        .checked_add(word_size - 1)
        .ok_or(AllocateError::InvalidArgument)?
        / word_size;
    let mut staged_words = Vec::<usize>::new();
    staged_words
        .try_reserve_exact(staged_word_count)
        .map_err(|_| AllocateError::Allocation)?;
    staged_words.resize(staged_word_count, 0);
    let staged_bytes = staged_words.as_mut_ptr().cast::<u8>();
    // SAFETY: validated source range; destination has at least payload_size
    // initialized writable bytes and word alignment for every pointer slot.
    unsafe { std::ptr::copy_nonoverlapping(payload, staged_bytes, payload_size) };

    let body_layout = Layout::new::<GcMutexBody>();
    let body_end = HEADER_BYTES
        .checked_add(body_layout.size())
        .ok_or(AllocateError::InvalidArgument)?;
    let payload_offset = body_end
        .checked_add(payload_align - 1)
        .map(|value| value & !(payload_align - 1))
        .ok_or(AllocateError::InvalidArgument)?;
    // Keep a non-empty backing allocation for zero-sized source payloads so
    // the inline pointer remains within the collector allocation.
    let allocation_size = payload_offset
        .checked_add(payload_size.max(1))
        .ok_or(AllocateError::InvalidArgument)?;
    Layout::from_size_align(allocation_size, body_layout.align().max(payload_align))
        .map_err(|_| AllocateError::InvalidArgument)?;

    // Root slots point into stable native staging memory and stay registered
    // until the GC allocation and inline object initialization have completed.
    let mut roots = Vec::<*mut c_void>::new();
    roots
        .try_reserve_exact(offset_count)
        .map_err(|_| AllocateError::Allocation)?;
    for &offset in &staged_offsets {
        // SAFETY: each offset was validated as a complete aligned pointer slot.
        roots.push(unsafe { staged_bytes.add(offset).cast() });
    }
    unsafe { crate::collector::zirk_rt_push_frame(roots.as_mut_ptr(), roots.len() as i64) };
    let _root_frame = RootFrameGuard;

    // `zirk_rt_alloc` may collect. Every embedded managed pointer is rooted
    // from the staging bytes until it has been copied into the traced object.
    let object = unsafe { crate::memory::zirk_rt_alloc(allocation_size, payload_align) };
    unsafe {
        body(object).write(GcMutexBody {
            lock: CooperativeMutex::new(()),
            reference_offsets: staged_offsets,
            drop_payload: drop_bytes,
            payload_offset,
            payload_size,
        });
        std::ptr::copy_nonoverlapping(
            staged_bytes,
            (object as *mut u8).add(payload_offset),
            payload_size,
        );
        // Publish the identifying descriptor only after body/payload are ready.
        (object as *mut *mut c_void).write(descriptor());
    }
    drop(_root_frame);
    Ok(object)
}

unsafe fn drop_bytes(_: *mut u8) {}

/// Allocates an inline dynamically-sized payload with GC-traced pointer slots.
/// Status: 0 success, 1 invalid argument/layout/offset, 4 caught runtime panic.
/// The caller must provide readable input ranges and a writable aligned status.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_gc_mutex_alloc(
    payload: *const c_void,
    payload_size: usize,
    payload_align: usize,
    reference_offsets: *const usize,
    reference_count: usize,
    status_out: *mut u32,
) -> *mut c_void {
    if status_out.is_null() || !(status_out as usize).is_multiple_of(std::mem::align_of::<u32>()) {
        return std::ptr::null_mut();
    }
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
        allocate_dynamic(
            payload.cast(),
            payload_size,
            payload_align,
            reference_offsets,
            reference_count,
        )
    }));
    match result {
        Ok(Ok(handle)) => {
            unsafe { status_out.write(STATUS_OK) };
            handle
        }
        Ok(Err(AllocateError::InvalidArgument)) => {
            unsafe { status_out.write(STATUS_INVALID_ARGUMENT) };
            std::ptr::null_mut()
        }
        Ok(Err(AllocateError::Allocation)) | Err(_) => {
            unsafe { status_out.write(STATUS_INTERNAL_PANIC) };
            std::ptr::null_mut()
        }
    }
}

/// Acquires a managed mutex and returns its payload address in `payload_out`.
/// Status: 0 success, 1 invalid argument/context, 2 invalid handle,
/// 3 cancellation, 4 caught runtime panic. Reentrant acquisition, use outside
/// a task/managed worker, and release by a non-owner map to status 1.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_gc_mutex_acquire(
    handle: *mut c_void,
    payload_out: *mut *mut c_void,
    status_out: *mut u32,
) -> u32 {
    if status_out.is_null() || !(status_out as usize).is_multiple_of(std::mem::align_of::<u32>()) {
        return STATUS_INVALID_ARGUMENT;
    }
    if payload_out.is_null()
        || !(payload_out as usize).is_multiple_of(std::mem::align_of::<*mut c_void>())
    {
        unsafe { status_out.write(STATUS_INVALID_ARGUMENT) };
        return STATUS_INVALID_ARGUMENT;
    }
    unsafe { payload_out.write(std::ptr::null_mut()) };
    let result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe { acquire(handle) }));
    let status = match result {
        Ok(Ok(payload)) => {
            unsafe { payload_out.write(payload) };
            STATUS_OK
        }
        Ok(Err(GcMutexError::InvalidHandle)) => STATUS_INVALID_HANDLE,
        Ok(Err(GcMutexError::Lock(crate::sync_mutex::MutexError::Cancelled))) => STATUS_CANCELLED,
        Ok(Err(GcMutexError::Lock(
            crate::sync_mutex::MutexError::OutsideTask
            | crate::sync_mutex::MutexError::Reentrant
            | crate::sync_mutex::MutexError::WrongOwner,
        ))) => STATUS_INVALID_ARGUMENT,
        Err(_) => STATUS_INTERNAL_PANIC,
    };
    unsafe { status_out.write(status) };
    status
}

/// Releases the current participant's acquisition. Status codes match acquire;
/// releasing from a non-owner is reported as invalid argument/context.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_gc_mutex_release(
    handle: *mut c_void,
    status_out: *mut u32,
) -> u32 {
    if status_out.is_null() || !(status_out as usize).is_multiple_of(std::mem::align_of::<u32>()) {
        return STATUS_INVALID_ARGUMENT;
    }
    let result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe { release(handle) }));
    let status = match result {
        Ok(Ok(())) => STATUS_OK,
        Ok(Err(GcMutexError::InvalidHandle)) => STATUS_INVALID_HANDLE,
        Ok(Err(GcMutexError::Lock(crate::sync_mutex::MutexError::Cancelled))) => STATUS_CANCELLED,
        Ok(Err(GcMutexError::Lock(
            crate::sync_mutex::MutexError::OutsideTask
            | crate::sync_mutex::MutexError::Reentrant
            | crate::sync_mutex::MutexError::WrongOwner,
        ))) => STATUS_INVALID_ARGUMENT,
        Err(_) => STATUS_INTERNAL_PANIC,
    };
    unsafe { status_out.write(status) };
    status
}

/// Runs a synchronous callback while holding a managed mutex and releases it
/// through RAII on normal return or unwind. The payload pointer is valid only
/// for the duration of the call and must not be retained by the callback.
/// Source lowering additionally proves that the callback cannot suspend,
/// allocate, block, spawn, or capture/escape the borrowed view.
///
/// Status: 0 success, 1 invalid argument/context, 2 invalid handle,
/// 3 cancellation while waiting, 4 caught Rust unwind.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_gc_mutex_with(
    handle: *mut c_void,
    callback: Option<MutexCallback>,
    context: *mut c_void,
) -> u32 {
    let Some(callback) = callback else {
        return STATUS_INVALID_ARGUMENT;
    };
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let body = unsafe { checked_body(handle) }?;
        let mutex = unsafe { &*body };
        mutex.lock.acquire().map_err(GcMutexError::Lock)?;
        let _guard = LockGuard(&mutex.lock);
        // SAFETY: the lock is held until `_guard` drops after the callback.
        let payload = unsafe { (handle as *mut u8).add(mutex.payload_offset).cast() };
        // SAFETY: callback validity and context readability are part of this
        // compiler/runtime ABI contract; generated callers pass a lifted
        // synchronous thunk and its live capture block.
        unsafe { callback(payload, context) };
        Ok::<(), GcMutexError>(())
    }));
    match result {
        Ok(Ok(())) => STATUS_OK,
        Ok(Err(GcMutexError::InvalidHandle)) => STATUS_INVALID_HANDLE,
        Ok(Err(GcMutexError::Lock(crate::sync_mutex::MutexError::Cancelled))) => STATUS_CANCELLED,
        Ok(Err(GcMutexError::Lock(_))) => STATUS_INVALID_ARGUMENT,
        Err(_) => STATUS_INTERNAL_PANIC,
    }
}

/// Acquires a GC mutex and returns its inline payload address. The returned
/// view is valid only until `release` and must not cross a scheduler, GC, or
/// host boundary. The source operation uses the synchronous callback ABI
/// below; this pair remains internal for runtime-level tests.
///
/// # Safety
/// `object` must be a live allocation created by `allocate` and rooted by the
/// caller for the entire lock lifetime.
#[allow(dead_code)] // used by the internal acquire/release runtime tests
pub(crate) unsafe fn acquire(object: *mut c_void) -> Result<*mut c_void, GcMutexError> {
    let body = unsafe { checked_body(object) }?;
    unsafe { &*body }
        .lock
        .acquire()
        .map_err(GcMutexError::Lock)?;
    // SAFETY: the caller holds the object's cooperative lock until `release`.
    let payload = unsafe { (object as *mut u8).add((*body).payload_offset).cast() };
    Ok(payload)
}

/// Releases an acquisition made by the current executor task.
///
/// # Safety
/// `object` must be the same live, rooted allocation previously passed to
/// `acquire` by this task.
#[allow(dead_code)] // used by the internal acquire/release runtime tests
pub(crate) unsafe fn release(object: *mut c_void) -> Result<(), GcMutexError> {
    let body = unsafe { checked_body(object) }?;
    unsafe { &*body }.lock.release().map_err(GcMutexError::Lock)
}

/// Resolves a caller-supplied handle without dereferencing it until exact
/// collector membership has been confirmed.
///
/// # Safety
/// The caller must prevent collection from racing this lookup. Generated
/// callers do so by running on the executor thread and keeping the object in a
/// GC-rooted slot.
unsafe fn checked_body(object: *mut c_void) -> Result<*mut GcMutexBody, GcMutexError> {
    if object.is_null()
        || !(object as usize).is_multiple_of(crate::collector::allocation_align(1))
        || !crate::collector::is_managed_allocation(object)
    {
        return Err(GcMutexError::InvalidHandle);
    }
    // SAFETY: membership established a live, exact-base collector allocation.
    if unsafe { crate::collector::object_descriptor(object) } != descriptor() {
        return Err(GcMutexError::InvalidHandle);
    }
    // SAFETY: the descriptor identifies the initialized GC mutex body.
    Ok(unsafe { body(object) })
}

/// Enqueues every managed reference embedded in the inline payload.
///
/// # Safety
/// `object` must be a live allocation carrying this module's sentinel.
pub(crate) unsafe fn mark_payload(object: *mut c_void, enqueue: &mut dyn FnMut(*mut c_void)) {
    let body = unsafe { &*body(object) };
    for &offset in &body.reference_offsets {
        debug_assert!(offset + std::mem::size_of::<*mut c_void>() <= body.payload_size);
        let field = unsafe {
            (object as *mut u8)
                .add(body.payload_offset + offset)
                .cast::<*mut c_void>()
        };
        enqueue(unsafe { *field });
    }
}

/// Releases Rust-owned mutex metadata and the inline payload before sweep
/// deallocates the enclosing allocation.
///
/// # Safety
/// `object` must be an unreachable allocation carrying this module's sentinel.
pub(crate) unsafe fn drop_object(object: *mut c_void) {
    let body_ptr = unsafe { body(object) };
    let payload_offset = unsafe { (*body_ptr).payload_offset };
    let drop_payload = unsafe { (*body_ptr).drop_payload };
    unsafe {
        drop_payload((object as *mut u8).add(payload_offset));
        std::ptr::drop_in_place(body_ptr);
    }
}

unsafe fn drop_payload<T>(payload: *mut u8) {
    unsafe { std::ptr::drop_in_place(payload.cast::<T>()) };
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use crate::collector::{
        collect_for_tests, descriptor_with_fields_for_tests, is_live_for_tests,
        reset_state_for_tests, set_threshold_for_tests, synthetic_object_for_tests, test_support,
    };
    use crate::executor::Executor;
    use crate::task::TaskOutcome;

    #[repr(C)]
    struct Nested {
        _tag: u32,
        managed: *mut c_void,
    }

    #[repr(C, align(16))]
    struct OverAligned {
        _bytes: [u8; 16],
    }

    #[test]
    fn payload_layout_respects_alignment_after_gc_header_and_mutex_metadata() {
        let _guard = test_support::test_guard();
        reset_state_for_tests();
        let object = allocate(OverAligned { _bytes: [0; 16] }, &[]);
        let payload = unsafe { (object as *mut u8).add((*body(object)).payload_offset) };
        assert_eq!(object as usize % 16, 0);
        assert_eq!(payload as usize % std::mem::align_of::<OverAligned>(), 0);
        let body_end = HEADER_BYTES + std::mem::size_of::<GcMutexBody>();
        let payload_offset = unsafe { (*body(object)).payload_offset };
        assert!(payload_offset >= body_end);
        if !body_end.is_multiple_of(std::mem::align_of::<OverAligned>()) {
            assert!(
                payload_offset > body_end,
                "misaligned payload needs padding"
            );
        }
        assert!(is_live_for_tests(object));
        collect_for_tests();
        assert!(!is_live_for_tests(object));
    }

    #[test]
    fn nested_payload_reference_is_traced_after_forced_collection() {
        let _guard = test_support::test_guard();
        reset_state_for_tests();
        let leaf_descriptor = descriptor_with_fields_for_tests(&[]);
        let leaf = unsafe { synthetic_object_for_tests(leaf_descriptor.as_ptr().cast(), 0) };
        let payload = Nested {
            _tag: 7,
            managed: leaf,
        };
        let offset = std::mem::offset_of!(Nested, managed);
        let mut object = allocate(payload, &[offset]);
        let mut roots = [(&raw mut object).cast::<c_void>()];
        unsafe { crate::collector::zirk_rt_push_frame(roots.as_mut_ptr(), 1) };
        set_threshold_for_tests(1);
        unsafe { crate::memory::zirk_rt_alloc(HEADER_BYTES, std::mem::align_of::<usize>()) };
        assert!(
            is_live_for_tests(leaf),
            "nested payload edge was not traced"
        );
        crate::collector::zirk_rt_pop_frame();
        collect_for_tests();
        reset_state_for_tests();
    }

    static DROPS: AtomicUsize = AtomicUsize::new(0);

    struct DropProbe(u8);
    impl Drop for DropProbe {
        fn drop(&mut self) {
            let _ = self.0;
            DROPS.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn sweep_drops_payload_and_rust_metadata_exactly_once() {
        let _guard = test_support::test_guard();
        reset_state_for_tests();
        DROPS.store(0, Ordering::SeqCst);
        let object = allocate(DropProbe(1), &[]);
        assert_eq!(
            unsafe { crate::collector::object_descriptor(object) },
            descriptor()
        );
        assert_eq!(DROPS.load(Ordering::SeqCst), 0);
        collect_for_tests();
        assert!(
            !is_live_for_tests(object),
            "unrooted mutex survived collection"
        );
        assert_eq!(DROPS.load(Ordering::SeqCst), 1);
        collect_for_tests();
        assert_eq!(DROPS.load(Ordering::SeqCst), 1);
        reset_state_for_tests();
    }

    #[test]
    fn cooperative_gc_mutex_exposes_payload_only_during_owned_lock() {
        let _guard = test_support::test_guard();
        reset_state_for_tests();
        let outcome = Executor::new().run_with_root(move || {
            let object = allocate(41usize, &[]);
            let payload = unsafe { acquire(object).expect("current task acquires mutex") };
            unsafe { *payload.cast::<usize>() += 1 };
            unsafe { release(object).expect("owner releases mutex") };

            let payload = unsafe { acquire(object).expect("mutex can be acquired again") };
            let value = unsafe { *payload.cast::<usize>() };
            unsafe { release(object).expect("owner releases mutex again") };
            value
        });
        assert!(matches!(outcome, TaskOutcome::Value(42)));
        collect_for_tests();
        reset_state_for_tests();
    }

    #[test]
    fn mutex_lookup_rejects_non_gc_handles_before_reading_the_header() {
        let _guard = test_support::test_guard();
        reset_state_for_tests();
        let outcome = Executor::new().run_with_root(|| {
            #[repr(align(16))]
            struct FakeHandle([u8; 16]);

            let mut fake = FakeHandle([0; 16]);
            let pointer = (&raw mut fake).cast::<c_void>();
            assert_eq!(
                unsafe { acquire(pointer) },
                Err(GcMutexError::InvalidHandle)
            );
            assert_eq!(
                unsafe { release(std::ptr::null_mut()) },
                Err(GcMutexError::InvalidHandle)
            );
            let _ = fake.0;
            0
        });
        assert!(matches!(outcome, TaskOutcome::Value(0)));
        reset_state_for_tests();
    }

    #[test]
    fn dynamic_mutex_abi_copies_and_traces_nested_payload_references() {
        let _guard = test_support::test_guard();
        reset_state_for_tests();
        let outcome = Executor::new().run_with_root(|| {
            let leaf_descriptor = descriptor_with_fields_for_tests(&[]);
            let leaf = unsafe { synthetic_object_for_tests(leaf_descriptor.as_ptr().cast(), 0) };
            let mut payload = Nested {
                _tag: 19,
                managed: leaf,
            };
            let reference_offset = std::mem::offset_of!(Nested, managed);
            let mut handle: *mut c_void = std::ptr::null_mut();
            let mut roots = [
                (&raw mut payload.managed).cast::<c_void>(),
                (&raw mut handle).cast::<c_void>(),
            ];
            unsafe { crate::collector::zirk_rt_push_frame(roots.as_mut_ptr(), roots.len() as i64) };
            set_threshold_for_tests(1);

            let mut status = u32::MAX;
            let allocated = unsafe {
                zirk_rt_gc_mutex_alloc(
                    (&raw const payload).cast(),
                    std::mem::size_of::<Nested>(),
                    std::mem::align_of::<Nested>(),
                    &reference_offset,
                    1,
                    &mut status,
                )
            };
            assert_eq!(status, STATUS_OK);
            assert!(!allocated.is_null());
            handle = allocated;
            assert!(is_live_for_tests(handle));
            assert_eq!(
                unsafe { crate::collector::object_descriptor(handle) },
                descriptor()
            );

            // Force collection after construction to verify the mutex's
            // descriptor traces pointers stored inline in the payload.
            unsafe { crate::memory::zirk_rt_alloc(HEADER_BYTES, std::mem::align_of::<usize>()) };
            assert!(is_live_for_tests(leaf));
            let mut view = std::ptr::null_mut();
            status = u32::MAX;
            assert_eq!(
                unsafe { zirk_rt_gc_mutex_acquire(handle, &mut view, &mut status) },
                STATUS_OK
            );
            assert_eq!(status, STATUS_OK);
            assert_eq!(unsafe { (*view.cast::<Nested>())._tag }, 19);
            assert_eq!(unsafe { (*view.cast::<Nested>()).managed }, leaf);
            assert_eq!(
                unsafe { zirk_rt_gc_mutex_release(handle, &mut status) },
                STATUS_OK
            );
            assert_eq!(status, STATUS_OK);
            crate::collector::zirk_rt_pop_frame();
            1
        });
        assert!(matches!(outcome, TaskOutcome::Value(1)));
        collect_for_tests();
        reset_state_for_tests();
    }

    #[test]
    fn dynamic_mutex_abi_rejects_malformed_layout_and_reports_reentrancy() {
        let _guard = test_support::test_guard();
        reset_state_for_tests();
        let payload = [0u8; 16];
        let mut status = u32::MAX;
        let bad_offset = usize::MAX;
        assert!(
            unsafe {
                zirk_rt_gc_mutex_alloc(
                    payload.as_ptr().cast(),
                    payload.len(),
                    8,
                    &bad_offset,
                    1,
                    &mut status,
                )
            }
            .is_null()
        );
        assert_eq!(status, STATUS_INVALID_ARGUMENT);
        let outcome = Executor::new().run_with_root(move || {
            let mut handle: *mut c_void = std::ptr::null_mut();
            let mut roots = [(&raw mut handle).cast::<c_void>()];
            unsafe { crate::collector::zirk_rt_push_frame(roots.as_mut_ptr(), 1) };
            handle = unsafe {
                zirk_rt_gc_mutex_alloc(
                    payload.as_ptr().cast(),
                    payload.len(),
                    1,
                    std::ptr::null(),
                    0,
                    &mut status,
                )
            };
            assert_eq!(status, STATUS_OK);
            assert!(is_live_for_tests(handle));
            assert_eq!(
                unsafe { crate::collector::object_descriptor(handle) },
                descriptor()
            );

            let mut view = std::ptr::null_mut();
            status = u32::MAX;
            assert_eq!(
                unsafe { zirk_rt_gc_mutex_acquire(handle, &mut view, &mut status) },
                STATUS_OK
            );
            assert_eq!(
                unsafe { zirk_rt_gc_mutex_acquire(handle, &mut view, &mut status) },
                STATUS_INVALID_ARGUMENT
            );
            assert_eq!(status, STATUS_INVALID_ARGUMENT);
            assert_eq!(
                unsafe { zirk_rt_gc_mutex_release(handle, &mut status) },
                STATUS_OK
            );
            crate::collector::zirk_rt_pop_frame();
            1
        });
        assert!(matches!(outcome, TaskOutcome::Value(1)));
        reset_state_for_tests();
    }

    #[test]
    fn dynamic_mutex_abi_accepts_zero_sized_payloads() {
        let _guard = test_support::test_guard();
        reset_state_for_tests();
        let outcome = Executor::new().run_with_root(|| {
            let payload = ();
            let mut handle: *mut c_void = std::ptr::null_mut();
            let mut roots = [(&raw mut handle).cast::<c_void>()];
            unsafe { crate::collector::zirk_rt_push_frame(roots.as_mut_ptr(), 1) };
            let mut status = u32::MAX;
            handle = unsafe {
                zirk_rt_gc_mutex_alloc(
                    (&raw const payload).cast(),
                    0,
                    std::mem::align_of::<()>(),
                    std::ptr::null(),
                    0,
                    &mut status,
                )
            };
            assert_eq!(status, STATUS_OK);
            assert!(!handle.is_null());
            let mut view = std::ptr::null_mut();
            assert_eq!(
                unsafe { zirk_rt_gc_mutex_acquire(handle, &mut view, &mut status) },
                STATUS_OK
            );
            assert!(!view.is_null());
            assert_eq!(
                unsafe { zirk_rt_gc_mutex_release(handle, &mut status) },
                STATUS_OK
            );
            crate::collector::zirk_rt_pop_frame();
            0
        });
        assert!(matches!(outcome, TaskOutcome::Value(0)));
        collect_for_tests();
        reset_state_for_tests();
    }

    unsafe extern "C-unwind" fn mutate_then_unwind(payload: *mut c_void, _: *mut c_void) {
        // SAFETY: the test invokes the callback only for a Mutex<u64> payload.
        unsafe { *payload.cast::<u64>() += 1 };
        panic!("injected callback unwind");
    }

    #[test]
    fn mutex_with_releases_the_lock_when_callback_unwinds() {
        let _guard = test_support::test_guard();
        reset_state_for_tests();
        let outcome = Executor::new().run_with_root(|| {
            let mut initial = 40_u64;
            let mut handle: *mut c_void = std::ptr::null_mut();
            let mut roots = [(&raw mut handle).cast::<c_void>()];
            unsafe { crate::collector::zirk_rt_push_frame(roots.as_mut_ptr(), 1) };
            let mut status = u32::MAX;
            handle = unsafe {
                zirk_rt_gc_mutex_alloc(
                    (&raw const initial).cast(),
                    std::mem::size_of::<u64>(),
                    std::mem::align_of::<u64>(),
                    std::ptr::null(),
                    0,
                    &mut status,
                )
            };
            assert_eq!(status, STATUS_OK);
            assert_eq!(
                unsafe {
                    zirk_rt_gc_mutex_with(handle, Some(mutate_then_unwind), std::ptr::null_mut())
                },
                STATUS_INTERNAL_PANIC
            );
            let mut view = std::ptr::null_mut();
            assert_eq!(
                unsafe { zirk_rt_gc_mutex_acquire(handle, &mut view, &mut status) },
                STATUS_OK,
                "the callback unwind must not strand the mutex"
            );
            assert_eq!(unsafe { *view.cast::<u64>() }, 41);
            assert_eq!(
                unsafe { zirk_rt_gc_mutex_release(handle, &mut status) },
                STATUS_OK
            );
            crate::collector::zirk_rt_pop_frame();
            initial = 0;
            let _ = initial;
            0
        });
        assert!(matches!(outcome, TaskOutcome::Value(0)));
        collect_for_tests();
        reset_state_for_tests();
    }
}
