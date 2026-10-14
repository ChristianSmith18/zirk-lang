use std::ffi::c_void;
use std::mem::{align_of, offset_of, size_of};
use std::ptr;
use std::sync::atomic::{AtomicU32, Ordering};
use zirk_runtime::{
    zirk_rt_alloc, zirk_rt_cancel, zirk_rt_init, zirk_rt_job_wait, zirk_rt_pop_frame,
    zirk_rt_push_frame, zirk_rt_run_main, zirk_rt_sleep_checked, zirk_rt_spawn_checked,
};

const OK: u32 = 0;
const INVALID_ARGUMENT: u32 = 1;
const INVALID_HANDLE: u32 = 2;
const CANCELLED: u32 = 3;

unsafe extern "C" {
    fn zirk_rt_gc_mutex_alloc(
        payload: *const c_void,
        payload_size: usize,
        payload_align: usize,
        reference_offsets: *const usize,
        reference_count: usize,
        status_out: *mut u32,
    ) -> *mut c_void;
    fn zirk_rt_gc_mutex_acquire(
        handle: *mut c_void,
        payload_out: *mut *mut c_void,
        status_out: *mut u32,
    ) -> u32;
    fn zirk_rt_gc_mutex_release(handle: *mut c_void, status_out: *mut u32) -> u32;
    fn zirk_rt_gc_mutex_with(
        handle: *mut c_void,
        callback: Option<unsafe extern "C-unwind" fn(*mut c_void, *mut c_void)>,
        context: *mut c_void,
    ) -> u32;
}

#[repr(C)]
struct ParentPayload {
    marker: u64,
    child: *mut c_void,
}

static CANCEL_STATUS: AtomicU32 = AtomicU32::new(u32::MAX);
static WAITER_STARTED: AtomicU32 = AtomicU32::new(0);
static FOLLOWUP_CALLBACK_RAN: AtomicU32 = AtomicU32::new(0);

extern "C-unwind" fn cancelled_mutex_waiter(argument: *mut c_void) -> usize {
    WAITER_STARTED.store(1, Ordering::SeqCst);
    // SAFETY: argument is a rooted managed handle and this no-op callback is
    // synchronous, does not retain its payload, and is never invoked because
    // the task is cancelled while queued behind the owning task.
    let result =
        unsafe { zirk_rt_gc_mutex_with(argument, Some(noop_mutex_callback), ptr::null_mut()) };
    CANCEL_STATUS.store(result, Ordering::SeqCst);
    result as usize
}

unsafe extern "C-unwind" fn noop_mutex_callback(_payload: *mut c_void, _context: *mut c_void) {}

unsafe extern "C-unwind" fn mark_mutex_callback(_payload: *mut c_void, _context: *mut c_void) {
    FOLLOWUP_CALLBACK_RAN.store(1, Ordering::SeqCst);
}

extern "C-unwind" fn followup_mutex_waiter(argument: *mut c_void) -> usize {
    // SAFETY: argument is the rooted parent handle and the callback only
    // publishes a test flag while the runtime owns the lock.
    unsafe { zirk_rt_gc_mutex_with(argument, Some(mark_mutex_callback), ptr::null_mut()) as usize }
}

