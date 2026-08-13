## Context

La Fase 0 dejó verificado el tramo final del pipeline: dada una IR de LLVM, el proyecto produce un binario nativo que ejecuta, en Linux, macOS y Windows. Lo que falta es todo lo anterior.

Esta fase es la única del roadmap donde se tocan **todas** las capas a la vez. Las decisiones que se tomen acá —sobre todo la forma de la IR— condicionan diez fases siguientes, así que el criterio no es "lo que haga andar el hola mundo" sino "lo que no haya que deshacer en Fase 3".

ADRs vigentes que restringen esta fase:

| Decisión | ADR |
|---|---|
| La IR no asume modelo de memoria | [ADR-003](../../../docs/decisions/ADR-003-memoria.md) |
| El runtime es staticlib con frontera ABI C | [ADR-002](../../../docs/decisions/ADR-002-runtime-staticlib.md) |
| `String` es opaco tras el runtime | [ADR-005](../../../docs/decisions/ADR-005-representacion-string.md) |
| Pin de LLVM 20.1 | [ADR-001](../../../docs/decisions/ADR-001-pin-llvm.md) |

## Goals / Non-Goals

**Goals:**

- `zirk run hola.zrk` compila y ejecuta; `zirk build hola.zrk` produce un ejecutable.
- El subset de `ZIRK_ROADMAP.md` Fase 1 funciona de punta a punta.
- Cada regla del lenguaje tiene un caso válido y uno inválido en tests.
- Todo error del compilador sale con código estable, ubicación, causa y ayuda.
- La IR es tipada y no asume representación de memoria.

**Non-Goals:**

- Rendimiento del compilador. Una sola pasada, sin caché ni incrementalidad.
- Calidad del código generado. `OptimizationLevel::None` es suficiente.
- Cobertura gramatical. Lo que no está en el subset debe **fallar con un diagnóstico claro**, no parsearse a medias.
- Cross-compilation. Solo el host; los sysroots son Fase 6.
- Syntax API pública, formatter, linter, LSP.

## Decisions

### D1 — El árbol es una AST tipada por nodo, no un árbol genérico

`ZIRK_COMPILER_SPEC.md` sección 3 menciona un "árbol de sintaxis" y una Syntax API pública versionada. Son dos cosas distintas: la AST interna es privada y puede evolucionar; la Syntax API llega en Fase 10 con los decoradores.

Se implementa una AST con un tipo por construcción (`FnDecl`, `IfStmt`, `BinaryExpr`, …), no un árbol homogéneo de nodos genéricos con hijos.

**Alternativa descartada:** un árbol homogéneo estilo rowan/CST, que sería mejor para un LSP con recuperación de errores y trivia. Se descarta porque el LSP es Fase 9 y adoptar un CST ahora impone complejidad en todas las capas para un beneficio que tarda ocho fases en llegar. Cuando llegue, se introduce como capa adicional bajo la AST, no en su lugar.

Todo nodo lleva su span, sin excepción: un nodo sin ubicación no puede producir el diagnóstico que exige el spec.

### D2 — La IR es de tres direcciones, tipada, en forma de bloques básicos

```
   AST verificada  ──▶  IR  ──▶  LLVM IR
                        │
              tipada, con bloques básicos
              y operaciones de alocación abstractas
```

Se elige una IR de bloques básicos y no un árbol, aunque para el subset de esta fase un árbol alcanzaría, porque:

- `ZIRK_COMPILER_SPEC.md` sección 4 exige que conserve información suficiente para escape analysis, devirtualización y vectorización — todos análisis de flujo, que sobre un árbol son incómodos;
- Fase 2 introduce bucles y `break`/`continue`, que sobre un árbol obligan a rehacer la representación;
- el mapeo a LLVM es directo, porque LLVM ya es exactamente eso.

**No se adopta SSA con funciones phi todavía.** Las variables locales se representan como slots con carga y almacenamiento, y se delega a LLVM la promoción a registros. SSA propia es trabajo real que no paga hasta que existan optimizaciones propias.

### D3 — La alocación es una operación abstracta de IR

Consecuencia directa de ADR-003. La IR **no** dice "malloc", "gc_alloc" ni "refcount_inc": expresa `alloc <tipo>` y el runtime decide.

En esta fase la única cosa que se aloca es `String`, y el runtime la resuelve como quiera. Cuando Fase 4 elija la estrategia de memoria, cambia el runtime y el lowering, no la IR.

### D4 — `println` es un intrínseco, no stdlib

`import { stdout } from std.io;` no se implementa: no hay módulos multi-archivo en esta fase.

El compilador reconoce `stdout.println(<expr>)` como una forma sintáctica especial y la baja a una llamada a `zirk_io_println` del runtime. Es deuda deliberada y acotada, que se retira en Fase 7 cuando exista `std.io` de verdad.

