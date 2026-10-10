# Traspaso: el juego educativo de Zirk

> Documento para el agente que continúe este trabajo. Está escrito para leerse de
> arriba abajo una vez y luego consultarse. Todo lo que afirma se comprobó el
> **2026-10-10**; las cifras de la sección 2 son las de ese día. Si algo no coincide
> con lo que ves, confía en lo que ejecutes y avisa del desfase.
>
> Si solo tienes cinco minutos: lee **0**, **2** y **6**.

---

## 0. En 60 segundos

**El proyecto.** Un juego en el navegador, con Three.js, que enseña el lenguaje **Zirk**
(estilo Swift Playgrounds): el jugador escribe programas Zirk reales, el juego los
ejecuta en el intérprete WASM del navegador y reproduce lo que hicieron con un robot.

**Qué hay hecho** (todo commiteado en `develop`, nada publicado con `push`):

1. **El intérprete del navegador ya ejecuta OOP**, `clone()` profundo y una **traza
   de ejecución** opt-in (ADR-022), y `print`/`println` aceptan cualquier valor
   (`Stringifiable`).
2. **El paquete WASM** se construye, se optimiza con `wasm-opt`, se verifica por
   comportamiento y ya está adoptado en `zirk-lang-site`.
3. **`game-assets/`**: 56 assets 3D procedurales (robots, monedas con el logo de Zirk,
   metas, baldosas, escenario, efectos y piezas de enseñanza de OOP), con catálogo,
   vista previa, exportación a `.glb` y tests.
4. **El guión del curso básico** (`ZIRK_LEARNING_GAME_CURRICULUM.md`): 53 niveles en
   7 sectores, **ejecutable y verificado**, con un exportador a `levels.json` y un
   jugador de terminal (`npm run level`).

**Qué NO hay** (y es tu trabajo): **el juego jugable**. No existe una página con editor,
ejecución, escena 3D y niveles juntos. Tampoco hay arte final, ni UI de progreso, ni
pruebas con jugadores reales. Ver la sección 5.

**Por dónde empezar:** lee la sección 5.1 (decisiones que debe tomar el usuario antes de
construir) y la 5.2 (arquitectura propuesta del vertical slice).

---

## 1. Mapa de repos y rutas

> ⚠️ **La ruta contiene un espacio:** `.../PROPIOS/Zirk Lang/...`. Entrecomilla *todas* las
> rutas en el shell. Ya hubo un bug por esto (ver sección 6, punto 1).

Raíz de trabajo: `/Users/cristian/Projects/PROPIOS/Zirk Lang/`

| Carpeta | Qué es | Rama | Notas |
| --- | --- | --- | --- |
| `zirk-lang/` | **Compilador, intérprete, docs, juego** (este documento vive aquí) | `develop` | ≈ 60 commits por delante de `origin/develop`, sin publicar (ver 7.2) |
| `zirk-lang-site/` | Sitio web (Angular 21). Ya ejecuta código Zirk en su *playground* con el paquete WASM | `develop` | 13 commits por delante de `origin`, sin publicar |
| `zirk-lang-browser-interpreter/` | **Worktree huérfano** (su `.git` apunta a una ruta que ya no existe). Su trabajo ya está archivado en `develop` | — | No lo uses; `git worktree prune` lo limpia desde `zirk-lang/` |
| `zirk-lang-collection-sequence-pipeline/`, `zirk-lang-promos/` | Otros worktrees, ajenos a este trabajo | — | No tocar |

Piezas que importan, dentro de `zirk-lang/`:

| Ruta | Contenido |
| --- | --- |
| `crates/zirk-interpreter/src/lib.rs` | Intérprete de IR verificado: heap, `Clone` profundo, traza, preflight de capacidades |
| `crates/zirk-browser/` | Fachada del navegador: `src/lib.rs` (check/run/format), `src/contract.rs` (tipos JSON), `browser-api.d.ts`, `fixtures/*.json` (conformidad), `oop-catalog.json`, `tests/corpus_parity.rs` |
| `crates/zirk-cli/tests/corpus/valid/` | 218 programas Zirk con su salida nativa esperada (`.out`): la fuente de verdad de paridad |
| `scripts/` | `build-browser-package.sh`, `verify-browser-package.sh`, `verify-browser-package-behavior.mjs`, `check-browser-package-size.sh`, `write-browser-manifest.mjs`, `ensure-wasm-opt.sh`, `ensure-wasm-bindgen.sh`, `sync-website-content.sh`, `audit-browser-dependencies.sh` |
| `stdlib/std/io/` | `std.io` escrito en Zirk (`Stringifiable`, `stdout`, `stderr`, `stdin`) |
| `docs/decisions/ADR-020…022` | Autoridad del navegador (20), módulos (21), **traza (22)** |
| `docs/browser-interpreter-support-matrix.md`, `docs/browser-oop-coverage.md`, `docs/browser-package-handoff.md` | Qué corre en el navegador y con qué evidencia |
| `openspec/changes/` | Los cuatro cambios de este trabajo (completos, **sin archivar**, ver 5.6) |
| `game-assets/` | **La biblioteca de assets y las herramientas del currículo** |
| `ZIRK_LEARNING_GAME_CURRICULUM.md` | **El guión de 53 niveles (ejecutable)** |
| `ZIRK_LEARNING_GAME_ANALYSIS.md` | Análisis de factibilidad (**sin commitear**; parcialmente desactualizado, ver 7) |

