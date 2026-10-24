# `std.json`

`std.json` converts strict JSON text to ordinary Zirk values and serializes
JSON-compatible Zirk values. It does not expose a second JSON object tree:
objects are `Object`, arrays are `List<Dynamic>`, and scalar values remain
ordinary Zirk values. A tagged internal number preserves the exact JSON number
text until a checked numeric conversion is requested.

## Parse and stringify

```zirk
import { JSON } from std.json;

inmut value: Dynamic = JSON.parse("\\{\\\"name\\\":\\\"Ada\\\",\\\"active\\\":true\\}");
inmut json: String = JSON.stringify(value);
```

`JSON.parse(document)` returns `Dynamic`. Object properties can be read through
`Object` and narrowed with a checked cast. Arrays contain dynamic values. JSON
`null` becomes Zirk `null`. The parser rejects malformed JSON, duplicate keys,
documents larger than 16 MiB, and nesting deeper than 128 levels.

`JSON.stringify(value)` accepts JSON-compatible dynamic values, ordinary
objects, lists, and scalar values. It preserves parsed number tokens, emits
Unicode directly when permitted by JSON, and rejects duplicate keys at parse
time, cycles, callables, resources, and other values that JSON cannot
represent. Failures throw a catchable `JsonConversionError`; serialization
never returns partial output.

Object literals use Zirk's regular object syntax and can be sent directly:

```zirk
inmut name = "Ada";
inmut body: Object = {
    status: "ok",
    "{name}": null
};
inmut encoded: String = JSON.stringify(body);
```

Identifier keys become literal strings. Quoted interpolated keys use normal
string interpolation and conversion. Computed access uses a string expression,
for example `body[key]`. JSON serialization is separate from ordinary object
rendering.

## Typed codecs

Typed conversion is explicit and uses the `JSONCodec<T>` contract. The codec
controls validation and calls the normal constructor, so JSON processing never
bypasses class invariants or scans runtime fields.

```zirk
import { JSON, JSONCodec } from std.json;

class Person {
    name: String;
    construct(name: String) { this.name = name; }
}

class PersonCodec implements JSONCodec<Person> {
    decode(value: Dynamic): Person {
        inmut object: Object = value as Object;
        return Person(object.name as String);
    }

    encode(value: Person): Dynamic {
        return { name: value.name };
    }
}

inmut codec: PersonCodec = PersonCodec();
inmut person: Person = JSON.decode<Person>(document, codec);
inmut encoded: String = JSON.encode(person, codec);
```

A codec's `decode` method may validate a dynamic object against one or more
contracts before constructing its result. Missing properties, incorrect types,
and contract violations use ordinary Zirk errors and can be caught by the
caller. `JSON.encode` first asks the codec for an ordinary JSON-compatible value
and then delegates to `JSON.stringify`.

## Limits and errors

The current parser enforces a 16 MiB input limit, a nesting limit of 128, valid
JSON syntax, and unique object keys after escape decoding. For example,
`"x"` and `"\\u0078"` are duplicate keys. Parse and serialization failures use
`JsonConversionError`, which callers can handle with `try`/`catch`.

Configurable limits, pretty/canonical output, streaming reader/writer APIs,
JSON Pointer, JSON Schema, and JSON5 are not part of the current `std.json`
surface.

---

**Previous:** [← std.http](12-std-http.md) · **Next:** [std.crypto →](14-std-crypto.md)
