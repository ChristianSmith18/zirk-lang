//! Typed-channel storage and the indirect element-slot C ABI.
//!
//! A channel is a collector-managed handle whose Rust body starts immediately
//! after the ordinary three-word object header. Elements are copied as raw
//! bytes according to the layout supplied by codegen; this keeps the ABI
//! generic without forcing scalar values through heap boxes.

use std::collections::{HashSet, VecDeque};
use std::ffi::c_void;

use crate::collector::HEADER_BYTES;
use crate::executor;
use crate::task::TaskId;

/// Operation result codes shared by blocking and non-blocking C calls.
pub const CHAN_SENT: i32 = 0;
pub const CHAN_FULL: i32 = 1;
pub const CHAN_CLOSED: i32 = 2;
pub const CHAN_RECEIVED: i32 = 0;
pub const CHAN_EMPTY: i32 = 1;

/// Bare `Channel<T>()` uses this bounded defense limit.
pub const DEFAULT_UNBOUNDED_LIMIT: usize = 1 << 16;

/// A sentinel descriptor recognized by the collector's channel trace hook.
#[unsafe(no_mangle)]
#[allow(non_upper_case_globals)]
pub static zirk_rt_channel_descriptor: u8 = 0;

#[inline]
pub(crate) fn channel_descriptor() -> *mut c_void {
    (&raw const zirk_rt_channel_descriptor) as *mut c_void
}

struct WaitingSender {
    task: TaskId,
    value: Vec<u8>,
}

struct WaitingReceiver {
    task: TaskId,
    destination: *mut u8,
}

/// The Rust payload stored after the GC header. It never crosses the C ABI.
struct ChannelBody {
    element_size: usize,
    _element_align: usize,
    reference_offsets: Vec<usize>,
    /// `None` is growable; `Some(0)` is rendezvous.
    capacity: Option<usize>,
    limit: usize,
    closed: bool,
    queue: VecDeque<Vec<u8>>,
    senders: VecDeque<WaitingSender>,
    receivers: VecDeque<WaitingReceiver>,
    completed_senders: HashSet<TaskId>,
    completed_receivers: HashSet<TaskId>,
}

#[inline]
unsafe fn body(channel: *mut c_void) -> *mut ChannelBody {
    unsafe { (channel as *mut u8).add(HEADER_BYTES).cast::<ChannelBody>() }
}

/// Releases the Rust-owned payload before the collector returns the enclosing
/// allocation to the allocator. Collector-managed objects do not otherwise
/// run Rust destructors, so this is required for queued byte buffers and wait
/// lists not to leak when an unreachable channel is swept.
pub(crate) unsafe fn drop_body(channel: *mut c_void) {
    unsafe { std::ptr::drop_in_place(body(channel)) };
}

unsafe fn copy_from_slot(source: *const u8, size: usize) -> Vec<u8> {
    if size == 0 {
        return Vec::new();
    }
    assert!(!source.is_null(), "channel send requires an element slot");
    let mut value = vec![0; size];
    unsafe { std::ptr::copy_nonoverlapping(source, value.as_mut_ptr(), size) };
    value
}

unsafe fn copy_to_slot(value: &[u8], destination: *mut u8) {
    if value.is_empty() {
        return;
    }
    assert!(
        !destination.is_null(),
        "channel receive requires an element slot"
    );
    unsafe { std::ptr::copy_nonoverlapping(value.as_ptr(), destination, value.len()) };
}

fn receiver_waiting(body: &mut ChannelBody) -> Option<WaitingReceiver> {
    while let Some(receiver) = body.receivers.pop_front() {
        if !executor::task_cancel_requested(receiver.task) {
            return Some(receiver);
        }
    }
    None
}

fn sender_waiting(body: &mut ChannelBody) -> Option<WaitingSender> {
    while let Some(sender) = body.senders.pop_front() {
        if !executor::task_cancel_requested(sender.task) {
            return Some(sender);
        }
    }
    None
}

fn room(body: &ChannelBody) -> bool {
    match body.capacity {
        Some(capacity) => body.queue.len() < capacity,
        None => body.queue.len() < body.limit,
    }
}

