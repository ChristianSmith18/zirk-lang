## MODIFIED Requirements

### Requirement: Construcciones fuera del subset

El parser SHALL emitir un diagnóstico específico ante construcciones que existen en el lenguaje pero no están implementadas todavía, distinguiéndolas de errores de sintaxis.

Esta fase retira de esa lista `class`, `construct`, `this`, `record`, `type`, `public`, `private`, `protected`, `abstract`, `implements`, `extends`, `from`, `as` e `is`.

#### Scenario: Construcción de fase posterior
- **WHEN** se parsea `try`, `task`, `parallel` o `thread`
- **THEN** el diagnóstico SHALL nombrar la construcción
- **AND** SHALL indicar que no está implementada todavía
- **AND** NO SHALL reportarse como token inesperado

#### Scenario: `init.zrk` fuera de alcance
- **WHEN** se encuentra un archivo `init.zrk`
- **THEN** el diagnóstico indica que la configuración declarativa de proyecto llega en una fase posterior

### Requirement: Nulabilidad

El parser SHALL reconocer `T?` como anotación de tipo, `null` como literal, `??` como coalescencia nula y `?.` como acceso seguro, según `ZIRK_LANGUAGE_SPEC.md` sección 4.

`?.` deja de diferirse: la fase anterior lo pospuso porque ningún tipo tenía miembros, y las clases los traen.

#### Scenario: Tipo nulable
- **WHEN** se parsea `mut name: String? = null;`
- **THEN** se produce una declaración con tipo `String?` e inicializador nulo

#### Scenario: Coalescencia nula
- **WHEN** se parsea `name ?? "anónimo"`
- **THEN** se produce una expresión con fallback

#### Scenario: Precedencia de `??`
- **WHEN** se parsea `a ?? b || c`
- **THEN** el árbol representa `(a ?? b) || c`

#### Scenario: Acceso seguro
- **WHEN** se parsea `usuario?.nombre`
- **THEN** se produce una expresión de acceso seguro sobre el miembro

## ADDED Requirements

### Requirement: Sintaxis de clases

El parser SHALL reconocer `class`, sus campos y métodos, `construct`, `this`, los modificadores de visibilidad, `abstract`, `extends` e `implements`, según `ZIRK_LANGUAGE_SPEC.md` sección 7.

#### Scenario: Clase completa
- **WHEN** se parsea `class User implements Serializable { public inmut id: Int32; construct(id: Int32) { this.id = id; } }`
- **THEN** se produce una declaración con un contrato implementado, un campo y un constructor

#### Scenario: Herencia y contratos combinados
- **WHEN** se parsea `class Admin extends User implements Auditable, Cloneable { }`
- **THEN** se produce una declaración con una superclase y dos contratos

#### Scenario: `construct` fuera de una clase
- **WHEN** `construct` aparece en el nivel superior del archivo
- **THEN** se emite un diagnóstico indicando que un constructor pertenece a una clase

### Requirement: Sintaxis de contratos

El parser SHALL reconocer `interface` y `trait` con sus métodos, y admitir cuerpo únicamente en los de un `trait`.

#### Scenario: Interfaz
- **WHEN** se parsea `interface Serializable { fn serialize(): String; }`
- **THEN** se produce una declaración con una firma sin cuerpo

#### Scenario: Trait con implementación
- **WHEN** se parsea `trait Greet { fn hello(): String { return "hola"; } }`
- **THEN** se produce una declaración cuyo método tiene cuerpo

### Requirement: Sintaxis de genéricos

El parser SHALL reconocer parámetros de tipo `<T>` en declaraciones, argumentos de tipo en los usos, y restricciones con `from`.

#### Scenario: Función genérica con restricción
- **WHEN** se parsea `fn serialize<T from Serializable>(value: T): String { }`
- **THEN** se produce un parámetro de tipo con una restricción

#### Scenario: Varios parámetros de tipo
- **WHEN** se parsea `class Map<K, V> { }`
- **THEN** se producen dos parámetros de tipo

#### Scenario: Argumento de tipo en el uso
- **WHEN** se parsea `mut b: Box<Int32>;`
- **THEN** el tipo lleva un argumento

#### Scenario: `<` que no abre genéricos
- **WHEN** se parsea `a < b`
- **THEN** se produce una comparación, no un argumento de tipo

### Requirement: Sintaxis de tipos de datos

El parser SHALL reconocer `record`, value classes, variantes de enum con datos asociados, uniones `A | B` y alias con `type`.

#### Scenario: Enum con datos asociados
- **WHEN** se parsea `enum Shape { Circle(Int32), Rect(Int32, Int32) }`
- **THEN** se producen dos variantes con uno y dos tipos asociados

#### Scenario: Patrón con destructuring
- **WHEN** se parsea `match s { Shape.Circle(r) => r, _ => 0 }`
- **THEN** el patrón liga un nombre al valor asociado

#### Scenario: Unión
- **WHEN** se parsea `mut x: Int32 | String;`
- **THEN** se produce un tipo con dos miembros

#### Scenario: Alias
- **WHEN** se parsea `type Id = Int32;`
- **THEN** se produce una declaración de alias

### Requirement: No hay sintaxis de tipo función

El parser NO SHALL reconocer una sintaxis de tipo función como `(Int32, Int32) => Int32` en posición de tipo.

Es la contraparte sintáctica de D9: mientras un tipo función no se pueda escribir, una closure no puede anotarse y por tanto no puede escapar. La representación actual —capturas inline, un tipo por lambda— es consecuencia de eso y **no prejuzga** cómo se escribirán ni cómo se compararán los tipos función cuando existan.

#### Scenario: Tipo función en una anotación
- **WHEN** se parsea una anotación de tipo con la forma de una firma de función
- **THEN** se emite un diagnóstico indicando que los tipos función llegan en una fase posterior

#### Scenario: El lambda como expresión no cambia
- **WHEN** se parsea `(a: Int32): Int32 => a + 1` en posición de valor
- **THEN** se produce un lambda, igual que en la fase anterior

### Requirement: Sintaxis de casts

El parser SHALL reconocer la forma postfija `expr as T` y la prefija `<T>expr`, según `ZIRK_LANGUAGE_SPEC.md` sección 11.

#### Scenario: Cast postfijo
- **WHEN** se parsea `mut v = source as String;`
- **THEN** se produce una conversión al tipo nombrado

#### Scenario: Cast prefijo
- **WHEN** se parsea `mut v = <String>source;`
- **THEN** se produce la misma conversión que la forma postfija

#### Scenario: Cast que reinterpreta memoria
- **WHEN** se parsea un cast que exige `unsafe`
- **THEN** se emite un diagnóstico indicando que el nivel bajo llega en una fase posterior
