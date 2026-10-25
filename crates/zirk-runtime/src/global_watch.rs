//! Bounded event delivery for project-global subscriptions.
//!
//! Global cells serialize registration and publication in generated code.
//! This hub owns the per-cell revision and serializes channel operations so
//! the collector-managed channel bodies are never mutated concurrently.

use std::collections::BTreeMap;
use std::ffi::c_void;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

pub const GLOBAL_WATCH_RECEIVED: i32 = 0;
pub const GLOBAL_WATCH_EMPTY: i32 = 1;
pub const GLOBAL_WATCH_LAGGED: i32 = 2;
pub const GLOBAL_WATCH_CLOSED: i32 = 3;
pub const GLOBAL_WATCH_ERROR: i32 = 4;

const MAX_GLOBAL_WATCHES: usize = 4_096;

#[derive(Default)]
struct GlobalState {
    revision: u64,
    subscriptions: BTreeMap<u64, Subscription>,
}

struct Subscription {
    channel: usize,
    root: u64,
    lagged: bool,
    lagged_reported: bool,
}

#[derive(Default)]
struct Hub {
    globals: BTreeMap<usize, GlobalState>,
    tokens: BTreeMap<u64, usize>,
}

static HUB: OnceLock<Mutex<Hub>> = OnceLock::new();
static NEXT_TOKEN: AtomicU64 = AtomicU64::new(1);

fn allocate_token(counter: &AtomicU64) -> Option<u64> {
    counter
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |token| {
            (token != 0).then(|| token.checked_add(1)).flatten()
        })
        .ok()
}

fn hub() -> &'static Mutex<Hub> {
    HUB.get_or_init(|| Mutex::new(Hub::default()))
}

fn lock_hub() -> std::sync::MutexGuard<'static, Hub> {
    hub()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn remove_subscription(hub: &mut Hub, global: usize, token: u64) -> bool {
    let Some(state) = hub.globals.get_mut(&global) else {
        return false;
    };
    let Some(subscription) = state.subscriptions.remove(&token) else {
        return false;
    };
    hub.tokens.remove(&token);
    unsafe {
        crate::channel::zirk_rt_chan_close(subscription.channel as *mut c_void);
        crate::collector::zirk_rt_global_root_remove(subscription.root);
    }
    true
}

/// Resets per-cell state when a generated project-global cell is initialized.
///
/// # Safety
/// `global` must be a live project-global cell handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_global_watch_reset(global: *mut c_void) {
    if global.is_null() {
        return;
    }
    let key = global as usize;
    let mut hub = lock_hub();
    let Some(state) = hub.globals.remove(&key) else {
        hub.globals.insert(key, GlobalState::default());
        return;
    };
    for (token, subscription) in state.subscriptions {
        hub.tokens.remove(&token);
        unsafe {
            crate::channel::zirk_rt_chan_close(subscription.channel as *mut c_void);
            crate::collector::zirk_rt_global_root_remove(subscription.root);
        }
    }
    hub.globals.insert(key, GlobalState::default());
}

/// Registers a bounded event channel and returns the revision observed while
/// the caller holds the corresponding global-cell lock.
///
/// # Safety
/// `global` and `channel` must be live runtime handles. Output pointers must
/// point to writable `u64` values.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_global_watch_subscribe(
    global: *mut c_void,
    channel: *mut c_void,
    token_out: *mut u64,
    revision_out: *mut u64,
) -> i32 {
    if global.is_null() || channel.is_null() || token_out.is_null() || revision_out.is_null() {
        return GLOBAL_WATCH_ERROR;
    }
    let root = unsafe { crate::collector::zirk_rt_global_root_add(channel) };
    if root == 0 {
        return GLOBAL_WATCH_ERROR;
    }
    let mut hub = lock_hub();
    let subscription_count = hub
        .globals
        .values()
        .map(|state| state.subscriptions.len())
        .sum::<usize>();
    if subscription_count >= MAX_GLOBAL_WATCHES {
        crate::collector::zirk_rt_global_root_remove(root);
        return GLOBAL_WATCH_ERROR;
    }
    let Some(token) = allocate_token(&NEXT_TOKEN) else {
        crate::collector::zirk_rt_global_root_remove(root);
        return GLOBAL_WATCH_ERROR;
    };
    let revision = {
        let state = hub.globals.entry(global as usize).or_default();
        state.subscriptions.insert(
            token,
            Subscription {
                channel: channel as usize,
                root,
                lagged: false,
                lagged_reported: false,
            },
        );
        state.revision
    };
    hub.tokens.insert(token, global as usize);
    unsafe {
        token_out.write(token);
        revision_out.write(revision);
    }
    GLOBAL_WATCH_RECEIVED
}

