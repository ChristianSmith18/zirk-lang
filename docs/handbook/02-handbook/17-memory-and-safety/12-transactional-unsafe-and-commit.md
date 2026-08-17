# Transactional Unsafe and `commit`

An ordinary unsafe block isolates writes to managed state and validated native
ranges. Success commits; a controlled `Error`, exception, trap, or cancellation
before commit closes new resources and restores the original values.

The transaction cannot `await`, spawn a task/thread, or publish tentative state.
Those actions would let another observer see data that might later roll back.

Irreversible work is explicit:

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

Entering `commit` publishes pending reversible writes. External I/O,
unknown-effect FFI, volatile/device writes, manual release, concurrent
publication, and unbounded raw writes belong there. Permissions still apply.
If the committed operation fails, its typed failure propagates but already
published state and external effects remain.

For the complete rollback matrix and implementation optimizations, read
[Memory and Unsafe Semantics](../../../MEMORY_AND_UNSAFE_SEMANTICS.md).

---

**Previous:** [← Undefined Behavior](11-undefined-behavior.md) · **Next:** [ Concurrency](../18-concurrency/README.md)
