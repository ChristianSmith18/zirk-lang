## Context

Las Fases 0–2 se construyeron contra una versión anterior de la documentación.
El refinamiento normativo posterior retiró la familia `Decimal`, definió `Char`
como grafema Unicode, fijó `String` como referencia mutable compartida indexada
por grafemas, e incorporó la familia temporal, la conversión contextual profunda
y `inmut::strict` con análisis de aliases.

Una auditoría del código contra las fuentes normativas vigentes deja un
resultado asimétrico:

- **La semántica implementada es correcta.** División truncada hacia cero y
  resto con signo del dividendo salen de `sdiv`/`srem`; el overflow y la
  división por cero son errores controlados mediante intrínsecos de LLVM,
  incluidos `-Int32.MIN` y `Int32.MIN / -1`; no hay truthiness ni conversiones
  implícitas; `T?` es un bit sobre la base y `null` solo habita tipos nulables.
  Nada de esto hay que tocarlo.
- **La superficie léxica está desalineada.** El lexer no reconoce literales que
  el lenguaje tiene, y la tabla de tipos pendientes anuncia tipos que el
  lenguaje ya no tiene.

El caso más grave no es un diagnóstico equivocado sino uno ausente: `1.5` se
tokeniza como `1`, `.`, `5` sin decir nada. El lexer ya tiene la disciplina
correcta para esto —reconocer construcciones de fases posteriores para diferirlas
con nombre y fase— y este cambio la extiende a lo que quedó fuera.

Además, varias características documentadas no aparecen en ninguna fase del
roadmap. Mientras eso siga así, la Fase 3 tiene una puerta abierta para
absorberlas, que es exactamente lo que su propio alcance prohíbe.

## Goals / Non-Goals

**Goals:**

- Que el compilador reconozca todo el vocabulario léxico del lenguaje y difiera
  con nombre y fase lo que no implementa.
- Que las formas de sentencia que la norma define y el parser nunca implementó
  —paréntesis opcionales, `do ... while`, `if` sin llaves, ternario, incremento
  como expresión— pasen a compilar.
- Que la tabla de tipos pendientes describa el lenguaje vigente: sin `Decimal`,
  con `Float` y con la familia temporal.
- Habilitar los aliases `Int` e `Integer`, que nombran un tipo ya implementado.
- Fijar identidad, igualdad y hash de `String` con una implementación que no
  pague normalización en el caso frecuente.
- Dar fase dueña a cada característica documentada que hoy no tiene ninguna.
- Enmendar el ADR-005 y registrar la decisión sobre normalización.

**Non-Goals:**

- Implementar aritmética `Float`, `Char`, la familia temporal, `inmut::strict`,
  interpolación o los operadores bit a bit. Este cambio los hace visibles al
  compilador, no los construye.
- Modificar el alcance de la Fase 3.
- Reescribir la representación interna de `String` o construir el índice de
  grafemas. Eso lo trae la fase que indexe.
- Cambiar el comportamiento de cualquier programa que hoy compile y corra. El
  conjunto de programas válidos no se reduce ni se amplía; solo mejora lo que se
  dice sobre los inválidos.

## Decisions

### D1 — El lexer reconoce el lenguaje completo; el parser difiere lo que falta

El reconocimiento léxico y la disponibilidad de una característica son cosas
distintas, y confundirlas es lo que produce el diagnóstico "carácter no
reconocido: no inicia ningún token del lenguaje" sobre `'`, una afirmación
falsa.

Cada literal, operador y palabra clave nuevos se lexan igual que `class` o
`task`: se convierten en token, y quien los rechaza es la capa que sabe de fases.

La alternativa —reconocerlos solo cuando se implementen— es la que produjo la
situación actual: cada característica documentada pero no lexada es una promesa
de mal diagnóstico.

### D2 — Un literal Float se difiere en el chequeador, no en el lexer

