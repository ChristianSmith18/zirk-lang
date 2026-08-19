## ADDED Requirements

### Requirement: Contrato `to_string()`

El lenguaje SHALL definir `to_string()` como un contrato reservado, en la misma familia que los contratos de operador (`ZIRK_LANGUAGE_SPEC.md` sección 4): todo tipo nativo escalar lo implementa, y un tipo del usuario — clase, record, enum — puede implementarlo para producir su propia representación textual. `print`/`println` SHALL rutear por él en vez de aceptar únicamente una lista cerrada de tipos.

#### Scenario: Tipo nativo imprimible sin declarar nada
- **WHEN** se imprime un `Int32`, `Float64`, `Boolean` o `Char`
- **THEN** el chequeo tiene éxito sin que el programa declare `to_string()` para ellos

#### Scenario: Tipo del usuario que implementa `to_string()`
- **WHEN** una clase declara `to_string(): String` y una instancia se pasa a `println`
- **THEN** el chequeo tiene éxito y el valor impreso es el que ese método produce

#### Scenario: Tipo del usuario que no lo implementa
- **WHEN** una clase sin `to_string()` se pasa a `println`
- **THEN** se emite un diagnóstico que nombra el tipo, distinto de un `TYPE_MISMATCH` genérico

### Requirement: Interpolación de strings

Un literal de `String` SHALL admitir `{expr}` en su interior, desazucarado a concatenar el texto literal con `expr.to_string()` para cada expresión interpolada, en el orden en que aparecen.

#### Scenario: Interpolación de una variable
- **WHEN** se escribe `"User: {user.name}"`
- **THEN** el resultado concatena el texto literal con `user.name.to_string()`

#### Scenario: Interpolación de un tipo no imprimible
- **WHEN** la expresión interpolada tiene un tipo sin `to_string()`
- **THEN** se emite el mismo diagnóstico que pasar ese valor directamente a `println`

### Requirement: Contexto de literal fraccionario y aritmética mixta

Un literal fraccionario sin anotación SHALL tener tipo `Float64`, y una operación aritmética entre un tipo entero y uno `Float` SHALL producir `Float`. La sección 3 de `ZIRK_LANGUAGE_SPEC.md` ya documentaba ambas reglas; esta fase es la primera en la que `Float` existe y se vuelven verificables.

#### Scenario: Literal fraccionario sin anotación
- **WHEN** se escribe `mut x = 1.5;` sin anotación de tipo
- **THEN** `x` tiene tipo `Float64`

#### Scenario: Aritmética mixta entero y `Float`
- **WHEN** se suma un `Int32` y un `Float64`
- **THEN** el resultado tiene tipo `Float64`
