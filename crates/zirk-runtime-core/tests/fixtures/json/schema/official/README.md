Official JSON Schema Test Suite

Source: https://github.com/json-schema-org/JSON-Schema-Test-Suite
Revision: 89e83a74014617ffbe79a56d83d14d054cd11417
Imported subset: tests/draft2020-12 and remotes/draft2020-12.

The harness passes all 1,302 required Draft 2020-12 cases, including all 116
required tests that use externally referenced schemas. It also passes 161
optional cases (128 cases use explicit local resolver resources). It excludes
896 optional format assertion cases because this public profile does not expose
a caller-controlled format-assertion mode, and one optional cross-draft case
because only Draft 2020-12 is in scope. The test asserts that zero required
cases are skipped. This evidence covers the pinned Draft 2020-12 suite subset;
it is not a claim of support for other schema drafts or optional format
assertions.