El lexer produce el literal con su valor y su ancho si lo tiene. El diagnóstico
de fase lo emite quien conoce los tipos.

Así, cuando la Fase 3b implemente `Float`, el lexer no cambia: solo se retira un
rechazo. Y mientras tanto el token existe, que es lo que impide la reinterpretación
silenciosa de `1.5`.

### D3 — La interpolación se lexa como estructura, no como texto

Un literal de cadena interpolado produce partes literales y expresiones
incrustadas como elementos distintos, con llaves balanceadas. Guardar las llaves
dentro del texto obligaría a re-lexar la cadena más adelante, y el punto de
partida —a qué offset del archivo corresponde cada expresión— ya se habría
perdido para los diagnósticos.

El balanceo es el mismo que necesita `0..{number}`, así que es un solo mecanismo.

### D4 — `Int` e `Integer` se habilitan; `UInt` no

Un alias es exactamente su objetivo. `Int` e `Integer` son `Int32`, que está
implementado desde la Fase 1: bloquearlos bloqueaba un nombre, no una capacidad.
`UInt` es `UInt32`, que no está implementado, así que sigue difiriéndose junto
con él.

La resolución se hace en el mapeo de nombres a tipos, no creando bases nuevas:
después de resolver, nada distingue un `Int` de un `Int32`, que es justamente lo
que significa ser un alias.

### D5 — `is` compara handles; `==` compara contenido canónico

La regla observable es la que el autor pidió: `"hó" == "hó"` es `true` aunque
una esté en NFC y la otra en NFD, y `is` distingue referentes.

La implementación evita el costo en el caso frecuente, en este orden:

1. Mismo handle → iguales. Es también la respuesta de `is`.
2. Longitudes y bytes idénticos → iguales. Un `memcmp`, sin asignar memoria.
3. Ambos marcados como canónicos y con bytes distintos → distintos.
4. Solo entonces, comparación canónica incremental, sin materializar copias
   normalizadas cuando sea evitable.

El handle guarda, junto a los bytes, banderas y campos cacheados con nombres en
inglés como el resto del código: `is_ascii`, `normalization`, `grapheme_count`,
`hash`. Una cadena ASCII no puede tener formas equivalentes distintas, así que
`is_ascii` corta por el paso 2 siempre.

El hash se deriva de la forma canónica: si se derivara de los bytes, dos claves
iguales por `==` caerían en cubetas distintas y `Map<String, _>` contradiría al
operador.

### D6 — Los literales se normalizan en compilación

El compilador emite los literales ya en forma canónica y marcados como tales.
Es trabajo que se hace una vez, en una máquina que no tiene prisa, y que
convierte la comparación entre literales —el caso abrumadoramente más común— en
comparación de bytes.

La alternativa, normalizar en ejecución, paga en cada comparación un costo que
el source ya conocía.

### D7 — El ADR-005 se enmienda, no se reemplaza

La norma nueva no contradice la opacidad del handle: la confirma. El handle es
la identidad que `is` compara, y ser opaco es lo que permite que grafemas,
normalización y caché vivan enteros dentro del runtime sin que el compilador
tenga que saber de ellos.

La enmienda registra ambas cosas: que el handle es identidad observable, y que
la igualdad y el hash son responsabilidad del runtime.

### D8 — Se introduce la Fase 3b en vez de estirar la Fase 3

Los anchos enteros completos, la familia `Float`, `Char`, la conversión
contextual profunda, los operadores bit a bit y la interpolación forman un
bloque mutuamente dependiente: la conversión contextual profunda no significa
nada sin `Float`, y la interpolación necesita el `to_string()` que la Fase 3
convierte en contrato.

Meterlas en la Fase 3 duplicaría una fase que ya es la más grande del roadmap.
Dejarlas sin fase las convertiría en deuda sin fecha. Una fase propia,
inmediatamente después, es lo que respeta ambas cosas.

