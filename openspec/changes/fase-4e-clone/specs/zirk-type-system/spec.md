## MODIFIED Requirements

### Requirement: Projection copy and whole-reference aliasing
The checker SHALL classify reference expressions as whole references, projection reads, or places. Whole-reference assignment/passing/return/capture SHALL preserve aliasing; projection reads SHALL require deep Clone and produce independence; places SHALL preserve access to original storage. Destructuring, matching, callable capture, collection extraction, and generic T SHALL follow the same rule.

#### Scenario: Generic projection needs Clone
- **WHEN** a generic function returns `values[0]` for unconstrained T
- **THEN** the checker requires `T from Clone` or rejects extraction

### Requirement: Strictness de referencias de objeto

En referencias de clase, `mut` SHALL permitir reasignar y mutar el objeto,
`inmut` SHALL impedir solo la reasignación, e `inmut::strict` SHALL impedir la
mutación alcanzable. Una referencia strict NO SHALL convertirse en alias
mutable ni adquirirse mientras permanezca accesible un alias mutable.

#### Scenario: Clon mutable desde referencia strict
- **WHEN** un objeto strict implementa `Clone` y se clona a un binding `mut`
- **THEN** el clon independiente puede mutarse sin alterar el objeto original
