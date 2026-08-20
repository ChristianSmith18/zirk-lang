# Build a File Processor

Build `normalize-users`, which reads a bounded UTF-8 input, validates each line,
and atomically publishes output only after every line succeeds.

> **Status:** target Zirk 1.x example; grouped resource cleanup and the complete
> stdlib may be ahead of the current compiler.

```zirk
import { File, OpenOptions } from std.fs;
import { Path } from std.path;

fn normalize_line(line: String): Result<String, LineError> {
    mut cleaned = line.trim();
    if cleaned.length == 0 return Error(LineError.Empty);
    return Ok(cleaned.lowercase());
}

fn process(input: Path, output: Path): Result<Void, ProcessFileError> {
    match File.open(input, OpenOptions.read_only()) with source {
        Ok(source) => {
            mut normalized = List<String>();
            for line in source.lines(encoding: Encoding.Utf8) {
                match normalize_line(line) {
                    Ok(value) => normalized.add(value);
                    Error(error) => return Error(ProcessFileError.Line(error));
                }
            }

            return File.write_text_atomic(output, normalized.join("\n"));
        }
        Error(error) => return Error(ProcessFileError.Input(error));
    }
}
```

The source closes on success, return, error, or cancellation. Atomic output
prevents a partially transformed destination from looking complete. Production
code should stream to a temporary sibling when the result cannot fit the
selected bound, then sync and replace through the same filesystem.

Declare read authority for the input and write/create authority for the output
directory. Canonicalization and symlink policy are enforced by `std.fs`, not by
manual string checks.

Test normalization as a pure unit, then E2E-test invalid lines, malformed
UTF-8, permission denial, size limit, existing destination, flush/sync failure,
cancellation, and cleanup in a runner-provided temporary directory. Continue
with [Resources](../02-handbook/16-resources/README.md) and
[`std.fs`](../04-standard-library/03-std-fs.md).

## Completion contract

- **Prerequisites:** the CLI tutorial, resources, paths, lists, and filesystem
  permissions.
- **Expected success:** normalized UTF-8 lines atomically replace the output.
- **Failure recovery:** invalid input or cleanup failure leaves no published
  partial output and returns a typed error.
- **Next:** process independent jobs with a [worker pool](03-build-a-concurrent-worker-pool.md).

---

**Previous:** [← Build a CLI](01-build-a-cli.md) · **Next:** [ Build a Concurrent Worker Pool](03-build-a-concurrent-worker-pool.md)
