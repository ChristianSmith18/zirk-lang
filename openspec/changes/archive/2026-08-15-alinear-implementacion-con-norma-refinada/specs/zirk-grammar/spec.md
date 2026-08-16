## MODIFIED Requirements

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

## ADDED Requirements

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

## REMOVED Requirements

### Requirement: Complete conditional and loop forms

**Reason**: Enunciaba juntas cuatro formas —`if` como valor, `if` sin llaves, el `for` tradicional y `do ... while`— que pertenecen a requisitos distintos, y lo hacía sin las reglas que este cambio necesitó fijar: qué cierra el header sin paréntesis, y por qué la forma sin llaves no admite `else`.

**Migration**: El `for` tradicional y `do ... while` pasan a *Bucles*; el `if` como valor y el `if` sin llaves, a *Sentencia condicional*, que además fija el rechazo de `else` sobre la forma sin llaves; la preferencia por el ternario, a *Expresión ternaria*; y los paréntesis opcionales, que ninguna fuente recogía, a *Paréntesis opcionales en headers de control*.
