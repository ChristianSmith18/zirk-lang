## MODIFIED Requirements

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
