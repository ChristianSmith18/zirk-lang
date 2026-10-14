# Zirk: el juego — guión de niveles (fundamentos, sin OOP)

> Este documento es **ejecutable**. Cada nivel lleva su código inicial y su solución
> de referencia, y `game-assets/curriculum/verify.mjs` las corre en el intérprete
> del navegador con un simulador de la cuadrícula. Si un nivel no se puede resolver
> con su propia solución, o su código inicial ya pasa, la verificación falla.
>
> ```sh
> cargo build -p zirk-browser --example interp_repro
> node game-assets/curriculum/verify.mjs
> ```
>
> Alcance: salida, variables, tipos primitivos, referencias frente a valores,
> flujo de control, funciones, colecciones, y los tipos de datos `record`, `enum` y
> `Result`. **No** hay clases, interfaces, herencia ni excepciones: eso es el
> segundo tramo del juego.

## 1. Cómo funciona el juego

### 1.1 La premisa

Zirk, un robot, despierta en una estación espacial a oscuras. Cada sector de la
estación enseña una parte del lenguaje y se reactiva cuando el jugador la
domina. El jugador **escribe programas Zirk reales**; el juego los ejecuta y
reproduce lo que hicieron.

| Sector | Qué enseña | Niveles |
| --- | --- | --- |
| 1. Cubierta de mando | Programas, `println`, el orden de las instrucciones, leer errores | 1.1 a 1.5 |
| 2. Jardín de datos | Variables, `mut`/`inmut`, tipos primitivos, números, texto, `Boolean`, `null` | 2.1 a 2.12 |
| 3. Laberinto de circuitos | `if`, `while`, `for`, `break`, `continue`, bucles anidados, `match` | 3.1 a 3.8 |
| 4. Taller | Funciones, parámetros, retorno, argumentos con nombre, recursión, lambdas, lo que capturan y funciones como datos | 4.1 a 4.8 |
| 5. Almacén | `Array`, `List`, `Map`, `Set`, tuplas, **valor frente a referencia**, `clone()`, `==` frente a `is`, parámetros | 5.1 a 5.13 |
| 6. Archivo | `record`, `enum`, `enum` con datos, `Result` | 6.1 a 6.6 |
| Final | Bahía de lanzamiento: todo junto | 7.1 |

### 1.2 Cómo se ejecuta lo que escribe el jugador

El programa corre en el worker WASM (ADR-020: sin efectos de host salvo salida
acotada). Para que un programa pueda *mover al robot* sin ampliar esa autoridad,
el juego **inyecta funciones auxiliares ocultas** que escriben líneas de protocolo
por la salida estándar, y las reproduce después:

- La salida que empieza con `@` son **órdenes** (`@walk 2`, `@turn left`,
  `@collect`, `@inspect coins 3`). El juego las anima.
- Cualquier otra línea es **texto de terminal** (lo que el jugador imprime con
  `stdout.println`). Se muestra en la pantalla del nivel.

El código del jugador se ensambla así, sin tocar los números de línea:

```
import { stdout, Stringifiable } from std.io;  ← oculto, en la primera línea del jugador
<código del jugador>                           ← incluye su propio import visible
<funciones auxiliares ocultas>                 ← al final del archivo
```

El `import` duplicado es válido, y las funciones del final pueden usarse antes de
declararse, así que el jugador escribe `walk(2)` sin ver nada más.

Esto es deliberado: el jugador usa el lenguaje real (`stdout`, `import`, `fn main`),
y las órdenes del robot son llamadas a funciones comunes. Las funciones disponibles
crecen con cada sector:

```zirk prelude core
fn walk(steps: Int32): Void { stdout.println("@walk", steps); }
fn turn_left(): Void { stdout.println("@turn", "left"); }
fn turn_right(): Void { stdout.println("@turn", "right"); }
fn collect(): Void { stdout.println("@collect"); }
fn hop(): Void { stdout.println("@hop"); }
fn inspect(label: String, value: Stringifiable): Void { stdout.println("@inspect", label, value); }
```

| Función | Efecto en el juego |
| --- | --- |
| `walk(n)` | Avanza `n` casillas hacia donde mira. Un muro detiene el programa del nivel. |
| `turn_left()`, `turn_right()` | Gira 90 grados. |
| `collect()` | Recoge la moneda de la casilla actual. |
| `hop()` | Salta dos casillas hacia delante (sobre agua o un hueco). |
| `inspect(etiqueta, valor)` | Muestra el valor en un **panel de datos** junto al robot (chip, batería…). |

Los **sensores** (`sensor()`, `light()`, etc.) son funciones auxiliares que
devuelven un valor distinto en cada *caso de prueba* del nivel. Así un `if` o un
bucle de verdad es necesario: la misma solución tiene que funcionar para todos los
casos, no solo para uno.

### 1.3 Tipos de nivel

| Tipo | El jugador debe… | Cómo se valida |
| --- | --- | --- |
| `grid` | Llegar a la meta (y recoger las monedas) con órdenes | Un simulador reproduce las órdenes sobre el mapa |
| `output` | Hacer que el programa imprima exactamente algo | La salida coincide línea a línea, en **todos** los casos |
| `fix` | Arreglar un programa roto leyendo el diagnóstico del compilador | Compila y produce la salida pedida |
| `predict` | Predecir qué imprime (o qué falla) un programa antes de ejecutarlo | La respuesta se compara con la ejecución real |

### 1.4 Reglas pedagógicas del guión

1. **Un concepto nuevo por nivel**, y el nivel siguiente lo usa.
2. **Errores reales.** Los niveles `fix` muestran los diagnósticos auténticos del
   compilador (`E0404`, `E0403`…), no mensajes inventados. Aprender a leerlos es
   parte del curso.
3. **Predecir antes de ejecutar.** Los conceptos que sorprenden (división, decimales
   exactos, referencias) se enseñan con niveles `predict`: primero apostar, luego ver.
4. **Pistas en tres escalones:** una pregunta que orienta, el concepto con un
   ejemplo mínimo, y la forma de la solución sin darla completa.
5. **Fallar es barato.** Un error de compilación o un fallo de ejecución se muestra
   con el robot (`sad`) y el diagnóstico, nunca como pantalla de derrota.
6. **Estrellas por dominio, no por velocidad:** ★ resolverlo, ★★ sin ayudas,
   ★★★ cumplir el reto extra (menos órdenes, sin repetir código, etc.).

### 1.5 Formato de cada nivel

Cada nivel tiene: concepto, historia, objetivo, mapa (si es `grid`), código inicial,
pistas, errores comunes, solución, extra de ★★★ y los assets que usa
(`game-assets/`, ver su README). Los bloques `level` y `zirk` son los que lee el
verificador.

---

## 2. Sector 1 — Cubierta de mando

*La estación está a oscuras. La terminal principal espera una orden. Aquí Zirk
aprende que un programa es una lista de instrucciones y que el orden importa.*

### Nivel 1.1 — Hola, Zirk

**Concepto:** un programa Zirk empieza en `fn main()`; `stdout.println` escribe una línea.
**Historia:** la terminal de la cubierta parpadea y espera un saludo para despertar.
**Objetivo:** que la terminal muestre `Hola, Zirk`.
**Assets:** `terminal`, `bot-zirk`.
**Notas de arte:** `terminal`, `bot-zirk` (`think` mientras se escribe, `celebrate` al acertar).

```level
{ "id": "1.1", "type": "output", "prelude": ["core"], "expect": ["Hola, Zirk"] }
```

```zirk starter 1.1
import { stdout } from std.io;

fn main(): Void {
    // Escribe aquí el mensaje que despierta la terminal
}
```

```zirk solution 1.1
import { stdout } from std.io;

fn main(): Void {
    stdout.println("Hola, Zirk");
}
```

**Pistas:** (1) ¿Qué función de `stdout` escribe una línea? (2) Lo que va entre comillas
es un texto: se imprime tal cual. (3) `stdout.println("...");` dentro de `main`.
**Errores comunes:** olvidar las comillas (`E0401`: *`Hola` is not declared*, porque
sin comillas es un nombre, no un texto); escribir mal el texto (la terminal compara
carácter a carácter, incluida la coma y la mayúscula).
**★★★:** ninguno, es el primer paso.

### Nivel 1.2 — Una orden tras otra

**Concepto:** las instrucciones se ejecutan **en orden**, de arriba abajo.
**Historia:** el pasillo hasta el ascensor tiene dos giros. Zirk solo sabe seguir órdenes.
**Objetivo:** llegar a la meta `G`.
**Assets:** `bot-zirk`, `start-pad`, `goal-portal`, `tile-circuit`, `wall`.
**Notas de arte:** `bot-zirk` (`walk`), `start-pad`, `goal-portal`, `tile-circuit`, `wall`.

```level
{ "id": "1.2", "type": "grid", "prelude": ["core"], "facing": "east",
  "map": ["S..#",
          "##.#",
          "G..#"],
  "par": 5 }
```

```zirk starter 1.2
import { stdout } from std.io;

fn main(): Void {
    walk(2);
}
```

```zirk solution 1.2
import { stdout } from std.io;

fn main(): Void {
    walk(2);
    turn_right();
    walk(2);
    turn_right();
    walk(2);
}
```

**Pistas:** (1) Zirk mira al este al empezar: ¿hacia dónde tiene que girar en cada esquina?
(2) `turn_right()` gira sin avanzar; hay que volver a llamar a `walk`. (3) Son cinco
instrucciones: avanzar, girar, avanzar, girar, avanzar.
**Errores comunes:** olvidar un giro (el robot choca con el muro y el nivel se detiene);
contar mal las casillas.
**★★★:** resolverlo con las cinco órdenes mínimas.

### Nivel 1.3 — print y println

**Concepto:** `println` escribe y salta de línea; `print` escribe **sin** saltar.
**Historia:** la cuenta atrás de despegue debe verse en una sola línea.
**Objetivo:** imprimir `3-2-1` en una línea y `¡Despegue!` en la siguiente.
**Assets:** `terminal`, `battery`.

```level
{ "id": "1.3", "type": "output", "prelude": ["core"], "expect": ["3-2-1", "¡Despegue!"] }
```

```zirk starter 1.3
import { stdout } from std.io;

fn main(): Void {
    stdout.println("3-");
    stdout.println("2-");
    stdout.println("1");
    stdout.println("¡Despegue!");
}
```

```zirk solution 1.3
import { stdout } from std.io;

fn main(): Void {
    stdout.print("3-");
    stdout.print("2-");
    stdout.println("1");
    stdout.println("¡Despegue!");
}
```

**Pistas:** (1) ¿Qué pasa con la línea cuando usas `println` en cada número? (2) `print` no
añade el salto de línea. (3) Cambia las dos primeras llamadas.
**Errores comunes:** usar `print` también en la última línea (todo queda en una sola).

### Nivel 1.4 — Lee el error

**Concepto:** el compilador te dice **qué** falla y **dónde**; leerlo es una habilidad.
**Historia:** el panel de energía muestra un mensaje rojo. Zirk lo lee en voz alta.
**Objetivo:** arreglar el programa para que imprima `Energía: 100`.
**Assets:** `terminal`, `bot-zirk`.
**Notas de arte:** `terminal`, `bot-zirk` (`sad` mientras haya error).

```level
{ "id": "1.4", "type": "fix", "prelude": ["core"], "starterDiagnostic": "E0301",
  "expect": ["Energía: 100"] }
```

```zirk starter 1.4
import { stdout } from std.io;

fn main(): Void {
    stdout.println("Energía:", 100;
}
```

```zirk solution 1.4
import { stdout } from std.io;

fn main(): Void {
    stdout.println("Energía:", 100);
}
```

**Diagnóstico real que ve el jugador:** `error[E0301]: expected ')' to close the argument list`,
con una flecha en la línea y la columna.
**Pistas:** (1) El mensaje habla de un `)`. ¿Dónde empieza la llamada? (2) Cada `(` necesita su
`)`. (3) Cierra la llamada a `println`.
**★★★:** además, arreglarlo sin cambiar la línea más que lo necesario.

### Nivel 1.5 — Varios valores, una línea

**Concepto:** `println` acepta **varios valores** de cualquier tipo y los une con espacios.
**Historia:** la ficha de la nave mezcla textos, números y verdadero/falso.
**Objetivo:** imprimir `Nave: Zirk-1 motores: 4 listos: true`.
**Assets:** `terminal`, `chip`.

```level
{ "id": "1.5", "type": "output", "prelude": ["core"], "expect": ["Nave: Zirk-1 motores: 4 listos: true"] }
```

```zirk starter 1.5
import { stdout } from std.io;

fn main(): Void {
    stdout.println("Nave:");
}
```

