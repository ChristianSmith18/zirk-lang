# zirk-grammar

## Purpose

Defines the grammar of the language and the construction of the syntax tree, together with the errors it reports.

The parser decides whether a program is well *formed*, not whether it makes *sense*: that belongs to the type system.
## Requirements
### Requirement: Declaración de funciones

El parser SHALL reconocer declaraciones de función con la forma de `ZIRK_LANGUAGE_SPEC.md` sección 6: `fn` nombre, lista de parámetros tipados, tipo de retorno tras `:`, y cuerpo entre llaves.

#### Scenario: Función sin parámetros
- **WHEN** se parsea `fn main(): Void { }`
- **THEN** se produce una declaración de función llamada `main`, sin parámetros y con retorno `Void`

#### Scenario: Función con parámetros
- **WHEN** se parsea `fn add(a: Int32, b: Int32): Int32 { return a + b; }`
- **THEN** se produce una función con dos parámetros tipados y retorno `Int32`

#### Scenario: Tipo de retorno ausente
- **WHEN** una función se declara sin tipo de retorno
- **THEN** se emite un diagnóstico que señala dónde se esperaba el tipo
- **AND** la ayuda indica que el tipo de retorno es obligatorio en esta fase

### Requirement: Declaración de variables

El parser SHALL reconocer declaraciones con `mut` e `inmut`, con anotación de tipo opcional cuando haya inicializador.

#### Scenario: Variable con tipo explícito
- **WHEN** se parsea `mut count: Int32 = 0;`
- **THEN** se produce una declaración mutable con tipo `Int32` e inicializador

#### Scenario: Variable con tipo inferido
- **WHEN** se parsea `mut count = 0;`
- **THEN** se produce una declaración mutable sin anotación de tipo

#### Scenario: Declaración sin inicializador ni tipo
- **WHEN** se parsea `mut count;`
- **THEN** se emite un diagnóstico indicando que falta el tipo o el inicializador

### Requirement: Expresiones y precedencia

El parser SHALL construir expresiones respetando la precedencia y asociatividad convencionales de los operadores de `ZIRK_LANGUAGE_SPEC.md` sección 4.

De mayor a menor precedencia: exponenciación (`**`, asociativa a la derecha); unarios (`!`, `-`); multiplicativos (`*`, `/`, `%`); aditivos (`+`, `-`); comparación (`<`, `<=`, `>`, `>=`); igualdad (`==`, `!=`); conjunción (`&&`); disyunción (`||`).

#### Scenario: Precedencia multiplicativa sobre aditiva
- **WHEN** se parsea `1 + 2 * 3`
- **THEN** el árbol representa `1 + (2 * 3)`

#### Scenario: Asociatividad izquierda
- **WHEN** se parsea `10 - 4 - 3`
- **THEN** el árbol representa `(10 - 4) - 3`

#### Scenario: Paréntesis alteran la precedencia
- **WHEN** se parsea `(1 + 2) * 3`
- **THEN** el árbol representa la suma como operando izquierdo del producto

#### Scenario: Conjunción sobre disyunción
- **WHEN** se parsea `a || b && c`
- **THEN** el árbol representa `a || (b && c)`

### Requirement: Sentencia condicional

El parser SHALL reconocer `if` y `else` como sentencia, con cuerpos entre llaves salvo en la forma de efecto que gobierna una sola sentencia. Cuando ambas ramas están presentes, `if`/`else` SHALL admitirse también como expresión, según `ZIRK_LANGUAGE_SPEC.md` sección 5.

Que sea sentencia o expresión no lo decide el parser: lo decide el chequeo de tipos, según si el uso exige un valor y ambas ramas son type-compatible.

Los paréntesis alrededor de la condición SHALL ser opcionales.

#### Scenario: Condicional simple
- **WHEN** se parsea `if x > 0 { }`
- **THEN** se produce una sentencia condicional sin rama alternativa

#### Scenario: Condicional con alternativa
- **WHEN** se parsea `if x > 0 { } else { }`
- **THEN** se produce una sentencia condicional con ambas ramas

#### Scenario: Encadenamiento
- **WHEN** se parsea `if a { } else if b { } else { }`
- **THEN** se produce un condicional cuya rama alternativa es otro condicional

#### Scenario: Cuerpo sin llaves
- **WHEN** se parsea `if closed return;`
- **THEN** se produce un condicional cuya rama única es esa sentencia

#### Scenario: Rama alternativa sobre un cuerpo sin llaves
- **WHEN** un `if` sin llaves va seguido de `else`
- **THEN** se emite un diagnóstico indicando que la forma sin llaves gobierna una sola sentencia

