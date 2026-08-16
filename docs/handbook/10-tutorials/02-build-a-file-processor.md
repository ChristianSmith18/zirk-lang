# Build a File Processor

Model input and output as `Path`, not strings. Open input through `Resource<FileError>`, parse each line into an algebraic result, and write transformed output to a separate managed file.

Define policy for invalid lines, partial output, flush failure, cancellation, and replacement of an existing destination. Declare scoped read/write permissions and validate symlink/canonical-path behavior. Unit-test parsing; E2E-test files in unique temporary directories.

---

**Previous:** [← Build a CLI](01-build-a-cli.md) · **Next:** [ Build a Concurrent Worker Pool](03-build-a-concurrent-worker-pool.md)
