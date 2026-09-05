# Plantilla de definición del lenguaje de programación Zirk

> **Checkpoint de identidad — 12 de agosto de 2026.** El lenguaje adopta oficialmente el nombre **Zirk**, la extensión de código fuente **`.zrk`**, el comando de CLI **`zirk`**, el lockfile **`zirk.lock`** y el contenedor de paquetes **`.zpkg`**.
>
> El nombre **Zirk** nace de transformar el nombre *Crist*: al invertir su sonido se obtiene *Sirc* y, posteriormente, se cambian sus letras hasta llegar a una escritura propia que conserva esa sonoridad.

> **Checkpoint de tasks, duraciones, entrada y aliases numéricos — 12 de agosto de 2026.** Se definieron cancelación y timeout mediante exceptions recuperables, literales `Duration`, notación científica, separadores numéricos, `ReadResult` para streams, workers por composición y la ausencia inicial de `comptime {}` general.

> **Checkpoint de intrinsics, SIMD y arquitectura del compilador — 12 de agosto de 2026.** La primera versión utilizará intrinsics portables en lugar de assembly textual, SIMD automático y explícito mediante vectores portables, AST interna privada, Syntax API pública y LLVM como backend inicial único.

> **Checkpoint de testing, tooling y compilación — 12 de agosto de 2026.** Se definieron tests unitarios en `.spec.zrk`, E2E en `test/*.e2e.zrk`, CLI nativa, formatter canónico, linter, LSP, debugger, modos de build, optimizaciones e incrementalidad orientados a latencia baja y seguridad estable.

> **Checkpoint de FFI, paquetes, sintaxis, diagnósticos y modelo de valores — 12 de agosto de 2026.** Se definieron el ABI C como frontera nativa, entrypoint mediante `init.zrk` y `main`, gestor de paquetes seguro, operadores adicionales, convenciones, diagnósticos y la separación entre tipos con identidad y tipos con semántica de valor.

> **Checkpoint de reflection, tipos algebraicos, iteración y seguridad — 12 de agosto de 2026.** Se definieron reflection controlada, compile-time reflection mediante `fn dec`, enums algebraicos, aliases, unions, conversiones y casts, lambdas, iteradores, generators, operaciones funcionales, scopes y las garantías de `unsafe`.

> **Checkpoint de decoradores, paquetes, targets y permisos — 12 de agosto de 2026.** Se incorporaron `fn dec`, paquetes con API pública e implementación intermedia portable, `build_targets`, proyectos `application`/`library` y la separación entre `permissions`, `compile_permissions` y `requires`. Los globals quedan reservados a las aplicaciones.

> **Semántica contextual de `match` — 12 de agosto de 2026.** Como sentencia, `match` solo controla el flujo; en cualquier contexto que espere una expresión, todas sus ramas deben producir un valor compatible. No existen `capture` ni `yield`.

> **Checkpoint de revisión autoral del handbook — 15 de agosto de 2026.** Las 37 acotaciones del autor corrigen respuestas anteriores de esta plantilla. Quedan confirmados `**`/`**=`, ranges descendentes y con `step`, bounds interpolados, `if` de una sola sentencia, `do ... while`, regex `re'...'`, alternativas de `match` separadas por coma, parámetros opcionales siempre tipados, variadics iterables, `fn` opcional en lambdas, capturas desambiguadas con `this`, campos `public mut` por defecto, múltiples `construct` por firma, enums tradicionales con valor string nominal o mapping explícito, arrays siempre fijos, slicing `[inicio:fin:salto]`, strings iterables, generators, pipelines de funciones puras, miembros convenientes de la stdlib y cloning de callables ligados. Este checkpoint prevalece sobre respuestas incompatibles más abajo.
>

> **Checkpoint semántico del núcleo — 16 de agosto de 2026.** La fuente
> normativa consolidada es `docs/CORE_LANGUAGE_SEMANTICS.md`. Se aceptan
> `Function(P...) => R` y el alias preferido `Fn(P...) => R`, closures
> escapables, alias solo al mover una referencia completa y copia profunda al
> proyectar atributos/índices/slices/patrones, atributos sin construcción
> `property`, abstract classes adoptadas con `implements`, traits sin estado,
> constraints múltiples, variance explícita, `Box<T>`, tuples por `result[n]`,
> enums sin métodos, match exhaustivo sin guards, slices independientes con
> defaults tipo Python pero bounds explícitos estrictos, e iteración por
> `Iteration<T>`. Las preguntas y respuestas históricas que contradigan este
> checkpoint se conservan únicamente como procedencia, no como diseño vigente.
>

> **Checkpoint de errores, recursos y permisos — 16 de agosto de 2026.** La
> fuente normativa consolidada es `docs/ERROR_RESOURCE_PERMISSION_SEMANTICS.md`.
> `Result` exige manejo, las excepciones explícitas usan `throws`, los errores
> runtime implícitos siguen siendo tipados/capturables, y `catch` usa patrones.
> `match with` conserva errores de cuerpo/cierre, soporta transferencia explícita
> y prohíbe clonar handles. El modelo público de autoridad contiene solo
> `requires` y `permissions`, con `during` por operación; `compile_permissions`
> queda reemplazado. El consentimiento no vive en `init.zrk`: es una aprobación
> firmada fuera del repo, ligada al nombre y ubicación canónica del proyecto y a
> cada dependencia solicitante. Mover/renombrar, ampliar permisos o actualizar
> solicitantes exige nueva aprobación; el estado idéntico usa validación
> incremental. Las respuestas incompatibles posteriores son históricas.
>

> **Checkpoint de memoria y concurrencia — 17 de agosto de 2026.** Las fuentes
> normativas consolidadas son `docs/MEMORY_AND_UNSAFE_SEMANTICS.md` y
> `docs/STRUCTURED_CONCURRENCY_SEMANTICS.md`. La memoria administrada no expone
> ownership/lifetimes públicos; `Weak<T>` y las vistas nativas tienen contratos
> explícitos; el deep clone conserva la topología. Los bloques `unsafe` comunes
> revierten escrituras administradas y rangos validados ante fallos controlados,
> y `commit` delimita efectos irreversibles. Las tasks son tipadas y
> estructuradas, `Task.settled` preserva todos los outcomes, `select` espera de
> forma justa, canales aplican backpressure, y transferencia/compartición se
> derivan internamente para garantizar ausencia de data races en código seguro.
> Toda respuesta histórica incompatible más abajo queda reemplazada.
>

> **Checkpoint de recursos — 12 de agosto de 2026.** Se formalizó `match with` para la adquisición y cierre automático de `Resource<E>`, manteniendo `match` sin `with` para la transferencia manual.
>

> **Revisión terminológica — 11 de agosto de 2026.** Convención unificada: `share` expone código, `import` incorpora código o módulos y `use` habilita variables globales declaradas exclusivamente en `globals` de `init.zrk`.
>

> **Checkpoint fiel — 11 de agosto de 2026.** Se conservaron las preguntas y ejemplos de la plantilla rellenada. Solo se ordenaron y redactaron sus respuestas y se aplicaron las decisiones explícitas del checkpoint; las respuestas vacías permanecen vacías.
>
> Documento base para diseñar de forma sistemática un lenguaje compilado a binarios nativos, con tipado estático, clases reales, concurrencia/paralelismo explícito y posibilidad de múltiples targets.
>
> **Orden de ejemplos de referencia:** TypeScript → Python → C.
>
> La idea es rellenar cada decisión antes de implementar el compilador. Cuando una pregunta tenga varias alternativas, marca la elegida y agrega tus propias reglas.

---

# 0. Identidad y filosofía del lenguaje

## 0.1 Nombre oficial

**Pregunta:** ¿Cómo se llamará el lenguaje?

**Respuesta:**

```text
Nombre: Zirk
Extensión de archivos: .zrk
Comando CLI: zirk
```

**Ejemplo conceptual:**

```bash
zirk build
zirk run
zirk test
```

El nombre **Zirk** es una transformación personal de *Crist*: su sonido se invierte para obtener *Sirc* y luego se modifica su escritura hasta formar una identidad breve, propia y reconocible.

---

## 0.2 Objetivo principal

**Pregunta:** ¿Para qué tipo de software estará diseñado principalmente?

- [ ] Backend
- [ ] Frontend web
- [ ] Full-stack
- [ ] Sistemas
- [ ] CLI
- [ ] Desktop
- [ ] Mobile
- [ ] Embedded
- [ ] Data / IA
- [x] Propósito general
- [ ] Otro:

**Respuesta:**

Será un lenguaje de propósito general. Estará diseñado para aplicaciones backend, CLI, desktop, sistemas y librerías, sin limitar su modelo semántico a una sola clase de software. Otros targets, como web mediante WebAssembly, podrán incorporarse posteriormente sin redefinir el núcleo del lenguaje.

---

## 0.3 Filosofía principal

**Pregunta:** ¿Qué principios debe priorizar el lenguaje?

Ejemplos:

- simplicidad
- rendimiento
- seguridad
- legibilidad
- concurrencia sencilla
- compilación rápida
- predictibilidad
- bajo consumo de memoria
- interoperabilidad
- desarrollo productivo

**Respuesta:**

El lenguaje priorizará la simplicidad, el rendimiento, la seguridad, una concurrencia sencilla, la compilación rápida y un bajo consumo de memoria.

---

## 0.4 Nivel de abstracción

**Pregunta:** ¿Será un lenguaje de alto nivel, medio nivel o permitirá ambos estilos?

**Referencia C:**

```c
int *ptr = &value;
```

C permite control directo de memoria.

**Referencia TypeScript:**

```ts
const user = new User();
```

TypeScript es de alto nivel y no expone memoria directamente.

**Respuesta:**

Será un lenguaje híbrido con dos capas. Por defecto ofrecerá abstracciones de alto nivel, pero permitirá descender a bajo nivel cuando el desarrollador necesite mayor control. Las operaciones que no puedan verificarse de forma segura se delimitarán explícitamente mediante bloques `unsafe { }`; no se utilizará un flag global para cambiar el nivel de seguridad del proyecto.

---

## 1.1 Terminación de sentencias

**Pregunta:** ¿Las sentencias requieren `;`?

**TypeScript:**

```ts
const name: string = "Cristian";
console.log(name);
```

Posibles decisiones:

- [ ] Obligatorio
- [ ] Opcional
- [ ] Prohibido
- [x] Opcional pero formatter lo agrega

**Respuesta:**

El punto y coma será opcional al escribir código, pero el formatter oficial lo agregará de manera consistente. El parser aceptará sentencias con o sin `;` siempre que no exista ambigüedad.

---

## 1.2 Bloques

**Pregunta:** ¿Cómo se delimitan los bloques?

**TypeScript:**

```ts
if (active) {
    console.log("Activo");
}
```

**Python:**

```python
if active:
    print("Activo")
```

Opciones:

- [x] Llaves `{ }`
- [ ] Indentación
- [ ] Palabras `begin/end`
- [ ] Otro:

**Respuesta:**

Los bloques se delimitarán mediante llaves `{ }`. La indentación será una convención aplicada por el formatter, pero no determinará la estructura semántica del programa.

---

## 1.3 Comentarios

**Pregunta:** ¿Cómo se escriben comentarios de una línea y múltiples líneas?

**TypeScript:**

```ts
// Una línea

/*
Varias
líneas
*/
```

**Respuesta propuesta:**

```text
Una línea: //
Multilínea: /* */
Documentación: 
/*
* Acá va la documentación
*/
```

---

## 1.4 Sensibilidad a mayúsculas

**Pregunta:** ¿`User`, `user` y `USER` son identificadores diferentes?

**TypeScript:** sí.

```ts
const user = 1;
const User = 2;
```

**Respuesta:**

Sí. `User`, `user` y `USER` serán identificadores distintos.

---

## 2.1 Variables mutables

Concepto inicial definido:

```text
mut mi_variable: String = "Hello World!";
```

**Pregunta:** ¿Esta será la sintaxis definitiva?

**TypeScript equivalente:**

```ts
let miVariable: string = "Hello World!";
```

**Respuesta:**

Sí. Esta será la sintaxis definitiva para variables mutables.

---

## 2.2 Variables inmutables

Concepto inicial:

```text
inmut name: String = "Cristian";
```

**TypeScript equivalente:**

```ts
const name: string = "Cristian";
```

**Pregunta:** ¿`inmut` significa que la referencia no cambia o que todo el objeto queda profundamente inmutable?

**TypeScript:**

```ts
const user = { name: "Cristian" };

user.name = "Pedro"; // permitido
// user = otro;      // no permitido
```

Opciones:

- [ ] Inmutabilidad de referencia
- [ ] Inmutabilidad profunda
- [x] Dependiente del tipo
- [ ] Otro modelo:

**Respuesta:**

`inmut` hará inmutable la referencia, mientras que `inmut::strict` aplicará inmutabilidad profunda e impedirá modificar el valor.

Tanto `inmut` como `inmut::strict` usarán nombres en mayúsculas como convención. Se elimina el prefijo `S__`; el nombre no tendrá semántica especial para el compilador.

```text
inmut API_URL: String = "https://example.test";
inmut::strict DEFAULT_CONFIG: Config = Config();
```

---

## 2.3 Variables globales

Concepto actualizado:

```text
// init.zrk
globals {
    mut REQUEST_COUNT: Atomic<UInt64> = Atomic(0);
    inmut APP_NAME: String = "App";
}
```

**Pregunta:** ¿Dónde pueden declararse las variables globales?

**TypeScript aproximado:**

```ts
export let requestCount = 0;
```

Pero en tu lenguaje `global` tendría alcance de proyecto completo.

**Respuesta:**

Las variables globales solo podrán declararse dentro del bloque `globals` de `init.zrk` de un proyecto `application`. Los proyectos `library` no podrán declarar globals. Como el bloque ya establece que sus miembros son globales, las declaraciones no repetirán la palabra `global`.

```text
// init.zrk

globals {
    inmut APP_NAME: String = "App";
    mut REQUEST_COUNT: Atomic<UInt64> = Atomic(0);
}
```

Declarar una variable global fuera de ese bloque producirá un error de compilación. Los demás archivos deberán habilitar explícitamente cada global que utilicen mediante `use`:

```text
use APP_NAME;
use REQUEST_COUNT;
```

`use` no importa código ni módulos; únicamente habilita variables globales de `init.zrk`.

---

## 2.4 Inferencia de tipos

**Pregunta:** ¿Se puede omitir el tipo?

**TypeScript:**

```ts
let age = 30;
```

Alternativa explícita:

```ts
let age: number = 30;
```

En tu lenguaje:

```text
mut age = 30;
mut age: Int32 = 30;
```

Opciones:

- [ ] Siempre explícito
- [x] Inferencia permitida
- [ ] Inferencia solo local
- [ ] Inferencia excepto APIs públicas

**Respuesta:**

La inferencia de tipos estará permitida cuando el compilador pueda determinar el tipo sin ambigüedad:

```text
mut age = 30;
```

El tipo podrá declararse explícitamente para documentar el contrato, limitar la inferencia o formar parte de una API pública:

```text
mut age: Int32 = 30;
```

---

## 2.5 Declaración sin inicialización

**Pregunta:** ¿Esto estará permitido?

```text
mut name: String;
```

**TypeScript:**

```ts
let name: string;
```

Preguntas relacionadas:

- ¿Qué valor tiene antes de asignarlo?
- ¿Debe producir error si se lee antes?
- ¿Existe inicialización automática?

**Respuesta:**

Cada tipo tendrá un valor predeterminado. Una variable declarada sin inicializador tomará ese valor. Leer una variable local antes de que esté disponible producirá un error.

Los miembros del bloque `globals` de `init.zrk` se inicializarán durante el arranque del programa y estarán disponibles para los archivos que los habiliten mediante `use`.

---

## 2.6 Desestructuración

**Pregunta:** ¿Existirá?

**TypeScript:**

```ts
const { name, age } = user;
const [first, second] = values;
```

Posible sintaxis:

```text
inmut { name, age } = user;
```

**Respuesta:**

Sí. La desestructuración funcionará como en TypeScript, pero el cambio de nombre utilizará `->` en lugar de `:`.

---

## 3.1 Todo es una clase

Principio deseado:

> Todo valor pertenece a una clase real.

Ejemplos:

```text
String
Boolean
Char
Int8
Int16
Int32
Int64
Int128
UInt8
UInt16
UInt32
UInt64
UInt128
Float16
Float32
Float64
Float128
```

**Pregunta:** ¿Los tipos fundamentales podrán tener métodos?

**TypeScript:**

```ts
"hola".toUpperCase();
(12).toString();
```

En tu lenguaje:

```text
mut age: Int32 = 30;
age.toString();
age.isPositive();
```

**Respuesta:**

Sí. Los tipos fundamentales podrán tener métodos y deberá definirse un valor predeterminado para cada tipo, incluidos los numéricos.

---

## 3.2 Jerarquía de clases base

**Pregunta:** ¿Todas las clases heredan de `Object`?

Posible modelo:

```text
Object
├── Value
│   ├── Numeric
│   │   ├── Integer
│   │   └── Float
│   ├── Boolean
│   ├── Char
│   ├── Temporal
│   ├── Record
│   ├── ValueClass
│   └── Enum
├── Reference
│   ├── String
│   ├── Collection
│   └── Class
└── Special
    ├── Null
    ├── Void
    └── Never
```

**TypeScript equivalente conceptual:**

```ts
class User extends Object {}
```

**Respuesta:**

Sí. `Object` será la raíz conceptual, sin obligar a que todos los valores se
alojen o se empaqueten como objetos. Los tipos obtendrán sus capacidades por
contratos. Se distinguirán valores, referencias y tipos especiales; `Char` es
un valor grapheme mientras `String` es una referencia mutable.

---

## 3.3 Enteros con signo

**Pregunta:** ¿Qué tamaños soporta el lenguaje?

- [x] Int8
- [x] Int16
- [x] Int32
- [x] Int64
- [x] Int128
- [ ] Otros:

**C:**

```c
int8_t a;
int16_t b;
int32_t c;
int64_t d;
```

**Respuesta:**

Existirán enteros con signo de 8, 16, 32, 64 y 128 bits:

```text
Int8
Int16
Int32
Int64
Int128
```

La familia admitirá dos nombres equivalentes sin tamaño explícito:

```text
Int
Integer
```

`Int` e `Integer` serán aliases exactos entre sí y equivaldrán a `Int32`:

```text
Int == Integer == Int32
```

Podrán utilizarse según la preferencia del desarrollador y su significado no dependerá del sistema operativo ni de la arquitectura. Los demás tamaños continuarán disponibles mediante `Int8`, `Int16`, `Int32` e `Int128`.

---

## 3.4 Enteros sin signo

**Pregunta:** ¿Existirán?

**C:**

```c
uint8_t age;
uint64_t counter;
```

Posibles:

```text
UInt8
UInt16
UInt32
UInt64
UInt128
```

**Respuesta:**

Sí. Existirán los mismos tamaños que para los enteros con signo: `UInt8`, `UInt16`, `UInt32`, `UInt64` y `UInt128`.

---

## 3.5 Floats

**Pregunta:** ¿Qué tipos existirán?

**C:**

```c
float a;
double b;
long double c;
```

Posible diseño:

```text
Float16
Float32
Float64
Float128
```

O distinguir:

```text
Float32
Float64
Float128
```

**Respuesta:**

Se utilizará una sola familia binaria llamada `Float`, diferenciada por tamaño:

```text
Float16
Float32
Float64
Float128
```

La forma sin tamaño será:

```text
Float
```

`Float` será alias exacto de `Float64`:

```text
Float == Float64
```

Los demás tamaños continuarán disponibles mediante `Float16`, `Float32` y
`Float128`. `NaN` no será un valor válido: una operación indeterminada dará un
error controlado. Un tipo decimal base diez exacto podrá añadirse después en la
biblioteca estándar para dominios como dinero.

---

## 3.6 Literales numéricos

**Pregunta:** ¿Cómo se indica el tipo del literal?

**C:**

```c
10U
10L
10ULL
3.14f
```

Posible lenguaje:

```text
10i8
10u64
3.14f32
```

O:

```text
Int8(10)
Float64(3.14)
```

**Respuesta:**

Se utilizará el segundo formato, mediante construcción explícita del tipo:

```text
Int8(10)
Float64(3.14)
```

También existirá notación científica decimal:

```text
1e2       // 100
1e3       // 1000
1.5e2     // 150
2.5e-3    // 0.0025
6.022e23
```

`e` representará una potencia de diez. El formatter normalizará `E` a `e`. Un literal con notación científica se inferirá como miembro de la familia `Float`, incluso cuando su valor matemático sea entero.

Los literales numéricos admitirán `_` como separador visual:

```text
1_000
1_000_000
3.141_592
0xFF_FF
0b1010_1100
1.5e1_000
```

El separador no cambiará el valor ni impondrá un tamaño de grupo. No podrá aparecer al principio, al final, repetido consecutivamente ni junto al punto decimal, el marcador de exponente o su signo.

```text
_1000   // Inválido.
1000_   // Inválido.
1__000  // Inválido.
1_.5    // Inválido.
1e_10   // Inválido.
```

El formatter conservará las agrupaciones válidas, porque pueden expresar la intención del desarrollador en números binarios, hexadecimales o dominios específicos.

---

## 3.7 Overflow

**Pregunta:** ¿Qué sucede si un número supera su rango?

**C:**

```c
uint8_t x = 255;
x++;
```

Alternativas:

- [x] Error de runtime
- [ ] Wrap-around
- [ ] Saturación
- [ ] Depende del modo compilación
- [ ] Métodos explícitos `wrappingAdd`, `checkedAdd`

**Respuesta:**

El overflow numérico producirá un error de runtime y nunca realizará wrap-around silencioso en operaciones normales.

Los casos que necesiten otro comportamiento deberán solicitarlo explícitamente mediante operaciones comprobadas, con wrap-around o con saturación. Los nombres exactos de esos métodos se definirán en la biblioteca estándar.

---

## 3.8 String

**Pregunta:** ¿`String` es UTF-8, UTF-16 u otra representación?

**TypeScript/JavaScript:** internamente usa una representación basada en UTF-16 desde el punto de vista observable del lenguaje.

```ts
const text = "Hola";
```

Posible lenguaje:

```text
mut text: String = "Hola";
```

Preguntas:

- ¿Mutable o inmutable? 
- ¿Indexable?
- ¿Por bytes, code points o graphemes?
- ¿Strings interpolados?