/// Publishes one already-compared change while the caller holds the global
/// cell lock. The hub increments the revision once, writes it through
/// `revision_out`, and copies the event bytes into each bounded channel.
/// `changed == false` leaves the revision and every subscription untouched.
///
/// # Safety
/// `global` must be a live global cell. `event` must point to `event_size`
/// initialized bytes, and `revision_out` must point to a writable `u64` field
/// in the event object or its backing storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_global_watch_publish(
    global: *mut c_void,
    event: *const u8,
    event_size: usize,
    revision_out: *mut u64,
    changed: bool,
) -> u64 {
    if global.is_null() || event.is_null() || revision_out.is_null() || event_size == 0 || !changed
    {
        return 0;
    }
    let mut hub = lock_hub();
    let Some(state) = hub.globals.get_mut(&(global as usize)) else {
        return 0;
    };
    if state.subscriptions.is_empty() {
        return state.revision;
    }
    let Some(revision) = state.revision.checked_add(1) else {
        return 0;
    };
    state.revision = revision;
    unsafe { revision_out.write(revision) };
    let bytes = unsafe { std::slice::from_raw_parts(event, event_size) }.to_vec();
    for subscription in state.subscriptions.values_mut() {
        if subscription.lagged {
            continue;
        }
        let result = unsafe {
            crate::channel::zirk_rt_chan_try_send(
                subscription.channel as *mut c_void,
                bytes.as_ptr(),
            )
        };
        if result == crate::channel::CHAN_FULL {
            subscription.lagged = true;
            unsafe { crate::channel::zirk_rt_chan_close(subscription.channel as *mut c_void) };
        } else if result != crate::channel::CHAN_SENT {
            subscription.lagged = true;
            unsafe { crate::channel::zirk_rt_chan_close(subscription.channel as *mut c_void) };
        }
    }
    revision
}

/// Reads one event without blocking. Buffered events drain before a terminal
/// lag error is returned.
///
/// # Safety
/// `destination` must have enough writable bytes for the channel element.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_global_watch_next(token: u64, destination: *mut u8) -> i32 {
    if destination.is_null() || token == 0 {
        return GLOBAL_WATCH_ERROR;
    }
    let mut hub = lock_hub();
    let Some(&global) = hub.tokens.get(&token) else {
        return GLOBAL_WATCH_CLOSED;
    };
    let Some(state) = hub.globals.get_mut(&global) else {
        return GLOBAL_WATCH_CLOSED;
    };
    let Some(subscription) = state.subscriptions.get_mut(&token) else {
        return GLOBAL_WATCH_CLOSED;
    };
    let result = unsafe {
        crate::channel::zirk_rt_chan_try_recv(subscription.channel as *mut c_void, destination)
    };
    if result == crate::channel::CHAN_RECEIVED {
        return GLOBAL_WATCH_RECEIVED;
    }
    if subscription.lagged {
        if subscription.lagged_reported {
            return GLOBAL_WATCH_CLOSED;
        }
        subscription.lagged_reported = true;
        return GLOBAL_WATCH_LAGGED;
    }
    if result == crate::channel::CHAN_EMPTY {
        GLOBAL_WATCH_EMPTY
    } else {
        GLOBAL_WATCH_CLOSED
    }
}

