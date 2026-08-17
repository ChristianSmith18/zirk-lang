# Zirk Memory, References, and Unsafe Semantics

This document is the normative source of truth for managed memory, reference
lifetime, cloning, native views, pointers, unsafe operations, rollback, and
irreversible effects. The handbook teaches these rules progressively; compiler
and runtime documents describe how their respective layers enforce them.

## 1. Public memory model

Zirk manages ordinary memory automatically. A program reasons about values,
references, identity, mutation, scope, and observable lifetime—not stack versus
heap, collector generations, ownership counters, or allocation regions.

The runtime must reclaim unreachable managed memory, including cycles and
concurrent structures. It may combine tracing collection, generations, regions,
escape analysis, physical moves, and reference counting. None of these choices
may change:

- structural equality through `==`;
- reference identity through `is`;
- the value/reference and projection rules;
- mutation permitted by `mut`, `inmut`, and `inmut::strict`;
- whether a safe reference remains valid;
- deterministic resource cleanup.

A live object keeps its identity even when the runtime relocates its storage.
Safe code cannot observe a managed object's address.

Managed objects do not have general-purpose observable destructors. Memory
reclamation cannot be used to close a file, socket, process, lock, or other
external resource. `Resource<E>` and `match with` own deterministic cleanup.

## 2. Values, complete references, projections, and places

Assigning, passing, returning, or capturing a complete reference shares the
referent:

```zirk
mut first = User(name: "Ada");
mut second = first;

second.name = "Grace";
println(first.name); // Grace
```

Reading inside a reference—an attribute, index, slice, destructured component,
iterator item, or projected closure capture—produces an independent logical
value. If that value is reference-backed, the read performs its deep `Clone`:

```zirk
mut users = [User(name: "Ada")];
mut selected = users[0];

selected.name = "Grace";
println(users[0].name); // Ada
```

A place expression on the left of assignment retains access to original
storage:

```zirk
users[0].name = "Grace"; // writes through the place
```

If a projected reference-backed type does not implement `Clone`, reading it as
an independent value is a compile-time error. An API must offer an explicit
borrowed view or ownership-transfer operation instead.

## 3. Mutability and alias safety

- `mut` permits binding replacement and, for references, referent mutation.
- `inmut` prevents binding replacement but does not freeze reachable state.
- `inmut::strict` prevents binding replacement and mutation anywhere in the
  reachable graph.

The compiler must reject construction of a writable alias that would break a
strict guarantee. Strictness is not defined as cloning: it is an observable
transitive promise that may use representation sharing internally.

## 4. Safe and weak references

Safe references are non-null, remain valid for their proven lifetime, and never
expose reclaimed storage. Nullable safe values use `T?`; raw native nullability
belongs to `Pointer<T>`.

`Weak<T>` observes a managed reference without keeping it alive:

```zirk
mut weak = Weak.from(service);

match weak.upgrade() {
    Some(live) => live.refresh(),
    None => println("service was reclaimed"),
}
```

Its core API is:

```zirk
class Weak<T> {
    static fn from(value: T): Weak<T>;
    fn upgrade(): Option<T>;
    inmut is_alive: Boolean;
}
```

`is_alive` is only an observation. Another execution context may release the
last strong reference immediately afterward; safe use always goes through
`upgrade()`.

## 5. Dependent lifetimes and automatic pinning

Some values depend on storage owned elsewhere:

- a view into a native buffer;
- a resource-derived handle;
- a borrowed iterator or internal-storage view;
- a native reference created while a managed object is pinned.

Such a value cannot escape its owner or be used after the owner closes, moves,
or releases the relevant storage. Zirk tracks these constraints internally and
does not expose lifetime parameters in 1.x.

Validated native borrowing automatically pins movable managed storage for the
borrow's bounded extent. Ordinary users do not construct `Pin<T>`. A pin cannot
escape, cross an unsupported suspension point, or outlive the native operation.

## 6. Deep clone contract

`clone()` is available only through `Clone`. For a reference graph it:

- creates new identity for every cloned reference object;
- preserves internal sharing inside the new graph;
- reproduces cycles without infinite recursion;
- never retains an alias to a cloned mutable source node;
- fails at compile time if the declared graph includes a resource, raw pointer,
  lock, task, or another non-`Clone` member.

If `a.left is a.right`, then after `mut b = a.clone()`, `b.left is b.right` is
true while `b.left is a.left` is false.

## 7. Raw pointers

`Pointer<T>` represents a native address and may be null. It never implicitly
becomes a safe reference. Its low-level operations require `unsafe`:

```zirk
unsafe {
    if pointer.is_null {
        return Error(NativeError("null pointer"));
    }

    mut value = pointer.read();
    mut next = pointer + 1; // one T element
}
```

Pointer arithmetic is measured in `T` elements. Byte arithmetic requires a
`Pointer<Byte>` or explicit `offset_bytes`. Representation casts use
`pointer.cast<T>()` and must establish size, alignment, lifetime, and aliasing
requirements. Integers never cast directly into safe references.

The core pointer surface includes:

```zirk
pointer.is_null
pointer.read()
pointer.write(value)
pointer.offset(elements)
pointer.offset_bytes(bytes)
pointer.cast<T>()
pointer.read_volatile()
pointer.write_volatile(value)
```

Volatile access models device or externally observed memory. It is not a
replacement for concurrent synchronization.

## 8. Validated native views

Prefer bounded views over a loose pointer/length pair:

