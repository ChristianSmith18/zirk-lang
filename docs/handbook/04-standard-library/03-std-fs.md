# `std.fs`

`std.fs` provides bounded file reads and writes, managed file and directory
resources, progressive operations, and advanced directory, link, temporary,
lock, watcher, and asynchronous APIs. The package remains `std.fs::fs`; paths
are values from `std.path::path`.

```zirk
import { File, OpenOptions, Encoding } from std.fs::fs;
import { Path } from std.path::path;

mut result = File.read_text(Path.of("input.txt"), 1048576, Encoding.Utf8);
mut output = File.write_text(Path.of("output.txt"), "processed");
```

Each operation returns a typed `Result`; filesystem errors are ordinary values
that callers can match. Denied access emits a runtime warning with the
requesting application or dependency and the exact path. The failure can be
handled without terminating unrelated code. The runtime stores exact resource
consent separately from `.zkinit`; the manifest's single `requires` list names
`filesystem.read`, `filesystem.write`, and the narrower `filesystem.metadata`
capability. A full read grant also allows metadata inspection for the same
exact path; metadata consent never allows reading file contents.

## Path and permission boundary

`Path` is lexical until `absolute()` or `canonicalize()` is called. Filesystem
operations resolve the effective path and authorize that canonical resource.
They open relative to an already-open parent directory and verify object
identity before using the file. On Unix, final-component symlinks are rejected
when opening a file. The browser compiler rejects reachable filesystem calls
before execution; it never provides a virtual or ambient filesystem.

The runtime uses a 256-handle process ceiling. Read, write, or read/write modes
request their respective capability families; a handle opened for both modes
checks each operation independently.

## Asynchronous file and directory operations

Methods ending in `_async` suspend the current Zirk task while the host
operation runs through the runtime's bounded managed-blocking pool. They do not
occupy the cooperative executor while waiting. The filesystem shares the
process-wide ceiling of 64 managed workers and a FIFO queue capped at 1,024
jobs; admission applies cooperative backpressure instead of creating extra
threads or an unbounded queue.

The API includes `File.read_bytes_async`, `read_text_async`,
`write_bytes_async`, `write_text_async`, `open_async`, `append_bytes_async`, and
`append_text_async`; opened files provide `read_async`, `read_exact_async`,
`write_async`, and `append_async`. `Directory.entries_async` and
`Directory.walk_async` provide asynchronous open and `next_entry_async`
operations. Unix targets also provide `lock_shared_async`,
`lock_exclusive_async`, and `FileWatcher.next_async`. Directory creation, copy,
move, recursive removal, metadata, link, and temporary operations currently
have synchronous APIs only.

These methods retain the same typed `Result`, path authorization, and size
bounds as their synchronous counterparts. Cancellation removes work that has
not started. A host operation already in progress is joined at its safe
completion point; cancellation does not imply that a partially completed
write was rolled back. Async lock waiters recheck operation authority while
retrying, and watcher wait cancellation unregisters the waiter.

```zirk
match File.read_text_async(Path.of("input.txt"), 1048576) {
    Ok(text) => process(text);
    Error(error) => report(error);
}
```

## Bounded reads and managed files

Whole-file reads require a caller limit and also enforce a hard 64 MiB ceiling.
They check metadata before allocation and enforce the ceiling while consuming
the stream, so a file that grows during reading cannot exceed the limit.

```zirk
match File.open(Path.of("input.txt"), OpenOptions.read_only()) with file {
    Ok(file) => {
        mut first = file.read_line();
        mut block = file.read(65536);
    }
    Error(error) => report(error);
}
```

`File` implements `Resource<FileResourceError>`. `match with` closes a
successfully acquired resource at the managed scope boundary. Explicit `close()`
is also available and idempotent; `is_closed()` reports handle state. The
resource does not close merely because it becomes unreachable.

`OpenOptions` is immutable. It offers `none`, `read_only`, `write_only`,
`read_and_write`, `appending`, and `creating_new`; `with_read`, `with_write`,
`with_append`, `with_create`, `with_create_new`, and `with_truncate` return new
values. Empty access modes, conflicting creation modes, append-plus-truncate,
and creation/truncation without write access return `FileError.InvalidOptions`.
`create_new` uses the operating system's exclusive create operation, so only one
of two competing callers can create the same file.

## Text, lines, and chunks

Text defaults to strict UTF-8. Explicit encodings are UTF-8, Latin-1, UTF-16LE,
and UTF-16BE. Invalid byte sequences return `FileError.Decoding`; byte APIs do
not decode or alter their contents. A whole-file read returns `FileError.Limit`
if the requested limit or the hard ceiling is exceeded.

