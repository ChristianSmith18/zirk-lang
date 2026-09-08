# `std.json`

`std.json` converts strict JSON text, a mutable in-memory JSON tree and ordinary
typed Zirk values without runtime reflection. Every public JSON type begins with
the canonical uppercase acronym: `JSONValue`, `JSONNumber`, `JSONCodec<T>`,
`JSONLimits`, `JSONPath`, `JSONPointer` and the `JSON*Error` families.

> **Implementation status:** this chapter specifies the accepted Zirk 1.x API.
> Generated codecs and streaming support may be ahead of the current compiler.

## The JSON value tree

```zirk
enum JSONValue {
    Null;
    Boolean(Boolean);
    Number(JSONNumber);
    String(String);
    Array(List<JSONValue>);
    Object(Map<String, JSONValue>);
}
```

`JSONNumber` preserves the exact legal JSON number representation instead of
immediately losing precision through `Float`. Its `to_int32`, `to_int64`,
`to_uint64` and `to_float` conversions return typed results; `to_decimal` —
the exact conversion — is specified but not yet implemented. JSON and Zirk both
reject `NaN` and infinities. Object member order follows deterministic Map
insertion order.

## Parse, stringify and typed conversion

The API mirrors the familiar JSON workflow while adding static conversion:

```zirk
JSON.parse(text): Result<JSONValue,JSONParseError>;
JSON.stringify(value): Result<String,JSONEncodeError>;
JSON.decode<User>(text): Result<User,JSONDecodeError>;
JSON.to_value(user): Result<JSONValue,JSONEncodeError>;
JSON.from_value<User>(value): Result<User,JSONDecodeError>;
```

There is no redundant `JSON.encode`. `parse` produces the generic Zirk tree;
`decode<T>` produces a requested native type. `stringify` accepts a JSON tree or
a typed value with a compiler-known codec. `to_value`/`from_value` expose the
intermediate tree when a program needs to transform it.

```zirk
record User {
    id: UInt64;
    name: String;
}

inmut user = User(id: 42, name: "Cristian");

match JSON.stringify(user) {
    Ok(text) => println(text),
    Error(error) => report(error),
}
```

Result:

```json
{"id":42,"name":"Cristian"}
```

Calling the conversion explicitly requests compile-time codec synthesis when
the type is structurally eligible. Reusable or customized codecs are generated
as ordinary typed code or implemented manually; the runtime never scans fields.

## Strict JSON parsing

`JSON.parse` and `JSON.parse_bytes` accept standard JSON only. Bytes must be
valid UTF-8. Comments, trailing commas, single-quoted strings, unquoted keys,
hexadecimal, `NaN` and infinity are rejected. JSON5 belongs to a separate
package.

Duplicate object keys always produce `JSONDuplicateKeyError`; there is no
keep-first/keep-last compatibility mode. This prevents parsers and security
layers from interpreting the same document differently.

`JSONParseError` contains a stable code, message, line, column, byte offset and
`JSONPath`. A bounded source excerpt may be included after secret redaction;
the error never retains an unlimited input document.

## Defensive limits

```zirk
JSONLimits(
    max_bytes: 8MiB,
    max_depth: 128,
    max_string_bytes: 1MiB,
    max_array_items: 100_000,
    max_object_members: 100_000,
    max_number_characters: 1_024,
);
```

Every entry point has safe defaults and accepts explicit limits. Whole inputs
are size-checked before parse where possible; streaming stops when a limit is
reached. A limit failure returns `JSONLimitError` and never exposes a partial
document as success.

## Reading and modifying a tree

Parsing creates an in-memory Zirk value; it does not retain or automatically
modify the source file. A program may edit the tree and later stringify/write
it:

```zirk
match JSON.parse(source) {
    Ok(value) => {
        mut document = value;
        document["enabled"] = JSONValue.Boolean(true);

        match JSON.stringify(document) {
            Ok(updated) => File.write_text(path, updated),
            Error(error) => report(error),
        }
    },
    Error(error) => report(error),
}
```

Direct mismatched access raises the documented controlled runtime error;
`get` returns `Result`. `JSONPath` provides typed error/navigation locations and
`JSONPointer` implements the small standard `/users/0/name` notation. Advanced
JSONPath queries and JSON Patch remain packages.

Projection rules are ordinary Zirk rules: reading a nested value creates an
independent logical copy, while an expression used as a mutation place updates
the original tree.

## Formatting and canonical form

`JSON.stringify` is compact. `JSON.stringify_pretty(value, indent: 2)` uses
configurable indentation. Valid Unicode is emitted directly; mandatory/control
characters are escaped, and `ascii_only: true` deliberately emits Unicode
escapes. `JSON.canonicalize` uses fixed key, number and escaping rules for
signatures/hashes and is separate from presentation formatting.

Cycles cannot be represented by JSON. Encoding a cyclic object graph returns
`JSONCycleError` with the path at which the repeated reference was detected.

## Typed codecs and decorators

`JSONCodec<T>` converts between `T` and `JSONValue`. An explicit call may ask the
compiler to synthesize a codec for structurally eligible public data. Classes
with construction/invariants use generated or authored codecs that invoke the
public constructor/factory rather than bypassing memory safety.

Standard attribute decorators customize generated code:

```zirk
@JSON
class User {
    @JSONName("user_id")
    id: UInt64;

    name: String;

    @JSONIgnore
    password_hash: String;

    @JSONDefault("guest")
    role: String;
}
```

`@JSON` is valid on a class. Records do not acquire a new decorator target: their
codec is requested/generated explicitly, while their attributes may use the
supported attribute-target configuration when a generator processes them.
Decorator applications disappear after expansion and produce ordinary
`JSONCodec<User>` code.

A missing field is an error unless it is nullable, explicitly optional or has
`@JSONDefault`. Missing and explicit `null` remain distinct. Unknown fields are
errors by default; `@JSONAllowUnknown` on a supported class codec or the
equivalent explicit codec policy opts into ignoring them.

Traditional Zirk enums are written as `Status.Active`, but valid JSON encodes
the case name as the generated string `"Active"`; JSON has no unquoted enum
value. No declaration-order integer is inferred. Algebraic enums default to
configurable discriminated objects. Date/time, Duration and bytes require
explicit ISO/base64 codecs.

## Streaming and task-aware I/O

`JSONReader` consumes tokens or root-array elements incrementally;
`JSONWriter` emits without constructing a complete text value. Parsing itself
is CPU work. Reading a file/socket remains visibly task-aware:

```zirk
inmut chunk = await body.read();
reader.push(chunk);
```

No synchronous iterator hides `await`. The underlying writer applies
backpressure, and its task-aware operation is explicit when suspension is
required. Limits, cancellation and typed errors apply throughout; cancellation
does not publish a partial decoded value as success.

## Deliberate exclusions

JSON Schema, advanced JSONPath, JSON Patch, JSON5, YAML/TOML/XML, ORM mapping and
domain validation are packages. `std.json` supplies the secure interoperable
tree, codecs and streaming foundation on which those packages can build.

---

**Previous:** [← std.http](12-std-http.md) · **Next:** [ std.crypto](14-std-crypto.md)
