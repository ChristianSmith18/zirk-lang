# zirk-type-system

## Purpose

Defines the types of the language, name resolution, mutability, inference and flow analysis.

This is where the rules of `ZIRK_LANGUAGE_SPEC.md` that most often surprise someone coming from another language are enforced: no numeric truthiness, no implicit conversions and no function overloading.

## Requirements

### Requirement: Tipos del subset

El chequeador SHALL soportar los tipos `Void`, `Int32`, `Boolean` y `String`, según `ZIRK_LANGUAGE_SPEC.md` sección 3.

#### Scenario: Tipo desconocido
- **WHEN** una anotación nombra un tipo que no pertenece al subset
- **THEN** se emite un diagnóstico que nombra el tipo
- **AND** si el tipo existe en el lenguaje completo, la ayuda indica que no está implementado todavía

### Requirement: Ausencia de conversiones implícitas

El chequeador NO SHALL insertar conversiones implícitas entre tipos distintos. Según `ZIRK_LANGUAGE_SPEC.md` sección 3, las conversiones que puedan perder información no son implícitas.

#### Scenario: Asignación de tipo incompatible
- **WHEN** se declara `mut total: Int32 = "cuarenta";`
- **THEN** se emite un diagnóstico que señala el inicializador
- **AND** la causa indica que no hay conversión implícita de `String` a `Int32`

#### Scenario: Operandos de tipos distintos
- **WHEN** se evalúa una operación aritmética entre `Int32` y `String`
- **THEN** se emite un diagnóstico que señala la operación

### Requirement: Ausencia de truthiness

El chequeador SHALL exigir que toda condición sea de tipo `Boolean`. No existe truthiness numérico ni de cadena, según `ZIRK_LANGUAGE_SPEC.md` sección 3.

#### Scenario: Condición numérica
- **WHEN** se escribe `if 1 { }`
- **THEN** se emite un diagnóstico indicando que la condición debe ser `Boolean`

#### Scenario: Condición booleana
- **WHEN** se escribe `if x > 0 { }`
- **THEN** el chequeo tiene éxito

### Requirement: Operadores lógicos solo sobre booleanos

Los operadores `&&`, `||` y `!` SHALL aceptar únicamente operandos `Boolean`.

#### Scenario: Conjunción sobre enteros
- **WHEN** se evalúa `1 && 2`
- **THEN** se emite un diagnóstico que señala los operandos

### Requirement: Mutabilidad

El chequeador SHALL permitir la reasignación de variables `mut` y rechazarla en variables `inmut`, según `ZIRK_LANGUAGE_SPEC.md` sección 2.

#### Scenario: Reasignación de variable mutable
- **WHEN** una variable declarada con `mut` se reasigna
- **THEN** el chequeo tiene éxito

#### Scenario: Reasignación de variable inmutable
- **WHEN** una variable declarada con `inmut` se reasigna
- **THEN** se emite un diagnóstico que señala la reasignación
- **AND** la ayuda sugiere declararla con `mut` si debe cambiar

### Requirement: Inferencia inequívoca

El chequeador SHALL inferir el tipo de una variable sin anotación a partir de su inicializador, cuando la inferencia sea inequívoca.

#### Scenario: Inferencia desde literal entero
- **WHEN** se declara `mut count = 0;`
- **THEN** el tipo inferido es `Int32`

#### Scenario: Inferencia desde literal de cadena
- **WHEN** se declara `mut name = "Zirk";`
- **THEN** el tipo inferido es `String`

### Requirement: Resolución de nombres

El chequeador SHALL resolver cada identificador a su declaración, respetando los scopes de bloque y de función de `ZIRK_LANGUAGE_SPEC.md` sección 2.

#### Scenario: Identificador no declarado
- **WHEN** se usa un identificador que no ha sido declarado
- **THEN** se emite un diagnóstico que lo nombra y señala su uso

#### Scenario: Variable fuera de su scope
- **WHEN** se usa una variable declarada dentro de un bloque, fuera de ese bloque
- **THEN** se emite un diagnóstico de identificador no declarado

#### Scenario: Sombra en bloque interno
- **WHEN** un bloque interno declara una variable con el nombre de una externa
- **THEN** los usos dentro del bloque resuelven a la declaración interna

### Requirement: Uso antes de disponibilidad

El análisis de flujo SHALL impedir leer una variable que todavía no tenga valor, según `ZIRK_LANGUAGE_SPEC.md` sección 2.

#### Scenario: Lectura antes de asignar
- **WHEN** se lee una variable declarada sin inicializador y aún no asignada
- **THEN** se emite un diagnóstico que señala la lectura

### Requirement: Correspondencia de firma en llamadas

El chequeador SHALL verificar aridad y tipos de los argumentos de cada llamada contra la firma de la función.

#### Scenario: Cantidad incorrecta de argumentos
- **WHEN** una llamada pasa más o menos argumentos que los declarados
- **THEN** se emite un diagnóstico que indica la cantidad esperada y la recibida

#### Scenario: Tipo incorrecto de argumento
- **WHEN** un argumento no coincide con el tipo del parámetro
- **THEN** se emite un diagnóstico que señala ese argumento

### Requirement: Coherencia del retorno

El chequeador SHALL verificar que el valor retornado coincida con el tipo de retorno declarado, y que toda ruta de una función no `Void` retorne un valor.

#### Scenario: Retorno de tipo incorrecto
- **WHEN** una función declarada `Int32` retorna una cadena
- **THEN** se emite un diagnóstico que señala la expresión retornada

#### Scenario: Ruta sin retorno
- **WHEN** una función no `Void` tiene una ruta de ejecución que termina sin retornar
- **THEN** se emite un diagnóstico que señala el final de esa ruta

#### Scenario: Retorno con valor en función Void
- **WHEN** una función `Void` retorna un valor
- **THEN** se emite un diagnóstico que señala la expresión

### Requirement: Overflow de literales enteros

El chequeador SHALL rechazar literales enteros que no quepan en su tipo, según la regla de `ZIRK_LANGUAGE_SPEC.md` sección 3 de que el overflow ordinario produce error controlado.

#### Scenario: Literal fuera de rango
- **WHEN** se asigna a un `Int32` un literal mayor que su valor máximo
- **THEN** se emite un diagnóstico que indica el rango admitido

### Requirement: Entrypoint

El chequeador SHALL exigir que exista una función `main` con la firma `fn main(): Void`, según `ZIRK_RUNTIME_SPEC.md` sección 2.

#### Scenario: Entrypoint ausente
- **WHEN** el archivo compilado no declara `main`
- **THEN** se emite un diagnóstico indicando que falta el entrypoint

#### Scenario: Entrypoint con firma incorrecta
- **WHEN** `main` se declara con parámetros o con retorno distinto de `Void`
- **THEN** se emite un diagnóstico que indica la firma esperada