```zirk solution 1.5
import { stdout } from std.io;

fn main(): Void {
    stdout.println("Nave:", "Zirk-1", "motores:", 4, "listos:", true);
}
```

**Pistas:** (1) Los valores se separan con comas dentro de los paréntesis. (2) Los textos van entre
comillas; los números y `true` no. (3) Seis valores: tres textos, un número, otro texto y un `Boolean`.
**Errores comunes:** poner el número entre comillas (`"4"` también se imprime igual, pero deja de ser
un número: el próximo sector explica por qué importa).

---

## 3. Sector 2 — Jardín de datos

*Una cúpula botánica cuyas compuertas se abren con datos. Aquí Zirk aprende qué es
una variable, qué tipos de valores existen y por qué un `42` no es lo mismo que un
`"42"`.*

### Nivel 2.1 — Cajas con nombre

**Concepto:** una variable `mut` es una caja con nombre cuyo contenido puede cambiar.
**Historia:** el invernadero lleva la cuenta de las semillas. El contador tiene que subir.
**Objetivo:** mostrar el contador en el panel de datos: primero `0`, luego `5`, luego `8`.
**Assets:** `object-panel`, `chip`, `bot-zirk`.
**Notas de arte:** `object-panel` (la barra del contador sube), `chip`, `bot-zirk` (`think`).

```level
{ "id": "2.1", "type": "output", "prelude": ["core"],
  "expect": ["@inspect seeds 0", "@inspect seeds 5", "@inspect seeds 8"],
  "mistakes": [ { "replace": ["seeds = seeds + 5;", "seed = seeds + 5;"], "diagnostic": "E0401" } ] }
```

```zirk starter 2.1
import { stdout } from std.io;

fn main(): Void {
    mut seeds = 0;
    inspect("seeds", seeds);
    // Suma 5 semillas y muéstralas; luego suma 3 más y muéstralas
}
```

```zirk solution 2.1
import { stdout } from std.io;

fn main(): Void {
    mut seeds = 0;
    inspect("seeds", seeds);
    seeds = seeds + 5;
    inspect("seeds", seeds);
    seeds += 3;
    inspect("seeds", seeds);
}
```

**Pistas:** (1) Una caja `mut` se puede reasignar con `=`. (2) El nuevo valor puede usar el viejo: `seeds = seeds + 5`.
(3) `seeds += 3` es la forma corta de `seeds = seeds + 3`.
**Errores comunes:** escribir mal el nombre de la variable (`E0401`: *`seed` is not declared*).

### Nivel 2.2 — Lo que nunca cambia

**Concepto:** `inmut` crea una caja que **no** se puede reasignar; el compilador lo impide.
**Historia:** el reactor tiene un límite de 100 que nadie debe tocar; la energía actual sí baja.
**Objetivo:** gastar 30 de energía sin alterar el límite, y mostrar ambos.
**Assets:** `battery`, `object-panel`.
**Notas de arte:** `battery` (nivel), `object-panel`.

```level
{ "id": "2.2", "type": "fix", "prelude": ["core"], "starterDiagnostic": "E0404",
  "expect": ["@inspect energy 70", "@inspect limit 100"] }
```

```zirk starter 2.2
import { stdout } from std.io;

fn main(): Void {
    inmut max_energy = 100;
    mut energy = max_energy;
    max_energy = max_energy - 30;
    inspect("energy", energy);
    inspect("limit", max_energy);
}
```

```zirk solution 2.2
import { stdout } from std.io;

fn main(): Void {
    inmut max_energy = 100;
    mut energy = max_energy;
    energy = energy - 30;
    inspect("energy", energy);
    inspect("limit", max_energy);
}
```

**Diagnóstico real:** `error[E0404]: cannot reassign 'max_energy'`.
**Pistas:** (1) El error dice qué variable no se puede reasignar. (2) ¿Cuál de las dos cajas *debería*
cambiar? (3) La energía actual es la `mut`.

### Nivel 2.3 — ¿De qué tipo es?

**Concepto:** todo valor tiene un **tipo**: `Int32` (entero), `String` (texto), `Decimal`
(número con decimales exactos), `Boolean` (verdadero/falso), `Char` (una letra).
**Historia:** el escáner del jardín clasifica cada objeto en una bandeja distinta.
**Objetivo (predicción):** ¿qué imprime el programa?
**Assets:** `chip`.
**Notas de arte:** cinco `chip` de colores distintos, una bandeja por tipo.

```level
{ "id": "2.3", "type": "predict",
  "prelude": ["core"],
  "options": ["Int32 Int32 Int32 Int32 Int32", "Int32 String Decimal Boolean Char", "Int String Float Bool Char"],
  "answer": 1 }
```

```zirk program 2.3
import { stdout } from std.io;

fn main(): Void {
    stdout.println(42.type, "42".type, 4.2.type, true.type, 'Z'.type);
}
```

**Explicación al revelar:** `42` es un entero; `"42"` entre comillas dobles es **texto**; `4.2` es un
`Decimal`; `true` es `Boolean`; `'Z'` entre comillas simples es **un** carácter (`Char`). El
compilador decide el tipo mirando cómo está escrito el valor.

### Nivel 2.4 — Un número y un texto no se suman igual

**Concepto:** `+` suma números pero **une** textos: `40 + 2` y `"40" + "2"` son cosas distintas.
**Historia:** la cerradura pide la clave `42`, pero hay dos maneras de armarla.
**Objetivo (predicción):** ¿qué imprime el programa?
**Assets:** `chip`, `terminal`.
**Notas de arte:** `chip` (dos con el mismo 42: uno numérico y otro de texto, de distinto color), `terminal`.

```level
{ "id": "2.4", "type": "predict", "prelude": ["core"],
  "options": ["42\n42", "42\n402", "402\n402"],
  "answer": 1 }
```

```zirk program 2.4
import { stdout } from std.io;

fn main(): Void {
    stdout.println(40 + 2);
    stdout.println("40" + "2");
}
```

**Explicación al revelar:** con números, `+` calcula (`42`). Con textos, `+` los pega (`"402"`).

### Nivel 2.5 — No mezcles tipos sin pedirlo

**Concepto:** Zirk no convierte tipos a escondidas: sumar un `String` y un `Int32` es un error.
**Historia:** el marcador de puntos intenta pegar un texto con un número.
**Objetivo:** que el marcador imprima `Puntos: 5`.
**Assets:** `terminal`, `chip`.

```level
{ "id": "2.5", "type": "fix", "prelude": ["core"], "starterDiagnostic": "E0403",
  "expect": ["Puntos: 5"],
  "mistakes": [ { "replace": ["\"Puntos:\", 5", "\"Puntos: \" + 5"], "diagnostic": "E0403" } ] }
```

```zirk starter 2.5
import { stdout } from std.io;

fn main(): Void {
    stdout.println("Puntos: " + 5);
}
```

```zirk solution 2.5
import { stdout } from std.io;

fn main(): Void {
    stdout.println("Puntos:", 5);
}
```

**Diagnóstico real:** `error[E0403]: '+' is not available on String and Int32`.
**Pistas:** (1) El error dice qué dos tipos chocan. (2) `println` ya sabe mostrar varios valores de
cualquier tipo. (3) Sepáralos con una coma en vez de sumarlos.
**★★★:** resolverlo también con `String(5)` para pegarlos: `"Puntos: " + String(5)`.

### Nivel 2.6 — El cofre de 8 bits

**Concepto:** los enteros tienen **tamaño**: `Int8` guarda de -128 a 127; `Int16`, `Int32` y `Int64`
guardan más. Un valor que no cabe es un error.
**Historia:** el cofre de carga solo tiene 8 bits, y hay que guardar 200 cajas.
**Objetivo:** escoger un tipo donde quepa el 200.
**Assets:** `chest`, `battery`.

```level
{ "id": "2.6", "type": "fix", "prelude": ["core"], "starterDiagnostic": "E0408",
  "expect": ["200"],
  "mistakes": [ { "replace": ["Int16", "Int8"], "diagnostic": "E0408" } ] }
```

```zirk starter 2.6
import { stdout } from std.io;

fn main(): Void {
    mut cargo: Int8 = 200;
    stdout.println(cargo);
}
```

```zirk solution 2.6
import { stdout } from std.io;

fn main(): Void {
    mut cargo: Int16 = 200;
    stdout.println(cargo);
}
```

**Diagnóstico real:** `error[E0408]: the literal 200 does not fit in Int8`.
**Pistas:** (1) `Int8` llega a 127. (2) Hay tipos más grandes. (3) `Int16` guarda hasta 32 767.
**★★★:** usar `UInt8` (sin signo: de 0 a 255), que también cabe.

### Nivel 2.7 — Cuando un número se pasa

**Concepto:** si una cuenta se sale del rango de su tipo, Zirk **detiene el programa** con
`ArithmeticOverflowError` en lugar de dar una respuesta falsa.
**Historia:** el cargador intenta subir un `Int8` que ya está en su máximo.
**Objetivo (predicción):** ¿qué ocurre?
**Assets:** `battery`, `bot-zirk`.
**Notas de arte:** `battery` (se sobrecarga), `bot-zirk` (`sad` al detenerse).

```level
{ "id": "2.7", "type": "predict", "prelude": ["core"],
  "options": ["-128", "128", "ArithmeticOverflowError"],
  "answer": 2 }
```

```zirk program 2.7
import { stdout } from std.io;

fn main(): Void {
    mut charge: Int8 = 127;
    stdout.println("antes:", charge);
    charge = charge + 1;
    stdout.println("después:", charge);
}
```

**Explicación al revelar:** se imprime `antes: 127` y el programa se detiene con
`ArithmeticOverflowError`; la segunda línea nunca se imprime. En otros lenguajes el valor
"daría la vuelta" a -128 sin avisar; aquí el error es explícito.

### Nivel 2.8 — Dividir no siempre da un entero

**Concepto:** `/` entre enteros da un **`Decimal` exacto**, `%` da el resto, y `Int32(...)` corta los decimales.
**Historia:** hay que repartir 7 baterías entre 2 robots.
**Objetivo (predicción):** ¿qué imprime el programa?
**Assets:** `battery`, `crate`, `bot-zirk`.
**Notas de arte:** `battery` (siete), `crate`, `bot-zirk`.

```level
{ "id": "2.8", "type": "predict", "prelude": ["core"],
  "options": ["3\n1\n3", "3.5\n1\n3", "3.5\n3.5\n3.5"],
  "answer": 1 }
```

```zirk program 2.8
import { stdout } from std.io;

fn main(): Void {
    stdout.println(7 / 2);
    stdout.println(7 % 2);
    stdout.println(Int32(7 / 2));
}
```

**Explicación al revelar:** `7 / 2` es `3.5`; `7 % 2` es `1` (lo que sobra); `Int32(7 / 2)` convierte
a entero **truncando** a `3`.

### Nivel 2.9 — Decimales exactos y decimales binarios

**Concepto:** los decimales normales de Zirk (`Decimal`) son **exactos**; los de coma flotante binaria
(con sufijo `f`, como `0.1f`) tienen el error de redondeo clásico.
**Historia:** el balanza de la cúpula pesa dos semillas de `0.1` y `0.2` y las compara con `0.3`.
**Objetivo (predicción):** ¿qué imprime el programa?
**Assets:** `crystal`, `chip`.
**Notas de arte:** `crystal` (las dos semillas), `chip`.

```level
{ "id": "2.9", "type": "predict", "prelude": ["core"],
  "options": ["true\ntrue", "true\nfalse", "false\nfalse"],
  "answer": 1 }
```

```zirk program 2.9
import { stdout } from std.io;

fn main(): Void {
    stdout.println(0.1 + 0.2 == 0.3);
    stdout.println(0.1f + 0.2f == 0.3f);
}
```

**Explicación al revelar:** con `Decimal`, `0.1 + 0.2` es exactamente `0.3`. Con `f` (binario),
`0.1f + 0.2f` vale `0.30000000000000004`, que no es igual a `0.3f`. Zirk elige lo exacto por
defecto; lo binario es opcional.

### Nivel 2.10 — Texto y letras

**Concepto:** un `String` se puede unir, medir (`length`) y leer letra a letra con `[ ]`; la
**interpolación** `"{valor}"` mete valores dentro de un texto.
**Historia:** la placa de la entrada debe mostrar el nombre del jardín con su tamaño.
**Objetivo:** imprimir `Jardín: Zirk (4 letras, empieza con Z)` usando la variable `name`.
**Assets:** `sign`, `terminal`.
**Notas de arte:** `sign` (la placa de entrada), `terminal`.