**TypeScript:**

```ts
const message = `Hola ${name}`;
```

**Respuesta:**

1. `String` utilizará UTF-16, como TypeScript.
2. Será mutable y permitirá reemplazar varios elementos mediante slicing. Por ejemplo, `text[0:2] = "ca";`.
3. Será indexable al estilo de Python mediante `[i]`, `[i:j]` y `[i:j:k]`. Los índices positivos avanzarán de izquierda a derecha y los negativos de derecha a izquierda.
4. La indexación operará por graphemes. También existirán métodos explícitos como `.bytes()`, `.chars()` y `.graphemes()`.
5. Permitirá interpolación con `"Hola {name}"`; las llaves podrán escaparse.

Para acelerar el acceso por graphemes, `String` mantendrá internamente un índice o caché. Su mantenimiento será responsabilidad del runtime y no formará parte de la semántica pública.

`String` será una referencia compartida. `mut` permitirá reasignar y editar;
`inmut` impedirá reasignar pero permitirá editar el contenido; y
`inmut::strict` impedirá ambas cosas y no podrá coexistir con aliases mutables.
`clone()` producirá una copia lógica independiente. La asignación de slices
exigirá igual cantidad de graphemes. `String + String` concatenará y
`"ja" * 3`/`3 * "ja"` producirán `"jajaja"`; conteos negativos o no enteros
serán errores controlados.

---

## 3.9 Char

**Pregunta:** ¿Qué representa un `Char`?

Opciones:

- [ ] Byte
- [ ] Unicode code point
- [x] Grapheme completo
- [ ] Unidad UTF

**C:**

```c
char c = 'A';
```

Posible lenguaje:

```text
mut letter: Char = 'A';
```

**Respuesta:**

`Char` representará un único grapheme. Al igual que `String`, ofrecerá métodos
para acceder a bytes y code points cuando se necesite control.
`ascii_code(): Int32` retornará el código cuando el grapheme sea exactamente un
carácter ASCII y `-1` en cualquier otro caso. Las conversiones de mayúsculas y
minúsculas retornarán `String`, porque Unicode puede producir varios graphemes.

---

## 3.10 Boolean

**Pregunta:** ¿Solo acepta `true` y `false`?

**TypeScript:**

```ts
const active: boolean = true;
```

¿Se permiten coerciones?

```ts
if (1) {}
```

JavaScript permite truthiness, pero TypeScript hereda ese comportamiento de JS.

**Respuesta:**

`Boolean` solo admitirá `true` y `false`. `Boolean?` podrá admitir además `null`. No habrá truthiness numérico: `0` y `1` no serán valores booleanos ni podrán usarse como condición sin una comparación explícita.

---

## 4.1 Existencia de null

**Pregunta:** ¿Existe `null`?

**TypeScript:**

```ts
let user: User | null = null;
```

Alternativa sin null:

```text
mut user: User? = none;
```

Opciones:

- [x] `null`
- [ ] `none`
- [ ] `Option<T>`
- [ ] `T?`
- [ ] No existe valor nulo

**Respuesta:**

Existirá `null` y los tipos que puedan contenerlo se escribirán con `?`, por ejemplo: `mut user: User? = null;`.

---

## 4.2 Acceso seguro

**Pregunta:** ¿Existirá optional chaining?

**TypeScript:**

```ts
user?.address?.city;
```

Posible:

```text
user?.address?.city;
```

**Respuesta:**

Sí. Funcionará como en TypeScript: si algún elemento de la cadena de acceso no está disponible, la expresión retornará `null`.

---

## 4.3 Valor por defecto

**TypeScript:**

```ts
const name = user.name ?? "Sin nombre";
```

Posible:

```text
inmut name = user.name ?? "Sin nombre";
```

**Respuesta:**

Sí. Se utilizará `??` tal como en el ejemplo propuesto.

---

## 5.1 Igualdad

**Pregunta:** ¿Cómo se compara igualdad?

**TypeScript:**

```ts
a === b;
a !== b;
```

Preguntas:

- ¿`==` compara valor?
- ¿`===` existe?
- ¿Comparación de identidad y valor están separadas?

Posible:

```text
a == b       // igualdad de valor
a is b       // misma referencia
```

**Respuesta:**

`==` comparará estructuralmente los objetos, propiedad por propiedad. Dos instancias distintas con el mismo contenido podrán ser iguales.

```text
a == b; // igualdad estructural
a is b; // misma instancia
```

`is` se utilizará cuando sea necesario comprobar identidad de instancia.

---

## 5.2 Comparadores

**TypeScript:**

```ts
a < b;
a <= b;
a > b;
a >= b;
```

**Pregunta:** ¿Se mantienen estos operadores?

**Respuesta:**

Sí. Se conservarán `<`, `<=`, `>` y `>=`.

---

## 5.3 Operadores lógicos

**TypeScript:**

```ts
a && b;
a || b;
!a;
```

Alternativa textual:

```text
a and b
a or b
not a
```

**Respuesta:**

Se utilizarán únicamente los operadores simbólicos `&&`, `||` y `!`, no sus variantes textuales.

---

## 5.4 Operadores aritméticos

**TypeScript:**

```ts
a + b;
a - b;
a * b;
a / b;
a % b;
a ** b;
```

**Pregunta:** ¿Todos existen?

**Respuesta:**

Sí. Existirán todos los operadores aritméticos propuestos.

---

## 5.5 Sobrecarga de operadores

**Pregunta:** ¿Las clases pueden redefinir operadores?

**Python:**

```python
class Vector:
    def __add__(self, other):
        ...
```

Posible lenguaje:

```text
class Vector {
    operator +(other: Vector): Vector {
        ...
    }
}
```

**Respuesta:**

Sí. Los tipos propios implementarán los contratos de operador mediante métodos reservados como `_add` y `_subtract`. Esto será código seguro. Los tipos nativos no podrán reabrirse ni reemplazar su comportamiento desde una aplicación; `unsafe` seguirá reservado para memoria y ABI.

---

## 6.1 Scope de bloque

**TypeScript:**

```ts
if (true) {
    let value = 10;
}

// value no existe aquí
```

**Pregunta:** ¿`mut` e `inmut` tienen scope de bloque?

**Respuesta:**

Sí. `mut` e `inmut` tendrán scope de bloque y existirán únicamente dentro de las llaves `{ }`, como en TypeScript.

---

## 6.2 Scope de función

**TypeScript:**

```ts
function test() {
    const local = 10;
}
```

**Pregunta:** ¿Las variables internas son siempre privadas a la función?

**Respuesta:**

Las variables declaradas dentro de una función serán privadas a esa ejecución y no podrán accederse desde fuera de la función.

```text
fn calculate(): Int32 {
    mut result = 10;
    return result;
}

stdout.println(result); // Error de compilación.
```

Una lambda creada dentro de la función podrá capturarlas, siempre que respete sus reglas de mutabilidad, vida útil y concurrencia.

---

## 6.3 Scope de archivo

**Pregunta:** ¿Una declaración de nivel superior es privada al archivo por defecto?

**TypeScript:**

```ts
const internal = 1;
export const publicValue = 2;
```

**Respuesta:**

Sí. Toda declaración de nivel superior será privada al archivo por defecto. Para exponer código a otros archivos se utilizará `share`:

```text
class InternalParser {}

share class UserService {}

share fn create_user(): User {
    ...
}
```

Los demás archivos solo podrán importar declaraciones compartidas:

```text
import { UserService } from "./user_service";
```

Las variables globales serán una excepción controlada: solo podrán declararse en `globals` del `init.zrk` de una aplicación y se habilitarán explícitamente mediante `use`.

---

## 6.4 Scope de módulo

**Pregunta:** ¿Cómo se define un módulo?

Posible:

```text
module users.auth;
```

**TypeScript:**

```ts
export class AuthService {}
```

**Respuesta:**

Los módulos se organizarán mediante archivos y rutas. Una declaración deberá marcarse con `share` para que pueda importarse desde otro archivo o módulo.

```text
share class AuthService {
}
```

El consumidor la incorporará mediante `import`:

```text
import { AuthService } from "users/auth";
```

`share` expone código; `import` lo incorpora.

---

## 6.5 Scope global

Definido conceptualmente:

```text
// init.zrk
globals {
    mut COUNTER: Atomic<UInt64> = Atomic(0);
}
```

Preguntas:

- ¿Puede ser leído desde cualquier archivo?
- ¿Requiere `use`?
- ¿Puede modificarse desde cualquier archivo?
- ¿Puede ser privado a un módulo?
- ¿Cómo se sincroniza si varios threads lo usan?

**Respuesta:**

1. Las variables globales solo se declararán dentro de `globals` en `init.zrk`.
2. Cualquier archivo podrá acceder a ellas si las habilita explícitamente mediante `use`.
3. `use` no requiere una ruta porque los globales proceden de una única tabla central definida en `init.zrk`.
4. Sin `use`, el global no será accesible dentro del archivo.
5. Todos los archivos que habiliten un global observarán la misma instancia y el mismo valor compartido.

```text
use APP_NAME;
use REQUEST_COUNT;
```

Un global `mut` utilizado desde `parallel` o `thread` deberá protegerse con `sync` o declararse como `Atomic<T>`; de lo contrario habrá un error de compilación.

---

## 7.1 If / else

**TypeScript:**

```ts
if (age >= 18) {
    console.log("Adulto");
} else {
    console.log("Menor");
}
```

Posible lenguaje:

```text
if age >= 18 {
    print("Adulto");
} else {
    print("Menor");
}
```

**Pregunta:** ¿Paréntesis obligatorios?

**Respuesta:**

Los paréntesis serán opcionales.

---

## 7.2 If como expresión

**Pregunta:** ¿Puede retornar valor?

**Python:**

```python
status = "adulto" if age >= 18 else "menor"
```

Posible:

```text
inmut status = if age >= 18 {
    "adulto"
} else {
    "menor"
};
```

**Respuesta:**

`if` podrá ser una expresión cuando todas sus ramas produzcan tipos compatibles. Para una elección breve se preferirá el operador ternario `condition ? yes : no`; para ramas con varias operaciones se utilizará `if` como expresión.

---

## 7.3 Match / switch

**TypeScript:**

```ts
switch (status) {
    case 200:
        break;
}
```

Posible lenguaje moderno:

```text
match status {
    200 => "ok";
    404 => "not found";
    _   => "error";
}
```

**Respuesta:**

Se utilizará `match` en lugar de `switch`. Una misma rama podrá aceptar varios valores:

```text
match status {
    200, 201, 202, 203 => {
        stdout.println("OK");
    }

    404 => {
        stderr.println("Not Found");
    }

    _ => {
        stderr.println("Unknown");
    }
};
```

`match` tendrá semántica contextual:

- Como sentencia independiente, ejecutará una rama y no producirá un valor.
- En un contexto que requiera una expresión, producirá un valor obligatoriamente.

Las alternativas de una misma rama se separarán con comas y compartirán un solo
`=>`. Los regex serán literales tipados `re'patrón'` y podrán utilizarse como
patrones de strings:

```text
match input {
    re'^[0-9]+$' => parse_number(input);
    _ => reject(input);
}
```

Ejemplo como expresión:

```text
mut message: String = match status {
    200, 201, 202, 203 => "OK";
    404 => "Not Found";
    _ => "Unknown";
};
```

No existirán modificadores `capture` ni `yield`. `return` continuará reservado para finalizar la función completa.

---

## 8.1 for

**TypeScript:**

```ts
for (let i = 0; i < 10; i++) {
    console.log(i);
}
```

¿Quieres este estilo?

**Respuesta:**

Sí. Se permitirá el `for` clásico, utilizando `mut` en lugar de `let`.

---

## 8.2 for-in / for-of

**TypeScript:**

```ts
for (const user of users) {
    console.log(user);
}
```

Posible:

```text
for user in users {
    print(user);
}
```

**Respuesta:**

Sí. Se usará `for element in collection` y el elemento podrá desestructurarse, por ejemplo `for { name } in users` o `for [item, index] in list`.

---

## 8.3 while

**TypeScript:**

```ts
while (running) {
    tick();
}
```

**Respuesta:**

Se usará la sintaxis propuesta y los paréntesis serán opcionales.

También existirá la variante con comprobación posterior:

```text
do {
    tick();
} while running;
```

El cuerpo de `do ... while` se ejecutará al menos una vez.

---

## 8.4 loop infinito

**Pregunta:** ¿Existirá una palabra dedicada?

Posible:

```text
loop {
    tick();
}
```

**C:**

```c
for (;;) {
}
```

**Respuesta:**

No existirá una palabra reservada `loop`; un bucle infinito se escribirá con `while true`.

---

## 8.5 break / continue

**TypeScript:**

```ts
break;
continue;
```

**Pregunta:** ¿Existen? ¿Permiten labels?

**Respuesta:**

Sí. `break` y `continue` serán válidos.

---

## 9.1 Declaración

**TypeScript:**

```ts
function add(a: number, b: number): number {
    return a + b;
}
```

Posible:

```text
fn add(a: Int32, b: Int32): Int32 {
    return a + b;
}
```

**Respuesta:**

Se utilizará la sintaxis propuesta con `fn`.

---

## 9.2 Funciones expresión

**TypeScript:**

```ts
const add = (a: number, b: number) => a + b;
```

Posible:

```text
inmut add = fn(a: Int32, b: Int32): Int32 => a + b;
```

**Respuesta:**

Se usará la sintaxis propuesta, pero `fn` será opcional en funciones expresión.

---

## 9.3 Parámetros opcionales

**TypeScript:**

```ts
function greet(name?: string) {}
```

Posible:

```text
fn greet(name: String?) {}
```

**Respuesta:**

El `?` se colocará en el nombre del parámetro para indicar que puede omitirse: `name?`. En el tipo, `String?` significará `String` o `null`.

---

## 9.4 Valores por defecto

**TypeScript:**

```ts
function greet(name: string = "Mundo") {}
```

Posible:

```text
fn greet(name: String = "Mundo"): Void {}
```

**Respuesta:**

Sí. Se admitirán valores predeterminados con la sintaxis propuesta.

---

## 9.5 Parámetros nombrados

**Python:**

```python
create_user(name="Cristian", active=True)
```

Posible:

```text
createUser(name: "Cristian", active: true);
```

**Respuesta:**

Sí. Se permitirán parámetros nombrados, utilizando nombres en `snake_case`.

---

## 9.6 Variadic

**TypeScript:**

```ts
function sum(...values: number[]) {}
```

Posible:

```text
fn sum(...values: Int32[]): Int32 {}
```

**Respuesta:**

Sí. Se usará la sintaxis variádica propuesta.

---

## 9.7 Sobrecarga

**TypeScript:**

```ts
function parse(value: string): number;
function parse(value: number): number;
```

**Pregunta:** ¿Se permite sobrecarga real?

**Respuesta:**

No habrá sobrecarga real. Se utilizarán union types, por ejemplo `String | Number`.

---

## 10.1 Declaración

**TypeScript:**

```ts
class User {
    name: string;

    constructor(name: string) {
        this.name = name;
    }
}
```

Posible lenguaje:

```text
class User {
    public mut name: String;

    construct(name: String) {
        self.name = name;
    }
}
```

**Respuesta:**

Se utilizará la sintaxis propuesta, sustituyendo `self` por `this`.

---

## 10.2 Constructor

**Pregunta:** ¿Cómo se llama?

Opciones:

- `constructor`
- `construct`
- `init`
- mismo nombre de la clase

**Respuesta:**

El constructor se llamará `construct`.

Una clase podrá declarar múltiples `construct` siempre que sus firmas efectivas
no sean idénticas ni ambiguas. La resolución considerará cantidad, tipos,
parámetros opcionales y argumentos nombrados. Esta excepción no habilitará
sobrecarga general de funciones o métodos.

---

## 10.3 Instanciación

**TypeScript:**

```ts
const user = new User("Cristian");
```

Posibles:

```text
mut user = new User("Cristian");
```

o:

```text
mut user = User("Cristian");
```

**Respuesta:**

La instanciación no utilizará `new`. Las funciones seguirán `snake_case` y las clases `UpperCamelCase`, por lo que serán distinguibles.

---

## 10.4 Self / this

**Pregunta:** ¿Cómo se referencia la instancia actual?

**TypeScript:**

```ts
this.name
```

**Python:**

```python
self.name
```

Opciones:

- `this`
- `self`

**Respuesta:**

La instancia actual se referenciará con `this`.

---

## 10.5 Visibilidad

**TypeScript:**

```ts
public name: string;
private password: string;
protected id: number;
```

Posibles:

```text
public
private
protected
internal
package
```

**Respuesta:**

Existirán `public`, `private` y `protected`.

---

## 10.6 Visibilidad por defecto

**Pregunta:** ¿Qué ocurre si no se especifica?

```text
mut name: String;
```

Opciones:

- public
- private
- internal

**Respuesta:**

La visibilidad predeterminada será `public`.

Los campos de clase también serán `mut` por defecto. Por tanto, `name: String;`
equivaldrá a `public mut name: String;`; `private` e `inmut` deberán escribirse
cuando se quiera apartar de esos defaults.

---

## 10.7 Herencia

**TypeScript:**

```ts
class Admin extends User {}
```

Posible:

```text
class Admin extends User {}
```

**Pregunta:** ¿Una clase puede heredar de una sola clase o múltiples?

**Respuesta:**

Se elimina la herencia múltiple de clases. Cada clase podrá extender una única clase padre, implementar múltiples interfaces y utilizar múltiples traits.

---

## 10.8 Clases cerradas por defecto

**Pregunta:** ¿Las clases pueden heredarse automáticamente?

Posible:

```text
class User {}
```

cerrada por defecto.

Para heredar:

```text
open class User {}
```

**Respuesta:**

Las clases serán heredables automáticamente.

---

## 10.9 Clase abstracta

**TypeScript:**

```ts
abstract class Animal {
    abstract speak(): void;
}
```

Posible:

```text
abstract class Animal {
    abstract fn speak(): Void;
}
```

**Respuesta:**

Sí. Se usará la sintaxis propuesta para clases y métodos abstractos.

---

## 10.10 Clase final

**Pregunta:** ¿Existe `final`?

Posible:

```text
final class DatabaseConnection {}
```

**Respuesta:**

No existirá `final`.

---

## 10.11 Métodos y propiedades estáticas

**TypeScript:**

```ts
class MathUtil {
    static PI = 3.14;
}
```

Posible:

```text
class MathUtil {
    static inmut PI: Float64 = 3.14;
}
```

**Respuesta:**

Sí. Los métodos y propiedades estáticos utilizarán la sintaxis propuesta.

---

## 11.1 Interfaces

**TypeScript:**

```ts
interface Serializable {
    serialize(): string;
}
```

Posible:

```text
interface Serializable {
    fn serialize(): String;
}
```

**Respuesta:**

Sí. Las interfaces utilizarán la sintaxis propuesta.

---

## 11.2 Implementación

**TypeScript:**

```ts
class User implements Serializable {}
```

Posible:

```text
class User implements Serializable {}
```

**Respuesta:**

Sí. Las clases implementarán interfaces mediante `implements`.

---

## 11.3 Traits

TypeScript no tiene traits reales.

**Python aproximado con mixins:**

```python
class LoggableMixin:
    def log(self):
        ...
```

Posible lenguaje:

```text
trait Loggable {
    fn log(): Void {
        ...
    }
}
```

**Pregunta:** ¿Existirán traits con implementación reutilizable?

**Respuesta:**

Sí. Existirán traits con implementación reutilizable mediante la sintaxis propuesta.

---

## 12.1 Sintaxis

**TypeScript:**

```ts
class Box<T> {
    value: T;
}
```

Posible:

```text
class Box<T> {
    mut value: T;
}
```

**Respuesta:**

Sí. Los genéricos estarán permitidos y usarán una sintaxis equivalente a la de TypeScript.

---

## 12.2 Restricciones

**TypeScript:**

```ts
function serialize<T extends Serializable>(value: T) {}
```

Posible:

```text
fn serialize<T: Serializable>(value: T): String {}
```

**Respuesta:**

Las restricciones utilizarán `from`: `fn serialize<T from Serializable>(value: T): String {}`.

---

## 12.3 Especialización

**Pregunta:** ¿El compilador genera versiones especializadas para tipos concretos?

Ejemplo:

```text
Box<Int32>
Box<String>
```

Esto afecta rendimiento y tamaño del binario.

**Respuesta:**

Sí. El compilador generará especializaciones para tipos concretos.

---

## 13.1 Array

**TypeScript:**

```ts
const values: number[] = [1, 2, 3];
```

Posibles:

```text
mut values: Array<Int32> = [1, 2, 3];
```

o:

```text
mut values: Int32[] = [1, 2, 3];
```

**Respuesta:**

Los arrays admitirán `Array<Int32>` e `Int32[]`. Serán de tamaño fijo. Si no se especifica un tamaño, se tomará el definido por el inicializador; para indicarlo explícitamente se usará `Array<Int32>(n)` o `Int32[n]`.

---

## 13.2 Arrays de tamaño fijo

**C:**

```c
int values[10];
```

Posible:

```text
mut values: Int32[10];
```

**Respuesta:**

Sí. Los arrays tendrán tamaño fijo según lo definido en la sección anterior.

---

## 13.3 Listas dinámicas

**Pregunta:** ¿Array y List son diferentes?

Posible:

```text
Array<Int32>
List<Int32>
Vector<Int32>
```

**Respuesta:**

Sí. `Array` y `List` serán diferentes: `Array` tendrá tamaño fijo y `List` será dinámica, como los arrays de JavaScript.

---

## 13.4 Map / Dictionary

**TypeScript:**

```ts
const users = new Map<string, User>();
```

Posible:

```text
mut users: Map<String, User>;
```

**Respuesta:**

Sí. Existirá `Map<K, V>` con la sintaxis propuesta.

---

## 13.5 Set

**TypeScript:**

```ts
const ids = new Set<number>();
```

Posible:

```text
mut ids: Set<UInt64>;
```

**Respuesta:**

Sí. Existirá `Set<T>` con la sintaxis propuesta.

---

## 14.1 Excepciones

**TypeScript:**

```ts
try {
    execute();
} catch (error) {
    console.error(error);
}
```

Posible:

```text
try {
    execute();
} catch error {
    print(error);
}
```

**Pregunta:** ¿Existirán excepciones?

**Respuesta:**

