## Context

Cada fase anterior trabajó con exactamente los escalares que necesitaba y nada más: la Fase 1 fijó `Int32`/`Boolean`/`String` opaco porque el pipeline mínimo no pedía más, y ninguna fase posterior volvió a tocarlos. `ZIRK_LANGUAGE_SPEC.md` sección 3 siempre prometió la familia completa; esta es la primera fase que la construye.

El riesgo central es de alcance real, no de dificultad conceptual: `IrType::Int32` (y su equivalente en el checker, `Base::Int32`) está escrito como si fuera el único entero posible en cientos de sitios de `zirk-sema`, `zirk-ir` y `zirk-codegen-llvm` — cada `match` sobre un tipo, cada conversión a LLVM, cada regla de aritmética. Generalizar a diez anchos enteros más `Float` no es agregar variantes nuevas a un lado; es revisar cada uno de esos sitios para confirmar que deja de asumir `Int32`.

ADRs vigentes que restringen esta fase:

| Decisión | ADR |
|---|---|
| `String` es opaco tras la frontera del runtime | [ADR-005](../../../docs/decisions/ADR-005-representacion-string.md) |
| Identidad, igualdad y normalización de `String` | [ADR-011](../../../docs/decisions/ADR-011-identidad-e-igualdad-de-string.md) |
| Forma de la IR: tres direcciones, bloques básicos, slots | [ADR-007](../../../docs/decisions/ADR-007-forma-de-la-ir.md) |
| Layout de objetos: cabecera separada, records/value classes inline | [ADR-012](../../../docs/decisions/ADR-012-layout-de-objetos.md) |
| Forma del despacho: directo por defecto, tabla solo si hace falta | [ADR-013](../../../docs/decisions/ADR-013-forma-del-despacho.md) |

## Goals / Non-Goals

**Goals:**

- Los diez anchos enteros con y sin signo, con aritmética comprobada igual de estricta que la que `Int32` ya tiene.
- La familia `Float` completa, sin `NaN` válido, con infinitos explícitos.
- `Char` como un grapheme Unicode real, no un alias de un entero de 32 bits.
- Conversión contextual profunda (`Float(3 / 4)`) y las reglas de ensanchamiento implícito/explícito de la sección 3.
- Operadores bitwise y de shift.
- El contrato `to_string()` (`ZIRK_STDLIB_SPEC.md` sección 3), `print`/`println` ruteando por él, e interpolación de strings (`"{expr}"`, misma sección).

**Non-Goals:**

- `Decimal`, tipos temporales, cualquier cosa de `std.collections` — no los pide esta fase.
- `format`/`format_dynamic` con especificadores (`:name`, `:0`, `:name|format`) y `Regex` — `ZIRK_STDLIB_SPEC.md` sección 7 los deja para cuando exista `std.text` como módulo, que es superficie de biblioteca, no del lenguaje.
- Wrapping/saturating aritmético explícito como operación — el spec no lo pide todavía fuera de la conversión contextual.
- Cualquier normalización o algoritmo Unicode más allá de reconocer límites de grapheme para `Char`.

## Decisions

### D1 — Un entero es un ancho más una señal, no diez tipos distintos en la IR

`IrType::Int32` se generaliza a algo con dos parámetros —ancho y con/sin signo— en vez de multiplicarse en diez variantes de enum. LLVM ya modela cualquier ancho de entero de forma nativa (`IntType`), así que el backend no gana complejidad real: gana un parámetro donde antes tenía una constante.

El costo se paga en el checker y en `lower.rs`: cada sitio que hoy compara contra `Base::Int32`/`IrType::Int32` directamente necesita revisarse — algunos deben generalizar a "cualquier entero", otros (los que hoy asumen 32 bits porque nunca hubo otra opción, como el tamaño de un discriminante de enum) necesitan una decisión explícita de qué ancho usar. Se audita exhaustivamente al empezar la implementación, no se asume que "generalizar el tipo" basta.

**Alternativa descartada:** una variante de IR por ancho (`Int8`, `Int16`, …). Multiplica cada `match` exhaustivo por diez sin ninguna ganancia — el ancho es un dato, no una forma distinta de instrucción.

**Auditoría (tarea 1.1, hecha):** conteo de sitios que nombran `Base::Int32`/`IrType::Int32` directamente, por archivo:

| Archivo | Ocurrencias | Naturaleza |
|---|---|---|
| `zirk-sema/src/types.rs` | 4 | `Type::INT32` (constante), `has_default`, `integer_range`, `sort_bases` — lógica genuinamente por tipo, no renombrado mecánico |
| `zirk-sema/src/checker.rs` | ~25 sitios con `match ... .base { ... }` que incluyen un brazo `Base::Int32` | Mezcla: la mayoría son "¿es esto un entero?" (generaliza sin más), pero `native_arithmetic` (la tabla de operadores nativos) y `require_printable` necesitan decidir explícitamente qué anchos participan y con qué reglas de conversión |
| `zirk-ir/src/ir.rs` | 6 | Definiciones/helpers de tipo — mecánico |
| `zirk-ir/src/lower.rs` | 30 | Conversión `Base`→`IrType` y contabilidad de tipos de operandos — mayormente mecánico, salvo la comprobación de overflow que necesita el ancho real |
| `zirk-ir/src/verify.rs` | 4 | Chequeo de invariantes de la IR — mecánico |
| `zirk-codegen-llvm/src/emit.rs` | **1** | `IrType::Int32 => context.i32_type().into()` — el backend en sí es barato: LLVM ya soporta cualquier ancho, este es el único sitio que traduce el tipo |

**Conclusión de la auditoría:** el costo no está repartido parejo. El backend LLVM es casi gratis (un sitio). El costo real está en `zirk-sema/checker.rs`: `Base::Int32` no es hoy "el entero", es "el único entero que existe", así que la tabla de aritmética nativa, la impresión, el valor por defecto y el orden de interning de uniones lo asumen estructuralmente, no por descuido. Generalizar `Base::Int32` a `Base::Int { width, signed }` (o equivalente) es un cambio que el propio compilador de Rust hace imposible dejar a medias — cada `match` exhaustivo sobre `Base` que no cubra el nuevo brazo no compila — así que no hay riesgo real de un sitio olvidado, solo de subestimar cuánto trabajo de **decisión** (no de mecánica) cada sitio necesita.

**Decisión de alcance (tarea 1.2):** una sola migración atómica — parametrizar `Base::Int32`/`IrType::Int32` una vez, con los diez anchos ya declarados desde el principio — en vez de generalizar primero y agregar anchos después. Agregar anchos en dos pasadas pagaría el costo de revisar cada sitio dos veces; la exhaustividad del compilador ya garantiza que ningún sitio queda a medias en una sola pasada.

### D2 — `Float` prohíbe `NaN` en el tipo, no lo descarta después

Una operación que en IEEE 754 produciría `NaN` (`0.0 / 0.0`, la raíz de un negativo, etc.) se convierte en un error controlado en el punto donde ocurre, igual que el overflow de un entero ya lo es desde la Fase 1 — no se deja que el bit pattern de `NaN` se propague y se comprueba al final. Los infinitos, en cambio, son valores válidos y explícitos.

Esto significa que cada operación de `Float` que LLVM expondría con semántica IEEE 754 estándar necesita su propia comprobación antes o después del `fdiv`/`fsub`/etc. — no es una validación que se pueda centralizar en un solo punto, porque cada operador puede producir `NaN` por una razón distinta.

**Alternativa descartada:** representar `NaN` como valor válido y rechazar solo al observarlo (comparación, impresión). El spec es explícito ("no valid NaN") y esperar hasta el punto de observación permite que un `NaN` viva arbitrariamente en el programa antes de fallar, con el diagnóstico apuntando lejos de su causa real.

### D3 — Aritmética comprobada por ancho, reutilizando el mecanismo de `Int32`

La Fase 1 ya construyó el camino de overflow comprobado para `Int32` (checker más soporte de runtime). Cada ancho nuevo reutiliza exactamente ese mecanismo, parametrizado por el ancho y la señal — no una implementación paralela. Firmado y sin firmar difieren en qué comparación de overflow usan (LLVM expone intrínsecos `*.with.overflow` para ambos), no en la forma del chequeo.

## Risks / Trade-offs

