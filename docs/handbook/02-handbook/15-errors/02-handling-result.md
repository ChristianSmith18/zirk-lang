# Handling `Result`

Use direct `match` to inspect success and failure without losing information:

```zirk
match load(path) {
    Ok(document) => render(document);
    Error(error) => report(error);
}
```

When only the value matters, let the surrounding value context project the Result to a nullable value:

```zirk
inmut document: Document? = load(path);
inmut required_document = load(path) ?? Document.empty();
```

`??` selects its fallback for every `Error`, including permission and parsing errors. Use `match` if those errors need different handling. To inspect the same result later, retain it with an explicit type:

```zirk
inmut result: Result<Document, LoadError> = load(path);
match result {
    Ok(document) => render(document);
    Error(error) => report(error);
}
```

A Result-producing expression used as a standalone statement is still an error. `_ = load(path);` is the explicit discard form.

---

**Previous:** [← Result](01-result.md) · **Next:** [Error Propagation](03-error-propagation.md)