Sí. Existirán excepciones y soporte para capturar errores por tipo, además de un caso predeterminado y `finally`:

```text
try {
    execute();
} catch<HttpError> error {
    print(error);
} default error {
    print(error);
} finally {
}
```

Las excepciones se reservarán para situaciones excepcionales recuperables.

---

## 14.2 Result tipado

TypeScript no tiene un `Result` nativo, pero puede modelarse:

```ts
type Result<T, E> =
    | { ok: true; value: T }
    | { ok: false; error: E };
```

Posible lenguaje:

```text
fn findUser(id: UInt64): Result<User, UserError> {
}
```

**Pregunta:** ¿Será el mecanismo principal de errores recuperables?

**Respuesta:**

Sí. `Result<T, E>` será el mecanismo nativo principal para errores esperables y recuperables.

---

## 14.3 Propagación de errores

**Pregunta:** ¿Existirá operador equivalente a `?` de Rust?

Posible:

```text
mut user = findUser(id)?;
```

**Respuesta:**

No existirá el operador `?` para propagar un `Result`. Los ejemplos usarán manejo explícito mediante `match` y `return Error(error)` hasta que se decida otro mecanismo.

---

## 14.4 Panic / fatal

**Pregunta:** ¿Existe un error irrecuperable?

Posible:

```text
panic("Estado imposible");
```

**Respuesta:**

Sí. Los estados irreparables se representarán mediante `fatalError("Estado imposible");`.

En conjunto: `Result` se usará para fallos esperables y recuperables; las exceptions, para situaciones excepcionales recuperables; y `fatalError`, para estados irreparables.

---

## 15.1 Stack y heap

**Pregunta:** ¿El programador puede distinguirlos o el compilador decide?

**C:**

```c
int value = 10;              // stack
int *ptr = malloc(sizeof(int)); // heap
```

Opciones:

- [ ] Totalmente automático
- [ ] Control explícito opcional
- [ ] Control total
- [x] Híbrido

**Respuesta:**

La administración de memoria será automática como modelo oficial. El compilador/runtime decidirá la ubicación óptima —stack o heap— y podrá aplicar escape analysis u otras estrategias internas. El desarrollador no dependerá de una sintaxis pública como `mut<heap/stack>` en código normal.

---

## 15.2 Garbage collector

**Pregunta:** ¿Habrá GC?

**TypeScript/JavaScript:** administración automática.

```ts
let user = new User();
user = null;
```

El runtime eventualmente libera memoria.

**Respuesta:**

El runtime liberará memoria automáticamente y de manera eficiente.

---

## 15.3 Reference counting

**Pregunta:** ¿Se usará conteo de referencias?

No existe directamente en TypeScript.

**Concepto aproximado:**

```text
objeto.refCount += 1;
objeto.refCount -= 1;
```

**Respuesta:**

El lenguaje no comprometerá su semántica pública a reference counting. El runtime podrá utilizarlo internamente cuando sea eficiente, junto con otras estrategias de administración automática.

---

## 15.4 Ownership

TypeScript no tiene ownership.

**Concepto inspirado en lenguajes de sistemas:**

```text
mut data = Buffer(...);
process(data);
// ¿data sigue siendo válido?
```

**Pregunta:** ¿Quieres ownership/move semantics?

**Respuesta:**

No habrá ownership obligatorio en el código normal. El compilador/runtime administrará la memoria automáticamente y podrá aplicar movimientos u optimizaciones internas cuando sea necesario.

---

## 16.1 Punteros

TypeScript y Python no exponen punteros; usamos C.

**C:**

```c
int value = 10;
int *ptr = &value;

printf("%d", *ptr);
```

**Pregunta:** ¿Tu lenguaje permitirá punteros explícitos?

Posible:

```text
mut value: Int32 = 10;
mut ptr: Pointer<Int32> = &value;
```

**Respuesta:**

Sí. Existirán punteros explícitos mediante `Pointer<T>`, tratados como clases según la sintaxis propuesta, pero solo podrán utilizarse dentro de `unsafe { }`.

---

## 16.2 Dereferencia

**C:**

```c
*ptr = 20;
```

Posible:

```text
*ptr = 20;
```

**Respuesta:**

Sí. La dereferencia usará `*ptr` y solo será válida dentro de `unsafe { }`.

---

## 16.3 Unsafe

**Pregunta:** ¿Los punteros solo pueden utilizarse dentro de un bloque especial?

Posible:

```text
unsafe {
    mut ptr: Pointer<Int32> = &value;
    *ptr = 20;
}
```

**Respuesta:**

Sí. Los punteros, su dereferencia y las demás operaciones de memoria insegura estarán limitados a bloques `unsafe { }`.

---

## 16.4 Referencias seguras

**Pregunta:** ¿Existirá un concepto separado de referencia que no pueda ser null ni apuntar a memoria inválida?

Posible:

```text
fn modify(user: ref User): Void {}
```

**Respuesta:**

Podrán existir referencias seguras (`ref T`) cuando aporten eficiencia sin perder seguridad. Serán no nulas y estarán separadas de los punteros crudos de `unsafe`.

---

## 17.1 Objetivo

Principio deseado:

> Paralelismo real, pero sencillo para el desarrollador.

**Pregunta:** ¿Qué abstracciones tendrá el lenguaje?

Posibles:

```text
task
parallel
worker
thread
await
sync
channel
```

**Respuesta:** El lenguaje será síncrono y secuencial por defecto.

Las abstracciones principales de ejecución serán:

- task: concurrencia administrada por el runtime.
- parallel: paralelismo real para trabajo CPU, usando múltiples núcleos cuando sea posible.
- thread: creación explícita de un hilo nativo del sistema operativo.
- await: espera no bloqueante del resultado de una task.
- sync: protección de acceso a estado compartido.
- channel: comunicación segura y tipada entre unidades concurrentes.

Worker no será una abstracción principal del lenguaje por ahora, ya que su comportamiento puede construirse posteriormente sobre task, thread y channel si realmente se necesita.

Las abstracciones podrán combinarse. Por ejemplo:

task parallel {
    processLargeDataset();
}

Esto significa ejecutar el trabajo en paralelo usando múltiples núcleos, pero sin bloquear inmediatamente el flujo que lo inició.

El objetivo es que el desarrollador no tenga que administrar pools de threads, cantidad de núcleos o schedulers para obtener concurrencia o paralelismo real.

---

## 17.2 Task

**TypeScript aproximado:**

```ts
const result = await fetchData();
```

Posible:

```text
mut result = await task fetchData();
```

**Pregunta:** ¿`task` representa concurrencia administrada por runtime?

**Respuesta:** Sí.

`task` representa una operación concurrente administrada completamente por el runtime.

No crea obligatoriamente un thread nuevo ni garantiza ejecución en otro núcleo.

El runtime decide cómo y dónde ejecutarla mediante su scheduler interno.

Por defecto, una función normal es síncrona:

mut users = getUsers();

Al utilizar task:

mut usersTask = task getUsers();

el flujo actual continúa ejecutándose mientras la operación puede avanzar concurrentemente.

Para obtener el resultado se utiliza `await`:

mut users = await usersTask;

Una task retorna conceptualmente:

Task<T>

donde T corresponde al tipo retornado por la operación original.

Ejemplo:

fn getUsers(): Array<User>;

mut operation: Task<Array<User>> = task getUsers();
mut users: Array<User> = await operation;

Las tasks utilizarán structured concurrency por defecto: estarán asociadas al scope donde fueron creadas y no podrán quedar ejecutándose indefinidamente sin una declaración explícita que permita desprenderlas.

---

## 17.3 Parallel

TypeScript no ofrece paralelismo CPU real directamente.

**Python aproximado con multiprocessing:**

```python
from multiprocessing import Pool
```

Posible lenguaje:

```text
parallel {
    processLargeDataset();
}
```

**Pregunta:** ¿`parallel` debe garantizar ejecución en otro núcleo cuando sea posible?

**Respuesta: ** Sí.

`parallel` indica explícitamente que una operación debe utilizar paralelismo CPU real cuando el hardware y el runtime lo permitan.

El runtime distribuirá automáticamente el trabajo entre múltiples núcleos y threads internos.

El desarrollador no deberá configurar manualmente:

- cantidad de threads
- cantidad de workers
- número de núcleos
- tamaño del pool
- distribución del trabajo

Por defecto, `parallel` mantiene comportamiento síncrono respecto del flujo que lo llamó.

Ejemplo:

mut result = parallel {
    return processLargeDataset();
};

La ejecución no continúa después del bloque hasta que el trabajo paralelo haya terminado.

Para ejecutar trabajo paralelo sin bloquear inmediatamente al caller se combinará con task:

mut operation = task parallel {
    return processLargeDataset();
};

doSomethingElse();

mut result = await operation;

Por lo tanto:

parallel
→ multicore + espera el resultado.

task parallel
→ multicore + ejecución concurrente respecto del caller.

---

## 17.4 Parallel for

Posible:

```text
parallel for file in files {
    process(file);
}
```

Preguntas:

- ¿Orden garantizado?
- ¿Número de workers automático?
- ¿Cómo se recolectan resultados?
- ¿Qué pasa si una iteración falla?

**Respuesta:**

`parallel for` distribuirá automáticamente las iteraciones entre los núcleos disponibles. El runtime decidirá la cantidad de unidades de ejecución según los núcleos, la carga del sistema, el tamaño de la colección y la naturaleza del trabajo.

No se garantizará el orden de ejecución. Cuando se produzca una colección de resultados, esta conservará por defecto el orden lógico de la entrada. Si el cuerpo no retorna un valor, el resultado será `Void`; si retorna uno, se inferirá una colección del tipo correspondiente.

Si una iteración retorna `Result<T, E>`, el error se manejará explícitamente, sin utilizar `?`. Al fallar una iteración se registrará el primer error relevante, se solicitará la cancelación segura del trabajo pendiente y el `parallel for` retornará el error al caller. Nunca se terminarán threads abruptamente de forma insegura.

---

## 17.5 Worker

**TypeScript usa Web Workers en navegador:**

```ts
const worker = new Worker("worker.js");
```

Posible lenguaje:

```text
worker ImageProcessor {
    fn process(image: Image): Image {
        ...
    }
}
```

Uso:

```text
mut processor = ImageProcessor.spawn();
mut result = await processor.process(image);
```

**Pregunta:** ¿Worker tiene memoria aislada?

**Respuesta:**

`worker` no será una keyword, un tipo fundamental ni una abstracción nativa del lenguaje.

Su comportamiento se construirá mediante la composición de `task`, `thread`, `Channel<Message>` y estado privado. La biblioteca estándar o paquetes externos podrán ofrecer posteriormente una clase `Worker<Message, Result>`, pero se implementará sobre esas primitivas y no introducirá una semántica adicional en el compilador.

Esta decisión evita duplicar reglas de concurrencia, memoria, cancelación y comunicación que ya están cubiertas por las abstracciones existentes.

---

## 17.6 Thread

TypeScript no tiene threads nativos como sintaxis del lenguaje.

**C:**

```c
pthread_create(...);
```

Posible:

```text
thread {
    runLoop();
}
```

Preguntas:

- ¿`thread` crea siempre un thread nativo?
- ¿Se puede nombrar?
- ¿Se puede seleccionar prioridad?
- ¿Se puede cancelar?

**Respuesta:**
Sí.

`thread` siempre creará un thread nativo real del sistema operativo.

No será una abstracción virtual ni una task administrada por el scheduler.

Ejemplo:

thread {
    runLoop();
}

Esto creará un hilo independiente al thread actual.

---

Sí.

Será posible asignar un nombre al thread para debugging, profiling y observabilidad.

Ejemplo:

mut audioThread = thread "audio-engine" {
    audio.run();
};

El nombre será opcional.

---

Sí, pero no mediante la sintaxis básica de `thread`.

La prioridad será una capacidad avanzada y explícita.

Ejemplo conceptual:

mut audioThread = thread "audio-engine" priority High {
    audio.run();
};

El runtime traducirá las prioridades abstractas del lenguaje a las capacidades disponibles en cada sistema operativo.

Las prioridades podrán ser:

Low
Normal
High
Critical

Su comportamiento exacto dependerá del sistema operativo y no se garantizará una equivalencia exacta entre plataformas.

---

Sí, pero la cancelación será cooperativa y segura.

No se permitirá matar arbitrariamente un thread mientras está ejecutando código, ya que podría dejar memoria, locks o recursos en estados inconsistentes.

Ejemplo:

mut workerThread = thread {
    while !Thread.cancelled() {
        process();
    }
};

workerThread.cancel();
workerThread.join();

`cancel()` solicitará la finalización.

El thread deberá alcanzar un punto seguro de cancelación.

`join()` esperará su terminación.

---

Las operaciones creadas mediante `task` o `parallel` dentro de un thread no estarán obligadas a permanecer físicamente en ese mismo thread.

Ejemplo:

thread {
    mut result = parallel {
        calculate();
    };
}

El thread actual coordina la operación, pero `parallel` puede utilizar otros threads administrados por el runtime.

`thread` controla el flujo actual.

`task` y `parallel` pueden delegar trabajo al scheduler global.

---

Código normal
→ síncrono + secuencial

task
→ concurrente + scheduler automático

parallel
→ síncrono respecto al caller + multicore

task parallel
→ concurrente + multicore

thread
→ thread nativo explícito

---

## 17.7 Channels

**Pregunta:** ¿Los workers/threads se comunican mediante channels?

Posible:

```text
mut channel: Channel<Message> = Channel();

parallel {
    channel.send(message);
}

mut result = channel.receive();
```

**Respuesta:**

Sí. Existirá `Channel<T>` como mecanismo de comunicación segura y tipada entre tasks, operaciones `parallel` y threads. Su uso con workers dependerá de la decisión pendiente sobre `worker`.

Un channel transportará exclusivamente valores compatibles con `T`, será thread-safe por diseño y no requerirá `sync` para `send()` o `receive()`.

```text
mut channel: Channel<String> = Channel();

task {
    channel.send("Proceso terminado");
}

mut message: String = await channel.receive();
```

---

## 17.8 Data races

**Pregunta:** ¿El compilador intenta impedir condiciones de carrera?

Ejemplo peligroso:

```text
// init.zrk
globals {
    mut COUNTER: UInt64 = 0;
}

// otro archivo
use COUNTER;

parallel {
    COUNTER++;
}
```

Posibles respuestas:

- Error de compilación
- Requiere `sync`
- Automáticamente atómico
- Permitido bajo responsabilidad del programador

**Respuesta:**

El compilador impedirá acceder sin protección a una variable mutable declarada en `globals` desde `parallel` o `thread`. Será obligatorio utilizar `sync` o declarar la variable mediante `Atomic<T>`; de lo contrario habrá un error de compilación.

```text
// init.zrk
globals {
    mut REQUEST_COUNT: Atomic<UInt64> = Atomic(0);
}
```

```text
// otro archivo
use REQUEST_COUNT;

parallel {
    REQUEST_COUNT.increment();
}
```

---

## 17.9 Mutex / lock

**C conceptual:**

```c
pthread_mutex_lock(&mutex);
```

Posible:

```text
sync counter {
    counter++;
}
```

**Respuesta:**

Sí. `sync value { ... }` protegerá el acceso exclusivo a estado compartido.

---

## 17.10 Atomic

**Pregunta:** ¿Habrá tipos atómicos?

Posible:

```text
mut counter: Atomic<UInt64> = 0;
counter.increment();
```

**Respuesta:**

Sí. Existirá `Atomic<T>` para operaciones atómicas sobre los tipos compatibles.

---

## 18.1 Función async

**TypeScript:**

```ts
async function load(): Promise<User> {
    return await getUser();
}
```

Posible:

```text
async fn load(): User {
    return await getUser();
}
```

Pregunta importante:

¿El tipo debe mostrar explícitamente la asincronía?

```text
async fn load(): Future<User>
```

vs.

```text
async fn load(): User
```

**Respuesta:**

```
No existirá `async fn`.

Las funciones serán síncronas y secuenciales por defecto. El caller
decidirá si una llamada debe ejecutarse concurrentemente mediante
`task`.

Ejemplo:

fn load(): User {
    return getUser();
}

Llamada síncrona:

mut user: User = load();

Llamada concurrente:

mut operation: Task<User> = task load();
mut user: User = await operation;

Forma directa:

mut user: User = await task load();

Los tipos se determinarán según la expresión:

load()
→ User

task load()
→ Task<User>

await task load()
→ User

`await` solo podrá aplicarse a valores esperables, como `Task<T>`,
operaciones de Channel<T> u otras abstracciones administradas por
el runtime.

Cuando una task espere I/O, el runtime suspenderá la task sin bloquear
innecesariamente el thread nativo que la ejecuta.
```

---

## 18.2 Cancelación

**Pregunta:** ¿Las tareas async pueden cancelarse?

Posible:

```text
mut task = async loadData();
task.cancel();
```

**Respuesta:** Sí.

Las instancias de `Task<T>` podrán recibir una solicitud de cancelación
mediante `cancel()`.

Ejemplo:

mut operation: Task<Data> = task loadData();

operation.cancel();

La cancelación será cooperativa y segura. `cancel()` no terminará
abruptamente el thread nativo que esté ejecutando la task.

El runtime cancelará las esperas que puedan interrumpirse y la task
terminará cuando alcance un punto seguro de cancelación.

Las operaciones extensas de CPU podrán consultar explícitamente la solicitud de cancelación mediante el contexto de la task actual:

```text
Task.is_cancellation_requested();
```

Cancelar una task será una situación excepcional recuperable, nunca un `fatalError`. Al esperar una task que terminó cancelada, `await` lanzará `TaskCancelledException`.

`await` conservará normalmente el tipo contenido por la task:

```text
Task<User>                      → await retorna User
Task<Result<User, NetworkError>> → await retorna Result<User, NetworkError>
```

Esto evitará introducir tipos anidados como `Result<Result<T, E>, TaskError>` en todas las operaciones concurrentes.

```text
mut operation = task load_user();

operation.cancel();

try {
    mut result = await operation;
} catch TaskCancelledException {
    stdout.println("La operación fue cancelada");
}
```

`cancel()` será idempotente. Si la task terminó antes de procesar la solicitud conservará su resultado normal.

```text
operation.is_cancellation_requested();
operation.is_cancelled();
```

La primera operación indicará que se solicitó cancelación; la segunda, que la task terminó efectivamente cancelada. Las tasks extensas de CPU podrán consultar la solicitud en puntos seguros.



---

## 18.3 Timeout

**Posible:**

```text
await loadData() timeout 5s;
```

**Respuesta:** Si

`await` podrá establecer un tiempo máximo de espera mediante `timeout`.

Ejemplo:

mut operation: Task<Data> = task loadData();

mut result = await operation timeout 5s;

Si la task termina antes del límite, se obtiene su resultado.

Si el tiempo se agota, `await` lanzará la exception recuperable `TaskTimeoutException`.

Por defecto, el timeout solo finaliza la espera del caller. La task
continuará ejecutándose y podrá esperarse posteriormente o cancelarse
de forma explícita:

operation.cancel();

Un timeout nunca producirá un `fatalError` ni terminará abruptamente el thread que ejecuta la task.

```text
try {
    mut result = await operation timeout 5s;
} catch TaskTimeoutException {
    operation.cancel();
}
```

El timeout solo interrumpirá la espera del caller. Cancelar la task seguirá siendo una decisión explícita.

Las cantidades de tiempo serán valores `Duration` con literales legibles:

```text
500ns
250us
5_000ms
5s
5m
2h
1d
2w
```

Los sufijos serán `ns`, `us`, `ms`, `s`, `m`, `h`, `d` y `w`. Meses y años
pertenecerán a `Period`, por lo que `m` no será ambiguo con mes.

```text
mut wait_limit: Duration = 5s;
mut interval: Duration = 250ms;

await operation timeout wait_limit;
task.sleep(interval);
```

`Duration` será una cantidad exacta signed con precisión de nanosegundos y rango
conceptual `Int128`. Los valores negativos expresarán dirección en diferencias;
las APIs de espera exigirán valores no negativos. `Period` representará
años/meses/semanas/días de calendario sin total de segundos independiente.

La familia sellada `Temporal` distinguirá `Date`, `Time`, `DateTime`, `Instant`,
`ZonedDateTime`, `TimeZone`, `Duration` y `Period`, permitiendo solo
composiciones con significado. Las zonas serán IANA y las ambigüedades DST se
rechazarán por defecto salvo política explícita.

---

# 19. Módulos e imports

## 19.1 Import

**TypeScript:**

```ts
import { User } from "./User";
```

Posible:

```text
import User from "./User";
```

o:

```text
import { User } from "users";
```

**Respuesta:**

`import` incorporará módulos y declaraciones de código compartidas desde otros archivos, paquetes o la biblioteca estándar.

```text
import { User } from "path/users";
import std.io;
```

Las rutas no incluirán la extensión `.zrk`. Solo podrán importarse declaraciones expuestas mediante `share`.

`import` no se utilizará para variables globales; estas se habilitarán mediante `use`.

Al importar un objeto conocido de la biblioteca estándar se podrán invocar sus
miembros de conveniencia directamente cuando no exista ambigüedad:

```text
import { stdout } from std.io;
println("Hola"); // stdout.println("Hola")
```

Si existe otro `println`, deberá escribirse `stdout.println(...)`. Los objetos
de archivos locales o paquetes no inyectarán automáticamente sus métodos.

---

## 19.2 Export

**TypeScript:**

```ts
export class User {}
```

Pregunta:

- ¿Todo es privado salvo `public/export`?
- ¿Todo se exporta por defecto?

**Respuesta:**

Para exponer una declaración de código a otros archivos se utilizará `share`, no `export`.

```text
share class User {
}

share fn find_user(id: UInt64): Result<User, UserError> {
}
```

Las declaraciones que no lleven `share` no podrán importarse desde otros archivos.

`share` se aplicará a código —clases, funciones, interfaces, traits y otras declaraciones importables—, no a las variables globales de `init.zrk`.

---

## 19.3 Alias

**TypeScript:**

```ts
import { User as DomainUser } from "./User";
```

Posible:

```text
import { User as DomainUser } from "./User";
```

**Respuesta:**

Sí. Los aliases de imports utilizarán `->`, al igual que los aliases en la desestructuración.

```text
import { User -> DomainUser } from "path/users";
```

Dentro del archivo, `DomainUser` será el nombre local del símbolo original `User`. El alias no modificará su nombre original ni afectará a otros archivos.