- **`Char` como grapheme real no cabe en un escalar de tamaño fijo, en general.** Un grapheme cluster extendido (una base más marcas combinantes, o una secuencia de emoji unida por ZWJ) puede ocupar arbitrariamente más de 4 bytes UTF-8. La sección 3 es explícita: "even when composed of multiple code points and bytes". Esto es una **pregunta abierta real, no una decisión tomada** — ver más abajo. Mitigación provisional: diseñar `Char` como una vista opaca respaldada por el runtime, con el mismo principio que ADR-005 ya aplicó a `String` (frontera opaca, sin representación LLVM directa que el frontend necesite conocer), en vez de forzarlo a cualquier ancho fijo antes de tener la respuesta.

- **Generalizar `IrType::Int32` es el riesgo de alcance más grande de esta fase.** No hay forma de medir cuántos sitios asumen 32 bits sin auditarlos uno por uno; la estimación previa a esa auditoría no es confiable. Mitigación: la auditoría es la primera tarea de implementación, antes de escribir ningún ancho nuevo — si el costo real excede lo esperado, se recorta alcance (por ejemplo, entregar solo `Int64`/`UInt64` primero) en vez de forzar una fecha.

- **La conversión contextual profunda interactúa con los contratos de operador de la Fase 3, y el spec no dice explícitamente cómo.** `Float(3 / 4)` reescribe `/` antes de bajarlo cuando los operandos son nativos; con un tipo de usuario que sobrecarga `_divide` de por medio, no está escrito si el contexto se propaga a través de la llamada al método reservado o se detiene en el operando que lo dispara. Se decide durante la implementación, con el precedente de que los contratos de operador **no** alteran precedencia ni aridad (sección 4) como guía — el contexto tampoco debería alterar cuál método se llama, solo qué valores recibe.

- **`to_string()` es una pieza más grande que "una función más".** Necesita: el contrato reservado en sí, un tipo nativo que lo implemente para cada escalar (incluidos los diez enteros nuevos y `Float`), despacho desde `println`/`print`, y el desugar de interpolación llamándolo por cada expresión `{...}`. Es, en tamaño, comparable a lo que los contratos de operador fueron en la Fase 3 — se trata como una pieza propia del plan de tareas, no como un detalle de la interpolación.

## Migration Plan

Aditivo sobre un pipeline que funciona de punta a punta, igual que las fases anteriores. `Int32`/`Boolean`/`String` no cambian de comportamiento observable; lo que cambia es que dejan de ser señalados como especiales en el checker.

El diagnóstico actual de `require_printable` — que solo acepta `Int32`/`Boolean`/`String` en `println` — se retira una vez que `to_string()` exista para todo tipo nativo y de usuario.

Rollback: revertir el merge. Ninguna fase posterior depende de esta todavía.

## Open Questions

- **¿Cómo se representa `Char` en la IR y en tiempo de ejecución?** Ver el riesgo de arriba. Un grapheme cluster extendido no tiene un tamaño máximo garantizado por el estándar Unicode. Opciones a evaluar antes de comprometerse: (a) una vista opaca respaldada por el runtime, como `String`; (b) un valor inline con optimización de buffer pequeño y un escape para el caso raro que excede el buffer; (c) restringir `Char` a un solo code point para esta fase y anotar el caso de grapheme extendido como deuda explícita hacia una fase posterior — lo que contradice la sección 3 tal como está escrita hoy, así que necesitaría primero una corrección de spec, no solo de implementación.

- **¿El contexto de conversión profunda cruza un operador sobrecargado por contrato?** Ver el riesgo correspondiente arriba. Propuesta a confirmar durante la implementación: no — el contexto convierte los operandos antes de que el método reservado se llame, y el método reservado ve sus parámetros con el tipo que ya declaró, sin conocimiento del contexto que los produjo.

- **¿Qué nombre exacto tiene el contrato de impresión?** `ZIRK_STDLIB_SPEC.md` sección 3 dice `to_string(): String` en prosa; `ZIRK_LANGUAGE_SPEC.md` sección 4 documenta la convención de métodos reservados con guion bajo (`_add`, `_subtract`) para los contratos de operador, pero `to_string` no es un operador — falta confirmar si sigue esa misma convención (`_to_string`) o si es un método público ordinario sin el prefijo reservado, dado que también se llama directamente por el usuario (`valor.to_string()`), no solo de forma implícita por el compilador como `_add`.
