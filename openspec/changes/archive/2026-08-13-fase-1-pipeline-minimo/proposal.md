## Why

La Fase 0 dejó los cimientos verificados —nueve crates, diagnósticos, emisión para los nueve targets y un binario nativo que ejecuta— pero **Zirk todavía no compila una sola línea de Zirk**. Los crates de lexer, parser, sema e ir están vacíos por diseño.

Esta fase conecta la boca de entrada. Al terminar, este archivo compila a un binario nativo real:

```zirk
fn main(): Void {
    stdout.println("Hola desde Zirk");
}
```

El valor no es el "hola mundo": es **validar que la columna vertebral completa funciona de punta a punta**. Todo lo que sigue en el roadmap extiende esta columna en vez de construir una nueva, así que un error estructural acá se paga en las diez fases restantes.

Por eso el criterio es un pipeline **flaco pero completo**, no una capa exhaustiva. Un parser que cubra toda la gramática sin backend conectado vale menos, hoy, que un subset mínimo que llegue hasta el binario.

## What Changes

### El subset del lenguaje

Se implementa exactamente lo que fija `ZIRK_ROADMAP.md` Fase 1:

- **Declaraciones**: `fn` con parámetros tipados y tipo de retorno; `main` como entrypoint.
- **Tipos**: `Void`, `Int32`, `Boolean`, `String`.
- **Variables**: `mut` e `inmut`, con anotación de tipo explícita e inferencia cuando sea inequívoca.
- **Literales**: enteros —con `_` como separador—, cadenas, `true` y `false`.
- **Operadores**: aritméticos `+ - * / %`, comparación `== != < <= > >=`, lógicos `&& || !`.
- **Control de flujo**: `if` / `else` como sentencia.
- **Llamadas** a funciones definidas en el mismo archivo, y `return`.
- **Comentarios** `//` y `/* */`.
- **`stdout.println`** como intrínseco reconocido por el compilador, no como stdlib real.

### El pipeline

- **`zirk-lexer`**: texto → tokens con ubicación, incluyendo errores léxicos con diagnóstico.
- **`zirk-ast`**: los nodos del subset y sus spans.
- **`zirk-parser`**: tokens → AST, con recuperación mínima ante error.
- **`zirk-sema`**: resolución de nombres, chequeo de tipos y análisis de flujo suficiente para el subset.
- **`zirk-ir`**: IR tipada mínima, **sin asumir modelo de memoria** (ADR-003).
- **`zirk-codegen-llvm`**: IR → LLVM IR → objeto, extendiendo lo que ya existe.
- **`zirk-runtime`**: `zirk_io_println` sobre la frontera ABI C, y `String` opaco (ADR-002, ADR-005).
- **`zirk-cli`**: `zirk run` y `zirk build` sobre un único archivo, invocando el linker.

### Reglas que se aplican desde ya

- **Cada regla del lenguaje con al menos un caso válido y uno inválido** en tests, según `ZIRK_SPEC_FINAL.md` sección 8.
- **Todo error con código estable, causa y ayuda**, usando `zirk-diagnostics`.
- **Sin truthiness numérico**: `if 1` es error de tipos, no verdad.
- **El overflow ordinario produce error controlado**, no envolvimiento silencioso (`ZIRK_LANGUAGE_SPEC.md` sección 3).

### Explícitamente fuera de alcance

Genéricos, clases, `Result`, concurrencia, decoradores, módulos multi-archivo, `init.zrk`, `match`, bucles, closures, nullability (`T?`, `?.`, `??`), `Decimal`, enteros distintos de `Int32`, y la stdlib más allá de `println`.

También queda fuera la **compilación incremental**: el pipeline es de un solo archivo y una sola pasada.

## Capabilities

### New Capabilities

- `zirk-lexical-syntax`: el léxico del subset — tokens, literales, comentarios, ubicaciones y errores léxicos.
- `zirk-grammar`: la gramática del subset y la construcción del árbol de sintaxis, con sus errores.
- `zirk-type-system`: tipos del subset, resolución de nombres, mutabilidad, inferencia y análisis de flujo.
- `zirk-ir-lowering`: la IR tipada mínima y su generación a partir del árbol verificado.
- `zirk-native-codegen`: la traducción de IR a LLVM y la producción del ejecutable enlazado.
- `zirk-runtime-io`: el contrato ABI C del runtime para salida estándar y representación de `String`.
- `zirk-cli-commands`: `zirk run` y `zirk build` sobre un archivo único.

### Modified Capabilities

- `compiler-workspace`: los crates dejan de estar vacíos. El requisito *Ausencia de sintaxis de Zirk en esta fase* deja de aplicar y se reemplaza.
- `target-matrix`: se añade la verificación de que el ejecutable producido corre en el host, no solo que el objeto se emite.

## Impact

**Crates que pasan de vacíos a implementados:** `zirk-lexer`, `zirk-ast`, `zirk-parser`, `zirk-sema`, `zirk-ir`.

**Crates extendidos:** `zirk-codegen-llvm` (lowering desde IR propia), `zirk-runtime` (`zirk_io_println`, `String`), `zirk-cli` (subcomandos reales).

**Nuevo:** un corpus de archivos `.zrk` de prueba, válidos e inválidos, con snapshots de diagnósticos.

**Sin dependencias externas nuevas.** El pin de LLVM y el toolchain no cambian.

**Riesgos:**

- **El diseño de la IR es la decisión de mayor apalancamiento de esta fase.** Es lo que se distribuirá en `.zpkg` (Fase 8) y lo que consumirá un eventual segundo backend. Una IR que asuma modelo de memoria contradice ADR-003 y sería caro de deshacer.
- **`String` es la primera prueba real de ADR-005.** Si el layout se filtra al codegen, la indexación por graphemes de Fase 7 se vuelve un refactor transversal.
- **El alcance tienta a crecer.** Bucles, `match` y nullability van a parecer "casi gratis" una vez que el parser funcione. No lo son: cada uno arrastra reglas de tipos y de flujo, y son Fase 2.
