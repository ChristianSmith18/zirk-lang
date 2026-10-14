//! Scheduler-backed building blocks for structured concurrency combinators.
//!
//! These APIs require an active executor task. They suspend through executor
//! wait queues and timers; they never join OS threads or poll while keeping the
//! executor runnable.
//!
//! This is a native Rust API only. It does not register closure captures or
//! returned values as Zirk GC roots. Do not pass managed Zirk values, heap
//! pointers, or handles through these APIs; only unmanaged Rust values with
//! valid `Send` ownership are supported until a rooted ABI is implemented.

use std::any::Any;
use std::cell::Cell;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::mpsc::{Receiver, TryRecvError, sync_channel};
use std::time::Duration;

use crate::executor::{self, Cancelled};
use crate::task::TaskId;

/// Maximum branches admitted by one combinator invocation.
pub const MAX_CONCURRENT_BRANCHES: usize = 256;
/// Default active callback window for the source-level `Concurrent.each*` API.
pub const DEFAULT_EACH_CONCURRENCY: usize = 32;

/// The terminal result of one concurrent branch.
#[derive(Debug)]
pub enum Outcome<T> {
    /// The branch returned normally.
    Fulfilled(T),
    /// An unhandled panic/throwable escaped the branch; its original payload
    /// remains available for propagation or downcasting.
    Rejected(TaskFailure),
    /// The executor delivered cooperative cancellation to the branch.
    Cancelled(CancelledError),
}

/// A captured panic/throwable payload from a failed task.
pub struct TaskFailure {
    payload: Box<dyn Any + Send + 'static>,
    suppressed: Vec<TaskFailure>,
}

impl TaskFailure {
    /// Creates a failure from a panic payload. Primarily useful to runtime
    /// adapters that translate native errors into an unhandled failure.
    pub fn new(payload: Box<dyn Any + Send + 'static>) -> Self {
        Self {
            payload,
            suppressed: Vec::new(),
        }
    }

    /// Returns a readable message for common string panic payloads.
    pub fn message(&self) -> &str {
        self.payload
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| self.payload.downcast_ref::<&'static str>().copied())
            .unwrap_or("task failed with a non-string payload")
    }

    /// Returns the original payload for propagation through an outer task.
    pub fn into_payload(self) -> Box<dyn Any + Send + 'static> {
        self.payload
    }

    /// Borrows the original panic payload for a typed downcast.
    pub fn downcast_ref<T: Any>(&self) -> Option<&T> {
        self.payload.downcast_ref::<T>()
    }

    /// Returns failures from sibling branches that completed before cleanup
    /// finished, in executor completion order.
    pub fn suppressed(&self) -> &[TaskFailure] {
        &self.suppressed
    }

    fn suppress(&mut self, failure: TaskFailure) {
        self.suppressed.push(failure);
    }
}

impl std::fmt::Debug for TaskFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TaskFailure")
            .field("message", &self.message())
            .field("suppressed", &self.suppressed)
            .finish()
    }
}

/// The runtime representation of a cancelled branch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CancelledError;

/// Errors returned by a bounded combinator or timer-backed operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WithinError {
    /// The operation exceeds [`MAX_CONCURRENT_BRANCHES`].
    TooManyBranches,
    /// Nanoseconds exceed the signed timer ABI range.
    DurationOutOfRange,
    /// The deadline expired; all branches were cancelled and joined.
    Timeout,
}

#[derive(Debug)]
pub enum EachError {
    /// The rejected branch's original payload; siblings were cancelled/joined.
    BranchFailed(TaskFailure),
    /// The operation was cancelled; siblings were cancelled/joined.
    Cancelled,
    /// The input exceeds [`MAX_CONCURRENT_BRANCHES`].
    TooManyBranches,
    /// The explicit concurrency limit is outside `1..=MAX_CONCURRENT_BRANCHES`.
    InvalidConcurrencyLimit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CombinatorError {
    /// The operation exceeds [`MAX_CONCURRENT_BRANCHES`].
    TooManyBranches,
    /// The explicit concurrency limit is outside `1..=MAX_CONCURRENT_BRANCHES`.
    InvalidConcurrencyLimit,
}

struct Join<T> {
    id: TaskId,
    receiver: Receiver<Outcome<T>>,
    joined: Cell<bool>,
}

fn spawn_one<T, F>(body: F) -> Join<T>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    let (sender, receiver) = sync_channel(1);
    let id = executor::spawn(move || {
        let result = catch_unwind(AssertUnwindSafe(body));
        let outcome = match result {
            Ok(value) if executor::current_task_is_cancelled() => {
                drop(value);
                Outcome::Cancelled(CancelledError)
            }
            Ok(value) => Outcome::Fulfilled(value),
            Err(payload) if payload.is::<Cancelled>() => Outcome::Cancelled(CancelledError),
            Err(payload) => Outcome::Rejected(TaskFailure::new(payload)),
        };
        // Capacity one and exactly one send make this non-waiting for a
        // receiver. The receiver remains alive until the task has been joined.
        let _ = sender.send(outcome);
        0
    });
    Join {
        id,
        receiver,
        joined: Cell::new(false),
    }
}

fn collect_bounded<T>(items: impl IntoIterator<Item = T>) -> Result<Vec<T>, CombinatorError> {
    let items = items
        .into_iter()
        .take(MAX_CONCURRENT_BRANCHES + 1)
        .collect::<Vec<_>>();
    if items.len() > MAX_CONCURRENT_BRANCHES {
        Err(CombinatorError::TooManyBranches)
    } else {
        Ok(items)
    }
}

fn take_outcome<T>(join: &Join<T>) -> Outcome<T> {
    match join.receiver.try_recv() {
        Ok(outcome) => outcome,
        Err(TryRecvError::Disconnected) => Outcome::Cancelled(CancelledError),
        Err(TryRecvError::Empty) => {
            // The task may have been cancelled by the executor before it could
            // enter its wrapper and send an outcome.
            Outcome::Cancelled(CancelledError)
        }
    }
}

fn join_one<T>(join: &Join<T>) -> Outcome<T> {
    assert!(
        !join.joined.replace(true),
        "task result joined more than once"
    );
    let joined = catch_unwind(AssertUnwindSafe(|| executor::await_task(join.id)));
    match joined {
        Ok(_) => take_outcome(join),
        Err(payload) if payload.is::<Cancelled>() => take_outcome(join),
        Err(payload) => Outcome::Rejected(TaskFailure::new(payload)),
    }
}

fn wait_all_uninterruptibly(ids: &[TaskId]) {
    loop {
        let pending = ids
            .iter()
            .copied()
            .filter(|id| !executor::is_done(*id))
            .collect::<Vec<_>>();
        if pending.is_empty() {
            return;
        }
        executor::wait_any_current(&pending);
    }
}

/// If the caller is cancelled, cancels the full set, waits inside the runtime
/// cancellation shield for cooperative cleanup, then lets the caller propagate
/// cancellation after all task outcomes have been consumed.
fn wait_until_all(ids: &[TaskId]) -> bool {
    loop {
        if executor::current_task_cancel_requested() {
            for id in ids {
                if !executor::is_done(*id) {
                    executor::cancel_current(*id);
                }
            }
            executor::with_cancellation_shield(|| wait_all_uninterruptibly(ids));
            return true;
        }
        let pending = ids
            .iter()
            .copied()
            .filter(|id| !executor::is_done(*id))
            .collect::<Vec<_>>();
        if pending.is_empty() {
            return false;
        }
        executor::wait_any_current(&pending);
    }
}