fn wake_receiver(receiver: WaitingReceiver, value: &[u8], body: &mut ChannelBody) {
    unsafe { copy_to_slot(value, receiver.destination) };
    body.completed_receivers.insert(receiver.task);
    executor::wake_task(receiver.task);
}

/// Moves one waiter into newly available buffer capacity. The waiter owns a
/// stable byte copy, so its original stack slot need not stay live.
fn refill_from_sender(body: &mut ChannelBody) {
    if body.closed || !room(body) {
        return;
    }
    if let Some(sender) = sender_waiting(body) {
        body.queue.push_back(sender.value);
        body.completed_senders.insert(sender.task);
        executor::wake_task(sender.task);
    }
}

fn remove_sender(body: &mut ChannelBody, task: TaskId) {
    body.senders.retain(|sender| sender.task != task);
}

fn remove_receiver(body: &mut ChannelBody, task: TaskId) {
    body.receivers.retain(|receiver| receiver.task != task);
}

/// Allocates a channel. `capacity == usize::MAX` denotes growable storage;
/// `limit == 0` then selects [`DEFAULT_UNBOUNDED_LIMIT`].
///
/// # Safety
///
/// `reference_offsets` must be null for zero count or point to that many valid
/// `usize` offsets; the supplied layout must describe every future element slot.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_chan_new(
    capacity: usize,
    limit: usize,
    element_size: usize,
    element_align: usize,
    reference_offsets: *const usize,
    reference_offset_count: usize,
) -> *mut c_void {
    let allocation_size = HEADER_BYTES + std::mem::size_of::<ChannelBody>();
    let channel = unsafe { crate::memory::zirk_rt_alloc(allocation_size, element_align.max(1)) };
    unsafe { *(channel as *mut *mut c_void) = channel_descriptor() };
    let offsets = if reference_offset_count == 0 {
        Vec::new()
    } else {
        assert!(
            !reference_offsets.is_null(),
            "channel reference offsets are null"
        );
        unsafe { std::slice::from_raw_parts(reference_offsets, reference_offset_count) }.to_vec()
    };
    for &offset in &offsets {
        assert!(
            offset + std::mem::size_of::<*mut c_void>() <= element_size,
            "channel GC offset is outside its element"
        );
    }
    let channel_body = ChannelBody {
        element_size,
        _element_align: element_align,
        reference_offsets: offsets,
        capacity: (capacity != usize::MAX).then_some(capacity),
        limit: if capacity == usize::MAX {
            if limit == 0 {
                DEFAULT_UNBOUNDED_LIMIT
            } else {
                limit
            }
        } else {
            capacity
        },
        closed: false,
        queue: VecDeque::new(),
        senders: VecDeque::new(),
        receivers: VecDeque::new(),
        completed_senders: HashSet::new(),
        completed_receivers: HashSet::new(),
    };
    unsafe { body(channel).write(channel_body) };
    channel
}

/// Sends without suspension. A rendezvous send succeeds only if a receiver is
/// already parked; otherwise it reports `CHAN_FULL`.
///
/// # Safety
///
/// `channel` must be a live handle from [`zirk_rt_chan_new`] and `source` must
/// point to an initialized element of that channel's configured size.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_chan_try_send(channel: *mut c_void, source: *const u8) -> i32 {
    let body = unsafe { &mut *body(channel) };
    if body.closed {
        return CHAN_CLOSED;
    }
    if let Some(receiver) = receiver_waiting(body) {
        let value = unsafe { copy_from_slot(source, body.element_size) };
        wake_receiver(receiver, &value, body);
        return CHAN_SENT;
    }
    if !room(body) {
        return CHAN_FULL;
    }
    let value = unsafe { copy_from_slot(source, body.element_size) };
    body.queue.push_back(value);
    CHAN_SENT
}

/// Receives without suspension, copying a value only for `CHAN_RECEIVED`.
///
/// # Safety
///
/// `channel` must be live and `destination` must point to writable storage for
/// one configured element whenever a value can be received.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_chan_try_recv(channel: *mut c_void, destination: *mut u8) -> i32 {
    let body = unsafe { &mut *body(channel) };
    if let Some(value) = body.queue.pop_front() {
        unsafe { copy_to_slot(&value, destination) };
        refill_from_sender(body);
        return CHAN_RECEIVED;
    }
    if body.capacity == Some(0)
        && let Some(sender) = sender_waiting(body)
    {
        unsafe { copy_to_slot(&sender.value, destination) };
        body.completed_senders.insert(sender.task);
        executor::wake_task(sender.task);
        return CHAN_RECEIVED;
    }
    if body.closed { CHAN_CLOSED } else { CHAN_EMPTY }
}

