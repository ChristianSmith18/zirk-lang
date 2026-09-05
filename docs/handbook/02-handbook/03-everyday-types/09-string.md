# String

`String` is Zirk's native mutable Unicode reference. Its public sequence is made
of graphemes (`Char` values), while bytes, code points, caching and allocation
remain runtime details unless explicitly requested.

```zirk
mut language = "Zirk";
inmut greeting = "Hello, {language}";
```

`length` counts graphemes. Interpolation evaluates expressions through their
formatting contracts; it is not raw source substitution.

## Reference identity and cloning

Assignment shares an instance:

```zirk
mut first = "hello";
mut second = first;
second[0] = 'H';

first == "Hello";  // true: content equality
first is second;   // true: same reference
```

`clone()` makes independent logical content:

```zirk
mut copy = first.clone();
copy[0] = 'Y';
first == "Hello";
copy == "Yello";
first is copy; // false
```

## Binding permissions

`mut` permits reassignment and content mutation. `inmut` fixes the binding but
still permits element/slice mutation. `inmut::strict` prohibits both and cannot
yield a mutable alias or be acquired while one remains accessible.

```zirk
inmut fixed = "hello";
fixed[0] = 'H'; // valid
fixed = "bye";  // error

inmut::strict frozen = "hello";
frozen[0] = 'H';       // error
mut safe = frozen.clone(); // valid independent copy
```

## Indexing and slicing

Positive indices count from the start; negative indices count from the end.
Every bound is checked—Zirk never clamps an invalid index silently.

```zirk
text[0]
text[-1]
text[start:end:step]
text[:end]
text[start:]
text[::-1]
```

Individual assignment requires one `Char`. Slice assignment requires exactly
the same number of graphemes on both sides, preserving sequence length:

```zirk
mut text = "hola";
text[0] = 'H';       // valid
text[0:2] = "ca";   // valid
text[1:3] = "i";    // slice length mismatch
text[0] = "Hello";  // expected Char
```

## Concatenation and contextual conversion

Only two Strings concatenate directly:

```zirk
"value=" + "42"; // valid
"value=" + 42;   // type error
```

Convert explicitly with `String(42)`/`to_string()`, or establish an explicit
deep context around the concatenation tree:

```zirk
String("value=" + 42); // "value=42"
```

The context converts operands before `+`; it does not leak outward or enter
called function bodies.

## Repetition

`String * Integer` and `Integer * String` return a new repeated String:

```zirk
"ja" * 3; // "jajaja"
3 * "ja"; // "jajaja"
"ja" * 0; // ""
```

The count must be a non-negative integer. Negative/non-integer counts and a
result too large to represent or allocate are controlled errors. `*=` stores a
new result and therefore requires a `mut` binding.

## Ordering and API

Operators use deterministic locale-independent Unicode ordering. Human
linguistic sorting uses an explicit locale-aware collation API.

The native API includes `length`, `byte_length`, `is_empty()`, `contains()`,
`starts_with()`, `ends_with()`, `find()`, `replace()`, `trim()`,
`trim_start()`, `trim_end()`, `to_lowercase()`, `to_uppercase()`, `split()`,
`lines()`, `substring()`, `normalize()`, `bytes()`, `codepoints()`, `chars()`,
`clone()` and `to_string()`. Transforming methods return a String/value or view
as documented; mutation occurs only through explicit assignment/mutating APIs.

## API

`String` is the native mutable grapheme-indexed reference. `length` counts
graphemes; `byte_length` counts encoded bytes.

### Properties

| Member | Type | Description | Status |
| --- | --- | --- | --- |
| `length` | `Int32` | Grapheme count | implemented |
| `byte_length` | `Int32` | Encoded byte count | implemented |

### Methods

| Signature | Returns | Description | Status |
| --- | --- | --- | --- |
| `text.is_empty()` | `Boolean` | `length == 0` | implemented |
| `text.contains(needle)` | `Boolean` | Substring test | implemented |
| `text.starts_with(prefix)` | `Boolean` | Prefix test | implemented |
| `text.ends_with(suffix)` | `Boolean` | Suffix test | implemented |
| `text.find(needle)` | `Int32?` | First grapheme index of `needle`, or `null` | implemented |
| `text.replace(needle, replacement)` | `String` | New string with all occurrences replaced | implemented |
| `text.trim()` | `String` | Removes Unicode whitespace at both ends | implemented |
| `text.trim_start()` / `text.trim_end()` | `String` | One-sided trims | implemented |
| `text.to_lowercase()` | `String` | Unicode-aware lowercase | implemented |
| `text.to_uppercase()` | `String` | Unicode-aware uppercase | implemented |
| `text.split(separator)` | `List<String>` | Split, preserving empty fields unless `remove_empty: true` | specified — pending `List<T>` |
| `text.split_whitespace()` | `List<String>` | Unicode-whitespace split | specified |
| `text.lines()` | `List<String>` | Line terminators removed; meaningful empty lines kept | specified |
| `text.substring(start, end?)` | `String` | Grapheme-range copy; bounds checked | implemented |
| `text.normalize(form)` | `String` | `form: UnicodeNormalization` | implemented |
| `text.bytes()` | `Iterator<UInt8>` | Byte view | specified |
| `text.codepoints()` | `Iterator<UInt32>` | Scalar view | specified |
| `text.chars()` | `Iterator<Char>` | Grapheme view | specified |
| `text.clone()` | `String` | Independent logical copy | implemented |
| `text.to_string()` | `String` | Identity | implemented |
| `String(value)` | `String` | Explicit conversion; also establishes a deep conversion context over a concatenation tree | implemented |

> Indexing `text[i]` yields `Char`; slicing `text[start:end:step]` copies.
> Negative indices count from the end and every bound is checked — Zirk never
> clamps silently. Slice assignment requires equal grapheme counts on both
> sides.

### Examples

```zirk
mut text = "hola";
text[0] = 'H';                  // "Hola"
text.contains("ol");            // true
text.substring(0, 2);           // "Ho"
"ja" * 3;                       // "jajaja"
stdout.println("len={\"πa\".length}");  // graphemes, not bytes
```

---

**Previous:** [← Char](08-char.md) · **Next:** [ Void, Never, Null, and Object](10-void-never-null-object.md)
