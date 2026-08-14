## Why

El soporte de editor se desarrolló en una rama aparte y se trajo a `develop` como código, sin pasar por OpenSpec. Este change lo registra retroactivamente para que la capacidad quede en `openspec/specs/` con requisitos verificables, en vez de existir solo como archivos en `editors/`.

Lo que motiva registrarlo no es la ceremonia: **el resaltado tiene una propiedad que se degrada en silencio**. Cada palabra clave que se agregue al lexer en las Fases 2 a 5 —y son muchas— puede quedar sin resaltar sin que nada avise. Convertir "el resaltador cubre todas las palabras clave" en un requisito hace que esa deuda sea visible.

## What Changes

- **Extensión de VS Code** en `editors/vscode/`, con resaltado de sintaxis, configuración de lenguaje —comentarios, pares de cierre, plegado— y un formateador provisional.
- **Cobertura completa del léxico**: las 46 palabras clave que reconoce `zirk-lexer`, verificado por comparación cruzada.
- El resaltador cubre el **lenguaje completo** de las specs, no solo el subset implementado.

Explícitamente **fuera de alcance**: LSP, diagnósticos dentro del editor, autocompletado y navegación. Todo eso es Fase 9 y requiere que el compilador exponga un frontend incremental.

## Capabilities

### New Capabilities

- `editor-tooling`: el soporte de editor para escribir Zirk — resaltado de sintaxis, configuración del lenguaje y su relación con el formateador canónico que llega en la Fase 9.

### Modified Capabilities

Ninguna.

## Impact

**Nuevo:** `editors/vscode/` y una entrada en `.gitignore` para `*.vsix`.

**Sin efecto sobre el compilador:** no se toca ningún crate. La suite de tests y CI no cambian.

**Tensión conocida:** el formateador de la extensión respeta la configuración del editor, y `ZIRK_COMPILER_SPEC.md` sección 10 exige que el formateador oficial sea canónico y sin configuración que fragmente el estilo. Se conserva como comodidad provisional mientras `zirk format` no exista, documentado en el README de la extensión.
