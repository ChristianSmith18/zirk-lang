## ADDED Requirements

### Requirement: La IR aloca por una operación abstracta

La IR SHALL expresar la creación de un objeto sin nombrar estrategia de memoria, y el runtime SHALL materializarla tras la frontera ABI C.

Es la aplicación directa de `docs/decisions/ADR-003-memoria.md` y de `docs/decisions/ADR-002-runtime-staticlib.md`: la estrategia se elige en la Fase 4, y el runtime es el punto único donde se materializa.

#### Scenario: Construcción de un objeto
- **WHEN** se baja la construcción de una instancia
- **THEN** la IR usa la operación abstracta de alocación nombrando el tipo
- **AND** NO nombra `malloc`, recuento de referencias ni recolección de basura

#### Scenario: El runtime expone la alocación sin mangling
- **WHEN** se inspeccionan los símbolos de la biblioteca estática
- **THEN** la función de alocación aparece con su nombre `extern "C"`

### Requirement: La memoria no se libera en esta fase

El runtime NO SHALL liberar la memoria de los objetos que aloca.

No es un descuido: liberar exige haber decidido **cuándo**, y esa es exactamente la pregunta que ADR-003 deja abierta hasta la Fase 4. Construir una liberación parcial ahora sería trabajo que hay que deshacer, con la trampa de que un recuento de referencias a medias parece funcionar hasta el primer ciclo.

#### Scenario: Un programa termina sin liberar
- **WHEN** un programa construye objetos y termina
- **THEN** el proceso termina normalmente
- **AND** el sistema operativo recupera la memoria

#### Scenario: La deuda está declarada
- **WHEN** se consulta la documentación del runtime
- **THEN** indica que la estrategia de memoria llega en la Fase 4

### Requirement: Cabecera de objeto

Todo objeto con identidad SHALL llevar una cabecera que identifique su tipo en tiempo de ejecución, antes de sus campos.

La necesitan el despacho dinámico, los casts comprobables y la identidad básica de tipo que `ZIRK_LANGUAGE_SPEC.md` sección 12 garantiza siempre.

#### Scenario: Identidad de tipo disponible
- **WHEN** se inspecciona un objeto en tiempo de ejecución
- **THEN** su cabecera identifica su tipo

#### Scenario: La cabecera precede a los campos
- **WHEN** se calcula el desplazamiento de un campo
- **THEN** se cuenta desde después de la cabecera

### Requirement: Los campos heredados preceden a los propios

El layout de una clase SHALL colocar los campos heredados antes que los suyos, en el orden en que la jerarquía los declara.

Así el prefijo del layout de una subclase coincide con el de su superclase, y el desplazamiento de un campo heredado no depende del tipo desde el que se mire.

#### Scenario: Mismo desplazamiento en base y subclase
- **WHEN** una subclase añade campos a los de su base
- **THEN** un campo de la base ocupa el mismo desplazamiento en ambas

#### Scenario: Acceso a través del tipo base
- **WHEN** una variable del tipo base sostiene una instancia de la subclase y se lee un campo heredado
- **THEN** se lee el valor correcto sin comprobación en tiempo de ejecución
