# Build a Concurrent Worker Pool

Zirk has no standalone `worker` primitive. Compose a bounded `Channel<Job>`, a structured scope, and several child tasks. Producers observe backpressure; consumers exit on channel closure or cancellation.

Return typed results through a second channel, preserve the chosen output order, and avoid shared mutable accumulators. On the first relevant failure, request cancellation and wait for all children to clean up. Test with a fixed seed and bounded timeouts.

---

**Previous:** [← Build a File Processor](02-build-a-file-processor.md) · **Next:** [ Build an HTTP Service](04-build-an-http-service.md)