```level
{ "id": "2.10", "type": "output", "prelude": ["core"],
  "expect": ["Jardín: Zirk (4 letras, empieza con Z)"],
  "mistakes": [ { "replace": ["{name.length}", "{name.largo}"], "diagnostic": "E0426" } ] }
```

```zirk starter 2.10
import { stdout } from std.io;

fn main(): Void {
    inmut name = "Zirk";
    stdout.println("Jardín:", name);
}
```

```zirk solution 2.10
import { stdout } from std.io;

fn main(): Void {
    inmut name = "Zirk";
    stdout.println("Jardín: {name} ({name.length} letras, empieza con {name[0]})");
}
```

**Pistas:** (1) Dentro de un texto, `{ ... }` evalúa lo que haya dentro. (2) `name.length` cuenta las letras.
(3) `name[0]` es la primera letra.
**Errores comunes:** inventarse un miembro (`E0426`: *String has no member*).

### Nivel 2.11 — Verdadero o falso

**Concepto:** las comparaciones (`>=`, `<`, `==`) dan `Boolean`; se combinan con `&&` (y), `||` (o), `!` (no).
**Historia:** la compuerta solo abre si la energía está entre 50 y 89.
**Objetivo:** imprimir `true` si `50 <= energía < 90`, y `false` si no, para cualquier lectura del sensor.
**Assets:** `door`, `plate`.
**Notas de arte:** `door` (se abre si imprime `true`), `plate`.

```level
{ "id": "2.11", "type": "output", "prelude": ["core"],
  "cases": [
    { "name": "energía 70", "helpers": "fn energy_level(): Int32 { return 70; }", "expect": ["true"] },
    { "name": "energía 20", "helpers": "fn energy_level(): Int32 { return 20; }", "expect": ["false"] },
    { "name": "energía 95", "helpers": "fn energy_level(): Int32 { return 95; }", "expect": ["false"] }
  ],
  "mistakes": [ { "replace": ["&&", "||"], "wrongResult": true } ] }
```

```zirk starter 2.11
import { stdout } from std.io;

fn main(): Void {
    inmut energy = energy_level();
    stdout.println(energy >= 50);
}
```

```zirk solution 2.11
import { stdout } from std.io;

fn main(): Void {
    inmut energy = energy_level();
    stdout.println(energy >= 50 && energy < 90);
}
```

**Pistas:** (1) Hacen falta *dos* condiciones a la vez. (2) `&&` exige que ambas sean verdaderas.
(3) La segunda condición es `energy < 90`.
**Errores comunes:** usar `||` (se abre con cualquier valor).

### Nivel 2.12 — Cuando no hay nada

**Concepto:** un tipo con `?` (como `String?`) puede ser `null` (ausencia); `??` da un valor de
repuesto cuando no hay nada.
**Historia:** el banco de semillas devuelve el nombre de la semilla, o nada si el cajón está vacío.
**Objetivo:** imprimir el nombre de las semillas 1, 2 y 3, o `sin semilla` donde no haya.
**Assets:** `chest`, `crate`.
**Notas de arte:** `chest` (el cajón del banco de semillas), `crate`.

```level
{ "id": "2.12", "type": "output", "prelude": ["core"],
  "cases": [ { "name": "banco", "helpers": "fn find_seed(id: Int32): String? { if id == 1 { return \"rosa\"; } return null; }" } ],
  "expect": ["rosa", "sin semilla", "sin semilla"],
  "mistakes": [ { "replace": ["?? \"sin semilla\"", ""], "wrongResult": true } ] }
```

```zirk starter 2.12
import { stdout } from std.io;

fn main(): Void {
    stdout.println(find_seed(1) ?? "sin semilla");
}
```

```zirk solution 2.12
import { stdout } from std.io;

fn main(): Void {
    stdout.println(find_seed(1) ?? "sin semilla");
    stdout.println(find_seed(2) ?? "sin semilla");
    stdout.println(find_seed(3) ?? "sin semilla");
}
```

**Pistas:** (1) `find_seed` puede devolver `null`. (2) `a ?? b` significa «`a`, o `b` si `a` es `null`».
(3) Repite la línea para las semillas 2 y 3.
**Errores comunes:** olvidar el `??`: se imprime `<Null>` donde no hay semilla.

---

## 4. Sector 3 — Laberinto de circuitos

*Un laberinto de pasillos que cambian según los sensores. Un programa que solo sabe una
ruta no sirve: tiene que **decidir** y **repetir**. Aquí Zirk aprende `if`, los bucles y `match`.*

> En este sector los niveles `grid` y `output` traen **varios casos de prueba**: el sensor
> devuelve un valor distinto en cada uno y la misma solución tiene que servir en todos.

### Nivel 3.1 — Si… o si no

**Concepto:** `if` / `else` ejecuta un bloque u otro según una condición `Boolean`.
**Historia:** un puente levadizo cruza el agua. A veces está bajado y a veces no.
**Objetivo:** llegar a la meta tanto si el puente está bajado como si no (en ese caso se salta con `hop()`).
**Assets:** `tile-water`, `tile-bridge`, `bot-zirk`, `goal-portal`.
**Notas de arte:** `tile-water`, `tile-bridge`, `bot-zirk` (`jump` en `hop`), `goal-portal`.

```level
{ "id": "3.1", "type": "grid", "prelude": ["core"], "facing": "east", "par": 4,
  "map": ["S...G"],
  "cases": [
    { "name": "puente bajado", "map": ["S...G"], "helpers": "fn bridge_down(): Boolean { return true; }" },
    { "name": "puente levantado", "map": ["S.~.G"], "helpers": "fn bridge_down(): Boolean { return false; }" }
  ],
  "mistakes": [ { "replace": ["if bridge_down()", "if !bridge_down()"], "wrongResult": true } ] }
```

```zirk starter 3.1
import { stdout } from std.io;

fn main(): Void {
    walk(4);
}
```

```zirk solution 3.1
import { stdout } from std.io;

fn main(): Void {
    if bridge_down() {
        walk(4);
    } else {
        walk(1);
        hop();
        walk(1);
    }
}
```

**Pistas:** (1) `bridge_down()` devuelve `true` o `false`. (2) `if condición { ... } else { ... }`.
(3) Con el puente levantado: avanza una casilla, salta y avanza otra.
**Errores comunes:** negar la condición (`!`) invierte los dos casos: el robot se cae al agua.

### Nivel 3.2 — Varias posibilidades

**Concepto:** `else if` encadena condiciones; `if` también puede **devolver un valor**.
**Historia:** el semáforo del laberinto tiene tres luces: 0 rojo, 1 amarillo, 2 verde.
**Objetivo:** imprimir `alto`, `cuidado` o `avanza` según `light()`.
**Assets:** `lamp`, `plate`.
**Notas de arte:** `lamp` (rojo / amarillo / verde), `plate`.

```level
{ "id": "3.2", "type": "output", "prelude": ["core"],
  "cases": [
    { "name": "rojo", "helpers": "fn light(): Int32 { return 0; }", "expect": ["alto"] },
    { "name": "amarillo", "helpers": "fn light(): Int32 { return 1; }", "expect": ["cuidado"] },
    { "name": "verde", "helpers": "fn light(): Int32 { return 2; }", "expect": ["avanza"] }
  ],
  "mistakes": [ { "replace": ["else if light() == 1", "else if light() = 1"], "diagnostic": "E0301" } ] }
```

```zirk starter 3.2
import { stdout } from std.io;

fn main(): Void {
    if light() == 0 {
        stdout.println("alto");
    }
}
```

```zirk solution 3.2
import { stdout } from std.io;

fn main(): Void {
    if light() == 0 {
        stdout.println("alto");
    } else if light() == 1 {
        stdout.println("cuidado");
    } else {
        stdout.println("avanza");
    }
}
```

**Pistas:** (1) Hay tres casos y solo uno está escrito. (2) Después de `if`, añade `else if` y por
último `else`. (3) `==` compara; `=` asigna, y aquí no se puede usar.
**★★★:** guardar la palabra con un `if` que devuelve valor
(`inmut word = if light() == 0 { "alto" } else if light() == 1 { "cuidado" } else { "avanza" };`) y
llamar a `println` una sola vez.

### Nivel 3.3 — Repetir con while

**Concepto:** `while` repite un bloque **mientras** su condición sea verdadera; hay que cambiar algo
dentro para que acabe.
**Historia:** el pasillo mide una longitud distinta cada vez que lo recorres. El sensor la dice.
**Objetivo:** llegar a la meta en pasillos de 3, 5 y 7 casillas, avanzando de una en una.
**Assets:** `tile-conveyor`, `bot-zirk`.
**Notas de arte:** `tile-conveyor`, `bot-zirk` (`walk`).

```level
{ "id": "3.3", "type": "grid", "prelude": ["core"], "facing": "east",
  "map": ["S..G"],
  "cases": [
    { "name": "3 casillas", "map": ["S..G"], "helpers": "fn corridor(): Int32 { return 3; }" },
    { "name": "5 casillas", "map": ["S....G"], "helpers": "fn corridor(): Int32 { return 5; }" },
    { "name": "7 casillas", "map": ["S......G"], "helpers": "fn corridor(): Int32 { return 7; }" }
  ],
  "mistakes": [ { "replace": ["remaining -= 1;", ""], "limit": true } ] }
```

```zirk starter 3.3
import { stdout } from std.io;

fn main(): Void {
    walk(3);
}
```

```zirk solution 3.3
import { stdout } from std.io;

fn main(): Void {
    mut remaining = corridor();
    while remaining > 0 {
        walk(1);
        remaining -= 1;
    }
}
```

**Pistas:** (1) ¿Cuántas veces hay que avanzar? Lo dice `corridor()`. (2) Guarda ese número en una
variable `mut` y baja de uno en uno. (3) Si nada cambia dentro del bucle, **nunca termina**.
**Errores comunes:** olvidar `remaining -= 1`: el bucle no acaba y el juego lo corta por exceder el
límite de instrucciones (`limit-exceeded`).

### Nivel 3.4 — Contar con for

**Concepto:** `for i in 0..{n}` repite exactamente `n` veces; el rango `0..n` va de 0 hasta `n - 1`.
**Historia:** una fila de monedas, y no sabes cuántas hay hasta que el sensor lo dice.
**Objetivo:** recoger todas las monedas de la fila y llegar a la meta.
**Assets:** `coin`, `bot-zirk`, `goal-portal`.
**Notas de arte:** `coin` (con el logo de Zirk), `bot-zirk`, `goal-portal`.

> Cuando el final del rango es una **variable** y no un número escrito, va entre llaves:
> `0..{n}`. Así el compilador distingue el rango del `{` que abre el bloque.

```level
{ "id": "3.4", "type": "grid", "prelude": ["core"], "facing": "east",
  "map": ["SccG"],
  "cases": [
    { "name": "2 monedas", "map": ["SccG"], "helpers": "fn coins_in_row(): Int32 { return 2; }" },
    { "name": "3 monedas", "map": ["ScccG"], "helpers": "fn coins_in_row(): Int32 { return 3; }" },
    { "name": "5 monedas", "map": ["ScccccG"], "helpers": "fn coins_in_row(): Int32 { return 5; }" }
  ],
  "mistakes": [ { "replace": ["0..{n}", "0..n"], "diagnostic": "E0466" } ] }
```

```zirk starter 3.4
import { stdout } from std.io;

fn main(): Void {
    walk(1);
    collect();
    walk(1);
    collect();
    walk(1);
}
```

```zirk solution 3.4
import { stdout } from std.io;

fn main(): Void {
    inmut n = coins_in_row();
    for i in 0..{n} {
        walk(1);
        collect();
    }
    walk(1);
}
```

**Pistas:** (1) Avanzar y recoger se repite `n` veces. (2) `for i in 0..{n} { ... }` repite el
bloque `n` veces. (3) Después del bucle falta un último paso hasta la meta.
**Errores comunes:** escribir `0..n` sin llaves (`E0466`: *the end of a range must be enclosed in `{...}`*).
**★★★:** resolverlo también con el `for` clásico `for mut i = 0; i < n; i++ { ... }`.

### Nivel 3.5 — Saltar y parar

**Concepto:** `continue` salta a la siguiente vuelta del bucle; `break` sale del bucle del todo.
**Historia:** el contador de la cinta solo debe mostrar los números impares, y apagarse tras el 7.
**Objetivo:** imprimir `1`, `3`, `5`, `7`, uno por línea, recorriendo los números del 1 al 10.
**Assets:** `tile-conveyor`, `terminal`.

```level
{ "id": "3.5", "type": "output", "prelude": ["core"],
  "expect": ["1", "3", "5", "7"],
  "mistakes": [ { "replace": ["continue;", "break;"], "wrongResult": true } ] }
```

