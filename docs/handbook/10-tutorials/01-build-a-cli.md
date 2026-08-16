# Build a CLI

Create `word-count` with an application manifest and `src/main.zrk`. Obtain arguments from `std.process`, validate one input path, and represent user mistakes with a typed `Result` rather than `fatalError`.

Open the file with `match with`, count Unicode text deliberately, print the result through `std.io`, and grant only read access to the selected scope. Add `.spec.zrk` tests for parsing and an E2E test for exit code and output.

```console
zirk check
zirk format
zirk lint
zirk test --all
zirk build
```

---

**Previous:** [← Tutorials](README.md) · **Next:** [ Build a File Processor](02-build-a-file-processor.md)