**Se prefiere esto a implementar medio sistema de módulos**, que es Fase 2 y arrastra `share`/`import`, resolución de rutas y visibilidad.

El diagnóstico ante un `import` debe decir explícitamente que los módulos llegan en una fase posterior, no "símbolo desconocido".

### D5 — `String` cruza la frontera como handle opaco

Aplicación de ADR-005. El codegen emite:

```
  zirk_str_from_utf8(ptr, len) -> ZirkStr     // literal
  zirk_io_println(ZirkStr)                     // salida
```

`ZirkStr` es un puntero opaco para el compilador. Que hoy el runtime lo implemente como `{ptr, len}` es invisible desde la IR.

### D6 — Lo no soportado falla con diagnóstico explícito de fase

Un `class`, un `for` o un `match` no deben producir "token inesperado". El lexer reconoce las palabras reservadas del lenguaje completo y el parser emite un diagnóstico que dice que la construcción existe pero no está implementada todavía.

Es coherente con la regla del proyecto de no inventar comportamiento no especificado, y convierte el subset en algo comprensible en vez de en un lenguaje distinto que casualmente se parece a Zirk.

### D7 — El linker se invoca desde `zirk-cli`, no desde el codegen

`zirk-codegen-llvm` produce objetos; enlazar es orquestación. En esta fase se invoca el `clang` de la instalación de LLVM pineada, que ya se exige, en vez de depender del `cc` de cada host — mismo criterio que ADR-004 aplicó al linker.

## Risks / Trade-offs

- **La IR se diseña con un subset trivial y tiene que servir para clases y genéricos** → Mitigación: bloques básicos y tipado explícito desde el inicio, que es lo que no se puede añadir después sin rehacer. Lo que sí se pospone (SSA, optimizaciones) es aditivo.

- **`println` como intrínseco puede quedarse** → Mitigación: el requisito lo declara deuda con fecha de retiro en Fase 7, y el diagnóstico ante `import` referencia esa fase.

- **La AST tipada dificulta el LSP de Fase 9** → Trade-off aceptado en D1, con el camino de salida anotado: el CST se introduce como capa adicional.

- **El subset tienta a crecer** → Mitigación: D6 hace que lo no soportado falle de forma clara y barata, lo que quita la presión de "ya que estamos, agrego `for`".

- **Chequeo de overflow en tiempo de compilación vs runtime** → El spec exige error controlado, no envolvimiento. Para constantes se detecta al compilar; para operaciones en runtime, LLVM ofrece intrínsecos con detección. Se usa la vía de intrínsecos aunque cueste rendimiento: la alternativa contradice el spec.

## Migration Plan

No aplica: es puramente aditivo sobre crates vacíos.

Rollback: revertir el merge. La Fase 0 no depende de nada de esta fase.

## Open Questions

Las tres quedaron resueltas durante la implementación. Se conservan con su resolución en vez de borrarlas: la pregunta explica por qué la respuesta no era obvia.

- **¿`zirk run` deja el ejecutable en disco o lo borra?** `ZIRK_COMPILER_SPEC.md` sección 9 no lo especifica.

  **Resuelto: lo deja, en `build/`.** Quien ejecutó su programa lo más probable es que quiera distribuirlo, y un compilador que esconde su salida obliga a un segundo comando para recuperarla. Hay un test que lo fija.

- **¿Qué exit code produce `main` en esta fase?** `ZIRK_RUNTIME_SPEC.md` sección 2 lo delega a un "contrato de entrypoint" que no está definido.

  **Resuelto: solo `Void`, con exit code 0.** El chequeador rechaza cualquier otra firma de `main` con un diagnóstico que dice cuál se espera. La pregunta de fondo —qué significa que `main` retorne un `Result`— sigue abierta y corresponde a la Fase 4, cuando `Result` exista.

- **¿La inferencia cubre `mut x = 5`?** El spec la permite "cuando es inequívoca".

  **Resuelto: sí.** Con un solo tipo entero en el subset la inferencia es inequívoca. Prohibirla ahora y permitirla después habría sido un cambio incompatible en los tests.

## Decisiones tomadas durante la implementación

Dos que no estaban previstas y quedaron documentadas donde corresponde:

- **La forma de la IR pasó a [ADR-007](../../../docs/decisions/ADR-007-forma-de-la-ir.md).** El `design.md` de un change se archiva; la forma de la IR es un contrato con la Fase 8 y necesitaba un lugar durable.

- **Las funciones de Zirk llevan prefijo `zk_` en el código generado.** Evita que una función Zirk llamada `printf` se convierta en la de C, y deja libre el nombre `main` para el entrypoint que invoca el sistema operativo. El esquema es interno y se revisa cuando lleguen los paquetes en la Fase 8.