#### Scenario: Condición entre paréntesis
- **WHEN** se parsea `if (x > 0) { }`
- **THEN** se produce el mismo árbol que sin paréntesis

#### Scenario: Uso como expresión
- **WHEN** se parsea `mut resultado = if x > 0 { "positivo" } else { "no positivo" };`
- **THEN** se produce una declaración cuyo inicializador es el condicional

### Requirement: Llamadas a función y retorno

El parser SHALL reconocer llamadas con argumentos posicionales y la sentencia `return`.

#### Scenario: Llamada con argumentos
- **WHEN** se parsea `add(1, 2)`
- **THEN** se produce una llamada con dos argumentos

#### Scenario: Retorno con valor
- **WHEN** se parsea `return a + b;`
- **THEN** se produce un retorno cuya expresión es la suma

#### Scenario: Retorno sin valor
- **WHEN** se parsea `return;`
- **THEN** se produce un retorno sin expresión

### Requirement: Punto y coma opcional

El parser SHALL admitir la omisión del punto y coma cuando no haya ambigüedad, según `ZIRK_LANGUAGE_SPEC.md` sección 1.

#### Scenario: Sentencias sin punto y coma
- **WHEN** se parsean sentencias separadas por saltos de línea y sin `;`
- **THEN** el árbol resultante es equivalente al de las mismas sentencias con `;`

### Requirement: Construcciones fuera del subset

El parser SHALL emitir un diagnóstico específico ante construcciones que existen en el lenguaje pero no están implementadas todavía, distinguiéndolas de errores de sintaxis.

Esta fase retira de esa lista `class`, `construct`, `this`, `record`, `type`, `public`, `private`, `protected`, `abstract`, `implements`, `extends`, `from`, `as` e `is`.

#### Scenario: Construcción de fase posterior
- **WHEN** se parsea `try`, `task`, `parallel` o `thread`
- **THEN** el diagnóstico SHALL nombrar la construcción
- **AND** SHALL indicar que no está implementada todavía
- **AND** NO SHALL reportarse como token inesperado

#### Scenario: `init.zrk` fuera de alcance
- **WHEN** se encuentra un archivo `init.zrk`
- **THEN** el diagnóstico indica que la configuración declarativa de proyecto llega en una fase posterior

### Requirement: Ubicación en todo nodo

Todo nodo del árbol SHALL exponer el span del source que lo originó.

Un nodo sin ubicación no puede producir el diagnóstico que exige `ZIRK_COMPILER_SPEC.md` sección 8.

#### Scenario: Span de un nodo cualquiera
- **WHEN** se inspecciona cualquier nodo del árbol producido
- **THEN** expone la ubicación de inicio y fin en el source

### Requirement: Bucles

El parser SHALL reconocer `for` con inicialización/condición/incremento, `for ... in` sobre una expresión iterable, `while`, `do ... while` y `loop`, junto con `break` y `continue`, según `ZIRK_LANGUAGE_SPEC.md` sección 5.

Los paréntesis alrededor del header SHALL ser opcionales en todas estas formas. La forma canónica los omite y ambas SHALL producir el mismo árbol.

#### Scenario: `for` con las tres cláusulas
- **WHEN** se parsea `for mut i = 0; i < 10; i++ { }`
- **THEN** se produce un bucle con inicialización, condición e incremento

#### Scenario: `for` con paréntesis
- **WHEN** se parsea `for (mut i = 0; i < 10; i++) { }`
- **THEN** se produce el mismo árbol que sin paréntesis

#### Scenario: `for ... in`
- **WHEN** se parsea `for x in 0..10 { }`
- **THEN** se produce un bucle que itera la variable `x` sobre el rango

#### Scenario: `while`
- **WHEN** se parsea `while x > 0 { }` o `while (x > 0) { }`
- **THEN** se produce un bucle condicional en ambos casos

#### Scenario: `do ... while`
- **WHEN** se parsea `do { poll(); } while pending;`
- **THEN** se produce un bucle de post-condición cuyo cuerpo precede a su condición

#### Scenario: `loop`
- **WHEN** se parsea `loop { break; }`
- **THEN** se produce un bucle incondicional cuyo cuerpo contiene `break`

#### Scenario: `break` y `continue` fuera de un bucle
- **WHEN** se parsea `break;` o `continue;` fuera de cualquier bucle
- **THEN** se emite un diagnóstico indicando que solo son válidos dentro de un bucle

### Requirement: Parámetros opcionales, nombrados, variadic y valores por defecto

El parser SHALL reconocer parámetros con signo `?` para opcionales, valores por defecto tras `=`, y un parámetro variadic prefijado con `...`, según `ZIRK_LANGUAGE_SPEC.md` sección 6.

