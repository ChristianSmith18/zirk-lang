# What Is Zirk?

Zirk is a statically typed, general-purpose programming language that compiles to standalone native binaries. It aims to make ordinary application code direct and readable while keeping lower-level control available behind explicit boundaries.

That combination matters because many programs eventually cross abstraction levels. A command-line tool may begin as string processing and later need concurrent work, native interoperability, careful resource cleanup, or predictable distribution. Zirk is designed so those needs belong to one coherent language rather than to unrelated escape hatches.

## A first look

The smallest useful program imports standard output and defines the application entry point:

```zirk
import { stdout } from std.io;

fn main(): Void {
    stdout.println("Hello from Zirk");
}
```

Even this small program exposes several parts of the language:

- `import` names the APIs a file uses.
- `fn` declares a function.
- Parameters and return values have static types; `Void` means this function does not return a value.
- Braces delimit blocks.
- The official formatter writes semicolons, even though the parser may omit them when there is no ambiguity.
- `stdout` is supplied by the standard library rather than being a compiler keyword.

Zirk source files use the `.zrk` extension. An application also has an `init.zrk` manifest that tells the toolchain what it is building:

```zirk
project {
    name: "hello-zirk";
    version: "0.1.0";
    type: application;
    entry: "src/main.zrk";
}
```

The manifest is code-shaped and typed, but it has a specific declarative role. It identifies the entry file, dependencies, permissions, targets, and build settings for the project.

## Static types without constant annotation

Zirk checks types before producing a binary, but local code can use inference when the answer is unambiguous:

```zirk
mut attempts = 0;
inmut service_name = "catalog";

attempts += 1;
```

`mut` allows reassignment. `inmut` prevents reassignment of the binding, while `inmut::strict` requests transitive immutability. These are semantic declarations rather than naming conventions.

Inference does not make the program dynamically typed. The compiler still determines a concrete type and rejects operations that do not belong to it. Public APIs normally make important types visible:

```zirk
fn retry_message(service: String, attempt: Int32): String {
    return "Retrying {service}, attempt {attempt}";
}
```

## Absence and failure are explicit

`null` can inhabit only a nullable type such as `String?`; there is no `undefined`. Safe access uses `?.`, and `??` supplies a fallback. This keeps absence visible in the type system.

Expected failures use `Result<T, E>`. Pattern matching makes both outcomes explicit:

```zirk
mut message: String = match load_profile() {
    Ok(profile) => "Welcome, {profile.name}";
    Error(error) => "Could not load the profile: {error}";
};
```

An expression-valued `match` must cover every possible case. Zirk also has exceptions for exceptional but recoverable situations and `fatalError` for irreparable states. The language does not force all kinds of failure into one mechanism.

## Concurrency is part of the language model

Zirk distinguishes different kinds of concurrent work:

- `task` and `await` express structured asynchronous work.
- `parallel` requests CPU work that may run across cores.
- `thread` represents a real operating-system thread.
- `Channel<T>`, synchronization primitives, and atomics coordinate shared work.

These constructs are not aliases for a public global event loop. The runtime may use an internal reactor, but application code works with structured operations whose lifetime and cancellation rules are explicit.

The distinction helps communicate intent. Waiting for a network response, splitting a CPU-heavy transform across cores, and integrating with thread-affine native code are different problems; Zirk does not pretend they are interchangeable.

## High-level by default, low-level by consent

Memory management is automatic. The compiler and runtime may use stack placement, heap allocation, escape analysis, moves, or reference counting internally, but Zirk 1.x does not expose ownership or reference counting as its public programming model.

Safe code must not permit use-after-free, null dereferences, data races, or undefined behavior. Operations that cannot preserve those guarantees—raw pointers and unsafe casts, for example—belong inside an `unsafe {}` boundary. This makes low-level code possible while keeping the risk visible during review.

Native interoperability follows the same principle. Zirk can call and export native functions, but ABI assumptions, platform constraints, and resource ownership have to be declared rather than inferred from hope.

## What Zirk is for

The initial Zirk 1.x scope targets:

- backend applications and services;
- command-line applications;
- desktop applications;
- systems-oriented programs;
- native libraries.

Builds produce standalone binaries for supported combinations of Windows, Linux, macOS, architecture, linker, SDK, and native dependencies. The initial compiler uses LLVM and supports explicit target selection and cross-compilation when the complete target toolchain is available.

Zirk 1.x does **not** target WebAssembly or browser APIs. It also excludes several ideas that appear in historical design material, including `async fn`, a standalone `worker` primitive, general compile-time blocks, traditional function overloading, and public ownership semantics. The [status chapter](./03-language-status.md) explains how to read normative design separately from implementation progress and historical exploration.

## A language and its toolchain

The official `zirk` command is designed to cover the complete project lifecycle: creating a project, checking code, building, running, testing, formatting, linting, managing dependencies, auditing permissions, packaging, publishing, and generating documentation.

Formatter, linter, language server, and `zirk check` share the incremental compiler frontend. They do not need to invoke LLVM merely to understand source code. This architecture is intended to keep editor feedback fast while preserving a single interpretation of syntax and types.

## The central idea

Zirk's working philosophy is:

> Easy by default, explicit when you need control.

“Easy” does not mean hiding behavior that changes correctness. Nullability, expected failures, unsafe operations, permissions, cancellation, and target constraints remain visible. “Control” does not mean every program must manage memory or threads manually. The language starts from safe, high-level defaults and asks for precision at the boundaries where precision pays for itself.

That trade-off is the thread connecting the rest of this handbook.

## Normative sources

- [Zirk Master Specification](../../ZIRK_SPEC_FINAL.md), especially §§1–6
- [Zirk Language Specification](../../ZIRK_LANGUAGE_SPEC.md)
- [Zirk Compiler and Tooling Specification](../../ZIRK_COMPILER_SPEC.md)

---

**Previous:** [Getting Started](./README.md) · **Next:** [Why Zirk Exists →](./02-why-zirk-exists.md)