`use` no admitirá aliases, para que el nombre de una variable global permanezca reconocible en todos los archivos:

```text
use REQUEST_COUNT;
```

---

## 19.4 Import global

**Pregunta:** ¿`init.zrk` puede registrar imports disponibles en todo el proyecto?

Posible:

```text
global import std.logging;
```

**Respuesta:**

No. `init.zrk` no podrá registrar imports que aparezcan automáticamente en todo el proyecto.

Cada archivo deberá declarar explícitamente sus dependencias de código mediante `import`:

```text
import std.logging;
import { User } from "path/users";
```

`use` quedará reservado exclusivamente para habilitar variables globales declaradas en el bloque `globals` de `init.zrk`:

```text
use APP_NAME;
use REQUEST_COUNT;
```

No existirá una sintaxis como:

```text
global import std.logging;
```

---

## 20.1 Rol de init.zrk

Concepto inicial:

> Archivo especial que configura el proyecto y registra valores globales.

**Pregunta:** ¿Qué puede contener?

- globals
- metadata
- targets
- dependencias
- permisos
- flags
- entrypoint
- configuración de runtime
- configuración del compilador

**Respuesta:**

`init.zrk` será el archivo especial de configuración básica del proyecto y el único lugar donde podrán declararse variables globales.

Los bloques permitidos dependerán del tipo de proyecto. Una aplicación podrá contener:

- `project`
- `build_targets`
- `globals`
- `permissions`
- `compile_permissions`
- `dependencies`
- `dev_dependencies`
- `test_permissions`

Una librería utilizará `project` y podrá declarar `requires`, pero no podrá declarar `globals`.

`project` definirá la metadata esencial, el tipo de proyecto y, en una aplicación, el punto de entrada.

`build_targets` será opcional. Registrará los destinos que deberán generarse al ejecutar `zirk build` sin un target explícito; no limitará las plataformas en las que el proyecto podría compilarse.

`globals` contendrá las variables globales compartidas por la aplicación. Dentro de este bloque no se repetirá la palabra `global`:

```text
project {
    name: "my-app";
    version: "0.1.0";
    type: application;
    entry: "src/main.zrk";
}

build_targets {
    "x86_64-windows";
    "x86_64-linux";
    "aarch64-macos";
}

globals {
    inmut APP_NAME: String = "My App";
    mut REQUEST_COUNT: Atomic<UInt64> = Atomic(0);
}

permissions {
    network: ["api.example.com"];
}

compile_permissions {
    read: ["./schemas"];
}
```

`init.zrk` no contendrá imports globales ni configuración del compilador o del runtime. Estos dos últimos decidirán automáticamente su comportamiento predeterminado; el control adicional se expresará directamente en el código.

`permissions` concederá capacidades durante la ejecución. `compile_permissions` concederá capacidades a decoradores y otros procesos ejecutados durante la compilación. Las librerías no podrán concederse permisos: solo declararán lo que necesitan mediante `requires`, y la aplicación decidirá si lo autoriza.

---

## 20.2 Ejemplo de estructura

```text
project {
    name: "my-app";
    version: "0.1.0";
    type: application;
    entry: "src/main.zrk";
}

build_targets {
    "x86_64-windows";
    "x86_64-linux";
    "aarch64-macos";
}

globals {
    inmut APP_NAME: String = "My App";
    mut REQUEST_COUNT: Atomic<UInt64> = Atomic(0);
}

permissions {
    network: ["api.example.com"];
}

compile_permissions {
    read: ["./schemas"];
}
```

**Pregunta:** ¿Esto es código normal o una DSL especial dentro del lenguaje?

**Respuesta:**

`init.zrk` será una DSL declarativa especial y no un archivo de código ejecutable normal.

Solo permitirá los bloques declarativos correspondientes al tipo de proyecto: `project`, `build_targets`, `globals`, `permissions`, `compile_permissions`, `test_permissions`, `requires`, `dependencies` y `dev_dependencies`.

```text
project {
    name: "my-app";
    version: "0.1.0";
    type: application;
    entry: "src/main.zrk";
}

build_targets {
    "x86_64-linux";
    "aarch64-macos";
}

globals {
    inmut APP_NAME: String = "My App";
    mut REQUEST_COUNT: Atomic<UInt64> = Atomic(0);
}

permissions {
    read: ["./data"];
}
```

No podrá contener funciones, clases, interfaces, traits, condicionales, bucles, tasks, bloques `parallel`, threads, bloques `unsafe`, imports ni código ejecutable arbitrario.

Las inicializaciones que requieran I/O, puedan fallar o necesiten ejecutar lógica deberán realizarse desde el entrypoint o desde funciones invocadas explícitamente por este.

---

## 21.1 Target nativo

Objetivo:

```text
source.zrk
    ↓
compiler
    ↓
ARM64 / x86-64
    ↓
Mach-O / ELF / PE
```

**Pregunta:** ¿El comando detecta automáticamente OS y arquitectura?

Posible:

```bash
zirk build
```

o explícito:

```bash
zirk build --target aarch64-apple-darwin
```

**Respuesta:** Sí.

Si no existe `build_targets` y tampoco se proporciona un target mediante la CLI, `zirk build` detectará automáticamente el sistema operativo y la arquitectura de la máquina actual.

Si `init.zrk` contiene `build_targets`, ejecutar:

```bash
zirk build
```

generará todos los destinos declarados:

```text
build_targets {
    "x86_64-windows";
    "x86_64-linux";
    "aarch64-macos";
}
```

También podrá solicitarse un destino concreto:

```bash
zirk build --target aarch64-macos
```

El target de la CLI tendrá prioridad y, durante esa ejecución, ignorará `build_targets`.

Las arquitecturas se nombrarán explícitamente: `x86` y `x86_64` para la familia x86; `armv7` y `aarch64` para ARM. Solo se admitirán combinaciones soportadas por el sistema operativo, el backend, el linker y las dependencias nativas. Por ejemplo, no se prometerá soporte para macOS moderno de 32 bits.

---

## 21.2 Target WebAssembly

Posible:

```bash
zirk build --target wasm32-browser
```

**Pregunta:** ¿El lenguaje soportará web directamente mediante Wasm?

**Respuesta:** Pendiente.

Por ahora, el lenguaje no tendrá soporte para WebAssembly.

La primera implementación se enfocará exclusivamente en generar
binarios nativos para sistemas operativos y arquitecturas compatibles.

El soporte para WebAssembly podrá evaluarse e incorporarse
posteriormente como un target adicional.

---

## 21.3 Directivas runtime

Concepto previo:

```text
@runtime JS_ENV;
```

Posible rediseño:

```text
@target WASM;
@host BROWSER;
```

o:

```text
@runtime WEB;
```

**Pregunta:** ¿Qué semántica exacta tendrá esta directiva?

**Respuesta:** Pendiente.

Por ahora, no existirán directivas como:

@runtime
@target
@host

El compilador y el runtime determinarán automáticamente el
comportamiento predeterminado.

Los targets se seleccionarán mediante la CLI o el bloque `build_targets` de
`init.zrk`, sin incluir directivas de runtime dentro del código.

Este mecanismo podrá evaluarse posteriormente si llega a ser
necesario.

---

## 21.4 Código específico por plataforma

Posible:

```text
@platform(macos)
fn openNativeWindow(): Void {}

@platform(browser)
fn openNativeWindow(): Void {}
```

**Pregunta:** ¿Cómo se controla código por plataforma?

**Respuesta:** Pendiente.

Por ahora, el lenguaje no incluirá anotaciones ni sintaxis especial
para declarar código específico por plataforma.

No se implementarán inicialmente construcciones como:

@platform(macos)
@platform(linux)
@platform(windows)

El diseño de este mecanismo se evaluará posteriormente cuando exista
una necesidad concreta de mantener implementaciones diferentes para
cada plataforma.

---

## 21.5 Compilación cruzada

**Pregunta:** ¿Desde macOS podrá compilar directamente para Linux y Windows?

**Go:**

```bash
GOOS=linux GOARCH=amd64 go build
```

Posible:

```bash
zirk build --target x86_64-linux
```

**Respuesta:** Sí.

El compilador permitirá generar binarios para un sistema operativo o
arquitectura diferente de aquellos donde se ejecuta.

Ejemplo:

```bash
zirk build --target x86_64-linux
zirk build --target x86_64-windows
zirk build --target aarch64-macos
```

Si no se especifica `--target`, se compilarán todos los destinos de `build_targets`. Si ese bloque tampoco existe, el compilador utilizará automáticamente el sistema operativo y la arquitectura actuales.

La compilación cruzada estará disponible siempre que el backend, el
linker y las dependencias nativas del proyecto sean compatibles con el
target seleccionado.

Si alguna dependencia no puede compilarse para el target solicitado,
el compilador mostrará un error indicando cuál es incompatible.

---

# 22. Runtime y biblioteca estándar

## 22.1 Runtime mínimo

**Pregunta:** ¿El ejecutable debe ser completamente standalone?

Ejemplo conceptual:

```bash
./app
```

sin necesitar:

```bash
node
python
java
```

**Respuesta:** Sí.

El compilador generará ejecutables standalone que podrán ejecutarse
directamente en el sistema operativo objetivo.

Ejemplo:

zirk build
./app

El usuario final no necesitará instalar:

- el compilador del lenguaje;
- una máquina virtual;
- un intérprete;
- Node.js;
- Python;
- Java;
- un runtime distribuido por separado.

Las partes necesarias del runtime del lenguaje, como la administración
automática de memoria, el scheduler de tasks, los channels y el manejo
de errores, se incorporarán directamente en el ejecutable.

El compilador incluirá únicamente los componentes del runtime y de la
biblioteca estándar que utilice el programa, cuando sea posible, para
reducir el tamaño del binario.

El ejecutable podrá depender de las APIs y librerías básicas del sistema
operativo objetivo, además de las librerías dinámicas que el proyecto
declare explícitamente mediante FFI.

---

## 22.2 Biblioteca estándar

¿Qué módulos existirán?

Posibles:

```text
std.io
std.fs
std.net
std.http
std.json
std.crypto
std.time
std.thread
std.sync
std.collections
std.testing
```

**Respuesta:** La biblioteca estándar se dividirá en dos niveles:

1. `core`
2. `std`

`core` contendrá los tipos y mecanismos fundamentales, disponibles
automáticamente sin necesidad de utilizar `import`.

Inicialmente incluirá:

- Object
- String
- Char
- Boolean
- Int8, Int16, Int32, Int64 e Int128
- UInt8, UInt16, UInt32, UInt64 y UInt128
- Float16, Float32, Float64 y Float128
- Array<T>
- List<T>
- Map<K, V>
- Set<T>
- Option<T>
- Result<T, E>
- Task<T>
- Channel<T>
- Atomic<T>

Ejemplo:

mut users: List<User> = List();
mut operation: Task<User> = task load_user();

Los módulos de `std` deberán incorporarse explícitamente mediante
`import`.

La primera versión de la biblioteca estándar incluirá:

- std.io
- std.fs
- std.path
- std.time
- std.collections
- std.task
- std.thread
- std.sync
- std.testing

Los módulos completos se importarán mediante su namespace, sin
comillas:

import std.io;
import std.fs;

Sus elementos se utilizarán mediante el nombre del módulo:

io.println("Hola");

mut content = fs.read_text("data.txt");

También podrán importarse elementos específicos mediante
desestructuración:

import { print, read_line } from std.io;
import { read_text, write_text } from std.fs;
import { DateTime, Duration } from std.time;

Uso:

print("Nombre: ");

mut name: String = read_line();

match read_text("data.txt") {
    Ok(content) => print(content);
    Error(error) => print(error);
}

Los aliases utilizarán `->`:

import {
    print -> write,
    read_line -> input
} from std.io;

Uso:

write("Nombre: ");

mut name: String = input();

Las rutas de la biblioteca estándar se escribirán sin comillas:

import std.io;
import { Queue } from std.collections;

Las rutas de archivos del proyecto o de dependencias externas se
escribirán entre comillas y sin la extensión `.zrk`:

import { User } from "users/user";
import { Database } from "packages/database";

Solo podrán importarse símbolos expuestos mediante `share`.

Los módulos iniciales tendrán estas responsabilidades:

std.io
→ entrada y salida básica.

Ejemplos:

import std.io;

io.print("Hola");
io.println("Mundo");

mut line: String = io.read_line();

std.fs
→ archivos y directorios.

Ejemplos:

import { read_text, write_text, exists } from std.fs;

if exists("config.txt") {
    match read_text("config.txt") {
        Ok(content) => print(content);
        Error(error) => print(error);
    }
}

write_text("output.txt", "Contenido");

std.path
→ manipulación portable de rutas.

Ej

---

# 23. Entrada y salida

## 23.1 Print

**TypeScript:**

```ts
console.log("Hola");
```

**Python:**

```python
print("Hola")
```

Posible:

```text
print("Hola");
```

¿`print` es función global o método?

**Respuesta:** `print` no será una función global.

Las operaciones de salida pertenecerán al módulo `std.io`, que expondrá
dos streams:

- `stdout`: salida estándar.
- `stderr`: salida de errores y diagnósticos.

Se importarán explícitamente:

import { stdout, stderr } from std.io;

Salida normal:

stdout.print("Hola");
stdout.println("Mundo");

`stdout.print(value)` escribirá el valor sin agregar un salto de línea.

`stdout.println(value)` escribirá el valor y agregará el salto de línea
correspondiente a la plataforma.

`stdout.println()` sin argumentos escribirá únicamente un salto de
línea.

`stderr` ofrecerá los mismos métodos de escritura, pero enviará el
contenido al stream de errores del sistema operativo:

stderr.print("Advertencia: ");
stderr.println("Archivo inválido");

Esto permitirá separar la salida normal de los errores y redirigir
ambos streams de manera independiente.

`print` y `println` aceptarán cualquier valor que pueda representarse
mediante:

to_string(): String

Ejemplos:

stdout.println(42);
stdout.println(true);
stdout.println(user);

Para mostrar varios valores se favorecerá la interpolación:

stdout.println("Usuario: {user.name}, edad: {user.age}");

El formato visual será independiente del stream utilizado. `stdout` y
`stderr` determinarán el destino del contenido; las opciones de formato
determinarán su representación.

---

## 23.2 stdin

**Python:**

```python
name = input("Nombre: ")
```

Posible:

```text
mut name: String = input("Nombre: ");
```

**Respuesta:** La entrada estándar pertenecerá al módulo `std.io` y se expondrá
mediante el objeto `stdin`.

No existirá una función global `input`.

`stdin` se importará explícitamente:

import { stdin } from std.io;

Para leer una línea se manejarán explícitamente el dato, el final del stream y los errores:

```text
match stdin.read_line() {
    Data(name) => stdout.println("Hola {name}");
    End => stdout.println("Fin de la entrada");
    Error(error) => stderr.println(error);
}
```

Para mostrar una pregunta antes de leer, se utilizará `stdout`:

import { stdin, stdout } from std.io;

stdout.print("Nombre: ");

match stdin.read_line() {
    Data(name) => stdout.println("Hola {name}");
    End => stdout.println("No se ingresó un nombre");
    Error(error) => stderr.println(error);
}

`stdin` ofrecerá inicialmente operaciones como:

stdin.read_line();
stdin.read_char();
stdin.read_bytes();

`read_line()` leerá hasta encontrar un salto de línea y retornará el
texto sin incluir ese salto.

`read_char()` leerá un `Char` completo según la definición del lenguaje,
es decir, un grapheme.

`read_bytes()` permitirá obtener datos binarios para operaciones de
más bajo nivel.

Las operaciones de lectura retornarán un enum algebraico explícito:

```text
enum ReadResult<T, E> {
    Data(T);
    End;
    Error(E);
}
```

Sus firmas conceptuales serán:

```text
stdin.read_line(): ReadResult<String, InputError>
stdin.read_char(): ReadResult<Char, InputError>
stdin.read_bytes(size: UInt64): ReadResult<Bytes, InputError>
```

`Data(value)` indicará una lectura correcta, `End` el final normal del stream y `Error(error)` un fallo real. EOF no será un error ni se representará mediante `null`.

Al leer bytes, solicitar una cantidad determinada podrá retornar `Data(bytes)` con menos elementos si se leyó al menos uno. `End` solo se producirá cuando el stream haya terminado sin entregar ningún byte adicional.

Los errores iniciales podrán incluir stream cerrado, interrupción, encoding inválido, límite excedido y error del sistema operativo:

```text
enum InputError {
    Closed;
    Interrupted;
    InvalidEncoding;
    LimitExceeded;
    System(SystemError);
}
```

La cancelación y el timeout provenientes de una task continuarán representándose mediante `TaskCancelledException` y `TaskTimeoutException`, sin duplicarse dentro de cada tipo de error de I/O.

---

# 24. Archivos y sistema operativo

## 24.1 File API

**Pregunta:** ¿Cómo se leen archivos?

**Python:**

```python
with open("file.txt") as f:
    content = f.read()
```

Posible:

```text
inmut file = File.open("file.txt");
inmut content = file.readText();
```

**Respuesta:**

La API de archivos pertenecerá a `std.fs` y ofrecerá dos niveles.

Para operaciones pequeñas y completas se utilizarán funciones del módulo:

```text
import std.fs;

match fs.read_text("file.txt") {
    Ok(content) => {
        stdout.println(content);
    }

    Error(error) => {
        stderr.println(error);
    }
};
```

Inicialmente existirán operaciones como:

```text
fs.read_text(path);
fs.write_text(path, content);
fs.read_bytes(path);
fs.write_bytes(path, content);
fs.exists(path);
fs.create_directory(path);
fs.remove(path);
```

Estas funciones retornarán `Result<T, FileError>` cuando puedan fallar.

Para streaming, archivos grandes o control explícito se utilizará `File` junto con `match with`:

```text
import { File, FileMode } from std.fs;

mut first_line: Result<String, FileError> =
    match with File.open("file.txt", mode: FileMode.Read) {
        Ok(file) => file.read_line();
        Error(error) => Error(error);
    };
```

Como `match with` aparece dentro de una declaración, todas sus ramas deberán producir un valor compatible. Antes de asignarlo a `first_line`, el archivo se cerrará automáticamente.

También podrá utilizarse como sentencia cuando solo se necesite ejecutar acciones:

```text
match with File.open("file.txt", mode: FileMode.Read) {
    Ok(file) => {
        match file.read_text() {
            Ok(content) => stdout.println(content);
            Error(error) => stderr.println(error);
        };
    }

    Error(error) => {
        stderr.println(error);
    }
};
```

En ese caso no se produce ningún valor al exterior.

El archivo se cerrará al abandonar la rama `Ok`, incluso mediante `return`, `break`, `continue`, una exception o una cancelación cooperativa. El recurso no podrá escapar de esa rama.

El identificador de `Ok(...)` será directamente el nombre local elegido por el desarrollador:

```text
Ok(file)
Ok(config_file)
Ok(input_file)
```

No se utilizará `->` dentro de `Ok`, porque el patrón declara una variable nueva y no crea un alias de un símbolo existente.

Para conservar, retornar o transferir el archivo se utilizará `match` sin `with`. En ese caso, quien reciba el recurso será responsable de cerrarlo:

```text
mut opened_file: Result<File, FileError> =
    match File.open("file.txt") {
        Ok(file) => Ok(file);
        Error(error) => Error(error);
    };
```

`File` ofrecerá operaciones como:

```text
file.read_text();
file.read_bytes();
file.read_line();
file.write_text(content);
file.write_bytes(content);
file.flush();
file.close();
```

La administración automática de memoria no sustituirá el cierre determinista de recursos del sistema operativo.

---

## 24.2 Paths

Posible:

```text
inmut path = Path("./data/file.txt");
```

**Respuesta:**

Existirá una clase `Path` dentro de `std.path` para representar rutas de manera portable.

```text
import std.path;

inmut FILE_PATH: Path = Path("./data/file.txt");
```

También podrán construirse rutas mediante el namespace del módulo:

```text
inmut FILE_PATH: Path = path.join("data", "file.txt");
```

`Path` ofrecerá operaciones que solo transforman o consultan la representación de la ruta:

```text
FILE_PATH.name();
FILE_PATH.extension();
FILE_PATH.parent();
FILE_PATH.is_absolute();
FILE_PATH.normalize();
FILE_PATH.join("other.txt");
FILE_PATH.to_string();
```

`Path` no accederá por sí mismo al sistema de archivos. Las consultas y modificaciones reales pertenecerán a `std.fs`:

```text
import std.fs;
import std.path;

inmut FILE_PATH: Path = path.join("data", "file.txt");

if fs.exists(FILE_PATH) {
    match fs.read_text(FILE_PATH) {
        Ok(content) => stdout.println(content);
        Error(error) => stderr.println(error);
    }
}
```

Las funciones de conveniencia de `std.fs` aceptarán `Path` y podrán aceptar `String` cuando la conversión sea inequívoca. Las APIs de control avanzado favorecerán `Path` explícito.

---

## 24.3 Procesos

**Pregunta:** ¿Puede lanzar procesos externos?

**Python:**

```python
import subprocess
subprocess.run(["git", "status"])
```

Posible:

```text
Process.run("git", ["status"]);
```

**Respuesta:**

Sí. La ejecución de procesos externos pertenecerá a `std.process`.

Para ejecutar un comando y esperar su finalización se utilizará `process.run()`:

```text
import std.process;

match process.run("git", ["status"]) {
    Ok(output) => {
        stdout.print(output.stdout);

        if !output.stderr.is_empty() {
            stderr.print(output.stderr);
        }
    }

    Error(error) => {
        stderr.println(error);
    }
};
```

El programa y sus argumentos se proporcionarán por separado. `run()` ejecutará directamente el programa sin utilizar una shell, por lo que no interpretará automáticamente operadores como `|`, `>`, `&&` o `*`.

`ProcessOutput` contendrá al menos el código de salida y el contenido de `stdout` y `stderr`.

Para procesos de larga duración o control avanzado se utilizará `Process.spawn()`, que retornará un recurso administrable. `match with` podrá entregar el resultado de la operación sin introducir bloques posteriores:

```text
import { Process } from std.process;

mut exit_code: Result<ExitCode, ProcessError> =
    match with Process.spawn("git", ["status"]) {
        Ok(child_process) => child_process.wait();
        Error(error) => Error(error);
    };
```

Antes de asignar `exit_code`, `match with` cerrará automáticamente los pipes y handles administrados por `child_process`.

