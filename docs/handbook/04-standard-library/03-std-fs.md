# `std.fs`

`File` implements `Resource<FileError>` and supports typed open/create options, text/byte/line I/O, append, metadata, flush, and async variants.

```zirk
inmut text = match with File.open(path) {
    Ok(file) => file.read_text();
    Error(error) => return Error(error);
};
```

Read/write permissions are scoped and real paths are checked against `..` and symlink escapes. Errors distinguish absence, denial, existence, invalid path/type, EOF where applicable, and system failure.

---

**Previous:** [← `std.io`](./02-std-io.md) · **Next:** [`std.path` →](./04-std-path.md)
