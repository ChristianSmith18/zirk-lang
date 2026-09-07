# `std.testing`

`std.testing` provides compile-time-discovered tests, typed fixtures, direct and
fluent assertions, explicit snapshots, and benchmark declarations. The `zirk
test` runner executes unit and end-to-end workloads without adding private
access, permissions, mutable global environment, or release-binary code.

Tests use the same language, type system, visibility, resource, task, and
permission rules as ordinary source files. Testing APIs improve observation and
reporting; they do not weaken the program being tested.

> **Implementation status:** this page defines the target Zirk 1.x module and
> discovery contract. The dedicated [Testing](../08-testing/README.md) unit owns
> runner isolation, CI, deterministic facilities, and benchmark methodology in
> greater depth.

## Imports and discovery

Assertions are explicit imports:

```zirk
import { assert, expect } from std.testing;
```

The compiler recognizes `@test`, `@e2e`, `@bench`, suite hooks, and fixtures in
their valid testing files without another import. They are testing declarations,
not retained runtime annotations.

- `@test` is valid only in files ending in `.spec.zrk`;
- `@e2e` is valid in `test/*.e2e.zrk`;
- `@bench` is valid in runner-recognized benchmark files;
- using one outside its context is a compile-time diagnostic.

Test declarations and generated runner registries are excluded from release
binaries.

## Assertion style

Zirk supports both direct `assert` and fluent `expect`:

```zirk
@test
fn calculates_total() {
    assert.equal(calculate_total(order), 120);
}
```

```zirk
@test
fn creates_active_user() {
    mut user = create_user();

    expect(user.name).to_equal("Ada");
    expect(user.active).to_be_true();
    expect(user.roles).to_contain(Role.Admin);
}
```

Official examples prefer `assert` for small, direct checks. `expect` is useful
when several readable expectations describe one subject. A test should use one
vocabulary consistently unless mixing them materially improves clarity.

All assertion methods use `lower_snake_case`. Helpers preserve the call-site
source span so diagnostics identify the expression in the test, not only the
implementation inside `std.testing`.

## Test outcomes

A test can return any of:

```zirk
Void
Result<Void, E>
Task<Void>
Task<Result<Void, E>>
```

The runner waits for `Task` results without introducing `async fn`. A returned
`Error`, thrown `AssertionError`, unexpected throwable, timeout, cancellation,
or leaked resource produces a failed test with its distinct category.

An assertion aborts only the current case, after which cleanup runs. Independent
tests continue according to the selected runner failure policy. The runner
captures `AssertionError`; outside a runner it remains an ordinary throwable.

## Core direct assertions

The minimum direct vocabulary includes:

```zirk
assert.equal(actual, expected);          // `==`
assert.not_equal(actual, unexpected);
assert.identical(first, second);         // `is`
assert.not_identical(first, second);
assert.is_true(condition);
assert.is_false(condition);
assert.is_null(value);
assert.is_not_null(value);
assert.contains(collection, value);
assert.empty(collection);
assert.length(collection, expected);
assert.sequence_equal(actual, expected);
```

Equality uses the type's ordinary equality contract. Identity uses observable
identity and is rejected for types that do not support it. Collection failures
show the relevant missing, unexpected, or differently ordered elements rather
than only printing `false`.

Floating comparison requires explicit tolerance:

```zirk
assert.approx(actual, expected, tolerance: 0.001);
```

There is no hidden global epsilon. Invalid, negative, or meaningless tolerance
is rejected.

## Results and throwables

`Result` assertions preserve the enum's `Ok`/`Error` model:

```zirk
assert.is_ok(result);
assert.is_error(result);
expect(result).to_be_ok(expected);
expect(result).to_be_error<NotFoundError>();
```

Expected throwable checks accept a closure so invocation remains explicit:

```zirk
assert.throws<ValidationError>(fn() => validate(input));
expect(fn() => validate(input)).to_throw<ValidationError>();
```

Named options can constrain stable error code, message, cause, or another public
error property. A type match alone does not silently swallow a different cause
or infrastructure failure outside the closure's observed execution.

## Grouped assertions

Assertions stop at the first failure by default. Explicit `assert.all` evaluates
independent checks and reports every captured assertion failure:

```zirk
assert.all([
    fn() => assert.equal(user.name, "Ada"),
    fn() => assert.is_true(user.active),
]);
```

Each closure must be an assertion-only observation. A non-assertion throwable,
cancellation, fatal error, or external failure stops the group immediately and
remains primary. Grouping does not create parallel execution or roll back side
effects.

## Test configuration

All decorator arguments are optional:

```zirk
@test(
    name: "creates a user",
    tags: ["user", "unit"],
    timeout: 2s,
)
fn creates_user() {}
```

The function name is the default display name. Tags are normalized, stable
filtering labels rather than permissions or runtime attributes. Timeout is a
`Duration` and bounds the whole case, including fixtures and cleanup according
to the documented cleanup grace policy.

## Suites

`@suite` is Zirk's compile-time equivalent of a `describe` group:

```zirk
@suite(name: "UserService")
class UserServiceTests {
    @before_each
    prepare() {}

    @test(name: "creates an active user")
    creates_active_user() {}

    @test(name: "rejects a duplicate email")
    rejects_duplicate_email() {}

    @after_each
    cleanup() {}
}
```

`@test(name:)` changes one case's display name; it does not form a suite. The
compiler discovers suite membership, validates hooks, and emits a typed runner
registry without executing a `describe` function at startup. Reports use paths
such as `UserService > creates an active user`.

Suites do not grant test methods access to private production declarations. A
suite may manage its own private test state under ordinary class visibility.

## Parameterized cases

Cases bind tuples to typed parameters in declaration order:

