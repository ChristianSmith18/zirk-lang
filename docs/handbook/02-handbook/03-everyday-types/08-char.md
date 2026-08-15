# Char

`Char` represents one Unicode code point, not one byte and not necessarily one user-perceived character.

```zirk
inmut letter: Char = 'Z';
inmut symbol: Char = 'λ';
```

A grapheme may contain multiple code points, so text editing should use `String`'s grapheme-aware operations rather than assuming that one displayed character always equals one `Char`.

Conversions between `Char`, numeric code points, bytes, and strings are explicit where information or encoding rules matter. An invalid Unicode scalar value cannot produce a valid `Char`.

---

**Previous:** [← Boolean](./07-boolean.md) · **Next:** [String →](./09-string.md)