fn propagate_cancellation() -> ! {
    std::panic::resume_unwind(Box::new(Cancelled));
}

fn cancel_unfinished<'a, T: 'a>(joins: impl IntoIterator<Item = &'a Join<T>>) {
    for join in joins {
        if !join.joined.get() && !executor::is_done(join.id) {
            executor::cancel_current(join.id);
        }
    }
}

fn unjoined_ids<T>(joins: &[Join<T>]) -> Vec<TaskId> {
    joins
        .iter()
        .filter(|join| !join.joined.get())
        .map(|join| join.id)
        .collect()
}

/// For `first`, cancelled branches do not win while another branch is still
/// active. If every branch is cancelled, the earliest cancellation is returned.
/// With a timer, the earliest non-cancelled branch completion competes against
/// the timer; all-cancelled wins only if it precedes the deadline.
fn wait_for_first(branches: &[TaskId], timer: Option<TaskId>) -> Result<usize, ()> {
    loop {
        if executor::current_task_cancel_requested() {
            return Err(());
        }
        let earliest = branches
            .iter()
            .enumerate()
            .filter_map(|(index, id)| {
                let sequence = executor::completion_sequence(*id)?;
                (!executor::task_was_cancelled(*id)).then_some((sequence, index))
            })
            .min();
        let timer_event =
            timer.and_then(|id| executor::completion_sequence(id).map(|s| (s, branches.len())));
        if let Some(candidate) = earliest
            && timer_event.is_none_or(|event| candidate < event)
        {
            return Ok(candidate.1);
        }
        let all_branches_done = branches
            .iter()
            .all(|id| executor::completion_sequence(*id).is_some());
        if all_branches_done {
            let earliest_cancelled = branches
                .iter()
                .enumerate()
                .filter_map(|(index, id)| {
                    let sequence = executor::completion_sequence(*id)?;
                    executor::task_was_cancelled(*id).then_some((sequence, index))
                })
                .min();
            if let Some((sequence, index)) = earliest_cancelled
                && timer_event.is_none_or(|event| (sequence, index) < event)
            {
                return Ok(index);
            }
        }
        if let Some((_, index)) = timer_event {
            return Ok(index);
        }
        let mut pending = branches
            .iter()
            .copied()
            .filter(|id| executor::completion_sequence(*id).is_none())
            .collect::<Vec<_>>();
        if let Some(timer) = timer.filter(|id| executor::completion_sequence(*id).is_none()) {
            pending.push(timer);
        }
        if pending.is_empty() {
            unreachable!("completed branches are handled above");
        }
        executor::wait_any_current(&pending);
    }
}

/// Runs every branch and returns outcomes in input order, regardless of
/// completion order. All branches are joined before this function returns.
///
/// This API carries native Rust values only; it does not register Zirk GC roots.
pub fn settled<T, F>(
    branches: impl IntoIterator<Item = F>,
) -> Result<Vec<Outcome<T>>, CombinatorError>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    assert!(
        executor::in_task_context(),
        "settled outside an executor task"
    );
    let branches = collect_bounded(branches)?;
    let joins = branches.into_iter().map(spawn_one).collect::<Vec<_>>();
    let ids = joins.iter().map(|join| join.id).collect::<Vec<_>>();
    let cancelled = wait_until_all(&ids);
    let outcomes = joins.iter().map(join_one).collect::<Vec<_>>();
    if cancelled {
        drop(outcomes);
        propagate_cancellation();
    }
    Ok(outcomes)
}

/// Returns the first successful or rejected branch. A cancelled branch does
/// not win while any branch remains active; if all branches are cancelled, the
/// earliest cancellation is returned. Remaining branches are cooperatively
/// cancelled and joined before returning.
///
/// This API carries native Rust values only; it does not register Zirk GC roots.
pub fn first<T, F>(
    branches: impl IntoIterator<Item = F>,
) -> Result<Option<Outcome<T>>, CombinatorError>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    assert!(
        executor::in_task_context(),
        "first outside an executor task"
    );
    let branches = collect_bounded(branches)?;
    let joins = branches.into_iter().map(spawn_one).collect::<Vec<_>>();
    if joins.is_empty() {
        return Ok(None);
    }
    let winner = match wait_for_first(&joins.iter().map(|join| join.id).collect::<Vec<_>>(), None) {
        Ok(winner) => winner,
        Err(()) => {
            let ids = unjoined_ids(&joins);
            let _ = wait_until_all(&ids);
            for join in &joins {
                if !join.joined.get() {
                    let _ = join_one(join);
                }
            }
            propagate_cancellation();
        }
    };
    for (index, join) in joins.iter().enumerate() {
        if index != winner && !executor::is_done(join.id) {
            executor::cancel_current(join.id);
        }
    }
    let ids = joins.iter().map(|join| join.id).collect::<Vec<_>>();
    let cancelled = wait_until_all(&ids);
    let outcomes = joins.iter().map(join_one).collect::<Vec<_>>();
    if cancelled {
        drop(outcomes);
        propagate_cancellation();
    }
    Ok(resolve_first_outcome(winner, outcomes, &joins))
}

/// Keeps failures produced while cancelling and joining losing branches.
/// When the selected branch succeeded, the first cleanup failure becomes the
/// operation's failure instead of being silently discarded.
fn resolve_first_outcome<T>(
    winner: usize,
    outcomes: Vec<Outcome<T>>,
    joins: &[Join<T>],
) -> Option<Outcome<T>> {
    let mut winner_outcome = None;
    let mut failures = Vec::new();
    for (index, outcome) in outcomes.into_iter().enumerate() {
        match outcome {
            Outcome::Rejected(failure) if index == winner => {
                winner_outcome = Some(Outcome::Rejected(failure));
            }
            Outcome::Rejected(failure) => {
                let sequence = executor::completion_sequence(joins[index].id).unwrap_or(u64::MAX);
                failures.push((sequence, failure));
            }
            outcome if index == winner => winner_outcome = Some(outcome),
            _ => {}
        }
    }
    failures.sort_by_key(|(sequence, _)| *sequence);
    match winner_outcome {
        Some(Outcome::Rejected(mut primary)) => {
            for (_, failure) in failures {
                primary.suppress(failure);
            }
            Some(Outcome::Rejected(primary))
        }
        Some(outcome) if failures.is_empty() => Some(outcome),
        Some(_) => {
            let mut cleanup_failure = failures.remove(0).1;
            for (_, failure) in failures {
                cleanup_failure.suppress(failure);
            }
            Some(Outcome::Rejected(cleanup_failure))
        }
        None => None,
    }
}

