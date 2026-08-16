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

El parser SHALL reconocer `if` y `else` como sentencia, con cuerpos siempre entre llaves. Cuando ambas ramas están presentes, `if`/`else` SHALL admitirse también como expresión, según `ZIRK_LANGUAGE_SPEC.md` sección 5.

Que sea sentencia o expresión no lo decide el parser: lo decide el chequeo de tipos, según si el uso exige un valor y ambas ramas son type-compatible.

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
- **WHEN** se parsea `if x > 0 return;`
- **THEN** se emite un diagnóstico indicando que el cuerpo debe ir entre llaves

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

#### Scenario: Construcción de fase posterior
- **WHEN** se parsea `class`, `try`, `task`, `parallel` o `thread`
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

El parser SHALL reconocer `for` con inicialización/condición/incremento, `for ... in` sobre una expresión iterable, `while` y `loop`, junto con `break` y `continue`, según `ZIRK_LANGUAGE_SPEC.md` sección 5.

#### Scenario: `for` con las tres cláusulas
- **WHEN** se parsea `for (mut i = 0; i < 10; i++) { }`
- **THEN** se produce un bucle con inicialización, condición e incremento

#### Scenario: `for ... in`
- **WHEN** se parsea `for x in 0..10 { }`
- **THEN** se produce un bucle que itera la variable `x` sobre el rango

#### Scenario: `while`
- **WHEN** se parsea `while x > 0 { }`
- **THEN** se produce un bucle condicional

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

El parser SHALL reconocer `T?` como anotación de tipo, `null` como literal y `??` como operador de coalescencia nula, según `ZIRK_LANGUAGE_SPEC.md` sección 4.

`?.` se reconoce pero se rechaza con el diagnóstico de fase (decisión D8): requiere miembros que acceder, y ningún tipo de esta fase los tiene.

#### Scenario: Tipo nulable
- **WHEN** se parsea `mut nombre: String? = null;`
- **THEN** se produce una declaración con tipo `String?` e inicializador nulo

#### Scenario: Coalescencia nula
- **WHEN** se parsea `nombre ?? "anónimo"`
- **THEN** se produce una expresión con fallback

#### Scenario: Precedencia de `??`
- **WHEN** se parsea `a ?? b || c`
- **THEN** el árbol representa `(a ?? b) || c`, porque `??` liga más fuerte que los operadores lógicos

#### Scenario: `?.` difiere a Fase 3
- **WHEN** se parsea `usuario?.nombre`
- **THEN** se emite el diagnóstico de construcción no implementada indicando Fase 3

### Requirement: Asignación compuesta e incremento

El parser SHALL reconocer `+=`, `-=`, `*=`, `/=`, `%=`, `**=`, `++` y `--` en posición de **sentencia**, expandiéndolos al árbol de la asignación equivalente, según la decisión D9 del design.

#### Scenario: Asignación compuesta
- **WHEN** se parsea `total += 5;`
- **THEN** el árbol producido es equivalente al de `total = total + 5;`

#### Scenario: Incremento
- **WHEN** se parsea `i++;` o `++i;`
- **THEN** el árbol producido es equivalente al de `i = i + 1;`

#### Scenario: Incremento en posición de expresión
- **WHEN** se parsea `mut x = i++;`
- **THEN** se emite un diagnóstico indicando que el incremento solo se admite como sentencia en esta fase

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

### Requirement: Complete conditional and loop forms
The parser SHALL accept value-producing `if`, SHALL accept a single immediate statement without braces, SHALL accept `for mut i = 0; i < 10; i++`, and SHALL accept `do { ... } while condition;`. A ternary SHALL remain the preferred compact value form but SHALL NOT replace the `if` expression.

#### Scenario: Single-statement conditional
- **WHEN** source contains `if closed return;`
- **THEN** the return is the sole conditional statement

#### Scenario: Traditional for
- **WHEN** source contains `for mut i = 0; i < 10; i++ { work(i); }`
- **THEN** the parser records initializer, condition, update, and body

#### Scenario: Do while
- **WHEN** source contains `do { poll(); } while pending;`
- **THEN** the parser records a post-condition loop whose body runs before its condition

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