/// Sends with cooperative backpressure. On success it returns `CHAN_SENT`; a
/// closed channel returns `CHAN_CLOSED`, which codegen maps to the language
/// channel-closed failure.
///
/// # Safety
///
/// `channel` and `source` follow the same validity contract as
/// [`zirk_rt_chan_try_send`]; this function additionally requires a running
/// cooperative executor task.
#[unsafe(no_mangle)]
pub unsafe extern "C-unwind" fn zirk_rt_chan_send(channel: *mut c_void, source: *const u8) -> i32 {
    let task = executor::current_task();
    loop {
        executor::check_cancelled();
        let result = unsafe { zirk_rt_chan_try_send(channel, source) };
        if result != CHAN_FULL {
            return result;
        }
        let value = {
            let body = unsafe { &mut *body(channel) };
            if body.completed_senders.remove(&task) {
                return CHAN_SENT;
            }
            remove_sender(body, task);
            unsafe { copy_from_slot(source, body.element_size) }
        };
        {
            let body = unsafe { &mut *body(channel) };
            if body.closed {
                return CHAN_CLOSED;
            }
            body.senders.push_back(WaitingSender { task, value });
        }
        executor::suspend_current(crate::task::WaitReason::ChannelSend);
        let body = unsafe { &mut *body(channel) };
        if body.completed_senders.remove(&task) {
            return CHAN_SENT;
        }
        remove_sender(body, task);
    }
}

/// Receives with cooperative suspension. `CHAN_CLOSED` represents `None`.
///
/// # Safety
///
/// `channel` and `destination` follow [`zirk_rt_chan_try_recv`]'s contract;
/// this function additionally requires a running cooperative executor task.
#[unsafe(no_mangle)]
pub unsafe extern "C-unwind" fn zirk_rt_chan_recv(
    channel: *mut c_void,
    destination: *mut u8,
) -> i32 {
    let task = executor::current_task();
    loop {
        executor::check_cancelled();
        let result = unsafe { zirk_rt_chan_try_recv(channel, destination) };
        if result != CHAN_EMPTY {
            return result;
        }
        {
            let body = unsafe { &mut *body(channel) };
            if body.completed_receivers.remove(&task) {
                return CHAN_RECEIVED;
            }
            remove_receiver(body, task);
            if body.closed {
                return CHAN_CLOSED;
            }
            body.receivers
                .push_back(WaitingReceiver { task, destination });
        }
        executor::suspend_current(crate::task::WaitReason::ChannelReceive);
        let body = unsafe { &mut *body(channel) };
        if body.completed_receivers.remove(&task) {
            return CHAN_RECEIVED;
        }
        remove_receiver(body, task);
    }
}

#[unsafe(no_mangle)]
/// # Safety
///
/// `channel` must be a live channel handle created by this runtime.
pub unsafe extern "C" fn zirk_rt_chan_close(channel: *mut c_void) {
    let body = unsafe { &mut *body(channel) };
    if body.closed {
        return;
    }
    body.closed = true;
    for sender in body.senders.drain(..) {
        executor::wake_task(sender.task);
    }
    for receiver in body.receivers.drain(..) {
        executor::wake_task(receiver.task);
    }
}

#[unsafe(no_mangle)]
/// # Safety
///
/// `channel` must be a live channel handle created by this runtime.
pub unsafe extern "C" fn zirk_rt_chan_len(channel: *mut c_void) -> usize {
    unsafe { (&*body(channel)).queue.len() }
}

#[unsafe(no_mangle)]
/// # Safety
///
/// `channel` must be a live channel handle created by this runtime.
pub unsafe extern "C" fn zirk_rt_chan_is_closed(channel: *mut c_void) -> bool {
    unsafe { (&*body(channel)).closed }
}