Cerrar el handle no matará silenciosamente el proceso externo. El desarrollador expresará la acción mediante:

```text
child_process.wait();
child_process.cancel();
child_process.kill();
```

`cancel()` solicitará una terminación controlada. `kill()` forzará la terminación mediante las capacidades del sistema operativo y será una operación explícita.

Para conservar o transferir el handle se utilizará `match` sin `with`; el nuevo responsable deberá cerrarlo posteriormente.

La ejecución mediante una shell será una API separada y explícita:

```text
process.shell("git status | grep modified");
```

Los errores de creación o ejecución se representarán mediante `Result<T, ProcessError>`.

---

# 25. Metaprogramación y anotaciones

## 25.1 Decoradores/anotaciones

**TypeScript:**

```ts
@Controller()
class UserController {}
```

Posible:

```text
@controller
class UserController {}
```

**Pregunta:** ¿Las anotaciones solo agregan metadata o pueden transformar código?

**Respuesta:**

Los decoradores serán una característica nativa del lenguaje y podrán agregar metadata o realizar transformaciones controladas durante la compilación.

Se declararán mediante la forma especial `fn dec`. La cabecera contendrá exclusivamente los parámetros de configuración proporcionados al utilizar el decorador:

```text
fn dec route(path: String) {
    class(target, context) {
        target.metadata.set("route", path);
    }

    method(target, context) {
        target.metadata.set("route", path);
        target.wrap(...);
    }
}
```

Dentro del decorador se incluirá un bloque por cada clase de elemento que
soporte. Los únicos targets de Zirk 1.x son `class`, `attribute`, `function`,
`method` y `parameter`; este último incluye parámetros de inicialización.
`constructor`, `property` y `accessor` no son targets. La construcción se
resuelve con parámetros, atributos o factories tipadas generadas por un
decorador de clase, y el acceso controlado usa métodos `get_`/`set_`.

```text
@route("/users")
class UserController {
    @route("/{id}")
    fn find_user(id: UInt64): User {
        ...
    }
}
```

El compilador seleccionará estáticamente el bloque correspondiente. Aplicar un decorador a una clase de elemento que este no soporte producirá un error de compilación.

Cada bloque recibe un `target` tipado e inmutable. Los demás valores del
compilador nunca son implícitos: se introducen en los payloads de
`Inspect(context)`, `Augment(builder)` y `Wrap(wrapper)`. En decoradores
`repeatable`, cada fase que lo necesite enlaza explícitamente `applications` y
cada elemento se consulta como `application.arguments.<nombre>`.

La expansión sigue `Inspect -> Augment -> Wrap`. Dentro de `Wrap`,
`match target.wrap` ofrece `Before`, `After`, `Catch` o `Around`. El orden
visible compone desde el decorador superior hacia el inferior; `requires`,
`before` y `after` validan ese orden sin modificarlo. Autorreferencias y ciclos
son errores.

Cada bloque recibe valores fijos proporcionados por el compilador:

- `target`: representación tipada del elemento decorado y API de inspección o transformación permitida.
- `context`: propietario, ubicación, metadata y mecanismos para emitir diagnósticos.

Las transformaciones se realizarán mediante operaciones explícitas y verificables, como agregar metadata, envolver o reemplazar una implementación, añadir un inicializador o añadir miembros permitidos. El compilador validará el resultado y rechazará transformaciones que incumplan el sistema de tipos o el contrato del elemento.

Los decoradores incluidos en paquetes deberán aparecer en su API pública con los tipos de elementos que soportan. Sus efectos y permisos de compilación también deberán declararse en el manifiesto del paquete.

La descripción pública de un decorador podrá conservar solo su contrato:

```text
share fn dec route(path: String) {
    class(target, context);
    method(target, context);
}
```

Su implementación permanecerá en la representación intermedia portable del paquete.

---

## 25.2 Reflection

**TypeScript:**

```ts
typeof value;
```

Posible:

```text
user.type();
User.fields();
```

**Pregunta:** ¿Existe reflection en runtime?

**Respuesta actualizada:**

La identidad básica de tipos puede existir según el contrato general de tipos,
pero los decoradores no se conservan automáticamente en runtime. Zirk 1.x no
incluye `runtime fn dec` ni `Reflection.decorators(...)`.

Todos los valores permitirán consultar la identidad básica de su tipo:

```text
mut type: Type = user.type();

stdout.println(type.name);
stdout.println(type.is(User));
```

Cuando una librería necesite información estructural en runtime, su decorador
genera explícitamente un descriptor o registry ordinario y tipado:

```text
@Serializable()
class User {
    mut name: String;
    mut age: UInt8;
}
```

```text
for field in User::descriptor().fields {
    stdout.println(field.name);
}
```

El descriptor contiene solo la estructura requerida por la librería, obedece
visibilidad y mutabilidad, participa en `public.api` si es público y puede ser
eliminado si es privado y no se usa. La identidad y los argumentos del
decorador desaparecen después de la expansión.

---

## 25.3 Compile-time reflection

**Pregunta:** ¿El compilador puede inspeccionar clases durante compilación?

Posible:

```text
comptime {
    for field in User.fields {
        ...
    }
}
```

**Respuesta:**

Sí. El compilador podrá inspeccionar declaraciones durante la compilación, inicialmente a través de los bloques tipados de `fn dec` y no mediante bloques `comptime` arbitrarios.

```text
fn dec serializable() {
    class(target, context) {
        for field in target.fields() {
            ...
        }
    }
}
```

Un decorador podrá inspeccionar nombres, tipos, campos, métodos, parámetros, retornos, visibilidad, mutabilidad, generics, traits, interfaces, decoradores y metadata.

Las transformaciones se realizarán exclusivamente mediante la API controlada de `target`. No se expondrá acceso irrestricto a la AST interna del compilador.

No existirá un bloque general `comptime {}` en la primera versión.

La metaprogramación durante compilación se resolverá mediante `fn dec`, constant folding y herramientas explícitas como `zirk prepare`. Esto conservará builds deterministas, rápidos, auditables y cacheables, evitando que código arbitrario acceda silenciosamente a red, reloj o filesystem durante cada compilación.

Si posteriormente se incorpora ejecución general en compile time, deberá ejecutarse en un sandbox con inputs, outputs y permisos declarados; sin red por defecto; con límites de tiempo y memoria; y con resultados direccionados por hash y reutilizables desde cache.

---

# 26. Enums y tipos algebraicos

## 26.1 Enum tradicional

**TypeScript:**

```ts
enum Status {
    Active,
    Inactive
}
```

Posible:

```text
enum Status {
    Active;
    Inactive;
}
```

**Respuesta:**

Sí. Los enums serán tipos nominales y sus miembros no se convertirán automáticamente en enteros.

```text
enum Status {
    Active;
    Inactive;
    Suspended;
}

mut status: Status = Status.Active;
```

`match` verificará que se cubran todas las variantes:

```text
match status {
    Status.Active => "Activo";
    Status.Inactive => "Inactivo";
    Status.Suspended => "Suspendido";
}
```

No existirán valores numéricos implícitos. Cuando se necesite asociar un código o valor, deberá declararse explícitamente.

Sin mapping explícito, el valor string observable de una variante será
exactamente su nombre (`Status.Active` → `"Active"`). Un mapping se declarará
con `->`, por ejemplo `North -> "N"` o `Success -> 0`; la variante conservará
su tipo enum y todos los mappings del enum tendrán tipos compatibles.

---

## 26.2 Enum con valores asociados

TypeScript no tiene ADTs reales de forma nativa, pero puede modelarlos con unions.

```ts
type Result =
    | { type: "ok"; value: string }
    | { type: "error"; error: Error };
```

Posible:

```text
enum Result<T, E> {
    Ok(T);
    Error(E);
}
```

**Respuesta:**

Sí. Los enums podrán representar tipos algebraicos y aceptar generics:

```text
enum Result<T, E> {
    Ok(T);
    Error(E);
}
```

Sus variantes podrán contener cero, uno o varios valores:

```text
enum Message {
    Quit;
    Text(String);
    Move(Int32, Int32);
}
```

Los valores asociados se extraerán mediante `match`:

```text
match load_user() {
    Ok(user) => stdout.println(user.name);
    Error(error) => stderr.println(error);
}
```

Este será el mismo mecanismo utilizado por `Result`, estados, eventos y mensajes enviados mediante channels.

---

# 27. Type aliases y unions

## 27.1 Alias

**TypeScript:**

```ts
type UserId = number;
```

Posible:

```text
type UserId = UInt64;
```

**Pregunta:** ¿Alias es solo otro nombre o crea un tipo distinto?

**Respuesta:**

`type` creará un alias transparente y no un tipo nominal diferente:

```text
type UserId = UInt64;

mut id: UserId = UInt64(10);
```

`UserId` y `UInt64` serán el mismo tipo para el compilador.

Esto no contradice el uso de `->`: la flecha renombra símbolos durante imports o destructuración, mientras que `type` declara un nombre alternativo para un tipo.

Cuando se necesite crear un tipo nominal realmente distinto deberá utilizarse un `record` u otra construcción nominal, no un alias.

---

## 27.2 Union types

**TypeScript:**

```ts
let id: string | number;
```

Posible:

```text
mut id: String | UInt64;
```

**Respuesta:**

Sí. Existirán union types:

```text
mut identifier: String | UInt64;
```

Un valor podrá asignarse si su tipo pertenece a la unión. Antes de realizar una operación exclusiva de uno de sus miembros, el programa deberá estrechar el tipo mediante `match` u otra comprobación reconocida por el compilador.

```text
match identifier {
    String(value) => stdout.println(value);
    UInt64(value) => stdout.println(value);
}
```

El orden de los miembros no producirá una unión diferente, los tipos repetidos se eliminarán y no habrá coerción automática entre los miembros. Las unions podrán utilizarse en variables, parámetros, retornos y APIs públicas.

`T?` será la forma abreviada de `T | null`.

---

# 28. Casts y conversiones

## 28.1 Conversión segura

**TypeScript:**

```ts
const value = Number(text);
```

Posible:

```text
mut value: Int32 = Int32.parse(text);
```

**Respuesta:**

Las conversiones reales que puedan fallar retornarán `Result`:

```text
mut parsed: Result<Int32, ParseError> = Int32.parse(text);
mut small: Result<Int8, RangeError> = Int8.from(number);
```

Un texto inválido o un número fuera de rango serán errores esperables y no lanzarán una exception.

Las conversiones garantizadas podrán expresarse mediante un constructor o un método explícito:

```text
mut text: String = value.to_string();
mut decimal: Float64 = Float64(count);
```

Un constructor explícito aplicado a un árbol de operadores establecerá el
dominio contextual antes de ejecutar esos operadores:

```text
Float(3 / 4)                 // 0.75, no 0.0
String("value=" + 42)       // "value=42"
Float((a + 1) / (b * 2))
```

El contexto atravesará solo el árbol aritmético o de concatenación contenido;
no modificará operandos ni entrará al cuerpo de funciones llamadas.

---

## 28.2 Cast explícito

**C:**

```c
double value = (double)count;
```

Posible:

```text
mut value: Float64 = count as Float64;
```

**Respuesta:**

Existirán dos sintaxis equivalentes para un cast explícito:

```text
value as Type
<Type>value
```

Ejemplos:

```text
mut decimal = count as Float64;
mut decimal = <Float64>count;
```

La forma prefija aplicará el cast a la expresión que aparezca a continuación. Los paréntesis delimitarán expresiones compuestas y permitirán continuar accediendo al resultado casteado:

```text
<CustomObject>(obj.field).field;
```

Esta expresión primero evaluará `obj.field`, luego casteará ese resultado a `CustomObject` y finalmente accederá a `.field` sobre el valor casteado.

Ambas formas tendrán exactamente las mismas reglas y solo se permitirán para casts verificables, como ampliaciones numéricas sin pérdida y upcasting entre clases.

Un cast no realizará una conversión real del valor. Estas operaciones permanecerán diferenciadas:

```text
value as Type           // cast verificable
<Type>value             // cast verificable equivalente
Type(value)             // conversión real garantizada
value.to_string()       // método de conversión
Type.from(value)        // conversión fallable mediante Result
```

No se permitirá utilizar `as` ni `<Type>` para esconder pérdida de información o una operación que pueda fallar. Por ejemplo, convertir un entero grande a `Int8` deberá realizarse mediante `Int8.from(value)` y retornar `Result<Int8, RangeError>`.

---

## 28.3 Cast inseguro

**C:**

```c
void *ptr;
int *value = (int *)ptr;
```

**Pregunta:** ¿Requiere `unsafe`?

**Respuesta:**

Sí. Las reinterpretaciones físicas de memoria requerirán `unsafe` y una operación explícita distinta de los casts normales:

```text
unsafe {
    mut value = reinterpret<Int32>(pointer);
}
```

Por tanto:

```text
as / <Type>
→ cast semántico verificable.

reinterpret<Type>()
→ reinterpretación física de memoria.
→ solo dentro de unsafe.
```

---

# 29. Lambdas y closures

## 29.1 Lambda

**TypeScript:**

```ts
const double = (x: number) => x * 2;
```

Posible:

```text
inmut double = (x: Int32): Int32 => x * 2;
```

**Respuesta:**

Sí. Las lambdas serán funciones anónimas que podrán almacenarse en variables, recibirse como parámetros o retornarse desde otras funciones. Serán el equivalente de `const func = () => {}` en TypeScript.

```text
inmut FUNC = (): Void => {
};

FUNC();
```

Podrán recibir parámetros:

```text
inmut GREET = (name: String): Void => {
    stdout.println("Hola {name}");
};

GREET("Cristian");
```

Y retornar valores:

```text
inmut DOUBLE = (value: Int32): Int32 => {
    return value * 2;
};

mut result: Int32 = DOUBLE(10);
```

Cuando el cuerpo contenga una sola expresión podrá abreviarse:

```text
inmut DOUBLE = (value: Int32): Int32 => value * 2;
```

El compilador podrá inferir los tipos de los parámetros o del retorno cuando el tipo esperado los haga inequívocos.

---

## 29.2 Captura de variables

**TypeScript:**

```ts
let count = 0;

const increment = () => {
    count++;
};
```

Preguntas:

- ¿Captura por referencia?
- ¿Por valor?
- ¿Depende de `mut/inmut`?
- ¿Cómo interactúa con threads?

**Respuesta:**

La captura dependerá de la mutabilidad del valor.

- Un valor `inmut` podrá capturarse para lectura.
- Un valor `mut` podrá capturarse y modificarse mientras su vida útil sea válida.
- Si la closure sobrevive al scope original, el runtime conservará automáticamente su estado capturado.
- Las capturas utilizadas entre tasks o threads deberán cumplir las reglas normales contra data races.

```text
mut count = 0;

inmut INCREMENT = (): Void => {
    count += 1;
};
```

Modificar una captura mutable sin protección desde `parallel` o `thread` producirá un error de compilación. Para compartir ese estado se utilizarán mecanismos como `Atomic<T>`, `sync` o channels:

```text
mut count: Atomic<UInt64> = Atomic(0);

parallel {
    count.increment();
}
```

No se introducirá inicialmente una palabra como `move` ni sintaxis manual para capturas. El compilador podrá decidir su representación interna sin comprometer la semántica pública.

---

# 30. Iteradores

## 30.1 Iterable

**TypeScript:**

```ts
for (const item of collection) {}
```

**Pregunta:** ¿Qué interfaz debe implementar una clase para poder usarse en `for`?

Posible:

```text
interface Iterable<T> {
    fn iterator(): Iterator<T>;
}
```

**Respuesta:**

Una clase utilizable mediante `for` deberá implementar `Iterable<T>`:

```text
interface Iterable<T> {
    fn iterator(): Iterator<T>;
}
```

El iterador utilizará un enum para distinguir un elemento del final de la secuencia, incluso cuando `T` pueda contener `null`:

```text
enum IteratorStep<T> {
    Item(T);
    End;
}

interface Iterator<T> {
    fn next(): IteratorStep<T>;
}
```

El ciclo:

```text
for user in users {
    stdout.println(user.name);
}
```

será azúcar sintáctica sobre `iterator()` y `next()`.

---

## 30.2 Generators

**TypeScript:**

```ts
function* numbers() {
    yield 1;
    yield 2;
}
```

Posible:

```text
generator fn numbers(): Int32 {
    yield 1;
    yield 2;
}
```

**Respuesta:**

Sí. Los generators permitirán producir secuencias de forma incremental mediante `yield`:

```text
generator fn numbers(): Int32 {
    yield 1;
    yield 2;
    yield 3;
}
```

El tipo declarado será el tipo de cada valor producido. Invocar la función retornará conceptualmente un `Generator<Int32>`, que implementará `Iterator<Int32>` e `Iterable<Int32>`:

```text
for number in numbers() {
    stdout.println(number);
}
```

El estado local permanecerá suspendido entre cada `yield` y se liberará automáticamente cuando el generator termine o sea abandonado.

---

# 31. Programación funcional

## 31.1 map/filter/reduce

**TypeScript:**

```ts
users
    .filter(user => user.active)
    .map(user => user.name);
```

**Pregunta:** ¿Las colecciones estándar incluyen estas operaciones?

**Respuesta:**

Sí. `map`, `filter` y `reduce` formarán parte de las operaciones de `Iterable<T>` y de las colecciones estándar.

```text
mut names = users
    .filter((user) => user.active)
    .map((user) => user.name)
    .to_list();
```

`map` y `filter` utilizarán evaluación lazy para evitar colecciones intermedias innecesarias. `reduce`, `count`, `find`, `to_list` y operaciones equivalentes serán terminales y ejecutarán la iteración.

Las colecciones concretas podrán proporcionar implementaciones optimizadas sin cambiar la semántica pública.

---

## 31.2 Pipe operator

**Pregunta:** ¿Existirá?

Posible:

```text
users
    |> filter(.active)
    |> map(.name);
```

**Respuesta:**

Existirá el pipe operator para ordenar llamadas a funciones puras sin anidarlas:

```text
users
    |> filter(is_active)
    |> map(to_name)
    |> reduce(join_names);
```

El encadenamiento de métodos seguirá disponible cuando la operación pertenezca
al contrato de la colección:

```text
users
    .filter((user) => user.active)
    .map((user) => user.name)
    .to_list();
```

El pipe permite reutilizar funciones independientes; el chaining llama métodos
del valor receptor. Ambos conservan los mismos tipos, errores y efectos.

---

# 32. Pattern matching

## 32.1 Match por valor

Posible:

```text
match status {
    200 => "OK";
    404 => "Not Found";
    _ => "Unknown";
}
```

**Respuesta:**

Sí. `match` permitirá comparar valores y agrupar varios patrones en una misma rama.

Como sentencia independiente solo controlará el flujo:

```text
match status {
    200 => stdout.println("OK");
    404 => stderr.println("Not Found");
    _ => stderr.println("Unknown");
};
```

Cuando aparezca en un contexto que requiera una expresión, producirá un valor:

```text
mut message: String = match status {
    200 => "OK";
    404 => "Not Found";
    _ => "Unknown";
};
```

Los contextos de valor incluirán declaraciones, asignaciones, retornos, argumentos e inicialización de propiedades.

---

## 32.2 Desestructuración en match

Posible:

```text
match result {
    Ok(value) => print(value);
    Error(error) => print(error.message);
}
```

**Respuesta:**

Sí. `match` permitirá desestructurar variantes como `Ok(value)` y `Error(error)`.

Como sentencia:

```text
match result {
    Ok(value) => stdout.println(value);
    Error(error) => stderr.println(error.message);
};
```

Como expresión:

```text
mut message: String = match result {
    Ok(value) => value.to_string();
    Error(error) => "Error: {error.message}";
};
```

Cuando una rama de un `match` usado como expresión contenga varias instrucciones, su última instrucción producirá el valor de la rama:

```text
mut message: String = match result {
    Ok(value) => {
        stdout.println("Valor recibido");
        value.to_string();
    }

    Error(error) => {
        stderr.println(error);
        "Sin valor";
    }
};
```

No existirán `capture` ni `yield`. `return` finalizará la función completa y no solo la rama.

---

## 32.3 Exhaustividad

**Pregunta:** ¿El compilador exige cubrir todos los casos?

**Respuesta:**

Cuando `match` se utilice como expresión, el compilador exigirá que sea exhaustivo y que todas las ramas produzcan valores compatibles con el tipo esperado.

```text
mut message: String = match status {
    200 => "OK";
    404 => "Not Found";
    _ => "Unknown";
};
```

Omitir `_` o cualquier variante posible será un error si el compilador no puede demostrar exhaustividad.

Cuando `match` se utilice como sentencia, también se exigirá exhaustividad para enums, `Result`, `Option` y otros tipos cerrados. Para dominios abiertos podrá utilizarse `_` como caso predeterminado.

En un `match` usado como sentencia, las ramas no tendrán que producir tipos compatibles porque sus resultados se descartarán.

---

# 33. Seguridad del lenguaje

## 33.1 Acceso fuera de rango

**Pregunta:** ¿Qué ocurre?

```text
array[100];
```

Alternativas:

- excepción
- Result
- panic
- unsafe
- undefined behavior

**Respuesta:**

El acceso normal mediante `[]` comprobará los límites. Un índice inválido lanzará una exception recuperable:

```text
mut item = values[100];
// IndexOutOfBoundsException
```

También existirá una operación explícita para tratarlo como un error esperado:

```text
mut item: Result<T, IndexError> = values.get(index);
```

El acceso fuera de rango nunca producirá comportamiento indefinido en código seguro.

---

## 33.2 Use-after-free

**Pregunta:** ¿El lenguaje debe impedirlo por diseño?

**C ejemplo del problema:**

```c
int *ptr = malloc(sizeof(int));
free(ptr);
printf("%d", *ptr);
```

**Respuesta:**

Sí. El lenguaje impedirá el use-after-free en código seguro.

La administración automática de memoria garantizará que los objetos normales no se liberen mientras sigan siendo accesibles. Las referencias seguras `ref T` tampoco podrán sobrevivir al valor referenciado.

Los recursos administrados mediante `match with` solo serán válidos dentro de la rama `Ok` correspondiente:

```text
match with File.open("file.txt") {
    Ok(file) => {
        file.read_text(); // Válido.
    }

    Error(error) => {
    }
}

file.read_text(); // Error: `file` no existe fuera del scope.
```

Tampoco se permitirá que el recurso escape del scope administrado:

