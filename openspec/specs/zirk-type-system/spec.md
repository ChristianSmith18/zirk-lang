# zirk-type-system

## Purpose

Defines the types of the language, name resolution, mutability, inference and flow analysis.

This is where the rules of `ZIRK_LANGUAGE_SPEC.md` that most often surprise someone coming from another language are enforced: no numeric truthiness, no implicit conversions and no function overloading.

## Requirements

### Requirement: Tipos del subset

El chequeador SHALL soportar los tipos `Void`, `Int32`, `Boolean`, `String`, tipos nulables (`T?`) y `enum` sin datos asociados, según `ZIRK_LANGUAGE_SPEC.md` secciones 3 y 4.

#### Scenario: Tipo desconocido
- **WHEN** una anotación nombra un tipo que no pertenece al subset
- **THEN** se emite un diagnóstico que nombra el tipo
- **AND** si el tipo existe en el lenguaje completo, la ayuda indica que no está implementado todavía

#### Scenario: `T?` es distinto de `T`
- **WHEN** se compara el tipo `String?` con `String`
- **THEN** el chequeador los trata como tipos distintos, no intercambiables sin coalescencia o acceso seguro

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

El chequeador SHALL verificar que el valor retornado coincida con el tipo de retorno declarado, y que toda ruta de una función no `Void` retorne un valor, considerando que `if`, `match` y bloques terminados en bucle infinito (`loop` sin `break` que salga) pueden formar parte de esa ruta.

#### Scenario: Retorno de tipo incorrecto
- **WHEN** una función declarada `Int32` retorna una cadena
- **THEN** se emite un diagnóstico que señala la expresión retornada

#### Scenario: Ruta sin retorno
- **WHEN** una función no `Void` tiene una ruta de ejecución que termina sin retornar
- **THEN** se emite un diagnóstico que señala el final de esa ruta

#### Scenario: Retorno con valor en función Void
- **WHEN** una función `Void` retorna un valor
- **THEN** se emite un diagnóstico que señala la expresión

#### Scenario: Toda rama de `if` retorna
- **WHEN** una función no `Void` termina en un `if`/`else` donde ambas ramas retornan
- **THEN** el chequeo tiene éxito sin exigir un retorno adicional después

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

### Requirement: Tipado de bucles y de `break`/`continue`

El chequeador SHALL exigir que la condición de `for` y `while` sea `Boolean`, sin truthiness, y SHALL rechazar `break`/`continue` fuera de un bucle.

#### Scenario: Condición no booleana
- **WHEN** se escribe `while 1 { }`
- **THEN** se emite un diagnóstico indicando que la condición debe ser `Boolean`

#### Scenario: `break` fuera de bucle
- **WHEN** `break` aparece fuera de todo bucle, incluso dentro de una función anidada
- **THEN** se emite un diagnóstico que señala el `break`

### Requirement: `for ... in` sobre el protocolo mínimo de iteración

El chequeador SHALL admitir `for ... in` sobre rangos (`0..N`, `0..=N`) y sobre `String` iterada por carácter. Sobre cualquier otro tipo, SHALL rechazarlo indicando que la iteración de tipos propios llega con los traits de Fase 3.

#### Scenario: Iteración sobre rango
- **WHEN** se escribe `for i in 0..10 { }`
- **THEN** `i` tiene tipo `Int32` dentro del cuerpo

#### Scenario: Iteración sobre tipo no soportado
- **WHEN** se escribe `for x in valor { }` y `valor` no es un rango ni `String`
- **THEN** se emite un diagnóstico que indica que ese tipo no es iterable todavía

### Requirement: `if` como expresión exige ramas compatibles

El chequeador SHALL admitir `if`/`else` en posición de expresión únicamente cuando ambas ramas están presentes y producen tipos compatibles. En cualquier otro caso, `if` solo es válido como sentencia.

#### Scenario: Ramas compatibles
- **WHEN** se evalúa `if x > 0 { "positivo" } else { "no positivo" }` en posición de expresión
- **THEN** el tipo resultante es `String`

#### Scenario: Rama `else` ausente en posición de expresión
- **WHEN** un `if` sin `else` se usa donde se espera un valor
- **THEN** se emite un diagnóstico que indica que falta la rama alternativa

#### Scenario: Ramas de tipos incompatibles
- **WHEN** las ramas de un `if` usado como expresión producen tipos distintos y no relacionados
- **THEN** se emite un diagnóstico que señala ambos tipos

### Requirement: Tipado de parámetros opcionales, nombrados, variadic y valores por defecto

El chequeador SHALL verificar que toda llamada resuelva a una asignación válida de argumentos a parámetros: los nombrados se emparejan por nombre, los ausentes con valor por defecto lo toman de la firma, y los que sobran se agrupan en el parámetro variadic si existe.

#### Scenario: Parámetro opcional sin proveer
- **WHEN** se llama a `saludo()` con `nombre?: String` sin argumento
- **THEN** `nombre` tiene valor `null` dentro del cuerpo

