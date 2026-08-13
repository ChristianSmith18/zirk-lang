# ADR-006 — Los diagnósticos del compilador van en inglés

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

**Toda salida del compilador dirigida al usuario se emite en inglés.** Esto cubre mensajes, causas, ayudas, las etiquetas del formato y la salida de la CLI.

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

- **La documentación del proyecto sigue en español**: specs, ADRs, `CONTRIBUTING.md`, comentarios y documentación de código.
- **Los mensajes de commit siguen en español.**

La frontera es nítida: **lo que ve quien usa Zirk va en inglés; lo que ve quien construye Zirk va en español.**

## Motivo

Es el estándar de facto del ecosistema. Rust, Go, Zig, Swift, TypeScript y Clang emiten diagnósticos en inglés, y sus usuarios —incluidos los hispanohablantes— buscan esos mensajes en inglés cuando algo falla. Un diagnóstico en español no tiene resultados en Stack Overflow ni en la documentación de nadie.

También afecta al futuro del proyecto:

- `ZIRK_COMPILER_SPEC.md` sección 9 exige salida estructurada (`--json`) para herramientas. Un LSP, un linter o un CI que consuman esos mensajes esperan inglés.
- Un compilador que habla español acota su base de contribuyentes y de usuarios sin ganar nada a cambio.

## Alternativas descartadas

**Español, por coherencia con la documentación.** Coherente hacia adentro, hostil hacia afuera. Confunde dos audiencias que no son la misma.

**Diagnósticos traducibles con selección de idioma.** Es lo que hace `rustc` con `--error-format` y traducciones parciales, y es trabajo real: catálogo de mensajes, parametrización, y el riesgo de que las traducciones queden desfasadas de los códigos. No corresponde a la Fase 1, y este ADR no lo impide: los códigos estables son justamente el gancho que lo haría posible más adelante.

## Consecuencias

- Todos los mensajes existentes de `zirk-lexer`, `zirk-parser`, `zirk-codegen-llvm`, `zirk-cli` y el build script se traducen.
- Las etiquetas de `zirk-diagnostics` pasan a `cause:` y `help:`.
- Los tests que verifican texto de diagnósticos se actualizan.
- **Conviene corregir `ZIRK_COMPILER_SPEC.md` sección 8** para que su ejemplo use las etiquetas reales y la ambigüedad no reaparezca. Es un cambio al spec normativo y corresponde al autor decidirlo.