#### Scenario: Parámetro opcional
- **WHEN** se parsea `fn saludo(nombre?: String): Void { }`
- **THEN** se produce un parámetro marcado como opcional

#### Scenario: Valor por defecto
- **WHEN** se parsea `fn saludo(nombre: String = "mundo"): Void { }`
- **THEN** se produce un parámetro con expresión de valor por defecto

#### Scenario: Parámetro variadic
- **WHEN** se parsea `fn suma(...valores: Int32): Int32 { }`
- **THEN** se produce un parámetro variadic
- **AND** un variadic que no es el último parámetro produce un diagnóstico

#### Scenario: Argumentos nombrados en la llamada
- **WHEN** se parsea `saludo(nombre: "Ana")`
- **THEN** se produce una llamada con un argumento nombrado

### Requirement: Closures y lambdas

El parser SHALL reconocer expresiones lambda con la forma `(parámetros): TipoRetorno => expresión` o `(parámetros): TipoRetorno => { ... }`, según `ZIRK_LANGUAGE_SPEC.md` sección 6.

#### Scenario: Lambda de expresión
- **WHEN** se parsea `(a: Int32, b: Int32): Int32 => a + b`
- **THEN** se produce una lambda cuyo cuerpo es la expresión de suma

#### Scenario: Lambda de bloque
- **WHEN** se parsea `(): Void => { stdout.println("ok"); }`
- **THEN** se produce una lambda cuyo cuerpo es un bloque de sentencias

### Requirement: `match`

El parser SHALL reconocer `match` tanto en posición de expresión como de sentencia, con brazos de la forma `patrón => cuerpo`, según `ZIRK_LANGUAGE_SPEC.md` sección 5.

Los patrones admitidos esta fase son: literales, constructores de un `enum` sin datos asociados, variables de binding y el comodín `_`.

#### Scenario: `match` como sentencia
- **WHEN** se parsea `match direccion { Direction.North => stdout.println("norte"); _ => {} }`
- **THEN** se produce una sentencia `match` con sus brazos

#### Scenario: `match` como expresión
- **WHEN** se parsea `mut texto = match direccion { Direction.North => "norte"; _ => "otra" };`
- **THEN** se produce una declaración cuyo inicializador es el `match`

#### Scenario: Declaración de `enum` mínimo
- **WHEN** se parsea `enum Direction { North, South, East, West }`
- **THEN** se produce una declaración de enum con cuatro constructores sin datos asociados

#### Scenario: `match with` fuera de alcance
- **WHEN** se parsea `match with`
- **THEN** el diagnóstico indica que `Resource<E>` y `match with` llegan en una fase posterior

### Requirement: Nulabilidad

El parser SHALL reconocer `T?` como anotación de tipo, `null` como literal, `??` como coalescencia nula y `?.` como acceso seguro, según `ZIRK_LANGUAGE_SPEC.md` sección 4.

`?.` deja de diferirse: la fase anterior lo pospuso porque ningún tipo tenía miembros, y las clases los traen.

#### Scenario: Tipo nulable
- **WHEN** se parsea `mut name: String? = null;`
- **THEN** se produce una declaración con tipo `String?` e inicializador nulo

#### Scenario: Coalescencia nula
- **WHEN** se parsea `name ?? "anónimo"`
- **THEN** se produce una expresión con fallback

#### Scenario: Precedencia de `??`
- **WHEN** se parsea `a ?? b || c`
- **THEN** el árbol representa `(a ?? b) || c`

#### Scenario: Acceso seguro
- **WHEN** se parsea `usuario?.nombre`
- **THEN** se produce una expresión de acceso seguro sobre el miembro

### Requirement: Asignación compuesta e incremento

El parser SHALL reconocer `+=`, `-=`, `*=`, `/=`, `%=`, `**=`, `++` y `--` en posición de sentencia, expandiéndolos al árbol de la asignación equivalente.

`++` y `--` SHALL admitirse además en posición de expresión, preservando la semántica convencional de prefijo y postfijo de `ZIRK_LANGUAGE_SPEC.md` sección 4: la forma postfija evalúa al valor previo y la prefija al valor ya incrementado. Ambas SHALL exigir un lugar asignable y mutable.

#### Scenario: Asignación compuesta
- **WHEN** se parsea `total += 5;`
- **THEN** el árbol producido es equivalente al de `total = total + 5;`

#### Scenario: Incremento
- **WHEN** se parsea `i++;` o `++i;`
- **THEN** el árbol producido es equivalente al de `i = i + 1;`

