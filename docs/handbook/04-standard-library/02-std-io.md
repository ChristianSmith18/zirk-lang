# `std.io`

`std.io` provides typed text and byte streams, console-oriented printing and
the three process-standard objects: `stdin`, `stdout` and `stderr`.

> **Implementation status:** target Zirk 1.x contract; some terminal and
> task-aware operations may not yet be implemented by the current runtime.

```zirk
import { stdin, stdout, stderr, ReadResult } from std.io;
```

Importing a standard object makes its unique convenience members available
unqualified:

```zirk
println("ready");          // stdout.println("ready")
stderr.println("warning");
```

A local collision requires qualification. No import or convenience call grants
additional permissions.

## Printing values

`print` and `println` accept zero or more values. Each value is converted through
its `to_string()` contract. Multiple values are joined with `separator`, whose
default is one space.

```zirk
fn print(values: Object..., separator: String = " "): Void throws IoError;
fn println(values: Object..., separator: String = " "): Void throws IoError;
```

`print` emits no line ending; `println` appends exactly the platform line ending.
Neither accepts an `end` argument because that distinction belongs to their
names.

```zirk
println("User:", user, "Age:", user.age);
println("a", "b", "c", separator: ", ");
println();
```

The convenience operations propagate a catchable `IoError` if the destination
is closed, a downstream pipe disappears or the operating system rejects the
write. Code that treats output failure as an expected branch uses the explicit
result form:

```zirk
match stdout.try_println(report) {
    Ok(_) => {},
    Error(error) => backup_log(error),
}
```

## Style, formatting and live output

`format` constructs text; `style` controls presentation. Keeping them separate
prevents ANSI or terminal concerns from contaminating stored strings.

```zirk
inmut message = format(
    "{completed}/{total} tasks completed",
    completed:,
    total:,
);

println(message, style: Style.success);
```

`Style` supports semantic presets and explicit foreground/background color,
bold, dim, italic and underline where available. Terminal output detects its
capabilities, respects `NO_COLOR` and falls back to plain text when redirected.

Cursor movement, live regions and progress belong to `Terminal`, not to
`println`:

```zirk
mut progress = Terminal.progress(total: files.length);

for file in files {
    compile(file);
    progress.advance();
}

progress.finish("Compilation completed");
```

The terminal implementation coordinates live regions with ordinary log writes
so that concurrent task output does not permanently corrupt the display.

## `print` versus `write`

Printing is a presentation API: it converts values, inserts separators and may
apply terminal style. `write` is the exact stream primitive: it adds nothing,
performs no arbitrary object conversion and accepts text or bytes supported by
the destination.

```zirk
stdout.write("progress: ");
stdout.write(percent.to_string());
stdout.write("\r");

file.write(bytes);
socket.write(packet);
```

Each individual call is atomic with respect to other calls on the same standard
stream. A sequence of calls is not a transaction. Use `stream.lock` when a
multi-call console record must remain contiguous.

## Reading and EOF

EOF is data, not an exception. Text and scalar reads return an exhaustive
algebraic result without using `?`:

```zirk
enum ReadResult<T> {
    Value(T);
    End;
    Error(IoError);
}
```

```zirk
match stdin.read_line() {
    Value(line) => process(line),
    End => println("input completed"),
    Error(error) => report(error),
}
```

`Value("")` means an empty line and remains distinct from `End`. `read_line`
removes the terminating line break. The input API distinguishes storage and
Unicode units:

```zirk
stdin.read_byte(): ReadResult<Byte>;
stdin.read_code_point(): ReadResult<CodePoint>;
stdin.read_grapheme(): ReadResult<Char>;
stdin.read_line(): ReadResult<String>;
```

An exact-count operation uses `Result` because early EOF violates its requested
contract:

```zirk
stream.read_exact(size: 128);
```

## Buffering, tasks and cancellation

Streams may be wrapped in explicit readers and writers with configurable buffer
limits. `flush()` moves buffered data to the operating-system boundary; it does
not promise durable storage.

Blocking operations have `_async` variants when a task-aware alternative is
needed:

```zirk
inmut pending: Task<ReadResult<String>> = stdin.read_line_async();
inmut result = await pending;
```

Cancellation closes only resources owned by the operation and preserves the
standard stream. Extracting `stdout.println` creates an independent safe
callable capability and never allows replacement or mutation of the original
standard object:

```zirk
mut log = stdout.println;
log("bound safely");
```

---

**Previous:** [← Common API Contracts](01-common-api-contracts.md) · **Next:** [ std.fs](03-std-fs.md)