```zirk
unsafe {
    mut view: NativeSliceMut<Byte> = match pointer.as_slice_mut(length) {
        Ok(validated) => validated,
        Error(error) => return Error(error),
    };
    view[0] = 0x7f;
}
```

`NativeSlice<T>` is read-only; `NativeSliceMut<T>` permits writes. Construction
validates nullability, alignment, extent, provenance where available, mutation
rights, and owner lifetime. Bounds checks remain active in safe operations over
the resulting view. The view does not own or free the storage.

## 9. Closed unsafe operation set

The following require an explicit unsafe boundary:

- constructing or dereferencing raw pointers;
- raw pointer arithmetic and representation casts;
- calling an `unsafe fn` or unsafe native declaration;
- constructing a value from unvalidated native memory;
- reading an untagged native union member;
- requesting weak atomic memory ordering;
- manually implementing an internal compiler safety contract.

`unsafe` does not disable name resolution, typing, scope, mutability,
permissions, resource rules, or operating-system validation.

An unsafe function declares an unsafe calling contract:

```zirk
unsafe fn read_header(pointer: Pointer<Byte>): Header {
    unsafe {
        return parse_header(pointer.read());
    }
}
```

Calling it requires `unsafe`. Its body still uses visible unsafe blocks around
dangerous expressions, keeping audits local.

## 10. Transactional unsafe blocks

An ordinary unsafe block is a transaction for:

- Zirk-managed state;
- validated native ranges represented by a bounded mutable view.

Writes remain private until success. On normal completion the runtime commits
them. On a controlled `Error` propagated from the block, exception, checked
runtime trap, or cancellation before commit, it:

1. closes resources acquired inside the transaction;
2. restores logged native ranges;
3. restores managed locations;
4. propagates the original outcome, composing cleanup failures normally.

```zirk
mut status = "ready";

mut result = unsafe {
    status = "updating";
    mut view = match pointer.as_slice_mut(length) {
        Ok(validated) => validated,
        Error(error) => return Error(error),
    };
    view[0] = marker;
    match validate(view) {
        Ok(_) => {},
        Error(error) => return Error(error),
    }
};
// On Error, status and view[0] have their original values.
```

Rollback is observable behavior; its implementation is not. The runtime may
combine first-write journals, copy-on-write, range snapshots, escape analysis,
static commit points, and merged nested transactions. It must not clone the
entire reachable heap merely to enter a block.

## 11. Isolation restrictions

A reversible unsafe transaction cannot:

- `await` or otherwise suspend;
- spawn a task or thread;
- publish tentative state to another execution context;
- perform an irreversible external effect;
- use a raw write whose provenance and extent cannot be proven.

These restrictions ensure no observer sees a value that may later roll back.
They also make transaction journals bounded and avoid locks spanning suspension.

## 12. Irreversible `commit`

Effects that cannot honestly be undone require an explicit commit region:

```zirk
unsafe {
    mut packet = match validate_packet(pointer, length) {
        Ok(validated) => validated,
        Error(error) => return Error(error),
    };
    state.last_packet = packet.id;

    commit {
        socket.send(packet.bytes());
    }
}
```

Entering `commit` publishes all pending reversible writes. The following belong
inside this boundary:

- filesystem, network, process, shell, and device effects;
- unknown-effect native library calls;
- volatile or memory-mapped device writes;
- manual native release;
- concurrently observable publication;
- raw writes without proven provenance and extent.

Permissions are still checked before the effect. If an operation in `commit`
fails, its typed failure propagates, but already committed state and external
effects are not rolled back. `commit` is valid only inside `unsafe`.

## 13. Controlled failures and undefined behavior

Rollback applies when Zirk detects failure before irreversible corruption:

- `Result.Error` propagated from the transaction;
- an exception;
- bounds, null, alignment, or validity traps performed before access;
- cancellation before commit;
- sanitizer findings converted into a controlled pre-effect trap.

Zirk cannot promise recovery after arbitrary memory corruption, invalid machine
instructions, damage performed inside unknown native code, abrupt process
termination, or true undefined behavior. Debug and sanitizer builds should
convert as many precondition violations as possible into early diagnostics or
controlled traps; release correctness may not rely on those diagnostics.

## 14. Concurrency interaction

Safe references crossing concurrent boundaries follow the derived `Transfer`
and `Share` rules in
[STRUCTURED_CONCURRENCY_SEMANTICS.md](./STRUCTURED_CONCURRENCY_SEMANTICS.md).
Unsafe does not waive them. Tentative transaction state cannot cross a boundary,
and weak atomic ordering requires both an unsafe proof and the atomic contract.

## 15. Implementation checklist

An implementation is conforming only if it can:

- preserve identity while relocating managed storage;
- reclaim cycles without observable destructors;
- reject invalid dependent-reference escapes;
- preserve clone graph topology;
- diagnose unsafe-only operations outside their boundary;
- roll back managed and validated-range writes on controlled failure;
- reject suspension and publication from reversible transactions;
- require commit for irreversible effects;
- enforce permissions even inside unsafe/commit;
- distinguish controlled traps from unrecoverable corruption.

**Related:** [Core Language Semantics](./CORE_LANGUAGE_SEMANTICS.md) ·
[Error, Resource, and Permission Semantics](./ERROR_RESOURCE_PERMISSION_SEMANTICS.md) ·
[Structured Concurrency Semantics](./STRUCTURED_CONCURRENCY_SEMANTICS.md)