En `zirk-lang-site/` lo relevante para el juego: `src/app/features/playground/`
(`playground-compiler-runtime.ts`, `playground-compiler.worker.ts`,
`playground-execution-client.ts`, `playground-execution.models.ts`) —el código que ya
llama al WASM desde un worker, listo para reutilizar—, `public/browser-compiler/`
(artefactos con hash en el nombre), `tools/browser-compiler/` (`acquire.ts`,
`verify.ts`), `DESIGN.md` (sistema de diseño) y su `AGENTS.md` (reglas propias).

---

## 2. Entorno y comprobación de que todo está sano

### 2.1 Versiones usadas

Node 24.15, Rust 1.94.1, `wasm-bindgen` 0.2.128 (**fijada**), `wasm-opt` (binaryen)
`version_133` (**fijada**, `brew install binaryen`), `three` 0.180.0, `openspec` 1.13.0.
LLVM 20.1 en `/opt/homebrew/opt/llvm@20`.

### 2.2 Variable imprescindible

```sh
export LLVM_SYS_201_PREFIX=/opt/homebrew/opt/llvm@20
```

Sin ella `zirk-codegen-llvm` y `zirk-cli` fallan al compilar. Las crates del navegador
(`zirk-interpreter`, `zirk-browser`) **no** la necesitan, pero sus tests de desarrollo
dependen de `zirk-cli`, así que expórtala siempre.

### 2.3 Comandos y resultado esperado hoy

Desde `zirk-lang/` (el repo del compilador):

| Comando | Resultado esperado (2026-10-10) |
| --- | --- |
| `cargo test --workspace --no-fail-fast` | **1560 pasados**, 44 suites, sin fallos |
| `cargo clippy --workspace --all-targets -- -D warnings` | sin avisos |
| `cargo fmt --all --check` | sin diferencias |
| `./scripts/audit-browser-dependencies.sh` | `browser dependency audit passed` |
| `cargo build -p zirk-browser --example interp_repro` | compila; deja `target/debug/examples/interp_repro` (lo usan el verificador y el jugador de terminal) |
| `./scripts/build-browser-package.sh /ruta/vacía/pkg` | paquete en `pkg/`; ver tamaños abajo |
| `./scripts/verify-browser-package.sh /ruta/pkg` | integridad, tamaño y comportamiento OK (exige árbol limpio) |

Desde `zirk-lang/game-assets/`:

| Comando | Resultado esperado |
| --- | --- |
| `npm install` | instala `three` |
| `npm test` | **22 pruebas pasan** (assets, currículo, servidor) |
| `npm run curriculum:verify` | `53 levels, 8 findings and 14 diagnostics checked` y `all levels verified` |
| `npm run curriculum:export` | regenera `curriculum/levels.json` (53 niveles) |
| `npm run preview` | sirve `http://localhost:5173/` con los assets en 3D |
| `npm run shots` | regenera `preview/shots/*.png` (Playwright, ver sección 6, punto 6) |
| `npm run export-check` | prueba la exportación `.glb` de las 7 categorías en un navegador real |
| `npm run level -- --list` | lista los 53 niveles |

Tamaño del paquete WASM verificado (con `wasm-opt -Oz`): **≈ 3.52 MB crudo,
1.27 MB gzip, 0.90 MB brotli**. Presupuestos (`scripts/check-browser-package-size.sh`):
crudo aviso 3.7 MB / error 4.0 MB; gzip 1.30 / 1.45 MB; brotli 0.95 / 1.20 MB.

Desde `zirk-lang-site/`: `yarn test` (187 pruebas) y `yarn build` pasan. El *build*
avisa de que el bundle inicial excede su presupuesto (528.81 kB frente a 515 kB) y de
un SCSS; **no pude atribuirlo a nada de este trabajo** (el WASM no está en el bundle
inicial). No es un fallo, pero conviene vigilarlo.

---

## 3. Lo que ya está hecho

### 3.1 El intérprete del navegador (cuatro cambios OpenSpec, todos completos)

