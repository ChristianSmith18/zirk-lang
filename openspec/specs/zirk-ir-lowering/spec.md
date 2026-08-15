# zirk-ir-lowering

## Purpose

Defines the typed intermediate representation and its generation from the verified tree.

The IR is the boundary that gets distributed inside a `.zpkg`, so its form is a contract. See `docs/decisions/ADR-007-forma-de-la-ir.md`.

## Requirements

### Requirement: IR tipada

La representación intermedia SHALL ser tipada: toda operación y todo valor conocen su tipo, según `ZIRK_COMPILER_SPEC.md` sección 4.

#### Scenario: Tipo de todo valor
- **WHEN** se inspecciona cualquier valor de la IR
- **THEN** expone su tipo

#### Scenario: Operación con operandos incompatibles
- **WHEN** se construye una operación cuyos operandos no coinciden con su firma
- **THEN** la construcción falla y NO produce IR inválida

### Requirement: Forma de bloques básicos

La IR SHALL representar el cuerpo de cada función como un grafo de bloques básicos, cada uno terminado por exactamente una instrucción de terminación. El grafo SHALL admitir ciclos, producidos por el lowering de bucles.

Es lo que permite el análisis de flujo que `ZIRK_COMPILER_SPEC.md` sección 4 exige, y la forma que Fase 1 (ADR-007) ya anticipó pensando en los bucles de esta fase.

#### Scenario: Bloque con terminador único
- **WHEN** se inspecciona cualquier bloque básico
- **THEN** termina en salto, salto condicional o retorno
- **AND** no contiene instrucciones de terminación en posición intermedia

#### Scenario: Condicional
- **WHEN** se baja una sentencia `if` con ambas ramas
- **THEN** se producen bloques para la condición, cada rama y la continuación
- **AND** el bloque de la condición termina en salto condicional

#### Scenario: Bucle
- **WHEN** se baja un `while`, `loop` o `for`
- **THEN** se produce un bloque de condición o cuerpo que salta de vuelta a sí mismo o a un bloque anterior
- **AND** el grafo de bloques resultante contiene un ciclo

### Requirement: Independencia del modelo de memoria

La IR NO SHALL expresar operaciones ligadas a una estrategia de memoria concreta. La alocación se expresa de forma abstracta y la resuelve el runtime. Esto incluye la alocación del entorno de captura de una closure.

Es requisito directo de `docs/decisions/ADR-003-memoria.md`.

#### Scenario: Operación de alocación
- **WHEN** la IR necesita expresar que un valor se aloca
- **THEN** usa una operación abstracta que nombra el tipo
- **AND** NO nombra `malloc`, recuento de referencias ni recolección de basura

#### Scenario: Alocación del entorno de una closure
- **WHEN** se baja una lambda que captura variables del scope envolvente
- **THEN** el entorno de captura se aloca con la misma operación abstracta que cualquier otro tipo
- **AND** la IR no nombra dónde vive ese entorno en memoria

### Requirement: Variables locales como slots

Las variables locales SHALL representarse como slots con operaciones de carga y almacenamiento, sin forma SSA propia.

La promoción a registros se delega al backend. SSA propia no paga hasta que existan optimizaciones propias.

#### Scenario: Lectura y escritura de una local
- **WHEN** se baja el uso de una variable local
- **THEN** se produce una carga desde su slot
- **AND** una asignación produce un almacenamiento en ese slot

### Requirement: Trazabilidad al source

Toda instrucción de la IR SHALL conservar la ubicación del source que la originó.

Sin ella no es posible el mapeo fiel a `.zrk` que `ZIRK_COMPILER_SPEC.md` sección 11 exige del debugger.

#### Scenario: Ubicación de una instrucción
- **WHEN** se inspecciona cualquier instrucción de la IR
- **THEN** expone la ubicación del source correspondiente

### Requirement: Lowering desde el árbol verificado

La generación de IR SHALL partir del árbol ya resuelto y verificado, no del árbol crudo del parser.

