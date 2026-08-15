# Indexing

Indexing retrieves an element using a collection-specific key, commonly an integer position.

```zirk
inmut first = names[0];
```

Safe indexing cannot produce undefined behavior. An out-of-range position must produce the collection's defined controlled error or use a separate safe-access API that returns absence.

String indexing is semantically grapheme-aware; byte and code-point access require APIs that name those units. Do not assume one indexing operation has identical cost across all collection types.

---

**Previous:** [← Ranges](./06-ranges.md) · **Next:** [Slicing →](./08-slicing.md)
