# Build a Concurrent Worker Pool

Zirk has no standalone `worker` primitive. This tutorial composes a bounded
channel and child tasks inside one structured scope.

> **Status:** target Phase 5 semantics; use this as a design/tutorial contract,
> not a claim that the current compiler runs it.

```zirk
fn run_jobs(jobs: List<Job>, workers: UInt): Task<List<JobResult>> {
    return task scope {
        mut input = Channel<Job>.bounded(capacity: workers * 2);
        mut output = Channel<JobResult>.bounded(capacity: workers * 2);

        for mut index = 0; index < workers; index++ {
            task(name: "worker-{index}") {
                loop {
                    match await input.receive() {
                        Value(job) => await output.send(process(job));
                        Closed => break;
                        Error(error) => throw ChannelFailure(error);
                    }
                }
            }
        }

        task {
            for job in jobs { await input.send(job); }
            input.close();
        }

        mut results = List<JobResult>();
        while results.length < jobs.length {
            match await output.receive() {
                Value(result) => results.add(result);
                Closed => break;
                Error(error) => throw ChannelFailure(error);
            }
        }
        return results;
    };
}
```

The bounded input provides backpressure. Scope exit waits for every child and
closes scope-owned channels. An unhandled child exception cancels siblings and
cleanup completes before propagation. If every outcome is required instead,
collect child tasks with `Task.settled` and inspect `Fulfilled`, `Rejected`, and
`Cancelled` in input order.

Do not share an unsynchronized result list between workers. Either send owned
results through a channel or use synchronization-aware state. Test one worker,
more workers than jobs, channel closure, first failure, settled aggregation,
cancellation during send/receive, bounded timeout, stable output ordering, and
leak-free cleanup with a fixed seed. See
[Structured Concurrency](../02-handbook/18-concurrency/README.md) and
the structured-concurrency handbook.

## Completion contract

- **Prerequisites:** tasks, channels, cancellation, `Result`, and collections.
- **Expected success:** exactly one result per input with the selected ordering.
- **Failure recovery:** sibling failure or timeout cancels remaining work and
  awaits cleanup; settled mode retains every outcome.
- **Next:** apply structured tasks to an [HTTP service](04-build-an-http-service.md).

---

**Previous:** [← Build a File Processor](02-build-a-file-processor.md) · **Next:** [ Build an HTTP Service](04-build-an-http-service.md)
