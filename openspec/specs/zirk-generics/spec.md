# zirk-generics Specification

## Purpose
TBD - created by archiving change document-refined-core-language-semantics. Update Purpose after archive.
## Requirements
### Requirement: Generic declarations, constraints, defaults, and inference
Generic parameters SHALL use `<T>`, combined constraints SHALL use `T from A & B`, and trailing parameters MAY declare defaults satisfying their constraints. Inference SHALL use arguments, receiver, expected result, callable context, and constraints and SHALL fail rather than choose an arbitrary solution.

#### Scenario: Multiple missing constraints
- **WHEN** a concrete type is supplied for `T from Clone & Serializable` and lacks Serializable
- **THEN** the call-site diagnostic identifies T and the missing Serializable contract

### Requirement: Declared generic variance
Generic parameters SHALL be invariant by default. `out T` SHALL be legal only in covariant output positions, `in T` only in contravariant input positions, and any mutable attribute use SHALL require invariance. `Fn` SHALL retain its intrinsic parameter/result variance.

#### Scenario: Mutable List invariance
- **WHEN** Dog extends Animal and `List<Dog>` is supplied as `List<Animal>`
- **THEN** compilation fails because List permits insertion

### Requirement: Recursive generics and managed indirection
Verifiable recursive constraints SHALL be accepted. Infinite inline recursive layout SHALL fail and MAY be broken with managed `Box<T>`. Associated types and higher-kinded parameters SHALL remain outside the initial language.

#### Scenario: Recursive record requires Box
- **WHEN** a record directly stores its own type without indirection
- **THEN** compilation fails with an infinite-size diagnostic and suggests `Box<T>`

### Requirement: Generic identity and monomorphization
Concrete instantiations SHALL retain distinct static and runtime type identity. Generic bodies SHALL be checked once against constraints; portable IR SHALL retain generic information and final builds MAY monomorphize/share code only when ABI and observable semantics remain unchanged.

#### Scenario: Distinct runtime instantiations
- **WHEN** runtime type identity is requested for List<String> and List<Int32>
- **THEN** the two complete instantiations remain distinguishable

### Requirement: Parámetros de tipo

Funciones, clases y tipos de datos SHALL admitir parámetros de tipo con la sintaxis `<T>`, según `ZIRK_LANGUAGE_SPEC.md` sección 7.

#### Scenario: Función genérica
- **WHEN** se declara `fn identity<T>(value: T): T { return value; }`
- **THEN** `identity(1)` produce un `Int32` y `identity("a")` un `String`

#### Scenario: Clase genérica
- **WHEN** se declara `class Box<T>` con un campo de tipo `T`
- **THEN** `Box<Int32>` y `Box<String>` son tipos distintos

#### Scenario: Parámetro de tipo sin usar
- **WHEN** se declara un parámetro de tipo que no aparece en la firma
- **THEN** se emite un diagnóstico que lo nombra

### Requirement: Restricciones con `from`

Un parámetro de tipo SHALL admitir restricciones con `from`, y el chequeador SHALL verificarlas en el sitio de uso.

#### Scenario: Argumento que cumple la restricción
- **WHEN** se llama `serialize<T from Serializable>` con un tipo que implementa `Serializable`
- **THEN** el chequeo tiene éxito

#### Scenario: Argumento que no la cumple
- **WHEN** se llama con un tipo que no implementa el contrato
- **THEN** se emite un diagnóstico que nombra el tipo concreto y el contrato que le falta

#### Scenario: El cuerpo solo usa lo que la restricción garantiza
- **WHEN** el cuerpo de una función genérica llama a un método que la restricción no declara
- **THEN** se emite un diagnóstico
- **AND** la ayuda indica que la restricción debe declararlo

### Requirement: El genérico se chequea una vez

El chequeador SHALL verificar el cuerpo de una declaración genérica una sola vez contra sus restricciones, no una vez por instanciación.

#### Scenario: Error en el cuerpo se reporta una vez
- **WHEN** una función genérica con un error de tipos se instancia con tres tipos distintos
- **THEN** el error se reporta una sola vez, en la declaración

### Requirement: Especialización al bajar

El lowering SHALL producir una copia por combinación de argumentos de tipo efectivamente usada.

Es lo que `ZIRK_LANGUAGE_SPEC.md` sección 7 llama especializar donde corresponda, y lo que permite almacenar value classes inline en vez de tras un puntero.

#### Scenario: Dos instanciaciones, dos copias
- **WHEN** `identity` se usa con `Int32` y con `String`
- **THEN** la IR contiene una función por cada una

#### Scenario: Instanciación repetida
- **WHEN** la misma combinación de tipos se usa varias veces
- **THEN** la IR contiene una sola copia

### Requirement: Alcance de los genéricos en esta fase

El chequeador NO SHALL admitir varianza declarada, tipos asociados ni parámetros de tipo de orden superior.

Ninguno está en `ZIRK_LANGUAGE_SPEC.md`, y admitirlos fijaría semántica que el spec no fija.

#### Scenario: Construcción no soportada
- **WHEN** se escribe una anotación de varianza
- **THEN** se emite un diagnóstico indicando que no forma parte del lenguaje

