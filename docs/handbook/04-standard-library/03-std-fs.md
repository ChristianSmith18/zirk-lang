# `std.fs`

`std.fs` provides bounded file reads, direct writes, managed open handles, and
progressive operations for reading, appending, metadata, and atomic replacement.
The current package is `std.fs::fs`; paths are values from `std.path::path`.

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

The intended native support matrix is macOS ARM64, Linux x86_64, Linux ARM64,
and Windows x86_64. Current evidence includes native macOS ARM64 runtime tests
and Windows x86_64 cross-compilation; Windows runtime execution and Linux
cross-compilation/runtime checks remain unverified. Directory traversal and
mutation, links, temporary directories, locks, watchers, async file operations,
and non-Unicode native paths remain future work.

---

**Previous:** [← std.io](02-std-io.md) · **Next:** [std.path →](04-std-path.md)