| Cambio | Qué entregó | Evidencia |
| --- | --- | --- |
| `browser-oop-conformance` | El modelo de objetos corre en el navegador con paridad nativa: clases, herencia, `#override`, `final`, abstractas, interfaces, traits con defaults, `static`, anidadas/`inner`/locales, genéricas, records, enums, colecciones polimórficas, excepciones tipadas | 14 fixtures `oop-*`; `corpus_parity.rs` (218 programas del corpus: 186 coinciden con el nativo y 32 son exclusiones explícitas de capacidad); `oop-catalog.json`; `docs/browser-oop-coverage.md` |
| `browser-deep-clone` | `InstKind::Clone` en el heap del intérprete (identidad nueva, sharing preservado, ciclos, closures con capturas, coste acotado) | 5 fixtures `clone-*`; tests de presupuesto de instrucciones y de heap |
| `browser-execution-trace` | `run` acepta `trace` opt-in y devuelve eventos `alloc`, `fieldWrite`, `call`, `return`, `throw`, `output`; **solo lectura**, acotada, determinista | ADR-022; 6 tests; `browser-api.d.ts`; sección `trace` del manifiesto |
| `stringifiable-print-arguments` | `Stringifiable` es un trait real de `std.io`; `println("a", 2, true, obj)` acepta cualquier valor; las clases con `to_string()` necesitan `#override`; un evento de salida por **línea** | Fixture `std-io-mixed-values`; test de líneas por evento |

También se corrigió un bug del compilador: una subclase que añadía un método propio
producía IR inválido (`subclass_adds_method` en el corpus).

**Contrato del navegador** (`crates/zirk-browser/browser-api.d.ts`, `apiVersion: 1`, **no cambiar**):

```ts
// Petición
{ apiVersion: 1, source: string, virtualName: string, limits?: Limits,
  trace?: { events?: number, bytes?: number, valueBytes?: number, kinds?: TraceEventKind[] } }
// Respuesta (campos relevantes)
{ output: [{ text, destination: "stdout"|"stderr", truncated }],
  completion: "completed" | null,
  failure: { kind: "request"|"runtime"|"capability"|"limit"|"internal"|"version",
             code, message, limit } | null,
  diagnostics: [{ code, severity, message, stage, file, start, end }],
  trace?: { events: [{ seq, kind, object?, class?, field?, ty?, value?, callee?, receiver?, outputIndex? }],
            truncated, limits }
}
```

El programa se ejecuta con **solo dos efectos**: salida acotada y cancelación (ADR-020). No
hay entrada, reloj, red, DOM ni archivos. Límites por defecto: 2 000 000 instrucciones,
profundidad de llamadas 128, heap 16 MiB, salida 256 KiB y 2 048 eventos (cuentan
**líneas**, no escrituras).

### 3.2 El paquete WASM y su canal de entrega

- `scripts/build-browser-package.sh` compila a `wasm32`, genera el *glue* con
  `wasm-bindgen`, **optimiza con `wasm-opt -Oz`** y exige que el binario optimizado sea
  **observablemente idéntico** al original sobre 298 casos (`verify-browser-package-behavior.mjs`:
  fixtures normales y con traza + corpus nativo). Si difiere, el *build* falla.
- El manifiesto (`manifest.json`) publica límites, exclusiones, versiones de herramientas
  (incluida `wasmOpt`), el catálogo `objectModel` y la descripción de `trace`.
- **Adopción en el sitio:** `yarn compiler:acquire --checkout <ruta> --revision <sha> --package <dir> --date YYYY-MM-DD`
  (en `zirk-lang-site/`) valida el paquete contra la revisión exacta y reemplaza
  `public/browser-compiler/` (nombres con hash) y la procedencia generada. Ya se hizo con
  `c9f0ba0`; el sitio quedó en `624eece`.
- **Si cambias el compilador**, el orden es: commit limpio en `zirk-lang` → construir y
  verificar el paquete → `./scripts/sync-website-content.sh` (+ `--audit-date` solo si
  cambió evidencia de estado, **con revisión humana**) → `yarn compiler:acquire` → `yarn test && yarn build`
  en el sitio → commit separado del sitio.

### 3.3 `game-assets/` (los assets 3D)

Biblioteca ES modules, **sin modelos ni texturas externas**, paleta de la marca
(`src/palette.js`). Detalles completos en `game-assets/README.md`; lo esencial:

- **56 assets** en `CATALOG` (`src/catalog.js`): personajes (`bot-zirk`, `bot-scout`,
  `bot-builder`, `bot-guard`, `bot-courier`, `bot-ghost`), ítems, metas, 13 tipos de
  baldosa más muro y pilar, props, efectos y piezas de enseñanza de OOP (`blueprint`,
  `cloner`, `contract-scroll`, `object-panel`, `reference-link`).
- `createAsset(id, opciones)` instancia por id. Los assets con movimiento tienen
  `update(dt)`; los interactivos exponen setters acotados (`open(0..1)`, `setLevel(0..1)`,
  `press(on)`, `setField(i, ratio)`, `setProgress(0..1)`).
- **Convenciones:** 1 baldosa = 1 unidad; los pies/base en `y = 0`; los personajes miran a
  `+Z`; materiales y geometrías **cacheados y compartidos**; todo es determinista.
- `bot.play('idle'|'walk'|'jump'|'celebrate'|'sad'|'think')`, `bot.face(ángulo)`.
- **La moneda lleva el logo real de Zirk** (la Z cruzada por su diagonal, con los
  degradados del SVG `docs/assets/zirk-clean.svg`), construido en `src/logo.js`.