#### Scenario: Incremento postfijo en posición de expresión
- **WHEN** se parsea `mut x = i++;`
- **THEN** `x` recibe el valor de `i` previo al incremento

#### Scenario: Incremento prefijo en posición de expresión
- **WHEN** se parsea `mut x = ++i;`
- **THEN** `x` recibe el valor de `i` ya incrementado

#### Scenario: Incremento sobre un lugar no asignable
- **WHEN** el operando de `++` no es un lugar asignable y mutable
- **THEN** se emite un diagnóstico que señala el operando

### Requirement: Módulos dentro de un crate

El parser SHALL reconocer `share` como modificador de declaración, `import { nombres } from "ruta"` con rutas locales entre comillas y módulos estándar sin comillas, y `use` para habilitar globals, según `ZIRK_LANGUAGE_SPEC.md` sección 10.

#### Scenario: Declaración compartida
- **WHEN** se parsea `share class Usuario {}`
- **THEN** se emite el diagnóstico correspondiente a `class`, que sigue fuera de alcance esta fase
- **AND** `share fn saludo(): Void {}` sí se acepta, marcando la función como compartida

#### Scenario: Importación local
- **WHEN** se parsea `import { Usuario, Rol } from "./dominio/usuario";`
- **THEN** se produce una importación con ruta local y dos nombres

#### Scenario: Importación con alias
- **WHEN** se parsea `import { Rol -> RolDeDominio } from "./dominio/usuario";`
- **THEN** el nombre importado se expone bajo el alias

#### Scenario: `init.zrk` no se importa desde código
- **WHEN** se encuentra un `import` de configuración de proyecto
- **THEN** se emite el diagnóstico de `init.zrk` fuera de alcance

### Requirement: Complete range and slice forms
The parser SHALL accept `start..end`, `start..=end`, descending bounds, `.step(distance)`, `.reverse()`, interpolated bounds such as `0..{number}`, and slices `[start:end:step]` with omitted or negative components.

#### Scenario: Descending stepped range
- **WHEN** source contains `10..=0.step(2)`
- **THEN** it represents an inclusive descending range with distance two

#### Scenario: Reverse slice
- **WHEN** source contains `values[::-1]`
- **THEN** it represents a slice with omitted bounds and negative step

### Requirement: Match alternatives, regex, and nested destructuring
The parser SHALL group alternative patterns with commas followed by one `=>` body, SHALL accept regex literals as patterns, and SHALL compose enum payload and record destructuring as `UserCreated({ id, name })`.

#### Scenario: Multiline alternatives
- **WHEN** `200`, `201`, and `204` appear on separate lines before one `=>` body
- **THEN** all three patterns select that body

#### Scenario: Nested enum payload pattern
- **WHEN** source contains `UserCreated({ id, name }) => handle(id, name)`
- **THEN** the enum payload is destructured and both names bind in the branch

### Requirement: Function and constructor surface
The parser SHALL accept lambdas with or without leading `fn`, SHALL require a type on every optional parameter, SHALL accept multiple `construct` declarations, SHALL accept reordered named construction arguments, and SHALL parse `fn gen` with `yield`.

#### Scenario: Optional fn lambda
- **WHEN** source contains `fn(a: Int32): Int32 => a + 1`
- **THEN** it produces the same lambda form as `(a: Int32): Int32 => a + 1`

#### Scenario: Multiple constructors
- **WHEN** a class contains two `construct` declarations with different parameter lists
- **THEN** both constructor signatures are retained for semantic resolution

#### Scenario: Generator declaration
- **WHEN** source contains `fn gen numbers(): Int32 { yield 1; }`
- **THEN** it produces a generator function whose declared type is its yielded element type

### Requirement: Traditional enum mappings
The parser SHALL accept traditional enum cases without mappings and cases mapped with `->` to compatible string or numeric values.

#### Scenario: String-mapped case
- **WHEN** source contains `North -> "N";`
- **THEN** the enum case retains `"N"` as its explicit observable mapping

### Requirement: Contextual constructor expressions
The grammar SHALL retain the complete contained operator tree of `Float(expression)` and `String(expression)` so semantic analysis can apply explicit deep contextual conversion before evaluating compatible contained arithmetic or concatenation operators.

#### Scenario: Nested contextual arithmetic
- **WHEN** `Float((a + 1) / (b * 2))` is parsed
- **THEN** the constructor contains the entire nested arithmetic tree rather than an already-evaluated integer result

### Requirement: Native String repetition syntax
The grammar SHALL accept multiplication and compound multiplication between a String expression and an integer expression, leaving type checking to enforce operand types, non-negative counts, mutability for `*=`, and allocation bounds.

