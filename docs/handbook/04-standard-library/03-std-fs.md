# `std.fs`

`std.fs` provides bounded whole-file helpers and resource-oriented streaming
APIs for files, directories, metadata, permissions, links, temporary storage,
locks and change notification. Filesystem operations accept `Path`; accepting a
`String` is convenience conversion, never textual path concatenation.

> **Implementation status:** target Zirk 1.x contract. The current runtime may
> implement only a subset. Every example requires an appropriate filesystem
> grant in `init.zrk`.

## Opening files

`OpenOptions` is typed and closed by default. Every capability starts as
`false`; attempting to open with no capability is a diagnostic.

```zirk
OpenOptions(
    read: Boolean = false,
    write: Boolean = false,
    append: Boolean = false,
    truncate: Boolean = false,
    create: Boolean = false,
    create_new: Boolean = false,
);
```

```zirk
inmut options = OpenOptions
    .read_and_write()
    .with_create(true);

match File.open(path, options) with file {
    Ok(file) => update(file),
    Error(error) => report(error),
}
```

Presets include `read_only`, `write_only`, `read_and_write`, `append`, `create`,
`create_new` and `truncate`. Builder methods return new immutable options.
`create_new` atomically fails if the target exists and is preferred over an
unsafe `exists`-then-create sequence.

`File` implements `Resource<FileError>`. Its close is idempotent; managed scope
exit closes it on every control path. A closed handle returns or raises its
documented closed-resource failure rather than touching a recycled native
handle.

## Whole-file and streaming operations

Static methods are the documentation convention:

```zirk
File.read(path, limit: 32MiB);
File.read_text(path, encoding: Encoding.Utf8, limit: 8MiB);
File.write(path, bytes);
File.write_text(path, text, encoding: Encoding.Utf8);
```

Equivalent free functions may exist for pipelines, and static methods remain
first-class callables:

```zirk
path |> File.read_text |> parse_config;
```

Whole-file reads require an explicit or safely bounded allocation limit. Large
or untrusted files use resource APIs:

```zirk
match File.open(path, OpenOptions.read_only()) with file {
    Ok(file) => {
        for chunk in file.chunks(size: 64KiB) {
            consume(chunk);
        }
    },
    Error(error) => report(error),
}
```

Files support bytes, chunks, lines, exact reads and seeking. `read_exact(n)`
fails on premature EOF. Text defaults to strict UTF-8; invalid input returns an
encoding error unless a deliberately lossy decoder is requested. `String`
operations remain grapheme-oriented even though interchange defaults to UTF-8.

`Encoding` includes UTF-8, endian-specific UTF-16/UTF-32, ASCII and Latin-1.
Additional legacy encodings may ship in an official package rather than expand
the runtime core.

## Writing, append and durability

Direct writes may leave partial output if interrupted. Atomic writes create a
temporary sibling, persist it as requested and replace the destination through
the strongest same-filesystem operation available:

```zirk
File.write_text_atomic(path, configuration);
```

Append opens with operating-system append semantics. One API call is treated as
one logical append where the platform can guarantee it; several calls are not a
transaction, and very large writes may be split by the platform. Use an
exclusive `FileLock` to protect a multi-call record.

`flush()` drains Zirk's userspace buffer. `sync_data()` requests durable file
content. `sync_all()` additionally requests relevant metadata durability and is
usually more expensive. The platform chapter documents where hardware or
filesystem guarantees are weaker.

## Directories, links and metadata

The initial contract includes:

- `Directory.create` and `create_all`;
- lazy `entries` and recursive `walk`;
- `copy`, `move`, `remove` and visibly destructive `remove_all`;
- `is_empty`, bounded recursive `size` and `temporary`;
- `File.copy`, `move`, `remove`, `metadata`, links and temporary files;
- `Metadata`, `Entry`, `FileType`, filesystem `Permissions` and timestamps.

Recursive operations do not follow symbolic links by default. Following them
is explicit and includes cycle/depth limits. `exists(path)` returns
`Result<Boolean,FsError>` because absence differs from insufficient authority
to inspect the path.

## Locks

Advisory locks coordinate cooperating processes:

```zirk
match File.lock(path, mode: LockMode.Exclusive) with lock {
    Ok(lock) => update_shared_file(path),
    Error(error) => report(error),
}
```

`Shared` permits cooperating readers; `Exclusive` permits one writer.
`try_lock` returns immediately, `lock` may block, and `lock_async` suspends a
task. Advisory means another program can deliberately ignore the protocol.

## Reactive filesystem watchers

`Directory.watch` creates a managed native watcher. Its notification API is
task-aware without introducing `async fn` or `async for`:

```zirk
match Directory.watch("src", recursive: true, debounce: 100ms) with watcher {
    Ok(watcher) => {
        loop {
            match await watcher.next() {
                Event(Created(path)) => rebuild(path),
                Event(Modified(path)) => rebuild(path),
                Event(Removed(path)) => invalidate(path),
                Event(Renamed(from, to)) => update_path(from, to),
                Overflow => rescan(),
                Closed => break,
                Error(error) => report(error),
            }
        }
    },
    Error(error) => report(error),
}
```

`await watcher.next()` suspends the current task; a separate `task` block is
used only when watching must run concurrently with other work. Cancellation
interrupts the wait, closes scope-owned watcher state and awaits cleanup.
Native systems may coalesce, duplicate or overflow notifications, so `Overflow`
requires a rescan rather than pretending the event history is complete.

## Errors, permissions and races

`FileError`/`FsError` distinguish not found, denied, already exists, wrong type,
invalid path, closed resource, encoding failure, allocation/size limit,
unsupported operation, lock contention and system failure. Filesystem grants
separate read and write patterns. Checks canonicalize `.`/`..`, apply symlink
policy and use handle-relative/native atomic operations where necessary to
avoid validation/use races.

---

**Previous:** [← std.io](02-std-io.md) · **Next:** [ std.path](04-std-path.md)
