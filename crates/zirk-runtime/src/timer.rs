//! The executor's monotonic timer service: a min-heap of deadlines the loop
//! consults once per scheduling turn.
//!
//! Backs the timer API introduced by the pending concurrency-surface changes.
//! Uses [`std::time::Instant`] — a wall-clock jump must not disturb scheduling,
//! so this is deliberately *not* the `SystemTime` clock the temporal `now_*`
//! helpers use.
//!
//! Cancellation is lazy: [`disarm`](TimerService::disarm) records the id and the
//! entry is dropped when it reaches the top of the heap. Design:
//! `openspec/changes/fase-5-executor-core/design.md` D6.

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashSet};
use std::time::{Duration as StdDuration, Instant};

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
    cancelled: HashSet<TimerId>,
    next_id: u64,
    /// Live (armed, not yet fired or cancelled) timer count.
    live: usize,
}

impl TimerService {
    pub fn new() -> Self {
        Self::default()
    }

    /// Arms a timer `delay_nanos` from now with the given wake `payload`.
    /// A negative delay is rejected before anything is scheduled.
    pub fn arm(&mut self, delay_nanos: i64, payload: u64) -> Result<TimerId, NegativeDuration> {
        let delay = u64::try_from(delay_nanos).map_err(|_| NegativeDuration)?;
        let id = TimerId(self.next_id);
        self.next_id += 1;
        self.heap.push(Entry {
            deadline: Instant::now() + StdDuration::from_nanos(delay),
            id,
            payload,
        });
        self.live += 1;
        Ok(id)
    }

    /// Cancels an armed timer. A no-op for an unknown, already-fired, or
    /// already-cancelled id.
    pub fn disarm(&mut self, id: TimerId) {
        if id.0 < self.next_id && self.cancelled.insert(id) {
            // Only counts against `live` if it has not already fired. We cannot
            // tell cheaply here, so `live` is corrected lazily in `drain_top`.
            self.live = self.live.saturating_sub(1);
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
                    self.live = self.live.saturating_sub(1);
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
        assert_eq!(timers.arm(-1, 0), Err(NegativeDuration));
        assert!(timers.is_empty());
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
    fn peek_deadline_bounds_the_idle_sleep() {
        let mut timers = TimerService::new();
        assert!(timers.peek_deadline().is_none());
        let before = Instant::now();
        timers.arm(10_000_000, 0).unwrap();
        let deadline = timers.peek_deadline().unwrap();
        assert!(deadline > before);
    }
}
