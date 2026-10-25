# `std.path`

`Path` is an immutable value for platform paths. The current package is
`std.path::path`.

```zirk
import { Path } from std.path::path;

inmut base = Path.of("project");
inmut source = base.join("src").join("main.zrk");
inmut parent = source.parent();
```

`Path` stores a Zirk `String`; it is not an open resource and does not prove
that its target exists. Construction with `Path.of` and `as_string` are value
conversions. The API deliberately keeps lexical operations separate from host
queries that need filesystem authority.

## Lexical operations

`join`, `components`, `parent`, `name`, `normalize`, and `equals` operate on
path spelling only. They do not inspect files, resolve links, or request
permission. `normalize` removes redundant `.` segments and resolves `..`
lexically where possible. `equals` compares normalized lexical paths; it does
not establish that two path values identify the same file.

Joining an absolute segment follows the host `PathBuf` behavior and can replace
the base. When user input must stay under a trusted directory, validate the
resolved path at the privileged filesystem operation; string checks and
lexical normalization alone are not a security boundary.

## Filesystem-aware queries

`absolute()` resolves a relative path against the process working directory
and returns an absolute normalized spelling. `canonicalize()` consults the
filesystem, resolves existing links, and returns the canonical location.
Both return `Result<Path, PathError>`. `absolute()` requests exact
`filesystem.read` authorization for the resulting path; `canonicalize()` uses
`filesystem.metadata`, which can resolve the physical path without authorizing
content reads. A full read grant also satisfies metadata inspection. Empty,
NUL-containing, missing, denied, unsupported, and other host failures are
represented by `PathError`.

```zirk
match source.canonicalize() {
    Ok(real_path) => use(real_path);
    Error(error) => report(error);
}
```

These queries use host path semantics on macOS, Linux, and Windows. `Path` is
currently string-backed, so a native path that cannot be represented as valid
Zirk text fails conversion rather than being silently rewritten. The API does
not yet expose `PathBuf`, `PathStyle`, `same_file`, `stem`, or extension
operations.

---

**Previous:** [← std.fs](03-std-fs.md) · **Next:** [std.process →](05-std-process.md)
