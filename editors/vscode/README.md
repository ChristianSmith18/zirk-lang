# Zirk Language Support

VS Code extension offering syntax highlighting and basic language configuration for the Zirk programming language.

## Features

- **Full Syntax Highlighting:** Covers the entire Zirk language specification including keywords, operators, decorators (`@test`, `@e2e`, `@bench`), variables, types, and strings (with escape sequence support and string interpolation).
- **Auto-Closing Pairs:** Automatic completion of curly braces `{}`, square brackets `[]`, parentheses `()`, and quotation marks `""`.
- **Comment Toggling:** Standard toggle for line comments (`//`) and block comments (`/* ... */`).

## Installation for Local Testing

To test the extension locally in VS Code, Cursor, Windsurf, or any other VS Code fork:

### Option A: Install via symlink (recommended for development)

1. Create a symlink to the `editors/vscode` directory inside your VS Code extension folder:
   ```bash
   ln -s "/Users/cristian/Projects/PROPIOS/zirk-lang-syntax/editors/vscode" ~/.vscode/extensions/zirk-lang
   ```
2. Restart your editor (VS Code, Cursor, Windsurf, etc.).
3. Open any file with `.zrk` extension (like the corpus examples).

### Option B: Build a portable `.vsix` package

1. Install the VS Code extension manager globally:
   ```bash
   npm install -g @vscode/vsce
   ```
2. Run the packaging command from the extension folder (`editors/vscode`):
   ```bash
   cd editors/vscode
   vsce package
   ```
3. Install the generated `.vsix` file in your editor via the command palette (`Developer: Install Extension from VSIX...`).

## Sobre el formateo

La extensión trae un formateador propio, básico, que solo alinea la
indentación siguiendo llaves y corchetes. Existe porque hoy **no hay
`zirk format`**: ese comando llega en la Fase 9 del roadmap.

Conviene saber en qué se diferencia del que vendrá:

- `ZIRK_COMPILER_SPEC.md` sección 10 define el formateador oficial como
  **canónico, idempotente y sin configuración que fragmente el estilo**. El de
  esta extensión respeta `tabSize` e `insertSpaces` del editor, que es
  exactamente el tipo de configuración que el spec descarta.
- No conoce cadenas ni comentarios: una línea que termine en `{` dentro de un
  comentario desplaza la indentación de lo que sigue.

Cuando exista `zirk format`, la extensión debe delegar en él y este formateador
se retira. Hasta entonces es una comodidad provisional, no la definición del
estilo de Zirk.

## Palabras clave

El resaltador cubre el **lenguaje completo** que definen las specs, no solo el
subset que el compilador implementa hoy. Es deliberado: el editor muestra el
lenguaje tal como está especificado, y es el compilador el que dice qué
construcción todavía no está disponible y en qué fase llega.