```text
match with File.open("file.txt") {
    Ok(file) => {
        return Ok(file); // Error de compilación.
    }

    Error(error) => {
        return Error(error);
    }
}
```

Para transferir deliberadamente un recurso se utilizará `match` sin `with`, haciendo explícita la responsabilidad de su nuevo propietario.

El código dentro de `unsafe` podrá realizar operaciones que el compilador no pueda verificar. Los posibles errores derivados de punteros crudos serán responsabilidad explícita del desarrollador.

---

## 33.3 Null dereference

**Pregunta:** ¿Puede existir?

**Respuesta:**

No podrá ocurrir accidentalmente en código seguro. El compilador impedirá acceder directamente a miembros de un tipo nullable:

```text
mut user: User? = null;

user.name;  // Error de compilación.
user?.name; // Válido.
```

El valor deberá validarse mediante `match`, una comparación reconocida por el compilador u optional chaining antes de acceder a sus miembros.

Inicialmente no existirá un operador para forzar el desempaquetado de un nullable.

---

## 33.4 Undefined behavior

**Pregunta:** ¿El lenguaje permite comportamiento indefinido?

**Respuesta:**

El código seguro no tendrá comportamiento indefinido.

Una operación segura deberá producir un resultado válido, retornar `Result`, lanzar una exception recuperable o finalizar mediante `fatalError` cuando se detecte una condición irreparable.

Solo determinadas operaciones dentro de `unsafe` podrán quedar sujetas a comportamiento indefinido si el desarrollador incumple expresamente su contrato. Siempre que sea viable, incluso una operación insegura deberá detectar el fallo y detenerse, pero esto no podrá garantizarse para punteros crudos, FFI o assembly.

---

# 34. Unsafe

## 34.1 Bloques unsafe

Posible:

```text
unsafe {
    mut ptr = &value;
    *ptr = 20;
}
```

**Pregunta:** ¿Qué operaciones exigen unsafe?

- punteros
- FFI
- casts de memoria
- assembly
- memoria manual
- acceso a hardware

**Respuesta:**

`unsafe` habilitará exclusivamente operaciones que el compilador no pueda verificar.

Requerirán un bloque `unsafe`:

- crear, convertir, operar o desreferenciar punteros crudos;
- reservar y liberar memoria manualmente;
- reinterpretar memoria;
- llamar directamente funciones FFI no marcadas como seguras;
- ejecutar assembly;
- acceder a hardware o memoria mapeada;
- construir valores ignorando invariantes verificables.

```text
unsafe {
    mut pointer = &value;
    *pointer = 20;
}
```

`unsafe` no desactivará el sistema de tipos completo, no ignorará la privacidad, no concederá permisos del sistema operativo y no permitirá data races sobre variables normales.

Como los punteros crudos solo existirán dentro de `unsafe`, no podrán retornarse, almacenarse en globals ni capturarse en una task que sobreviva al bloque:

```text
mut pointer = unsafe {
    return &value;
}; // Error de compilación.
```

Las APIs públicas deberán encapsular las operaciones inseguras y exponer un contrato seguro siempre que sea posible. Una función que no pueda garantizar dicho contrato deberá permanecer marcada como insegura para sus llamadores.

---

# 35. FFI e interoperabilidad nativa

## 35.1 Llamar C

**C es el ABI más común para interoperabilidad.**

Posible:

```text
extern "C" fn printf(format: Pointer<Char>, ...): Int32;
```

**Pregunta:** ¿Se podrá consumir bibliotecas C?

**Respuesta:**

Sí. El ABI de C será la frontera oficial para interoperabilidad nativa.

```text
extern "C" {
    fn calculate(value: Int32): Int32;
}
```

Las llamadas directas a una función nativa no declarada como segura requerirán `unsafe`:

```text
unsafe {
    mut result = calculate(10);
}
```

Esto permitirá consumir directamente bibliotecas C. Las bibliotecas C++ deberán exponer una interfaz C estable mediante `extern "C"`, porque la ABI de C++, su name mangling, sus clases, templates y exceptions varían entre compiladores y plataformas.

Las bibliotecas Rust utilizarán igualmente funciones `extern "C"` y representaciones compatibles, porque la ABI nativa de Rust no es estable. Por tanto, C++ y Rust serán interoperables mediante adaptadores C sin necesitar una ABI especial para cada lenguaje.

Solo cruzarán directamente la frontera tipos con representación compatible: enteros y floats de tamaño explícito, punteros, estructuras compatibles y handles opacos. Strings, clases, colecciones y exceptions requerirán adaptadores.

FFI requerirá el permiso correspondiente. El tooling podrá incorporar posteriormente generación de bindings desde headers C.

---

## 35.2 Exportar funciones

Posible:

```text
@export("calculate")
fn calculate(a: Int32, b: Int32): Int32 {
    return a + b;
}
```

**Respuesta:**

Sí. El lenguaje podrá exponer funciones mediante el ABI de C para que sean consumidas por C, C++, Rust y otros lenguajes compatibles.

```text
share extern "C" fn calculate(a: Int32, b: Int32): Int32 {
    return a + b;
}
```

La API exportada solo podrá utilizar tipos con representación ABI estable. Las exceptions no podrán atravesar directamente la frontera: deberán capturarse y transformarse en códigos de error, `Result` adaptados o estructuras compatibles.

El compilador permitirá indicar un nombre binario estable cuando sea necesario y generará headers C para las funciones exportadas.

---

## 35.3 Librerías dinámicas

**Pregunta:** ¿Puede compilar `.dll`, `.so`, `.dylib`?

**Respuesta:**

Sí. Un proyecto `library` podrá generar bibliotecas dinámicas `.dll`, `.so` y `.dylib`, además del paquete portable `.zpkg`.

El formato nativo se seleccionará según el target:

```text
x86_64-windows → .dll
x86_64-linux   → .so
aarch64-macos  → .dylib
```

Los paquetes que incluyan componentes nativos declararán los targets disponibles. Si una aplicación solicita un target sin artefacto compatible y el componente no puede recompilarse desde IR portable, el build fallará identificando la dependencia.

La carga dinámica será explícita, requerirá permisos y utilizará handles administrados como recursos. Las APIs de alto nivel deberán envolver el acceso inseguro y validar símbolos, versiones y errores de carga.

---

# 36. Web y navegador

## 36.1 Target browser

Posible:

```text
@runtime WEB;
```

o:

```text
@target WASM;
@host BROWSER;
```

**Pregunta:** ¿Cómo quieres representarlo sintácticamente?

**Respuesta:**

No formará parte de la primera versión ni del núcleo nativo del lenguaje.

La implementación inicial se enfocará en aplicaciones y librerías nativas. La integración con navegadores podrá abordarse posteriormente mediante una librería o herramienta externa, sin incorporar al lenguaje un target browser, APIs web ni una semántica especial dependiente del navegador.

---

## 36.2 DOM

TypeScript:

```ts
document.querySelector("#title")!.textContent = "Hola";
```

Posible lenguaje:

```text
import web.dom;

document.query("#title").text = "Hola";
```

**Pregunta:** ¿El DOM será parte de una biblioteca estándar `web`?

**Respuesta:**

El núcleo y la biblioteca estándar inicial no ofrecerán acceso nativo al DOM.

Si en el futuro existe soporte para navegador, la API del DOM pertenecerá a una librería separada que adapte las capacidades del host. Esto evitará mezclar conceptos específicos de una plataforma con la semántica general del lenguaje.

---

## 36.3 Eventos

**TypeScript:**

```ts
button.addEventListener("click", () => {});
```

Posible:

```text
button.on("click", fn(event) {
});
```

**Respuesta:**

Los eventos del navegador no formarán parte de la primera versión ni de la biblioteca estándar general.

Una futura librería de integración web podrá definir listeners, tipos de eventos, cancelación y vida útil de callbacks utilizando las funciones, lambdas y tasks normales del lenguaje.

---

## 36.4 Fetch

**TypeScript:**

```ts
const response = await fetch("/api/users");
```

Posible:

```text
mut response = await Http.get("/api/users");
```

**Respuesta:**

No existirá un `fetch` nativo específico del navegador en la primera versión.

Las aplicaciones nativas utilizarán los módulos de red y HTTP de la biblioteca estándar. Una futura librería web podrá adaptar la API del host y exponerla mediante los mismos tipos de `Result`, tasks, cancelación y permisos del lenguaje cuando sea técnicamente posible.

---

# 37. Entry point

## 37.1 Main

**C:**

```c
int main(int argc, char **argv) {
    return 0;
}
```

Posible:

```text
fn main(args: Array<String>): ExitCode {
    return ExitCode.success;
}
```

**Pregunta:** ¿Será obligatorio `main`?

**Respuesta:**

Las aplicaciones tendrán un archivo de entrada declarado explícitamente en `init.zrk`:

```text
project {
    name: "my-app";
    version: "1.0.0";
    type: application;
    description: "Aplicación de ejemplo";
    license: "MIT";
    repository: "https://example.com/user/my-app";
    entry: "src/main.zrk";
}
```

Ese archivo deberá contener una única función `main` válida:

```text
fn main(): Void {
    ...
}
```

También podrá retornar un código de salida:

```text
fn main(): Int32 {
    ...
    return 0;
}
```

Una función `main(): Void` equivaldrá a finalizar con código `0`. Los argumentos del proceso se obtendrán explícitamente desde la biblioteca estándar, por ejemplo mediante `std.process`, sin imponer un parámetro que muchas aplicaciones no necesitan.

Los proyectos `library` no tendrán entrypoint. Tests, benchmarks y otras herramientas utilizarán runners propios.

---

## 37.2 Entry annotation

Alternativa:

```text
@entry
fn start(): Void {}
```

**Respuesta:**

No existirá inicialmente `@entry`.

El bloque `project` de `init.zrk` identificará el archivo de entrada y la función convencional `main` identificará el símbolo ejecutable. Esto evita mantener dos mecanismos equivalentes para seleccionar el inicio de una aplicación.

---

# 38. Paquetes y dependencias

## 38.1 Manifest

**Pregunta:** ¿`init.zrk` reemplaza un archivo como `package.json`?

**TypeScript/npm:**

```json
{
  "name": "app",
  "dependencies": {}
}
```

Posible:

```text
project {
    name: "app";
    version: "1.0.0";
}
```

**Respuesta:**

Sí. `init.zrk` será el manifiesto declarativo del proyecto, tanto para aplicaciones como para librerías.

```text
project {
    name: "my-app";
    version: "1.0.0";
    type: application;
    entry: "src/main.zrk";
}
```

Los tipos iniciales de proyecto serán:

- `application`: produce un ejecutable, puede declarar `globals` y concede los permisos finales.
- `library`: define código reutilizable, no puede declarar `globals` y solo puede declarar capacidades requeridas.

Una librería pasará a ser un paquete cuando se publique o distribuya. No será necesario introducir un tercer tipo de proyecto llamado `package`.

El paquete se distribuirá como un contenedor único con la extensión `.zpkg`. Internamente contendrá conceptualmente:

```text
networking.zpkg
├── manifest
├── public.api
├── portable.ir
├── README.md
├── LICENSE
└── documentation
```

- `public.api` describirá exclusivamente la superficie pública: tipos, firmas, traits, interfaces, documentación y contratos de decoradores.
- `portable.ir` contendrá una representación intermedia tipada e independiente del target.
- `manifest` registrará identidad, versión, dependencias, requisitos, decoradores y compatibilidad.
- `documentation` será opcional.
- `README.md` y la licencia acompañarán los paquetes publicados en el repositorio oficial.

Los nombres de los componentes internos no serán extensiones públicas obligatorias y permanecerán conceptuales hasta especificar formalmente el formato del contenedor. La extensión pública del paquete será `.zpkg`.

La librería no se convertirá definitivamente a una arquitectura al publicarse. Durante el build final, su representación portable se compilará para el mismo target que la aplicación, la biblioteca estándar, el core y el runtime.

---

## 38.2 Instalación

Posible CLI:

```bash
zirk add http
zirk add github:user/package
```

**Respuesta:**

El lenguaje tendrá un gestor de paquetes oficial, inspirado en la ergonomía de npm y en la configuración declarativa de Dart.

```bash
zirk add http
zirk remove http
zirk update
zirk publish
```

Las dependencias se declararán en `init.zrk`:

```text
dependencies {
    http: "^2.1.0";

    local_utils: {
        path: "../local_utils";
    }

    parser: {
        git: "https://example.com/parser.git";
        revision: "a83f21c";
    }

    company_auth: {
        registry: "https://packages.company.com";
        version: "~3.2.0";
    }
}

dev_dependencies {
    testing: "^1.4.0";
    benchmark: "^2.0.0";
}
```

Las dependencias de desarrollo no formarán parte de una aplicación final ni de las dependencias runtime del paquete publicado.

Se utilizará versionamiento semántico y rangos como versiones exactas, `^`, `~` e intervalos explícitos. El resolvedor elegirá una versión compatible y registrará el resultado exacto en `zirk.lock`.

El repositorio oficial tendrá una API de metadata y búsqueda, almacenamiento inmutable de `.zpkg`, distribución mediante CDN o mirrors, namespaces con propietarios verificados y descargas verificadas por hash.

Una versión publicada no podrá reemplazarse silenciosamente. Podrá marcarse como retirada, pero su contenido permanecerá identificable para preservar builds existentes.

Los paquetes privados podrán proceder de registros privados, Git o rutas locales. Las credenciales se almacenarán fuera del proyecto mediante configuración local o el gestor seguro del sistema operativo; nunca en `init.zrk` ni en `zirk.lock`.

No se ejecutarán scripts de instalación automáticamente. Los decoradores y procesos de compilación deberán declararse junto con sus permisos.

Una librería publicable tendrá como mínimo:

```text
my_package/
├── init.zrk
├── README.md
├── LICENSE
└── src/
```

`README.md` y una declaración de licencia serán obligatorios para publicar en el repositorio oficial. `tests/` y `docs/` serán recomendados, pero opcionales. El código fuente podrá incluirse, pero no será obligatorio dentro del paquete binario portable; su API pública siempre será inspeccionable.

---

## 38.3 Lockfile

**Pregunta:** ¿Existe?

Posible:

```text
zirk.lock
```

**Respuesta:**

Sí. Existirá `zirk.lock` y contendrá la resolución exacta y verificable del grafo de dependencias.

Por cada paquete registrará:

- nombre y versión exacta;
- origen y registry;
- hash del contenido y del manifiesto;
- dependencias transitivas;
- versión del formato IR;
- compatibilidad de compilador;
- artefactos nativos y hashes por target.

Las aplicaciones versionarán `zirk.lock` en su repositorio. Los builds normales respetarán exactamente sus versiones y hashes; `zirk update` será la operación explícita para recalcularlos.

El lockfile no almacenará tokens, secretos ni permisos concedidos. Las autorizaciones permanecerán en `init.zrk` y las credenciales privadas en el entorno seguro del usuario.

---

# 39. Testing

## 39.1 Test integrado

**TypeScript/Jest:**

```ts
test("suma", () => {
    expect(add(1, 2)).toBe(3);
});
```

Posible:

```text
@test
fn sumTest(): Void {
    expect(add(1, 2)).toEqual(3);
}
```

**Respuesta:**

El lenguaje tendrá un runner de testing nativo. Los tests unitarios se declararán mediante `@test` exclusivamente en archivos cuyo nombre termine en `.spec.zrk`:

```text
// user_service.spec.zrk

import { expect } from std.testing;

@test
fn creates_a_user(): Void {
    mut user = User("Cristian");

    expect(user.name).to_equal("Cristian");
}
```

Los archivos `.spec.zrk` podrán colocarse junto al código que prueban:

```text
src/
└── users/
    ├── user_service.zrk
    └── user_service.spec.zrk
```

Utilizar `@test` fuera de un archivo `.spec.zrk` producirá un error de compilación, para evitar que una prueba sea ignorada silenciosamente. Estos archivos no formarán parte del binario de producción ni de la implementación runtime del paquete publicado y no obtendrán acceso automático a miembros privados.

Las pruebas end-to-end residirán en la carpeta oficial `test/`, utilizarán la extensión `.e2e.zrk` y se identificarán mediante `@e2e`:

```text
test/
├── authentication.e2e.zrk
├── users.e2e.zrk
├── fixtures/
└── resources/
```

```text
@e2e
fn user_can_complete_checkout(): Void {
    ...
}
```

Los comandos serán:

```bash
zirk test              # Tests unitarios .spec.zrk.
zirk test --unit       # Tests unitarios explícitamente.
zirk test --e2e        # Tests E2E de test/*.e2e.zrk.
zirk test --all        # Ambos grupos.
zirk test --file src/users/user_service.spec.zrk
zirk test --tag integration
zirk test --seed 38142
zirk test --jobs 8
zirk test --report json
```

Los unitarios se ejecutarán en paralelo por defecto y su orden no estará garantizado. El runner mostrará duración, semilla, fallos, excepciones y salida capturada. Los E2E podrán tener paralelismo limitado por configuración debido a servicios y estado externos.

Los permisos de testing estarán separados de producción:

```text
test_permissions {
    read: ["./test/resources"];
    write: ["./test/output"];
    network: ["localhost:5432", "localhost:8080"];
    process: ["docker"];
}
```

El runner detectará tasks abandonadas, threads todavía activos, recursos no cerrados, timeouts, exceptions y `fatalError`. Los tests concurrentes seguirán sujetos a las reglas normales contra data races.

Inicialmente no habrá hooks globales implícitos de setup y teardown. Se utilizarán funciones auxiliares y recursos administrados mediante `match with`. Las fixtures declarativas podrán evaluarse posteriormente.

---

## 39.2 Assert

**Python:**

```python
assert add(1, 2) == 3
```

Posible:

```text
assert add(1, 2) == 3;
```

**Respuesta:**

Sí. Existirá `assert`:

```text
assert total == 10;
assert user.active;
assert total == 10, "El total calculado es incorrecto";
```

La condición se evaluará una sola vez y deberá ser `Boolean`. Si falla, se lanzará una `AssertionException` con la expresión, ubicación y valores relevantes.

`assert` no se eliminará en release y no deberá utilizarse para validar input externo ni errores recuperables.

Dentro de tests se favorecerá `expect`, porque ofrecerá diagnósticos más específicos:

```text
expect(users.length).to_equal(3);
expect(result).to_be_ok();
expect(operation).to_throw<NetworkException>();
```

---

## 39.3 Benchmarks

Posible:

```text
@benchmark
fn sortingBenchmark(): Void {}
```

**Respuesta:**

Los benchmarks se declararán mediante `@benchmark` y se ejecutarán en un runner separado de los tests habituales:

```text
@benchmark
fn parse_user_benchmark(context: BenchmarkContext): Void {
    context.run((): Void => {
        parse_user(INPUT);
    });
}
```

```bash
zirk benchmark
zirk benchmark parser
zirk benchmark --compare baseline.json
```

El runner realizará warmup, múltiples muestras, detección de outliers y mostrará mediana, percentiles, variación, tiempo por operación y operaciones por segundo. Cuando el target lo permita también medirá asignaciones y memoria.

Los reportes registrarán CPU, sistema operativo, target y modo de build para evitar comparaciones engañosas.

---

# 40. Tooling oficial

## 40.1 CLI

¿Qué comandos tendrá?

```bash
zirk new
zirk init
zirk run
zirk build
zirk test
zirk fmt
zirk lint
zirk check
zirk doc
zirk add
zirk remove
zirk publish
```

**Respuesta:**

El tooling oficial se distribuirá como un único ejecutable nativo, sin depender de Node, Python o Java:

```bash
zirk new
zirk init
zirk run
zirk build
zirk check
zirk test
zirk benchmark
zirk fmt
zirk lint
zirk doc
zirk add
zirk remove
zirk update
zirk prepare
zirk publish
zirk clean
zirk lsp
zirk debug
```

`zirk new my_app` creará una carpeta nueva e inicializará el proyecto. `zirk init` inicializará la carpeta actual, respetando su contenido y agregando únicamente la estructura faltante.

```bash
zirk new my_app --application
zirk new utilities --library

cd existing_repository
zirk init --library
```

`run` compilará incrementalmente y ejecutará; `check` comprobará sintaxis, nombres y tipos sin generar un binario; `prepare` auditará permisos, dependencias, targets y publicación; `clean` eliminará únicamente caches y artefactos regenerables de alcance explícito.

La CLI tendrá arranque rápido, cargará solo los subsistemas requeridos por cada comando, admitirá salida humana y JSON, utilizará códigos de salida estables y responderá a cancelación. No accederá a internet ni recopilará telemetría salvo que el comando lo requiera y exista consentimiento explícito.

---

## 40.2 Formatter

**Pregunta:** ¿Habrá un formatter oficial y obligatorio como filosofía?

Posible:

```bash
zirk fmt .
```

**Respuesta:**

Sí. Existirá un formatter oficial con una representación canónica y configuración mínima:

```bash
zirk fmt
zirk fmt src/
zirk fmt --check
```

Será determinista, idempotente, preservará comentarios y no cambiará la semántica. Agregará el `;` opcional de manera consistente, aplicará llaves e indentación uniforme, procesará archivos en paralelo y solo escribirá aquellos cuyo contenido haya cambiado.

No realizará type checking, no cargará dependencias ni ejecutará decoradores. Utilizará tokens y un árbol sintáctico con recuperación de errores para mantener latencia baja incluso mientras se edita código incompleto.

---

## 40.3 Linter

**Respuesta:**

Existirá un linter oficial que compartirá lexer, parser, resolución de nombres, sistema de tipos, grafo del proyecto y caches con el compilador.

Operará por capas: análisis sintáctico local, análisis semántico y análisis global del proyecto. Incluirá reglas para símbolos sin uso, código inalcanzable, shadowing, casts redundantes, matches incorrectos, `Result` ignorados, tasks abandonadas, recursos sin administrar, posibles data races, permisos innecesarios, APIs públicas sin tipos explícitos y patrones de rendimiento problemáticos.

```bash
zirk lint
zirk lint --fix
zirk lint --deny-warnings
```

`--fix` solo aplicará correcciones catalogadas como seguras. Nunca modificará automáticamente el control de flujo, concurrencia, permisos o contratos públicos.

---

## 40.4 LSP

Funciones deseadas:

- autocomplete
- go to definition
- rename
- diagnostics
- hover
- references
- code actions

**Respuesta:**