```zirk starter 3.5
import { stdout } from std.io;

fn main(): Void {
    for n in 1..=10 {
        stdout.println(n);
    }
}
```

```zirk solution 3.5
import { stdout } from std.io;

fn main(): Void {
    for n in 1..=10 {
        if n % 2 == 0 {
            continue;
        }
        if n > 7 {
            break;
        }
        stdout.println(n);
    }
}
```

**Pistas:** (1) `n % 2 == 0` es verdadero para los pares. (2) Con `continue` no se llega al `println`.
(3) Con `break` se sale del bucle: úsalo cuando `n` pase de 7.
**Nota del rango:** `1..=10` **incluye** el 10; `1..10` terminaría en el 9.
**★★★:** mover la condición del `break` al principio del bucle y comprobar que la salida es la misma.

### Nivel 3.6 — Bucles dentro de bucles

**Concepto:** un bucle puede contener otro; el de dentro da todas sus vueltas por cada vuelta del de fuera.
**Historia:** la escalera de la cubierta se dibuja con asteriscos, peldaño a peldaño.
**Objetivo:** imprimir una escalera de cuatro peldaños.
**Assets:** `terminal`, `tile-plain`.

```level
{ "id": "3.6", "type": "output", "prelude": ["core"],
  "expect": ["*", "**", "***", "****"],
  "mistakes": [ { "replace": ["stdout.println();", ""], "wrongResult": true } ] }
```

```zirk starter 3.6
import { stdout } from std.io;

fn main(): Void {
    for row in 1..=4 {
        stdout.println("*");
    }
}
```

```zirk solution 3.6
import { stdout } from std.io;

fn main(): Void {
    for row in 1..=4 {
        for col in 1..={row} {
            stdout.print("*");
        }
        stdout.println();
    }
}
```

**Pistas:** (1) El peldaño `row` lleva `row` asteriscos. (2) Un segundo bucle, dentro del primero,
imprime los asteriscos sin saltar de línea. (3) Al terminar cada peldaño, `stdout.println()` sin
argumentos salta la línea.
**Errores comunes:** olvidar el salto de línea: todos los asteriscos quedan en una sola fila.

### Nivel 3.7 — Elegir con match

**Concepto:** `match` compara un valor con varios casos; `_` recoge todo lo demás.
**Historia:** el escáner del suelo devuelve un código de casilla: 0 suelo, 1 agua, 2 muro.
**Objetivo:** imprimir `suelo`, `agua`, `muro` o `desconocido` según `tile()`.
**Assets:** `tile-plain`, `tile-water`, `wall`.

```level
{ "id": "3.7", "type": "output", "prelude": ["core"],
  "cases": [
    { "name": "0", "helpers": "fn tile(): Int32 { return 0; }", "expect": ["suelo"] },
    { "name": "1", "helpers": "fn tile(): Int32 { return 1; }", "expect": ["agua"] },
    { "name": "2", "helpers": "fn tile(): Int32 { return 2; }", "expect": ["muro"] },
    { "name": "9", "helpers": "fn tile(): Int32 { return 9; }", "expect": ["desconocido"] }
  ],
  "mistakes": [ { "replace": ["        _ => stdout.println(\"desconocido\");\n", ""], "wrongResult": true } ] }
```

```zirk starter 3.7
import { stdout } from std.io;

fn main(): Void {
    match tile() {
        0 => stdout.println("suelo");
    }
}
```

```zirk solution 3.7
import { stdout } from std.io;

fn main(): Void {
    match tile() {
        0 => stdout.println("suelo");
        1 => stdout.println("agua");
        2 => stdout.println("muro");
        _ => stdout.println("desconocido");
    }
}
```

**Pistas:** (1) Cada caso es `valor => acción;`. (2) Añade los casos 1 y 2. (3) `_` es el caso que atrapa
cualquier otro valor: pon `desconocido` ahí.
**Errores comunes:** no cubrir el caso `_`: un código inesperado no imprime nada.

### Nivel 3.8 — Jefe: el guardián de la cubierta

**Concepto:** repaso — bucle, condiciones y `%` trabajando juntos.
**Historia:** un robot guardián bloquea la salida del sector y solo se aparta con una secuencia
secreta del 1 al 15: los múltiplos de 3 se dicen `zi`, los de 5 `rk`, y los de ambos `zirk`.
**Objetivo:** imprimir la secuencia, una entrada por línea.
**Assets:** `bot-guard`, `door`, `bot-zirk`.
**Notas de arte:** `bot-guard` (el jefe), `door`, `bot-zirk` (`celebrate` al abrirse).

```level
{ "id": "3.8", "type": "output", "prelude": ["core"],
  "expect": ["1", "2", "zi", "4", "rk", "zi", "7", "8", "zi", "rk", "11", "zi", "13", "14", "zirk"],
  "mistakes": [ { "replace": ["if n % 15 == 0", "if n % 3 == 0"], "wrongResult": true } ] }
```

```zirk starter 3.8
import { stdout } from std.io;

fn main(): Void {
    for n in 1..=15 {
        stdout.println(n);
    }
}
```

```zirk solution 3.8
import { stdout } from std.io;

fn main(): Void {
    for n in 1..=15 {
        if n % 15 == 0 {
            stdout.println("zirk");
        } else if n % 3 == 0 {
            stdout.println("zi");
        } else if n % 5 == 0 {
            stdout.println("rk");
        } else {
            stdout.println(n);
        }
    }
}
```

**Pistas:** (1) Un número es múltiplo de 3 si `n % 3 == 0`. (2) ¿Qué caso hay que comprobar **primero**
para que 15 no se diga `zi`? (3) Los múltiplos de 3 **y** 5 son los múltiplos de 15.
**Errores comunes:** comprobar `% 3` antes que `% 15`: el 15 sale como `zi`.
**★★★:** resolverlo sin repetir `println` en cada rama (un `if` que devuelve valor).

---

## 5. Sector 4 — Taller

*Un taller de piezas reutilizables. Escribir lo mismo cinco veces es lento y propenso a
errores: aquí Zirk aprende a darle nombre a un trozo de programa y a usarlo cuantas veces haga falta.*

### Nivel 4.1 — Tu primera función

**Concepto:** `fn nombre(): Void { ... }` agrupa instrucciones bajo un nombre; se usa **llamándola**.
**Historia:** el robot del taller repite siempre el mismo movimiento: avanzar dos y girar a la derecha.
**Objetivo:** llegar a la meta definiendo una función `side` y llamándola tres veces.
**Assets:** `bot-builder`, `goal-portal`, `tile-circuit`.
**Notas de arte:** `bot-builder` (con su llave inglesa), `goal-portal`, `tile-circuit`.

```level
{ "id": "4.1", "type": "grid", "prelude": ["core"], "facing": "east",
  "map": ["S..",
          "##.",
          "G.."],
  "mistakes": [ { "replace": ["    side();\n    side();\n    side();", "    side();\n    side();"], "wrongResult": true } ] }
```

```zirk starter 4.1
import { stdout } from std.io;

fn side(): Void {
    // avanza dos casillas y gira a la derecha
}

fn main(): Void {
    // llama a side() las veces que haga falta
}
```

```zirk solution 4.1
import { stdout } from std.io;

fn side(): Void {
    walk(2);
    turn_right();
}

fn main(): Void {
    side();
    side();
    side();
}
```

**Pistas:** (1) Dentro de `side` va lo que el robot hace en cada lado. (2) Para usar una función, escribe su
nombre con paréntesis: `side();`. (3) El camino tiene tres lados.
**Errores comunes:** definir la función pero no llamarla: no ocurre nada.
**★★★:** compararlo con el nivel 1.2: el mismo recorrido con la mitad de líneas en `main`.

### Nivel 4.2 — Funciones con parámetros

**Concepto:** un **parámetro** es un dato que la función recibe: `fn nombre(pasos: Int32)`.
**Historia:** el robot tiene que recoger monedas a distancias distintas; una función que avance siempre
dos casillas no sirve.
**Objetivo:** recoger las dos monedas y llegar a la meta usando una función con un parámetro.
**Assets:** `coin`, `bot-builder`.

```level
{ "id": "4.2", "type": "grid", "prelude": ["core"], "facing": "east",
  "map": ["S.c..c.G"],
  "mistakes": [ { "replace": ["walk(steps);", "walk(2);"], "wrongResult": true } ] }
```

```zirk starter 4.2
import { stdout } from std.io;

fn walk_and_collect(): Void {
    walk(2);
    collect();
}

fn main(): Void {
    walk_and_collect();
}
```

```zirk solution 4.2
import { stdout } from std.io;

fn walk_and_collect(steps: Int32): Void {
    walk(steps);
    collect();
}

fn main(): Void {
    walk_and_collect(2);
    walk_and_collect(3);
    walk(2);
}
```

**Pistas:** (1) La primera moneda está a 2 casillas y la segunda a 3 más. (2) Añade un parámetro
`steps: Int32` y úsalo dentro de la función. (3) Al llamar, pasa el valor: `walk_and_collect(3)`.
**Errores comunes:** olvidar usar el parámetro dentro del cuerpo (la función siempre avanza lo mismo).

### Nivel 4.3 — Devolver un valor

**Concepto:** una función puede **devolver** un resultado con `return`; su tipo va tras los dos puntos:
`fn double(n: Int32): Int32`.
**Historia:** la máquina de duplicar del taller recibe un número y devuelve el doble.
**Objetivo:** que la máquina duplique 1, 4 y 10.
**Assets:** `lever`, `chip`, `bot-builder`.
**Notas de arte:** `lever`, `chip` (la máquina de duplicar), `bot-builder`.

```level
{ "id": "4.3", "type": "output", "prelude": ["core"],
  "expect": ["2 8 20"],
  "mistakes": [ { "replace": ["return n * 2;", "n * 2;"], "diagnostic": "E0407" } ] }
```

```zirk starter 4.3
import { stdout } from std.io;

fn double(n: Int32): Int32 {
    return n;
}

fn main(): Void {
    stdout.println(double(1), double(4), double(10));
}
```

```zirk solution 4.3
import { stdout } from std.io;

fn double(n: Int32): Int32 {
    return n * 2;
}

fn main(): Void {
    stdout.println(double(1), double(4), double(10));
}
```

**Pistas:** (1) Ahora mismo la función devuelve el mismo número que recibe. (2) `return` entrega el
valor y termina la función. (3) El doble es `n * 2`.
**Errores comunes:** calcular pero olvidar `return`: la función promete un `Int32` y no lo entrega
(`E0407`: *not every path of `double` returns a value*, con la ayuda *add a `return` at the end*).

### Nivel 4.4 — Argumentos con nombre y valores por defecto

**Concepto:** un parámetro puede tener un valor por defecto (`times: Int32 = 2`) y se puede llamar a una
función **por nombre**, en cualquier orden.
**Historia:** el altavoz del taller saluda a quien entra; normalmente dos veces, pero a veces una.
**Objetivo:** saludar a Ana con el valor por defecto y a Zirk una sola vez.
**Assets:** `terminal`, `lamp`, `bot-builder`.
**Notas de arte:** `terminal`, `lamp` (el altavoz), `bot-builder`.

```level
{ "id": "4.4", "type": "output", "prelude": ["core"],
  "expect": ["hola Ana", "hola Ana", "hola Zirk"],
  "mistakes": [ { "replace": ["times: 1, name: \"Zirk\"", "times: 1, nombre: \"Zirk\""], "diagnostic": "E0416" } ] }
```

```zirk starter 4.4
import { stdout } from std.io;

fn greet(name: String, times: Int32 = 2): Void {
    for i in 0..{times} {
        stdout.println("hola", name);
    }
}

fn main(): Void {
    greet("Ana");
}
```

```zirk solution 4.4
import { stdout } from std.io;

fn greet(name: String, times: Int32 = 2): Void {
    for i in 0..{times} {
        stdout.println("hola", name);
    }
}

fn main(): Void {
    greet("Ana");
    greet(times: 1, name: "Zirk");
}
```

**Pistas:** (1) Sin indicar `times`, se usa el valor por defecto. (2) Para elegirlo, escribe el nombre
del parámetro: `times: 1`. (3) Con nombres, el orden no importa.
**Errores comunes:** escribir mal el nombre del parámetro (`E0416`: *`greet` has no parameter named `nombre`*,
y la ayuda lista los nombres válidos: `name, times`).

### Nivel 4.5 — Una función que se llama a sí misma

**Concepto:** la **recursión**: una función que se llama a sí misma necesita un **caso base** que la detenga.
**Historia:** la cuenta atrás de lanzamiento debe contar solo hasta cero.
**Objetivo:** imprimir `3`, `2`, `1` y `¡Despegue!`, y detenerse.
**Assets:** `battery`, `terminal`.
**Notas de arte:** `battery` (se descarga), `terminal`.

