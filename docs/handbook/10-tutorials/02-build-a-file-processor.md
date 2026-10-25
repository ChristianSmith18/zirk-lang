# Build a File Processor

Build `normalize-users`, which reads a bounded UTF-8 input line by line and
atomically publishes the normalized output after reading succeeds.

```zirk
import { File, FileError, FileLine, OpenOptions } from std.fs::fs;
import { Path } from std.path::path;

fn process(input: Path, output: Path): Result<Void, FileError> {
    match File.open(input, OpenOptions.read_only()) with source {
        Ok(source) => {
            mut normalized = List<String>();
            for item in source.lines() {
                match item {
                    FileLine.Line(line) => normalized.add(line.trim().lowercase());
                    FileLine.Error(error) => return Error(error);
                    FileLine.End => {}
                }
            }
            return File.write_text_atomic(output, normalized.join("\n"));
        }
        Error(error) => return Error(error);
    }
}
```

The whole-file helpers cap reads at 64 MiB. For this line-by-line version,
`read_line` enforces a 1 MiB line limit and `lines()` is lazy. The managed
`match with` scope closes the source on normal completion, return, error, and
cancellation. Atomic replacement creates a same-directory temporary, syncs it,
then renames it into place; it prevents readers from observing a half-written
output but does not promise power-loss durability.

Declare `filesystem.read` and `filesystem.write` in the project's single
`requires` list. The first operation asks for exact consent to the concrete
input or output path; `.zkinit` does not store an unrestricted directory grant.
Canonicalization and handle-relative identity checks belong to the filesystem
boundary, not manual string checks.

Test line normalization separately, then cover malformed UTF-8, permission
denial, size and line limits, competing create-new calls, replacement failures,
and resource cleanup. Continue with [Resources](../02-handbook/16-resources/README.md)
and [`std.fs`](../04-standard-library/03-std-fs.md).

## Completion contract

- **Prerequisites:** the CLI tutorial, resources, paths, lists, and filesystem
  capabilities.
- **Expected success:** normalized UTF-8 lines atomically replace the output.
- **Failure recovery:** read/decode/permission failures return a typed error;
  atomic replacement leaves the previous output until publication succeeds.
- **Next:** process independent jobs with a [worker pool](03-build-a-concurrent-worker-pool.md).

---

**Previous:** [← Build a CLI](01-build-a-cli.md) · **Next:** [ Build a Concurrent Worker Pool](03-build-a-concurrent-worker-pool.md)
