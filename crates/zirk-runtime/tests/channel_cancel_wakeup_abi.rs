//! Public C ABI regression: cancelling a parked receiver must not consume a
//! later send or prevent a subsequent receiver from being woken.

use std::ffi::c_void;
use std::sync::atomic::{AtomicI32, Ordering};

use zirk_runtime::{
    CHAN_CANCELLED, CHAN_RECEIVED, CHAN_SENT, zirk_rt_cancel, zirk_rt_chan_new, zirk_rt_chan_recv,
    zirk_rt_chan_send, zirk_rt_job_wait, zirk_rt_run_main, zirk_rt_sleep, zirk_rt_spawn,
};

static CANCELLED_RECEIVE_STATUS: AtomicI32 = AtomicI32::new(-1);

extern "C-unwind" fn receive(arg: *mut c_void) -> usize {
    let channel = arg as usize as *mut c_void;
    let mut value = 0_i32;
    let status = unsafe { zirk_rt_chan_recv(channel, (&raw mut value).cast()) };
    if status == CHAN_CANCELLED {
        CANCELLED_RECEIVE_STATUS.store(status, Ordering::SeqCst);
    }
    status as usize
}

extern "C-unwind" fn driver() {
    let channel = unsafe {
        zirk_rt_chan_new(
            0,
            0,
            std::mem::size_of::<i32>(),
            std::mem::align_of::<i32>(),
            std::ptr::null(),
            0,
        )
    };

    // Let the receiver enter the rendezvous receive and park before cancelling
    // it. Sleep is a public cooperative safe point, not a wall-clock thread wait.
    let cancelled = unsafe { zirk_rt_spawn(receive, channel) };
    unsafe { zirk_rt_sleep(1_000_000) };
    unsafe { zirk_rt_cancel(cancelled) };
    let cancelled_outcome = std::panic::catch_unwind(|| unsafe { zirk_rt_job_wait(cancelled) });
    assert!(
        cancelled_outcome.is_err(),
        "await must propagate cancellation"
    );
    assert_eq!(
        CANCELLED_RECEIVE_STATUS.load(Ordering::SeqCst),
        CHAN_CANCELLED,
        "the channel ABI must return its cancellation sentinel before task unwind"
    );

    // The cancelled waiter must be skipped. Its stale queue entry must neither
    // receive the value nor strand it; a later receiver must receive it once.
    let value = 73_i32;
    let sender = unsafe {
        zirk_rt_spawn(
            send_value,
            SendArg {
                channel: channel as usize,
                value,
            }
            .into_raw(),
        )
    };
    let receiver = unsafe { zirk_rt_spawn(receive_and_check, channel) };
    assert_eq!(unsafe { zirk_rt_job_wait(sender) }, CHAN_SENT as usize);
    assert_eq!(unsafe { zirk_rt_job_wait(receiver) }, value as usize);
}

struct SendArg {
    channel: usize,
    value: i32,
}

impl SendArg {
    fn into_raw(self) -> *mut c_void {
        Box::into_raw(Box::new(self)).cast()
    }
}

extern "C-unwind" fn send_value(arg: *mut c_void) -> usize {
    // SAFETY: the argument is allocated immediately before spawn and retained
    // until this callback runs exactly once.
    let arg = unsafe { Box::from_raw(arg.cast::<SendArg>()) };
    unsafe { zirk_rt_chan_send(arg.channel as *mut c_void, (&raw const arg.value).cast()) as usize }
}

extern "C-unwind" fn receive_and_check(arg: *mut c_void) -> usize {
    let channel = arg as usize as *mut c_void;
    let mut value = 0_i32;
    let status = unsafe { zirk_rt_chan_recv(channel, (&raw mut value).cast()) };
    assert_eq!(status, CHAN_RECEIVED);
    value as usize
}

#[test]
fn cancelled_parked_receiver_does_not_steal_next_rendezvous_send() {
    // This runs the same exported entrypoint used by generated programs.
    unsafe { zirk_rt_run_main(driver) };
}