extern "C-unwind" fn runtime_case() {
    let mut status = u32::MAX;
    let mut payload = ptr::null_mut();
    // SAFETY: output slots are valid and aligned; null handle is intentionally invalid.
    let got = unsafe { zirk_rt_gc_mutex_acquire(ptr::null_mut(), &mut payload, &mut status) };
    assert_eq!(
        (got, status, payload),
        (INVALID_HANDLE, INVALID_HANDLE, ptr::null_mut())
    );
    // SAFETY: the current task may allocate and this payload contains no roots.
    let handle = unsafe {
        zirk_rt_gc_mutex_alloc(
            (&41usize as *const usize).cast(),
            8,
            8,
            ptr::null(),
            0,
            &mut status,
        )
    };
    assert_eq!(status, OK);
    assert!(!handle.is_null());
    // Reentrant acquisition is an invalid argument/context, not cancellation.
    assert_eq!(
        unsafe { zirk_rt_gc_mutex_acquire(handle, &mut payload, &mut status) },
        OK
    );
    assert_eq!(
        unsafe { zirk_rt_gc_mutex_acquire(handle, &mut payload, &mut status) },
        INVALID_ARGUMENT
    );
    assert_eq!(status, INVALID_ARGUMENT);
    assert_eq!(unsafe { zirk_rt_gc_mutex_release(handle, &mut status) }, OK);

    let mut status = u32::MAX;
    // SAFETY: all output pointers are valid; an aligned stack address is not a GC handle.
    let got = unsafe {
        zirk_rt_gc_mutex_acquire((&mut 0u128 as *mut u128).cast(), &mut payload, &mut status)
    };
    assert_eq!((got, status), (INVALID_HANDLE, INVALID_HANDLE));

    let mut invalid_status = u32::MAX;
    // SAFETY: null payload is explicitly invalid and no payload bytes are read.
    let invalid =
        unsafe { zirk_rt_gc_mutex_alloc(ptr::null(), 8, 8, ptr::null(), 0, &mut invalid_status) };
    assert!(invalid.is_null());
    assert_eq!(invalid_status, INVALID_ARGUMENT);
    let bytes = [0u8; 16];
    let mut invalid_status = u32::MAX;
    // SAFETY: valid small readable buffer; declared alignment is not a power of two.
    let invalid = unsafe {
        zirk_rt_gc_mutex_alloc(
            bytes.as_ptr().cast(),
            8,
            3,
            ptr::null(),
            0,
            &mut invalid_status,
        )
    };
    assert!(invalid.is_null());
    assert_eq!(invalid_status, INVALID_ARGUMENT);
    let mut invalid_status = u32::MAX;
    // SAFETY: size is rejected before reading the short input buffer.
    let invalid = unsafe {
        zirk_rt_gc_mutex_alloc(
            bytes.as_ptr().cast(),
            16 * 1024 * 1024 + 1,
            1,
            ptr::null(),
            0,
            &mut invalid_status,
        )
    };
    assert!(invalid.is_null());
    assert_eq!(invalid_status, INVALID_ARGUMENT);
    let mut invalid_status = u32::MAX;
    // SAFETY: excessive count is rejected before reading the offsets pointer.
    let invalid = unsafe {
        zirk_rt_gc_mutex_alloc(
            bytes.as_ptr().cast(),
            bytes.len(),
            1,
            ptr::null(),
            4097,
            &mut invalid_status,
        )
    };
    assert!(invalid.is_null());
    assert_eq!(invalid_status, INVALID_ARGUMENT);
    let mut invalid_status = u32::MAX;
    // SAFETY: nonzero count with null offsets is rejected before forming a slice.
    let invalid = unsafe {
        zirk_rt_gc_mutex_alloc(
            bytes.as_ptr().cast(),
            bytes.len(),
            1,
            ptr::null(),
            1,
            &mut invalid_status,
        )
    };
    assert!(invalid.is_null());
    assert_eq!(invalid_status, INVALID_ARGUMENT);
    let bad_offset = [12usize];
    let mut invalid_status = u32::MAX;
    // SAFETY: arrays provide valid ranges; offset 12 cannot contain a full pointer in 16 bytes.
    let invalid = unsafe {
        zirk_rt_gc_mutex_alloc(
            bytes.as_ptr().cast(),
            bytes.len(),
            8,
            bad_offset.as_ptr(),
            bad_offset.len(),
            &mut invalid_status,
        )
    };
    assert!(invalid.is_null());
    assert_eq!(invalid_status, INVALID_ARGUMENT);
    let misaligned = unsafe { bytes.as_ptr().add(1) };
    let mut invalid_status = u32::MAX;
    // SAFETY: pointer is readable but violates the declared alignment and is rejected before copying.
    let invalid = unsafe {
        zirk_rt_gc_mutex_alloc(misaligned.cast(), 8, 8, ptr::null(), 0, &mut invalid_status)
    };
    assert!(invalid.is_null());
    assert_eq!(invalid_status, INVALID_ARGUMENT);

    // Keep the child alive through construction, then only the parent's traced
    // offset retains it across a forced mark/sweep collection.
    let mut child = unsafe {
        zirk_rt_gc_mutex_alloc(
            (&73usize as *const usize).cast(),
            8,
            8,
            ptr::null(),
            0,
            &mut status,
        )
    };
    assert_eq!(status, OK);
    let mut child_root = [(&mut child as *mut *mut c_void).cast::<c_void>()];
    // SAFETY: one writable root slot remains valid until paired pop below.
    unsafe { zirk_rt_push_frame(child_root.as_mut_ptr(), 1) };
    let parent_payload = ParentPayload { marker: 9, child };
    let child_offset = offset_of!(ParentPayload, child);
    let mut parent = unsafe {
        zirk_rt_gc_mutex_alloc(
            (&parent_payload as *const ParentPayload).cast(),
            size_of::<ParentPayload>(),
            align_of::<ParentPayload>(),
            &child_offset,
            1,
            &mut status,
        )
    };
    assert_eq!(status, OK);
    // SAFETY: parent now owns the only traced reference to child.
    zirk_rt_pop_frame();
    let mut roots = [(&mut parent as *mut *mut c_void).cast::<c_void>()];
    // SAFETY: parent handle is rooted across the allocation that triggers collection.
    unsafe { zirk_rt_push_frame(roots.as_mut_ptr(), 1) };
    // Exceed the default 1 MiB collection threshold. The target allocation is
    // intentionally unreachable; the nested child must remain reachable.
    unsafe { zirk_rt_alloc(2 * 1024 * 1024, 8) };

    let mut parent_data = ptr::null_mut();
    // SAFETY: current executor task owns parent; its rooted handle remains live.
    assert_eq!(
        unsafe { zirk_rt_gc_mutex_acquire(parent, &mut parent_data, &mut status) },
        OK
    );
    assert_eq!(status, OK);
    let parent_data = parent_data.cast::<ParentPayload>();
    assert_eq!(unsafe { (*parent_data).marker }, 9);

    // The child parks in `Mutex.with` behind the held mutex before being
    // cancelled; it must be removed from this mutex's wait queue.
    CANCEL_STATUS.store(u32::MAX, Ordering::SeqCst);
    WAITER_STARTED.store(0, Ordering::SeqCst);
    let mut spawn_status = u32::MAX;
    // SAFETY: callback is a valid thunk and parent is a rooted GC object.
    let waiter =
        unsafe { zirk_rt_spawn_checked(cancelled_mutex_waiter, parent, &mut spawn_status) };
    assert_eq!(spawn_status, OK);
    // Let the child enter the runtime ABI and park before cancelling it; this
    // exercises queued-waiter removal and status mapping through `.with`.
    // SAFETY: positive bounded timer sleep is valid inside the executor.
    assert_eq!(unsafe { zirk_rt_sleep_checked(10_000_000) }, OK);
    assert_eq!(WAITER_STARTED.load(Ordering::SeqCst), 1);
    // SAFETY: waiter is the live job returned immediately above.
    unsafe { zirk_rt_cancel(waiter) };
    let child_handle = unsafe { (*parent_data).child };
    let mut child_data = ptr::null_mut();
    // SAFETY: the child is retained through parent's descriptor-traced offset.
    assert_eq!(
        unsafe { zirk_rt_gc_mutex_acquire(child_handle, &mut child_data, &mut status) },
        OK
    );
    assert_eq!(status, OK);
    unsafe { *child_data.cast::<usize>() = 91 };
    assert_eq!(
        unsafe { zirk_rt_gc_mutex_release(child_handle, &mut status) },
        OK
    );
    assert_eq!(status, OK);
    assert_eq!(unsafe { *child_data.cast::<usize>() }, 91);
    assert_eq!(unsafe { zirk_rt_gc_mutex_release(parent, &mut status) }, OK);
    assert_eq!(status, OK);

    FOLLOWUP_CALLBACK_RAN.store(0, Ordering::SeqCst);
    let mut followup_status = u32::MAX;
    // SAFETY: parent remains rooted by the active root frame until this
    // callback and its join have completed.
    let followup =
        unsafe { zirk_rt_spawn_checked(followup_mutex_waiter, parent, &mut followup_status) };
    assert_eq!(followup_status, OK);
    assert_eq!(unsafe { zirk_rt_job_wait(followup) }, OK as usize);
    assert_eq!(FOLLOWUP_CALLBACK_RAN.load(Ordering::SeqCst), 1);

    // A valid, unlocked handle released by a non-owner is invalid context.
    let mut wrong_owner_status = u32::MAX;
    assert_eq!(
        unsafe { zirk_rt_gc_mutex_release(parent, &mut wrong_owner_status) },
        INVALID_ARGUMENT
    );
    assert_eq!(wrong_owner_status, INVALID_ARGUMENT);
    zirk_rt_pop_frame();
}

