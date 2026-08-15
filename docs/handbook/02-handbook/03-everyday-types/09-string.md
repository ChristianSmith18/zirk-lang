# String

`String` is a Unicode text sequence indexed semantically by grapheme clusters. This makes ordinary user-visible indexing safer than treating UTF-8 bytes or code points as complete characters.

```zirk
inmut language = "Zirk";
inmut greeting = "Hello, {language}";
```

Interpolation evaluates expressions and applies defined formatting contracts. It is not raw source substitution.

The runtime may maintain an adaptive index or cache internally so repeated grapheme operations do not require identical rescans. That optimization is not observable semantics.

Use byte-oriented APIs for protocols and binary files. Convert with an explicit encoding and handle invalid input rather than assuming bytes are text.

---

**Previous:** [← Char](./08-char.md) · **Next:** [Void, Never, Null, and Object →](./10-void-never-null-object.md)
