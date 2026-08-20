# Build a CLI

This tutorial creates `word-count`, a small application that validates one path,
reads strict UTF-8, counts graphemes, and reports every expected failure.

> **Status:** it uses the target Zirk 1.x stdlib. The current compiler may not
> execute every API yet; the owning chapters define the contract.

## Project

```text
word-count/
├── init.zrk
├── src/main.zrk
└── test/arguments.spec.zrk
```

```text
project {
    name: "word-count";
    version: "0.1.0";
    type: application;
    entry: "src/main.zrk";
}

permissions {
    filesystem {
        read: { paths: ["./input/**"]; during: runtime; }
    }
}
```

## Program

```zirk
import { File } from std.fs;
import { Path } from std.path;
import { System } from std.system;
import { stderr, stdout } from std.io;

fn run(arguments: List<String>): Result<Void, CliError> {
    if arguments.length != 1 {
        return Error(CliError.Usage("word-count <path>"));
    }

    mut path = Path(arguments[0]);
    match File.read_text(path, limit: 8MiB) {
        Ok(text) => {
            stdout.println(text.length);
            return Ok();
        }
        Error(error) => return Error(CliError.File(error));
    }
}

fn main(): Void {
    match run(System.arguments) {
        Ok(_) => {}
        Error(error) => stderr.println(error);
    }
}
```

`String.length` counts graphemes, not bytes. `Path` owns path semantics, and
the read limit prevents an untrusted file from allocating without bound.

## Verify

Open the file with `match with`, count Unicode text deliberately, print the result through `std.io`, and grant only read access to the selected scope. Add `.spec.zrk` tests for parsing and an E2E test for exit code and output.

```console
zirk check
zirk format
zirk lint
zirk test --all
zirk build
```

Test zero, one, and extra arguments in the pure `run` boundary. Add an E2E test
for a UTF-8 fixture, denied path, invalid encoding, size limit, output, and
nonzero failure exit policy. See [std.fs](../04-standard-library/03-std-fs.md),
[std.system](../04-standard-library/17-std-system.md), and
[Result](../02-handbook/15-errors/01-result.md).

## Completion contract

- **Prerequisites:** bindings, `Result`, project manifests, `std.fs`, and
  `std.system`.
- **Expected success:** one grapheme count on stdout and a successful exit.
- **Failure recovery:** usage and file errors go to stderr; no partial artifact
  is created.
- **Next:** build the streaming [file processor](02-build-a-file-processor.md).

---

**Previous:** [← Tutorials](README.md) · **Next:** [ Build a File Processor](02-build-a-file-processor.md)