```level
{ "id": "4.5", "type": "fix", "prelude": ["core"], "starterRuntime": "ZB102",
  "expect": ["3", "2", "1", "¡Despegue!"],
  "mistakes": [ { "replace": ["    if n == 0 {\n        stdout.println(\"¡Despegue!\");\n        return;\n    }\n", ""], "limit": true } ] }
```

```zirk starter 4.5
import { stdout } from std.io;

fn countdown(n: Int32): Void {
    stdout.println(n);
    countdown(n - 1);
}

fn main(): Void {
    countdown(3);
}
```

```zirk solution 4.5
import { stdout } from std.io;

fn countdown(n: Int32): Void {
    if n == 0 {
        stdout.println("¡Despegue!");
        return;
    }
    stdout.println(n);
    countdown(n - 1);
}

fn main(): Void {
    countdown(3);
}
```

**Qué ve el jugador con el código inicial:** la cuenta sigue en `0`, `-1`, `-2`… hasta que el juego la corta
por exceder el límite de profundidad de llamadas (`limit-exceeded`, `call-depth`).
**Pistas:** (1) ¿Cuándo debería dejar de llamarse la función? (2) Un `if` al principio puede terminar la
función con `return;`. (3) El caso base es `n == 0`: imprime `¡Despegue!` y vuelve.

### Nivel 4.6 — Funciones guardadas en variables

**Concepto:** una **lambda** es una función sin nombre que se guarda en una variable:
`inmut triple = (n: Int32): Int32 => n * 3;` y se llama como cualquier otra.
**Historia:** el taller tiene una pieza pequeña que triplica; no merece una función con nombre propio.
**Objetivo:** que la pieza triplique 5 y 2.
**Assets:** `chip`, `bot-builder`.
**Notas de arte:** `chip` (la pieza que triplica), `bot-builder`.

```level
{ "id": "4.6", "type": "output", "prelude": ["core"],
  "expect": ["15", "6"],
  "mistakes": [ { "replace": ["n * 3", "n + 3"], "wrongResult": true } ] }
```

```zirk starter 4.6
import { stdout } from std.io;

fn main(): Void {
    inmut triple = (n: Int32): Int32 => n;
    stdout.println(triple(5));
    stdout.println(triple(2));
}
```

```zirk solution 4.6
import { stdout } from std.io;

fn main(): Void {
    inmut triple = (n: Int32): Int32 => n * 3;
    stdout.println(triple(5));
    stdout.println(triple(2));
}
```

**Pistas:** (1) Lo que va después de `=>` es lo que la lambda devuelve. (2) Ahora devuelve `n` sin
cambiarlo. (3) Triplicar es multiplicar por 3.

### Nivel 4.7 — Lo que una lambda recuerda

**Concepto:** una lambda **captura** los valores de las variables de su alrededor **en el momento de crearse**;
cambiar la variable después no cambia lo que la lambda recuerda.
**Historia:** la pieza se programa con un bono de 10 y luego el taller sube el bono a 20.
**Objetivo (predicción):** ¿qué imprime el programa?
**Assets:** `battery`, `chip`.

```level
{ "id": "4.7", "type": "predict", "prelude": ["core"],
  "options": ["11", "21", "20"],
  "answer": 0 }
```

```zirk program 4.7
import { stdout } from std.io;

fn main(): Void {
    mut bonus = 10;
    inmut add_bonus = (n: Int32): Int32 => n + bonus;
    bonus = 20;
    stdout.println(add_bonus(1));
}
```

**Explicación al revelar:** la lambda guardó el valor `10` cuando se creó; luego `bonus` pasó a `20`, pero
la lambda sigue usando `10`, así que `add_bonus(1)` es `11`. Es el mismo comportamiento de copia que verás
con los valores simples en el sector 5.

### Nivel 4.8 — Funciones que reciben funciones

**Concepto:** una función puede recibir **otra función** como parámetro; su tipo se escribe
`Fn(Int32) => Int32` («recibe un `Int32` y devuelve un `Int32`»).
**Historia:** el taller tiene una prensa que aplica dos veces cualquier pieza que se le acople: el
mismo mecanismo, distintas herramientas.
**Objetivo:** que `apply_twice` aplique la función **dos** veces: la lambda `triple` a 2 da `18`, la función
`inc` a 5 da `7` y una lambda que eleva al cuadrado, a 3, da `81`.
**Assets:** `bot-builder`, `lever`, `cloner`.
**Notas de arte:** `bot-builder`, `lever` (acoplar la pieza), `cloner` como máquina de repetir.

```level
{ "id": "4.8", "type": "output", "prelude": ["core"],
  "expect": ["18", "7", "81"],
  "mistakes": [ { "replace": ["return f(f(x));", "return f(x);"], "wrongResult": true } ] }
```

```zirk starter 4.8
import { stdout } from std.io;

fn apply_twice(f: Fn(Int32) => Int32, x: Int32): Int32 {
    return f(x);
}

fn inc(n: Int32): Int32 {
    return n + 1;
}

fn main(): Void {
    inmut triple = (n: Int32): Int32 => n * 3;
    stdout.println(apply_twice(triple, 2));
    stdout.println(apply_twice(inc, 5));
    stdout.println(apply_twice((n: Int32): Int32 => n * n, 3));
}
```

```zirk solution 4.8
import { stdout } from std.io;

fn apply_twice(f: Fn(Int32) => Int32, x: Int32): Int32 {
    return f(f(x));
}

fn inc(n: Int32): Int32 {
    return n + 1;
}

fn main(): Void {
    inmut triple = (n: Int32): Int32 => n * 3;
    stdout.println(apply_twice(triple, 2));
    stdout.println(apply_twice(inc, 5));
    stdout.println(apply_twice((n: Int32): Int32 => n * n, 3));
}
```

**Pistas:** (1) Ahora `apply_twice` aplica `f` una sola vez. (2) El resultado de la primera aplicación se vuelve a
pasar a `f`. (3) `f(f(x))`.
**Idea para conectar:** una función es un valor más: se guarda en una variable (4.6), se recuerda con lo que
captura (4.7) y se pasa a otras funciones (4.8).

---

## 6. Sector 5 — Almacén

*Un almacén lleno de estanterías, listas y cajas etiquetadas. Aquí Zirk aprende a guardar
muchos datos, y lo más importante del sector: **qué pasa cuando copias algo**. Un número se
copia; una lista **se comparte**. Entender esa diferencia evita los errores más confusos de
la programación.*

> Los niveles 5.6 a 5.12 son el núcleo del tema «valor frente a referencia». Se apoyan en
> la metáfora visual del juego: un valor es **una caja con su propio contenido**; una
> referencia es **una etiqueta pegada a una caja compartida** (`reference-link` entre dos robots).

### Nivel 5.1 — Estantes fijos: Array

**Concepto:** un `Array<T>` guarda un número **fijo** de elementos del mismo tipo; se leen y se cambian
por posición (`arr[0]` es el primero) y `arr.length` dice cuántos hay.
**Historia:** el estante del almacén tiene exactamente tres huecos.
**Objetivo:** cambiar el segundo elemento (posición 1) a `99` e imprimir el estante y su tamaño.
**Assets:** `crate`, `object-panel`.
**Notas de arte:** `crate` (uno por hueco), `object-panel`.

```level
{ "id": "5.1", "type": "output", "prelude": ["core"],
  "expect": ["[10, 99, 30] 3"],
  "mistakes": [ { "replace": ["arr[1] = 99;", "arr[3] = 99;"], "runtime": "BoundsError" } ] }
```

```zirk starter 5.1
import { stdout } from std.io;

fn main(): Void {
    mut arr: Array<Int32> = Array(10, 20, 30);
    stdout.println(arr, arr.length);
}
```

```zirk solution 5.1
import { stdout } from std.io;

fn main(): Void {
    mut arr: Array<Int32> = Array(10, 20, 30);
    arr[1] = 99;
    stdout.println(arr, arr.length);
}
```

**Pistas:** (1) Las posiciones empiezan en 0: la segunda es la 1. (2) `arr[1] = 99;` cambia el elemento.
(3) Hazlo antes del `println`.
**Errores comunes:** usar una posición que no existe (`arr[3]` en un array de 3): el programa se detiene con
`BoundsError` (*collection index is out of bounds*). Un `Array` **no** puede crecer: no tiene `add`.

### Nivel 5.2 — La lista que crece: List

**Concepto:** una `List<T>` crece y se encoge: `add` añade, `remove(valor)` quita el primer elemento igual.
**Historia:** la mochila del robot cambia a lo largo de la misión.
**Objetivo:** empezar con `pala`, añadir `cuerda` y `linterna`, soltar la `pala`, e imprimir la lista y su tamaño.
**Assets:** `chest`, `crate`.

```level
{ "id": "5.2", "type": "output", "prelude": ["core"],
  "expect": ["[cuerda, linterna] 2"],
  "mistakes": [ { "replace": ["bag.remove(\"pala\");", ""], "wrongResult": true } ] }
```

```zirk starter 5.2
import { stdout } from std.io;

fn main(): Void {
    mut bag: List<String> = List("pala");
    stdout.println(bag, bag.length);
}
```

```zirk solution 5.2
import { stdout } from std.io;

fn main(): Void {
    mut bag: List<String> = List("pala");
    bag.add("cuerda");
    bag.add("linterna");
    bag.remove("pala");
    stdout.println(bag, bag.length);
}
```

**Pistas:** (1) `bag.add("...")` añade al final. (2) `bag.remove("pala")` quita ese elemento. (3) Imprime al final.
**★★★:** recorrer la lista con `for item in bag { ... }` y imprimir cada objeto en su propia línea.

### Nivel 5.3 — Buscar por clave: Map

**Concepto:** un `Map<K, V>` asocia **claves** con **valores**: `set` guarda, `get_or_null` busca (devuelve `null`
si no existe, y `??` da un valor de repuesto).
**Historia:** el panel de taquillas guarda el contenido de cada número de taquilla.
**Objetivo:** guardar la taquilla 1 («uno») y la 2 («dos»), y consultar la 2 y la 3.
**Assets:** `chest`, `object-panel`.
**Notas de arte:** `chest` etiquetados, `object-panel`.

```level
{ "id": "5.3", "type": "output", "prelude": ["core"],
  "expect": ["dos no hay 2"],
  "mistakes": [ { "replace": ["?? \"no hay\"", ""], "wrongResult": true } ] }
```

```zirk starter 5.3
import { stdout } from std.io;

fn main(): Void {
    mut lockers: Map<Int32, String> = Map();
    lockers.set(1, "uno");
    stdout.println(lockers.length);
}
```

```zirk solution 5.3
import { stdout } from std.io;

fn main(): Void {
    mut lockers: Map<Int32, String> = Map();
    lockers.set(1, "uno");
    lockers.set(2, "dos");
    stdout.println(lockers.get_or_null(2) ?? "nada", lockers.get_or_null(3) ?? "no hay", lockers.length);
}
```

**Pistas:** (1) `lockers.set(clave, valor)` guarda. (2) `get_or_null(3)` devuelve `null` si no hay taquilla 3.
(3) `a ?? b` usa `b` cuando `a` es `null`.
**Errores comunes:** olvidar el `??`: se imprime `<Null>` donde no hay valor.

### Nivel 5.4 — Sin repetidos: Set

**Concepto:** un `Set<T>` guarda **elementos únicos**: añadir uno que ya está no hace nada.
**Historia:** el escáner de la puerta registra cada carné una sola vez, aunque pase varias veces.
**Objetivo:** registrar los carnés 3, 5, 3, 7 y 5, e imprimir cuántos distintos hay y si están el 5 y el 4.
**Assets:** `plate`, `terminal`.

```level
{ "id": "5.4", "type": "output", "prelude": ["core"],
  "expect": ["3 true false"],
  "mistakes": [ { "replace": ["Set<Int32>", "List<Int32>"], "diagnostic": "E0403" } ] }
```

```zirk starter 5.4
import { stdout } from std.io;

fn main(): Void {
    mut badges: Set<Int32> = Set();
    badges.add(3);
    stdout.println(badges.length);
}
```

```zirk solution 5.4
import { stdout } from std.io;

fn main(): Void {
    mut badges: Set<Int32> = Set();
    badges.add(3);
    badges.add(5);
    badges.add(3);
    badges.add(7);
    badges.add(5);
    stdout.println(badges.length, badges.contains(5), badges.contains(4));
}
```

**Pistas:** (1) Añade los cinco carnés, repetidos incluidos. (2) El `Set` descarta los duplicados solo.
(3) `contains(x)` pregunta si un elemento está.

### Nivel 5.5 — Varios valores juntos: tuplas