- Tests: 14 de assets (modelos finitos y a escala, animaciones estables, determinismo,
  setters que acotan, materiales compartidos, proporciones del logo…).
- Vista previa en `preview/` (acepta `?category=…`, `?asset=<id>` para ver uno grande,
  `?shot=1&warm=<frames>` para capturas deterministas).

### 3.4 El currículo (guión ejecutable)

`ZIRK_LEARNING_GAME_CURRICULUM.md`: **53 niveles, 7 sectores**, solo fundamentos (salida,
variables y tipos, valor frente a referencia, flujo de control, funciones, colecciones,
`record`/`enum`/`Result`; **sin OOP**, y un test lo impone).

**Cómo corre el código del jugador** (decisión de diseño central, ya verificada):

```
import { stdout, Stringifiable } from std.io;  ← oculto, en la PRIMERA línea del jugador
<código del jugador>                           ← lleva su propio import visible (el duplicado es válido)
<funciones auxiliares ocultas>                 ← al final del archivo (usables antes de declararse)
<ayudantes del caso de prueba>                 ← p. ej. fn bridge_down(): Boolean { … }
```

- Los números de línea de los diagnósticos se conservan (el `import` oculto va en la misma
  línea que el primer renglón del jugador).
- Las auxiliares (`curriculum/levels.json → preludes.core`) escriben líneas de protocolo:
  `@walk n`, `@turn left|right`, `@collect`, `@hop`, `@inspect etiqueta valor`. **Toda línea
  que empieza por `@` es una orden** para el juego; el resto es texto de terminal.
- Los **casos de prueba** (`cases[]`) cambian los sensores (`helpers`) y, a veces, el mapa:
  la misma solución debe servir en todos.

**Tipos de nivel:** `grid` (llegar a la meta; lo juzga un simulador), `output` (salida exacta
en todos los casos), `fix` (arreglar leyendo un diagnóstico real) y `predict` (apostar la
salida antes de ejecutar; el programa no se edita).

**Bloques que lee el tooling** dentro del Markdown: ` ```level ` (JSON), ` ```zirk starter|solution|program <id> `,
` ```zirk prelude <nombre> `, ` ```zirk finding <id> ` + ` ```expect <id> `, ` ```diagnostics `.

**Herramientas** (`game-assets/curriculum/`):

| Archivo | Función |
| --- | --- |
| `verify.mjs` | Corre cada solución, comprueba que el código inicial no pasa, que cada "error común" produce el diagnóstico prometido, que los 8 hallazgos del lenguaje siguen valiendo y que el glosario de 14 diagnósticos es **exactamente** el que los niveles ejercitan |
| `export.mjs` | Genera `levels.json` y **valida la estructura** (tres pistas, concepto, historia, objetivo, assets existentes en el catálogo); **nunca exporta soluciones** |
| `lib.mjs` | El simulador de cuadrícula (`simulate`), compartido |
| `play.mjs` | Jugador de terminal: `npm run level -- 3.1 [--watch] [--hint N] [--answer N]` |
| `levels.json` | Lo que cargará el juego. Esquema: `{ schemaVersion: 1, preludes, levels[] }` |

Un test comprueba que `levels.json` está al día con el documento: tras editar el guión,
ejecuta `npm run curriculum:export` y commitea el JSON.

Cada nivel de `levels.json` tiene: `id, sector, sectorName, title, type, concept, story, goal,
assets[], artNotes, hints[3], extra, prelude[]` y, según el tipo, `starter, expect, cases[],
map[], facing, par` (o `program, options, answer, explanation` en `predict`).

### 3.5 Documentos relacionados

- `docs/browser-oop-coverage.md`: qué construcciones OOP corren en el navegador y los
  defectos ajenos al intérprete que la auditoría encontró.
- `docs/browser-interpreter-support-matrix.md`: política exhaustiva de qué se ejecuta y qué
  se rechaza (incluye la sección de la traza).
- `docs/init/ZIRK_FEATURE_STATUS.md`: estado oficial de funcionalidades.

---

## 4. Contratos que no hay que romper

1. **`apiVersion: 1`** del navegador y sus campos aditivos. No añadas campos obligatorios.
2. **ADR-020/022:** el programa del jugador **no recibe** efectos de host nuevos. La traza es
   observación, no autoridad. Una API para que el programa *cause* efectos (p. ej.
   `game.move()`) es otra decisión y necesita su propio ADR.
3. **El protocolo `@`** y las auxiliares ocultas (sección 3.4). Si añades órdenes, ampliarás
   `preludes`, `lib.mjs::simulate` y el verificador a la vez, y los tres tienen que seguir
   de acuerdo.
4. **Soluciones fuera del cliente.** Ninguna solución de referencia debe llegar al navegador
   del jugador. Hay un test que lo vigila.
