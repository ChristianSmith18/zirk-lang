//! Direct integration checks for resource ceilings exposed by the runtime.
//!
//! This deliberately tests only limits that production code already defines;
//! it does not imply an active-task or process-wide managed-thread ceiling.

use std::ffi::c_void;

use zirk_runtime::{
    AdmissionError, BACKGROUND_JOB_CAPACITY, BackgroundJobId, BackgroundSupervisor, CHAN_FULL,
    CHAN_SENT, DEFAULT_UNBOUNDED_LIMIT, zirk_rt_chan_len, zirk_rt_chan_new, zirk_rt_chan_try_send,
    zirk_rt_init, zirk_rt_pop_frame, zirk_rt_push_frame,
};

#[test]
fn growable_channel_rejects_exactly_the_first_value_over_its_default_limit() {
    // SAFETY: runtime initialization is required before allocating a channel.
    unsafe { zirk_rt_init() };
    let channel = unsafe {
        zirk_rt_chan_new(
            usize::MAX,
            0,
            std::mem::size_of::<u8>(),
            std::mem::align_of::<u8>(),
            std::ptr::null(),
            0,
        )
    };
    assert!(!channel.is_null());

    // Keep the managed channel alive while the queue grows and allocations
    // may trigger collection.
    let mut roots: [*mut c_void; 1] = [channel];
    unsafe { zirk_rt_push_frame(roots.as_mut_ptr(), roots.len() as i64) };

    let value = 0_u8;
    for index in 0..DEFAULT_UNBOUNDED_LIMIT {
        assert_eq!(
            unsafe { zirk_rt_chan_try_send(channel, (&raw const value).cast()) },
            CHAN_SENT,
            "send {index} should fit below the documented default limit"
        );
    }
    assert_eq!(
        unsafe { zirk_rt_chan_len(channel) },
        DEFAULT_UNBOUNDED_LIMIT
    );
    assert_eq!(
        unsafe { zirk_rt_chan_try_send(channel, (&raw const value).cast()) },
        CHAN_FULL,
        "the first value beyond the default limit must be rejected"
    );
    assert_eq!(
        unsafe { zirk_rt_chan_len(channel) },
        DEFAULT_UNBOUNDED_LIMIT
    );

    zirk_rt_pop_frame();
}

#[test]
fn background_supervisor_rejects_the_first_job_over_its_configured_ceiling() {
    let supervisor = BackgroundSupervisor::new(BACKGROUND_JOB_CAPACITY, |_| {});

    for raw_id in 0..BACKGROUND_JOB_CAPACITY as u64 {
        assert_eq!(supervisor.admit(BackgroundJobId(raw_id)), Ok(()));
    }
    assert_eq!(
        supervisor.admit(BackgroundJobId(BACKGROUND_JOB_CAPACITY as u64)),
        Err(AdmissionError::CapacityReached)
    );
    assert_eq!(supervisor.begin_drain().len(), BACKGROUND_JOB_CAPACITY);
}
