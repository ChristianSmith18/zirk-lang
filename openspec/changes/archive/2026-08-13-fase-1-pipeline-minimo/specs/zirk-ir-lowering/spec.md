## ADDED Requirements

### Requirement: IR tipada

La representación intermedia SHALL ser tipada: toda operación y todo valor conocen su tipo, según `ZIRK_COMPILER_SPEC.md` sección 4.

#### Scenario: Tipo de todo valor
- **WHEN** se inspecciona cualquier valor de la IR
- **THEN** expone su tipo

#### Scenario: Operación con operandos incompatibles
- **WHEN** se construye una operación cuyos operandos no coinciden con su firma
- **THEN** la construcción falla y NO produce IR inválida

### Requirement: Forma de bloques básicos

La IR SHALL representar el cuerpo de cada función como un grafo de bloques básicos, cada uno terminado por exactamente una instrucción de terminación.

Es lo que permite el análisis de flujo que `ZIRK_COMPILER_SPEC.md` sección 4 exige, y lo que evita rehacer la representación cuando lleguen los bucles en Fase 2.

#### Scenario: Bloque con terminador único
- **WHEN** se inspecciona cualquier bloque básico
- **THEN** termina en salto, salto condicional o retorno
- **AND** no contiene instrucciones de terminación en posición intermedia

#### Scenario: Condicional
- **WHEN** se baja una sentencia `if` con ambas ramas
- **THEN** se producen bloques para la condición, cada rama y la continuación
- **AND** el bloque de la condición termina en salto condicional

### Requirement: Independencia del modelo de memoria

La IR NO SHALL expresar operaciones ligadas a una estrategia de memoria concreta. La alocación se expresa de forma abstracta y la resuelve el runtime.

Es requisito directo de `docs/decisions/ADR-003-memoria.md`: la estrategia se decide en Fase 4 y la IR no debe adelantarla.

#### Scenario: Operación de alocación
- **WHEN** la IR necesita expresar que un valor se aloca
- **THEN** usa una operación abstracta que nombra el tipo
- **AND** NO nombra `malloc`, recuento de referencias ni recolección de basura

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
