# Cancellation

Cancellation is cooperative and idempotent. Cancellation-aware channel and
timer waits wake to observe it; generated code also checks at selected branch,
call, and cleanup boundaries. The compiler does not insert a cancellation poll
at every loop back-edge, so a tight loop or synchronous native call may delay
cancellation. GC safepoint polls do not imply cancellation checks.

Cancellation is a terminal runtime outcome, not an ordinary recoverable
exception a Zirk `catch` can consume. Scope cleanup runs before cancellation
propagates. `Job.cancel()` requests cancellation but does not wait for the job
to finish.

`Concurrent.protect((): Void => cleanup())` runs a zero-argument cleanup lambda
while cancellation is deferred. The compiler rejects incomplete, suspending,
blocking, or spawning cleanup effects, then restores the cancellation shield
and checks cancellation before dispatching a pending exception. This does not
prove termination or a time bound; cleanup must finish for cancellation to be
delivered.

---

**Previous:** [← `Job<T>`](16-job.md) · **Next:** [ Timeouts](06-timeouts.md)
