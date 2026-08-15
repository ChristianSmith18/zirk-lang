## ADDED Requirements

### Requirement: Enums algebraicos

Un `enum` SHALL admitir variantes con valores asociados, extendiendo el enum sin datos de la fase anterior.

#### Scenario: Variante con datos
- **WHEN** se declara `enum Shape { Circle(Int32), Rect(Int32, Int32) }`
- **THEN** `Shape.Circle(3)` construye un valor

#### Scenario: Enum sin datos sigue funcionando
- **WHEN** se declara `enum Direction { North, South }`
- **THEN** compila igual que en la fase anterior

#### Scenario: Constructor con aridad incorrecta
- **WHEN** se construye una variante con más o menos valores de los declarados
- **THEN** se emite un diagnóstico que indica la aridad esperada

### Requirement: Destructuring en `match`

Un patrón SHALL poder extraer los valores asociados de una variante, ligándolos a nombres dentro del brazo.

#### Scenario: Extraer los datos de una variante
- **WHEN** se escribe `match s { Shape.Circle(r) => r, Shape.Rect(w, h) => w * h }`
- **THEN** `r`, `w` y `h` están ligados en su brazo con el tipo declarado

#### Scenario: Patrón con aridad incorrecta
- **WHEN** un patrón de variante liga menos nombres de los que la variante declara
- **THEN** se emite un diagnóstico que indica la aridad

#### Scenario: Exhaustividad con datos asociados
- **WHEN** un `match` sobre un enum algebraico omite una variante y no tiene `_`
- **THEN** se emite el diagnóstico de exhaustividad nombrando la variante faltante

### Requirement: Records

Un `record` SHALL ser inmutable y tener semántica estructural, según `ZIRK_LANGUAGE_SPEC.md` sección 7.

#### Scenario: Igualdad estructural
- **WHEN** se comparan dos records con los mismos valores en sus campos
- **THEN** `==` produce `true` aunque sean instancias distintas

#### Scenario: Inmutabilidad
- **WHEN** se asigna a un campo de un record
- **THEN** se emite un diagnóstico indicando que un record no se modifica

### Requirement: Value classes

Una value class NO SHALL tener identidad observable y SHALL poder almacenarse inline.

#### Scenario: Sin identidad
- **WHEN** se aplica el operador de identidad a dos value classes con el mismo contenido
- **THEN** se emite un diagnóstico indicando que no tienen identidad observable

#### Scenario: Almacenada sin indirección
- **WHEN** una value class es campo de otra declaración
- **THEN** ocupa su espacio dentro de ella, sin puntero intermedio

### Requirement: Uniones y alias

`A | B` SHALL declarar una unión, y `type` SHALL declarar un alias.

#### Scenario: Valor de una unión
- **WHEN** una variable se declara `Int32 | String`
- **THEN** admite valores de cualquiera de los dos

#### Scenario: Uso sin discriminar
- **WHEN** se usa un valor de unión donde se espera uno de sus miembros
- **THEN** se emite un diagnóstico
- **AND** la ayuda indica discriminarlo con `match`

#### Scenario: Alias
- **WHEN** se declara `type Id = Int32;`
- **THEN** `Id` e `Int32` son intercambiables