**Concepto:** una **tupla** `(3, 4)` junta valores de tipos distintos; se **desempaqueta** con
`mut (x, y) = tupla;`.
**Historia:** la posición del robot es un par de coordenadas.
**Objetivo:** desempaquetar la posición y imprimir `x: 3 y: 4 suma: 7`.
**Assets:** `start-pad`, `object-panel`.

```level
{ "id": "5.5", "type": "output", "prelude": ["core"],
  "expect": ["x: 3 y: 4 suma: 7"],
  "mistakes": [ { "replace": ["mut (x, y) = position;", "mut (x) = position;"], "diagnostic": "E0403" } ] }
```

```zirk starter 5.5
import { stdout } from std.io;

fn main(): Void {
    inmut position = (3, 4);
    stdout.println(position);
}
```

```zirk solution 5.5
import { stdout } from std.io;

fn main(): Void {
    inmut position = (3, 4);
    mut (x, y) = position;
    stdout.println("x:", x, "y:", y, "suma:", x + y);
}
```

**Pistas:** (1) `mut (x, y) = position;` crea dos variables de una vez. (2) Después se usan como cualquier
variable. (3) `x + y` es la suma.

### Nivel 5.6 — Copiar un número

**Concepto:** los valores simples (`Int32`, `Boolean`, `Char`, `Decimal`…) **se copian**: cada variable tiene
su propia caja.
**Historia:** el robot anota el voltaje, hace una copia y sube la copia.
**Objetivo (predicción):** ¿qué imprime el programa?
**Assets:** `chip`.
**Notas de arte:** dos `chip` con la misma cifra; solo cambia el de la copia.

```level
{ "id": "5.6", "type": "predict", "prelude": ["core"],
  "options": ["4 4", "3 4", "3 3"],
  "answer": 1 }
```

```zirk program 5.6
import { stdout } from std.io;

fn main(): Void {
    mut x = 3;
    mut y = x;
    y += 1;
    stdout.println(x, y);
}
```

**Explicación al revelar:** `y = x` **copia** el valor; después, cambiar `y` no toca a `x`. Dos cajas, cada una
con su `3`; una pasa a `4`.

### Nivel 5.7 — Dos nombres, una lista

**Concepto:** una `List` es una **referencia**: asignarla a otra variable no copia la lista, crea **otro nombre
para la misma lista**.
**Historia:** dos robots comparten la misma caja de herramientas aunque la llamen distinto.
**Objetivo (predicción):** ¿qué imprime el programa?
**Assets:** `reference-link`, `chest`.
**Notas de arte:** `reference-link` entre dos robots y un solo `chest`.

```level
{ "id": "5.7", "type": "predict", "prelude": ["core"],
  "options": ["[1, 2] [1, 2, 3]", "[1, 2, 3] [1, 2, 3]", "[1, 2] [1, 2]"],
  "answer": 1 }
```

```zirk program 5.7
import { stdout } from std.io;

fn main(): Void {
    mut a: List<Int32> = List(1, 2);
    mut b = a;
    b.add(3);
    stdout.println(a, b);
}
```

**Explicación al revelar:** `b = a` no copia la lista: `a` y `b` son **dos etiquetas de la misma caja**. Añadir
con `b` cambia lo que ve `a`. Esto es la **referencia**.

### Nivel 5.8 — Una copia de verdad: clone()

**Concepto:** `clone()` crea una **copia independiente**: cambiar la copia no afecta al original.
**Historia:** antes de arriesgarse en la zona inestable, el robot guarda una copia de seguridad de su
inventario. La copia de seguridad no debe cambiar cuando cambia el inventario.
**Objetivo:** que la copia siga con `[pala, cuerda]` mientras el inventario pasa a `[pala, cuerda, linterna]`.
**Assets:** `cloner`, `bot-ghost`, `chest`.
**Notas de arte:** `cloner` (barra de progreso), `bot-ghost`, `chest`.

```level
{ "id": "5.8", "type": "fix", "prelude": ["core"],
  "expect": ["[pala, cuerda] [pala, cuerda, linterna]"],
  "mistakes": [ { "replace": ["inventory.clone()", "inventory"], "wrongResult": true } ] }
```

```zirk starter 5.8
import { stdout } from std.io;

fn main(): Void {
    mut inventory: List<String> = List("pala", "cuerda");
    mut backup = inventory;
    inventory.add("linterna");
    stdout.println(backup, inventory);
}
```

```zirk solution 5.8
import { stdout } from std.io;

fn main(): Void {
    mut inventory: List<String> = List("pala", "cuerda");
    mut backup = inventory.clone();
    inventory.add("linterna");
    stdout.println(backup, inventory);
}
```

**Qué ve el jugador con el código inicial:** `[pala, cuerda, linterna] [pala, cuerda, linterna]`: la «copia de
seguridad» cambió porque era la misma lista.
**Pistas:** (1) ¿`backup` es de verdad otra lista? (2) Asignar una lista no la copia. (3) `inventory.clone()`.

### Nivel 5.9 — Los textos también se comparten

**Concepto:** un `String` también es una referencia: dos variables que apuntan al mismo texto lo comparten, y
cambiar una letra se ve en las dos; `clone()` lo separa.
**Historia:** una etiqueta grabada se copia por error en dos taquillas.
**Objetivo (predicción):** ¿qué imprime el programa?
**Assets:** `sign`, `reference-link`.
**Notas de arte:** `sign` (la etiqueta grabada), `reference-link`.

```level
{ "id": "5.9", "type": "predict", "prelude": ["core"],
  "options": ["hello Hello Jello", "Hello Hello Jello", "hello hello Jello"],
  "answer": 1 }
```

```zirk program 5.9
import { stdout } from std.io;

fn main(): Void {
    mut s1 = "hello";
    mut s2 = s1;
    s2[0] = 'H';
    mut s3 = s1.clone();
    s3[0] = 'J';
    stdout.println(s1, s2, s3);
}
```

**Explicación al revelar:** `s2 = s1` comparte el texto: al cambiar la primera letra por `H`, `s1` también la ve.
`s3` es una copia (`clone`) hecha **después**, ya con `Hello`, y su `J` solo cambia a `s3`.

### Nivel 5.10 — Igual no es lo mismo que idéntico

**Concepto:** `==` pregunta si dos valores **tienen el mismo contenido**; `is` pregunta si son **el mismo objeto**.
**Historia:** el inspector distingue dos etiquetas con el mismo texto de una etiqueta vista dos veces.
**Objetivo (predicción):** ¿qué imprime el programa?
**Assets:** `reference-link`.
**Notas de arte:** `reference-link` (sale entre `a` y `c`, no entre `a` y `b`).

```level
{ "id": "5.10", "type": "predict", "prelude": ["core"],
  "options": ["true true true", "true false true", "false false true"],
  "answer": 1 }
```

```zirk program 5.10
import { stdout } from std.io;

fn main(): Void {
    mut a: String = "hi";
    mut b = a.clone();
    mut c = a;
    stdout.println(a == b, a is b, a is c);
}
```

**Explicación al revelar:** `a == b` es `true` (mismo contenido); `a is b` es `false` (la copia es otro objeto);
`a is c` es `true` (`c` es otra etiqueta del mismo objeto). Los números no tienen identidad: `is` solo
se usa con referencias.

### Nivel 5.11 — Qué recibe una función

**Concepto:** una función recibe los **valores** simples **copiados**, pero una **lista compartida**; por eso
puede modificar la lista, y un número que recibe no cambia el original.
**Historia:** dos máquinas del taller reciben el mismo paquete: una lo fotocopia, la otra lo abre.
**Objetivo (predicción):** ¿qué imprime el programa?
**Assets:** `reference-link`, `chest`.

```level
{ "id": "5.11", "type": "predict", "prelude": ["core"],
  "options": ["[1] 1 2", "[1, 9] 1 2", "[1, 9] 2 2"],
  "answer": 1 }
```

```zirk program 5.11
import { stdout } from std.io;

fn grow(items: List<Int32>): Void {
    items.add(9);
}

fn plus_one(n: Int32): Int32 {
    return n + 1;
}

fn main(): Void {
    mut list: List<Int32> = List(1);
    mut n = 1;
    grow(list);
    mut m = plus_one(n);
    stdout.println(list, n, m);
}
```

**Explicación al revelar:** `grow` recibe la **misma** lista y le añade un 9: `list` ahora es `[1, 9]`. `plus_one`
recibe una **copia** de `n` y devuelve otro valor: `n` sigue valiendo `1` y `m` vale `2`.

### Nivel 5.12 — Los parámetros son de solo lectura

**Concepto:** dentro de una función no se puede **reasignar** un parámetro; para «cambiar» un número se devuelve
el resultado.
**Historia:** el cargador intenta sumar uno al número que recibió, pero ese número no es suyo.
**Objetivo:** que `add_one` devuelva el valor aumentado y el programa imprima `2`.
**Assets:** `battery`, `lever`.

```level
{ "id": "5.12", "type": "fix", "prelude": ["core"], "starterDiagnostic": "E0404",
  "expect": ["2"] }
```

```zirk starter 5.12
import { stdout } from std.io;

fn add_one(n: Int32): Void {
    n += 1;
}

fn main(): Void {
    mut v = 1;
    add_one(v);
    stdout.println(v);
}
```

```zirk solution 5.12
import { stdout } from std.io;

fn add_one(n: Int32): Int32 {
    return n + 1;
}

fn main(): Void {
    mut v = 1;
    v = add_one(v);
    stdout.println(v);
}
```

**Diagnóstico real:** `error[E0404]: cannot reassign 'n'`.
**Pistas:** (1) El error dice que `n` no se puede reasignar. (2) La función puede **devolver** el nuevo valor.
(3) Quien llama guarda el resultado: `v = add_one(v);`.

### Nivel 5.13 — Jefe: el inventario del almacén

**Concepto:** repaso — listas, bucles, condiciones y acumuladores trabajando juntos.
**Historia:** el almacén recibe un lote de cajas con pesos distintos y el robot encargado tiene que
informar del peso total, de la caja más pesada y de cuántas pasan de 10.
**Objetivo:** imprimir `total T max M pesadas P` para cualquier lote.
**Assets:** `bot-courier`, `crate`, `object-panel`.
**Notas de arte:** `bot-courier` (el encargado), `crate` (con tamaños distintos), `object-panel`.

```level
{ "id": "5.13", "type": "output", "prelude": ["core"],
  "cases": [
    { "name": "lote grande", "helpers": "fn weights(): List<Int32> { return List(4, 12, 7, 20, 3); }", "expect": ["total 46 max 20 pesadas 2"] },
    { "name": "lote pequeño", "helpers": "fn weights(): List<Int32> { return List(1, 2); }", "expect": ["total 3 max 2 pesadas 0"] }
  ],
  "mistakes": [ { "replace": ["w > 10", "w > 12"], "wrongResult": true } ] }
```

```zirk starter 5.13
import { stdout } from std.io;

fn main(): Void {
    mut total = 0;
    for w in weights() {
        total += w;
    }
    stdout.println("total", total);
}
```

```zirk solution 5.13
import { stdout } from std.io;

fn main(): Void {
    mut total = 0;
    mut max = 0;
    mut heavy = 0;
    for w in weights() {
        total += w;
        if w > max {
            max = w;
        }
        if w > 10 {
            heavy += 1;
        }
    }
    stdout.println("total", total, "max", max, "pesadas", heavy);
}
```

**Pistas:** (1) Necesitas tres acumuladores: total, máximo y cantidad de pesadas. (2) Dentro del bucle, cada
condición actualiza el acumulador que corresponde. (3) Imprime tras el bucle.
**★★★:** resolverlo con una sola pasada por la lista (como arriba) y sin variables de más.

---

## 7. Sector 6 — Archivo

*El archivo de la estación guarda fichas, estados y resultados. Aquí Zirk aprende a darle
**forma** a los datos con `record` y `enum`, y a manejar fallos como un valor más con `Result`,
sin que el programa se estrelle.*

### Nivel 6.1 — Fichas con nombre: record

**Concepto:** un `record` agrupa campos con nombre en un solo tipo: `record Robot { name: String; energy: Int32; }`.
Se crea con los campos por nombre y se compara con `==` por contenido.
**Historia:** el archivo guarda la ficha de cada robot.
**Objetivo:** crear dos fichas con los mismos datos e imprimir el nombre, la energía y si son iguales.
**Assets:** `object-panel`, `chip`.
**Notas de arte:** `object-panel` (una barra por campo), `chip`.