5. **Ids del catálogo.** `levels.json` referencia assets por id; el exportador falla si no existen.
6. **El curso básico no usa OOP.** El verificador y un test rechazan `class`, `interface`,
   `trait`, `extends`, `implements`, `inner` y `abstract` en el código de los niveles.
7. **No afirmar de más en público.** El estado de soporte del navegador sale de la matriz y de
   la evidencia (fixtures y corpus), nunca de que "el código existe". Cualquier cambio de texto
   público pasa por revisión humana (ver sección 7).
8. **El presupuesto del paquete WASM** (sección 2.3).

---

## 5. Lo que falta por preparar

Ordenado por prioridad. Cada punto lleva su criterio de aceptación.

### 5.1 Decisiones que debe tomar el usuario (bloquean o condicionan lo demás)

| # | Decisión | Mi recomendación | Por qué importa |
| --- | --- | --- | --- |
| D1 | ¿Dónde vive el juego: ruta `/learn/` en `zirk-lang-site` o repo aparte? | En el sitio (`/learn/`), cargando Three.js y los assets en un *chunk* perezoso | Reutiliza el worker, el diseño y el despliegue; el coste es respetar el presupuesto de bundle y la accesibilidad del sitio |
| D2 | ¿Se reescribe el historial para quitar trailers y mensajes en español de commits sin publicar? | Sí, antes de publicar | `AGENTS.md` del repo pide mensajes en inglés **sin** `Co-Authored-By`. 16 de los 58 commits sin publicar los llevan (ver sección 7.2) |
| D3 | ¿Se hace `push` de `zirk-lang` (≈ 60 commits) y del sitio (13)? | Tras D2 y revisar | Nada de este trabajo está publicado |
| D4 | Origen de los assets finales: procedural (lo actual), packs CC0 (Kenney/Quaternius) o un artista | Mantener lo procedural para el vertical slice y decidir con el feedback | Los robots no tienen rig esqueletal real, solo articulaciones animadas por código |
| D5 | Idioma de las lecciones y si habrá i18n desde el inicio | El guión está en español; el sitio documenta en inglés | Cambia la estructura de `levels.json` si se traduce |
| D6 | ¿Se corrigen los defectos de compilador listados en 5.5 antes de escribir más lecciones? | Los dos de la lista "alta" sí | Algunos afectan a lo que se puede enseñar |
| D7 | ¿Entra la concurrencia en el alcance? | No por ahora | Está excluida del navegador (ADR-020); requeriría revisar ese ADR |

### 5.2 El vertical slice jugable (prioridad máxima)

**Meta:** una página con el **Sector 1** (niveles 1.1–1.5) jugable de principio a fin, con el
robot animado en 3D, el editor de código y la validación real. Después, ampliar a los sectores
2 y 3.

**Arquitectura propuesta:**

```
levels.json ──► Nivel actual ──► Editor de código (código del jugador)
                    │                       │
                    │              ensamblar programa (3.4)
                    ▼                       ▼
            Escena 3D (game-assets)   Worker WASM → run()  ──► output[] (líneas)
                    ▲                       │
                    │              separar: líneas "@…" = órdenes · resto = terminal
                    │                       │
              Reproductor de órdenes ◄──────┤        Panel de terminal ◄── texto libre
              (bot.play + posición)         │
                    ▲                       ▼
                    └────────── Simulador de cuadrícula (lib.mjs::simulate) ──► veredicto
```

Piezas y notas:

1. **Worker y llamada a `run`:** reutiliza `zirk-lang-site/src/app/features/playground/`
   (`playground-compiler.worker.ts`, `playground-compiler-runtime.ts`,
   `playground-execution-client.ts`). Ya gestionan inicialización, contrato, límites y un
   plazo de reloj (que mata y recrea el worker).
2. **Ensamblado del programa:** exactamente la receta de 3.4. Port a TypeScript de la función
   `run()` de `curriculum/verify.mjs`.
3. **Simulador:** portar `curriculum/lib.mjs::simulate` al cliente. Idealmente **extraerlo a un
   módulo compartido** que usen el verificador, el jugador de terminal y el juego, para que no
   diverjan.
4. **Reproductor:** convierte la lista de órdenes en una línea de tiempo. Coordenadas sugeridas:
   casilla `(col, fila)` → mundo `x = col`, `z = fila`; `bot.face(θ)` con este mapeo
   (rotación Y positiva lleva `+Z` hacia `+X`): este = `π/2`, sur = `0`, oeste = `-π/2`,
   norte = `π`. `walk(n)` = `play('walk')` y avanzar casilla a casilla; `hop` = `play('jump')`;
   `collect` = destello + ocultar la moneda; meta alcanzada = `celebrate` +
   `createSparkles`. **Para el fallo de ejecución, para la reproducción en el punto del
   fallo** (el robot `sad`).
5. **Traza:** para el currículo básico **no hace falta** (no hay objetos). Se usará en el sector
   OOP futuro: `alloc` → `createBot(kind)`, `fieldWrite` → `object-panel.setField`, etc. La tabla de
   correspondencias está en `game-assets/README.md`.
