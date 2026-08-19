# zirk-classes Specification

## Purpose
TBD - created by archiving change document-refined-core-language-semantics. Update Purpose after archive.
## Requirements
### Requirement: Attributes, defaults, and projection behavior
Classes SHALL expose attributes and ordinary methods, SHALL NOT expose a property declaration, and SHALL use conventional `get_`/`set_` methods when accessors are desired. Omitted attributes SHALL receive their type default. Reading a reference-valued attribute SHALL deep-clone an independent value, while a whole class variable SHALL share identity and an attribute place SHALL mutate original storage.

#### Scenario: Attribute getter is ordinary method
- **WHEN** a class declares `fn get_name(): String` and a caller reads the name
- **THEN** the caller invokes `user.get_name()` and no `user.name` property dispatch is synthesized

#### Scenario: Nested reference projection
- **WHEN** `mut name = user.name` is evaluated and `name` is mutated
- **THEN** `user.name` remains unchanged because extraction deep-cloned the String

### Requirement: Concrete inheritance and abstract implementation
A concrete class MAY extend at most one concrete class. An `abstract class` SHALL be a nominal set of required attributes and `abstract fn` signatures with no constructor, body, state allocation, or layout contribution, and concrete classes SHALL adopt it through `implements`. Interfaces and traits SHALL also use `implements`.

#### Scenario: Abstract class contract
- **WHEN** `class Circle implements Shape` supplies every attribute and `override fn` required by abstract class `Shape`
- **THEN** Circle is accepted where Shape is required without inheriting Shape storage

### Requirement: Explicit overriding and super dispatch
Only `override fn` SHALL replace a base, abstract, interface, or trait method. Public/protected instance methods SHALL dispatch virtually by default; private/static methods SHALL not. `super(...)` and `super.method()` SHALL address the concrete base, and `TraitName.super.method()` SHALL resolve a trait conflict.

#### Scenario: Missing override marker
- **WHEN** a subclass redeclares a compatible base method without `override fn`
- **THEN** compilation fails with both declarations identified

### Requirement: Object casts and identity
`is` SHALL compare reference identity, `==` SHALL require equality capability, `as` SHALL perform a checked related-type cast with controlled failure, and `as?` SHALL return nullable absence on failure. Unrelated casts SHALL fail at compile time.

#### Scenario: Optional downcast
- **WHEN** a User reference is evaluated with `as? Admin`
- **THEN** the result is the same Admin reference when compatible or `null` otherwise

### Requirement: Declaración de clase

El compilador SHALL admitir `class` con campos y métodos, según `ZIRK_LANGUAGE_SPEC.md` sección 7.

#### Scenario: Clase con campos y constructor
- **WHEN** se declara `class User { public inmut id: Int32; construct(id: Int32) { this.id = id; } }`
- **THEN** se produce un tipo `User` con un campo y un constructor

#### Scenario: Campo sin modificadores
- **WHEN** una clase declara `name: String;`
- **THEN** el campo equivale a `public mut name: String;`

#### Scenario: Múltiples constructores
- **WHEN** una clase declara dos `construct` con firmas efectivas distintas
- **THEN** ambos son constructores válidos y quedan disponibles para resolución

#### Scenario: Instanciación sin `new`
- **WHEN** se escribe `mut u = User(1);`
- **THEN** se construye una instancia
- **AND** escribir `new User(1)` emite un diagnóstico indicando que `new` no existe

#### Scenario: Campo sin tipo
- **WHEN** un campo se declara sin anotación de tipo
- **THEN** se emite un diagnóstico que señala el campo

### Requirement: `this` designa la instancia actual

Dentro de un método o constructor, `this` SHALL referirse a la instancia sobre la que se invoca.

#### Scenario: Acceso a un campo por `this`
- **WHEN** un constructor escribe `this.id = id;`
- **THEN** asigna al campo, no al parámetro

#### Scenario: `this` fuera de una clase
- **WHEN** `this` aparece en una función de nivel superior
- **THEN** se emite un diagnóstico indicando que no hay instancia

### Requirement: Visibilidad de miembros

Un miembro SHALL admitir `public`, `private` o `protected`, con `public` por defecto, según `ZIRK_LANGUAGE_SPEC.md` sección 7.

#### Scenario: Miembro privado desde fuera
- **WHEN** se accede a un campo `private` desde fuera de su clase
- **THEN** se emite un diagnóstico que nombra el miembro y su visibilidad

#### Scenario: Miembro protegido desde una subclase
- **WHEN** una subclase accede a un miembro `protected` de su superclase
- **THEN** el acceso es válido

#### Scenario: Visibilidad por defecto
- **WHEN** un miembro se declara sin modificador
- **THEN** es `public`

### Requirement: Herencia simple

Una clase SHALL extender a lo sumo una clase. Las clases SHALL ser heredables por defecto, y `final` NO SHALL existir.

#### Scenario: Subclase hereda campos y métodos
- **WHEN** `class Admin extends User` no declara `id`
- **THEN** una instancia de `Admin` tiene el campo `id` de `User`

#### Scenario: Herencia múltiple de clases
- **WHEN** una clase intenta extender dos clases
- **THEN** se emite un diagnóstico indicando que la herencia de clases es simple

#### Scenario: Ciclo de herencia
- **WHEN** dos clases se extienden mutuamente
- **THEN** se emite un diagnóstico que muestra el ciclo

### Requirement: Redefinición de métodos

Una subclase SHALL poder redefinir un método de su superclase con la misma firma, y la llamada SHALL resolver a la definición del tipo en tiempo de ejecución.

#### Scenario: Despacho a la redefinición
- **WHEN** una variable declarada del tipo base sostiene una instancia de la subclase y se llama al método redefinido
- **THEN** se ejecuta el de la subclase

#### Scenario: Redefinición con firma distinta
- **WHEN** una subclase redefine un método cambiando parámetros o retorno
- **THEN** se emite un diagnóstico que muestra ambas firmas

### Requirement: Clases y métodos abstractos

`abstract` SHALL admitirse en clases y métodos. Una clase abstracta NO SHALL instanciarse, y una clase concreta SHALL implementar todo método abstracto que herede.

#### Scenario: Instanciar una clase abstracta
- **WHEN** se construye una instancia de una clase `abstract`
- **THEN** se emite un diagnóstico indicando que no es instanciable

#### Scenario: Método abstracto sin implementar
- **WHEN** una clase concreta hereda un método `abstract` y no lo implementa
- **THEN** se emite un diagnóstico que nombra el método

#### Scenario: Método abstracto con cuerpo
- **WHEN** un método `abstract` declara un cuerpo
- **THEN** se emite un diagnóstico que señala el cuerpo

