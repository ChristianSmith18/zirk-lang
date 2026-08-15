## ADDED Requirements

### Requirement: Standard-library convenience member resolution
Importing a compiler-known standard-library object SHALL make its declared convenience members callable without qualification when the name is otherwise unambiguous. A local or imported collision SHALL require qualification through the imported object. Objects from local files or packages SHALL NOT inject their methods into file scope.

#### Scenario: Direct standard output call
- **WHEN** a file imports `{ stdout } from std.io` and has no competing `println`
- **THEN** `println("hello")` resolves to `stdout.println("hello")`

#### Scenario: Ambiguous convenience name
- **WHEN** the file also declares or imports another `println`
- **THEN** an unqualified call is diagnosed and `stdout.println(...)` selects the standard operation

#### Scenario: Package object import
- **WHEN** an object is imported from a package rather than a standard module
- **THEN** its methods remain accessible only through the object unless explicitly exported as declarations
