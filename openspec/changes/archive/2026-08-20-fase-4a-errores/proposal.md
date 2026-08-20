## Why

`ZIRK_ROADMAP.md` fija la Fase 4 como "Errors and memory" — pero es, en realidad, dos decisiones de tamaño e independencia muy distintos empujadas al mismo número de fase. La estrategia de memoria (GC trazado vs RC vs regiones, `unsafe`/`Pointer<T>`, referencias safe/weak/dependent) es, en palabras del propio roadmap, "la decisión de mayor apalancamiento del proyecto", y [ADR-003](../../../docs/decisions/ADR-003-memoria.md) dice explícitamente que su cierre necesita "un lenguaje con closures y objetos reales que medir" — evaluarla contra programas Zirk de juguete sería exactamente el error que ADR-003 evitó cometer en Fase 0.

El manejo de errores no tiene esa dependencia. `Result<T,E>` es un enum algebraico genérico con dos parámetros — la Fase 3 ya construyó enums algebraicos genéricos con uno (`Iteration<T>`) — y no necesita que la estrategia de memoria esté decidida para existir. Separarlo en su propia fase (`fase-4a`, dejando `fase-4b-excepciones` y la memoria para después) evita bloquear valor real detrás de la decisión más grande y más lenta del proyecto.

Esta fase, además, cubre solo la mitad *esperada* de errores (`Result<T,E>`) de la sección 9 de `ZIRK_LANGUAGE_SPEC.md`, no la mitad *extraordinaria* (`try`/`catch`/`throw`/`Throwable`). Esa segunda mitad necesita una máquina de propagación de excepciones (desenrollado de pila o su equivalente, `finally` que corre en cada salida, `RuntimeError` implícito capturable) que es, otra vez, una pieza propia — construirla apurada junto con `Result` arriesgaría las dos.

### Corrección sobre una supuesta deuda de Fase 3

Un pase anterior de este mismo agente documentó "un enum no puede implementar `to_string()`, ni ningún método" como deuda pendiente de la Fase 3. Es un error: `ZIRK_LANGUAGE_SPEC.md` sección 7 es explícito — "enums are data-only and declare no user methods" — la misma regla que `ZIRK_STDLIB_SPEC.md` y las páginas del handbook sobre enums repiten por separado. `EnumType` sin un campo `methods` (a diferencia de `ClassType`) es correcto a propósito, no una omisión; el comportamiento de dominio de un enum es una función externa que usa `match`, por diseño. `docs/init/ZIRK_AGENT_PROMPT.md` ya se corrigió para reflejar esto.

Esto simplifica el `Result<T,E>` de esta fase: su API (`is_ok`, `map`, `unwrap`, …) no puede ser una tabla de métodos de enum — no existe tal cosa, ni debe — así que se reconoce estructuralmente en el chequeador, exactamente el mismo patrón que `to_string()` explícito sobre un escalar nativo ya usa (`Checker::is_native_to_string_call`/`lower_native_to_string_call`, Fase 3b): por nombre y receptor, no por una tabla de métodos genérica.

## What Changes

### `Base::Never`, el tipo fondo

- Una expresión de tipo `Never` es asignable a cualquier tipo, y unifica con cualquier tipo en un punto de unión (`cond ? 5 : fatalError("...")`).
- Precondición de `fatalError`, que no retorna normalmente.

### `fatalError(message: String): Never`

- Función reconocida por el compilador, no un método — coincide con `ZIRK_LANGUAGE_SPEC.md` sección 9 y con el `zirk_rt_overflow`/`zirk_rt_division_by_zero`/etc. que el runtime ya tiene desde Fase 1, generalizado a un mensaje arbitrario del programa en vez de una causa fija del compilador.

### `Result<T,E>`

- Enum algebraico genérico compilador-conocido: `Ok(T) | Error(E)`, con el mismo mecanismo que registra `Iterable<T>`/`Iterator<T>`/`Iteration<T>` hoy (`docs/ERROR_RESOURCE_PERMISSION_SEMANTICS.md` sección 2, la fuente autorial más reciente sobre errores — tiene precedencia sobre descripciones más antiguas que entren en conflicto).
- API sin parámetro de tipo propio del método — solo los `T`/`E` que la instanciación de `Result` ya trae resueltos: `is_ok`, `is_error`, `ok_or_null`, `error_or_null`, `get_or`, `unwrap`, `unwrap_error`. `unwrap`/`unwrap_error` sobre la variante equivocada invoca `fatalError`, no una excepción — es una aserción del programador, no una falla recuperable (sección 9).
- Consumo obligatorio: un `Result` usado como sentencia de expresión descartada es un error de compilación; `_ = expr;` es el descarte explícito permitido (`docs/handbook/02-handbook/15-errors/02-handling-result.md`).

### Explícitamente fuera de alcance

- **`map`, `map_error`, `and_then`, `or_else`, `or_throw`** — cada uno introduce su propio parámetro de tipo (`map<U>(transform: Fn(T) => U): Result<U,E>`, etc.), inferido del retorno del callback en el sitio de la llamada. `MethodInfo` (`zirk-sema/src/types.rs`) no tiene un campo `type_params` propio hoy — solo la clase/contrato/enum contenedor puede ser genérico, no un método individual — así que esto es una pieza de inferencia de tipos nueva y genuina, no una extensión menor de lo demás. `or_throw` además necesita excepciones, que tampoco existen todavía. Cambio separado, después de este.
- **`get_or_else(factory: Fn() => T): T`** — a diferencia de los otros siete métodos en alcance, su parámetro es un tipo función. `docs/init/ZIRK_AGENT_PROMPT.md` es explícito (decisión D9): "function types have no syntax yet" — un closure infiere su tipo localmente pero no puede anotarse como tipo de parámetro, retorno o campo, en ninguna posición del lenguaje, no solo para `Result`. Fabricar una firma estructural alrededor de esa restricción sería exactamente el atajo que D9 dejó pendiente a propósito para una fase futura. Se mueve aquí, junto a los combinadores genéricos.
- **`try`/`catch`/`finally`/`throw`, la jerarquía `Throwable`/`RuntimeError`/`StackTrace`, `throws` en firmas de función** — `fase-4b-excepciones`, cambio separado.
- **`Resource<E>`/`match with`** — depende de excepciones (sección 9/10 de `ZIRK_STDLIB_SPEC.md`).
- **La estrategia de memoria, `unsafe`/`Pointer<T>`, referencias safe/weak/dependent, `inmut::strict`** — el resto de la Fase 4 del roadmap; necesita su propio proceso de decisión (ADR-003), no encaja en el mismo cambio que esto.
- **Propagación automática (`?`)** — el spec es explícito: "Zirk 1.x has no `?` propagation operator." No es deuda, es una decisión del lenguaje.

## Impact

- Specs afectadas: `zirk-errors` (implementa la mitad de `Result` de los cuatro requisitos que ya documenta; los otros tres — excepciones, `Throwable`, patrones de `catch` — se quedan como estaban, para `fase-4b`), `zirk-type-system` (`Base::Never`, `Result<T,E>` compilador-conocido), `zirk-grammar` (`_ = expr;`), `zirk-ir-lowering`, `zirk-native-codegen`.
- Sin cambios de ruptura: nada de lo que compila hoy deja de hacerlo.
