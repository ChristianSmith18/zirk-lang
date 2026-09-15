# Browser conformance fixtures

Each JSON fixture is a single browser-compatible source unit with these fields:

- `name`: stable fixture identifier.
- `source` and `virtualName`: the in-memory program and diagnostic-only name.
- `expectedOutput`: exact ordered output events.
- `completion`: `completed`, `runtime`, `capability`, or `limit`.
- `exclusions`: explicit native-only features the browser runner must not try
  to execute.

Runners compare only observable output and completion category. They never
read source paths from a fixture or grant filesystem, network, clock, DOM, or
native-runtime authority to the program.
