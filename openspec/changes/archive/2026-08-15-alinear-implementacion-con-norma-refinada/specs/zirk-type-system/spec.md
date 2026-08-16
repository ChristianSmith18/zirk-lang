## MODIFIED Requirements

### Requirement: Tipos del subset

El chequeador SHALL soportar los tipos `Void`, `Int32`, `Boolean`, `String`,
tipos nulables (`T?`) y `enum` sin datos asociados, según
`ZIRK_LANGUAGE_SPEC.md` secciones 3 y 4.

Los aliases `Int` e `Integer` SHALL resolver a `Int32`, que es exactamente lo
que nombran. Un alias de un tipo implementado no es una capacidad diferida.

#### Scenario: Tipo desconocido
- **WHEN** una anotación nombra un tipo que no pertenece al subset
- **THEN** se emite un diagnóstico que nombra el tipo
- **AND** si el tipo existe en el lenguaje completo, la ayuda indica que no está implementado todavía

#### Scenario: `T?` es distinto de `T`
- **WHEN** se compara el tipo `String?` con `String`
- **THEN** el chequeador los trata como tipos distintos, no intercambiables sin coalescencia o acceso seguro

#### Scenario: Alias corto del entero por defecto
- **WHEN** se declara `mut count: Int = 0;` o `mut count: Integer = 0;`
- **THEN** el chequeo tiene éxito y el tipo es indistinguible de `Int32`

#### Scenario: Alias de un tipo no implementado
- **WHEN** una anotación nombra `UInt`
- **THEN** se emite el diagnóstico de tipo no implementado con la fase de `UInt32`

### Requirement: `for ... in` sobre el protocolo mínimo de iteración

El chequeador SHALL admitir `for ... in` sobre rangos (`0..N`, `0..=N`) y sobre
`String`, que itera por grafemas ligando un elemento de tipo `Char`. Sobre
cualquier otro tipo, SHALL rechazarlo indicando que la iteración de tipos
propios llega con los traits de Fase 3.

Mientras `Char` no esté implementado, la iteración de `String` SHALL diferirse
con el diagnóstico de fase, sin ligar un elemento de otro tipo.

#### Scenario: Iteración sobre rango
- **WHEN** se escribe `for i in 0..10 { }`
- **THEN** `i` tiene tipo `Int32` dentro del cuerpo

#### Scenario: Iteración sobre tipo no soportado
- **WHEN** se escribe `for x in valor { }` y `valor` no es un rango ni `String`
- **THEN** se emite un diagnóstico que indica que ese tipo no es iterable todavía

#### Scenario: Iteración sobre `String`
- **WHEN** se escribe `for c in texto { }` con `texto: String`
- **THEN** el elemento ligado es un grafema de tipo `Char`
- **AND** mientras `Char` no esté implementado se emite el diagnóstico de fase en vez de ligar un `String`

### Requirement: Float family replaces Decimal family
The binary floating family SHALL be `Float16`, `Float32`, `Float64`, and `Float128`, with `Float` aliasing `Float64` and ordinary fractional literals inferring `Float64`. `NaN` SHALL NOT be a valid Zirk value; indeterminate operations SHALL produce controlled errors.

`Decimal16`, `Decimal32`, `Decimal64`, `Decimal128`, `Dec` and `Decimal` SHALL NOT be recognized as types of the language, nor announced as types of a future phase. An exact base-ten type may later arrive as a standard-library type, and it would be a different thing from `Float`.

#### Scenario: Default fractional literal
- **WHEN** `1.5` has no contextual type
- **THEN** its inferred type is `Float64`

#### Scenario: Indeterminate infinity operation
- **WHEN** positive infinity is subtracted from positive infinity
- **THEN** a controlled arithmetic error is produced instead of `NaN`

#### Scenario: Withdrawn Decimal family
- **WHEN** an annotation names `Decimal64`
- **THEN** an unknown-type diagnostic is emitted
- **AND** no arrival phase is announced for that name

## ADDED Requirements

### Requirement: Familia `Float` y tipos temporales reconocidos como pendientes

El chequeador SHALL reconocer `Float16`, `Float32`, `Float64`, `Float128`,
`Float`, los anchos enteros no implementados, `Char`, y los tipos temporales
`Date`, `Time`, `DateTime`, `Instant`, `ZonedDateTime`, `TimeZone`, `Duration`
y `Period` como tipos del lenguaje pendientes, cada uno declarando la fase que
lo trae.

#### Scenario: Anotación con un tipo Float
- **WHEN** se declara `mut ratio: Float64 = 0;`
- **THEN** el diagnóstico nombra el tipo e indica la fase en que llega
- **AND** NO se reporta como tipo inexistente

#### Scenario: Anotación con un tipo temporal
- **WHEN** una anotación nombra `Instant` o `Duration`
- **THEN** el diagnóstico indica la fase de la familia temporal

### Requirement: Identidad e igualdad observables de `String`

El chequeador y el runtime SHALL tratar `String` como referencia con identidad
observable: `is` SHALL comparar identidad de referente y `==` SHALL comparar
contenido.

La comparación de contenido SHALL ser indiferente a la forma de normalización
Unicode de los operandos. El hash de un `String` SHALL derivarse de su forma
canónica, de modo que dos cadenas iguales por `==` nunca produzcan hashes
distintos.

#### Scenario: Igualdad indiferente a la normalización
- **WHEN** se comparan con `==` dos cadenas con el mismo contenido percibido, una en NFC y la otra en NFD
- **THEN** el resultado es `true`

#### Scenario: Identidad frente a contenido
- **WHEN** dos bindings distintos aliasan el mismo `String` y un tercero tiene el mismo contenido en otro referente
- **THEN** `is` es `true` solo para los dos primeros y `==` es `true` para los tres

#### Scenario: Coherencia entre hash e igualdad
- **WHEN** dos cadenas iguales por `==` se usan como claves de un mapa
- **THEN** se resuelven a la misma entrada