```level
{ "id": "6.1", "type": "output", "prelude": ["core"],
  "expect": ["Zirk 90 true"],
  "mistakes": [ { "replace": ["Robot(name: \"Zirk\", energy: 90);\n    stdout", "Robot(\"Zirk\", 90);\n    stdout"], "diagnostic": "E0416" } ] }
```

```zirk starter 6.1
import { stdout } from std.io;

record Robot { name: String; energy: Int32; }

fn main(): Void {
    inmut a = Robot(name: "Zirk", energy: 90);
    stdout.println(a.name, a.energy);
}
```

```zirk solution 6.1
import { stdout } from std.io;

record Robot { name: String; energy: Int32; }

fn main(): Void {
    inmut a = Robot(name: "Zirk", energy: 90);
    inmut b = Robot(name: "Zirk", energy: 90);
    stdout.println(a.name, a.energy, a == b);
}
```

**Pistas:** (1) Crea otra ficha con los mismos datos. (2) Los campos de un `record` se dan **por nombre**.
(3) `a == b` compara el contenido de las dos fichas.
**Errores comunes:** pasar los campos sin nombre (`E0416`: *`Robot` takes only named arguments*).

### Nivel 6.2 — Los records no se editan

**Concepto:** un `record` es un **valor**: sus campos no se reasignan; para «cambiar» uno se crea otro record
nuevo.
**Historia:** el archivo nunca borra una ficha; si cambia una posición, archiva una nueva.
**Objetivo:** obtener `Point(x: 1, y: 2)` y `Point(x: 9, y: 2)`.
**Assets:** `object-panel`, `chip`.

```level
{ "id": "6.2", "type": "fix", "prelude": ["core"], "starterDiagnostic": "E0404",
  "expect": ["Point(x: 1, y: 2) Point(x: 9, y: 2)"] }
```

```zirk starter 6.2
import { stdout } from std.io;

record Point { x: Int32; y: Int32; }

fn main(): Void {
    mut a = Point(x: 1, y: 2);
    mut b = a;
    b.x = 9;
    stdout.println(a, b);
}
```

```zirk solution 6.2
import { stdout } from std.io;

record Point { x: Int32; y: Int32; }

fn main(): Void {
    mut a = Point(x: 1, y: 2);
    mut b = Point(x: 9, y: a.y);
    stdout.println(a, b);
}
```

**Diagnóstico real:** `error[E0404]: cannot assign to 'x'`.
**Pistas:** (1) El error dice que no se puede asignar a `x`. (2) Crea un `Point` nuevo en vez de editar.
(3) Reutiliza el campo que no cambia: `a.y`.
**Idea para conectar:** a diferencia de una `List` (nivel 5.7), copiar un `record` es seguro: nadie puede
cambiarlo a escondidas.

### Nivel 6.3 — Estados con nombre: enum

**Concepto:** un `enum` es un tipo con un **conjunto cerrado de valores**; `match` sobre un `enum` debe cubrir
**todos** los casos y el compilador lo comprueba.
**Historia:** el semáforo de la estación como tipo propio, no como números sueltos.
**Objetivo:** imprimir `alto`, `cuidado` o `avanza` según `light_now()`.
**Assets:** `lamp`, `plate`.

```level
{ "id": "6.3", "type": "output", "prelude": ["core"], "starterDiagnostic": "E0415",
  "cases": [
    { "name": "rojo", "helpers": "fn light_now(): Light { return Light.Red; }", "expect": ["alto"] },
    { "name": "amarillo", "helpers": "fn light_now(): Light { return Light.Yellow; }", "expect": ["cuidado"] },
    { "name": "verde", "helpers": "fn light_now(): Light { return Light.Green; }", "expect": ["avanza"] }
  ] }
```

```zirk starter 6.3
import { stdout } from std.io;

enum Light { Red, Yellow, Green }

fn main(): Void {
    match light_now() {
        Light.Red => stdout.println("alto");
    }
}
```

```zirk solution 6.3
import { stdout } from std.io;

enum Light { Red, Yellow, Green }

fn main(): Void {
    match light_now() {
        Light.Red => stdout.println("alto");
        Light.Yellow => stdout.println("cuidado");
        Light.Green => stdout.println("avanza");
    }
}
```

**Diagnóstico real:** `error[E0415]: the match does not cover every case of Light`, con la ayuda
*these variants have no arm: Yellow, Green*.
**Pistas:** (1) El compilador te dice qué variantes faltan. (2) Cada una necesita su rama. (3) También
valdría `_` para «todo lo demás», pero aquí conviene nombrarlas.
**Por qué importa:** si mañana se añade una luz al `enum`, el compilador señalará todos los `match` que
olvidaron cubrirla.

### Nivel 6.4 — Enums con datos

**Concepto:** cada variante de un `enum` puede llevar **datos propios**; `match` los extrae:
`Shape.Rect(w, h) => w * h`.
**Historia:** el archivo guarda figuras de distinto tipo y necesita su área.
**Objetivo:** completar `area` para que imprima `10 12 0`.
**Assets:** `blueprint`.
**Notas de arte:** `blueprint` (una figura distinta por variante).

```level
{ "id": "6.4", "type": "output", "prelude": ["core"], "starterDiagnostic": "E0415",
  "expect": ["10 12 0"],
  "mistakes": [ { "replace": ["Shape.Rect(w, h) => w * h,", "Shape.Rect(w, h) => w + h,"], "wrongResult": true } ] }
```

```zirk starter 6.4
import { stdout } from std.io;

enum Shape { Circle(r: Int32), Rect(w: Int32, h: Int32), Dot }

fn area(s: Shape): Int32 {
    return match s {
        Shape.Circle(r) => r * r * 3
    };
}

fn main(): Void {
    stdout.println(area(Shape.Rect(w: 2, h: 5)), area(Shape.Circle(r: 2)), area(Shape.Dot));
}
```

```zirk solution 6.4
import { stdout } from std.io;

enum Shape { Circle(r: Int32), Rect(w: Int32, h: Int32), Dot }

fn area(s: Shape): Int32 {
    return match s {
        Shape.Circle(r) => r * r * 3,
        Shape.Rect(w, h) => w * h,
        Shape.Dot => 0
    };
}

fn main(): Void {
    stdout.println(area(Shape.Rect(w: 2, h: 5)), area(Shape.Circle(r: 2)), area(Shape.Dot));
}
```

**Pistas:** (1) Faltan las ramas `Rect` y `Dot`. (2) El `match` que devuelve un valor lleva comas entre ramas.
(3) `Dot` no lleva datos: su área es `0`.

### Nivel 6.5 — Un resultado que puede fallar: Result

**Concepto:** `Int32.parse(texto)` no se estrella si el texto no es un número: devuelve un `Result` que es
`Ok(valor)` o `Error(motivo)`; `match` decide qué hacer en cada caso.
**Historia:** el lector de tarjetas recibe un código escrito a mano; algunos están dañados.
**Objetivo:** imprimir el código más uno si es válido, y `tarjeta dañada` si no.
**Assets:** `terminal`, `bot-scout`.

```level
{ "id": "6.5", "type": "output", "prelude": ["core"],
  "cases": [
    { "name": "42", "helpers": "fn card(): String { return \"42\"; }", "expect": ["ok 43"] },
    { "name": "x7", "helpers": "fn card(): String { return \"x7\"; }", "expect": ["tarjeta dañada"] }
  ],
  "mistakes": [ { "replace": ["Error(e) => stdout.println(\"tarjeta dañada\");", ""], "diagnostic": "E0415" } ] }
```

```zirk starter 6.5
import { stdout } from std.io;

fn main(): Void {
    stdout.println(Int32.parse(card()));
}
```

```zirk solution 6.5
import { stdout } from std.io;

fn main(): Void {
    match Int32.parse(card()) {
        Ok(v) => stdout.println("ok", v + 1);
        Error(e) => stdout.println("tarjeta dañada");
    }
}
```

**Pistas:** (1) `parse` devuelve `Ok(...)` o `Error(...)`. (2) Un `match` con una rama por cada uno.
(3) En `Ok(v)`, `v` es el número ya convertido.
**Errores comunes:** olvidar la rama `Error`: el compilador exige cubrir ambos casos (`E0415`).

### Nivel 6.6 — Fallar sin estrellarse

**Concepto:** una función puede **declarar que falla** devolviendo `Result<T, E>`: así quien la usa está
obligado a decidir qué hacer ante el error.
**Historia:** la calculadora del archivo se bloquea al dividir por cero y apaga la estación.
**Objetivo:** imprimir `2` y luego `no se puede dividir por cero`, sin que el programa se detenga.
**Assets:** `terminal`, `bot-scout`.

```level
{ "id": "6.6", "type": "fix", "prelude": ["core"], "starterRuntime": "DivisionByZeroError",
  "expect": ["2", "no se puede dividir por cero"] }
```

```zirk starter 6.6
import { stdout } from std.io;

fn div(a: Int32, b: Int32): Int32 {
    return Int32(a / b);
}

fn main(): Void {
    stdout.println(div(6, 3));
    stdout.println(div(1, 0));
}
```

```zirk solution 6.6
import { stdout } from std.io;

fn safe_div(a: Int32, b: Int32): Result<Int32, String> {
    if b == 0 {
        return Error("no se puede dividir por cero");
    }
    return Ok(Int32(a / b));
}

fn show(result: Result<Int32, String>): Void {
    match result {
        Ok(v) => stdout.println(v);
        Error(e) => stdout.println(e);
    }
}

fn main(): Void {
    show(safe_div(6, 3));
    show(safe_div(1, 0));
}
```

**Qué ve el jugador con el código inicial:** `2` y después el programa se detiene con `DivisionByZeroError`.
**Pistas:** (1) ¿Qué debería devolver `div` cuando `b` vale 0? (2) `Result<Int32, String>` es «un `Int32`, o un
texto explicando el fallo». (3) Quien llama usa `match` para tratar los dos casos.
**★★★:** escribir `show` una sola vez y reutilizarla para las dos llamadas (como arriba).

---

## 8. Nivel final — Bahía de lanzamiento

*Para despegar, la nave ejecuta un **programa** guardado como una lista de comandos de texto. La bahía
no tiene intérprete: Zirk va a escribir el suyo.*

### Nivel 7.1 — Construye un intérprete

**Concepto:** repaso de todo el curso — una `List` de textos, un bucle `for`, `match` sobre textos, funciones del
juego y un caso de error.
**Historia:** el ordenador de a bordo guarda la ruta de salida como una lista de comandos. Hay que leerla y
ejecutarla paso a paso: `walk` avanza, `turn` gira, `grab` recoge, y cualquier otra cosa es una avería.
**Objetivo:** ejecutar el programa de la nave: recoger la moneda y llegar al portal.
**Assets:** `bot-zirk`, `terminal`, `coin`, `goal-portal`, `start-pad`.
**Notas de arte:** `bot-zirk` (`celebrate` en la meta), `terminal`, `coin`, `goal-portal`, `start-pad`.

```level
{ "id": "7.1", "type": "grid", "prelude": ["core"], "facing": "east",
  "map": ["S.c",
          "##.",
          "G.."],
  "cases": [ { "name": "ruta de la nave", "helpers": "fn program(): List<String> { return List(\"walk\", \"walk\", \"grab\", \"turn\", \"walk\", \"walk\", \"turn\", \"walk\", \"walk\"); }" } ],
  "mistakes": [ { "replace": ["\"turn\" => turn_right();", "\"turn\" => turn_left();"], "wrongResult": true } ] }
```

```zirk starter 7.1
import { stdout } from std.io;

fn main(): Void {
    for command in program() {
        stdout.println(command);
    }
}
```

```zirk solution 7.1
import { stdout } from std.io;

fn main(): Void {
    for command in program() {
        match command {
            "walk" => walk(1);
            "turn" => turn_right();
            "grab" => collect();
            _ => stdout.println("avería: comando desconocido", command);
        }
    }
}
```

**Pistas:** (1) `program()` devuelve la lista de comandos; recórrela con `for`. (2) Un `match` sobre el texto del
comando elige qué función llamar. (3) `_` recoge los comandos que no existen: imprime un aviso.
**★★★:** que un comando desconocido **no** detenga el resto del programa, y que el aviso diga cuál fue.

---

## 9. Progresión de conceptos

Cada concepto se introduce una vez y se **reutiliza** después. Si un nivel necesita algo que no se ha
enseñado, el guión está mal.

