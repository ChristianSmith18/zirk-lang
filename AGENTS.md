# Zirk — reglas para agentes

## Commits

- **Nunca** agregar `Co-Authored-By`, `Generated with`, ni ningún trailer o
  co-autor de agente/herramienta a los mensajes de commit. Los commits van
  solo con el mensaje del autor del repo.
- Estilo de mensajes: español, prefijos tipo `docs(scope):`, o descripciones
  cortas del cambio (ver `git log` para ejemplos).

## Verificación

- Tests: `cargo test -p <crate>` (zirk-lexer, zirk-parser, zirk-sema,
  zirk-ir, zirk-cli).
- Extensión VS Code: tras editar `editors/vscode/*`, copiar a
  `~/.devin/extensions/christiansmith.zirk-lang-0.4.2/` y validar JSON/JS.
- Sync de docs al sitio: `./scripts/sync-website-content.sh` (requiere docs
  commiteados y el repo `../zirk-lang-site` limpio; usar `--audit-date
  YYYY-MM-DD` si cambió project-status).
