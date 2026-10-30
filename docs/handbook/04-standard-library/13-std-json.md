# `std.json`

`std.json` converts strict JSON text to ordinary Zirk values and serializes
JSON-compatible Zirk values. It does not expose a second JSON object tree:
objects are `Object`, arrays are `List<Dynamic>`, and scalar values remain
ordinary Zirk values. A tagged internal number preserves the exact JSON number
text until a checked numeric conversion is requested.

## Parse and stringify

```zirk
import { stdout } from std.io;
import { JSON } from std.json;

inmut value: Dynamic = JSON.parse("\\{\\\"name\\\":\\\"Ada\\\",\\\"active\\\":true\\}");
inmut json: String = JSON.stringify(value);
inmut pretty: String = JSON.stringify_pretty(value);
```

`JSON.parse(document)` returns `Dynamic`. Object properties can be read through
`Object` and narrowed with a checked cast. Arrays contain dynamic values. JSON
`null` becomes Zirk `null`. The parser rejects malformed JSON, duplicate keys,
documents larger than 16 MiB, and nesting deeper than 128 levels.

Call `JSON.parse_with_limits(document, limits)` to choose limits for a parse.
`JSONLimits.defaults()` preserves the standard 16 MiB document and depth-128
defaults; callers can also set string bytes, per-container item count, and
numeric token bytes. Values above the runtime hard ceilings fail with a
catchable `JsonConversionError` before the parsed value is materialized:

```zirk
import { JSON, JSONLimits } from std.json;
import { stdout } from std.io;

fn main(): Void throws JsonConversionError {
    inmut limits: JSONLimits = JSONLimits(1048576, 64, 65536, 10000, 1024);
    inmut document: String = "\{\"status\":\"ok\"\}";
    inmut value: Dynamic = JSON.parse_with_limits(document, limits);
}
try { main(); } catch JsonConversionError(error) { stdout.println(error.message()); }
```

`JSON.stringify(value)` accepts JSON-compatible dynamic values, ordinary
objects, lists, and scalar values. It preserves parsed number tokens, emits
Unicode directly when permitted by JSON, and rejects duplicate keys at parse
time, cycles, callables, resources, and other values that JSON cannot
represent. Failures throw a catchable `JsonConversionError`; serialization
never returns partial output.
`JSON.stringify_pretty(value)` writes the same value model with deterministic
two-space indentation. It uses the shared default serialization limits, and
fails with `JsonConversionError` if the value is unsupported, cyclic, or
exceeds a limit. `JSON.stringify_with_limits` remains the compact form with
caller-selected limits, including the output-byte ceiling.

`JSON.canonicalize(value)` emits RFC 8785 JSON Canonicalization Scheme output:
object keys are sorted by UTF-16 code units, strings are not Unicode-normalized,
and number output follows the JCS binary64 profile. It throws
`JsonConversionError` when a value is unsupported or an exact parsed number
would change when represented in the JCS/I-JSON domain. For example,
`9007199254740993` is rejected instead of being rounded to a neighboring
binary64 value. Canonicalization also follows the shared default JSON limits.

## JSON Pointer and JSONPath

`JSON.pointer(value, pointer)` applies an RFC 6901 string pointer to an
ordinary JSON-compatible `Dynamic` value and returns the selected value as
`Dynamic`:

```zirk
inmut document: Dynamic = JSON.parse("\{\"users\":[{\"name\":\"Ada\"}]\}");
inmut name: Dynamic = JSON.pointer(document, "/users/0/name");
```

The empty pointer selects the root. Object names use exact Unicode string
comparison, `~1` decodes to `/`, and `~0` decodes to `~`. Array indexes use
canonical non-negative decimal notation. Invalid pointers and paths that do
not select a value throw a catchable `JsonConversionError`.

`JSON.query(value, expression)` evaluates an RFC 9535 JSONPath expression and
returns selected values as `List<Dynamic>`. A valid query with no matches
returns an empty list. Invalid syntax, unsupported functions, or query and
result-budget exhaustion throw `JsonConversionError`.

## JSON Schema

`JSON.validate(value, schema)` validates a JSON-compatible value against JSON
Schema Draft 2020-12 Core and Validation and returns a list of issues. Schemas
without `$schema` use Draft 2020-12. Local references and bundled 2020-12
meta-schemas resolve without network access. External resources must be supplied
explicitly with `JSON.validate_with_resolver(value, schema, resources)`; the
runtime never fetches remote references implicitly. Issues include RFC 6901
instance pointers, and shared document limits apply to the schema, instance,
and supplied resources.

## JSON5

`JSON.parse5(document)` parses the separate JSON5 grammar, including comments,
unquoted identifier keys, single-quoted strings, trailing commas, and JSON5
number forms. Results use the same ordinary `Dynamic`, `Object`, and
`List<Dynamic>` representation as strict JSON. JSON5's last-value-wins rule
applies to duplicate object keys. `JSON.parse` remains strict JSON.