| Concepto | Se introduce en | Se reutiliza en |
| --- | --- | --- |
| `fn main`, `stdout.println` | 1.1 | todos |
| Orden de ejecución | 1.2 | 3.x, 4.1 |
| `print` frente a `println` | 1.3 | 3.6 |
| Leer un diagnóstico | 1.4 | 2.2, 2.5, 2.6, 4.3, 4.4, 5.12, 6.x |
| `println` con varios valores | 1.5 | 2.x, 5.x |
| `mut` y reasignación | 2.1 | 3.3, 5.13 |
| `inmut` | 2.2 | 5.5, 6.1 |
| Tipos primitivos y `.type` | 2.3 | 2.4 a 2.12 |
| Texto frente a número | 2.4, 2.5 | 2.10 |
| Tamaño de los enteros y desbordamiento | 2.6, 2.7 | 5.1 (límites de un array) |
| División, `%`, conversión explícita | 2.8 | 3.5, 3.8, 6.6 |
| `Decimal` exacto frente a binario | 2.9 | — |
| Interpolación, `length`, `[ ]` | 2.10 | 3.6, 5.2 |
| `Boolean`, `&&`, `||`, `!` | 2.11 | 3.1, 3.5 |
| `null`, `T?`, `??` | 2.12 | 5.3, 6.5 |
| `if`, `else`, `else if` | 3.1, 3.2 | 3.5, 3.8, 5.13 |
| `while` | 3.3 | 3.5 (contrastado con `for`) |
| `for` y rangos `..`, `..=` | 3.4 | 3.5, 3.6, 3.8, 4.4 |
| `break`, `continue` | 3.5 | 3.8 |
| Bucles anidados | 3.6 | — |
| `match` sobre valores | 3.7 | 6.3, 7.1 |
| Definir y llamar una función | 4.1 | 4.x en adelante |
| Parámetros | 4.2 | 4.3, 4.4, 5.11 |
| `return` y tipo de retorno | 4.3 | 5.12, 6.6 |
| Argumentos con nombre y por defecto | 4.4 | 6.1 (campos de un `record`) |
| Recursión y caso base | 4.5 | — |
| Lambdas y captura por valor | 4.6, 4.7 | 4.8, 5.6 (la misma idea de copia) |
| Funciones como valores | 4.8 | — |
| `Array`, `List`, `Map`, `Set` | 5.1 a 5.4 | 5.13, 7.1 |
| Tuplas y desestructuración | 5.5 | — |
| **Valor frente a referencia** | 5.6, 5.7 | 5.8 a 5.11 |
| `clone()` | 5.8 | 5.9 |
| `==` frente a `is` | 5.10 | — |
| Parámetros de solo lectura | 5.12 | 6.2 |
| `record` (valor inmutable) | 6.1, 6.2 | — |
| `enum` y `match` exhaustivo | 6.3 | 6.4, 7.1 |
| `enum` con datos | 6.4 | — |
| `Result` y manejo de fallos | 6.5, 6.6 | — |

Los tres hilos que recorren el curso son: **ver el efecto** (grid), **predecir antes de ejecutar** (predict) y
**leer lo que dice el compilador** (fix).

---

## 10. Sistema de ayuda, puntuación y recompensas

### 10.1 Pistas

Cada nivel tiene tres pistas que el jugador desbloquea de una en una (la tercera tarda más en ofrecerse):

1. **Orientar:** una pregunta que apunta a la idea.
2. **Explicar:** el concepto con un ejemplo mínimo.
3. **Encaminar:** la forma de la solución, sin darla entera.

En los niveles `fix` el diagnóstico real del compilador se muestra **siempre**, con su código y la línea
resaltada, antes de cualquier pista.

### 10.2 Estrellas y monedas

| Premio | Condición |
| --- | --- |
| ★ | Pasar el nivel |
| ★★ | Pasarlo sin abrir la tercera pista |
| ★★★ | Cumplir el reto extra del nivel (`par` de órdenes, otra técnica, sin repetir código) |
| Moneda | Una por nivel superado; las monedas desbloquean cosméticos del robot, no contenido |
| Gema | Una por sector, al superar su último nivel (1.5, 2.12, 3.8, 4.8, 5.13, 6.6 y 7.1). Los sectores 3 y 5 acaban con un jefe (3.8 y 5.13); el resto, con un nivel de cierre |

Un sector se abre al conseguir **todas las gemas** de los anteriores. Dentro del sector el jugador puede
elegir el orden de los niveles que no sean jefes, salvo cuando el guión marca dependencia.

### 10.3 Reacciones del robot

| Situación | Animación de `bot-zirk` |
| --- | --- |
| Escribiendo | `think` |
| Ejecutando el programa | `walk` o el movimiento que toque |
| Nivel superado | `celebrate` y `createSparkles` en la meta |
| Error de compilación | `sad` y el diagnóstico en el panel |
| Fallo de ejecución (por ejemplo `BoundsError`) | `sad`, parar la reproducción **en el punto del fallo** |
| Predicción acertada | `jump` |

### 10.4 Modo predicción

En los niveles `predict` el programa **no se puede editar**. El jugador elige una de las opciones antes de
ejecutarlo; después se ejecuta de verdad y se muestra la salida, con la explicación del nivel. No hay
penalización por fallar la apuesta: lo que importa es la sorpresa.

---

## 11. Diagnósticos que el curso enseña a leer

El juego muestra el **diagnóstico real** del compilador y le añade una explicación breve. Esta tabla es el
glosario y el verificador exige que sea exacta: cada código está ejercitado por algún nivel o hallazgo, y no
falta ninguno de los que se usan.

```diagnostics
{
  "E0301": "Falta o sobra algo en la sintaxis (un paréntesis, una llave, un nombre).",
  "E0401": "Usaste un nombre que no existe: ¿lo escribiste bien, o falta declararlo?",
  "E0403": "Los tipos no encajan: el compilador no convierte entre tipos a escondidas.",
  "E0404": "Intentas cambiar algo que no se puede cambiar (una variable `inmut`, un parámetro, un campo de `record`).",
  "E0407": "Una función promete devolver un valor pero algún camino termina sin hacerlo.",
  "E0408": "El número no cabe en el tipo elegido; prueba con un tipo más grande.",
  "E0415": "El `match` no cubre todos los casos; el compilador te dice cuáles faltan.",
  "E0416": "Argumentos mal dados: faltan nombres o el nombre no existe.",
  "E0426": "Ese tipo no tiene ese miembro (método o propiedad).",
  "E0466": "El final de un rango que es una variable va entre llaves: `0..{n}`.",
  "BoundsError": "Te saliste de los límites de un `Array` o `List`: esa posición no existe.",
  "DivisionByZeroError": "Dividir entre cero detiene el programa.",
  "ArithmeticOverflowError": "El resultado no cabe en el tipo; Zirk lo detiene en lugar de darte un número falso.",
  "ZB102": "El programa excedió un límite del juego (demasiadas instrucciones o llamadas anidadas)."
}
```

---

## 12. Límites del lenguaje que condicionan el guión

El guión se diseñó contra lo que el compilador **hace hoy**, no contra lo que la especificación promete.
Estos hallazgos están comprobados por el verificador (excepto el último, que es un fallo del compilador que
no se puede mostrar al jugador) y explican varias decisiones del diseño.

**1. `match` no agrupa valores numéricos con coma.** `1, 2 => ...` no compila; hay que escribir una rama por
valor (3.7).

```zirk finding f1
fn main(): Void {
    mut n = 7;
    match n {
        1, 2 => println("chico");
        _ => println("otro");
    }
}
```
```expect f1
{ "diagnostic": "E0301" }
```

**2. Las tuplas no se devuelven ni se indexan.** No existe un tipo `(Int32, Int32)` como retorno ni `t.0`; solo se
crean y se **desestructuran** (5.5).

```zirk finding f2
fn pair(): (Int32, Int32) { return (1, 2); }
fn main(): Void { println(pair()); }
```
```expect f2
{ "diagnostic": "E0301" }
```

**3. `List<Int32>.remove(0)` quita por valor, no por posición.** Con enteros el argumento se interpreta como el
valor a quitar; por eso el nivel 5.2 usa una lista de textos.

```zirk finding f3
fn main(): Void {
    mut l: List<Int32> = List(5, 6, 7);
    println(l.remove(0), l);
}
```
```expect f3
{ "output": ["false [5, 6, 7]"] }
```

**4. `length` es `UInt64`.** Un rango como `0..{arr.length}` no compila contra un contador `Int32`; el guión recorre
colecciones con `for item in coleccion` o con el `for` clásico (`i < arr.length`).

```zirk finding f4
fn main(): Void {
    mut arr: Array<Int32> = Array(1, 2);
    for i in 0..{arr.length} { println(i); }
}
```
```expect f4
{ "diagnostic": "E0403" }
```

**5. `is` solo vale con referencias que tienen identidad.** Un `String` la tiene (5.10); una `List` no, y el
compilador lo dice.

```zirk finding f5
fn main(): Void {
    mut a: List<Int32> = List(1);
    mut b = a;
    println(a is b);
}
```
```expect f5
{ "diagnostic": "E0403" }
```

**6. Un texto no se convierte en número con `Int32("7")`.** Se usa `Int32.parse`, que devuelve un `Result` (6.5).

```zirk finding f6
fn main(): Void { println(Int32("7")); }
```
```expect f6
{ "diagnostic": "E0403" }
```

**7. Los nombres de métodos importan.** `String` tiene `to_uppercase`, no `to_upper`.

```zirk finding f7
fn main(): Void { println("Zirk".to_upper()); }
```
```expect f7
{ "diagnostic": "E0426" }
```

**8. Fallo del compilador: `?.` sobre un `String?` que lee `length`.** `a?.length` hace que el compilador
se detenga con un pánico interno en lugar de dar un error. El guión evita el acceso seguro y enseña `??` y `match`
sobre `null` (2.12). Está comunicado como defecto del compilador, no como comportamiento del lenguaje.

```zirk finding f8
fn main(): Void {
    mut a: String? = "hola";
    println(a?.length);
}
```
```expect f8
{ "compilerPanic": true }
```

**Defectos del entorno nativo que el juego no sufre.** El juego ejecuta el código en el intérprete del navegador
(ADR-020), que sí los respeta, pero conviene saberlos si algún día se compara con el compilador nativo:
`==` sobre dos `List` termina el proceso nativo de forma abrupta, y `Map`/`Set` con claves `String` comparan por
identidad en el runtime nativo. Ambos están registrados en `docs/browser-oop-coverage.md`. Por eso el guión usa
claves y elementos **enteros** en `Map` y `Set` (5.3, 5.4).

---

## 13. Assets y datos que necesita el juego

### 13.1 Assets existentes (`game-assets/`) por sector

| Sector | Personajes | Objetos y metas | Mundo y escenario |
| --- | --- | --- | --- |
| 1 | `bot-zirk` | `terminal`, `start-pad`, `goal-portal`, `battery`, `chip` | `tile-circuit`, `wall`, `pillar` |
| 2 | `bot-zirk` | `chip`, `battery`, `chest`, `door`, `plate`, `coin` | `tile-grass`, `crate`, `sign`, `tree` |
| 3 | `bot-zirk`, `bot-guard` (jefe 3.8) | `door`, `plate`, `lever`, `coin`, `goal-portal` | `tile-conveyor`, `tile-water`, `tile-bridge`, `tile-spikes`, `wall`, `lamp` |
| 4 | `bot-zirk`, `bot-builder` | `lever`, `battery`, `coin` | `tile-circuit`, `crystal`, `pillar` |
| 5 | `bot-zirk`, `bot-courier` (jefe 5.13), `bot-ghost` | `chest`, `crate`, `cloner`, `reference-link`, `object-panel` | `tile-plain`, `fence`, `lamp` |
| 6 | `bot-zirk`, `bot-scout` | `object-panel`, `blueprint`, `terminal` | `tile-teleporter`, `banner`, `pillar` |
| Final | todos | `terminal`, `coin`, `goal-portal` | `tile-circuit` |

### 13.2 Lo que habrá que añadir

- **Panel de datos genérico** para `inspect(etiqueta, valor)`: un chip por valor simple y una barra por número
  (el `object-panel` actual sirve para campos; falta uno de «variable suelta»).
- **Marcadores de tipo:** una forma y un color por tipo (`Int32`, `String`, `Boolean`, `Char`, `Decimal`) para los
  niveles 2.3 y 2.4.
- **Animación de desbordamiento** (el cofre que no cierra) y de `BoundsError` (el estante sin hueco).
- **Cuadrícula editable** para el simulador de los niveles `grid` (mapa a partir de los textos del guión).

### 13.3 Cómo consume el juego este documento

`game-assets/curriculum/export.mjs` convierte el guión en `levels.json` (id, sector, título, concepto, historia,
objetivo, pistas, código inicial, casos, mapa y premios), que es lo que el juego carga. Las soluciones de
referencia **no** se exportan al cliente. Ese mismo `levels.json` alimenta el simulador de la cuadrícula
y el validador de salida del juego, que aplican las mismas reglas que el verificador de este documento.
