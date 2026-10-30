# RFC 8785 JSON Canonicalization Scheme test data

These `input/` and `output/` files are copied from the upstream JCS test suite at
[`cyberphone/json-canonicalization`](https://github.com/cyberphone/json-canonicalization),
revision `32a48195b9b20fe67b1795e39e4509f14d860b45`, under `testdata/`. The upstream license is preserved as
`LICENSE.upstream`. The upstream corpus is described by RFC 8785 Appendix I.

`values.json` has an input number that exercises ECMAScript binary64 rounding.
The test converts its numeric nodes to IEEE-754 binary64 before canonicalization,
matching RFC 8785's parsed-data model. Zirk's separate language-level tests
verify that exact Zirk number tokens outside that domain are rejected rather
than rounded.
