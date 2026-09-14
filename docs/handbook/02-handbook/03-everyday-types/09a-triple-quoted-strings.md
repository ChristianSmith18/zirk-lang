# Triple-Quoted Strings

`'''…'''` writes a multiline `String` without joining escaped one-line
literals. Physical spacing is literal, interpolation is active, and the
opening delimiter defines a visible left edge.

```zirk
inmut report =
    '''
    Name: {name}
      Status: active
    ''';
```

The newline immediately after the opening delimiter and the line containing
the close do not contribute text. Each non-empty content line must reach the
opening delimiter's column; exactly that many leading characters are removed.
Extra indentation is content and blank lines are preserved.

`'` and `''` remain literal within the body; the first `'''` closes it.
Escapes (`\n`, `\t`, `\r`, `\0`, `\\`, `\'`, `\{`, `\}`) and `{expression}`
interpolation work exactly as in a double-quoted string. A content line or
closing delimiter left of the edge, an inline closing delimiter, or an
unclosed literal is a diagnostic.

## Canonical layout

`zirk format file.zrk` currently has one intentionally narrow rule: it moves
each triple-quoted literal below the `=`, `(`, or named argument that
introduces it, one indent deeper, aligns both delimiters, and re-indents the
content relative to that edge. `zirk format --check file.zrk` writes nothing
and exits non-zero when a change is needed. Files with syntax errors are never
modified. Other formatting rules remain future work.

The VS Code extension highlights this form and draws a guide at the opening
delimiter column for every line through its close.

---

**Previous:** [← String](09-string.md) · **Next:** [ Void, Never, Null, and Object](10-void-never-null-object.md)