#[test]
fn dynamic_inline_mutex_c_abi_validates_roots_and_lock_status() {
    let mut bootstrap_status = u32::MAX;
    // SAFETY: stack payload is readable for the duration of allocation.
    let bootstrap_handle = unsafe {
        zirk_rt_gc_mutex_alloc(
            (&17usize as *const usize).cast(),
            8,
            8,
            ptr::null(),
            0,
            &mut bootstrap_status,
        )
    };
    assert_eq!(bootstrap_status, OK);
    let mut bootstrap_payload = ptr::null_mut();
    let mut outside_status = u32::MAX;
    // SAFETY: valid bootstrap handle and output slots; no executor participant exists.
    assert_eq!(
        unsafe {
            zirk_rt_gc_mutex_acquire(
                bootstrap_handle,
                &mut bootstrap_payload,
                &mut outside_status,
            )
        },
        INVALID_ARGUMENT
    );
    assert_eq!(outside_status, INVALID_ARGUMENT);

    CANCEL_STATUS.store(u32::MAX, Ordering::SeqCst);
    WAITER_STARTED.store(0, Ordering::SeqCst);
    // SAFETY: initialize once and run a valid executor root callback.
    unsafe {
        zirk_rt_init();
        zirk_rt_run_main(runtime_case);
    }
    assert_eq!(CANCEL_STATUS.load(Ordering::SeqCst), CANCELLED);
}
