# Análisis de factibilidad: juego educativo de Zirk (estilo Swift Playgrounds)

> Estado: **análisis actualizado el 2026-10-07**, tras auditar e implementar la base de OOP en el intérprete del navegador.
> Objetivo: evaluar qué implica un juego en el navegador con Three.js que enseñe Zirk y conceptos como OOP, con personajes y lecciones guiadas.
> El trabajo de intérprete vive en la rama `worktree-browser-oop` (worktree `.claude/worktrees/browser-oop`), con tres cambios OpenSpec: `browser-oop-conformance`, `browser-deep-clone` y `browser-execution-trace`.

## 1. Qué cambió respecto al primer análisis

El primer análisis decía que el intérprete del navegador no podía ejecutar OOP. **Era incorrecto.** Auditado contra el corpus de pruebas nativo (218 programas con salida registrada):

- 184 programas reproducen exactamente la salida nativa en el navegador; los otros 34 son exclusiones explícitas (concurrencia, timers, `parallel`, canales, punteros, native slices, journals `unsafe`, `Pin`/`Dependent`, transferencia de recursos y `clone()` profundo).
- Todo el modelo de objetos documentado en el handbook corre en el navegador: clases, constructores, `this`, visibilidad, herencia, `#override`, `final`, clases abstractas, interfaces, traits con implementaciones por defecto, miembros `static`, clases anidadas/`inner`/locales, clases genéricas, records y enums dentro de jerarquías, colecciones polimórficas y excepciones tipadas.
- Se agregaron 19 fixtures (`oop-*` y `clone-*`) con salida tomada del compilador nativo, un test de paridad contra todo el corpus y un catálogo de construcciones que viaja en el manifiesto del paquete.

Lo que sí faltaba y ya está implementado:

| Pieza | Estado |
|---|---|
| `clone()` profundo en el navegador (identidad nueva, sharing preservado, ciclos, closures con capturas, costo acotado) | Implementado, con 5 fixtures |
| Traza de ejecución opt-in (alloc, escritura de campo, call, return, throw, marcadores de output) para animar objetos reales sin que el alumno haga `println` de su estado | Implementado (ADR-022), `apiVersion` sigue en 1 |
| Mensajes de capacidad con nombre de característica ("deep `clone()` is unavailable…") en vez de un volcado de IR | Implementado |
| Corrección del compilador: una subclase que agregaba un método producía IR inválido | Corregido |

Pendiente de revisión humana: sincronizar con `zirk-lang-site` (paquete reconstruido, catálogo de estado y `--audit-date`), reconstruir y publicar el paquete, y mergear la rama a `develop` (hay que reconciliar `lower.rs` con el trabajo de `std.io` sin commitear).

## 2. Cómo ejecutaría el código el juego

El programa del alumno corre en el worker WASM; el juego no puede ser invocado desde el programa (ADR-020: sin efectos de host salvo salida acotada).

- **Camino A, ya viable:** el programa imprime comandos y el juego los anima. Para OOP, ahora hay una opción mejor: la **traza de ejecución** entrega eventos reales de objetos (`alloc` con clase, `fieldWrite` con campo, tipo y valor, `call` con receptor, `throw`), con ids deterministas por ejecución. El juego puede dibujar cada objeto como un personaje, ver sus campos cambiar y resaltar qué instancia recibe cada método (polimorfismo), sin instrumentar el código del alumno.
- **Camino B, sigue pendiente:** una API de host (`game.move()`) para que el programa cause efectos. Amplía la autoridad del host y necesita su propio ADR; no es necesario para el MVP.

Limitación: la ejecución es "corre y reproduce", no interactiva en vivo. Sensores como `front_is_blocked()` solo serían posibles pasando el mapa como datos.

## 3. Currículo propuesto

Unas 30 a 40 lecciones en 4 a 5 mundos.

| Mundo | Temas | ¿Corre hoy en el navegador? |
|---|---|---|
| 1 | Comandos y secuencia (`println`, orden de ejecución) | Sí |
| 2 | Variables y tipos, condicionales | Sí |
| 3 | Bucles y funciones con parámetros | Sí |
| 4 | Clases y objetos, constructores, encapsulación, identidad vs. copia (`clone()`) | Sí |
| 5 | Herencia, interfaces/traits, polimorfismo, genéricos, excepciones | Sí |
| Opcional | Concurrencia (`concurrent`/`spawn`) | No (excluida por ADR-020) |

Metáfora OOP sugerida: una "fábrica de robots". La clase es el plano; cada instancia es un robot (la traza muestra su `alloc`); la herencia son familias de robots; las interfaces son contratos de habilidades; el polimorfismo es el mismo comando ejecutado por cada robot a su manera; `clone()` es el duplicador, y la traza muestra por qué un alias comparte estado y un clon no.

