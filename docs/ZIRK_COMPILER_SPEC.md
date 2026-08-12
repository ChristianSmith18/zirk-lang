# Zirk — Especificación del compilador y tooling

## 1. Objetivos

El compilador debe priorizar diagnósticos seguros, compilación incremental, binarios nativos eficientes y resultados reproducibles. Formatter, linter, `check` y LSP deben operar sobre el frontend incremental sin invocar LLVM.

Metas de rendimiento como “milisegundos para aproximadamente 100 archivos” son objetivos medidos, no garantías independientes del hardware. El proyecto mantendrá benchmarks públicos de startup, parsing, formatting, análisis incremental, build completo, build sin cambios, LSP, memoria máxima y tamaño de binario.

## 2. Pipeline

```text
source .zrk
    ↓
lexer incremental
    ↓
parser con árbol de sintaxis
    ↓
resolución de módulos y nombres
    ↓
type checker y análisis de flujo
    ↓
expansión validada de decoradores
    ↓
IR tipada y portable
    ↓
LLVM IR
    ↓
objeto, linker y binario Mach-O / ELF / PE
```

El frontend comparte estructuras persistentes, interning de símbolos, cachés por contenido y un grafo fino de dependencias. Solo se invalidan archivos y símbolos afectados.

## 3. Árboles y Syntax API

La AST semántica interna es privada y puede evolucionar con el compilador. Herramientas, IDEs y decoradores usan una **Syntax API pública, versionada, inmutable y validada**.

La API pública permite:

- inspeccionar tokens, nodos, tipos, firmas, atributos y ubicaciones;
- recorrer declaraciones públicas y metadata autorizada;
- construir transformaciones mediante builders tipados;
- emitir diagnósticos asociados a source spans;
- solicitar reflection explícita.

No permite mutar memoria interna, fabricar nodos inválidos, omitir el type checker ni acceder al filesystem/red/procesos sin `compile_permissions`.

Las transformaciones se vuelven a parsear, resolver, tipar y validar. El compilador conserva trazabilidad entre código original y generado para diagnósticos y debugging.

## 4. IR y paquetes

La IR es tipada, independiente del target y versionada. Conserva suficiente información para especialización de genéricos, devirtualización, escape analysis, comprobaciones de seguridad, vectorización y generación de debug info.

Un `.zpkg` contiene conceptualmente:

```text
package.zpkg
├── manifest
├── public.api
├── portable.ir
├── README.md
├── LICENSE
└── documentation
```

`public.api` expone únicamente tipos, firmas, traits, interfaces, contratos de decoradores y documentación. La implementación portable se transforma al target durante el build de la aplicación. El core, runtime, stdlib, dependencias y código de la aplicación deben alinearse al mismo target y ABI.

## 5. Backend

LLVM es el único backend inicial. Una interfaz interna permite añadir otro backend en el futuro sin cambiar la semántica pública, pero no se mantendrán varios inicialmente.

- Debug: optimización mínima, símbolos completos y correspondencia clara con source.
- Release: optimización alta, eliminación de código muerto, LTO cuando sea apropiado, vectorización y optimización de tamaño/memoria sin alterar garantías.

No se implementa assembly textual inline. Zirk ofrece intrinsics portables y vectores SIMD; el backend selecciona instrucciones por arquitectura y emite una alternativa segura cuando no exista una instrucción equivalente.

## 6. Targets y cross-compilation

Formato canónico sugerido:

```text
x86-windows
x86_64-windows
x86-linux
x86_64-linux
armv7-linux
aarch64-linux
aarch64-macos
x86_64-macos
```

La validez depende del sistema operativo, LLVM, linker, SDK y dependencias nativas. macOS moderno de 32 bits no se promete.

Resolución:

1. `zirk build --target ...`;
2. todos los `build_targets` de `init.zrk`;
3. detección del host.

Un target de CLI reemplaza temporalmente `build_targets`. El compilador verifica temprano bibliotecas nativas incompatibles y explica qué dependencia bloquea el target.

WebAssembly, browser, DOM y directivas de plataforma están fuera de Zirk 1.x.

## 7. Builds incrementales y reproducibles

Las claves de caché incluyen contenido, versión de compilador, flags relevantes, target, API de dependencias, versión de IR y configuración. El sistema reutiliza parsing, tipos, IR, objetos y paquetes no invalidados.

Un build bloqueado por `zirk.lock` debe usar exactamente versiones y hashes registrados. `zirk update` recalcula la resolución. Artefactos release destinados a distribución deben poder reproducirse con el mismo source, lockfile, toolchain y target.

## 8. Diagnósticos

Formato mínimo:

```text
error[E1234]: descripción precisa
  src/users.zrk:18:12
   |
18 |     expresión problemática
   |            ^ explicación localizada
   |
   = causa: motivo semántico
   = ayuda: acción concreta
```

Todo diagnóstico debe incluir severidad, código estable, ubicación, causa y ayuda cuando exista una reparación clara. Los errores generados por decoradores muestran tanto el source original como la expansión relevante.

Los warnings no cambian la semántica. Categorías configurables incluyen código inalcanzable, símbolo sin uso, shadowing confuso, cast redundante, permiso innecesario y operación cuyo resultado se ignora. `--warnings-as-errors` puede elevarlos.

## 9. CLI oficial

Comandos mínimos:

```text
zirk new <name>       crea un proyecto nuevo
zirk init             inicializa Zirk en un directorio existente
zirk run              compila incrementalmente y ejecuta
zirk build            genera artefactos
zirk check            analiza sin generar código
zirk test             ejecuta tests
zirk bench            ejecuta benchmarks
zirk format           aplica formato canónico
zirk lint             ejecuta reglas estáticas
zirk prepare          audita permisos, targets y publicación
zirk add/remove       modifica dependencias
zirk install          resuelve dependencias
zirk update           actualiza zirk.lock explícitamente
zirk package          crea .zpkg
zirk publish          publica un paquete
zirk doc              genera documentación
```

La CLI debe tener arranque rápido, salida determinista y modo estructurado (`--json`) para herramientas.

## 10. Formatter, linter y LSP

El formatter es canónico, idempotente y sin configuración que fragmente el estilo. Agrega `;`, normaliza espacios, llaves, saltos y orden donde sea semánticamente neutro.

El linter comparte parser, resolución y tipos con el compilador. Las reglas rápidas se ejecutan por defecto; análisis costosos son explícitos. Los fixes automáticos deben ser seguros y revisables.

El LSP usa snapshots incrementales, cancelación de solicitudes obsoletas y prioridades interactivas. Ofrece diagnósticos, completion, hover, navegación, referencias, rename, formatting, semantic tokens, firmas y code actions.

## 11. Debugger

El toolchain genera símbolos y mapeo fiel a `.zrk`, incluyendo async/tasks y código decorado. Debe permitir breakpoints, step, inspección de variables, stack traces, threads, tasks y channels. Las optimizaciones release pueden limitar la observabilidad y deben indicarlo.

## 12. Tests del compilador

Se requieren suites de lexer/parser, snapshots de diagnósticos, type checker, seguridad, IR, codegen por target, ABI, incrementalidad, reproducibilidad, formatter idempotente, fuzzing y differential tests entre debug/release cuando proceda.

Cada regla del lenguaje debe tener al menos un caso válido y uno inválido.