```zirk
@test(cases: [
    (2, 2, 4),
    (3, 4, 7),
])
fn adds(left: Int, right: Int, expected: Int) {
    assert.equal(left + right, expected);
}
```

Arity and types are checked during compilation. Each case receives its own
report identity and cleanup cycle. A failing case does not hide the remaining
case identities, and filtering can select the owning test without changing the
case data.

## Hooks and cleanup

Supported suite hooks are:

```zirk
@before_all
fn start_suite() {}

@before_each
fn prepare() {}

@after_each
fn cleanup() {}

@after_all
fn stop_suite() {}
```

Setup runs outer-to-inner and cleanup unwinds inner-to-outer. `after_each` runs
when the case succeeds, fails, throws, or is canceled; `after_all` runs after
the suite's attempted cases. A test failure and cleanup failure are both
preserved using the standard primary/suppressed or resource-failure rules.

Hooks can return the same supported outcome families as tests. Duplicate hooks,
invalid signatures, ambiguous ordering, and lifecycle cycles are compile-time
errors.

## Typed fixtures

Fixtures are explicit typed producers:

```zirk
@fixture
fn database(): Result<TestDatabase, FixtureError> {
    return TestDatabase.create();
}

@test
fn creates_user(database: TestDatabase) {
    assert.is_ok(database.create_user("Ada"));
}
```

The runner resolves a fixture by compatible type and visible name. Ambiguity,
missing fixtures, incompatible scopes, and dependency cycles are diagnosed
during compilation. Fixtures can depend on other fixtures through parameters.

A fixture returning a managed resource transfers ownership to the case or suite
scope selected by its declaration. Cleanup is automatic and bounded. Fixture
values do not become ambient globals and cannot escape their lifetime.

## Fakes, spies, and replacement

General mocking, monkey patching, and replacement of global functions are not
part of `std.testing`. Tests substitute behavior through interfaces, traits,
first-class functions, explicit dependencies, generated typed adapters, or
small handwritten fakes.

Testing helpers may provide typed observation/spying around a callable or
interface implementation, but cannot rewrite an arbitrary global symbol,
violate visibility, suppress permission effects, or modify a standard-library
operation in place.

## Snapshots

Snapshots are explicit assertions:

```zirk
expect(rendered).to_match_snapshot();
```

The default run compares but never rewrites snapshot files. Updates require:

```text
zirk test --update-snapshots
```

The runner displays additions, removals, and modifications before writing.
Snapshot storage is deterministic and associated with source identity and case
name. Secrets, unstable addresses, unauthorized environment values, and other
sensitive diagnostics remain redacted. Snapshot access requires the applicable
test filesystem permission and is unavailable to release code.

## End-to-end tests

E2E declarations exercise public application boundaries:

```zirk
@e2e(name: "serves the health endpoint", tags: ["http"])
fn serves_health_endpoint(): Task<Result<Void, TestError>> {
    // Start through documented public APIs and verify the observable boundary.
}
```

They receive no implicit filesystem, network, environment, process, shell,
native, or secret authority. Their declared test policy must grant each effect,
and noninteractive CI fails closed when approval is absent.

## Runner commands

The initial command surface is:

```text
zirk test
zirk test --unit
zirk test --e2e
zirk test --all
zirk test --file <path>
zirk test --tag <tag>
zirk test --seed <seed>
zirk test --jobs <n>
zirk test --report human|json|junit
zirk test --update-snapshots
```

`--tag` selects declarations carrying a tag. Selection never grants their
permissions or changes semantics.

`--seed` fixes runner-controlled randomization, generated/property-test data,
and other explicitly deterministic test facilities. Every affected failure
prints its reproduction seed. It does not control `SecureRandom` or
cryptographic behavior.

`--jobs` bounds concurrent test execution. `1` is serial; the default derives
from `System.available_parallelism` and runner resource limits. Suite and test
isolation constraints can reduce concurrency but never exceed the requested
bound.

`--report human` is the default readable output. `json` is a stable structured
format for Zirk tooling and CI. `junit` provides ecosystem interoperability.
Reports include suite/case identity, status, duration, source, failure, diff,
and seed while redacting secrets and sensitive paths.

## Benchmarks

`@bench` declarations run only through `zirk bench`, not ordinary tests or
release startup. They use warmup, multiple samples, optimization-aware
dead-code prevention, statistical summaries, and machine-readable output. A
benchmark result records target, toolchain, profile, hardware context, input,
sample count, variance, and seed where relevant.

Testing and benchmarking remain distinct: assertions answer correctness;
benchmarks measure evidence under a recorded environment. Benchmark details are
deepened in the dedicated testing unit.

## Visibility, permissions, and isolation

A test can access exactly what an ordinary source file at the same visibility
boundary can access. It receives no friend/private access. Test compilation can
include test-only public helpers, but production privacy does not disappear.

Tests receive no undeclared permissions, do not mutate the real process
environment, and do not silently share ports, directories, clocks, randomness,
or mutable globals. Runner-provided isolated facilities remain explicit typed
fixtures. Permission denial, sandbox failure, test failure, and infrastructure
failure remain separate report and exit categories.

## Failure diagnostics

Failures show the original source location, evaluated actual and expected
values, a type-appropriate diff, suite/case path, seed, and relevant bounded
task/fixture context. Collection, text, JSON, and multiline values use structured
diffs. Output is bounded and truncation is identified rather than silently
hiding which side differed.

No diagnostic prints `SecretString`, authorization fields, private environment
values, or unapproved filesystem contents. Machine-readable reports preserve
the same redaction guarantees as terminal output.

---

**Previous:** [← std.crypto](14-std-crypto.md) · **Next:** [std.reflect →](16-std-reflect.md)
