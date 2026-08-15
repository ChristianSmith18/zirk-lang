## MODIFIED Requirements

### Requirement: Tipos del subset

El chequeador SHALL soportar los tipos `Void`, `Int32`, `Boolean`, `String`, tipos nulables (`T?`) y `enum` sin datos asociados, según `ZIRK_LANGUAGE_SPEC.md` secciones 3 y 4.

#### Scenario: Tipo desconocido
- **WHEN** una anotación nombra un tipo que no pertenece al subset
- **THEN** se emite un diagnóstico que nombra el tipo
- **AND** si el tipo existe en el lenguaje completo, la ayuda indica que no está implementado todavía

#### Scenario: `T?` es distinto de `T`
- **WHEN** se compara el tipo `String?` con `String`
- **THEN** el chequeador los trata como tipos distintos, no intercambiables sin coalescencia o acceso seguro

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

## ADDED Requirements

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