Límites del lenguaje que condicionan las lecciones (documentados en `docs/browser-oop-coverage.md`, afectan igual al compilador nativo): una excepción propia no puede declararse en el `throws` de un método de clase; `finally` no corre si hay `return` dentro de un `catch`; una clase que implementa `B` (con `B implements A`) no se acepta donde se espera `A`; los literales `[Sq(2), Rect(2, 5)]` no infieren `List<Shape>`; y el runtime nativo compara las claves `String` de `Map`/`Set` por identidad (el navegador lo hace bien). Conviene diseñar las lecciones alrededor de esto hasta que se corrijan.

## 4. Integración y diseño

- **Encaje técnico:** ruta lazy `/learn/` en `zirk-lang-site` (Angular 21, signals, OnPush), canvas Three.js en un chunk aislado y el worker WASM existente. Presupuesto actual del paquete WASM: ya excede el límite de error de tamaño (4,008,382 B en la base, límite 4,000,000 B) y el smoke del formateador falla; ambos son anteriores a estos cambios y hay que resolverlos antes de publicar un paquete nuevo.
- **Diseño visual:** reutilizar la paleta de `DESIGN.md` (violeta `#8E6CFF`, cian `#39C6FF`, rosa `#FF4FA3`, aqua `#32D4C6`; Space Grotesk / Roboto Mono).
- **Accesibilidad:** el `AGENTS.md` del sitio exige AXE y WCAG AA. El canvas necesita una vista textual equivalente (la propia traza sirve como transcripción accesible), control por teclado y `prefers-reduced-motion`.
- **Contenido:** el handbook ya tiene "Zirk for new programmers" y capítulos por tema; hay que reescribirlos en formato guiado con objetivos, pistas y validadores. Los mensajes de error del compilador están pensados para desarrolladores y habría que traducirlos a lenguaje de aprendizaje.

## 5. Assets

- **Viable con buena calidad por código:** geometría low-poly y materiales estilizados en Three.js, partículas, animaciones procedurales, UI, iconos SVG y sonido sintetizado con WebAudio.
- **No viable con calidad profesional:** modelado y rig en Blender con ciclos de animación complejos, ilustración detallada.
- **Opciones:** packs CC0 (Kenney, Quaternius) en glTF; un estilo propio minimalista procedural (robot de primitivas con la paleta de marca); o contratar a un artista para la mascota.
- **Recomendación:** mascota procedural + entornos con packs CC0, y decidir después si vale la pena arte propio.

## 6. Estimación actualizada

Trabajo enfocado de Claude, sin contar revisiones tuyas. Los rangos no son de calendario.

| Hito | Contenido | Estimación |
|---|---|---|
| ~~Intérprete: OOP~~ | Auditoría, fixtures, `clone()` profundo, traza | **Hecho** (queda merge y handoff al sitio) |
| 0. Diseño | Currículo detallado, dirección de arte, formato de lección, validadores | 1 a 2 días |
| 1. Motor del juego | Escena Three.js, personajes, mapa, reproductor de traza, cámara, UI, integración con el worker, accesibilidad | 1 a 2 semanas |
| 2. Mundos 1-3 (~15 lecciones) | Contenido, niveles, pistas, validadores, tests | 1 a 2 semanas |
| 3. Mundos 4-5 (OOP, ~15-20 lecciones) | Contenido y niveles apoyados en la traza | 1 a 2 semanas |
| 4. Pulido | Persistencia de progreso, sonido, móvil, SEO, AXE, rendimiento | ~1 semana |
| **Total restante** | | **~5 a 8 semanas** (antes: 3 a 4 meses) |

## 7. Riesgos

1. **Lenguaje en movimiento:** hay trabajo activo en `std.io`, colecciones y tipos de datos; atar cada lección a un snapshot del compilador.
2. **Defectos del compilador que afectan a la pedagogía** (sección 3): decidir cuáles se corrigen antes de escribir lecciones.
3. **Paquete WASM sobre el presupuesto de tamaño** y smoke roto: bloquean publicar un paquete nuevo.
4. **Accesibilidad:** un canvas 3D con exigencia AXE/WCAG AA requiere diseño paralelo.
5. **Pedagogía:** la calidad de pistas y mensajes pesa más que el 3D.
6. **Alcance:** cerrar un vertical slice (un mundo completo y pulido) antes de seguir.

## 8. Ruta recomendada

1. Revisar y mergear la rama `worktree-browser-oop`; reconstruir el paquete y resolver tamaño/smoke; sincronizar `zirk-lang-site` con revisión humana.
2. Vertical slice (~2 semanas): mundo 4 (clases y objetos) con la traza, mascota procedural. Es el mundo que más se beneficia del trabajo ya hecho y valida pedagogía y tono.
3. Ampliar a los mundos 1-3 y 5.

## 9. Decisiones pendientes

- ¿El juego vive en `zirk-lang-site` (`/learn/`) o en un repo aparte?
- Origen de los assets: procedural, CC0 o artista.
- Idioma de las lecciones (el sitio documenta en inglés; definir i18n desde el inicio).
- ¿Se corrigen los defectos de compilador listados antes de escribir lecciones?
- Si la concurrencia entra en el alcance (requeriría el camino B y revisar ADR-020).
