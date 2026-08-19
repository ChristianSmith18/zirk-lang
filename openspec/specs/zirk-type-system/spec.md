# zirk-type-system

## Purpose

Defines the types of the language, name resolution, mutability, inference and flow analysis.

This is where the rules of `ZIRK_LANGUAGE_SPEC.md` that most often surprise someone coming from another language are enforced: no numeric truthiness, no implicit conversions and no function overloading.
## Requirements
### Requirement: Runtime type identity is safe and representation-independent
Every value and declared type MUST expose compiler-provided `Type` identity.
Identity SHALL distinguish constructed generic arguments and provide broad
`TypeKind`, safe names, and declared implements/extends relationships without
exposing layout, private members, or optimized representation. Value type
observation and checked casts SHALL remain separate operations.

#### Scenario: Interface value contains a concrete class
- **WHEN** `value` is declared as an interface and contains an implementing class instance
- **THEN** `value.type()` identifies the concrete class
- **AND** the interface's own kind remains available through `InterfaceName.type()`

#### Scenario: Constructed generic types share optimized code
- **WHEN** two constructed generic types reuse an implementation representation
- **THEN** their `Type` identities remain distinct when their type arguments differ

### Requirement: Structural runtime metadata is explicitly generated
General runtime reflection MUST NOT enumerate members, invoke string-named
methods, access fields dynamically, retain decorators automatically, or expose
compiler syntax. A library requiring runtime structure SHALL generate or author
an ordinary typed descriptor, registry, factory, or accessor that obeys normal
visibility, typing, public API, compatibility, permission, and dead-code rules.

#### Scenario: Framework requires route metadata
- **WHEN** a decorator-backed framework needs routes after compilation
- **THEN** expansion generates an ordinary typed route registry
- **AND** runtime code does not rediscover erased decorator applications

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
- **WHEN** un bloque interno declara una variable con el nombre de una externa todavía visible
- **THEN** se emite un diagnóstico que señala ambas declaraciones
- **AND** no se crea una declaración que oculte silenciosamente la anterior

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

### Requirement: Shadowing and capture qualification
The checker SHALL reject a local declaration that hides a still-visible local or parameter. A lambda parameter MAY share a captured outer name only when the capture is addressed as `this.name`; the plain name SHALL denote the lambda-local binding.

#### Scenario: Nested local shadowing
- **WHEN** a nested block declares `label` while an outer local `label` remains visible
- **THEN** a compile-time diagnostic names the existing binding

#### Scenario: Qualified capture collision
- **WHEN** a lambda parameter is `prefix` and its body reads `this.prefix`
- **THEN** `prefix` resolves to the parameter and `this.prefix` resolves to the captured outer value

### Requirement: Parameter collection semantics
Every optional parameter SHALL retain an explicit type and SHALL follow required positional parameters. A variadic parameter SHALL be an ordered read-only `Iterable<T>` for the duration of the call.

#### Scenario: Untyped optional parameter
- **WHEN** a declaration contains `prefix?` without `: Type`
- **THEN** the checker emits a missing parameter type diagnostic

#### Scenario: Variadic iteration
- **WHEN** a variadic `...values: String` is used in `for value in values`
- **THEN** `value` has type `String`

### Requirement: Class defaults and constructor resolution
An unmodified class field SHALL be `public mut`. A class MAY declare multiple constructors with distinct effective signatures. Selection SHALL consider arity, types, optional parameters, and names and SHALL reject ambiguous or duplicate effective signatures.

#### Scenario: Default field modifiers
- **WHEN** a class declares `name: String;`
- **THEN** the member has the same access and mutability as `public mut name: String;`

#### Scenario: Reordered named construction
- **WHEN** `User(name: "Cristian", id: 1)` matches `construct(id: UInt64, name: String)`
- **THEN** the constructor is selected by labels and each value binds to its named parameter

### Requirement: Reserved operator methods
User-defined types SHALL implement language operator contracts through reserved methods including `_add` and `_subtract` in safe code. Native types SHALL NOT be reopened by application code. Operator methods SHALL NOT change operator precedence, arity, or evaluation category.

#### Scenario: User type addition
- **WHEN** a class implements a valid `_add(other: Self): Self`
- **THEN** `left + right` resolves to that implementation

#### Scenario: Native type replacement
- **WHEN** application code attempts to replace `_add` on `String` or a numeric native type
- **THEN** the checker rejects reopening the native type

### Requirement: Value, enum, array, iteration, and generator semantics
Records SHALL be immutable with structural field equality; value classes SHALL be distinct domain types without observable identity and SHALL be storable inline; an unmapped traditional enum case SHALL expose its exact case name as its default string value and no implicit numeric index; all arrays SHALL have fixed length; `String` SHALL be iterable; and a generator SHALL be both `Iterator<T>` and `Iterable<T>` while preserving locals between yields.

#### Scenario: Enum default and explicit mapping
- **WHEN** `Direction.North` has no mapping and `Code.North` maps to `"N"`
- **THEN** their observable mapped strings are `"North"` and `"N"` respectively, while both values retain their enum types