`inmut::strict` no entra ahí: exige análisis de aliases, que es el mismo
análisis del modelo de memoria, y por eso va a la Fase 4. La familia temporal va
a la Fase 7, anotada como tipos nativos conocidos por el compilador y no como
objetos de biblioteca —la distinción importa porque el lexer y el chequeador los
conocen antes de que exista `std.time`.

### D9 — El conjunto de programas válidos solo crece

Todo lo que este cambio añade se aplica a source que hoy no compila, o que
compila produciendo algo distinto de lo escrito. El caso límite es `1.5`, que
hoy se lexa como tres tokens: no forma ninguna expresión válida del subset, así
que ningún programa que compile depende de esa lectura.

Las formas de sentencia que D10 a D12 incorporan son adiciones, no
sustituciones: `for (…)` sigue siendo válido. Ningún programa que hoy compile
deja de hacerlo ni cambia de comportamiento.

La validación sale directa de ahí: la batería existente pasa sin modificación,
salvo los tests que fijaban explícitamente lo que se corrige.

### D10 — Los paréntesis del header son opcionales, y eso hay que escribirlo

El parser exige paréntesis en el `for` tradicional. La norma lo escribe sin
ellos. No es que una fuente esté equivocada: es que **la regla real del lenguaje
no estaba escrita en ninguna parte**, y ante el silencio el implementador
asumió lo que conocía de C.

La regla es que el header de cualquier estructura de control —`if`, `while`,
`for`, `for ... in`, `do ... while`, `match`— admite paréntesis opcionales, y
que la forma canónica los omite.

Para `if`, `while` y `match` esto ya funciona sin tocar nada: `(cond)` es una
expresión entre paréntesis y el parser no distingue. El trabajo real está en el
`for` tradicional, cuyo header tiene tres partes y hoy exige el delimitador.

Sin paréntesis, lo que cierra el header es la `{` del bloque. Zirk no tiene
literales con llave en posición de expresión —los records se construyen como
`Type(...)`— así que no aparece la ambigüedad que obliga a otros lenguajes a
prohibir esta forma.

Se escribe la regla en `ZIRK_LANGUAGE_SPEC.md` §5 y como requisito de
`zirk-grammar`. Escribirla es más importante que implementarla: es la ausencia
de la regla, y no su contenido, lo que produjo la divergencia.

### D11 — `do ... while`, el `if` sin llaves y el ternario se implementan aquí

Las tres son gramática pura: no introducen tipos, contratos ni reglas de
mutabilidad nuevas. `do ... while` es un bucle cuya condición se evalúa al
final; el `if` sin llaves gobierna exactamente una sentencia; el ternario es una
expresión con las ramas que el `if` expresión ya sabe unificar.

Entran en este cambio porque el parser hay que abrirlo igual por D10, y una
segunda pasada por la misma gramática costaría más que hacerlo junto.

El `if` sin llaves gobierna **una** sentencia y no admite `else`: con `else`
volvería la ambigüedad del *dangling else*, y la norma lo presenta como forma de
efecto (`if closed return;`), no como condicional completo.

### D12 — El incremento pasa a ser expresión, porque su motivo caducó

La Fase 2 difirió `++` y `--` como expresiones con este motivo, escrito en el
parser: *su distinción prefijo/postfijo necesitaría un orden de evaluación que
ningún documento normativo define*.

Era correcto entonces. Ya no: `ZIRK_LANGUAGE_SPEC.md` §4 exige preservar la
semántica convencional de postfijo y prefijo, que es exactamente el orden que
faltaba. `count++` evalúa al valor previo e incrementa; `++count` incrementa y
evalúa al nuevo.

Además D10 lo vuelve inevitable: el `for` canónico de la norma es
`for mut i = 0; i < 10; i++ { }`, y su paso es precisamente un incremento.

Ambas formas exigen un lugar asignable y mutable, la misma condición que ya
comprueba la asignación compuesta.

## Risks / Trade-offs