#### Scenario: Entrada al lowering
- **WHEN** se genera IR
- **THEN** la entrada tiene los nombres resueltos y los tipos verificados
- **AND** el lowering NO vuelve a chequear tipos

### Requirement: Lowering de bucles y de `break`/`continue`

El lowering SHALL traducir `for`, `for ... in`, `while` y `loop` a bloques básicos con la condición evaluada en su propio bloque, y SHALL traducir `break`/`continue` a un salto directo al bloque de continuación o al bloque de condición del bucle que los contiene.

#### Scenario: `while`
- **WHEN** se baja `while cond { cuerpo }`
- **THEN** se produce un bloque de condición, un bloque de cuerpo y un bloque de continuación
- **AND** el bloque de cuerpo termina saltando de vuelta al bloque de condición

#### Scenario: `break`
- **WHEN** se baja un `break` dentro de un bucle
- **THEN** se produce un salto directo al bloque de continuación de ese bucle

#### Scenario: `continue`
- **WHEN** se baja un `continue` dentro de un `for`
- **THEN** se produce un salto directo al bloque de incremento del `for`, no al de condición

#### Scenario: Bucles anidados
- **WHEN** un `break` está dentro de un bucle interno, anidado en uno externo
- **THEN** el salto producido apunta a la continuación del bucle interno, no del externo

### Requirement: Lowering de `if` como expresión

El lowering SHALL traducir un `if`/`else` en posición de expresión a bloques cuyo bloque de continuación recibe el valor de la rama tomada, sin introducir una forma de instrucción distinta de la que ya usa el `if` como sentencia.

#### Scenario: Valor de la rama seleccionada
- **WHEN** se baja `mut r = if x > 0 { a } else { b };`
- **THEN** el bloque de continuación produce un valor que proviene del bloque de la rama ejecutada

### Requirement: Lowering de closures

El lowering SHALL traducir una lambda a una función independiente más un valor de entorno con los valores capturados copiados en el punto de creación, y SHALL traducir una llamada a una closure como una llamada indirecta que recibe el entorno como argumento implícito.

#### Scenario: Creación de una closure
- **WHEN** se baja `inmut ADD = (a: Int32, b: Int32): Int32 => a + b;` sin capturas
- **THEN** se produce una función independiente y un valor de closure sin entorno o con entorno vacío

#### Scenario: Closure con captura
- **WHEN** una lambda referencia una variable del scope envolvente
- **THEN** el entorno alocado contiene una copia de esa variable en el momento de la creación
- **AND** el cuerpo de la función bajada lee la variable desde el entorno, no desde el slot original

### Requirement: Lowering de `match`

El lowering SHALL traducir un `match` a una secuencia de comparaciones sobre el discriminante del `enum` (o sobre el valor, para literales), cada una con salto condicional a su bloque de brazo, terminando en el bloque del comodín `_` si existe.

#### Scenario: `match` sobre `enum`
- **WHEN** se baja un `match` con un brazo por cada constructor
- **THEN** se produce un bloque de comparación por constructor y un bloque por cada cuerpo de brazo

#### Scenario: `match` como expresión
- **WHEN** se baja un `match` usado como expresión
- **THEN** cada bloque de brazo termina saltando a un bloque de continuación común que recibe el valor de ese brazo

### Requirement: Lowering de coalescencia nula

El lowering SHALL traducir `a ?? b` a una comprobación explícita de nulidad con dos bloques, que produce `a` cuando no es nulo y evalúa `b` en el otro bloque.

#### Scenario: Comprobación con dos bloques
- **WHEN** se baja `a ?? b`
- **THEN** se produce una comprobación de nulidad sobre `a` con un bloque por cada resultado

#### Scenario: Coalescencia nula evalúa el fallback perezosamente
- **WHEN** se baja `a ?? costoso()`
- **THEN** `costoso()` solo se baja dentro del bloque que se ejecuta cuando `a` es nulo