#### Scenario: Fixed inferred array
- **WHEN** an array literal contains three elements
- **THEN** its length is fixed at three and append/remove operations are rejected

#### Scenario: String iteration
- **WHEN** a string is consumed by `for ... in`
- **THEN** iteration produces the string's public character units in order

### Requirement: Bound callable cloning and pipelines
`clone(receiver.method)` SHALL produce a local callable bound to the same receiver and preserving the method's parameters, result, effects, errors, permissions, and safety contract without cloning the receiver. The pipe operator SHALL pass its left value into the next ordinary function and SHALL NOT require that function to be a method of the value's class.

#### Scenario: Cloned stdout method
- **WHEN** `inmut print = clone(stdout.println)` is followed by `print("hello")`
- **THEN** the call invokes `println` on the original `stdout` receiver

#### Scenario: Pure-function pipeline
- **WHEN** `users |> filter(is_active) |> map(to_summary)` is checked
- **THEN** each stage is checked as an ordinary typed function receiving the previous stage's result

### Requirement: Public type taxonomy
The language SHALL distinguish compiler primitives, native reference types, user-defined value/reference types, and special types while retaining `Object` as their conceptual root. Storage inline or behind a reference SHALL NOT remove a value from the common type and contract system.

#### Scenario: Primitive with methods
- **WHEN** an `Int32` value invokes `abs()` or `to_string()`
- **THEN** the operation resolves through its native capabilities without boxing being observable

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

### Requirement: Deep contextual conversion
An explicit numeric or String constructor around an operator expression SHALL establish the target domain for the contained compatible arithmetic or concatenation tree, converting operands before those operators execute. The context SHALL NOT mutate operands or propagate through a called function's body.

#### Scenario: Contextual floating division
- **WHEN** `a` and `b` are integers equal to 3 and 4 and `Float(a / b)` is evaluated
- **THEN** division occurs in the Float domain and returns `0.75`

#### Scenario: Contextual String concatenation
- **WHEN** `String("value=" + 42)` is evaluated
- **THEN** the integer operand is converted before concatenation and the result is `"value=42"`

### Requirement: Reference mutability and strict aliases
For reference types, `mut` SHALL permit binding reassignment and referent mutation, `inmut` SHALL prohibit reassignment but permit referent mutation, and `inmut::strict` SHALL prohibit both. A strict reference SHALL NOT yield a mutable alias or be acquired while an accessible mutable alias exists; an independent `clone()` MAY be mutable.

#### Scenario: Inmut String element update
- **WHEN** an `inmut String` binding assigns a valid `Char` to one element
- **THEN** the shared referenced String is updated while binding reassignment remains prohibited

#### Scenario: Strict-to-mutable alias
- **WHEN** code assigns an `inmut::strict String` reference to a `mut` binding without cloning
- **THEN** type checking rejects the alias

### Requirement: Grapheme Char
`Char` SHALL represent exactly one Unicode grapheme, potentially containing multiple code points and bytes. `ascii_code()` SHALL return its ASCII code only when the grapheme is exactly one ASCII scalar and `-1` otherwise; case transformations SHALL return `String`.

#### Scenario: Multi-code-point grapheme
- **WHEN** a family emoji literal contains one extended grapheme
- **THEN** it is a valid single `Char` even though it contains multiple code points

#### Scenario: Non-ASCII code
- **WHEN** `ascii_code()` is invoked on `'é'`
- **THEN** it returns `-1`

### Requirement: Native String reference semantics and operators
`String` SHALL be a native reference type with shared mutation, explicit deep cloning, grapheme indexing/slicing, content equality, identity testing, checked concatenation, and checked repetition by a non-negative integer in either operand order.

#### Scenario: Shared String mutation
- **WHEN** two mutable bindings alias one String and one assigns a grapheme at an index
- **THEN** both bindings observe the changed content

#### Scenario: String repetition
- **WHEN** `"ja" * 3` or `3 * "ja"` is evaluated
- **THEN** the result is `"jajaja"`

#### Scenario: Negative repetition
- **WHEN** a String repetition count is negative
- **THEN** a controlled invalid-count error is produced

### Requirement: Native operator contracts by type
Each native type SHALL expose only its documented operator set. Integer division SHALL truncate toward zero, remainder SHALL preserve the dividend sign, mixed integer/Float arithmetic SHALL produce Float, Boolean SHALL have no truthiness, and unsupported operations SHALL fail at type checking.

#### Scenario: Signed remainder
- **WHEN** `-10 % 3` is evaluated
- **THEN** the result is `-1`

#### Scenario: Boolean arithmetic
- **WHEN** application code attempts `true + false`
- **THEN** type checking rejects the operation

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

### Requirement: Projection copy and whole-reference aliasing
The checker SHALL classify reference expressions as whole references, projection reads, or places. Whole-reference assignment/passing/return/capture SHALL preserve aliasing; projection reads SHALL require deep Clone and produce independence; places SHALL preserve access to original storage. Destructuring, matching, callable capture, collection extraction, and generic T SHALL follow the same rule.

