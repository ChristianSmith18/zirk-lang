# Char

`Char` is an inline semantic value containing exactly one extended Unicode
grapheme: one user-perceived character, not necessarily one code point or byte.

```zirk
inmut latin: Char = 'A';
inmut pi: Char = 'π';
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
'π'.ascii_code();          // -1
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

## API

One extended Unicode grapheme. Case conversion and normalization can expand a
grapheme, so they return `String`, not `Char`.

### Properties

| Member | Type | Description | Status |
| --- | --- | --- | --- |
| `byte_length` | `Int32` | Encoded byte count | implemented |
| `codepoint_count` | `Int32` | Unicode scalar count | implemented |

### Methods

| Signature | Returns | Description | Status |
| --- | --- | --- | --- |
| `value.ascii_code()` | `Int32` | ASCII value, or `-1` when not a single ASCII scalar | implemented |
| `value.is_ascii()` | `Boolean` | Single ASCII scalar | implemented |
| `value.is_alphabetic()` | `Boolean` | Unicode alphabetic | implemented |
| `value.is_letter()` | `Boolean` | Unicode letter | implemented |
| `value.is_numeric()` | `Boolean` | Unicode numeric | implemented |
| `value.is_digit()` | `Boolean` | Unicode digit | implemented |
| `value.is_alphanumeric()` | `Boolean` | Alphabetic or numeric | implemented |
| `value.is_whitespace()` | `Boolean` | Unicode whitespace | implemented |
| `value.is_uppercase()` | `Boolean` | Unicode uppercase | implemented |
| `value.is_lowercase()` | `Boolean` | Unicode lowercase | implemented |
| `value.normalize(form)` | `String` | `form: UnicodeNormalization` — NFC/NFD/NFKC/NFKD | implemented |
| `value.to_uppercase()` | `String` | Unicode-aware uppercase (may expand) | implemented |
| `value.to_lowercase()` | `String` | Unicode-aware lowercase (may expand) | implemented |
| `value.bytes()` | `List<UInt8>` | Byte-level view | implemented |
| `value.codepoints()` | `List<UInt32>` | Scalar-level view | implemented |
| `value.to_string()` | `String` | One-grapheme string | implemented |

### Examples

```zirk
inmut pi: Char = 'π';
pi.is_alphabetic();             // true
'ß'.to_uppercase();             // "SS" — expansion returns String
'A'.ascii_code();               // 65
'👨‍👩‍👧‍👦'.ascii_code();   // -1
```

---

**Previous:** [← Boolean](07-boolean.md) · **Next:** [ String](09-string.md)
