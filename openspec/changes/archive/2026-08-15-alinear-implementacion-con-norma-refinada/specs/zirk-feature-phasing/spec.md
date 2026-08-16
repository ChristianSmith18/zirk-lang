## ADDED Requirements

### Requirement: Toda característica documentada tiene una fase dueña

Cada característica definida por las fuentes normativas SHALL tener asignada
exactamente una fase de construcción en `docs/init/ZIRK_ROADMAP.md`.

Una característica sin fase asignada no es una característica diferida: es una
característica que nadie construirá. Asignarla es lo que impide que se cuele en
la fase activa y la desborde, o que quede como deuda sin fecha.

#### Scenario: Característica normativa sin fase
- **WHEN** una fuente normativa define una característica que ninguna fase del roadmap nombra
- **THEN** el roadmap se corrige asignándole una fase antes de implementar cualquier parte de ella

#### Scenario: Características huérfanas del refinamiento normativo
- **WHEN** se consultan la familia `Float`, `Char` grafémico, la conversión contextual profunda, los operadores bit a bit y de desplazamiento, y la interpolación de cadenas
- **THEN** el roadmap las asigna a la Fase 3b
- **AND** asigna `inmut::strict` a la Fase 4 y la familia temporal a la Fase 7

### Requirement: Diagnóstico de fase para lo no implementado

El compilador SHALL emitir un diagnóstico que nombre la construcción y su fase
de llegada ante cualquier construcción, palabra clave, operador, literal o tipo
que pertenezca al lenguaje pero no a la fase implementada.

El compilador NO SHALL producir un error de sintaxis genérico, un error de
"carácter no reconocido", ni una interpretación silenciosa distinta de la
construcción escrita.

#### Scenario: Construcción de una fase posterior
- **WHEN** el source contiene una construcción del lenguaje que la fase actual no implementa
- **THEN** el diagnóstico la nombra e indica la fase en que llega

#### Scenario: Ausencia de interpretación silenciosa
- **WHEN** el source contiene un literal o un operador del lenguaje que la fase actual no implementa
- **THEN** el compilador lo reconoce como tal y lo difiere con su fase
- **AND** NO lo reinterpreta como una secuencia distinta de tokens

### Requirement: La tabla de tipos pendientes refleja el lenguaje vigente

La tabla de tipos conocidos-pero-no-implementados SHALL contener únicamente
tipos que el lenguaje define hoy, cada uno con la fase que realmente lo trae.

Un tipo retirado del lenguaje SHALL desaparecer de la tabla: anunciar su llegada
enseña un lenguaje que no existe.

#### Scenario: Tipo retirado del lenguaje
- **WHEN** una anotación nombra `Decimal64` u otro miembro de la antigua familia `Decimal`
- **THEN** el diagnóstico lo trata como tipo inexistente
- **AND** NO anuncia ninguna fase de llegada

#### Scenario: Fase declarada correcta
- **WHEN** una anotación nombra un tipo pendiente
- **THEN** la fase indicada por el diagnóstico coincide con la que el roadmap le asigna