/// Returns the first branch to finish before `timeout`. On expiry, all
/// branches are cooperatively cancelled and joined before `Timeout` returns.
///
/// This API carries native Rust values only; it does not register Zirk GC roots.
pub fn first_within<T, F>(
    branches: impl IntoIterator<Item = F>,
    timeout: Duration,
) -> Result<Option<Outcome<T>>, WithinError>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    assert!(
        executor::in_task_context(),
        "first_within outside an executor task"
    );
    let nanos = i64::try_from(timeout.as_nanos()).map_err(|_| WithinError::DurationOutOfRange)?;
    let branches = collect_bounded(branches).map_err(|_| WithinError::TooManyBranches)?;
    let joins = branches.into_iter().map(spawn_one).collect::<Vec<_>>();
    if joins.is_empty() {
        return Ok(None);
    }
    let timer = spawn_one(move || executor::sleep(nanos));
    let winner = match wait_for_first(
        &joins.iter().map(|join| join.id).collect::<Vec<_>>(),
        Some(timer.id),
    ) {
        Ok(winner) => winner,
        Err(()) => {
            let ids = unjoined_ids(&joins)
                .into_iter()
                .chain(std::iter::once(timer.id))
                .collect::<Vec<_>>();
            let cancelled = wait_until_all(&ids);
            for join in &joins {
                if !join.joined.get() {
                    let _ = join_one(join);
                }
            }
            let _ = join_one(&timer);
            if cancelled {
                propagate_cancellation();
            }
            unreachable!("wait-any cancellation must be recorded")
        }
    };
    if winner == joins.len() {
        cancel_unfinished(joins.iter());
        let ids = unjoined_ids(&joins)
            .into_iter()
            .chain(std::iter::once(timer.id))
            .collect::<Vec<_>>();
        let cancelled = wait_until_all(&ids);
        for join in &joins {
            if !join.joined.get() {
                let _ = join_one(join);
            }
        }
        let _ = join_one(&timer);
        if cancelled {
            propagate_cancellation();
        }
        return Err(WithinError::Timeout);
    }

    for (index, join) in joins.iter().enumerate() {
        if index != winner && !executor::is_done(join.id) {
            executor::cancel_current(join.id);
        }
    }
    if !executor::is_done(timer.id) {
        executor::cancel_current(timer.id);
    }
    let ids = unjoined_ids(&joins)
        .into_iter()
        .chain(std::iter::once(timer.id))
        .collect::<Vec<_>>();
    let cancelled = wait_until_all(&ids);
    let outcomes = joins.iter().map(join_one).collect::<Vec<_>>();
    let _ = join_one(&timer);
    if cancelled {
        drop(outcomes);
        propagate_cancellation();
    }
    Ok(resolve_first_outcome(winner, outcomes, &joins))
}

/// Runs all branches under a scheduler timer. On timeout, requests cooperative
/// cancellation, joins every branch's cleanup, and then returns `Timeout`.
///
/// This API carries native Rust values only; it does not register Zirk GC roots.
pub fn settled_within<T, F>(
    branches: impl IntoIterator<Item = F>,
    timeout: Duration,
) -> Result<Vec<Outcome<T>>, WithinError>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    assert!(
        executor::in_task_context(),
        "settled_within outside an executor task"
    );
    let nanos = i64::try_from(timeout.as_nanos()).map_err(|_| WithinError::DurationOutOfRange)?;
    let branches = collect_bounded(branches).map_err(|_| WithinError::TooManyBranches)?;
    let joins = branches.into_iter().map(spawn_one).collect::<Vec<_>>();
    if joins.is_empty() {
        return Ok(Vec::new());
    }
    let timer = spawn_one(move || executor::sleep(nanos));
    let mut outcomes: Vec<Option<Outcome<T>>> = (0..joins.len()).map(|_| None).collect();
    let mut pending = joins.len();
    loop {
        let active = joins
            .iter()
            .enumerate()
            .filter(|(index, _)| outcomes[*index].is_none())
            .map(|(index, join)| (join.id, Some(index)))
            .chain(std::iter::once((timer.id, None)))
            .collect::<Vec<_>>();
        let active_ids = active.iter().map(|(id, _)| *id).collect::<Vec<_>>();
        let winner = match wait_for_proxy(&active_ids) {
            Ok(winner) => winner,
            Err(()) => {
                let ids = unjoined_ids(&joins)
                    .into_iter()
                    .chain(std::iter::once(timer.id))
                    .collect::<Vec<_>>();
                cancel_unfinished(joins.iter());
                if !executor::is_done(timer.id) {
                    executor::cancel_current(timer.id);
                }
                let _ = wait_until_all(&ids);
                for join in &joins {
                    if !join.joined.get() {
                        let _ = join_one(join);
                    }
                }
                let _ = join_one(&timer);
                propagate_cancellation();
            }
        };
        let branch_index = active[winner].1;
        let Some(branch_index) = branch_index else {
            cancel_unfinished(joins.iter());
            let ids = unjoined_ids(&joins)
                .into_iter()
                .chain(std::iter::once(timer.id))
                .collect::<Vec<_>>();
            let cancelled = wait_until_all(&ids);
            for join in &joins {
                if !join.joined.get() {
                    let _ = join_one(join);
                }
            }
            let _ = join_one(&timer);
            if cancelled {
                propagate_cancellation();
            }
            return Err(WithinError::Timeout);
        };
        outcomes[branch_index] = Some(join_one(&joins[branch_index]));
        pending -= 1;
        if pending == 0 {
            if !executor::is_done(timer.id) {
                executor::cancel_current(timer.id);
            }
            let ids = std::iter::once(timer.id).collect::<Vec<_>>();
            let cancelled = wait_until_all(&ids);
            let _ = join_one(&timer);
            if cancelled {
                drop(outcomes);
                propagate_cancellation();
            }
            return Ok(outcomes.into_iter().map(Option::unwrap).collect());
        }
    }
}

fn wait_for_proxy(joins: &[TaskId]) -> Result<usize, ()> {
    loop {
        if executor::current_task_cancel_requested() {
            return Err(());
        }
        if let Some(index) = joins
            .iter()
            .enumerate()
            .filter_map(|(index, id)| {
                executor::completion_sequence(*id).map(|sequence| (sequence, index))
            })
            .min()
            .map(|(_, index)| index)
        {
            return Ok(index);
        }
        executor::wait_any_current(joins);
    }
}

/// Concurrent map that preserves input order and returns one outcome per item.
///
/// This API carries native Rust values only; it does not register Zirk GC roots.
pub fn each_settled<I, F, T>(items: I, map: F) -> Result<Vec<Outcome<T>>, CombinatorError>
where
    I: IntoIterator,
    I::Item: Send + 'static,
    F: Fn(I::Item) -> T + Clone + Send + 'static,
    T: Send + 'static,
{
    each_settled_with_limit(items, DEFAULT_EACH_CONCURRENCY, map)
}