#### Scenario: Compound String repetition
- **WHEN** `laugh *= 3` is parsed
- **THEN** it is represented as a compound assignment whose semantic result is String repetition

### Requirement: Temporal construction and composition syntax
The grammar SHALL accept ordinary typed constructors, named components, method calls, ISO strings, and the documented temporal operator combinations without introducing a single ambiguous all-purpose Date literal.

#### Scenario: Named Duration construction
- **WHEN** `Duration(hours: 2, minutes: 30)` is parsed
- **THEN** the constructor retains both named temporal components

#### Scenario: Date and Time composition
- **WHEN** `date + time` is parsed
- **THEN** it remains a binary operation for type-directed `DateTime` composition

### Requirement: Paréntesis opcionales en headers de control

El parser SHALL admitir paréntesis opcionales alrededor del header de toda estructura de control —`if`, `while`, `for`, `for ... in`, `do ... while` y `match`— produciendo el mismo árbol con y sin ellos.

La forma sin paréntesis es la canónica. Ninguna estructura SHALL exigirlos.

Esta regla no estaba escrita en ninguna fuente normativa, y esa ausencia es lo que llevó a exigirlos en el `for` tradicional.

#### Scenario: Header sin paréntesis
- **WHEN** se parsea `while pending { }`
- **THEN** el parseo tiene éxito

#### Scenario: Header con paréntesis
- **WHEN** se parsea `while (pending) { }`
- **THEN** el parseo tiene éxito y produce el mismo árbol

#### Scenario: `for ... in` con paréntesis
- **WHEN** se parsea `for (x in 0..10) { }`
- **THEN** se produce el mismo árbol que `for x in 0..10 { }`

#### Scenario: `match` con paréntesis
- **WHEN** se parsea `match (valor) { }`
- **THEN** se produce el mismo árbol que `match valor { }`

### Requirement: Expresión ternaria

El parser SHALL reconocer la expresión ternaria `condición ? cuando_true : cuando_false`, asociativa a la derecha, según el nivel 16 de la tabla de precedencia.

El ternario es la forma compacta preferida para elegir un valor corto, y NO SHALL reemplazar al `if` en posición de expresión: ambos coexisten.

#### Scenario: Ternario simple
- **WHEN** se parsea `mut label = activo ? "sí" : "no";`
- **THEN** el inicializador es una expresión ternaria con sus tres partes

#### Scenario: Ternario anidado
- **WHEN** se parsea `a ? b : c ? d : e`
- **THEN** el anidamiento se agrupa por la derecha

#### Scenario: Ternario sin rama alternativa
- **WHEN** falta `:` y su expresión
- **THEN** se emite un diagnóstico que señala el ternario incompleto

### Requirement: Refined core syntax surface
The parser SHALL recognize `Fn(P...) => R` and `Function(P...) => R`, `override fn`, abstract classes adopted with `implements`, combined `from A & B` constraints, `in/out` generic variance, Tuple type/index syntax, collection literals, `as?`, and the accepted guard-free pattern forms. It SHALL reject property declarations, standalone override, enum user methods, match guards, and enum destructuring bindings.

#### Scenario: Callable type annotation
- **WHEN** source contains `mut print: Fn(String) => Void = stdout.println`
- **THEN** the parser produces a mutable binding whose annotation is a callable type

#### Scenario: Removed property syntax
- **WHEN** source declares `property name: String`
- **THEN** it receives a targeted diagnostic recommending an attribute plus `get_name`/`set_name` methods

### Requirement: Slice omission syntax
The parser SHALL preserve independently omitted start, end, and step components in `[:]`, `[::]`, `[n:]`, `[:w]`, `[n:w]`, `[::k]`, and reverse forms so semantic analysis can apply direction-sensitive defaults.

#### Scenario: Fully omitted slice
- **WHEN** `[::]` is parsed
- **THEN** start, end, and step are represented as omitted rather than fabricated source literals

### Requirement: Failure and resource syntax is unambiguous
The grammar SHALL accept `throws T | U` after a return type, `throw expression`, exact `throw;` inside a catch, `try` followed by pattern-shaped `catch Type(binding)` clauses and optional `finally`, and `match acquisition with binding` including grouped acquisitions. Historical `catch<Type> name` SHALL be rejected with migration guidance.

#### Scenario: Typed catch parses
- **WHEN** source contains `catch NetworkError.Timeout(duration) { retry(duration); }`
- **THEN** the parser produces a typed variant catch pattern without a guard

