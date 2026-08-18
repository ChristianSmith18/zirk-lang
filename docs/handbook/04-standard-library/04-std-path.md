# `std.path`

`Path` is an immutable value describing a platform path. It is neither a raw
`String` nor an open filesystem resource, and it may represent a target that
does not yet exist.

```zirk
import { Path, PathStyle } from std.path;

inmut base = Path("project");
inmut source = base.join("src").join("main.zrk");
// base remains "project".
```

Zirk exposes one optimized immutable `Path` rather than a public `Path`/
`PathBuf` split. Reassignment of a `mut` variable replaces the value; path
members do not mutate internal segments.

## Lexical operations

Construction, `join`, `components`, `parent`, `name`, `stem`, `extension`,
`with_name`, `with_extension` and lexical `normalize` do not access the
filesystem and require no filesystem permission.

`normalize()` removes redundant separators and resolves lexical `.`/`..`
segments without following links or proving existence. It must not be confused
with authorization or canonicalization.

## Filesystem-aware resolution

`absolute()` resolves a relative path against an explicit or current working
directory. `canonicalize()` consults the filesystem, resolves links and returns
a real normalized location; it can fail and requires read authority for the
traversed scope.

```zirk
match path.canonicalize() {
    Ok(real_path) => use(real_path),
    Error(error) => report(error),
}
```

Path equality is lexical after the type's defined normalization rules. It does
not claim that two spellings identify one filesystem object. Use
`Path.same_file(left, right)` for that fallible filesystem query.

## Native representation and Unicode

`Path` preserves native non-Unicode path data where the platform permits it.
`to_string()` is fallible; `to_string_lossy()` explicitly accepts replacement.
Filesystem operations consume `Path` directly, so ordinary programs do not
need a lossy conversion.

The default parser follows the current target's rules. Tools generating paths
for another target use explicit `PathStyle.Unix` or `PathStyle.Windows`.
Dynamic decisions use `std.system.Platform` (`os`, `arch`, `family`, separators
and executable/library extensions) instead of ad-hoc environment inspection.

## Security boundary

Textual normalization is not a permission check. Privileged operations resolve
and validate the effective path using symlink-aware, race-resistant filesystem
mechanisms. User input should be joined beneath an authorized base and passed to
the filesystem API as `Path`, never concatenated into a string.

---

**Previous:** [← std.fs](03-std-fs.md) · **Next:** [ std.process](05-std-process.md)