/// Concurrent map with bounded active admission and stable input ordering.
/// At most `limit` callbacks are admitted at once; no more than 256 inputs are
/// accepted in total. This internal Rust API supports unmanaged `Send` values
/// only and does not register Zirk GC roots.
pub fn each_settled_with_limit<I, F, T>(
    items: I,
    limit: usize,
    map: F,
) -> Result<Vec<Outcome<T>>, CombinatorError>
where
    I: IntoIterator,
    I::Item: Send + 'static,
    F: Fn(I::Item) -> T + Clone + Send + 'static,
    T: Send + 'static,
{
    assert!(
        executor::in_task_context(),
        "each_settled_with_limit outside an executor task"
    );
    if !(1..=MAX_CONCURRENT_BRANCHES).contains(&limit) {
        return Err(CombinatorError::InvalidConcurrencyLimit);
    }
    let mut items = collect_bounded(items)?.into_iter();
    let mut joins: Vec<Option<Join<T>>> = Vec::new();
    let mut outcomes: Vec<Option<Outcome<T>>> = Vec::new();
    let mut exhausted = false;
    while !exhausted || joins.iter().any(Option::is_some) {
        while !exhausted && joins.iter().filter(|join| join.is_some()).count() < limit {
            match items.next() {
                Some(item) => {
                    let map = map.clone();
                    joins.push(Some(spawn_one(move || map(item))));
                    outcomes.push(None);
                }
                None => exhausted = true,
            }
        }
        if joins.iter().all(Option::is_none) {
            break;
        }
        let active = joins
            .iter()
            .enumerate()
            .filter_map(|(index, join)| join.as_ref().map(|join| (index, join.id)))
            .collect::<Vec<_>>();
        let ids = active.iter().map(|(_, id)| *id).collect::<Vec<_>>();
        let winner = match wait_for_proxy(&ids) {
            Ok(index) => index,
            Err(()) => {
                let active_joins = joins.iter().filter_map(Option::as_ref).collect::<Vec<_>>();
                cancel_unfinished(active_joins.iter().copied());
                let pending = active_joins
                    .iter()
                    .filter(|join| !join.joined.get())
                    .map(|join| join.id)
                    .collect::<Vec<_>>();
                let _ = wait_until_all(&pending);
                for join in active_joins {
                    if !join.joined.get() {
                        let _ = join_one(join);
                    }
                }
                propagate_cancellation();
            }
        };
        let (slot, _) = active[winner];
        let join = joins[slot].take().expect("active join exists");
        outcomes[slot] = Some(join_one(&join));
    }
    Ok(outcomes.into_iter().map(Option::unwrap).collect())
}

/// Concurrent map that preserves input order. The first unhandled failure or
/// cancellation stops the operation after cooperative cancellation and join.
///
/// This API carries native Rust values only; it does not register Zirk GC roots.
pub fn each<I, F, T>(items: I, map: F) -> Result<Vec<T>, EachError>
where
    I: IntoIterator,
    I::Item: Send + 'static,
    F: Fn(I::Item) -> T + Clone + Send + 'static,
    T: Send + 'static,
{
    each_with_limit(items, DEFAULT_EACH_CONCURRENCY, map)
}