### Requirement: Permission manifests use requires and permissions
The manifest grammar SHALL accept library `requires`, application `permissions`, operation-specific scopes, and `during: build | runtime | both`. It SHALL reject a top-level `compile_permissions` block with guidance to move the phase into the relevant grant.

#### Scenario: Build-only filesystem grant
- **WHEN** `init.zrk` grants a filesystem read operation with `during: build`
- **THEN** the manifest AST preserves the operation, scope, and phase separately

### Requirement: Named argument shorthand is explicit
The call grammar SHALL accept `name: expression` as an explicit named argument
and `name:` as shorthand for `name: name`. The shorthand SHALL accept only a
simple identifier, SHALL remain reorderable with other named arguments, and
SHALL NOT reinterpret a reordered bare positional identifier by matching its
spelling to a parameter.

#### Scenario: Reordered shorthand arguments
- **WHEN** source calls `client.get(timeout:, url:)`
- **THEN** the AST records named arguments equivalent to
  `client.get(timeout: timeout, url: url)`

#### Scenario: Member expression uses explicit value
- **WHEN** source attempts `client.get(config.url:)`
- **THEN** compilation diagnoses invalid shorthand and recommends
  `client.get(url: config.url)`

#### Scenario: Positional argument follows a named argument
- **WHEN** source calls `client.get(timeout:, url)`
- **THEN** compilation rejects the positional argument after the named argument

### Requirement: Comma-grouped declarations and assignments are explicit
The grammar SHALL accept a comma-separated list of simple binding names before
one shared type annotation in a `mut`, `inmut`, or `inmut::strict` declaration.
It SHALL accept an optional comma-separated initializer list and simultaneous
assignment to a comma-separated list of assignable places. These forms SHALL
remain distinct from tuple construction and destructuring patterns.

#### Scenario: Shared-type declaration
- **WHEN** source declares `mut first, second: String;`
- **THEN** the AST records two mutable bindings with the shared `String` type

#### Scenario: Simultaneous swap
- **WHEN** source assigns `left, right = right, left;`
- **THEN** the AST records one simultaneous assignment with two destinations
  and two source expressions

#### Scenario: Assignment arity mismatch
- **WHEN** source assigns `left, right = right, left, extra;`
- **THEN** parsing preserves both arities so semantic analysis can emit a
  targeted count-mismatch diagnostic

### Requirement: Safety and concurrency grammar
The grammar SHALL parse unsafe function modifiers and blocks, `commit` regions, `task` blocks and callable sugar, `task scope`, `cancellation shield`, await timeouts, and `select` branches with `after`, `default`, and cancellation cases without introducing `async fn`.

#### Scenario: Select statement is parsed
- **WHEN** source contains task, channel, timer, and default select branches
- **THEN** the parser produces distinct guarded branches and their result bindings

### Requirement: Contextual safety restrictions
The parser and semantic frontend SHALL preserve enough contextual information to diagnose `await`, task/thread creation, or irreversible effects in a reversible unsafe transaction and unsafe-only operations outside an unsafe boundary.

#### Scenario: Commit appears outside unsafe
- **WHEN** source places `commit {}` outside an unsafe block
- **THEN** compilation fails with a contextual syntax or semantic diagnostic

### Requirement: Decorator declarations have explicit forms
The grammar SHALL recognize `fn dec Name(parameters) { target-blocks }` and `repeatable fn dec Name(parameters) { target-blocks }`. Each target block SHALL be named only `class`, `attribute`, `function`, `method`, or `parameter` and SHALL bind its typed target explicitly.

#### Scenario: Repeatable method decorator parses
- **WHEN** `repeatable fn dec Middleware(name: String) { method(target) { ... } }` is parsed
- **THEN** the syntax tree records a repeatable decorator, its typed argument schema, and one method target block

#### Scenario: Unsupported decorator target parses for diagnosis
- **WHEN** a decorator contains `construct(target) { ... }`
- **THEN** the parser preserves a recoverable target block and emits a targeted unsupported-decorator-target diagnostic rather than a generic unexpected-token error

### Requirement: Decorator applications are ordered syntax
The grammar SHALL recognize `@Name` and `@Name(arguments)` applications before supported declarations and parameters, preserving exact top-to-bottom source order and application spans.

#### Scenario: Multiple applications decorate a method
- **WHEN** `@Authorized() @Cached(5.minutes) fn report(): Report { ... }` is parsed across one or multiple lines
- **THEN** both applications are attached to the method in source order with independent spans

### Requirement: Decorator dependency clauses are parsed explicitly
The grammar SHALL recognize optional `requires decorators [...]`, `before decorators [...]`, and `after decorators [...]` clauses after a decorator header and before its body. Each list SHALL contain decorator names and SHALL preserve its source order and spans for semantic validation.