6. **UI mínima:** editor con resaltado (el sitio ya tiene `playground-highlighter`), botones
   Ejecutar/Reiniciar, pistas en tres escalones, panel de terminal, panel de datos
   (`@inspect`), diagnósticos con código y línea resaltada.
7. **Progreso:** estrellas (★ pasar, ★★ sin tercera pista, ★★★ reto extra) y monedas, guardado en
   `localStorage` **dentro de `try/catch`** (puede no estar disponible); la página debe funcionar sin él.
8. **Accesibilidad (exigencia del sitio: AXE y WCAG AA):** el canvas es contenido no textual.
   Hace falta una **transcripción textual** de lo que ocurre (la lista de órdenes lo es),
   control por teclado completo y camino para `prefers-reduced-motion` (saltar el `update(dt)`
   decorativo de monedas y portales).
9. **Rendimiento:** Three.js y `game-assets` en un chunk perezoso; no engordar el bundle inicial
   (ya avisa). Los assets comparten materiales/geometrías; llama a `disposeMaterialCache()` al
   salir de la ruta.

**Criterios de aceptación del slice:**

- Los 5 niveles del sector 1 se pueden jugar y superar con la solución del guión y **no** con el
  código inicial.
- Un error de compilación muestra el diagnóstico real (código, mensaje, línea) y el robot `sad`.
- El mismo nivel produce el mismo veredicto en el juego y en `npm run level`.
- `yarn test` y `yarn build` del sitio siguen pasando, sin pasar el presupuesto de errores.
- AXE sin violaciones y navegable por teclado.
- Una prueba con Playwright (Chromium y WebKit) que resuelva el nivel 1.2 de punta a punta.

### 5.3 Assets pendientes (sección 13.2 del currículo)

- **Panel de datos genérico** para `inspect(etiqueta, valor)` (hoy solo hay `object-panel` por campos).
- **Marcadores de tipo:** una forma y color por tipo (`Int32`, `String`, `Boolean`, `Char`, `Decimal`).
- Animaciones de **desbordamiento** (el cofre que no cierra) y de `BoundsError` (el estante sin hueco).
- **Cuadrícula editable / constructor de mapas** a partir de los textos de `levels.json`.
- Mejorar el arte del robot si se decide (D4); los assets tienen pruebas, pero no "pulido".
- Los assets **no** se probaron con cientos de piezas en pantalla, ni en móvil, ni en Firefox.

### 5.4 Contenido y pedagogía

- **Revisión humana del guión:** pistas, historias y explicaciones son redacción mía y
  **ningún jugador lo ha probado**. Necesita playtesting (la dificultad sube bien? qué pista no ayuda?).
- Sectores **sin jefe** (1, 2, 4, 6): hoy cierran con un nivel normal; valorar si merecen uno.
- **Segundo tramo del juego (OOP):** clases, herencia, interfaces, polimorfismo, `clone()` y
  referencias a objetos, usando la traza y los assets de enseñanza ya hechos. Aún no tiene guión.
- Si se traduce (D5), el exportador y los textos necesitan una capa de i18n.

### 5.5 Defectos del compilador y del runtime nativo (con repros)

Descubiertos durante el trabajo. Los de la columna "navegador" no afectan al juego (que
ejecuta en el navegador); los demás condicionan lo que se puede enseñar.

| Prioridad | Defecto | Repro mínima | Efecto en el juego |
| --- | --- | --- | --- |
| **Alta** | `a?.length` sobre `String?` hace que el compilador entre en **pánico** (`lower.rs`, «a verified field access reads an object…») | `mut a: String? = "hola"; println(a?.length);` | El guión evita el acceso seguro y enseña `??` y `match` |
| **Alta** | `finally` no se ejecuta si hay `return` dentro de un `catch` | `try { … } catch X(e) { return …; } finally { println("x"); }` | Afecta a futuras lecciones de errores |
| Media | Una excepción propia no puede declararse en el `throws` de un **método** de clase | `class S { f(): Int32 throws Oops { … } }` | Condiciona el tramo OOP |
| Media | Una clase que implementa `B` (con `B implements A`) no se acepta donde se espera `A` | pasar `C` a `fn use(x: A)` | Condiciona interfaces |
| Media | Un método `static` no puede nombrar su propia clase; no se puede escribir un campo `static mut` | `static bump() { Counter.count = … }` | — |
| Baja | `match` no agrupa números con coma (`1, 2 =>`) | ver hallazgo f1 del currículo | El guión usa una rama por valor |
| Baja | Las tuplas no se devuelven ni se indexan (`t.0`) | hallazgos f2 | El guión solo desestructura |
| Baja | `List<Int32>.remove(0)` quita **por valor** | hallazgo f3 | El nivel 5.2 usa una lista de textos |
| Solo nativo | `Map<String,V>`/`Set<String>` comparan claves por **identidad** (el navegador está bien) | `m.set("k",1); m.contains_key("k")` con otra instancia | El guión usa claves enteras |
| Solo nativo | `==` sobre dos `List` termina el proceso (código 130) | `a == c` con listas | Ídem |