/// Removes a subscription and releases its channel root. Closing is
/// idempotent from the language API's perspective.
#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_global_watch_close(token: u64) -> bool {
    if token == 0 {
        return false;
    }
    let mut hub = lock_hub();
    let Some(&global) = hub.tokens.get(&token) else {
        return false;
    };
    remove_subscription(&mut hub, global, token)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[repr(C)]
    #[derive(Clone, Copy, Debug)]
    struct Event {
        previous: u64,
        current: u64,
        revision: u64,
    }

    #[test]
    fn subscription_tokens_do_not_wrap_and_reuse_live_handles() {
        let counter = AtomicU64::new(u64::MAX);
        assert_eq!(allocate_token(&counter), None);
        assert_eq!(counter.load(Ordering::Relaxed), u64::MAX);

        let counter = AtomicU64::new(1);
        assert_eq!(allocate_token(&counter), Some(1));
        assert_eq!(allocate_token(&counter), Some(2));
        assert_eq!(counter.load(Ordering::Relaxed), 3);
    }

    unsafe fn event_channel(capacity: usize) -> *mut c_void {
        unsafe {
            crate::channel::zirk_rt_chan_new(
                capacity,
                0,
                std::mem::size_of::<Event>(),
                std::mem::align_of::<u64>(),
                std::ptr::null(),
                0,
            )
        }
    }

    fn event(previous: u64, current: u64) -> Event {
        Event {
            previous,
            current,
            revision: 0,
        }
    }

    #[test]
    fn subscriptions_start_at_current_revision_and_receive_ordered_changes() {
        let global = Box::into_raw(Box::new(0_u8)).cast::<c_void>();
        let channel = unsafe { event_channel(4) };
        unsafe { zirk_rt_global_watch_reset(global) };
        let mut token = 0;
        let mut initial_revision = u64::MAX;
        assert_eq!(
            unsafe {
                zirk_rt_global_watch_subscribe(global, channel, &mut token, &mut initial_revision)
            },
            GLOBAL_WATCH_RECEIVED
        );
        assert_eq!(initial_revision, 0);

        let mut first = event(1, 2);
        assert_eq!(
            unsafe {
                zirk_rt_global_watch_publish(
                    global,
                    (&first as *const Event).cast(),
                    std::mem::size_of::<Event>(),
                    &mut first.revision,
                    false,
                )
            },
            0
        );

        let mut first = event(1, 2);
        let mut second = event(2, 3);
        assert_eq!(
            unsafe {
                zirk_rt_global_watch_publish(
                    global,
                    (&first as *const Event).cast(),
                    std::mem::size_of::<Event>(),
                    &mut first.revision,
                    true,
                )
            },
            1
        );
        assert_eq!(
            unsafe {
                zirk_rt_global_watch_publish(
                    global,
                    (&second as *const Event).cast(),
                    std::mem::size_of::<Event>(),
                    &mut second.revision,
                    true,
                )
            },
            2
        );
        assert_eq!(first.revision, 1);
        assert_eq!(second.revision, 2);

        let mut received = std::mem::MaybeUninit::<Event>::uninit();
        assert_eq!(
            unsafe { zirk_rt_global_watch_next(token, received.as_mut_ptr().cast()) },
            GLOBAL_WATCH_RECEIVED
        );
        assert_eq!(unsafe { received.assume_init() }.current, 2);
        received = std::mem::MaybeUninit::<Event>::uninit();
        assert_eq!(
            unsafe { zirk_rt_global_watch_next(token, received.as_mut_ptr().cast()) },
            GLOBAL_WATCH_RECEIVED
        );
        assert_eq!(unsafe { received.assume_init() }.current, 3);
        assert_eq!(
            unsafe { zirk_rt_global_watch_next(token, received.as_mut_ptr().cast()) },
            GLOBAL_WATCH_EMPTY
        );
        assert!(zirk_rt_global_watch_close(token));
        assert_eq!(
            unsafe { zirk_rt_global_watch_next(token, received.as_mut_ptr().cast()) },
            GLOBAL_WATCH_CLOSED
        );
        unsafe { zirk_rt_global_watch_reset(global) };
        unsafe { drop(Box::from_raw(global.cast::<u8>())) };
    }

    #[test]
    fn a_slow_subscription_gets_one_terminal_lag_error() {
        let global = Box::into_raw(Box::new(0_u8)).cast::<c_void>();
        let channel = unsafe { event_channel(1) };
        unsafe { zirk_rt_global_watch_reset(global) };
        let mut token = 0;
        let mut revision = 0;
        assert_eq!(
            unsafe { zirk_rt_global_watch_subscribe(global, channel, &mut token, &mut revision) },
            GLOBAL_WATCH_RECEIVED
        );

        let mut first = event(0, 1);
        let mut second = event(1, 2);
        assert_eq!(
            unsafe {
                zirk_rt_global_watch_publish(
                    global,
                    (&first as *const Event).cast(),
                    std::mem::size_of::<Event>(),
                    &mut first.revision,
                    true,
                )
            },
            1
        );
        assert_eq!(
            unsafe {
                zirk_rt_global_watch_publish(
                    global,
                    (&second as *const Event).cast(),
                    std::mem::size_of::<Event>(),
                    &mut second.revision,
                    true,
                )
            },
            2
        );

        let mut received = std::mem::MaybeUninit::<Event>::uninit();
        assert_eq!(
            unsafe { zirk_rt_global_watch_next(token, received.as_mut_ptr().cast()) },
            GLOBAL_WATCH_RECEIVED
        );
        assert_eq!(
            unsafe { zirk_rt_global_watch_next(token, received.as_mut_ptr().cast()) },
            GLOBAL_WATCH_LAGGED
        );
        assert_eq!(
            unsafe { zirk_rt_global_watch_next(token, received.as_mut_ptr().cast()) },
            GLOBAL_WATCH_CLOSED
        );
        assert!(zirk_rt_global_watch_close(token));
        unsafe { zirk_rt_global_watch_reset(global) };
        unsafe { drop(Box::from_raw(global.cast::<u8>())) };
    }

    #[test]
    fn resetting_a_global_closes_and_releases_active_subscriptions() {
        let global = Box::into_raw(Box::new(0_u8)).cast::<c_void>();
        let channel = unsafe { event_channel(2) };
        unsafe { zirk_rt_global_watch_reset(global) };
        let mut token = 0;
        let mut revision = 0;
        assert_eq!(
            unsafe { zirk_rt_global_watch_subscribe(global, channel, &mut token, &mut revision) },
            GLOBAL_WATCH_RECEIVED
        );

        unsafe { zirk_rt_global_watch_reset(global) };

        let mut received = std::mem::MaybeUninit::<Event>::uninit();
        assert_eq!(
            unsafe { zirk_rt_global_watch_next(token, received.as_mut_ptr().cast()) },
            GLOBAL_WATCH_CLOSED
        );
        assert!(!zirk_rt_global_watch_close(token));
        unsafe { drop(Box::from_raw(global.cast::<u8>())) };
    }
}
