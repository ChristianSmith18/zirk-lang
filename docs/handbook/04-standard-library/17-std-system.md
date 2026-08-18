# `std.system`

`std.system` exposes the small, portable part of the current process and target
platform that applications commonly need. It deliberately avoids exposing
runtime internals or fingerprinting data as ambient authority. Environment
variables belong to [`std.environment`](18-std-environment.md), child processes
to [`std.process`](05-std-process.md), and filesystem paths to
[`std.path`](04-std-path.md).

> **Implementation status:** this chapter defines the target Zirk 1.x contract.
> Unsupported platform properties and signals must be represented explicitly;
> implementations must not guess or expose sensitive identifiers as fallback.

## Imports

```zirk
import { System, Platform, Signal, Signals } from std.system;
```

`System` contains current-process operations. `Platform` describes host and
target environments. `Signal` and `Signals` provide the portable shutdown
surface.

## Platform properties

The common platform properties are read-only values:

```zirk
stdout.println(Platform.os);
stdout.println(Platform.arch);
stdout.println(Platform.family);
stdout.println(Platform.path_separator);
stdout.println(Platform.line_separator);
stdout.println(Platform.executable_extension);
stdout.println(Platform.library_extension);
stdout.println(Platform.is_unix);
stdout.println(Platform.is_windows);
```

`Platform.os` uses `OperatingSystem`, whose portable cases include `Linux`,
`MacOS`, `Windows`, `Android`, `iOS`, `FreeBSD`, and `Unknown`. `Platform.arch`
uses `Architecture`, with conventional technical cases such as `X86_64`,
`AArch64`, and `Wasm32`. Unknown or newer targets remain representable without
being mislabeled as an existing platform.

`family` describes broad path and system conventions, not feature availability.
Portable code tests an explicit capability or handles `UnsupportedError`
instead of assuming that every Unix-family target exposes the same operation.

## Host and target

Cross-compilation makes the machine running the compiler different from the
machine that will run the program:

```zirk
mut build_machine = Platform.host;
mut output_machine = Platform.target;

if build_machine.os != output_machine.os {
    stdout.println("cross-compiling");
}
```

Both descriptors expose `os`, `arch`, `family`, and `abi`. `host` is the
toolchain execution environment; `target` is the selected output environment.
Ordinary application code normally uses `target`. Build tooling must choose
deliberately and must not inspect the host to infer target behavior.

## Current process

The non-sensitive current-process surface is:

```zirk
System.process_id
System.arguments
System.executable
System.current_directory()
System.available_parallelism
```

`process_id`, `arguments`, `executable`, and `available_parallelism` are
read-only snapshots. `current_directory()` is an operation returning
`Result<Path, SystemError>` because the directory may have been removed, become
unavailable, or fail platform decoding. Changing the process-wide current
directory is excluded: libraries use explicit paths, while child commands use
`Process.command(...).current_directory(...)`.

`available_parallelism` is a scheduler hint and is never a promise of dedicated
physical cores. It respects container, affinity, and runtime restrictions where
the platform exposes them.

## Sensitive system information

Total memory, detailed CPU model, current user name, host name, device serials,
stable machine identifiers, hardware inventories, and similar fingerprinting
data are not freely exposed by `System` or `Platform`. A future or specialized
API may expose a necessary item through a narrowly scoped permission, returning
a typed denial when authority is absent.

Portable platform selection never requires those values. A library cannot use
`std.system` to silently build a device fingerprint.

## Ordered shutdown and immediate exit

Returning from `main` is the normal termination path. It lets root tasks finish,
resources close in reverse acquisition order, managed threads join, and output
flush within configured bounds.

`System.exit(code)` terminates immediately:

```zirk
import { System } from std.system;

if configuration_is_unrecoverable {
    System.exit(78);
}
```

The same operation can be imported directly:

```zirk
import { exit } from std.system;

exit(78);
```

Immediate exit is reserved for process boundaries and emergency command-line
paths. It does not promise ordinary structured cleanup, application `finally`
execution, or complete buffered output. Code that can return a status from
`main` should do so instead.

## Shutdown signals

Native signal handlers cannot safely execute arbitrary Zirk callbacks. The
runtime translates supported termination signals into a task-aware event flow:

```zirk
for signal in Signals.shutdown_events() {
    match signal {
        Signal.Interrupt => stdout.println("interrupt requested");
        Signal.Terminate => stdout.println("termination requested");
        _ => {}
    }
}
```

`Signal.Interrupt` maps to `SIGINT` or the platform equivalent;
`Signal.Terminate` maps to `SIGTERM` or its equivalent. Other portable cases may
be exposed when the runtime can guarantee their semantics. Platform-only signal
numbers do not become portable enum cases.

The first shutdown signal marks the root supervisor as shutting down, cancels
root scopes, wakes cancellable waits, closes resources, joins managed threads,
flushes standard output within a bound, and exits. A second shutdown signal or
an exhausted shutdown deadline forces a controlled exit. Ordinary application
configuration cannot disable that escape path and leave a hung process.

`shutdown_events()` is a bounded broadcast-style flow: observing it does not
take ownership away from the root supervisor, and a slow observer cannot block
shutdown. Installing native handlers or handling arbitrary OS signals belongs
to an explicit low-level, platform-specific API with appropriate permissions.

## Errors and portability

`SystemError` covers unavailable current-process state, decoding failures,
unsupported capabilities, and permission denial through specific variants.
Portable enum cases and property meanings remain stable; numeric signal values,
path encodings, ABI spellings, and exact exit behavior after operating-system
failure are platform-specific.

---

**Previous:** [← std.reflect](16-std-reflect.md) · **Next:** [`std.environment` →](18-std-environment.md)
