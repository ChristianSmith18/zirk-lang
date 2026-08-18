# `std.text`

`std.text` complements native `String` and `Char` with construction, formatting,
safe regular expressions and Unicode algorithms. Everyday transformations stay
on the native types; the module owns operations that need an auxiliary type or
compiled program.

> **Implementation status:** accepted Zirk 1.x contract; regex, formatting and
> some Unicode facilities may be ahead of the current compiler/runtime.

## Building text

Repeated `String + String` creates intermediate values. `StringBuilder` provides
amortized linear construction and accepts every value with `to_string()`:

```zirk
mut builder = StringBuilder();
builder.append("User: ");
builder.append(user);
builder.append(' ');
builder.append(status);

inmut text = builder.build();
```

`build()` returns the accumulated `String` and empties the builder for reuse.
`to_string()` returns a non-destructive snapshot. Allocation failure is explicit;
implementation capacity is not a public program concern.

## Formatting

Native interpolation evaluates Zirk expressions inside `{}`. `format` instead
binds declared placeholders beginning with `:`:

```zirk
inmut greeting = "Hello {user.name}";
inmut summary = format(":name has :count messages", name:, count:);
inmut equation = format(":0 + :1 = :2", left, right, result);
```

`\:` emits a literal colon-placeholder prefix. A format follows `|`:

```zirk
format("Total: :amount|.2", amount:);
format("Date: :date|YYYY-MM-DD", date:);
format("ID: :id|08", id:);
```

Each type family defines valid formats through its formatting contract. Literal
templates are checked at compile time for missing/extra arguments and invalid
type-format combinations. Runtime templates use a separate API because Zirk
does not change a function's result type based on whether an argument is a
literal:

```zirk
format_dynamic(template, values): Result<String,FormatError>;
```

## Safe regular expressions

The canonical literal syntax is `re'...'`:

```zirk
inmut identifier = re'[A-Za-z_][A-Za-z0-9_]*';
```

An invalid literal is a compile-time error. Dynamic patterns use
`Regex.parse(pattern): Result<Regex,RegexError>`. The standard engine guarantees
linear-time matching and excludes constructs such as backreferences or unsafe
lookbehind that require catastrophic backtracking. Specialized engines belong
in separate packages.

`Regex` supports `is_match`, `find`, `find_all`, typed captures, `replace` and
`split`. Operations expose match/input limits and never silently truncate.

## Unicode operations

`String` is a mutable Unicode reference whose public sequence is graphemes.
Consequently `split("")` separates graphemes. Splitting preserves empty fields
unless `remove_empty: true` is explicit. `trim`, `split_whitespace` and line
recognition use Unicode semantics; `lines()` recognizes common platform line
terminators, removes them and preserves meaningful empty lines.

Normalization is never automatic:

```zirk
text.normalize(UnicodeNormalization.NFC);
```

NFC, NFD, NFKC and NFKD are available. Default case conversion and ordering are
deterministic and locale-independent; locale-sensitive casing, collation,
pluralization, transliteration and stemming belong to an internationalization
package. Parsing native values returns typed `ParseError`. Text diff belongs to
testing/tooling rather than the runtime text core.

---

**Previous:** [← std.process](05-std-process.md) · **Next:** [ std.collections](06-std-collections.md)