#### Scenario: Argumento nombrado inexistente
- **WHEN** una llamada nombra un argumento que no existe en la firma
- **THEN** se emite un diagnóstico que nombra el parámetro desconocido

#### Scenario: Tipo del variadic
- **WHEN** se llama a `suma(1, 2, 3)` con `...valores: Int32`
- **THEN** `valores` tiene el tipo de secuencia de `Int32` dentro del cuerpo

### Requirement: Tipado de closures y captura inmutable

El chequeador SHALL inferir el tipo de una lambda a partir de sus parámetros y su cuerpo, SHALL registrar qué variables del scope envolvente captura, y SHALL rechazar la mutación de una variable capturada dentro del cuerpo de la closure.

#### Scenario: Tipo de una lambda
- **WHEN** se declara `inmut ADD = (a: Int32, b: Int32): Int32 => a + b;`
- **THEN** `ADD` tiene tipo función de `(Int32, Int32)` a `Int32`

#### Scenario: Captura de una variable externa
- **WHEN** una lambda referencia una variable declarada en el scope que la contiene
- **THEN** el chequeo tiene éxito y la variable queda registrada como capturada

#### Scenario: Mutación de una variable capturada
- **WHEN** el cuerpo de una lambda intenta reasignar una variable capturada del scope envolvente
- **THEN** se emite un diagnóstico que indica que la captura es inmutable

### Requirement: Exhaustividad de `match`

El chequeador SHALL exigir que todo `match` sobre un `enum` cubra todos sus constructores, o incluya el comodín `_`. `match` sobre tipos sin un conjunto cerrado de valores SHALL exigir el comodín `_` como brazo final.

#### Scenario: `enum` cubierto por completo
- **WHEN** un `match` sobre `Direction` tiene un brazo para cada uno de sus cuatro constructores
- **THEN** el chequeo tiene éxito sin exigir `_`

#### Scenario: `enum` incompleto sin comodín
- **WHEN** un `match` sobre `Direction` cubre solo dos de sus cuatro constructores y no tiene `_`
- **THEN** se emite un diagnóstico que nombra los constructores faltantes

#### Scenario: `match` sobre `Int32` sin comodín final
- **WHEN** un `match` sobre un valor `Int32` no termina con un brazo `_`
- **THEN** se emite un diagnóstico indicando que el comodín es obligatorio para ese tipo

#### Scenario: Tipo del `match` como expresión
- **WHEN** todos los brazos de un `match` usado como expresión producen el mismo tipo
- **THEN** ese es el tipo del `match`

### Requirement: Coalescencia nula

El chequeador SHALL exigir que ambos operandos de `??` compartan un tipo común, produciendo el tipo no nulable cuando el operando derecho no es nulable.

#### Scenario: Coalescencia con fallback no nulable
- **WHEN** se evalúa `nombre ?? "anónimo"` con `nombre: String?`
- **THEN** el resultado tiene tipo `String`

#### Scenario: Coalescencia con fallback nulable
- **WHEN** ambos operandos de `??` son nulables
- **THEN** el resultado sigue siendo nulable

#### Scenario: `??` sobre operando izquierdo no nulable
- **WHEN** `??` se usa sobre una expresión de tipo no nulable
- **THEN** se emite un diagnóstico indicando que el operador es innecesario

#### Scenario: Operandos sin tipo común
- **WHEN** los operandos de `??` no comparten un tipo común
- **THEN** se emite un diagnóstico que señala ambos tipos

### Requirement: Asignación entre tipos nulables y no nulables

El chequeador SHALL admitir asignar un valor de tipo `T` donde se espera `T?`, y SHALL rechazar la dirección contraria sin coalescencia explícita.

#### Scenario: Ensanchar a nulable
- **WHEN** se asigna un `String` a una variable declarada `String?`
- **THEN** el chequeo tiene éxito

#### Scenario: Estrechar sin coalescencia
- **WHEN** se asigna un `String?` a una variable declarada `String`
- **THEN** se emite un diagnóstico
- **AND** la ayuda sugiere `??` para proveer un valor por defecto

#### Scenario: `null` como valor
- **WHEN** se asigna `null` a una variable de tipo no nulable
- **THEN** se emite un diagnóstico que indica que el tipo no admite ausencia de valor

### Requirement: `?.` difiere a la fase de objetos

El chequeador SHALL rechazar `?.` con el diagnóstico de construcción no implementada, indicando Fase 3.

El operador de acceso seguro requiere que exista un miembro que acceder, y ningún tipo de esta fase tiene miembros: clases, records y traits son Fase 3. Ver decisión D8 del design.

#### Scenario: Uso de `?.`
- **WHEN** se escribe `usuario?.nombre`
- **THEN** se emite un diagnóstico que nombra el operador
- **AND** indica que llega en Fase 3, junto con los tipos que tienen miembros
- **AND** NO se reporta como token inesperado
