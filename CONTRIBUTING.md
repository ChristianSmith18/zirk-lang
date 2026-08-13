# Contribuir a Zirk

## La regla que manda sobre todas

**Las specs de `docs/` son normativas y están en inglés.** Ante cualquier duda de sintaxis, semántica o alcance, mandan sobre criterio propio, memoria de otros lenguajes o "lo que suena razonable".

Y su corolario, que es regla explícita del propio spec:

> Toda ambigüedad debe producir una pregunta o quedar documentada — **nunca resolverse en silencio inventando comportamiento no especificado.**

Si algo hace falta para avanzar y el spec no lo cubre, se dice explícitamente antes de decidir.

## La segunda regla: no adelantarse de fase

El spec describe a Zirk **maduro**: es el resultado de años de trabajo, no el punto de partida. El trabajo avanza por fases según [ZIRK_ROADMAP.md](docs/init/ZIRK_ROADMAP.md).

No se implementan features de fases futuras aunque estén documentadas, definidas, y sea tentador porque "está todo ahí". Si en el camino aparece la necesidad **real** de algo de una fase posterior, se anota como nota pendiente y se sigue con la fase actual.

Una fase se da por completa cuando su "Salida" corre con tests reales, no cuando el código "está casi".

## Toolchain

Ver [docs/TOOLCHAIN.md](docs/TOOLCHAIN.md). Resumen: Rust 1.94+ (lo fija `rust-toolchain.toml`) y LLVM **20.1** con bibliotecas estáticas, con `LLVM_SYS_201_PREFIX` apuntando a su prefijo.

En Windows, **ninguna** distribución oficial de LLVM funciona con `llvm-sys`: el instalador `.exe` no trae bibliotecas estáticas, y el tarball de desarrollo está compilado contra una CRT distinta de la que usa Rust. Ver [TOOLCHAIN.md](docs/TOOLCHAIN.md) para la fuente que sí funciona.

## Modelo de ramas: git flow

```
   feature/*  ──▶  develop  ──▶  release/*  ──▶  main
   hotfix/*   ──────────────────────────────▶  main + develop
```

| Rama | Para qué |
|---|---|
| `main` | releases. Solo recibe merges de `release/*` y `hotfix/*` |
| `develop` | integración. Es la base de toda feature |
| `feature/*` | trabajo nuevo. Sale de `develop` y vuelve a `develop` |
| `release/*` | estabilización de una versión |
| `hotfix/*` | corrección urgente sobre un release |

**No se commitea directo a `main` ni a `develop`.** Todo entra por PR.

```sh
git checkout develop
git pull
git checkout -b feature/lexer-tokens
# ... trabajo ...
gh pr create --base develop
```

## Mensajes de commit

[Conventional Commits](https://www.conventionalcommits.org/):

```
feat(lexer): reconocer literales de duración
fix(codegen): corregir alineación en targets de 32 bits
docs(adr): registrar la decisión de estrategia de memoria
test(diagnostics): cubrir el renderizado sin fragmento de source
chore(ci): cachear la instalación de LLVM en Windows
```

Ámbitos habituales: `lexer`, `parser`, `ast`, `sema`, `ir`, `codegen`, `diagnostics`, `cli`, `runtime`, `ci`, `docs`, `adr`.

## Antes de abrir una PR

```sh
./scripts/check-local.sh
```

Ejecuta lo mismo que CI —formato, clippy y tests— detectando `LLVM_SYS_201_PREFIX` y validando la versión de LLVM antes de empezar.

### Dónde se verifica cada plataforma

CI cubre `linux-x86_64`, `linux-aarch64`, `macos-aarch64` y `windows-x86_64`.

Que funcione en tu máquina no es evidencia de que funcione en Linux ni en Windows: para eso está CI.

## Qué acompaña a cada feature

`ZIRK_SPEC_FINAL.md` sección 8 exige que cada característica venga con gramática, reglas de tipos, semántica observable, diagnósticos, ejemplos válidos e inválidos, y pruebas de conformidad.

En la práctica, y como mínimo:

- **un caso válido de test y un caso inválido** — es regla explícita del spec: cada regla del lenguaje debe tener al menos uno de cada;
- **diagnósticos con causa y ayuda** cuando el caso inválido produzca un error;
- **documentación de responsabilidad y límite** si tocás un crate nuevo.

## Diagnósticos

Todo error del compilador sigue el formato de `ZIRK_COMPILER_SPEC.md` sección 8:

```text
error[E0308]: incompatible types
  src/main.zrk:4:24
  |
4 |     mut total: Int32 = "cuarenta";
  |                        ^^^^^^^^^^ expected Int32, found String
  |
  = cause: there is no implicit conversion from String to Int32
  = help: use Int32.parse("cuarenta") to convert at runtime
```

### Idioma

**En inglés**: las specs normativas, el roadmap, el prompt de arranque, el código, sus comentarios, los mensajes de diagnóstico y los nombres de test.

**En español**: los ADRs, este archivo, el README, `docs/TOOLCHAIN.md`, los artefactos de OpenSpec y los mensajes de commit.

La frontera: **lo que define Zirk va en inglés; lo que registra cómo lo estamos construyendo va en español.** El razonamiento, y las dos veces que esta frontera estuvo mal puesta, están en [ADR-006](docs/decisions/ADR-006-language-of-the-codebase.md).

Se construye con `zirk-diagnostics`. Los códigos son **estables**: uno publicado no se reutiliza para un error semánticamente distinto.

Si no hay una reparación clara, se omite la ayuda. Una ayuda genérica sin valor accionable es peor que ninguna.

## Decisiones de arquitectura

Las decisiones que cascadean al resto del proyecto se registran como ADRs en [docs/decisions/](docs/decisions/). Son la **fuente durable**: un change de OpenSpec se archiva, un ADR no.

Se escribe un ADR cuando la decisión afecta a varias capas, es cara de revertir, o alguien va a preguntar en seis meses "¿por qué está hecho así?". Ejemplos ya registrados: el pin de LLVM, la estrategia de memoria, la frontera del runtime.

**Consultá antes de decidir por tu cuenta** en: estrategia de memoria, estructura de crates, formato interno de IR, o cualquier cosa que el roadmap no haya resuelto ya.

## OpenSpec

El proyecto usa [OpenSpec](https://github.com/Fission-AI/OpenSpec) para planificar. **Un change por fase del roadmap.**

```sh
openspec list                              # changes activos
openspec status --change fase-0-bootstrap  # progreso
openspec validate fase-0-bootstrap
```

Cada change tiene propuesta, diseño, specs de capacidades y tareas. El `design.md` referencia los ADRs en vez de duplicarlos.
