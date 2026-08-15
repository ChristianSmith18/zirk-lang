## ADDED Requirements

### Requirement: Declaración de clase

El compilador SHALL admitir `class` con campos y métodos, según `ZIRK_LANGUAGE_SPEC.md` sección 7.

#### Scenario: Clase con campos y constructor
- **WHEN** se declara `class User { public inmut id: Int32; construct(id: Int32) { this.id = id; } }`
- **THEN** se produce un tipo `User` con un campo y un constructor

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
