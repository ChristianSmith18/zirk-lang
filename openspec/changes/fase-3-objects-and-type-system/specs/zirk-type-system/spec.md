## MODIFIED Requirements

### Requirement: `?.` difiere a la fase de objetos

El chequeador SHALL tipar `expr?.miembro` como el tipo del miembro en su forma nulable cuando el receptor es nulable.

El diferimiento de la fase anterior termina aquí: su razón era que ningún tipo tenía miembros.

#### Scenario: Acceso seguro sobre valor nulable
- **WHEN** `usuario` tiene tipo `User?` y `nombre` es `String`
- **THEN** `usuario?.nombre` tiene tipo `String?`

#### Scenario: `?.` sobre valor no nulable
- **WHEN** `?.` se usa sobre una expresión que nunca es nula
- **THEN** se emite un diagnóstico indicando que el operador es innecesario, con `.` como sugerencia

#### Scenario: Miembro inexistente
- **WHEN** `?.` nombra un miembro que el tipo no tiene
- **THEN** se emite un diagnóstico que nombra el miembro y el tipo

## ADDED Requirements

### Requirement: Tipos nominales y subtipado

El chequeador SHALL tratar cada clase, record, value class y enum como un tipo nominal distinto, y SHALL admitir un valor donde se espera una superclase suya o un contrato que implementa.

#### Scenario: Subclase donde se espera la base
- **WHEN** se pasa una instancia de `Admin` a un parámetro de tipo `User`
- **THEN** el chequeo tiene éxito

#### Scenario: Implementación donde se espera el contrato
- **WHEN** se pasa una instancia a un parámetro cuyo tipo es un contrato que implementa
- **THEN** el chequeo tiene éxito

#### Scenario: Dos tipos con la misma forma no son el mismo
- **WHEN** dos clases declaran los mismos campos y se asigna una donde se espera la otra
- **THEN** se emite un diagnóstico: la equivalencia es por nombre, no por forma

#### Scenario: Base donde se espera la subclase
- **WHEN** se pasa una instancia de `User` a un parámetro de tipo `Admin`
- **THEN** se emite un diagnóstico

### Requirement: Resolución de miembros

El chequeador SHALL resolver un acceso `expr.miembro` contra el tipo de `expr` y su cadena de herencia, respetando la visibilidad.

#### Scenario: Miembro heredado
- **WHEN** se accede a un campo declarado en la superclase
- **THEN** resuelve a esa declaración

#### Scenario: Miembro inexistente
- **WHEN** se accede a un miembro que ningún ancestro declara
- **THEN** se emite un diagnóstico que nombra el miembro y el tipo

#### Scenario: Miembro oculto por visibilidad
- **WHEN** se accede desde fuera a un miembro `private`
- **THEN** se emite un diagnóstico de visibilidad, distinto del de miembro inexistente

### Requirement: Shadowing y captura calificada

El chequeador SHALL rechazar una declaración local que oculte otro local o parámetro todavía visible. Un parámetro de lambda MAY compartir el nombre de una captura únicamente cuando la captura se referencia como `this.nombre`; el nombre simple designa el parámetro.

#### Scenario: Local duplicado en scope anidado
- **WHEN** un bloque interno declara un nombre local todavía visible
- **THEN** se emite un diagnóstico que señala ambas declaraciones

#### Scenario: Colisión de parámetro y captura
- **WHEN** una lambda declara el parámetro `prefix` y lee `this.prefix`
- **THEN** `prefix` resuelve al parámetro y `this.prefix` a la captura exterior

### Requirement: Resolución de constructores

El chequeador SHALL seleccionar entre múltiples `construct` por aridad, tipos, opcionales y nombres, SHALL permitir reordenar argumentos nombrados y SHALL rechazar firmas efectivas duplicadas o llamadas ambiguas.

#### Scenario: Construcción nombrada reordenada
- **WHEN** `User(name: "Cristian", id: 1)` coincide con `construct(id: UInt64, name: String)`
- **THEN** se selecciona esa firma y cada valor se liga por nombre

### Requirement: Casts comprobables

Un cast a un tipo relacionado SHALL comprobarse en tiempo de ejecución y fallar de forma controlada; un cast entre tipos no relacionados SHALL rechazarse al compilar.

#### Scenario: Descenso válido
- **WHEN** se convierte un `User` que en realidad es un `Admin` a `Admin`
- **THEN** el resultado es la instancia

#### Scenario: Descenso inválido
- **WHEN** el valor no es del tipo pedido
- **THEN** el programa termina con un error de runtime diagnosticado
- **AND** NO incurre en comportamiento indefinido

#### Scenario: Tipos sin relación
- **WHEN** se convierte entre dos tipos que no comparten jerarquía ni contrato
- **THEN** se emite un diagnóstico al compilar

### Requirement: Los operadores resuelven por contrato

El chequeador SHALL resolver un operador buscando el contrato que su tipo implementa, en vez de comparar contra una lista fija de tipos.

#### Scenario: Concatenación
- **WHEN** se evalúa `"a" + "b"`
- **THEN** el tipo resultante es `String`

#### Scenario: Operando sin el contrato
- **WHEN** un operando no implementa el contrato del operador
- **THEN** el diagnóstico nombra el contrato que falta, no una lista de tipos admitidos

### Requirement: Los tipos función no se escriben todavía

El chequeador SHALL rechazar cualquier posición que exija anotar el tipo de una closure —parámetro, retorno o campo— con un diagnóstico que indique que los tipos función llegan en una fase posterior.

Un lambda sigue siendo un valor cuyo tipo se infiere y es nominal por expresión, tal como fijó D10 de la fase anterior y confirma D9 de esta. Que no se puedan escribir es lo que mantiene a una closure sin escapar de la función que la crea, y con ello evita decidir dónde vive y cuánto dura su entorno — una decisión de memoria, que corresponde a la Fase 4.

#### Scenario: Closure en una variable local
- **WHEN** se declara `inmut F = (a: Int32): Int32 => a + 1;` y se llama `F(1)`
- **THEN** el chequeo tiene éxito
- **AND** el tipo de `F` se infiere sin escribirse

#### Scenario: Closure como tipo de parámetro
- **WHEN** una función declara un parámetro cuyo tipo pretende ser una función
- **THEN** se emite un diagnóstico indicando que los tipos función llegan en una fase posterior
- **AND** NO se reporta como tipo desconocido

#### Scenario: Closure como tipo de retorno
- **WHEN** una función declara devolver una closure
- **THEN** se emite el mismo diagnóstico

#### Scenario: Closure como tipo de campo
- **WHEN** una clase declara un campo cuyo tipo pretende ser una función
- **THEN** se emite el mismo diagnóstico

#### Scenario: Retornar una closure creada localmente
- **WHEN** una función construye una closure y la retorna
- **THEN** se emite un diagnóstico indicando que una closure no puede escapar de la función que la crea