#### Scenario: Ordering constraint parses
- **WHEN** `fn dec Authorized() before decorators [Cached] { ... }` is parsed
- **THEN** the decorator node contains a `before` constraint referencing `Cached`

### Requirement: Decorator phase patterns expose their payloads
The grammar SHALL parse `Inspect`, `Augment`, and `Wrap` variants inside `match target.transform`, including explicit repeatable payloads such as `Inspect(context, applications)`. It SHALL parse `Before`, `After`, `Catch`, and `Around` variants inside `match target.wrap` and SHALL apply normal exact-arity pattern parsing, including `_` payload omissions.

#### Scenario: Repeatable applications are explicitly bound
- **WHEN** `Inspect(context, applications) => { ... }` is parsed
- **THEN** the arm introduces both payload bindings and no implicit `applications` binding is added elsewhere

#### Scenario: Wrapper payload is ignored
- **WHEN** `After(result, _) => { ... }` is parsed
- **THEN** the arm binds `result` and records one wildcard payload position

### Requirement: Sintaxis de clases

El parser SHALL reconocer `class`, sus campos y métodos, `construct`, `this`, los modificadores de visibilidad, `abstract`, `extends` e `implements`, según `ZIRK_LANGUAGE_SPEC.md` sección 7.

#### Scenario: Clase completa
- **WHEN** se parsea `class User implements Serializable { public inmut id: Int32; construct(id: Int32) { this.id = id; } }`
- **THEN** se produce una declaración con un contrato implementado, un campo y un constructor

#### Scenario: Herencia y contratos combinados
- **WHEN** se parsea `class Admin extends User implements Auditable, Clone { }`
- **THEN** se produce una declaración con una superclase y dos contratos

#### Scenario: `construct` fuera de una clase
- **WHEN** `construct` aparece en el nivel superior del archivo
- **THEN** se emite un diagnóstico indicando que un constructor pertenece a una clase

#### Scenario: Varios constructores y argumentos nombrados
- **WHEN** una clase declara varios `construct` y se construye con argumentos nombrados reordenados
- **THEN** el árbol conserva todas las firmas y las etiquetas de cada argumento para la resolución semántica

### Requirement: Sintaxis de contratos

El parser SHALL reconocer `interface` y `trait` con sus métodos, y admitir cuerpo únicamente en los de un `trait`.

#### Scenario: Interfaz
- **WHEN** se parsea `interface Serializable { fn serialize(): String; }`
- **THEN** se produce una declaración con una firma sin cuerpo

#### Scenario: Trait con implementación
- **WHEN** se parsea `trait Greet { fn hello(): String { return "hola"; } }`
- **THEN** se produce una declaración cuyo método tiene cuerpo

### Requirement: Sintaxis de genéricos

El parser SHALL reconocer parámetros de tipo `<T>` en declaraciones, argumentos de tipo en los usos, y restricciones con `from`.

#### Scenario: Función genérica con restricción
- **WHEN** se parsea `fn serialize<T from Serializable>(value: T): String { }`
- **THEN** se produce un parámetro de tipo con una restricción

#### Scenario: Varios parámetros de tipo
- **WHEN** se parsea `class Map<K, V> { }`
- **THEN** se producen dos parámetros de tipo

#### Scenario: Argumento de tipo en el uso
- **WHEN** se parsea `mut b: Box<Int32>;`
- **THEN** el tipo lleva un argumento

#### Scenario: `<` que no abre genéricos
- **WHEN** se parsea `a < b`
- **THEN** se produce una comparación, no un argumento de tipo

### Requirement: Sintaxis de tipos de datos

El parser SHALL reconocer `record`, value classes, variantes de enum con datos asociados, uniones `A | B` y alias con `type`.

#### Scenario: Enum con datos asociados
- **WHEN** se parsea `enum Shape { Circle(Int32), Rect(Int32, Int32) }`
- **THEN** se producen dos variantes con uno y dos tipos asociados

#### Scenario: Mapping de enum tradicional
- **WHEN** se parsea `enum Direction { North -> "N", South }`
- **THEN** `North` conserva su mapping explícito y `South` queda sin mapping explícito

#### Scenario: Patrón con destructuring
- **WHEN** se parsea `match s { Shape.Circle(r) => r, _ => 0 }`
- **THEN** el patrón liga un nombre al valor asociado

#### Scenario: Unión
- **WHEN** se parsea `mut x: Int32 | String;`
- **THEN** se produce un tipo con dos miembros

#### Scenario: Alias
- **WHEN** se parsea `type Id = Int32;`
- **THEN** se produce una declaración de alias

