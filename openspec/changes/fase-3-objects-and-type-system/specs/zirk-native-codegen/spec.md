## ADDED Requirements

### Requirement: Layout de objetos

El backend SHALL traducir un tipo con identidad a una estructura cuya cabecera precede a sus campos, y cuyos campos heredados preceden a los propios.

#### Scenario: Prefijo compartido
- **WHEN** se traducen una clase base y una subclase
- **THEN** el prefijo de la estructura de la subclase coincide con la de la base

#### Scenario: Value class inline
- **WHEN** una value class es campo de otra declaración
- **THEN** se traduce sin puntero intermedio

### Requirement: Tablas de métodos

El backend SHALL emitir una tabla de métodos por tipo con métodos virtuales, y una por interfaz implementada.

#### Scenario: Índice estable al heredar
- **WHEN** una subclase hereda un método virtual
- **THEN** ocupa el mismo índice que en la tabla de su base

#### Scenario: Despacho por interfaz
- **WHEN** se llama a un método a través de una interfaz
- **THEN** el código generado busca la tabla de esa interfaz en el descriptor y despacha por ella

### Requirement: Cast comprobado

El backend SHALL traducir un cast comprobable a una comparación del descriptor de tipo del valor contra el esperado, transfiriendo al runtime cuando no coincide.

#### Scenario: Cast que falla
- **WHEN** el descriptor no corresponde al tipo pedido
- **THEN** el programa termina con un error de runtime diagnosticado
- **AND** NO incurre en comportamiento indefinido

#### Scenario: Cast que no necesita comprobación
- **WHEN** el cast asciende en la jerarquía, donde el chequeador ya lo probó
- **THEN** el código generado no incluye comparación alguna