Object literals use Zirk's regular object syntax and can be sent directly:

```zirk
import { JSON } from std.json;
import { stdout } from std.io;

fn main(): Void throws JsonConversionError {
    inmut name = "Ada";
    inmut body: Object = {
        status: "ok",
        "{name}": true
    };
    inmut encoded: String = JSON.stringify(body);
    stdout.println(encoded);
}
try { main(); } catch JsonConversionError(error) { stdout.println(error.message()); }
```

Identifier keys become literal strings. Quoted interpolated keys use normal
string interpolation and conversion. Computed access uses a string expression,
for example `body[key]`. JSON serialization is separate from ordinary object
rendering.

## Typed codecs

Typed conversion is explicit and uses the `JSONCodec<T>` contract. A manual
codec controls validation and construction. A concrete class that implements
`JSONCodec<T>` with both methods omitted requests compile-time generation;
generation validates the declared schema and recursively converts supported
field types without scanning runtime objects.

```zirk
import { JSON, JSONCodec } from std.json;
import { stdout } from std.io;

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
        inmut object: Object = { name: value.name };
        return object;
    }
}

fn main(): Void throws JsonConversionError {
    inmut document = "\{\"name\":\"Ada\"\}";
    inmut codec: PersonCodec = PersonCodec();
    inmut person: Person = JSON.decode<Person>(document, codec);
    inmut encoded: String = JSON.encode(person, codec);
}
try { main(); } catch JsonConversionError(error) { stdout.println(error.message()); }
```

A codec's `decode` method may validate a dynamic object against one or more
contracts before constructing its result. Missing properties, incorrect types,
and contract violations use ordinary Zirk errors and can be caught by the
caller. `JSON.encode` first asks the codec for an ordinary JSON-compatible value
and then delegates to `JSON.stringify`.

Generated codecs preserve field defaults when a property is absent. When JSON
supplies an immutable defaulted field, the generated construction path decodes
that value before the instance is complete; it does not mutate the field after
construction. Manual `Codec implements JSONCodec<T>` implementations remain
unchanged, and a partial manual implementation is rejected rather than mixed
with generated methods. Generated support is partial until the full native
target matrix passes. The [generated codec fixtures](../../../crates/zirk-cli/tests/corpus/valid/json_generated_codec_field_defaults.zrk)
cover omitted and supplied immutable defaults on the local native/interpreter
paths.

## Limits and errors

Strict parsing enforces a 16 MiB input limit, a nesting limit of 128, valid JSON
syntax, and unique object keys after escape decoding. For example, `"x"` and
`"\\u0078"` are duplicate keys. `JSONLimits` configures document bytes, depth,
decoded string bytes, items per container, and numeric-token bytes; hard runtime
ceilings still apply. Parsing and serialization enforce the relevant limits
before unbounded value-tree or output-buffer growth. Failures use
`JsonConversionError`, which callers can handle with `try`/`catch`.

Strict parse/stringify, caller-limited parse and stringify, JSON Pointer,
JSONPath, pretty output, RFC 8785 canonical output, Draft 2020-12 schema
validation, and JSON5 parsing are implemented. Typed conversion supports
explicit `JSONCodec<T>` implementations and compile-time generation for the
currently supported field and enum compositions. The public `JSONStreamReader`
accepts `String` and `Array<Byte>` chunks; bytes may split a UTF-8 scalar. It
feeds a bounded one-chunk channel into the parser and retains no second full
source-document buffer. The public pull `JSONStreamWriter` serializes on a
worker into a bounded one-chunk channel instead of materializing the serialized
document. Its output can be consumed by any caller-managed sink; the tested
filesystem integration demonstrates that a caller may combine synchronous
chunk pulls with its own I/O strategy. The JSON reader and writer methods are
synchronous Zirk calls and do not require `task`/`await`; asynchronous JSON
stream adapters are deferred. Limits, cancellation, terminal state, and
serialization failures have explicit outcomes. The parser still materializes
the final JSON value tree, as required by the ordinary `Dynamic` API.

Executable evidence: [native/interpreter JSON parity](../../../crates/zirk-cli/tests/object_engine_parity.rs), [JSON corpus fixture](../../../crates/zirk-cli/tests/corpus/valid/json_stringify_dynamic.zrk), [public synchronous streaming fixture](../../../crates/zirk-cli/tests/corpus/valid/json_streaming_public_api.zrk), and [runtime-core JSON conformance and stream tests](../../../crates/zirk-runtime-core/src/json.rs).

---

**Previous:** [← std.http](12-std-http.md) · **Next:** [std.crypto →](14-std-crypto.md)
