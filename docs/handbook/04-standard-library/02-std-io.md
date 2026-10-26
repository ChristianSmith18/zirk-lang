# `std.io`

`std.io` is the synchronous standard-stream package. It publishes the `stdin`,
`stdout`, and `stderr` modules as ordinary Zirk source modules.

```zirk
import { stdin, stdout, stderr } from std.io;

println("ready"); // opened from stdout
stdout.println("explicit");
stderr.println("warning");
```

Importing a module opens its unique shared members. Explicit calls always use
`module.member`; `::` is only used to select a module in an import, for example
`import { println } from std.io::stdout;`.

## Output

Both `stdout` and `stderr` provide:

```zirk
trait Stringifiable {
    to_string(): String { return "<Value>"; }
}
fn print(...args: Stringifiable): Void;
fn println(...args: Stringifiable): Void;
fn flush(): Result<Void, IoError>;
```

`print` emits no line ending and `println` emits exactly one platform line
ending. Arguments are rendered left-to-right, joined by one space, and preserve
call order. `stdout` and `stderr` have separate destinations. `print()` emits
nothing; `println()` emits only a line ending.

`Stringifiable` is a real standard trait: every value-producing type satisfies
it automatically and receives a base `to_string()` implementation. A class may
replace that implementation, but must declare `#override`, just as when it
replaces any trait method with a body.

`print` and `println` are implemented in the bundled Zirk source: each one
calls `arg.to_string()` through the trait while it composes the stream output
and delegates only the final single-text writes to the compiler runtime.
`read_line()` likewise is Zirk code that maps
`read_line_result()` into `String?`. Those runtime boundaries have no keyword
or callable name available to application packages.

```zirk
class Label {
    text: String;

    #override to_string(): String { return this.text; }
}
```

```zirk
stdout.print("count", 2); // count 2
stdout.println();
match stderr.flush() {
    Ok(_) => { },
    Error(error) => stdout.println(error),
}
```

## Input and outcomes

```zirk
enum IoError { Io, Decoding, Limit, EndOfStream }
enum ReadResult<T> { Value(value: T), End, Error(error: IoError) }

stdin.read_line(): String?;
stdin.read_line_result(): ReadResult<String>;
stdin.read_char(): ReadResult<Char>;
stdin.read_bytes(max: UInt64): ReadResult<Array<Byte>>;
stdin.read_exact(count: UInt64): Result<Array<Byte>, IoError>;
```

`read_line()` is the nullable convenience API: it removes `\n` or `\r\n`,
returns an empty string for an empty terminated line, and returns `null` at EOF
or when it cannot return text. Use `read_line_result()` when EOF, decoding, and
host errors must remain distinguishable.

```zirk
match stdin.read_line_result() {
    ReadResult.Value(line) => stdout.println(line),
    ReadResult.End => stdout.println("input completed"),
    ReadResult.Error(error) => stderr.println(error),
}
```

Text reads use strict UTF-8; invalid data reports `IoError.Decoding`.
`read_bytes(max)` reports `IoError.Limit` when its request exceeds the stream
bound. `read_exact(count)` returns `Error(IoError.EndOfStream)` when fewer than
`count` bytes remain.

## Targets and limits

Native programs use the process standard streams. The interpreter receives
bounded injected input and records ordered output events labelled `stdout` or
`stderr`. Browser execution captures those output events but rejects a
reachable stdin operation before execution; it never prompts, touches the DOM,
storage, or network to emulate input.

This delivery is synchronous. Async stream APIs, terminal styling, cursor
control, prompts, raw mode, and progress regions are deferred to later
standard-library work.

---

**Previous:** [← Common API Contracts](01-common-api-contracts.md) · **Next:** [std.fs →](03-std-fs.md)
