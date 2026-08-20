# Call a C Library

Wrap a C checksum library without exposing pointers to ordinary callers.

> **Status:** target FFI/unsafe semantics; verify the selected target and native
> artifact before expecting current compiler execution.

The package manifest records the library, integrity, supported targets, ABI,
linkage, and native-load permission. The raw declaration mirrors the header:

```zirk
extern "C" fn checksum_update(
    state: Pointer<ChecksumState>,
    bytes: Pointer<UInt8>,
    length: UIntSize,
): CInt unsafe;
```

Do not publish this declaration. Build a safe wrapper that owns the handle,
validates bounds, creates a bounded view, pins only for the call, and translates
the status:

```zirk
share fn checksum(bytes: Bytes): Result<UInt64, ChecksumError> {
    unsafe {
        mut state = ChecksumState.create();
        mut status = checksum_update(
            state.pointer(),
            bytes.native_view().pointer(),
            bytes.length,
        );
        if status != 0 return Error(ChecksumError.Native(status));
        return Ok(state.finish());
    }
}
```

If the native call can mutate unknown memory, release native ownership, invoke
external I/O, or publish state, put that operation in an explicit `commit`
region. `unsafe` does not grant filesystem/network/native authority, and
`commit` does not grant it either.

Test null/empty/large buffers, error codes, ownership and double-close,
callback/reentrancy if applicable, exceptions translated before C, layouts
against a C companion, and every target. `zirk prepare --target ...` must reject
an unavailable architecture before the linker. See [C ABI](../05-native-and-low-level/01-c-abi.md),
[Importing C](../05-native-and-low-level/02-importing-c.md), and
[Transactional Unsafe](../02-handbook/17-memory-and-safety/12-transactional-unsafe-and-commit.md).

## Completion contract

- **Prerequisites:** C ABI, pointers/native views, resources, `unsafe`,
  `commit`, native permissions, and target preparation.
- **Expected success:** callers receive an ordinary `Result<UInt64,...>` and
  never observe a pointer or native ownership detail.
- **Failure recovery:** invalid targets fail before linking; native errors are
  translated and every owned handle closes exactly once.
- **Next:** generate safe framework code with a [decorator](07-write-a-decorator.md).

---

**Previous:** [← Build and Publish a Library](05-build-and-publish-a-library.md) · **Next:** [ Write a Decorator](07-write-a-decorator.md)