- **[Reconocer sin implementar puede leerse como una promesa]** → El diagnóstico
  nombra la fase, no una fecha. Es la misma convención que el lexer ya usa desde
  la Fase 1 con `class` y `task`, y que nadie ha leído como promesa de plazo.

- **[La comparación canónica requiere datos Unicode]** → El caso frecuente no la
  toca (D5, pasos 1–3), y los literales llegan normalizados (D6). Si el costo en
  binario resultara relevante, la tabla se acota a lo que la equivalencia
  canónica necesita, que es mucho menos que Unicode completo.

- **[La Fase 3b retrasa la Fase 4]** → Solo en el papel. Ese trabajo hay que
  hacerlo igual; hoy estaba escondido en un roadmap que no lo nombraba, que es
  peor que estar planificado.

- **[Marcar el estado de normalización puede desincronizarse de los bytes]** →
  El estado es privado del runtime y solo se fija donde se construye el handle.
  Ninguna ruta que modifique bytes existe todavía; cuando la fase que muta
  `String` la introduzca, invalidar los campos cacheados —`normalization`,
  `grapheme_count`, `hash`— es parte de esa ruta y de sus tests.

- **[Las formas de sentencia nuevas sí llegan al binario]** → Es la única parte
  del cambio que toca IR y codegen, y rompe la limpieza de "solo la puerta de
  entrada". Se acota con la regla de D11: nada que introduzca tipos o contratos
  entra aquí. `do ... while` baja a los mismos bloques básicos que `while` con
  el salto inicial invertido, y el ternario a los mismos que el `if` expresión;
  ninguno estrena forma de IR.

- **[El `if` sin llaves invita al *dangling else*]** → No admite `else` (D11).
  Un `else` tras un `if` sin llaves es un error con diagnóstico propio, no una
  ambigüedad que resolver por precedencia.

- **[La interpolación lexada sin semántica deja una estructura sin consumidor]**
  → Es deliberado: la estructura la produce el lexer y la consume la Fase 3b. El
  costo es una representación viva sin uso; el beneficio es que la Fase 3b no
  tenga que volver a tocar el lexer.

## Migration Plan

1. Enmendar ADR-005 y registrar la decisión de identidad, igualdad y
   normalización de `String`. Las decisiones van antes que el código.
2. Actualizar `ZIRK_ROADMAP.md` con la Fase 3b y la asignación de las
   características huérfanas. Sin esto, las fases que los tokens nuevos declaran
   no tendrían a qué referirse.
3. Lexer: tokens y palabras clave, luego reconocedores de literales.
4. Chequeador: tabla de tipos pendientes, aliases, elemento de `for ... in`.
5. Runtime: igualdad, hash y normalización de literales.
6. Verificar que la batería existente pasa sin modificación, salvo los tests que
   fijaban explícitamente el comportamiento corregido.

No hay rollback que planificar: el cambio no altera formatos persistidos ni el
binario generado. Revertir el commit basta.

## Open Questions

- ¿La Fase 3b es una fase propia o una segunda mitad numerada de la Fase 3? Se
  propone fase propia por tamaño; es una decisión de roadmap, no técnica, y no
  bloquea nada de este cambio. La Fase 7b sigue la misma convención por el mismo
  motivo: numerarla como 8 correría las fases 8 a 12, que existen desde que se
  escribió el roadmap.
- ¿El formateador debe **normalizar** los paréntesis opcionales a la forma
  canónica, o respetar lo que el autor escribió? La regla de D10 dice cuál es
  canónica, no si el formateador la impone. Se decide en la Fase 9, que es la
  que trae el formateador.
- ¿Los sufijos de ancho en literales (`1.5f32`) admiten también forma entera
  (`42i64`)? `04-literals.md` menciona "sufijo" para ambas familias sin fijar la
  ortografía entera. Se lexa lo documentado y se deja la extensión a la Fase 3b,
  que es la que la necesita.