Los hallazgos f1–f8 del currículo están **comprobados por el verificador**; el resto está
descrito en `docs/browser-oop-coverage.md`.

### 5.6 Higiene del repo pendiente

- **Archivar los cuatro cambios de OpenSpec** (`openspec archive <nombre>`): están completos y
  validados (`openspec validate --strict`) pero siguen en `openspec/changes/`, de modo que sus
  *deltas* de especificación **aún no se han fusionado** en `openspec/specs/`. Hazlo con
  cuidado y revisa el diff de specs.
- **Sitio:** `zirk-lang-site/.vscode/settings.json` aparece modificado (cambio ajeno a este
  trabajo); no lo incluyas en tus commits sin preguntar.
- **`ZIRK_LEARNING_GAME_ANALYSIS.md`**: sigue sin commitear; decide si se commitea (está
  parcialmente desactualizado, ver 7.3). Este documento y el currículo sí están commiteados.
- El worktree huérfano `zirk-lang-browser-interpreter/`: `git worktree prune`.
- **Firefox:** el Firefox de Playwright **no arranca** en esta máquina («Could not find profile
  folder»), aunque el binario es válido. El paquete y los assets **no** están probados en
  Firefox. Probar con `npx playwright install firefox` o en otra máquina.
- **CI:** nada de `game-assets/` está en CI. Valorar un job que ejecute `npm test` y
  `npm run curriculum:verify` (necesita construir `interp_repro`).

---

## 6. Trampas y lecciones aprendidas (léelo antes de ejecutar nada)

1. **La ruta tiene un espacio.** Entrecomilla todo. Un script que comparaba
   `import.meta.url` con `file://${argv[1]}` salía en silencio por esto (corregido en
   `preview/serve.mjs` con `pathToFileURL`; `export.mjs` usa una comparación equivalente).
2. **macOS no tiene `timeout`.** Usa temporizadores de Node o `perl -e 'alarm …'`.
3. **`cargo test --workspace` se detiene en la primera suite que falla.** Usa siempre
   `--no-fail-fast`; si no, un fallo oculta otros y los recuentos engañan.
4. **Exporta `LLVM_SYS_201_PREFIX`** (sección 2.2). Sin ella el árbol parece "roto" sin estarlo.
5. **El shell de la herramienta envuelve `cargo` (rtk)** y resume su salida («cargo test: N passed»),
   lo que puede ocultar nombres de tests fallidos. Para ver la salida cruda, redirige a un archivo o
   usa `rtk proxy cargo …`.
6. **Playwright:** no está en el `node_modules` del proyecto. Una instalación válida está en
   `~/.npm/_npx/e41f203b7505f1fb/node_modules/` (los scripts usan `createRequire` con esa ruta;
   se puede cambiar con `PLAYWRIGHT_MODULES`). Para WebGL en modo *headless* hacen falta los
   argumentos `--use-gl=swiftshader --enable-unsafe-swiftshader` (ya puestos en `preview/shots.mjs`).
7. **Servidores en segundo plano:** `nohup comando &` **muere** al terminar el comando de la
   herramienta. Hay que lanzarlos con la opción de ejecución en segundo plano de la herramienta.
8. **`git stash` es compartido entre worktrees.** Si lo usas, ponle una etiqueta única, captura el
   SHA, restaura con `git stash apply <sha>` y descarta con `git stash drop stash@{n}` (no con el SHA).
9. **Worktrees obsoletos:** el repo se movió de carpeta una vez; quedan worktrees "prunable".
   `git worktree prune` los limpia. Crea los tuyos en `.claude/worktrees/<nombre>` y bórralos al acabar.
10. **El árbol principal de `zirk-lang` puede tener trabajo ajeno sin commitear** (pasó con `std.io`).
    Antes de fusionar o construir un paquete (que exige árbol limpio), mira `git status`. No
    sobrescribas ni descartes cambios que no hiciste.
11. **El paquete WASM exige árbol limpio** (`build-browser-package.sh` y `verify-browser-package.sh`
    se niegan a trabajar con cambios sin commitear: es por trazabilidad).
12. **`import` duplicado:** el ensamblado del currículo repite `import { stdout … }`; es válido.
    Pero **un `import` después de código no lo es**: por eso el oculto va *delante* y las
    auxiliares *detrás*.
13. **Los verificadores detectan errores tuyos.** Mientras escribía el currículo, `verify.mjs`
    atrapó una letra cirílica en un mapa, códigos de diagnóstico equivocados y "errores comunes"
    que no fallaban. Confía en él antes de revisar a ojo.
14. **Una cadena de rango con variable lleva llaves** (`0..{n}`) y `length` es `UInt64`: no mezcles con
    contadores `Int32` (ver hallazgo f4).
