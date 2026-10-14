//! The executor's monotonic timer service: a min-heap of deadlines the loop
//! consults once per scheduling turn.
//!
//! Backs the timer API introduced by the pending concurrency-surface changes.
//! Uses [`std::time::Instant`] — a wall-clock jump must not disturb scheduling,
//! so this is deliberately *not* the `SystemTime` clock the temporal `now_*`
//! helpers use.
//!
//! Cancellation is lazy: [`disarm`](TimerService::disarm) records the id and the
//! entry is dropped when it reaches the top of the heap. Periodic compaction
//! bounds retained cancellation tombstones relative to live timers. Design:
//! `openspec/changes/fase-5-executor-core/design.md` D6.

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashSet};
use std::time::{Duration as StdDuration, Instant};

/// Maximum number of active timers per executor.
pub const MAX_ACTIVE_TIMERS: usize = 4096;

/// Identifies one armed timer. Never reused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TimerId(u64);

impl TimerId {
    /// The raw value, for stashing in a `WaitReason` (so a cancelled sleep can
    /// disarm its timer later).
    pub fn to_bits(self) -> u64 {
        self.0
    }

    /// Inverse of [`to_bits`](Self::to_bits).
    pub fn from_bits(bits: u64) -> Self {
        TimerId(bits)
    }
}

/// Raised when a caller asks to wait for a negative duration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NegativeDuration;

/// Why a timer could not be armed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerError {
    NegativeDuration,
    CapacityExceeded,
}

struct Entry {
    deadline: Instant,
    id: TimerId,
    /// Opaque wake payload — the executor stores the waiting task's id bits.
    payload: u64,
}

// A max-heap by default; order so the *earliest* deadline is the greatest, then
// break ties by id for determinism.
impl Ord for Entry {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .deadline
            .cmp(&self.deadline)
            .then_with(|| other.id.0.cmp(&self.id.0))
    }
}
impl PartialOrd for Entry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl PartialEq for Entry {
    fn eq(&self, other: &Self) -> bool {
        self.deadline == other.deadline && self.id == other.id
    }
}
impl Eq for Entry {}

/// A heap of pending timers.
#[derive(Default)]
pub struct TimerService {
    heap: BinaryHeap<Entry>,
    active: HashSet<TimerId>,
    /// Reservations held by active Timer.after/every jobs.
    reserved: usize,
    /// Active heap entries currently using one of those reservations.
    active_reserved: HashSet<TimerId>,
    cancelled: HashSet<TimerId>,
    next_id: u64,
    /// Live (armed, not yet fired or cancelled) timer count.
    live: usize,
}

impl TimerService {
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of armed timers, excluding cancelled heap tombstones.
    pub(crate) fn active_registrations(&self) -> usize {
        self.active.len()
    }

    fn occupied_capacity(&self) -> usize {
        self.active
            .len()
            .saturating_sub(self.active_reserved.len())
            .saturating_add(self.reserved)
    }

    /// Reserves one timer slot for a Timer.after/every job before the job is
    /// published to the executor's ready queue.
    pub(crate) fn reserve(&mut self) -> Result<(), TimerError> {
        if self.occupied_capacity() >= MAX_ACTIVE_TIMERS {
            return Err(TimerError::CapacityExceeded);
        }
        self.reserved += 1;
        Ok(())
    }

    /// Releases a timer-job reservation at terminal completion.
    pub(crate) fn release_reservation(&mut self) {
        self.reserved = self
            .reserved
            .checked_sub(1)
            .expect("timer reservation must be released exactly once");
    }

    /// Arms a timer `delay_nanos` from now with the given wake `payload`.
    /// A negative delay is rejected before anything is scheduled.
    pub fn try_arm(&mut self, delay_nanos: i64, payload: u64) -> Result<TimerId, TimerError> {
        let delay = u64::try_from(delay_nanos).map_err(|_| TimerError::NegativeDuration)?;
        if self.occupied_capacity() >= MAX_ACTIVE_TIMERS {
            return Err(TimerError::CapacityExceeded);
        }
        self.arm_validated(delay, payload, false)
    }

    /// Arms a timer using a reservation already owned by an active timer job.
    pub(crate) fn arm_reserved(
        &mut self,
        delay_nanos: i64,
        payload: u64,
    ) -> Result<TimerId, TimerError> {
        let delay = u64::try_from(delay_nanos).map_err(|_| TimerError::NegativeDuration)?;
        if self.reserved == 0 {
            return Err(TimerError::CapacityExceeded);
        }
        self.arm_validated(delay, payload, true)
    }

    fn arm_validated(
        &mut self,
        delay: u64,
        payload: u64,
        reserved: bool,
    ) -> Result<TimerId, TimerError> {
        let id = TimerId(self.next_id);
        self.next_id += 1;
        self.heap.push(Entry {
            deadline: Instant::now() + StdDuration::from_nanos(delay),
            id,
            payload,
        });
        self.active.insert(id);
        if reserved {
            self.active_reserved.insert(id);
        }
        self.live += 1;
        Ok(id)
    }