/// Fail-fast concurrent map with bounded admission, stable result ordering,
/// and cleanup completion for every admitted branch before returning.
pub fn each_with_limit<I, F, T>(items: I, limit: usize, map: F) -> Result<Vec<T>, EachError>
where
    I: IntoIterator,
    I::Item: Send + 'static,
    F: Fn(I::Item) -> T + Clone + Send + 'static,
    T: Send + 'static,
{
    assert!(
        executor::in_task_context(),
        "each_with_limit outside an executor task"
    );
    if !(1..=MAX_CONCURRENT_BRANCHES).contains(&limit) {
        return Err(EachError::InvalidConcurrencyLimit);
    }
    let mut items = collect_bounded(items)
        .map_err(|_| EachError::TooManyBranches)?
        .into_iter();
    let mut joins: Vec<Option<Join<T>>> = Vec::new();
    let mut outputs: Vec<Option<T>> = (0..joins.len()).map(|_| None).collect();
    let mut exhausted = false;
    while !exhausted || joins.iter().any(Option::is_some) {
        while !exhausted && joins.iter().filter(|join| join.is_some()).count() < limit {
            match items.next() {
                Some(item) => {
                    let map = map.clone();
                    joins.push(Some(spawn_one(move || map(item))));
                    outputs.push(None);
                }
                None => exhausted = true,
            }
        }
        if joins.iter().all(Option::is_none) {
            break;
        }
        let active = joins
            .iter()
            .enumerate()
            .filter_map(|(index, join)| join.as_ref().map(|join| (index, join.id)))
            .collect::<Vec<_>>();
        let active_ids = active.iter().map(|(_, id)| *id).collect::<Vec<_>>();
        let winner_index = match wait_for_proxy(&active_ids) {
            Ok(index) => index,
            Err(()) => {
                let active = joins.iter().filter_map(Option::as_ref).collect::<Vec<_>>();
                cancel_unfinished(active.iter().copied());
                let ids = active
                    .iter()
                    .filter(|join| !join.joined.get())
                    .map(|join| join.id)
                    .collect::<Vec<_>>();
                let _ = wait_until_all(&ids);
                for join in &active {
                    if !join.joined.get() {
                        let _ = join_one(join);
                    }
                }
                propagate_cancellation();
            }
        };
        let (index, _) = active[winner_index];
        let join = joins[index].take().expect("active join exists");
        match join_one(&join) {
            Outcome::Fulfilled(value) => {
                outputs[index] = Some(value);
            }
            Outcome::Rejected(failure) => {
                let active = joins.iter().filter_map(Option::as_ref).collect::<Vec<_>>();
                cancel_unfinished(active.iter().copied());
                let ids = active.iter().map(|join| join.id).collect::<Vec<_>>();
                let cancelled = wait_until_all(&ids);
                for join in &active {
                    if !join.joined.get() {
                        let _ = join_one(join);
                    }
                }
                if cancelled {
                    drop(failure);
                    propagate_cancellation();
                }
                return Err(EachError::BranchFailed(failure));
            }
            Outcome::Cancelled(_) => {
                let active = joins.iter().filter_map(Option::as_ref).collect::<Vec<_>>();
                cancel_unfinished(active.iter().copied());
                let ids = active.iter().map(|join| join.id).collect::<Vec<_>>();
                let _ = wait_until_all(&ids);
                for join in &active {
                    if !join.joined.get() {
                        let _ = join_one(join);
                    }
                }
                return Err(EachError::Cancelled);
            }
        }
    }
    Ok(outputs.into_iter().map(Option::unwrap).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task::TaskOutcome;
    use std::sync::{Arc, Mutex};

    fn run(body: impl FnOnce() -> usize + Send + 'static) -> usize {
        match executor::Executor::new().run_with_root(body) {
            TaskOutcome::Value(value) => value,
            TaskOutcome::Panicked(payload) => std::panic::resume_unwind(payload),
        }
    }

    #[test]
    fn settled_preserves_argument_order_and_keeps_failures_as_outcomes() {
        run(|| {
            let order = Arc::new(Mutex::new(Vec::new()));
            let result = settled(vec![
                Box::new({
                    let order = Arc::clone(&order);
                    move || {
                        executor::yield_now();
                        order.lock().unwrap().push(1);
                        10
                    }
                }) as Box<dyn FnOnce() -> i32 + Send>,
                Box::new({
                    let order = Arc::clone(&order);
                    move || {
                        order.lock().unwrap().push(2);
                        panic!("branch failure");
                    }
                }),
            ])
            .unwrap();
            assert!(matches!(&result[0], Outcome::Fulfilled(10)));
            assert!(
                matches!(&result[1], Outcome::Rejected(error) if error.message() == "branch failure")
            );
            assert_eq!(*order.lock().unwrap(), vec![2, 1]);
            0
        });
    }

    #[test]
    fn first_cancels_and_joins_loser_before_returning() {
        static CLEANED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        CLEANED.store(false, std::sync::atomic::Ordering::SeqCst);
        run(|| {
            let result = first(vec![
                || {
                    loop {
                        executor::sleep(60_000_000_000);
                        if executor::current_task_is_cancelled() {
                            CLEANED.store(true, std::sync::atomic::Ordering::SeqCst);
                            return 1;
                        }
                    }
                },
                || 2,
            ])
            .unwrap();
            assert!(matches!(result, Some(Outcome::Fulfilled(2))));
            assert!(CLEANED.load(std::sync::atomic::Ordering::SeqCst));
            0
        });
    }

    #[test]
    fn first_returns_the_first_failure_and_joins_siblings() {
        static CLEANED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        CLEANED.store(false, std::sync::atomic::Ordering::SeqCst);
        run(|| {
            let result = first(vec![|| -> usize { panic!("first failure") }, || {
                loop {
                    executor::sleep(60_000_000_000);
                    if executor::current_task_is_cancelled() {
                        CLEANED.store(true, std::sync::atomic::Ordering::SeqCst);
                        return 0;
                    }
                }
            }])
            .unwrap();
            assert!(
                matches!(result, Some(Outcome::Rejected(error)) if error.message() == "first failure")
            );
            assert!(CLEANED.load(std::sync::atomic::Ordering::SeqCst));
            0
        });
    }

    #[test]
    fn first_preserves_loser_failures_as_suppressed_in_completion_order() {
        run(|| {
            let completed = Arc::new(Mutex::new(Vec::new()));
            let fail = |message: &'static str,
                        completed: Arc<Mutex<Vec<(TaskId, &'static str)>>>| {
                move || {
                    completed
                        .lock()
                        .unwrap()
                        .push((executor::current_task(), message));
                    panic!("{message}");
                }
            };
            let result = first(vec![
                Box::new(fail("branch zero", Arc::clone(&completed)))
                    as Box<dyn FnOnce() -> usize + Send>,
                Box::new(fail("branch one", Arc::clone(&completed))),
                Box::new(fail("branch two", Arc::clone(&completed))),
            ])
            .unwrap();
            let Some(Outcome::Rejected(primary)) = result else {
                panic!("expected the first failure to remain primary")
            };
            let mut completed = completed.lock().unwrap().clone();
            completed.sort_by_key(|(task, _)| executor::completion_sequence(*task).unwrap());
            let actual = std::iter::once(primary.message())
                .chain(primary.suppressed().iter().map(TaskFailure::message))
                .collect::<Vec<_>>();
            let expected = completed
                .into_iter()
                .map(|(_, message)| message)
                .collect::<Vec<_>>();
            assert_eq!(actual, expected);
            0
        });
    }

    fn panic_during_cancellation(
        order: Arc<std::sync::atomic::AtomicUsize>,
    ) -> impl FnOnce() -> usize + Send + 'static {
        move || loop {
            if executor::current_task_cancel_requested() {
                let position = order.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                panic!(
                    "{} cleanup failure",
                    if position == 0 { "first" } else { "second" }
                );
            }
            executor::yield_now();
        }
    }

    fn assert_cleanup_failures_replace_successful_first(result: Option<Outcome<usize>>) {
        let Some(Outcome::Rejected(primary)) = result else {
            panic!("a losing branch cleanup failure must not be discarded")
        };
        assert_eq!(primary.message(), "first cleanup failure");
        assert_eq!(
            primary
                .suppressed()
                .iter()
                .map(TaskFailure::message)
                .collect::<Vec<_>>(),
            vec!["second cleanup failure"]
        );
    }

    #[test]
    fn first_promotes_cancellation_cleanup_failures_after_a_winner() {
        run(|| {
            let cleanup_order = Arc::new(std::sync::atomic::AtomicUsize::new(0));
            let result = first(vec![
                Box::new(panic_during_cancellation(Arc::clone(&cleanup_order)))
                    as Box<dyn FnOnce() -> usize + Send>,
                Box::new(|| 42),
                Box::new(panic_during_cancellation(cleanup_order)),
            ])
            .unwrap();
            assert_cleanup_failures_replace_successful_first(result);
            0
        });
    }

    #[test]
    fn first_within_promotes_cancellation_cleanup_failures_after_a_winner() {
        run(|| {
            let cleanup_order = Arc::new(std::sync::atomic::AtomicUsize::new(0));
            let result = first_within(
                vec![
                    Box::new(panic_during_cancellation(Arc::clone(&cleanup_order)))
                        as Box<dyn FnOnce() -> usize + Send>,
                    Box::new(|| 42),
                    Box::new(panic_during_cancellation(cleanup_order)),
                ],
                Duration::from_secs(60),
            )
            .unwrap();
            assert_cleanup_failures_replace_successful_first(result);
            0
        });
    }

    #[test]
    fn first_ignores_a_cancelled_branch_until_success_or_failure() {
        use std::sync::atomic::{AtomicU64, Ordering};

        run(|| {
            let second_id = Arc::new(AtomicU64::new(0));
            let result = first(vec![
                Box::new({
                    let second_id = Arc::clone(&second_id);
                    move || {
                        while second_id.load(Ordering::SeqCst) == 0 {
                            executor::yield_now();
                        }
                        executor::cancel_current(TaskId::from_bits(
                            second_id.load(Ordering::SeqCst),
                        ));
                        executor::sleep(1_000_000);
                        9_usize
                    }
                }) as Box<dyn FnOnce() -> usize + Send>,
                Box::new({
                    let second_id = Arc::clone(&second_id);
                    move || {
                        second_id.store(executor::current_task().to_bits(), Ordering::SeqCst);
                        executor::sleep(60_000_000_000);
                        2_usize
                    }
                }),
            ])
            .unwrap();
            assert!(matches!(result, Some(Outcome::Fulfilled(9))));
            0
        });
    }

    #[test]
    fn caller_cancellation_joins_children_before_propagating() {
        static CLEANED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        CLEANED.store(false, std::sync::atomic::Ordering::SeqCst);
        run(|| {
            let combinator = executor::spawn(|| {
                let _ = settled(vec![|| {
                    loop {
                        executor::sleep(60_000_000_000);
                        if executor::current_task_is_cancelled() {
                            CLEANED.store(true, std::sync::atomic::Ordering::SeqCst);
                            return 1;
                        }
                    }
                }]);
                0
            });
            executor::yield_now();
            executor::cancel_current(combinator);
            let cancelled = catch_unwind(AssertUnwindSafe(|| executor::await_task(combinator)));
            assert!(cancelled.is_err());
            assert!(CLEANED.load(std::sync::atomic::Ordering::SeqCst));
            0
        });
    }

    #[test]
    fn caller_cancellation_wins_over_pending_within_timeout_and_joins_all_children() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static STARTED: AtomicUsize = AtomicUsize::new(0);
        static CLEANED: AtomicUsize = AtomicUsize::new(0);
        STARTED.store(0, Ordering::SeqCst);
        CLEANED.store(0, Ordering::SeqCst);
        run(|| {
            let baseline = executor::waiter_registration_count_for_test();
            let timer_baseline = executor::timer_registration_count_for_test();
            let task_baseline = executor::task_count_for_test();
            for use_first in [false, true] {
                let expected_start = STARTED.load(Ordering::SeqCst) + 1;
                let combinator = executor::spawn(move || {
                    let branch = || {
                        STARTED.fetch_add(1, Ordering::SeqCst);
                        loop {
                            executor::sleep(60_000_000_000);
                            if executor::current_task_is_cancelled() {
                                CLEANED.fetch_add(1, Ordering::SeqCst);
                                return 1;
                            }
                        }
                    };
                    if use_first {
                        let _ = first_within(vec![branch], Duration::from_secs(60));
                    } else {
                        let _ = settled_within(vec![branch], Duration::from_secs(60));
                    }
                    0
                });
                while STARTED.load(Ordering::SeqCst) < expected_start {
                    executor::yield_now();
                }
                executor::cancel_current(combinator);
                assert!(
                    catch_unwind(AssertUnwindSafe(|| executor::await_task(combinator))).is_err()
                );
                assert_eq!(executor::waiter_registration_count_for_test(), baseline);
                assert_eq!(
                    executor::timer_registration_count_for_test(),
                    timer_baseline
                );
                assert_eq!(executor::task_count_for_test(), task_baseline);
            }
            assert_eq!(STARTED.load(Ordering::SeqCst), 2);
            assert_eq!(CLEANED.load(Ordering::SeqCst), 2);
            0
        });
    }

    #[test]
    fn caller_cancellation_after_deadline_expiry_but_before_timeout_handling_wins() {
        use std::sync::atomic::{AtomicBool, Ordering};

        let branch_started = Arc::new(AtomicBool::new(false));
        let branch_cleaned = Arc::new(AtomicBool::new(false));
        run({
            let branch_started = Arc::clone(&branch_started);
            let branch_cleaned = Arc::clone(&branch_cleaned);
            move || {
                let waiter_baseline = executor::waiter_registration_count_for_test();
                let timer_baseline = executor::timer_registration_count_for_test();
                let task_baseline = executor::task_count_for_test();
                let caller = executor::spawn({
                    let branch_started = Arc::clone(&branch_started);
                    let branch_cleaned = Arc::clone(&branch_cleaned);
                    move || {
                        let branch_started = Arc::clone(&branch_started);
                        let branch_cleaned = Arc::clone(&branch_cleaned);
                        let _ = first_within(
                            vec![move || {
                                branch_started.store(true, Ordering::SeqCst);
                                loop {
                                    executor::yield_now();
                                    if executor::current_task_is_cancelled() {
                                        branch_cleaned.store(true, Ordering::SeqCst);
                                        return 1;
                                    }
                                }
                            }],
                            Duration::ZERO,
                        );
                        0
                    }
                });
                let cancel_after_expiry = executor::spawn(move || {
                    // Queue order is: this task, branch, timer. Yield once so
                    // the branch yields and the timer arms its zero deadline;
                    // the executor expires it before this task's next turn.
                    // The timer task itself is still queued ahead of the caller,
                    // so cancellation is observed before timeout arbitration.
                    executor::yield_now();
                    assert!(branch_started.load(Ordering::SeqCst));
                    assert_eq!(
                        executor::timer_registration_count_for_test(),
                        timer_baseline,
                        "the zero-deadline timer must have expired before cancellation"
                    );
                    executor::cancel_current(caller);
                    0
                });

                assert!(catch_unwind(AssertUnwindSafe(|| executor::await_task(caller))).is_err());
                executor::await_task(cancel_after_expiry);
                assert_eq!(
                    executor::waiter_registration_count_for_test(),
                    waiter_baseline
                );
                assert_eq!(
                    executor::timer_registration_count_for_test(),
                    timer_baseline
                );
                assert_eq!(executor::task_count_for_test(), task_baseline);
                assert!(branch_cleaned.load(Ordering::SeqCst));
                0
            }
        });
    }

    #[test]
    fn each_settled_maps_concurrently_but_keeps_input_order() {
        run(|| {
            let result = each_settled(vec![3, 1, 2], |value| 10 / value).unwrap();
            let values = result
                .into_iter()
                .map(|outcome| match outcome {
                    Outcome::Fulfilled(value) => value,
                    _ => panic!("unexpected outcome"),
                })
                .collect::<Vec<_>>();
            assert_eq!(values, vec![3, 10, 5]);
            0
        });
    }

    #[test]
    fn default_each_uses_the_32_callback_window() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static ACTIVE: AtomicUsize = AtomicUsize::new(0);
        static MAX_ACTIVE: AtomicUsize = AtomicUsize::new(0);
        ACTIVE.store(0, Ordering::SeqCst);
        MAX_ACTIVE.store(0, Ordering::SeqCst);
        run(|| {
            let values = each(0..80, |value| {
                let active = ACTIVE.fetch_add(1, Ordering::SeqCst) + 1;
                MAX_ACTIVE.fetch_max(active, Ordering::SeqCst);
                executor::yield_now();
                ACTIVE.fetch_sub(1, Ordering::SeqCst);
                value
            })
            .unwrap();
            assert_eq!(values, (0..80).collect::<Vec<_>>());
            assert_eq!(MAX_ACTIVE.load(Ordering::SeqCst), DEFAULT_EACH_CONCURRENCY);
            0
        });
    }

    #[test]
    fn default_each_settled_uses_the_32_callback_window() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static ACTIVE: AtomicUsize = AtomicUsize::new(0);
        static MAX_ACTIVE: AtomicUsize = AtomicUsize::new(0);
        ACTIVE.store(0, Ordering::SeqCst);
        MAX_ACTIVE.store(0, Ordering::SeqCst);
        run(|| {
            let outcomes = each_settled(0..80, |value| {
                let active = ACTIVE.fetch_add(1, Ordering::SeqCst) + 1;
                MAX_ACTIVE.fetch_max(active, Ordering::SeqCst);
                executor::yield_now();
                ACTIVE.fetch_sub(1, Ordering::SeqCst);
                value
            })
            .unwrap();
            let values = outcomes
                .into_iter()
                .map(|outcome| match outcome {
                    Outcome::Fulfilled(value) => value,
                    _ => panic!("unexpected outcome"),
                })
                .collect::<Vec<_>>();
            assert_eq!(values, (0..80).collect::<Vec<_>>());
            assert_eq!(MAX_ACTIVE.load(Ordering::SeqCst), DEFAULT_EACH_CONCURRENCY);
            0
        });
    }

    #[test]
    fn limited_each_bounds_active_callbacks_and_preserves_order() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static ACTIVE: AtomicUsize = AtomicUsize::new(0);
        static MAX_ACTIVE: AtomicUsize = AtomicUsize::new(0);
        ACTIVE.store(0, Ordering::SeqCst);
        MAX_ACTIVE.store(0, Ordering::SeqCst);
        run(|| {
            let values = each_with_limit(0..12, 3, |value| {
                let active = ACTIVE.fetch_add(1, Ordering::SeqCst) + 1;
                MAX_ACTIVE.fetch_max(active, Ordering::SeqCst);
                executor::yield_now();
                ACTIVE.fetch_sub(1, Ordering::SeqCst);
                value * 2
            })
            .unwrap();
            assert_eq!(values, (0..12).map(|value| value * 2).collect::<Vec<_>>());
            assert_eq!(MAX_ACTIVE.load(Ordering::SeqCst), 3);
            0
        });
    }

    #[test]
    fn limited_settled_does_not_admit_past_window_until_a_slot_opens() {
        use std::sync::atomic::{AtomicBool, Ordering};
        static SECOND_FINISHED: AtomicBool = AtomicBool::new(false);
        SECOND_FINISHED.store(false, Ordering::SeqCst);
        run(|| {
            let results = each_settled_with_limit(0..3, 2, |value| match value {
                0 => {
                    while !SECOND_FINISHED.load(Ordering::SeqCst) {
                        executor::yield_now();
                    }
                    10
                }
                1 => {
                    SECOND_FINISHED.store(true, Ordering::SeqCst);
                    11
                }
                2 => {
                    assert!(SECOND_FINISHED.load(Ordering::SeqCst));
                    12
                }
                _ => unreachable!(),
            })
            .unwrap();
            assert!(matches!(results[0], Outcome::Fulfilled(10)));
            assert!(matches!(results[1], Outcome::Fulfilled(11)));
            assert!(matches!(results[2], Outcome::Fulfilled(12)));
            0
        });
    }

    #[test]
    fn limited_each_rejects_invalid_limits_and_fail_fast_joins_started_work() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        struct Cleanup<'a>(&'a AtomicUsize);
        impl Drop for Cleanup<'_> {
            fn drop(&mut self) {
                self.0.fetch_add(1, Ordering::SeqCst);
            }
        }
        static CLEANED: AtomicUsize = AtomicUsize::new(0);
        static STARTED: AtomicUsize = AtomicUsize::new(0);
        CLEANED.store(0, Ordering::SeqCst);
        STARTED.store(0, Ordering::SeqCst);
        run(|| {
            assert!(matches!(
                each_with_limit(0..1, 0, |x| x),
                Err(EachError::InvalidConcurrencyLimit)
            ));
            assert!(matches!(
                each_with_limit(0..1, MAX_CONCURRENT_BRANCHES + 1, |x| x),
                Err(EachError::InvalidConcurrencyLimit)
            ));
            let result = each_with_limit(0..20, 2, |value| {
                STARTED.fetch_add(1, Ordering::SeqCst);
                let _cleanup = Cleanup(&CLEANED);
                if value == 0 {
                    loop {
                        executor::sleep(60_000_000_000);
                        if executor::current_task_is_cancelled() {
                            return value;
                        }
                    }
                }
                if value == 1 {
                    panic!("bounded failure");
                }
                value
            });
            assert!(matches!(result, Err(EachError::BranchFailed(_))));
            assert_eq!(
                CLEANED.load(Ordering::SeqCst),
                STARTED.load(Ordering::SeqCst)
            );
            assert!(STARTED.load(Ordering::SeqCst) <= 2);
            0
        });
    }

    #[test]
    fn limited_caller_cancellation_cleans_up_all_admitted_branches() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static CLEANED: AtomicUsize = AtomicUsize::new(0);
        static STARTED: AtomicUsize = AtomicUsize::new(0);
        CLEANED.store(0, Ordering::SeqCst);
        STARTED.store(0, Ordering::SeqCst);
        run(|| {
            let child = executor::spawn(|| {
                let _ = each_settled_with_limit(0..10, 3, |_| {
                    STARTED.fetch_add(1, Ordering::SeqCst);
                    loop {
                        executor::sleep(60_000_000_000);
                        if executor::current_task_is_cancelled() {
                            CLEANED.fetch_add(1, Ordering::SeqCst);
                            return 1;
                        }
                    }
                });
                0
            });
            executor::yield_now();
            executor::cancel_current(child);
            assert!(catch_unwind(AssertUnwindSafe(|| executor::await_task(child))).is_err());
            assert!(STARTED.load(Ordering::SeqCst) > 0);
            assert_eq!(
                CLEANED.load(Ordering::SeqCst),
                STARTED.load(Ordering::SeqCst)
            );
            assert!(STARTED.load(Ordering::SeqCst) <= 3);
            0
        });
    }

    #[test]
    fn limited_settled_rejects_invalid_limits_and_accepts_exact_total_cap() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static STARTED: AtomicUsize = AtomicUsize::new(0);
        STARTED.store(0, Ordering::SeqCst);
        run(|| {
            assert!(matches!(
                each_settled_with_limit(0..0, 0, |x| x),
                Err(CombinatorError::InvalidConcurrencyLimit)
            ));
            assert!(matches!(
                each_settled_with_limit(0..0, MAX_CONCURRENT_BRANCHES + 1, |x| x),
                Err(CombinatorError::InvalidConcurrencyLimit)
            ));
            assert_eq!(
                each_settled_with_limit(0..MAX_CONCURRENT_BRANCHES, 1, |x| x)
                    .unwrap()
                    .len(),
                MAX_CONCURRENT_BRANCHES
            );
            assert!(matches!(
                each_settled_with_limit(0..=MAX_CONCURRENT_BRANCHES, 1, |_| {
                    STARTED.fetch_add(1, Ordering::SeqCst);
                }),
                Err(CombinatorError::TooManyBranches)
            ));
            assert!(matches!(
                each_with_limit(0..MAX_CONCURRENT_BRANCHES + 1, 1, |_| {
                    STARTED.fetch_add(1, Ordering::SeqCst);
                }),
                Err(EachError::TooManyBranches)
            ));
            assert_eq!(STARTED.load(Ordering::SeqCst), 0);
            0
        });
    }

    #[test]
    fn within_timeout_cancels_and_joins_branches() {
        static CLEANED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        CLEANED.store(false, std::sync::atomic::Ordering::SeqCst);
        run(|| {
            let result = settled_within(
                vec![|| {
                    loop {
                        executor::sleep(60_000_000_000);
                        if executor::current_task_is_cancelled() {
                            CLEANED.store(true, std::sync::atomic::Ordering::SeqCst);
                            return 1;
                        }
                    }
                }],
                Duration::from_millis(1),
            );
            assert_eq!(result.unwrap_err(), WithinError::Timeout);
            assert!(CLEANED.load(std::sync::atomic::Ordering::SeqCst));
            0
        });
    }

    #[test]
    fn each_fails_fast_and_joins_cancelled_siblings() {
        static CLEANED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        CLEANED.store(false, std::sync::atomic::Ordering::SeqCst);
        run(|| {
            let error = each(vec![0, 1, 2], |value| match value {
                0 => loop {
                    executor::sleep(60_000_000_000);
                    if executor::current_task_is_cancelled() {
                        CLEANED.store(true, std::sync::atomic::Ordering::SeqCst);
                        return 0;
                    }
                },
                1 => panic!("stop siblings"),
                _ => 2,
            })
            .unwrap_err();
            assert!(
                matches!(error, EachError::BranchFailed(error) if error.message() == "stop siblings")
            );
            assert!(CLEANED.load(std::sync::atomic::Ordering::SeqCst));
            0
        });
    }

    #[test]
    fn first_within_returns_before_timeout_and_preserves_winner() {
        run(|| {
            let result = first_within(vec![|| 17], Duration::from_secs(1)).unwrap();
            assert!(matches!(result, Some(Outcome::Fulfilled(17))));
            0
        });
    }

    #[test]
    fn first_within_times_out_after_cancelling_and_joining() {
        static CLEANED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        CLEANED.store(false, std::sync::atomic::Ordering::SeqCst);
        run(|| {
            let result = first_within(
                vec![|| loop {
                    executor::sleep(60_000_000_000);
                    if executor::current_task_is_cancelled() {
                        CLEANED.store(true, std::sync::atomic::Ordering::SeqCst);
                        return 1;
                    }
                }],
                Duration::from_millis(1),
            );
            assert!(matches!(result, Err(WithinError::Timeout)));
            assert!(CLEANED.load(std::sync::atomic::Ordering::SeqCst));
            0
        });
    }

    #[test]
    fn within_rejects_durations_outside_timer_abi_range() {
        run(|| {
            let result = settled_within(Vec::<fn() -> usize>::new(), Duration::from_secs(u64::MAX));
            assert!(matches!(result, Err(WithinError::DurationOutOfRange)));
            0
        });
    }

    #[test]
    fn branch_admission_limit_is_checked_before_spawning() {
        run(|| {
            let result = settled((0..=MAX_CONCURRENT_BRANCHES).map(|_| || 1));
            assert!(matches!(result, Err(CombinatorError::TooManyBranches)));
            0
        });
    }

    #[test]
    fn branch_limit_admits_exactly_256_and_rejects_257_without_running_them() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        static STARTED: AtomicUsize = AtomicUsize::new(0);
        STARTED.store(0, Ordering::SeqCst);
        run(|| {
            let values = settled((0..MAX_CONCURRENT_BRANCHES).map(|value| {
                move || {
                    STARTED.fetch_add(1, Ordering::SeqCst);
                    value
                }
            }))
            .unwrap();
            assert_eq!(values.len(), MAX_CONCURRENT_BRANCHES);
            assert_eq!(STARTED.load(Ordering::SeqCst), MAX_CONCURRENT_BRANCHES);

            let rejected = settled((0..=MAX_CONCURRENT_BRANCHES).map(|_| {
                || {
                    STARTED.fetch_add(1, Ordering::SeqCst);
                    0
                }
            }));
            assert!(matches!(rejected, Err(CombinatorError::TooManyBranches)));
            assert_eq!(STARTED.load(Ordering::SeqCst), MAX_CONCURRENT_BRANCHES);
            0
        });
    }

    #[test]
    fn empty_inputs_return_immediately_for_all_combinators() {
        run(|| {
            assert!(settled(Vec::<fn() -> usize>::new()).unwrap().is_empty());
            assert!(first(Vec::<fn() -> usize>::new()).unwrap().is_none());
            assert!(
                first_within(Vec::<fn() -> usize>::new(), Duration::ZERO)
                    .unwrap()
                    .is_none()
            );
            assert!(
                settled_within(Vec::<fn() -> usize>::new(), Duration::ZERO)
                    .unwrap()
                    .is_empty()
            );
            assert!(
                each_settled(Vec::<usize>::new(), |value| value)
                    .unwrap()
                    .is_empty()
            );
            assert!(each(Vec::<usize>::new(), |value| value).unwrap().is_empty());
            0
        });
    }

    #[test]
    fn repeated_combinator_returns_leave_executor_able_to_wake_new_waiters() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        static CLEANED: AtomicUsize = AtomicUsize::new(0);
        CLEANED.store(0, Ordering::SeqCst);
        run(|| {
            let baseline = executor::waiter_registration_count_for_test();
            for iteration in 0..64 {
                let outcomes = settled((0..8).map(|branch| {
                    move || {
                        if (branch + iteration) % 2 == 0 {
                            executor::yield_now();
                        }
                        CLEANED.fetch_add(1, Ordering::SeqCst);
                        branch
                    }
                }))
                .unwrap();
                assert_eq!(outcomes.len(), 8);
                assert!(
                    outcomes
                        .iter()
                        .all(|outcome| matches!(outcome, Outcome::Fulfilled(_)))
                );
                assert_eq!(executor::waiter_registration_count_for_test(), baseline);

                let winner = first((0..8).map(move |branch| move || branch + iteration * 8))
                    .unwrap()
                    .expect("non-empty input has a winner");
                assert!(matches!(winner, Outcome::Fulfilled(value) if value < 512));
                assert_eq!(executor::waiter_registration_count_for_test(), baseline);
            }
            assert_eq!(CLEANED.load(Ordering::SeqCst), 512);
            executor::yield_now();
            assert_eq!(executor::waiter_registration_count_for_test(), baseline);
            0
        });
    }

    #[test]
    fn repeated_timeouts_and_caller_cancellation_join_every_started_branch() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        static CLEANED: AtomicUsize = AtomicUsize::new(0);
        static STARTED: AtomicUsize = AtomicUsize::new(0);
        CLEANED.store(0, Ordering::SeqCst);
        STARTED.store(0, Ordering::SeqCst);
        run(|| {
            let baseline = executor::waiter_registration_count_for_test();
            let timer_baseline = executor::timer_registration_count_for_test();
            let task_baseline = executor::task_count_for_test();
            for _ in 0..16 {
                let result = settled_within(
                    (0..4).map(|_| {
                        || {
                            STARTED.fetch_add(1, Ordering::SeqCst);
                            loop {
                                executor::sleep(60_000_000_000);
                                if executor::current_task_is_cancelled() {
                                    CLEANED.fetch_add(1, Ordering::SeqCst);
                                    return 1;
                                }
                            }
                        }
                    }),
                    Duration::from_nanos(1),
                );
                assert!(matches!(result, Err(WithinError::Timeout)));
                assert_eq!(executor::waiter_registration_count_for_test(), baseline);
                assert_eq!(
                    executor::timer_registration_count_for_test(),
                    timer_baseline
                );
                assert_eq!(executor::task_count_for_test(), task_baseline);
            }

            for _ in 0..8 {
                let child = executor::spawn(|| {
                    let _ = settled((0..4).map(|_| {
                        || {
                            STARTED.fetch_add(1, Ordering::SeqCst);
                            loop {
                                executor::sleep(60_000_000_000);
                                if executor::current_task_is_cancelled() {
                                    CLEANED.fetch_add(1, Ordering::SeqCst);
                                    return 1;
                                }
                            }
                        }
                    }));
                    0
                });
                executor::yield_now();
                executor::cancel_current(child);
                assert!(catch_unwind(AssertUnwindSafe(|| executor::await_task(child))).is_err());
                assert_eq!(executor::waiter_registration_count_for_test(), baseline);
                assert_eq!(
                    executor::timer_registration_count_for_test(),
                    timer_baseline
                );
                assert_eq!(executor::task_count_for_test(), task_baseline);
            }
            assert!(STARTED.load(Ordering::SeqCst) > 0);
            assert_eq!(
                CLEANED.load(Ordering::SeqCst),
                STARTED.load(Ordering::SeqCst)
            );
            0
        });
    }
}