#### Scenario: Generic projection needs Clone
- **WHEN** a generic function returns `values[0]` for unconstrained T
- **THEN** the checker requires `T from Clone` or rejects extraction

### Requirement: Final callable and object typing supersedes delivery limits
The final language type system SHALL support structural `Fn` adaptation, escaping closures, compiler-managed capture environments, abstract-class implementation, explicit overrides, declared generic variance, normalized unions, constant tuple indexes, copied iteration, and exhaustive guard-free matching. Feature phasing MAY diagnose an undelivered construct but SHALL NOT describe the final construct as semantically forbidden.

#### Scenario: Phase-limited closure
- **WHEN** the current compiler phase does not yet implement escaping closures
- **THEN** its diagnostic identifies the delivery phase while documentation retains the final legal Fn semantics

### Requirement: Default initialization and immutable data
Every omitted attribute SHALL receive its type default. Construction MAY finalize an `inmut` attribute before the object becomes available. Records and tuples SHALL remain immutable values; enums SHALL remain closed data without user methods; collections SHALL enforce referent permissions and strict aliases.

#### Scenario: Omitted class attribute
- **WHEN** an Int32 class attribute has no initializer and construct does not replace it
- **THEN** its value is zero after construction

### Requirement: Failure effects and Result consumption are checked
The checker SHALL require explicit exceptions to be handled or declared, SHALL propagate declared exception sets through calls and callable compatibility, SHALL permit documented implicit `RuntimeError` exceptions without signature declaration, and SHALL reject an unconsumed `Result` except through explicit discard.

#### Scenario: Callable throws too broadly
- **WHEN** a callable declaring `throws StorageError` is assigned to `Fn() => Void`
- **THEN** assignment fails because the target does not permit that explicit exception

### Requirement: Resource responsibility is flow-sensitive
The checker SHALL track managed, transferred, closed, dependent, and abandoned resource responsibility through branches, returns, containers, closures, tasks, and exceptional exits. It SHALL reject statically provable duplicate close, use-after-transfer, illegal escape, non-cloneable projection, and leak paths.

#### Scenario: Every branch transfers or closes
- **WHEN** all control-flow paths either close or transfer one resource responsibility
- **THEN** the function satisfies resource lifetime checking

### Requirement: Permission effects propagate outside surface callable syntax
The checker SHALL retain compiler-internal permission-effect metadata on declarations and callable values, infer it transitively through higher-order calls, and report a path from entry point to privileged API. Permission effects SHALL NOT alter the written `Fn(P...) => R` grammar.

#### Scenario: Higher-order permission propagation
- **WHEN** a permission-free wrapper invokes a callback whose concrete value reads a secret
- **THEN** the call site and application acquire the secret-read requirement in compiler metadata

### Requirement: Memory and task type family
The type system SHALL define `Weak<T>`, `Pointer<T>`, `NativeSlice<T>`, `NativeSliceMut<T>`, `Task<T>`, `TaskSettlement<T>`, `Channel<T>`, synchronization types, and their capability constraints without exposing mandatory ownership or lifetime parameters.

#### Scenario: Await type is inferred
- **WHEN** an expression has type `Task<Result<User, LoadError>>`
- **THEN** awaiting it has type `Result<User, LoadError>`

### Requirement: Derived concurrent capabilities
Transferability and shareability SHALL be compiler-derived, non-forgeable properties based on the complete reachable type graph, mutability, resource ownership, and synchronization contract.

#### Scenario: Class contains mutex
- **WHEN** a class safely encapsulates mutable state behind a supported mutex
- **THEN** the compiler may derive sharing without exposing an ordinary user-implemented marker

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

### Requirement: Strictness de referencias de objeto

En referencias de clase, `mut` SHALL permitir reasignar y mutar el objeto,
`inmut` SHALL impedir solo la reasignación, e `inmut::strict` SHALL impedir la
mutación alcanzable. Una referencia strict NO SHALL convertirse en alias
mutable ni adquirirse mientras permanezca accesible un alias mutable.

#### Scenario: Clon mutable desde referencia strict
- **WHEN** un objeto strict implementa `Cloneable` y se clona a un binding `mut`
- **THEN** el clon independiente puede mutarse sin alterar el objeto original

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

### Requirement: La sintaxis final de tipos función no se entrega en Fase 3

El chequeador de Fase 3 SHALL rechazar cualquier posición que exija anotar el
tipo de una closure —parámetro, retorno o atributo— con un diagnóstico que
indique que `Function(P...) => R` / `Fn(P...) => R` llega en una fase posterior.

Este es un límite de implementación, no la semántica final. El lenguaje ya
define compatibilidad por firma, closures escapables y almacenamiento
automático conforme a `docs/CORE_LANGUAGE_SEMANTICS.md`.

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

#### Scenario: Closure como tipo de atributo
- **WHEN** una clase declara un atributo cuyo tipo pretende ser una función
- **THEN** se emite el mismo diagnóstico

#### Scenario: Retornar una closure creada localmente
- **WHEN** una función construye una closure y la retorna
- **THEN** se emite un diagnóstico indicando que una closure no puede escapar de la función que la crea

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