    pub fn arm(&mut self, delay_nanos: i64, payload: u64) -> Result<TimerId, TimerError> {
        self.try_arm(delay_nanos, payload)
    }

    /// Cancels an armed timer. A no-op for an unknown, already-fired, or
    /// already-cancelled id.
    pub fn disarm(&mut self, id: TimerId) {
        if self.active.remove(&id) {
            self.active_reserved.remove(&id);
            self.cancelled.insert(id);
            self.live -= 1;
            self.compact_cancelled_if_needed();
        }
    }

    /// Keeps lazy-cancellation storage bounded even when many far-future
    /// timers are repeatedly armed and cancelled. Compaction is amortized:
    /// retain at most roughly twice the live heap plus 64 tombstones.
    fn compact_cancelled_if_needed(&mut self) {
        let threshold = self.live.saturating_mul(2).max(64);
        if self.heap.len() > threshold {
            self.heap.retain(|entry| self.active.contains(&entry.id));
            self.cancelled.clear();
        }
    }

    /// Whether any timer could still fire.
    pub fn is_empty(&self) -> bool {
        self.live == 0
    }

    /// The earliest live deadline, if any — the bound for an idle executor's
    /// sleep. Drops cancelled entries it uncovers.
    pub fn peek_deadline(&mut self) -> Option<Instant> {
        self.drop_cancelled_top();
        self.heap.peek().map(|e| e.deadline)
    }

    /// Every timer whose deadline is at or before `now`, earliest first.
    /// Cancelled entries are silently discarded.
    pub fn poll_expired(&mut self, now: Instant) -> Vec<(TimerId, u64)> {
        let mut fired = Vec::new();
        loop {
            self.drop_cancelled_top();
            match self.heap.peek() {
                Some(entry) if entry.deadline <= now => {
                    let entry = self.heap.pop().expect("peeked");
                    if self.active.remove(&entry.id) {
                        self.active_reserved.remove(&entry.id);
                        self.live -= 1;
                    }
                    fired.push((entry.id, entry.payload));
                }
                _ => break,
            }
        }
        fired
    }

