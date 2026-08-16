# Lexical Rules

The lexer turns source characters into tokens before parsing establishes grammatical structure. Zirk is case-sensitive: `result`, `Result`, and `RESULT` are different identifiers.

Whitespace separates tokens but otherwise does not create blocks. Braces do:

```zirk
if ready {
    start();
}
```

String and character literals preserve their own contents; comments are ignored as executable tokens. Numeric literals may contain `_` separators and scientific notation. Duration literals such as `500ms` and `5s` are typed values, not a number followed by an arbitrary identifier.

The lexer must retain source spans so later diagnostics can point to the exact token and generated transformations can map back to original code.

Invalid characters or unterminated literals should fail lexically with a location and repair guidance. Later stages should not guess at a different token sequence merely to continue silently.

---

**Previous:** [← Source Files](01-source-files.md) · **Next:** [ Comments and Documentation](03-comments-and-documentation.md)
