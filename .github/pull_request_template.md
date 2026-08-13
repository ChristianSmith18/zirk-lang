<!--
  Base: develop (salvo release/* o hotfix/*, que van a main).
  Ver CONTRIBUTING.md
-->

## Qué cambia

<!-- Descripción breve. Qué problema resuelve, no cómo. -->

## Fase del roadmap

<!-- Ej: Fase 0 — cimientos. Confirmá que no adelanta features de fases posteriores. -->

- Fase:
- Change de OpenSpec (si aplica):

## Checklist

- [ ] `cargo fmt --all --check` pasa
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` pasa
- [ ] `cargo test --workspace` pasa
- [ ] No adelanta features de fases posteriores del roadmap
- [ ] Si toca reglas del lenguaje: hay al menos un caso válido y uno inválido en tests
- [ ] Si agrega diagnósticos: tienen código estable, causa y ayuda accionable
- [ ] Si toma una decisión de arquitectura: hay un ADR en `docs/decisions/`

## Ambigüedades encontradas

<!--
  Regla del spec: toda ambigüedad debe producir una pregunta o quedar
  documentada, nunca resolverse en silencio. Si el spec no cubría algo,
  decilo acá. Si no hubo ninguna, escribí "ninguna".
-->

## Verificación

<!-- Cómo comprobaste que funciona. Salida de tests, comandos, capturas. -->