#[unsafe(no_mangle)]
/// # Safety
///
/// `channel` must be a live channel handle created by this runtime.
pub unsafe extern "C" fn zirk_rt_chan_capacity(channel: *mut c_void) -> usize {
    unsafe { (&*body(channel)).capacity.unwrap_or(usize::MAX) }
}

/// Removes every queued value in FIFO order and appends it to `list`.
/// The channel is not closed by this operation; an open channel may receive
/// more values immediately after the drain completes.
///
/// # Safety
///
/// `channel` and `list` must be live objects created by this runtime, and the
/// element layout arguments must match the channel and list element layout.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_chan_drain(
    channel: *mut c_void,
    list: *mut c_void,
    elem_size: usize,
    elem_align: usize,
) {
    let mut slot = vec![0_u8; elem_size];
    loop {
        let status = unsafe { zirk_rt_chan_recv(channel, slot.as_mut_ptr()) };
        if status == CHAN_CLOSED {
            break;
        }
        debug_assert_eq!(status, CHAN_RECEIVED);
        unsafe {
            crate::list::zirk_rt_list_add(list, slot.as_ptr().cast(), elem_size, elem_align, false);
        }
    }
}

/// Called by the collector when it reaches a channel handle.
pub(crate) unsafe fn mark_queued(channel: *mut c_void, mark: &mut dyn FnMut(*mut c_void)) {
    let body = unsafe { &*body(channel) };
    for value in &body.queue {
        for &offset in &body.reference_offsets {
            let pointer = unsafe {
                value
                    .as_ptr()
                    .add(offset)
                    .cast::<*mut c_void>()
                    .read_unaligned()
            };
            mark(pointer);
        }
    }
    for sender in &body.senders {
        for &offset in &body.reference_offsets {
            let pointer = unsafe {
                sender
                    .value
                    .as_ptr()
                    .add(offset)
                    .cast::<*mut c_void>()
                    .read_unaligned()
            };
            mark(pointer);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    unsafe fn int_channel(capacity: usize) -> *mut c_void {
        unsafe {
            zirk_rt_chan_new(
                capacity,
                0,
                std::mem::size_of::<i32>(),
                std::mem::align_of::<i32>(),
                std::ptr::null(),
                0,
            )
        }
    }

    #[test]
    fn bounded_try_operations_preserve_fifo_and_close() {
        let channel = unsafe { int_channel(2) };
        let one = 1_i32;
        let two = 2_i32;
        let three = 3_i32;
        assert_eq!(
            unsafe { zirk_rt_chan_try_send(channel, (&raw const one).cast()) },
            CHAN_SENT
        );
        assert_eq!(
            unsafe { zirk_rt_chan_try_send(channel, (&raw const two).cast()) },
            CHAN_SENT
        );
        assert_eq!(
            unsafe { zirk_rt_chan_try_send(channel, (&raw const three).cast()) },
            CHAN_FULL
        );
        let mut received = 0_i32;
        assert_eq!(
            unsafe { zirk_rt_chan_try_recv(channel, (&raw mut received).cast()) },
            CHAN_RECEIVED
        );
        assert_eq!(received, 1);
        assert_eq!(
            unsafe { zirk_rt_chan_try_recv(channel, (&raw mut received).cast()) },
            CHAN_RECEIVED
        );
        assert_eq!(received, 2);
        assert_eq!(
            unsafe { zirk_rt_chan_try_recv(channel, (&raw mut received).cast()) },
            CHAN_EMPTY
        );
        unsafe { zirk_rt_chan_close(channel) };
        assert_eq!(
            unsafe { zirk_rt_chan_try_send(channel, (&raw const one).cast()) },
            CHAN_CLOSED
        );
        assert_eq!(
            unsafe { zirk_rt_chan_try_recv(channel, (&raw mut received).cast()) },
            CHAN_CLOSED
        );
    }

    #[test]
    fn queued_aggregate_marks_every_reference_offset() {
        let channel = unsafe {
            zirk_rt_chan_new(
                1,
                0,
                2 * std::mem::size_of::<*mut c_void>(),
                std::mem::align_of::<*mut c_void>(),
                [0usize, std::mem::size_of::<*mut c_void>()].as_ptr(),
                2,
            )
        };
        let first = 0x1000usize as *mut c_void;
        let second = 0x2000usize as *mut c_void;
        let value = [first, second];
        assert_eq!(
            unsafe { zirk_rt_chan_try_send(channel, value.as_ptr().cast()) },
            CHAN_SENT
        );
        let mut marked = Vec::new();
        unsafe { mark_queued(channel, &mut |pointer| marked.push(pointer)) };
        assert_eq!(marked, vec![first, second]);
    }

    #[test]
    fn drain_preserves_fifo_before_and_after_close() {
        let outcome = executor::Executor::new().run_with_root(|| {
            let channel = unsafe { int_channel(4) };
            for value in [1_i32, 2, 3] {
                assert_eq!(
                    unsafe { zirk_rt_chan_try_send(channel, (&raw const value).cast()) },
                    CHAN_SENT
                );
            }
            unsafe { zirk_rt_chan_close(channel) };
            let list = unsafe { crate::list::zirk_rt_list_new(4, 4, false) };
            unsafe { zirk_rt_chan_drain(channel, list, 4, 4) };
            assert_eq!(unsafe { crate::list::zirk_rt_list_length(list) }, 3);
            for (i, expected) in [1_i32, 2, 3].into_iter().enumerate() {
                let ptr = unsafe { crate::list::zirk_rt_list_element(list, i as i64, 4) };
                assert_eq!(unsafe { ptr.cast::<i32>().read() }, expected);
            }
            let empty = unsafe { crate::list::zirk_rt_list_new(4, 4, false) };
            unsafe { zirk_rt_chan_drain(channel, empty, 4, 4) };
            assert_eq!(unsafe { crate::list::zirk_rt_list_length(empty) }, 0);
            0
        });
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
    }

    #[test]
    fn explicit_growable_limit_applies_backpressure() {
        let channel = unsafe {
            zirk_rt_chan_new(
                usize::MAX,
                2,
                std::mem::size_of::<i32>(),
                std::mem::align_of::<i32>(),
                std::ptr::null(),
                0,
            )
        };
        let one = 1_i32;
        let two = 2_i32;
        let three = 3_i32;
        assert_eq!(
            unsafe { zirk_rt_chan_try_send(channel, (&raw const one).cast()) },
            CHAN_SENT
        );
        assert_eq!(
            unsafe { zirk_rt_chan_try_send(channel, (&raw const two).cast()) },
            CHAN_SENT
        );
        assert_eq!(
            unsafe { zirk_rt_chan_try_send(channel, (&raw const three).cast()) },
            CHAN_FULL
        );
    }

    #[test]
    fn rendezvous_send_and_receive_suspend_without_blocking_the_executor() {
        let channel = unsafe { int_channel(0) } as usize;
        let outcome = executor::Executor::new().run_with_root(move || {
            let sender_channel = channel;
            executor::spawn(move || {
                let value = 42_i32;
                assert_eq!(
                    unsafe {
                        zirk_rt_chan_send(sender_channel as *mut c_void, (&raw const value).cast())
                    },
                    CHAN_SENT
                );
                0
            });
            let mut received = 0_i32;
            assert_eq!(
                unsafe { zirk_rt_chan_recv(channel as *mut c_void, (&raw mut received).cast()) },
                CHAN_RECEIVED
            );
            assert_eq!(received, 42);
            0
        });
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
    }

    #[test]
    fn backpressured_sender_resumes_after_fifo_capacity_frees() {
        let channel = unsafe { int_channel(1) } as usize;
        let outcome = executor::Executor::new().run_with_root(move || {
            let first = 1_i32;
            assert_eq!(
                unsafe { zirk_rt_chan_try_send(channel as *mut c_void, (&raw const first).cast()) },
                CHAN_SENT
            );
            let sender_channel = channel;
            executor::spawn(move || {
                let second = 2_i32;
                assert_eq!(
                    unsafe {
                        zirk_rt_chan_send(sender_channel as *mut c_void, (&raw const second).cast())
                    },
                    CHAN_SENT
                );
                0
            });
            executor::yield_now();
            let mut received = 0_i32;
            assert_eq!(
                unsafe { zirk_rt_chan_recv(channel as *mut c_void, (&raw mut received).cast()) },
                CHAN_RECEIVED
            );
            assert_eq!(received, 1);
            assert_eq!(
                unsafe { zirk_rt_chan_recv(channel as *mut c_void, (&raw mut received).cast()) },
                CHAN_RECEIVED
            );
            assert_eq!(received, 2);
            0
        });
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
    }

    #[test]
    fn closing_a_full_channel_wakes_sender_with_closed_outcome() {
        let channel = unsafe { int_channel(1) } as usize;
        let outcome = executor::Executor::new().run_with_root(move || {
            let first = 1_i32;
            assert_eq!(
                unsafe { zirk_rt_chan_try_send(channel as *mut c_void, (&raw const first).cast()) },
                CHAN_SENT
            );
            let sender_channel = channel;
            executor::spawn(move || {
                let second = 2_i32;
                assert_eq!(
                    unsafe {
                        zirk_rt_chan_send(sender_channel as *mut c_void, (&raw const second).cast())
                    },
                    CHAN_CLOSED
                );
                0
            });
            executor::yield_now();
            unsafe { zirk_rt_chan_close(channel as *mut c_void) };
            executor::yield_now();
            0
        });
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
    }

    #[test]
    fn cancellation_interrupts_a_suspended_sender() {
        let channel = unsafe { int_channel(1) } as usize;
        let outcome = executor::Executor::new().run_with_root(move || {
            let first = 1_i32;
            assert_eq!(
                unsafe { zirk_rt_chan_try_send(channel as *mut c_void, (&raw const first).cast()) },
                CHAN_SENT
            );
            let sender_channel = channel;
            let sender = executor::spawn(move || {
                let second = 2_i32;
                (unsafe {
                    zirk_rt_chan_send(sender_channel as *mut c_void, (&raw const second).cast())
                }) as usize
            });
            executor::yield_now();
            executor::cancel_current(sender);
            executor::await_task(sender)
        });
        assert!(
            matches!(outcome, crate::task::TaskOutcome::Panicked(_)),
            "cancelled send must unwind at its safe point"
        );
    }

    #[test]
    fn cancellation_interrupts_a_suspended_receiver() {
        let channel = unsafe { int_channel(1) } as usize;
        let outcome = executor::Executor::new().run_with_root(move || {
            let receiver_channel = channel;
            let receiver = executor::spawn(move || {
                let mut value = 0_i32;
                (unsafe {
                    zirk_rt_chan_recv(receiver_channel as *mut c_void, (&raw mut value).cast())
                }) as usize
            });
            executor::yield_now();
            executor::cancel_current(receiver);
            executor::await_task(receiver)
        });
        assert!(
            matches!(outcome, crate::task::TaskOutcome::Panicked(_)),
            "cancelled receive must unwind at its safe point"
        );
    }

    #[test]
    fn queued_reference_is_traced_through_the_channel() {
        let _guard = crate::collector::test_support::test_guard();
        crate::collector::reset_state_for_tests();
        let descriptor = crate::collector::descriptor_with_fields_for_tests(&[]);
        let leaf =
            unsafe { crate::collector::synthetic_object_for_tests(descriptor.as_ptr().cast(), 0) };
        let offsets = [0_usize];
        let mut channel = unsafe {
            zirk_rt_chan_new(
                1,
                0,
                std::mem::size_of::<*mut c_void>(),
                std::mem::align_of::<*mut c_void>(),
                offsets.as_ptr(),
                offsets.len(),
            )
        };
        let mut roots = [(&raw mut channel).cast::<c_void>()];
        unsafe { crate::collector::zirk_rt_push_frame(roots.as_mut_ptr(), 1) };
        assert_eq!(
            unsafe { zirk_rt_chan_try_send(channel, (&raw const leaf).cast()) },
            CHAN_SENT
        );
        crate::collector::set_threshold_for_tests(1);
        unsafe { crate::memory::zirk_rt_alloc(HEADER_BYTES, std::mem::align_of::<usize>()) };
        let mut received: *mut c_void = std::ptr::null_mut();
        assert_eq!(
            unsafe { zirk_rt_chan_try_recv(channel, (&raw mut received).cast()) },
            CHAN_RECEIVED
        );
        assert_eq!(received, leaf, "the queued reference survived collection");
        crate::collector::zirk_rt_pop_frame();
    }
}