Existirá un LSP oficial con diagnósticos, autocomplete, hover, navegación a definición, referencias, rename seguro, signature help, símbolos, semantic highlighting, code actions, organización de imports, formato, navegación de traits, metadata de decoradores, tipos inferidos, ayuda de permisos y ejecución o depuración de tests.

El proceso será persistente y mantendrá en memoria los archivos abiertos, árboles sintácticos incrementales, grafo de dependencias y caches de símbolos y tipos.

Al editar se priorizará el archivo visible, se invalidarán únicamente los nodos afectados y el trabajo pesado será cancelable. Una modificación local no provocará recompilar el proyecto completo.

---

## 40.5 Debugger

**Pregunta:** ¿El compilador genera símbolos de depuración?

**Respuesta:**

El compilador generará símbolos estándar: DWARF en Linux y macOS, y PDB/CodeView en Windows. Esto permitirá integrarse inicialmente con LLDB, GDB y depuradores compatibles sin implementar desde cero un motor completo.

El lenguaje ofrecerá un adaptador oficial para representar correctamente clases, records, strings, colecciones, `Result`, tasks, channels, threads y variables capturadas.

```bash
zirk debug
zirk debug --test user_service
```

Soportará breakpoints, step in/over/out, inspección, watches, call stacks, threads y tasks, pausa en exceptions y mapeo al código original cuando intervengan decoradores.

La evaluación de expresiones con efectos secundarios requerirá una advertencia y confirmación explícita.

---

# 41. Optimización

## 41.1 Modos de build

Posible:

```bash
zirk build --debug
zirk build --release
```

**Respuesta:**

Existirán dos modos principales:

```bash
zirk build --debug
zirk build --release
```

`debug` será el modo predeterminado de `run`, `test` y `debug`. Priorizará compilación rápida, símbolos completos, stack traces detallados, comprobaciones y correspondencia clara entre source y ejecución.

`release` priorizará rendimiento y tamaño mediante optimización alta, eliminación de código muerto, inlining, monomorfización y vectorización segura. Los símbolos podrán conservarse, separarse o eliminarse mediante una opción explícita.

Release no debilitará la seguridad semántica. Seguirán activas las comprobaciones de límites, null safety, overflow, permisos y contratos de tipos. `assert` tampoco se eliminará silenciosamente.

---

## 41.2 Optimización automática

Preguntas:

- ¿Inlining?
- ¿Dead-code elimination?
- ¿Escape analysis?
- ¿Vectorización?
- ¿Constant folding?
- ¿Monomorfización?

**Respuesta:**

Sí. El compilador podrá aplicar constant folding, propagación de constantes, eliminación de código muerto y ramas imposibles, inlining, escape analysis, stack allocation segura, eliminación de allocations, devirtualization, monomorfización, vectorización, eliminación de bounds checks demostrablemente redundantes y optimización entre módulos y paquetes.

Las optimizaciones no formarán parte de la semántica pública y deberán conservar el comportamiento observable del programa.

Inicialmente la interfaz se limitará a `debug` y `release`. Posteriormente podrán agregarse preferencias explícitas como optimización por tamaño o velocidad sin cambiar el significado del código.

---

# 42. Compilación incremental

**Pregunta:** ¿Solo se recompilan los módulos modificados?

**Respuesta:**

Sí. La compilación incremental será una característica fundacional.

El compilador mantendrá un grafo de archivos, símbolos públicos, tipos, decoradores, IR y artefactos por target. Cada etapa utilizará fingerprints estables y solo invalidará el trabajo afectado.

- Cambios de formato o comentarios no recompilarán consumidores.
- Cambios en implementación privada recompilarán el módulo afectado.
- Cambios de API pública invalidarán sus consumidores.
- Cambios de un decorador invalidarán sus usos.
- Cambios de permisos o targets invalidarán únicamente las etapas relacionadas.
- Un build sin cambios reutilizará todos los resultados verificables.

El cache será local, direccionado por contenido, incluirá versión del compilador y target, detectará corrupción y podrá compartirse en CI en el futuro mediante hashes verificables.

Las unidades independientes se procesarán en un pool ajustado al hardware, evitando crear un thread por archivo.

Los decoradores deberán ser deterministas, declarar permisos e inputs, producir resultados cacheables y respetar límites de tiempo y memoria. Accesos ocultos a red, reloj o filesystem impedirán una invalidación segura y no estarán permitidos.

El LSP mantendrá el proyecto caliente en memoria para uso interactivo. Los comandos aislados tendrán arranque nativo rápido y reutilizarán caches en disco sin exigir un daemon permanente.

El proyecto mantendrá benchmarks públicos de arranque de CLI, parsing, formato, check incremental, build completo, recompilación de un archivo, build sin cambios, latencia del LSP y memoria máxima. Para proyectos cercanos a 100 archivos, las operaciones calientes e incrementales buscarán latencias de milisegundos; será un objetivo medido, no una garantía independiente del hardware y del código.

---

# 43. ABI y estabilidad binaria

## 43.1 ABI

**Pregunta:** ¿Quieres que librerías compiladas con distintas versiones del compilador sean compatibles?

**Respuesta:**

La compatibilidad principal entre versiones del compilador se apoyará inicialmente en la API pública y en una versión explícita del formato intermedio portable, no en prometer una ABI nativa estable entre cualquier versión.

Durante el build final, la aplicación, sus paquetes, la biblioteca estándar, el core y el runtime se alinearán con el mismo target: sistema operativo, arquitectura, ancho de puntero, ABI y formato binario.

Las dependencias que contengan FFI o binarios nativos deberán aportar una variante compatible con el target solicitado. Si no existe, el compilador detendrá el build e identificará la dependencia incompatible.

---

## 43.2 Name mangling

**C++ usa name mangling.**

Posible lenguaje:

```text
UserService_findUser_UInt64
```

**Pregunta:** ¿Será parte pública de la especificación?

**Respuesta:**

El name mangling interno no será parte estable de la especificación pública. El compilador podrá cambiarlo para representar overloads, generics, versiones y optimizaciones.

Solo tendrán nombres binarios estables los símbolos exportados explícitamente mediante `extern "C"` o una declaración equivalente para FFI.

La compatibilidad pública se apoyará en `public.api`, versiones del formato IR y contratos explícitos, no en nombres internos del compilador.

---

# 44. Sintaxis adicional

## 44.1 Ternario

**TypeScript:**

```ts
const value = active ? "yes" : "no";
```

**Pregunta:** ¿Existe?

**Respuesta:**

Sí. Existirá el operador ternario:

```text
mut message = active ? "Activo" : "Inactivo";
```

La condición deberá ser `Boolean`; no se aplicará truthiness. Solo se evaluará una de las dos ramas y sus resultados deberán tener tipos compatibles o formar una union válida. El operador se asociará de derecha a izquierda.

Para lógica con más casos o efectos complejos se favorecerá `match` o `if`.

---

## 44.2 Incremento

**TypeScript:**

```ts
counter++;
counter--;
```

**Pregunta:** ¿Se permite o solo `counter += 1`?

**Respuesta:**

Existirán incremento y decremento tanto en forma prefija como postfija:

```text
count++;
count--;
++count;
--count;
```

También podrán participar en expresiones. La forma postfija producirá el valor anterior y después modificará la variable; la forma prefija modificará primero la variable y producirá el valor nuevo.

```text
mut count = 0;

mut previous = count++; // previous = 0; count = 1.
mut current = ++count;  // count = 2; current = 2.
```

Solo podrán aplicarse sobre ubicaciones numéricas mutables. Cada operación evaluará su operando una sola vez y respetará las reglas normales de overflow.

---

## 44.3 Compound assignment

**TypeScript:**

```ts
value += 10;
value -= 10;
value *= 2;
```

**Respuesta:**

Sí. Existirán asignaciones compuestas:

```text
count += 1;
count -= 1;
count *= 2;
count /= 2;
count %= 10;
value **= 2;
```

El lado izquierdo deberá ser mutable y se evaluará una sola vez. Se aplicarán las mismas reglas de tipos, división y overflow que en la operación equivalente, sin introducir casts implícitos peligrosos.

---

## 44.4 Range

**Python:**

```python
for i in range(0, 10):
    pass
```

Posible:

```text
for i in 0..10 {
}
```

Preguntas:

- ¿Incluye el final?
- ¿`..` vs `..=`?

**Respuesta:**

Sí. Existirá `Range<T>` como una secuencia iterable y lazy, equivalente conceptualmente a `range()` de Python pero integrada mediante operadores.

```text
0..10  // Incluye 0 y excluye 10.
0..=10 // Incluye ambos extremos.
```

Uso:

```text
for index in 0..10 {
    stdout.println(index);
}
```

Los ranges implementarán `Iterable<T>` y podrán utilizar `map`, `filter` y `reduce` sin construir previamente un array.

```text
(0..100).step(5);
(0..10).reverse();
```

La dirección se inferirá por los bounds: `10..0` descenderá hasta excluir `0` y
`10..=0` lo incluirá. `step(n)` recibirá una distancia positiva distinta de
cero. Un bound calculado podrá escribirse como `0..{number}.step(1)`.

`Range<T>` será independiente del slicing. El slicing conservará su sintaxis `[inicio:fin:paso]`, mientras que un range será un objeto iterable.

---

# 45. Convenciones oficiales

## 45.1 Nombres de variables

Opciones:

- camelCase
- snake_case

Ejemplo TypeScript:

```ts
const userName = "Cristian";
```

**Respuesta:**

Las variables `mut` utilizarán `lower_snake_case`:

```text
mut request_count: UInt64 = 0;
```

Las variables `inmut` e `inmut::strict` utilizarán `UPPER_SNAKE_CASE`, tanto en scope local como global:

```text
inmut API_URL: String = "https://example.com";
inmut::strict DEFAULT_CONFIG: Config = Config();
```

Los globals también utilizarán `UPPER_SNAKE_CASE`, incluso cuando sean mutables. No existirán prefijos con semántica especial como `S__`, `m_` o `_private`.

---

## 45.2 Nombres de clases

Ejemplo:

```ts
class UserService {}
```

**Respuesta:**

Clases, interfaces, traits, enums y records utilizarán `PascalCase`:

```text
class UserService {}
interface Serializable {}
trait Comparable {}
enum RequestStatus {}
record UserSummary {}
```

Las variantes de enum también utilizarán `PascalCase`, por ejemplo `RequestStatus.Active`.

Los decoradores utilizarán `lower_snake_case`, como `@route` y `@reflect`. Los parámetros generic podrán utilizar nombres breves convencionales como `T` y `E`, o nombres descriptivos como `Item`.

---

## 45.3 Constantes globales

Ejemplo:

```text
// init.zrk
globals {
    inmut MAX_CONNECTIONS: UInt32 = 1000;
}
```

**Respuesta:**

Las constantes globales se declararán dentro del bloque `globals` de `init.zrk` y utilizarán `UPPER_SNAKE_CASE`.

```text
globals {
    inmut MAX_CONNECTIONS: UInt32 = 1000;
}
```

Los archivos que las utilicen deberán habilitarlas explícitamente:

```text
use MAX_CONNECTIONS;
```

---

## 45.4 Archivos

Ejemplos:

```text
User.zrk
user.zrk
user-service.zrk
user_service.zrk
```

**Respuesta:**

Los archivos, módulos y paquetes utilizarán `lower_snake_case`:

```text
user_service.zrk
users.authentication
http_client
```

La privacidad se expresará mediante las reglas del lenguaje y `share`, no mediante prefijos en el nombre del archivo o del símbolo.

---

# 46. Diagnósticos del compilador

## 46.1 Formato de errores

Ejemplo deseable:

```text
error[E102]: expected Int32, received String

12 | mut age: Int32 = "30";
   |                  ^^^^
   |                  expected Int32
```

**Pregunta:** ¿Los errores tendrán código estable?

**Respuesta:**

Los diagnósticos serán descriptivos, localizables y accionables. Cada error incluirá severidad, código estable, descripción breve, archivo, línea, columna, fragmento, ubicación exacta, causa, ayuda y notas relacionadas cuando correspondan.

```text
error[E0314]: acceso a un valor nullable

  src/users.zrk:18:12
   │
18 │     user.name;
   │     ^^^^ `user` tiene tipo `User?`
   │
   = causa: un valor `User?` puede contener `null`
   = ayuda: utiliza `user?.name` o valida el valor mediante `match`
```

Los errores originados en dependencias, decoradores, targets o permisos mostrarán su cadena causal:

```text
error[E0721]: permiso de red no declarado

  src/api.zrk:24:9
   │
24 │     http.get("https://api.example.com/users");
   │     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
   │
   = destino solicitado: api.example.com
   = requerido por: package `http_client@2.1.4`
   = ayuda: agrega el host a `permissions.network` en init.zrk
```

La salida utilizará colores sin depender exclusivamente de ellos, tendrá un formato textual para personas y otro estructurado para editores y CI. Los códigos de diagnóstico serán documentados y estables.

Los stack traces de runtime conservarán líneas de código originales y relaciones causales entre tasks. Los diagnósticos ocultarán secretos y credenciales.

---

## 46.2 Warnings

Ejemplos:

- variable sin usar
- cast innecesario
- código muerto
- shadowing
- posible data race

**Pregunta:** ¿Warnings pueden convertirse en errores?

**Respuesta:**

Existirán warnings para condiciones no necesariamente incorrectas, pero potencialmente problemáticas: variables o imports sin uso, código inalcanzable, casts redundantes, ramas redundantes, permisos sin uso, APIs obsoletas, shadowing y riesgos de rendimiento.

```bash
zirk check
zirk check --deny-warnings
```

Las supresiones serán locales, explícitas y referenciarán un código concreto:

```text
@allow("W0120")
fn compatibility_adapter(): Void {
    ...
}
```

El compilador podrá ofrecer correcciones, pero no modificará el código silenciosamente.

---

# 47. Shadowing

**TypeScript:**

```ts
let value = 1;

{
    let value = 2;
}
```

**Pregunta:** ¿Se permite declarar un identificador con el mismo nombre en un scope interno?

**Respuesta:**

No se permitirá declarar dentro de una función un identificador local que oculte otro identificador local o parámetro todavía visible:

```text
mut value = 10;

if active {
    mut value = 20; // Error de compilación.
}
```

Esta regla evitará confusiones sobre qué variable se está leyendo o modificando. Cuando se necesite otro valor deberá utilizarse un nombre distinto. Los aliases explícitos de imports y destructuración seguirán utilizando `->`.

---

# 48. Mutabilidad en parámetros

**Pregunta:** ¿Los parámetros son inmutables por defecto?

**TypeScript permite reasignarlos:**

```ts
function foo(value: number) {
    value = 20;
}
```

Posible lenguaje:

```text
fn foo(inmut value: Int32): Void {}
fn foo(mut value: Int32): Void {}
```

**Respuesta:**

Los parámetros serán bindings inmutables por defecto:

```text
fn process(user: User): Void {
    user = User(); // Error de compilación.
}
```

Un parámetro podrá marcarse `mut` para permitir su reasignación local:

```text
fn normalize(mut value: Int32): Int32 {
    value += 1;
    return value;
}
```

Reasignar un parámetro `mut` no modificará la variable del llamador. Si el parámetro contiene una referencia administrada a un objeto mutable, modificar ese objeto sí podrá ser observable por el llamador, salvo que el contrato utilice inmutabilidad profunda.

---

# 49. Pass by value / reference

## 49.1 Tipos simples

**C:**

```c
void foo(int value) {}
```

C pasa `int` por valor.

**Pregunta:** ¿Int32, Boolean, Char, etc. se pasan por valor?

**Respuesta:**

Sí. Números, `Boolean`, `Char`, enums y demás tipos con semántica de valor se pasarán conceptualmente por valor.

```text
fn increment(mut value: Int32): Int32 {
    value += 1;
    return value;
}
```

Modificar el parámetro no afectará la variable del llamador. El compilador podrá utilizar registros, referencias internas u otras optimizaciones siempre que el comportamiento observable sea equivalente a una copia del valor.

---

## 49.2 Objetos

**TypeScript:**

```ts
function rename(user: User) {
    user.name = "Pedro";
}
```

El objeto es accesible mediante una referencia.

**Pregunta:** ¿Las clases se pasan por referencia, referencia compartida o copia?

**Respuesta:**

Las instancias de clases normales se pasarán mediante una referencia administrada compartida. La referencia se pasará por valor, por lo que reasignar el parámetro no reemplazará la variable del llamador, pero modificar el objeto podrá ser observable desde ambas referencias.

```text
fn rename(user: User): Void {
    user.name = "Pedro";
}
```

La administración de memoria será automática. El runtime garantizará que el objeto continúe vivo mientras exista una referencia válida.

`inmut` impedirá reasignar la referencia, pero no hará inmutable el objeto.
`inmut::strict` aplicará inmutabilidad profunda y prohibirá crear un alias
mutable o adquirirse mientras siga accesible uno. Un `clone()` independiente
podrá recibir un binding mutable.

---

# 50. Copias y cloning

**Pregunta:** ¿Asignar un objeto copia o comparte referencia?

**TypeScript:**

```ts
const a = user;
const b = a;
```

`a` y `b` apuntan al mismo objeto.

Posible lenguaje:

```text
mut a: User = user;
mut b: User = a;
```

**Respuesta:**

Asignar una instancia de una clase normal compartirá la misma referencia administrada y no realizará una copia implícita:

```text
mut first = User();
mut second = first;

second.name = "Ana";

first is second; // true
```

Los tipos con semántica de valor —números, booleanos, chars, enums, records y tuplas— se copiarán semánticamente al asignarse.

El compilador decidirá la representación física, pero no podrá cambiar estas reglas observables.

---

## 50.1 Clone explícito

Posible:

```text
mut copy = user.clone();
```

**Respuesta:**

No existirá `Object.clone()` de forma universal. Solo los tipos que puedan producir correctamente una copia lógica independiente implementarán un trait explícito:

```text
trait Clone<T> {
    fn clone(): T;
}
```

```text
mut copy: User = user.clone();
```

`clone()` producirá una copia lógica independiente. El compilador podrá derivar su implementación cuando todos los campos sean clonables; en los demás casos deberá implementarse manualmente.

Un método también podrá clonarse como callable ligado a su receptor:

```text
inmut print = clone(stdout.println);
print("Hola");
```

Esto copiará el valor callable y conservará su firma, efectos y permisos; no
clonará `stdout` ni su handle nativo.

Recursos, archivos abiertos, sockets, threads, tasks activas, channels y handles nativos no serán clonables por defecto.

---

# 51. Value classes / structs

Aunque todo sea una clase, quizá convenga distinguir clases de valor.

**C:**

```c
struct Point {
    int x;
    int y;
};
```

Posible:

```text
record Point {
    inmut x: Float32;
    inmut y: Float32;
}
```

**Pregunta:** ¿Existirá una clase optimizada por valor?

**Respuesta:**

Sí. Existirán `record` para crear tipos nominales ligeros con semántica de valor:

```text
record UserId {
    inmut VALUE: UInt64;
}
```

Aunque internamente pueda representarse como un `UInt64`, `UserId` será un tipo diferente:

```text
mut user_id: UserId;
mut order_id: OrderId;

user_id = order_id; // Error de compilación.
```

Los records tendrán igualdad estructural, no tendrán identidad observable y podrán almacenarse inline. No heredarán de otras clases, pero podrán implementar traits e interfaces.

---

# 52. Records

**TypeScript:**

```ts
type Point = {
    x: number;
    y: number;
};
```

Posible:

```text
record Point(
    x: Float32,
    y: Float32
);
```

**Pregunta:** ¿Necesitas records o todo debe ser `class`?

**Respuesta:**

Sí. Los records representarán productos de datos estructurales y evitarán utilizar una clase completa para DTO, eventos y resultados simples.

```text
record UserSummary {
    id: UserId;
    name: String;
}
```

Serán inmutables por defecto, tendrán igualdad estructural, semántica de valor y no tendrán identidad observable mediante `is`.

Una clase normal se utilizará cuando se necesiten identidad, estado mutable, herencia o comportamiento complejo. Un record se utilizará cuando el significado esté determinado principalmente por sus datos.

---

# 53. Reflection sobre tipos primitivos

Si todo es clase:

```text
mut number: Int32 = 10;

number.type();
number.size();
number.toString();
```

**Pregunta:** ¿Qué métodos base debe tener `Object`?

Posibles:

```text
toString()
equals()
hash()
type()
clone()
```

**Respuesta:**

Los tipos primitivos expondrán la misma identidad básica que cualquier otra clase:

```text
Int32.type();
number.type();
number.to_string();
```

`Object` ofrecerá un contrato base reducido y coherente:

```text
to_string(): String
equals(other: Object): Boolean
hash(): UInt64
type(): Type
```

`==` utilizará la igualdad definida por el tipo. `clone()` no pertenecerá a `Object`; estará disponible únicamente mediante el trait `Clone<T>`.

Reflection podrá mostrar nombre, tamaño lógico cuando sea estable, rango, traits, métodos y metadata pública de un tipo primitivo. No expondrá detalles internos de representación que el compilador pueda optimizar.

---

# 54. Object identity

**Pregunta:** ¿Todo objeto tiene identidad propia?

Ejemplo:

```text
mut a = User("Cristian");
mut b = User("Cristian");

a == b; // ¿true por contenido?
a is b; // ¿false por identidad?
```

**Respuesta:**

Solo los tipos con semántica de referencia tendrán identidad propia. En clases normales, `==` comparará estructuralmente y `is` comprobará si ambas referencias apuntan a la misma instancia:

```text
mut a = User("Cristian");
mut b = User("Cristian");

a == b; // true si su contenido es estructuralmente igual.
a is b; // false porque son instancias diferentes.
```

Números, booleanos, chars, enums, records y tuplas no tendrán identidad observable. Aplicar `is` a esos tipos producirá un error de compilación; su representación física no formará parte de la semántica pública.

---

# 55. Destructores

TypeScript no tiene destructores deterministas.

**C++ conceptual:**

```cpp
~File() {
    close();
}
```

Posible:

```text
destruct() {
    file.close();
}
```

**Pregunta:** ¿Hay destructores deterministas?

**Respuesta:**

No existirán destructores deterministas definidos por el usuario en la primera versión.

La administración de memoria de objetos normales corresponderá al compilador y al runtime, por lo que el momento de liberar memoria no formará parte de la semántica pública.