### Requirement: Fase 3 todavía no parsea el tipo función final

El parser de Fase 3 NO SHALL aceptar todavía la sintaxis final
`Function(Int32, Int32) => Int32` ni su alias `Fn(Int32, Int32) => Int32`.

Es una restricción temporal del compilador de Fase 3. La sintaxis, compatibilidad
por firma y escape final ya están decididos en el checkpoint canónico.

#### Scenario: Tipo función en una anotación
- **WHEN** se parsea una anotación de tipo con la forma de una firma de función
- **THEN** se emite un diagnóstico indicando que los tipos función llegan en una fase posterior

#### Scenario: El lambda como expresión no cambia
- **WHEN** se parsea `(a: Int32): Int32 => a + 1` en posición de valor
- **THEN** se produce un lambda, igual que en la fase anterior

### Requirement: Sintaxis de casts

El parser SHALL reconocer la forma postfija `expr as T` y la prefija `<T>expr`, según `ZIRK_LANGUAGE_SPEC.md` sección 11.

#### Scenario: Cast postfijo
- **WHEN** se parsea `mut v = source as String;`
- **THEN** se produce una conversión al tipo nombrado

#### Scenario: Cast prefijo
- **WHEN** se parsea `mut v = <String>source;`
- **THEN** se produce la misma conversión que la forma postfija

#### Scenario: Cast que reinterpreta memoria
- **WHEN** se parsea un cast que exige `unsafe`
- **THEN** se emite un diagnóstico indicando que el nivel bajo llega en una fase posterior

### Requirement: Literales de anchos enteros y `Float`

La gramática SHALL reconocer un literal entero como cualquiera de los anchos con o sin signo cuando el contexto lo determina, y un literal fraccionario (con notación científica opcional) como `Float`, ambos con `_` como separador visual.

#### Scenario: Separador visual en un literal ancho
- **WHEN** se escribe `1_000_000`
- **THEN** se lexea como el entero `1000000`

#### Scenario: Notación científica
- **WHEN** se escribe `1e2`
- **THEN** se lexea como un literal `Float` de valor `100.0`

### Requirement: Literal `Char`

La gramática SHALL reconocer un literal `Char` delimitado por comillas simples, capaz de contener un grapheme Unicode extendido de más de un code point.

#### Scenario: Literal de un carácter ASCII
- **WHEN** se escribe `'a'`
- **THEN** se lexea como un literal `Char`

#### Scenario: Delimitador sin cerrar
- **WHEN** un literal `Char` no tiene su comilla de cierre antes del fin de línea
- **THEN** se emite un diagnóstico léxico

### Requirement: Operadores bitwise y de shift

La gramática SHALL reconocer `&`, `|`, `^`, `~`, `<<`, `>>` como operadores binarios (`~` unario), en los niveles de precedencia que `ZIRK_LANGUAGE_SPEC.md` fija para ellos, distintos de los lógicos `&&`/`||`.

#### Scenario: Precedencia distinta de la lógica
- **WHEN** se escribe una expresión que combina `&` con `&&`
- **THEN** se parsea según la precedencia de cada operador, no como si fueran el mismo

### Requirement: Interpolación en literales de `String`

La gramática SHALL reconocer `{expr}` dentro de un literal de `String` como una expresión interpolada, con `\{` como escape para una llave literal.

#### Scenario: Interpolación simple
- **WHEN** se escribe `"Hola, {nombre}"`
- **THEN** se parsea como texto literal más una expresión interpolada `nombre`

#### Scenario: Llave escapada
- **WHEN** se escribe `"\{no interpolado\}"`
- **THEN** se parsea como texto literal con llaves, sin expresión interpolada

### Requirement: Index expression grammar

The parser SHALL recognize `expr '[' expr ']'` as a postfix index expression at the same precedence tier as method call and field access, left-associative and chainable, valid both as an ordinary read expression and, when the receiver type permits mutation, as a simultaneous-assignment or ordinary assignment target. The grammar itself SHALL NOT restrict which receiver types support indexing — that restriction belongs to the checker.

#### Scenario: Index expression parses as a place
- **WHEN** `view[0] = 0x7f;` is parsed
- **THEN** it produces an index expression usable as an assignment destination, the same classification `expr.field` already receives

#### Scenario: Chained index and field access
- **WHEN** `a.field[i][j]` is parsed
- **THEN** it produces a left-associative chain of field access followed by two index expressions

#### Scenario: Indexing an unsupported receiver type is a checker error, not a parse error
- **WHEN** an expression whose type does not support indexing is subscripted
- **THEN** parsing succeeds and the checker rejects the expression, naming the receiver type
