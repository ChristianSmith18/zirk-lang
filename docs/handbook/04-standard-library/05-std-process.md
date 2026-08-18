# `std.process`

`std.process` starts programs without invoking a shell, connects typed streams,
captures results and binds every child to structured cancellation and cleanup.
The executable and its arguments are always separate.

> **Implementation status:** target Zirk 1.x contract. Process, environment and
> shell permissions must be granted separately in `init.zrk`.

## Commands and results

```zirk
inmut operation = Process.command("git")
    .args(["status", "--short"])
    .capture()
    .run(timeout: 5s);

match await operation {
    Ok(result) => inspect(result),
    Error(error) => report(error),
}
```

`run` waits task-safely and returns a `ProcessResult`. A nonzero exit is a
successfully observed process outcome, not a process API failure:

```zirk
if !result.success {
    println(result.stderr_text_lossy());
}
```

`ProcessResult` preserves `stdout` and `stderr` as bytes. Strict text conversion
can return an encoding error; lossy conversion is explicit. Start failures,
missing authority and management failures belong to `ProcessError`.

Executables accept `String` lookup or an explicit `Path`. Permission checks
resolve the canonical executable and declared argument patterns before launch.

## Standard I/O modes

```zirk
enum ProcessIo {
    Inherit;
    Pipe;
    Null;
    File(File);
}
```

`Process.run` inherits terminal streams by default so interactive CLI output is
visible. `.capture()` pipes and buffers bounded output for inspection. Explicit
pipes support streaming and backpressure without loading an unlimited child
output into memory.

```zirk
match Process.command("zirk")
    .arg("build")
    .stdout(ProcessIo.Pipe)
    .spawn() with child {
    Ok(child) => consume_build_output(child),
    Error(error) => report(error),
}
```

## Environment and working directory

Children inherit only a safe functional environment by default, such as
necessary path, locale and terminal information. Secrets and unrelated parent
variables are not copied implicitly.

```zirk
Process.command("deploy")
    .env("DEPLOY_REGION", "us-east-1")
    .current_directory(project_root)
    .run();
```

`inherit_environment()` deliberately requests the full accessible environment,
emits a security warning and requires corresponding authority.
`clear_environment()` starts from an empty set. Secret values remain
`SecretString` until deliberately revealed at the authorized process boundary.

## Managed children

`spawn()` returns `ChildProcess`, a `Resource<ProcessError>`. Scope exit for a
live child performs a bounded cleanup sequence:

1. request cooperative termination;
2. wait for the configured grace duration;
3. force `kill()` if the child does not exit;
4. `wait()` to reap native process state;
5. close every owned pipe and handle.

`terminate()` is cooperative, `kill()` is forceful and `wait()` observes and
reaps the exit. This policy prevents zombies and leaked pipes. A process may
outlive its scope only through explicit privileged `detach()`, which transfers
responsibility rather than silently abandoning it.

Child operations are task-aware cancellation points. Cancelling a task requests
child cleanup and awaits it before the task finishes. Processes therefore obey
the same `Task.all`, `Task.first`, `Task.settled` and `select` policies as other
structured work.

## Pipelines

Typed pipelines connect process streams directly and preserve the exit result
of every stage:

```zirk
inmut pipeline = Process.command("cat")
    .arg("access.log")
    .pipe(Process.command("grep").arg("ERROR"))
    .pipe(Process.command("sort"));

inmut operation = pipeline.capture().run(timeout: 10s);
```

Because methods are first-class, `|>` can compose builders, but fluent `.pipe`
is the documentation convention. Pipeline cancellation closes pipes, applies
backpressure, identifies a stage that could not start and cleans every child.

## Shell execution

No command API interprets quotes, globbing, redirection, substitutions or shell
metacharacters. `Shell.run(command)` is a separate visibly dangerous API with a
separate high-risk permission. Prefer `Process.command` whenever arguments can
be expressed structurally.

## Permissions and portability

Process grants restrict the canonical executable and optionally safe argument
forms. `arguments: all` is possible but warned. Environment access and shell
execution need their own grants. Platform-specific signals and termination
behavior are represented through typed capabilities and documented fallbacks;
portable code must not assume POSIX signal numbers on every target.

---

**Previous:** [← std.path](04-std-path.md) · **Next:** [ std.text](05a-std-text.md)
