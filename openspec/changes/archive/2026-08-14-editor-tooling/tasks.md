## 1. Extensión de VS Code

- [x] 1.1 Crear la estructura en `editors/vscode/` con su manifiesto
- [x] 1.2 Asociar la extensión `.zrk` al lenguaje Zirk
- [x] 1.3 Definir la gramática de resaltado en `syntaxes/zirk.tmLanguage.json`
- [x] 1.4 Definir comentarios, pares de cierre y plegado en `language-configuration.json`

## 2. Cobertura del léxico

- [x] 2.1 Cubrir las palabras clave del subset implementado
- [x] 2.2 Cubrir las palabras clave de fases posteriores
- [x] 2.3 Verificar por comparación cruzada contra `zirk-lexer` que la cobertura es completa
- [x] 2.4 Resaltar `this` como referencia al valor actual y no como control de flujo

## 3. Formateador provisional

- [x] 3.1 Implementar el proveedor de formato basado en indentación
- [x] 3.2 Documentar en qué se aparta de lo que exige `COMPILER_SPEC` sección 10
- [x] 3.3 Registrar que debe delegar en `zirk format` cuando exista

## 4. Cierre

- [x] 4.1 Verificar que los archivos de configuración son JSON válido
- [x] 4.2 Confirmar que no se toca ningún crate del compilador
