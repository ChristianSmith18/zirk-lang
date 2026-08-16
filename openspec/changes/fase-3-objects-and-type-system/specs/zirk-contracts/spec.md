## ADDED Requirements

### Requirement: Interfaces

Una `interface` SHALL declarar firmas sin implementación, y una clase SHALL poder combinar varias, según `ZIRK_LANGUAGE_SPEC.md` sección 7.

#### Scenario: Clase que implementa una interfaz
- **WHEN** `class User implements Serializable` define todos los métodos de la interfaz
- **THEN** el chequeo tiene éxito
- **AND** `User` es aceptable donde se espera `Serializable`

#### Scenario: Método de la interfaz sin implementar
- **WHEN** una clase declara implementar una interfaz y omite un método
- **THEN** se emite un diagnóstico que nombra el método faltante y su firma

#### Scenario: Interfaz con cuerpo
- **WHEN** un método de una `interface` declara un cuerpo
- **THEN** se emite un diagnóstico indicando que la implementación reutilizable corresponde a un `trait`

### Requirement: Traits con implementación

Un `trait` SHALL poder incluir métodos con cuerpo, que la clase que lo adopta recibe salvo que los redefina.

#### Scenario: Método heredado del trait
- **WHEN** una clase adopta un trait con un método implementado y no lo redefine
- **THEN** llamar a ese método sobre la clase ejecuta el cuerpo del trait

#### Scenario: La clase redefine el método del trait
- **WHEN** la clase define su propia versión
- **THEN** se ejecuta la de la clase

#### Scenario: Dos traits aportan el mismo método
- **WHEN** una clase adopta dos traits que implementan un método con el mismo nombre y no lo redefine
- **THEN** se emite un diagnóstico que nombra ambos traits
- **AND** la ayuda indica que redefinirlo en la clase resuelve el conflicto

### Requirement: Contratos de operador

La sobrecarga de un operador SHALL ocurrir únicamente implementando el contrato que el lenguaje define para él, y NO SHALL alterar su precedencia ni su aridad, según `ZIRK_LANGUAGE_SPEC.md` sección 4.

Los contratos usan métodos reservados como `_add` y `_subtract`. Los tipos definidos por el usuario MAY implementarlos en código seguro; los tipos nativos NO SHALL poder reabrirse desde código de aplicación.

`String` SHALL implement native concatenation and checked repetition contracts:
`String * Integer` and `Integer * String` return a new String, reject negative
counts, and diagnose unrepresentable allocation sizes.

#### Scenario: Concatenación de cadenas
- **WHEN** se evalúa `"a" + "b"`
- **THEN** el resultado es `"ab"`
- **AND** resuelve por el contrato que `String` implementa, no por un caso especial del operador

#### Scenario: Operador sobre un tipo que no lo implementa
- **WHEN** se aplica `+` a un tipo que no implementa el contrato
- **THEN** se emite un diagnóstico que nombra el contrato que falta

#### Scenario: Tipo propio que implementa el contrato
- **WHEN** una clase implementa el contrato de suma y se escriben dos de sus instancias con `+`
- **THEN** se invoca su implementación

### Requirement: Iteración por contrato

`for ... in` SHALL exigir que la expresión iterada implemente `Iterable<T>`, y el elemento SHALL tener el tipo `T` que el contrato declara.

Esto retira el protocolo cerrado de la fase anterior: los rangos y `String` pasan a implementar el contrato en vez de ser casos que el compilador reconoce.

#### Scenario: Iterar un tipo propio
- **WHEN** una clase implementa `Iterable<Int32>` y se escribe `for x in instancia { }`
- **THEN** `x` tiene tipo `Int32`
- **AND** el bucle recorre lo que el iterador produce

#### Scenario: Los rangos siguen funcionando
- **WHEN** se escribe `for i in 0..10 { }`
- **THEN** compila y recorre igual que antes
- **AND** lo hace porque el rango implementa `Iterable<Int32>`

#### Scenario: Tipo que no implementa el contrato
- **WHEN** se itera un tipo que no implementa `Iterable<T>`
- **THEN** se emite un diagnóstico que nombra el contrato que falta

### Requirement: Un contrato no se satisface a medias

Una declaración que dice implementar un contrato SHALL implementarlo por completo antes de ser aceptada.

#### Scenario: Implementación parcial
- **WHEN** una clase implementa dos de los tres métodos de un contrato
- **THEN** se emite un diagnóstico por cada método faltante
