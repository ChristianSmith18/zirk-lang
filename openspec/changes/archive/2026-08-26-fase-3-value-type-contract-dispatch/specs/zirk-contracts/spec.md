## MODIFIED Requirements

### Requirement: Interfaces

Una `interface` SHALL declarar firmas sin implementación, y una clase SHALL poder combinar varias, según `ZIRK_LANGUAGE_SPEC.md` sección 7. Un `record` que implementa una interfaz SHALL despachar correctamente a través de una referencia tipada por esa interfaz, sin exponer identidad observable ni mutación a través de esa referencia.

#### Scenario: Clase que implementa una interfaz
- **WHEN** `class User implements Serializable` define todos los métodos de la interfaz
- **THEN** el chequeo tiene éxito
- **AND** `User` es aceptable donde se espera `Serializable`

#### Scenario: Método de la interfaz sin implementar
- **WHEN** una clase declara implementar una interfaz y omite un método
- **THEN** se emite un diagnóstico que nombra el método faltante y su firma

#### Scenario: Record dispatches through an implemented interface
- **WHEN** a `record` implements an interface and is held through a variable typed as that interface
- **THEN** a call through the interface-typed reference dispatches to the record's own method, without granting the value observable identity or mutability through that reference

NOTE (not applied to the main spec as a `value class` scenario): `value class`'s own compact declaration grammar has no `implements` clause and no method-body syntax at all today, independent of this change — so a `value class` cannot yet satisfy a contract regardless of this dispatch mechanism now working for `record`. Extending `value class`'s grammar is separate, future work.
