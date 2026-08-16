# Char

`Char` is an inline semantic value containing exactly one extended Unicode
grapheme: one user-perceived character, not necessarily one code point or byte.

```zirk
inmut latin: Char = 'A';
inmut accent: Char = 'é';
inmut family: Char = '👨‍👩‍👧‍👦';
```

Each literal above is one `Char`. An empty literal or a literal containing two
graphemes is invalid. A grapheme can contain combining marks, several Unicode
scalars and many encoded bytes.

## Properties and low-level views

- `byte_length` reports the encoded byte count;
- `codepoint_count` reports Unicode scalar count;
- `bytes()` and `codepoints()` expose explicit lower-level iteration;
- `to_string()` produces a one-grapheme String.

`ascii_code(): Int32` returns an ASCII value only when the grapheme consists of
exactly one scalar in the ASCII range; otherwise it returns `-1`:

```zirk
'A'.ascii_code();          // 65
'0'.ascii_code();          // 48
'é'.ascii_code();          // -1
'👨‍👩‍👧‍👦'.ascii_code(); // -1
```

## Unicode API

`is_ascii()`, `is_alphabetic()`, `is_numeric()`, `is_alphanumeric()`,
`is_whitespace()`, `is_uppercase()`, `is_lowercase()`, `normalize(form)`,
`to_uppercase()` and `to_lowercase()` provide Unicode-aware behavior. Case
conversion returns `String`, because Unicode mapping can expand one grapheme
into multiple graphemes.

Char supports equality and deterministic locale-independent ordering. It has no
numeric arithmetic: use explicit code-point/byte APIs when implementing an
encoding or protocol.

---

**Previous:** [← Boolean](07-boolean.md) · **Next:** [ String](09-string.md)
