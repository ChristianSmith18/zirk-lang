# Name Resolution

Name resolution connects each identifier to one declaration before type checking
uses it. It covers lexical scopes, members, modules, imports, aliases, globals,
packages, generated declarations, and visibility. Resolution never guesses from
spelling when more than one declaration is viable.

> **Implementation status:** the current semantic crate implements lexical
> scopes, shadowing, functions, enums, classes/members, visibility, forward
> declaration collection, and the current import subset. The loader follows a
> multi-file graph, but the driver still temporarily flattens declarations into
> one crate namespace. Full per-module/project/package resolution remains target
> behavior.

## Resolution order

For each compilation unit the frontend conceptually:

1. establishes the project/package and module graph;
2. records published and private declarations without checking bodies;
3. resolves imports and their aliases;
4. creates type/member declarations and callable signatures;
5. resolves inheritance, contracts, and generic constraints;
6. checks bodies with nested lexical scopes;
7. repeats applicable resolution for generated decorator syntax.

Collecting signatures before bodies permits a function or type to refer to one
declared later and supports valid mutual module references. It does not make
every semantic initialization cycle valid.

## Lexical scope

Parameters, local declarations, patterns, loop bindings, catches, lambdas, and
generated hygienic bindings have explicit scopes. An inner declaration may
shadow an outer local where the language permits it; leaving the block removes
that binding.

```zirk
mut name = "outer";

if condition {
    mut name = "inner";
    stdout.println(name); // inner binding
}

stdout.println(name); // outer binding
```

A read before declaration does not search future locals. Definite initialization
is a later flow question; resolution first determines which binding the read
would address.

## Files and modules

Every source file is a private declaration boundary. `share` publishes a
declaration for import; it does not make private members public. A local import
uses a quoted path and standard modules use unquoted names:

```zirk
import { User, Role -> DomainRole } from "./domain/user";
import { stdout } from std.io;
```

An alias changes only the local binding. The exported declaration retains its
original identity. Imports are lexical dependencies and do not execute module
code.

The target resolver preserves a module table rather than flattening files. The
same physical file reached through canonical path aliases is one module. An
import cannot escape the project except through a declared package dependency.

## Standard-library convenience members

Importing a compiler-known standard object can expose its declared convenience
members when unambiguous:

```zirk
import { stdout } from std.io;

println("Hello"); // resolves to stdout.println
```

If a local/imported `println` competes, the unqualified call is diagnosed and
`stdout.println(...)` selects the standard operation. Local and package objects
do not inject all their methods into file scope.

## Globals

`use` enables an application global already declared in `init.zrk`; it does not
load code or infer a path. Libraries cannot declare application globals. Missing
or ambiguous enabled globals fail during resolution before type checking their
uses.

## Members and visibility

Member lookup starts from the receiver's resolved type and follows the defined
class/contract relationships. It distinguishes:

- no member with that name;
- a member that exists but is inaccessible;
- a field used as a callable;
- overload/constructor candidates that exist but do not match later typing;
- ambiguity between inherited/default contract implementations.

`private` is confined to its declaring class, `protected` follows the inheritance
contract, and public/package visibility follows the canonical object and package
specifications. Reflection and decorators cannot bypass the same checks.

## Generated names and hygiene

Decorator-generated private names receive hygienic compiler identities, so a
source local with the same text cannot capture them. Generated public names are
ordinary API: a collision is an error that points to the original declaration,
decorator application, and generating operation.

Expansion resolution occurs after each permitted augmentation/wrapping round.
Generated code cannot depend on a declaration that would be invisible to
handwritten code at that location.

## Cycles

File import cycles are not inherently invalid. Declaration and semantic graphs
are checked separately:

- cyclic file traversal loads each file once;
- valid signature references can be mutual;
- inheritance, generic constraint, decorator dependency, fixture, constant/
  initialization, and other semantic cycles follow their specific rules;
- invalid cycles report the complete relevant path, not only the final edge.

## Diagnostics

Resolution failures use stable categories for undeclared, duplicate, ambiguous,
inaccessible, and invalid import/member names. Helpful diagnostics show the use
site and candidate declaration/import sites, recommend qualification where it
resolves ambiguity, and avoid suggesting a private symbol the caller cannot use.

Recovery creates an internal unresolved binding/type only to continue checking.
It never inserts a real declaration or permits IR lowering.

---

**Previous:** [← Lexer and Parser](02-lexer-and-parser.md) · **Next:** [Type Checker →](04-type-checker.md)
