# ADR-006 — El proyecto se escribe en inglés

- **Estado:** aceptada
- **Fecha:** 13 de agosto de 2026
- **Fase:** 1

## Contexto

Ninguno de los cinco documentos normativos define **en qué idioma** se emiten los mensajes del compilador. La ambigüedad pasó desapercibida hasta que la Fase 1 empezó a producir diagnósticos reales.

El proyecto está documentado en español, y los primeros diagnósticos se escribieron en español por continuidad. Pero el idioma de la documentación interna y el de la salida del compilador son decisiones distintas: la primera afecta a quien desarrolla Zirk, la segunda a quien lo usa.

### La complicación

`ZIRK_COMPILER_SPEC.md` sección 8 fija el formato mínimo con este ejemplo:

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

Las etiquetas aparecen en español. Caben dos lecturas:

1. **`causa:` y `ayuda:` son cadenas normativas** que la implementación debe emitir literalmente.
2. **El ejemplo es prosa ilustrativa** que describe la *estructura* —severidad, código, ubicación, causa, ayuda— usando el mismo idioma que el resto del documento.

## Decisión

**El proyecto se escribe en inglés.** Esto cubre:

- las **cinco specs normativas** de `docs/`;
- el **roadmap** y el **prompt de arranque** de `docs/init/`;
- **identificadores de código**: funciones, variables, tipos, constantes y nombres de test;
- **comentarios y documentación de código** (`//`, `///`, `//!`);
- mensajes, causas, ayudas, las etiquetas del formato y la salida de la CLI;
- los scripts del repositorio.

Se adopta la **segunda lectura** del spec: el ejemplo es ilustrativo. Lo normativo es que existan una causa y una ayuda, no las palabras con las que se rotulan. El resto del documento está en español porque el documento está en español, no porque el compilador deba hablar español.

Las etiquetas quedan:

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

### Qué **no** cambia

Sigue en español la documentación de trabajo *alrededor* del proyecto, la que registra cómo se construye y por qué, y no lo que Zirk es:

- los ADRs, incluido este;
- `README.md`, `CONTRIBUTING.md` y `docs/TOOLCHAIN.md`;
- los artefactos de OpenSpec y los mensajes de commit.

La frontera: **lo que define Zirk va en inglés; lo que registra cómo lo estamos construyendo va en español.**

### Cómo llegamos acá

Este ADR se corrigió dos veces, y las dos correcciones vale dejarlas escritas porque muestran dónde estaba mal puesta la frontera:

1. **Primera versión:** solo los diagnósticos en inglés, con la frontera en "lo que ve quien usa Zirk contra lo que ve quien lo construye". Falló porque un contribuyente externo lee el código antes que cualquier documento: `sincronizar()` es una barrera de entrada tan real como un mensaje de error traducido.

2. **Segunda versión:** además el código, con la frontera en "código contra documento". Falló porque las specs normativas **no son documentación del proyecto: son la definición del lenguaje**. Alguien que quiera entender Zirk las lee antes que el código, y son el artefacto más público que el proyecto tiene.

## Motivo

Es el estándar de facto del ecosistema. Rust, Go, Zig, Swift, TypeScript y Clang emiten diagnósticos en inglés, y sus usuarios —incluidos los hispanohablantes— buscan esos mensajes en inglés cuando algo falla. Un diagnóstico en español no tiene resultados en Stack Overflow ni en la documentación de nadie.

También afecta al futuro del proyecto:

- `ZIRK_COMPILER_SPEC.md` sección 9 exige salida estructurada (`--json`) para herramientas. Un LSP, un linter o un CI que consuman esos mensajes esperan inglés.
- Un compilador que habla español acota su base de contribuyentes y de usuarios sin ganar nada a cambio.

## Alternativas descartadas

**Español, por coherencia con la documentación.** Coherente hacia adentro, hostil hacia afuera. Confunde dos audiencias que no son la misma.

**Diagnósticos traducibles con selección de idioma.** Es lo que hace `rustc` con `--error-format` y traducciones parciales, y es trabajo real: catálogo de mensajes, parametrización, y el riesgo de que las traducciones queden desfasadas de los códigos. No corresponde a la Fase 1, y este ADR no lo impide: los códigos estables son justamente el gancho que lo haría posible más adelante.

## Consecuencias

- Todos los mensajes, identificadores, comentarios y nombres de test del workspace se traducen.
- Las etiquetas de `zirk-diagnostics` pasan a `cause:` y `help:`.
- Los códigos de diagnóstico se renombran a inglés: `TOKEN_INESPERADO` → `UNEXPECTED_TOKEN`, `CADENA_SIN_CERRAR` → `UNTERMINATED_STRING`, etc. Los **códigos estables** (`E0301`, `E0202`) no cambian: son el contrato con herramientas y documentación.
- Los archivos de test se renombran: `lexico.rs` → `lexical.rs`, `gramatica.rs` → `grammar.rs`.
- `ZIRK_COMPILER_SPEC.md` sección 8 ya usa las etiquetas reales `cause:` y `help:`, así que la ambigüedad que originó este ADR desapareció.
- `docs/init/ZIRK_AGENT_PROMPT.md` abre declarando esta regla, para que cualquier agente que retome el proyecto la lea antes que cualquier otra cosa.