    /// Discards any cancelled entries sitting at the top of the heap.
    fn drop_cancelled_top(&mut self) {
        while let Some(entry) = self.heap.peek() {
            if self.cancelled.remove(&entry.id) {
                self.heap.pop();
            } else {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn negative_delay_is_rejected_before_scheduling() {
        let mut timers = TimerService::new();
        assert_eq!(timers.try_arm(-1, 0), Err(TimerError::NegativeDuration));
        assert!(timers.is_empty());
    }

    #[test]
    fn active_timer_quota_is_exact_and_disarm_and_expiry_release_once() {
        let mut timers = TimerService::new();
        let ids: Vec<_> = (0..MAX_ACTIVE_TIMERS)
            .map(|payload| timers.try_arm(60_000_000_000, payload as u64).unwrap())
            .collect();
        assert_eq!(timers.active_registrations(), MAX_ACTIVE_TIMERS);
        assert_eq!(timers.try_arm(1, 0), Err(TimerError::CapacityExceeded));

        timers.disarm(ids[0]);
        timers.disarm(ids[0]);
        let _replacement = timers.try_arm(60_000_000_000, u64::MAX).unwrap();
        assert_eq!(timers.active_registrations(), MAX_ACTIVE_TIMERS);

        let fired = timers.poll_expired(Instant::now() + StdDuration::from_secs(61));
        assert_eq!(fired.len(), MAX_ACTIVE_TIMERS);
        assert_eq!(timers.active_registrations(), 0);
        assert!(timers.try_arm(0, 7).is_ok());
    }

    #[test]
    fn timer_job_reservation_is_atomic_and_reusable_for_periodic_arms() {
        let mut timers = TimerService::new();
        for _ in 0..MAX_ACTIVE_TIMERS {
            timers.reserve().unwrap();
        }
        assert_eq!(timers.reserve(), Err(TimerError::CapacityExceeded));
        assert_eq!(
            timers.try_arm(0, 7),
            Err(TimerError::CapacityExceeded),
            "unreserved sleeps cannot consume slots held by active Timer jobs"
        );

        let first = timers.arm_reserved(0, 1).unwrap();
        assert_eq!(timers.poll_expired(Instant::now()), vec![(first, 1)]);
        let second = timers.arm_reserved(0, 2).unwrap();
        timers.disarm(second);
        timers.release_reservation();
        assert_eq!(timers.occupied_capacity(), MAX_ACTIVE_TIMERS - 1);
        assert!(timers.try_arm(0, 3).is_ok());
    }

    #[test]
    fn timers_fire_in_deadline_order() {
        let mut timers = TimerService::new();
        let far = timers.arm(50_000_000, 111).unwrap();
        let near = timers.arm(1_000_000, 222).unwrap();
        assert!(!timers.is_empty());

        // Nothing is due immediately.
        assert!(timers.poll_expired(Instant::now()).is_empty());

        std::thread::sleep(StdDuration::from_millis(5));
        let fired = timers.poll_expired(Instant::now());
        assert_eq!(fired, vec![(near, 222)]);
        assert!(!timers.is_empty(), "the far timer is still pending");
        let _ = far;
    }

    #[test]
    fn disarm_before_fire_is_honored() {
        let mut timers = TimerService::new();
        let a = timers.arm(1_000_000, 1).unwrap();
        let b = timers.arm(1_000_000, 2).unwrap();
        timers.disarm(a);
        assert!(!timers.is_empty());

        std::thread::sleep(StdDuration::from_millis(5));
        let fired = timers.poll_expired(Instant::now());
        assert_eq!(fired, vec![(b, 2)], "the disarmed timer must not fire");
        assert!(timers.is_empty());
    }

    #[test]
    fn disarm_of_unknown_or_fired_id_is_a_noop() {
        let mut timers = TimerService::new();
        timers.disarm(TimerId(999)); // never armed
        let a = timers.arm(0, 7).unwrap();
        std::thread::sleep(StdDuration::from_millis(1));
        assert_eq!(timers.poll_expired(Instant::now()), vec![(a, 7)]);
        timers.disarm(a); // already fired
        assert!(timers.is_empty());
    }

    #[test]
    fn disarming_a_fired_timer_does_not_hide_another_live_timer() {
        let mut timers = TimerService::new();
        let fired = timers.arm(0, 1).unwrap();
        let pending = timers.arm(60_000_000_000, 2).unwrap();
        assert_eq!(timers.poll_expired(Instant::now()), vec![(fired, 1)]);
        assert!(!timers.is_empty());
        assert_eq!(timers.active_registrations(), 1);

        timers.disarm(fired);
        assert!(!timers.is_empty());
        assert!(timers.peek_deadline().is_some());

        timers.disarm(pending);
        assert!(timers.is_empty());
        assert_eq!(timers.active_registrations(), 0);
        assert!(
            timers
                .poll_expired(Instant::now() + StdDuration::from_secs(61))
                .is_empty()
        );
    }

    #[test]
    fn mass_cancel_and_expiry_returns_timer_accounting_to_zero() {
        let mut timers = TimerService::new();
        let mut expected = Vec::new();
        for payload in 0..1024_u64 {
            let id = timers.arm(0, payload).unwrap();
            if payload % 2 == 0 {
                timers.disarm(id);
            } else {
                expected.push((id, payload));
            }
        }
        assert_eq!(timers.live, 512);
        assert_eq!(timers.active_registrations(), timers.live);

        let fired = timers.poll_expired(Instant::now() + StdDuration::from_secs(1));
        assert_eq!(fired, expected);
        assert!(timers.is_empty());
        assert!(timers.active.is_empty());
        assert!(timers.cancelled.is_empty());
        assert!(timers.heap.is_empty());
        assert_eq!(timers.active_registrations(), 0);
    }

    #[test]
    fn far_future_cancellation_compacts_tombstones_against_live_timer_count() {
        let mut timers = TimerService::new();
        let mut ids = Vec::new();
        for payload in 0..4096_u64 {
            ids.push(timers.arm(60_000_000_000, payload).unwrap());
        }

        for id in ids {
            timers.disarm(id);
        }

        assert_eq!(timers.active_registrations(), 0);
        assert!(timers.heap.len() <= 64);
        assert!(timers.cancelled.len() <= 64);
    }

    #[test]
    fn registrations_return_to_baseline_after_fire_cancel_and_compaction() {
        let mut timers = TimerService::new();
        let baseline = timers.arm(60_000_000_000, u64::MAX).unwrap();
        assert_eq!(timers.active_registrations(), 1);

        let mut cancelled = Vec::new();
        for payload in 0..(MAX_ACTIVE_TIMERS as u64 - 1) {
            cancelled.push(timers.arm(60_000_000_000, payload).unwrap());
        }
        for id in cancelled {
            timers.disarm(id);
        }

        assert_eq!(timers.active_registrations(), 1);
        assert_eq!(timers.live, 1);
        assert!(timers.heap.len() <= 66);

        // Fire the sole remaining live registration without sleeping. Cancelled
        // tombstones are drained but must never be reported as fired timers.
        assert_eq!(
            timers.poll_expired(Instant::now() + StdDuration::from_secs(61)),
            vec![(baseline, u64::MAX)]
        );
        assert_eq!(timers.active_registrations(), 0);
        assert!(timers.is_empty());
        assert!(timers.heap.is_empty());
    }

    #[test]
    fn peek_deadline_bounds_the_idle_sleep() {
        let mut timers = TimerService::new();
        assert!(timers.peek_deadline().is_none());
        let before = Instant::now();
        timers.arm(10_000_000, 0).unwrap();
        let deadline = timers.peek_deadline().unwrap();
        assert!(deadline > before);
    }
}
