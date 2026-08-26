## MODIFIED Requirements

### Requirement: Interfaces

Una `interface` SHALL declarar firmas sin implementación, y una clase SHALL poder combinar varias, según `ZIRK_LANGUAGE_SPEC.md` sección 7. Un `record` o `value class` que implementa una interfaz SHALL despachar correctamente a través de una referencia tipada por esa interfaz, sin exponer identidad observable ni mutación a través de esa referencia.

#### Scenario: Clase que implementa una interfaz
- **WHEN** `class User implements Serializable` define todos los métodos de la interfaz
- **THEN** el chequeo tiene éxito
- **AND** `User` es aceptable donde se espera `Serializable`

#### Scenario: Método de la interfaz sin implementar
- **WHEN** una clase declara implementar una interfaz y omite un método
- **THEN** se emite un diagnóstico que nombra el método faltante y su firma

#### Scenario: Value class dispatches through an implemented interface
- **WHEN** a `value class` implements an interface and is held through a variable typed as that interface
- **THEN** a call through the interface-typed reference dispatches to the value class's own method, without granting the value observable identity or mutability through that reference