Progressive reads support `read(max_bytes)`, `read_exact(count)`, and
`read_line()`. Exact reads return `EndOfStream` when fewer bytes remain than
requested. `lines()` emits line values lazily; LF and CRLF terminators are
removed, an empty line is preserved, EOF ends iteration, and an error is
emitted once before iteration stops. A line is limited to 1 MiB. `chunks(size)`
emits byte arrays lazily and accepts chunk sizes up to 4 MiB. The iterators use
their parent handle and must remain within the lifetime of that managed file.

```zirk
match File.open(Path.of("events.log"), OpenOptions.read_only()) with file {
    Ok(file) => {
        for line in file.lines() {
            consume(line);
        }
    }
    Error(error) => report(error);
}
```

## Writes, append, and metadata

`write_bytes` and `write_text` create a missing file and replace all contents
of an existing file. They authorize `filesystem.write`; denied writes leave the
existing file untouched. Append operations use native append semantics and
preserve existing contents. Read authority does not grant write authority.

`metadata()` and `metadata_for(path)` expose immutable `FileMetadata` values:
size, whether the item is a regular file or directory, and an optional
modification time in Unix milliseconds. `exists(path)` is fallible so absence,
denial, and host failures remain distinct. These calls use exact-path
`filesystem.metadata` authority. A `filesystem.read` grant also satisfies that
check, while metadata-only consent does not authorize content reads.

Atomic byte/text replacement creates an exclusive temporary beside the target,
writes and syncs it, then renames it into place. A pre-publication failure keeps
the old target. The contract promises atomic visibility where the target's
rename semantics provide it; it does not promise durability across sudden power
loss.

## Errors and support boundary

`FileError` distinguishes invalid path/options, missing path, denied permission,
existing path, wrong type, closed handle, decoding failure, limits, premature
EOF, unsupported operation, and I/O failure. `FileResourceError` adapts close
failures to the language `Resource<E>` contract.

`FileError.Cancelled` reports that a filesystem operation stopped at a
cooperative cancellation safe point. `FileWatchEvent` distinguishes create,
modify, remove, rename, overflow, closure, and a typed `FileError`. Watch events
are best-effort and may be coalesced by the operating system. The queue holds
at most 256 events per watcher. Overflow means the consumer must rescan using
the same approved directory scope; it does not represent a complete history.
Recursive watches install bounded, non-following watches for each directory.
Creating a directory, starting with an existing descendant tree, or removing
or renaming a watched directory can introduce a gap, so the runtime emits
`Overflow` and the consumer should rescan. Symlink targets are not watched or
reported as descendants. Watchers are explicitly closeable and limited to 32
open resources per process and 1,024 watched directories per recursive watcher.

The implementation support matrix is:

| Capability | macOS ARM64 | Linux x86_64 | Linux ARM64 | Windows x86_64 | Browser |
| --- | --- | --- | --- | --- | --- |
| Bounded directory operations, links, metadata | Implemented | Implemented | Implemented | Implemented; native runtime not yet verified | Unsupported |
| Temporary files and directories | Implemented | Implemented | Implemented | `FileError.Unsupported` | Unsupported |
| Synchronous advisory file locks | Implemented | Implemented | Implemented | `FileError.Unsupported` | Unsupported |
| No-replace directory move | Implemented | Implemented | Implemented | `FileError.Unsupported` | Unsupported |
| Directory watchers | Implemented; create/close E2E passed | Implemented; native backend registration checked in emulated x86_64 container | Implemented; native backend registration checked in ARM64 container | `FileError.Unsupported` | Unsupported |
| Async file and directory iteration | Implemented; E2E covers sibling progress, typed errors, open/read/write/append | Runtime filesystem suite passed in emulated x86_64 container; async CLI E2E not run | Runtime filesystem suite and 20 filesystem CLI E2E tests passed in ARM64 container | GNU cross-check and MSVC target/test compilation passed; native Windows execution unverified | Unsupported |
| Async advisory-lock and watcher waits | Implemented; contention and watcher E2E passed | Runtime filesystem suite passed in emulated x86_64 container; async wait E2E not run | Runtime filesystem suite and async lock/watcher E2E tests passed in ARM64 container | `FileError.Unsupported`; MSVC target/test compilation passed | Unsupported |

Watcher creation, recursive-symlink containment, denied consent, bounded
overflow, and close have focused macOS ARM64 tests. Linux ARM64 and emulated
Linux x86_64 runtime filesystem tests check backend registration and invariants;
they do not establish native Linux-hardware execution or watcher event delivery.
Async CLI E2E coverage runs on macOS ARM64 and Linux ARM64. The Linux x86_64
container passed runtime filesystem tests, but its CLI E2E build did not
resolve the pinned LLVM toolchain. Windows
x86_64 `zirk-runtime` cross-compiles for the GNU and MSVC targets, including
test-target compilation and typed unsupported lock and watcher paths; native
Windows runtime execution remains unverified. Browser preflight rejects reachable filesystem effects
before execution and is covered by focused browser tests.

---

**Previous:** [← std.io](02-std-io.md) · **Next:** [std.path →](04-std-path.md)
