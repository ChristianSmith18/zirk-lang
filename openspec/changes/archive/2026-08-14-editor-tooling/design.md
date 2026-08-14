## Context

El soporte de editor se desarrolló en la rama `feature/syntax-highlighting` y se trajo a `develop` como código. Este change lo registra retroactivamente.

No toca ningún crate del compilador: son archivos de configuración de VS Code más un script de formateo. El riesgo técnico es nulo; lo que sí hay son dos decisiones que conviene dejar escritas.

## Goals / Non-Goals

**Goals:**

- Escribir Zirk con resaltado que refleje el lenguaje tal como lo definen las specs.
- Que la cobertura del léxico sea un requisito verificable y no algo que se comprobó una vez.

**Non-Goals:**

- LSP, diagnósticos en el editor, autocompletado, navegación, renombrado. Requieren el frontend incremental de `ZIRK_COMPILER_SPEC.md` sección 10 y son Fase 9.
- Definir el estilo canónico de Zirk. Eso es `zirk format`.

## Decisions

### D1 — El resaltador cubre el lenguaje completo, no el subset implementado

Un editor que solo resaltara `fn`, `mut` e `if` daría a entender que `class` o `task` no son parte de Zirk, cuando el spec los define y el compilador ya los reconoce para decir en qué fase llegan.

La división del trabajo queda así: **el editor muestra el lenguaje; el compilador dice qué está disponible.**

**Consecuencia:** cada palabra clave que se agregue al lexer en las Fases 2 a 5 debe agregarse también al resaltador. Es una deuda que se degrada en silencio, y por eso la correspondencia con el lexer quedó como requisito verificable en vez de como comentario.

### D2 — El formateador de la extensión se conserva, marcado como provisional

`ZIRK_COMPILER_SPEC.md` sección 10 exige que el formateador oficial sea **canónico, idempotente y sin configuración que fragmente el estilo**. El de la extensión respeta `tabSize` e `insertSpaces` del editor, que es exactamente lo que ese requisito descarta. Tampoco conoce cadenas ni comentarios: una línea que termine en `{` dentro de un comentario desplaza la indentación siguiente.

**Alternativa descartada:** quitarlo hasta que exista `zirk format`. Se descarta porque dejaría a quien escribe Zirk hoy sin ninguna ayuda de indentación durante ocho fases, a cambio de una pureza que nadie observa: un formateador de editor no es el formateador del lenguaje, y nadie va a confundirlos si está dicho.

**Lo que sí se hizo** es dejarlo escrito donde se lee: el README de la extensión explica en qué se aparta del spec y que debe retirarse cuando `zirk format` exista.

## Risks / Trade-offs

- **El resaltado se desactualiza al crecer el lenguaje** → Mitigación: la correspondencia con el lexer es un requisito de la spec, no una nota. Conviene automatizarla como test cuando haya más de un editor soportado.

- **El formateador provisional se queda para siempre** → Mitigación: el requisito dice explícitamente que debe delegar en `zirk format` cuando exista, así que la deuda tiene condición de salida escrita.

- **Solo se soporta VS Code** → Aceptado. La gramática TextMate es reutilizable por otros editores; la configuración y el formateador no. Se resuelve cuando alguien lo necesite.

## Migration Plan

No aplica: es puramente aditivo y no toca el compilador.

## Open Questions

Ninguna.