15. **No mezcles commits de las dos repos** y no publiques nada sin permiso explícito.

---

## 7. Reglas del repo y estado de git

### 7.1 Reglas que hay que cumplir

- **`zirk-lang/AGENTS.md`:** los commits **no** llevan `Co-Authored-By`, `Generated with` ni ningún
  trailer de agente. Mensajes en **inglés**, con prefijos (`docs(scope):`, `feat(scope):`…).
- **`zirk-lang-site/AGENTS.md`:** los commits no llevan atribución y deben ser concisos, en
  **español**. Angular: componentes *standalone* sin `standalone: true` explícito, señales,
  `OnPush`, control de flujo nativo (`@if`, `@for`), `inject()`, **sin** `ngClass`/`ngStyle`;
  debe pasar AXE y WCAG AA.
- **`openspec/config.yaml`:** los artefactos de OpenSpec van **en inglés**. Todo cambio que
  afecte a documentación pública o estado debe actualizar también el sitio, en commit aparte,
  con `./scripts/sync-website-content.sh`; si cambia evidencia de estado, hace falta revisión humana y
  `--audit-date YYYY-MM-DD` explícito.
- **ADR-006:** el código y los documentos normativos del repo van en inglés. El currículo y los
  análisis del juego están en español por decisión del usuario.
- **Nunca** publiques (`push`), borres ramas remotas ni reescribas historial publicado sin permiso.

### 7.2 Estado de git (2026-10-10)

- `zirk-lang`, rama `develop`: **58 commits por delante de `origin/develop`** al escribir esto (59 con el commit de este
  documento), árbol limpio salvo `ZIRK_LEARNING_GAME_ANALYSIS.md` (sin seguimiento). **16 commits sin publicar contienen el trailer
  `Co-Authored-By`** y varios tienen el mensaje en español, contra lo que pide su `AGENTS.md`
  (se añadieron antes de que apareciera esa regla en el contexto). El usuario no ha decidido si se
  reescribe el historial (D2).
- `zirk-lang-site`, rama `develop`: 13 commits por delante de `origin/develop`; un archivo
  modificado ajeno (`.vscode/settings.json`).
- Los commits relevantes, de más antiguo a más reciente: propuestas OpenSpec → corrección de
  IR de subclases → paridad OOP y catálogo → clone profundo → traza (ADR-022) → `wasm-opt` y
  verificación de comportamiento → `Stringifiable` y líneas por evento → `game-assets` →
  moneda con el logo → currículo → jugador de terminal.

### 7.3 Sobre `ZIRK_LEARNING_GAME_ANALYSIS.md`

Es el análisis de factibilidad inicial. Su **sección 1** y las estimaciones se actualizaron
tras auditar el intérprete, pero conserva ideas anteriores. Tómalo como contexto histórico:
**las fuentes de verdad son este documento, el currículo y `game-assets/README.md`.** Su
estimación restante (≈ 5 a 8 semanas) quedó **antes** de construir el currículo y los assets;
el vertical slice (5.2) es ahora el trabajo grande.

---

## 8. Cómo trabajar sobre esto, en orden

1. Lee 0, 2, 6 y 7. Ejecuta los comandos de 2.3 y confirma que todo está verde (si no, para y avisa).
2. Pide al usuario las decisiones **D1–D3** (5.1). Sin D1 no sabes dónde construir; D2/D3 solo
   importan al publicar, pero conviene saberlas antes de commitear más.
3. Construye el slice (5.2) con TDD y en un *worktree* aparte. Empieza por el ensamblado del programa,
   el simulador compartido y el reproductor de órdenes; deja el 3D para el final.
4. Cada cambio que toque el compilador o el paquete sigue el flujo de 3.2.
5. Cada cambio del guión: edita el Markdown → `npm run curriculum:verify` →
   `npm run curriculum:export` → `npm test`.
6. Documenta lo que descubras (nuevos defectos, límites del lenguaje) como hallazgo **verificable**
   en el currículo, no solo en prosa.
7. Cuando termines un hito, deja este documento al día (sección 2.3 y 5).

## 9. Enlaces rápidos

| Qué | Dónde |
| --- | --- |
| Guión de niveles | `ZIRK_LEARNING_GAME_CURRICULUM.md` |
| Datos del juego | `game-assets/curriculum/levels.json` |
| Assets y su API | `game-assets/README.md` |
| Contrato del navegador | `crates/zirk-browser/browser-api.d.ts` |
| Traza | `docs/decisions/ADR-022-browser-execution-trace.md` |
| Qué corre en el navegador | `docs/browser-interpreter-support-matrix.md`, `docs/browser-oop-coverage.md` |
| Entrega del paquete | `docs/browser-package-handoff.md`, `docs/TOOLCHAIN.md` |
| Worker y ejecución en el sitio | `zirk-lang-site/src/app/features/playground/` |
| Diseño visual | `zirk-lang-site/DESIGN.md`, `game-assets/src/palette.js` |