Los recursos del sistema operativo no dependerán de destructores ni de la recolección de memoria. Los tipos que necesiten cierre determinista implementarán el contrato `Resource<E>` y se administrarán mediante `match with`:

```text
interface Resource<E> {
    fn close(): Result<Void, E>;
}
```

```text
match with File.open("file.txt") {
    Ok(file) => {
        file.read_text();
    }

    Error(error) => {
        stderr.println(error);
    }
}
```

Al finalizar la rama `Ok`, el recurso se cerrará automáticamente. El runtime podrá realizar limpieza defensiva de recursos abandonados, pero esta no sustituirá ni garantizará el cierre determinista.

---

# 56. Resource management

**Python:**

```python
with open("file.txt") as file:
    ...
```

Posible:

```text
using file = File.open("file.txt") {
    ...
}
```

**Pregunta:** ¿Existirá `using`, `defer` o RAII?

**Respuesta:**

Existirá administración estructurada de recursos mediante `match with`.

`match with` aceptará una expresión que retorne `Result<T, E>` cuando `T` implemente el contrato `Resource<CloseError>`.

Tendrá la misma semántica contextual que `match`.

Como sentencia, administrará el recurso sin producir un valor:

```text
match with File.open("file.txt") {
    Ok(file) => {
        match file.read_text() {
            Ok(content) => stdout.println(content);
            Error(error) => stderr.println(error);
        };
    }

    Error(error) => {
        stderr.println(error);
    }
};
```

En un contexto que requiera una expresión, todas las ramas deberán producir un valor compatible:

```text
mut content: Result<String, FileError> =
    match with File.open("file.txt") {
        Ok(file) => file.read_text();
        Error(error) => Error(error);
    };
```

El recurso se cerrará antes de entregar el valor producido. El compilador impedirá que ese valor contenga el propio recurso o referencias dependientes de su vida útil.

Si la adquisición retorna `Error`, se ejecutará esa rama y no habrá ningún recurso que cerrar.

Si retorna `Ok(resource)`, el recurso permanecerá abierto durante esa rama y se cerrará automáticamente al abandonarla, incluso mediante `return`, `break`, `continue`, una exception o una cancelación cooperativa.

El identificador dentro de `Ok(...)` será el nombre local elegido para el recurso. No se utilizará `->`, porque no se está creando un alias:

```text
Ok(file)
Ok(config_file)
Ok(child_process)
```

Un recurso administrado no podrá escapar de la rama `Ok`. El compilador rechazará retornarlo, almacenarlo en un objeto de mayor duración o capturarlo en una operación que sobreviva al scope.

Para conservar o transferir el recurso se utilizará `match` sin `with`:

```text
mut opened_file: Result<File, FileError> =
    match File.open("file.txt") {
        Ok(file) => Ok(file);
        Error(error) => Error(error);
    };
```

Por lo tanto:

```text
match with Resource.open(...)
→ cierre automático y recurso limitado al scope.

match Resource.open(...)
→ administración o transferencia manual.
```

Los errores inesperados producidos durante el cierre no se ignorarán ni se convertirán en `fatalError`. Se representarán como una exception recuperable de recursos. En recursos de escritura, `flush()` retornará `Result` para manejar fallos esperables antes de abandonar el scope.

No existirán inicialmente `using`, `defer` ni RAII basado en destructores.

---

# 57. Defer

**Go:**

```go
defer file.Close()
```

No está en TS/Python como keyword equivalente directa.

Posible:

```text
defer file.close();
```

**Pregunta:** ¿Existirá?

**Respuesta:**

No existirá `defer` en la primera versión.

El cierre repetitivo de recursos se resolverá mediante `match with`, que ofrece una regla más específica, visible y verificable por el compilador:

```text
match with File.open("file.txt") {
    Ok(file) => {
        file.read_text();
    }

    Error(error) => {
        stderr.println(error);
    }
}
```

No será necesario escribir:

```text
defer file.close();
```

Esta decisión evita mantener simultáneamente `match with`, `defer`, `using` y destructores como mecanismos diferentes para el mismo problema.

Si posteriormente aparecen casos importantes de limpieza que no puedan modelarse mediante `Resource<E>` y `match with`, `defer` podrá reevaluarse como una característica independiente.

---

# 58. Módulos nativos y permisos

**Pregunta:** ¿Un programa declara permisos/capacidades?

Posible:

```text
permissions {
    filesystem: read;
    network: true;
}
```

Esto sería especialmente útil para targets WebAssembly/sandbox.

**Respuesta:**

Sí. El lenguaje utilizará permisos explícitos y granulares para las capacidades externas del programa.

En una aplicación, `permissions` concederá capacidades necesarias durante la ejecución:

```text
permissions {
    read: ["./data", "./certificates"];
    write: ["./output"];
    network: ["api.example.com"];
    environment: ["APP_ENV"];
    process: ["git"];
}
```

`compile_permissions` concederá por separado las capacidades que podrán utilizar decoradores y otros procesos ejecutados durante la compilación:

```text
compile_permissions {
    read: ["./schemas"];
}
```

Las categorías iniciales podrán incluir lectura, escritura, red, variables de entorno, procesos, FFI, sistema y generación dinámica de código. Siempre que sea posible se limitarán por rutas, hosts, variables, ejecutables o bibliotecas concretas.

Una librería no podrá concederse autoridad. Declarará sus necesidades mediante `requires`:

```text
requires {
    runtime {
        network: ["api.example.com"];
        read: ["./certificates"];
    }

    compile {
        read: ["./schemas"];
    }
}
```

La aplicación será la única que podrá autorizar finalmente esos requisitos mediante `permissions` y `compile_permissions`. Instalar una dependencia no concederá sus permisos automáticamente.

Durante el desarrollo, `zirk run` y las herramientas de análisis podrán detectar permisos faltantes, explicar su causa y ofrecer agregarlos a `init.zrk`. El comando `zirk prepare` realizará una auditoría completa de la aplicación, sus dependencias y sus decoradores, mostrando permisos declarados, requeridos, faltantes y sin uso; una opción explícita como `--apply` podrá actualizar el manifiesto.

`zirk build` será estricto: no concederá permisos ni modificará el proyecto interactivamente. Si falta una autorización necesaria, el build fallará con un diagnóstico preciso.

`unsafe` no será un permiso. `unsafe` relajará garantías de memoria dentro del código; los permisos controlarán la autoridad sobre el sistema externo. El código seguro deberá acceder al sistema mediante el runtime o la biblioteca estándar. FFI, assembly y bibliotecas nativas se considerarán capacidades de alto riesgo porque podrían eludir controles del runtime.

---

# 59. Assembly inline

TypeScript/Python no aplica.

**C/GCC conceptual:**

```c
asm("nop");
```

Posible:

```text
unsafe asm {
    "nop"
}
```

**Pregunta:** ¿Se permite assembly inline?

**Respuesta:**

No existirá assembly textual inline en la primera versión. En su lugar, el lenguaje ofrecerá intrinsics portables: operaciones con una semántica estable que el compilador transformará en las instrucciones apropiadas para el target seleccionado.

No se intentará crear un ensamblador universal. Los registros, instrucciones, convenciones de llamada y capacidades difieren demasiado entre x86, ARM y sus variantes. La capa portable describirá la operación deseada, no la instrucción física que debe utilizarse.

El conjunto inicial de intrinsics podrá cubrir:

- rotaciones de bits;
- conteo de bits activos;
- conteo de ceros iniciales o finales;
- inversión de bytes y endianess;
- operaciones aritméticas comprobadas, con saturación o wrap-around;
- atomics y memory fences;
- prefetch como sugerencia no vinculante;
- copia, comparación y llenado de memoria;
- operaciones vectoriales SIMD portables.

Ejemplos conceptuales:

```text
mut rotated = intrinsics.rotate_left(value, 8);
mut active_bits = intrinsics.pop_count(value);
mut normalized = intrinsics.byte_swap(value);
```

LLVM elegirá instrucciones como `popcnt`, `bswap`, NEON u otras equivalentes cuando el target las soporte. Si no existe una instrucción directa, el compilador podrá generar una secuencia equivalente o una implementación portable.

Los intrinsics puramente numéricos y seguros no requerirán `unsafe`. Las operaciones sobre memoria cruda, hardware, punteros o fences con contratos que el compilador no pueda verificar sí deberán utilizarse dentro de `unsafe { }`.

Las operaciones no portables o que requieran instrucciones exactas se implementarán inicialmente en C, C++ o Rust y se consumirán mediante el ABI de C. El assembly inline podrá reconsiderarse posteriormente si aparecen necesidades que los intrinsics y FFI no puedan resolver.

---

# 60. SIMD

**Pregunta:** ¿Habrá vectores SIMD explícitos?

Posible:

```text
mut vector: SIMD<Float32, 4>;
```

**Respuesta:**

Sí. El lenguaje ofrecerá SIMD de dos maneras complementarias: vectorización automática y vectores explícitos portables.

En builds optimizados, el compilador intentará vectorizar automáticamente bucles y operaciones sobre colecciones cuando pueda demostrar que la transformación conserva la semántica:

```text
for index in 0..values.length {
    result[index] = left[index] + right[index];
}
```

El desarrollador no necesitará conocer SIMD para beneficiarse de estas optimizaciones.

Para casos especializados existirá un tipo explícito y portable:

```text
mut left: Vector<Float32, 4>;
mut right: Vector<Float32, 4>;

mut result = left + right;
```

`Vector<T, N>` describirá un vector lógico y no una instrucción concreta como AVX o NEON. LLVM decidirá si utiliza una instrucción SIMD, varias instrucciones o una implementación escalar según el target y sus capacidades.

Inicialmente se admitirán tipos numéricos compatibles y un tamaño `N` conocido durante compilación. Las operaciones incluirán aritmética por elemento, comparaciones, selección, carga, almacenamiento y reducciones seguras.

Las operaciones vectoriales normales no requerirán `unsafe`. Cargar o almacenar mediante punteros crudos, ignorar alineación o solicitar capacidades específicas del hardware sí requerirá `unsafe`.

El compilador deberá preservar las reglas del lenguaje sobre overflow, límites, alineación y memoria. SIMD no permitirá debilitar la seguridad de un programa normal.

---

# 61. Arquitectura del compilador

## 61.1 Frontend

Decidir:

```text
Source
↓
Lexer
↓
Parser
↓
AST
↓
Name Resolution
↓
Type Checker
```

**Pregunta:** ¿Quieres una AST pública para tooling/macros?

**Respuesta:**

El compilador mantendrá su AST semántica interna como una representación privada. Podrá modificarla y optimizarla sin convertir su estructura en un contrato público permanente.

El lenguaje ofrecerá por separado una Syntax API pública, documentada y versionada para tooling externo:

```text
import std.syntax;

mut document = syntax.parse(source);

for declaration in document.declarations() {
    stdout.println(declaration.name);
}
```

La Syntax API conservará tokens, posiciones, comentarios, nodos inválidos y código incompleto para permitir linters, documentación, migraciones, refactors, herramientas educativas e integraciones de editor.

Inicialmente será principalmente de solo lectura. Las herramientas que modifiquen código producirán ediciones de texto o transformaciones estructuradas verificables, pero no podrán insertar nodos arbitrarios dentro de la AST privada.

El formatter, linter, LSP y compilador compartirán el mismo frontend y sus caches. Los decoradores continuarán utilizando exclusivamente su API tipada `target/context`, sin obtener acceso irrestricto a la AST ni a la Syntax API para modificar el compilador.

---

## 61.2 IR

**Pregunta:** ¿Tendrá representación intermedia propia?

Posible:

```text
Typed AST
↓
Language IR
↓
LLVM IR / WebAssembly
```

**Respuesta:**

Sí. El compilador tendrá una representación intermedia propia, tipada y portable entre los targets compatibles.

```text
Source
↓
Typed AST
↓
Portable Language IR
↓
Target IR / machine code
↓
Mach-O / ELF / PE
```

Esta representación permitirá distribuir la implementación de una librería sin fijarla prematuramente a Windows, Linux, macOS, x86 o ARM. En el build final, el compilador transformará conjuntamente el IR de la aplicación, los paquetes, la biblioteca estándar, el core y el runtime para el target seleccionado.

Esto permitirá especializar generics, eliminar código no utilizado, realizar inlining entre paquetes y seleccionar layouts, atomics e instrucciones apropiados para la arquitectura final.

La representación intermedia tendrá una versión de formato comprobable por el compilador. `portable.ir` será por ahora un nombre conceptual interno del contenedor `.zpkg`, no una extensión que el desarrollador deba manipular directamente.

---

## 61.3 Backend

Opciones:

- LLVM
- Cranelift
- backend propio
- múltiples

**Respuesta:**

LLVM será el backend único de la primera versión.

```text
Source
↓
Frontend
↓
Portable Language IR
↓
LLVM IR
↓
ARM / x86
↓
Mach-O / ELF / PE
```

Los builds debug utilizarán LLVM con optimización mínima para priorizar velocidad de compilación, símbolos completos y correspondencia clara con el source. Los builds release utilizarán optimización alta para reducir tiempo de ejecución, tamaño del binario y uso de memoria cuando sea posible.

Formatter, linter, `check` y LSP no invocarán LLVM, por lo que su latencia dependerá únicamente del frontend incremental. LLVM solo participará cuando sea necesario generar código.

El backend procesará únicamente IR invalidado y reutilizará artefactos incrementales. La integración utilizará opciones orientadas a eliminar código no utilizado, reducir allocations, optimizar layouts, vectorizar y favorecer binarios pequeños sin cambiar la seguridad semántica.

La arquitectura interna mantendrá una interfaz de backend para poder incorporar Cranelift u otro generador en el futuro si las mediciones demuestran que LLVM impide alcanzar los objetivos de compilación o memoria. No se mantendrán varios backends inicialmente, evitando diferencias de comportamiento y duplicación de pruebas.

No se construirá un backend propio durante la primera etapa. Solo se considerará con un lenguaje estable, benchmarks reales y una necesidad técnica demostrada.

---

# 62. Semántica que debe quedar escrita formalmente

Esta sección no es una pregunta individual: debe convertirse eventualmente en la especificación oficial.

Para cada característica anterior documentar:

```text
1. Sintaxis
2. Semántica
3. Tipos involucrados
4. Errores de compilación
5. Errores de runtime
6. Ejemplos válidos
7. Ejemplos inválidos
8. Interacción con mut/inmut
9. Interacción con concurrencia
10. Interacción con targets
```

Ejemplo:

```text
mut age: Int8 = 200;
```

La especificación debe decir explícitamente si:

- falla durante compilación
- falla durante runtime
- hace overflow
- requiere cast

---

# 63. Decisiones fundacionales prioritarias

Antes de implementar el compilador, rellenar como mínimo estas decisiones.

## Sintaxis

- [ ] `;` obligatorio/opcional
- [ ] `{}` o indentación
- [ ] paréntesis en `if`
- [ ] sintaxis de funciones
- [ ] sintaxis de clases
- [ ] sintaxis de generics
- [ ] imports
- [ ] annotations

## Tipos

- [ ] tipado estático
- [ ] inferencia
- [ ] null/none
- [ ] numeric widths
- [ ] signed/unsigned
- [ ] tipos de valor
- [ ] unions
- [ ] generics

## Objetos

- [ ] todo es clase
- [ ] Object raíz
- [ ] herencia
- [ ] interfaces
- [ ] traits
- [ ] clases abiertas/cerradas
- [ ] identidad vs igualdad

## Memoria

- [ ] GC
- [ ] ownership
- [ ] reference counting
- [ ] stack/heap automático
- [ ] pointers
- [ ] unsafe
- [ ] destructores

## Concurrencia

- [ ] async
- [ ] task
- [ ] parallel
- [ ] worker
- [ ] thread
- [ ] channels
- [ ] locks
- [ ] atomics
- [ ] prevención de data races

## Errores

- [ ] exceptions
- [ ] Result
- [ ] panic
- [ ] propagación
- [ ] checked/unchecked

## Compilación

- [ ] native ARM64
- [ ] native x86-64
- [ ] Windows PE
- [ ] Linux ELF
- [ ] macOS Mach-O
- [ ] WebAssembly
- [ ] cross compilation
- [ ] standalone binaries

---

# 64. Ejemplo integrador para definir la personalidad del lenguaje

Rellena cómo debería escribirse este mismo programa en tu lenguaje.

## Objetivo

1. Declarar una clase `User`.
2. Tener `id` inmutable.
3. Tener `name` mutable.
4. Crear una colección de usuarios.
5. Procesarlos en paralelo.
6. Manejar un posible error.
7. Imprimir resultados.

**TypeScript de referencia:**

```ts
class User {
    constructor(
        public readonly id: number,
        public name: string,
    ) {}
}

async function processUser(user: User): Promise<string> {
    return `Procesado: ${user.name}`;
}

async function main() {
    const users = [
        new User(1, "Cristian"),
        new User(2, "Ana"),
    ];

    const results = await Promise.all(
        users.map(user => processUser(user)),
    );

    for (const result of results) {
        console.log(result);
    }
}
```

**Cómo debería verse en tu lenguaje:**

```text
// RELLENAR
```

---

# 65. Ejemplo integrador de bajo nivel

Define cómo debería hacerse algo cercano al sistema operativo.

**C de referencia:**

```c
#include <stdio.h>
#include <stdlib.h>

int main(void) {
    int *value = malloc(sizeof(int));

    if (value == NULL) {
        return 1;
    }

    *value = 42;
    printf("%d\n", *value);

    free(value);

    return 0;
}
```

**Cómo debería verse en tu lenguaje si soporta memoria manual:**

```text
// RELLENAR
```

O indica:

```text
El lenguaje no permitirá memoria manual porque:
```

---

# 66. Ejemplo integrador de concurrencia

Define cómo quieres que un desarrollador escriba paralelismo CPU real.

**Python de referencia:**

```python
from concurrent.futures import ProcessPoolExecutor

def process(value):
    return value * value

with ProcessPoolExecutor() as executor:
    results = list(executor.map(process, range(1000)))
```

**Cómo debería verse idealmente en tu lenguaje:**

```text
parallel for value in 0..1000 {
    // RELLENAR
}
```

Define además:

```text
¿Cómo se decide cantidad de workers?
¿Cómo se recolectan resultados?
¿Cómo se propagan errores?
¿Cómo se cancela?
¿Cómo se evita una data race?
```

---

# 67. Ejemplo integrador web

**TypeScript de referencia:**

```ts
const button = document.querySelector("#button")!;

button.addEventListener("click", async () => {
    const response = await fetch("/api/user");
    const user = await response.json();

    document.querySelector("#name")!.textContent = user.name;
});
```

**Cómo debería verse en tu lenguaje compilado a WebAssembly:**

```text
@runtime WEB;

// RELLENAR
```

Define:

```text
¿Cómo accede al DOM?
¿Cómo maneja eventos?
¿Cómo maneja HTTP?
¿Cómo convierte String entre Wasm y navegador?
¿Cómo se cargará el binario Wasm?
```

---

# 68. Ejemplo integrador backend nativo

**TypeScript de referencia conceptual:**

```ts
const server = createServer();

server.get("/users/:id", async request => {
    const user = await repository.find(request.params.id);

    return {
        status: 200,
        body: user,
    };
});

server.listen(3000);
```

**Cómo debería verse en tu lenguaje:**

```text
// RELLENAR
```

Define:

```text
¿La stdlib tendrá HTTP server?
¿Threads por request?
¿Event loop?
¿Structured concurrency?
¿Workers?
```

---

# 69. Preguntas de coherencia final

Cuando hayas rellenado el documento, revisar:

1. ¿Puede explicarse `mut` en una sola frase?
2. ¿Puede explicarse `inmut` en una sola frase?
3. ¿Puede explicarse `global` en una sola frase?
4. ¿Existe alguna variable cuya mutabilidad sea ambigua?
5. ¿Existe alguna conversión numérica implícita peligrosa?
6. ¿Existe null?
7. ¿Puede ocurrir una data race sin usar `unsafe`?
8. ¿Un `thread` siempre representa un thread del OS?
9. ¿Un `worker` siempre tiene memoria aislada?
10. ¿`parallel` garantiza paralelismo real o solo lo solicita?
11. ¿Async significa concurrencia, paralelismo o ninguna de las dos?
12. ¿Todo es realmente una clase desde la semántica del lenguaje?
13. ¿Los números generan objetos en heap o son clases optimizadas por valor?
14. ¿Cuál es la diferencia entre clase y record?
15. ¿Cómo se compara igualdad?
16. ¿Cómo se compara identidad?
17. ¿Qué ocurre con overflow?
18. ¿Qué ocurre con división por cero?
19. ¿Qué ocurre al acceder fuera de un array?
20. ¿Cómo se libera memoria?
21. ¿Puede haber use-after-free?
22. ¿Existen punteros?
23. ¿Qué necesita `unsafe`?
24. ¿Cómo se llaman funciones C?
25. ¿Cómo se crea una librería dinámica?
26. ¿Cómo se compila para Windows/Linux/macOS?
27. ¿Cómo se compila para navegador?
28. ¿Qué contiene `init.zrk`?
29. ¿Qué cosas nunca deberían estar en `init.zrk`?
30. ¿Cómo se importan módulos?
31. ¿Cómo funciona el package manager?
32. ¿Cómo se versiona una dependencia?
33. ¿Cómo se definen APIs públicas?
34. ¿Cómo se mantiene compatibilidad entre versiones del lenguaje?
35. ¿Cómo se escribe un test?
36. ¿Cómo se ejecuta un benchmark?
37. ¿Cómo se formatea el código?
38. ¿Cómo se depura?
39. ¿Cómo se generan source maps para Wasm?
40. ¿Qué comportamiento cambia entre debug y release?

---

# 70. Resultado esperado de esta plantilla

Después de rellenarla, debería ser posible producir cuatro documentos derivados:

```text
LANGUAGE_SPEC.md
├── sintaxis formal
├── semántica
├── sistema de tipos
└── reglas del lenguaje

COMPILER_SPEC.md
├── lexer
├── parser
├── AST
├── type checker
├── IR
└── codegen

RUNTIME_SPEC.md
├── memoria
├── concurrencia
├── GC
├── threads
├── workers
├── async
└── plataforma

STDLIB_SPEC.md
├── collections
├── io
├── filesystem
├── networking
├── web
├── concurrency
└── testing
```

La recomendación es **no implementar características importantes hasta que su comportamiento esté definido aquí con al menos un ejemplo válido y uno inválido**.
