## ADDED Requirements

### Requirement: Codegen de cada ancho entero sobre el tipo nativo de LLVM

El backend SHALL emitir cada tipo entero de la IR como el `IntType` de LLVM del ancho correspondiente, con las operaciones aritméticas y los intrínsecos de overflow (`*.with.overflow`) que correspondan a su señal.

#### Scenario: Suma comprobada con signo
- **WHEN** se emite código para una suma sobre `Int64`
- **THEN** se usa el intrínseco de overflow con signo de ese ancho

#### Scenario: Suma comprobada sin signo
- **WHEN** se emite código para una suma sobre `UInt64`
- **THEN** se usa el intrínseco de overflow sin signo de ese ancho

### Requirement: Codegen de `Float` sobre el tipo nativo de LLVM

El backend SHALL emitir cada tipo `Float` de la IR como el `FloatType` de LLVM del ancho correspondiente, con la comprobación de `NaN`/dominio inválido emitida explícitamente alrededor de la operación nativa, no delegada a la semántica de punto flotante del backend.

#### Scenario: Comprobación explícita antes del resultado
- **WHEN** se emite código para una operación de `Float` que la IR marcó como potencialmente indeterminada
- **THEN** el código generado comprueba la condición antes de que el resultado se use, y llama al runtime para el fallo controlado si se cumple

### Requirement: Codegen del despacho a `to_string()`

El backend SHALL emitir, para cada sitio donde `println`/`print`/una interpolación necesita convertir un valor a texto, una llamada al método `to_string()` resuelto para el tipo estático de ese valor — directa si no es redefinible, por la tabla del objeto o del contrato si lo es, con el mismo mecanismo que cualquier otra llamada a método (ADR-013).

#### Scenario: Conversión de un tipo nativo
- **WHEN** se emite código para imprimir un `Int32`
- **THEN** la llamada a `to_string()` resuelve al `to_string()` nativo de `Int32`, sin indirección

#### Scenario: Conversión de un tipo del usuario redefinible
- **WHEN** se emite código para imprimir un valor de un tipo cuyo `to_string()` puede ser redefinido por una subclase
- **THEN** la llamada pasa por la misma tabla que cualquier otro método virtual del objeto
