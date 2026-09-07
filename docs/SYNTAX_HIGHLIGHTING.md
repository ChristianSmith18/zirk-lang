# Zirk Syntax Highlighting

Canonical color theme for Zirk source code. It is the single source of truth
for every surface that renders Zirk code: the VS Code extension
(`editors/vscode`), the website (`zirk-lang-site`), and documentation
previews.

The palette is applied through TextMate scopes emitted by
`editors/vscode/syntaxes/zirk.tmLanguage.json`. Every scope ends in `.zirk`,
so the rules never affect other languages. The VS Code extension applies
these rules at runtime when `zirk.brandColors` is enabled (default: `true`);
the website maps the same scopes through Shiki in
`zirk-lang-site/src/app/core/config/zirk-syntax.config.ts`.

## Editor surface

| Element              | Value     |
|----------------------|-----------|
| Editor background    | `#111118` |
| Editor foreground    | `#F7F7FA` |

## Color per language element

| Language element                                   | Scope(s)                                                  | Color     | Style   |
|----------------------------------------------------|-----------------------------------------------------------|-----------|---------|
| Declaration keywords (`class`, `trait`, `interface`, `record`, `enum`, `import`, `use`, `fn`, `construct`, `extends`, `implements`, `type`, `gen`) | `keyword.declaration`, `keyword.control.import`, `storage.type.function` | `#8E6CFF` | normal  |
| Control flow (`if`, `else`, `match`, `for`, `while`, `loop`, `try`, `catch`, `task`, `await`, `select`, `parallel`, …) | `keyword.control`                                         | `#EC4899` | normal  |
| Exit keywords (`return`, `throw`, `break`, `continue`, `yield`) | `keyword.control.exit`                                    | `#F7C65C` | normal  |
| Mutability modifiers (`mut`, `inmut`, `inmut::strict`, `share`, `sync`, `strict`) | `storage.modifier.mutability`                             | `#F7C65C` | normal  |
| Other modifiers (`public`, `private`, `protected`, `abstract`, `final`, `inner`, `static`, `unsafe`, `repeatable`, …) | `storage.modifier`                                        | `#A78BFA` | normal  |
| Built-in types (`Int32`, `String`, `Void`, `Result`, `Option`, …) | `storage.type`                                            | `#32D4C6` | normal  |
| User-defined types (`Capitalized` names)           | `entity.name.type`, `entity.name.class`                   | `#34D399` | normal  |
| Function/method declarations                       | `entity.name.function`                                    | `#39C6FF` | normal  |
| Function/method calls                              | `entity.name.function.call`                               | `#38BDF8` | normal  |
| Built-ins (`stdin`, `stdout`, `stderr`, `print`, `println`, `assert`, …) | `support.function`, `support.variable`                    | `#39C6FF` | normal  |
| Strings, chars, regex (`"…"`, `'x'`, `re'…'`)      | `string.quoted.double`, `string.regexp`, `constant.character` | `#FF4FA3` | normal  |
| Numeric literals (integer, float, hex, octal, binary) and durations (`10ms`, `5s`) | `constant.numeric.*`                                      | `#60A5FA` | normal  |
| Language constants (`true`, `false`, `null`, `Ok`, `Err`, `Some`, `None`) | `constant.language`                                       | `#FF6B81` | normal  |
| `this`, `super`, `outer`                           | `variable.language`                                       | `#FFD166` | normal  |
| Fields after `this.` / `super.` / `outer.` and ordinary variables | `variable.other.readwrite`                                | `#F7F7FA` | normal  |
| Named arguments (`name:` in calls)                 | `variable.parameter`                                      | `#B8B8C3` | italic  |
| Decorators (`@name`)                               | `meta.decorator`, `entity.name.function.decorator`, `punctuation.decorator` | `#F4A261` | normal  |
| Member markers (`#override`)                       | `comment.line.marker`, `punctuation.definition.marker`    | `#6A6A75` | normal  |
| Module paths in imports (`std.io`)                 | `support.class`                                           | `#F4A261` | normal  |
| Imported names without alias (`{ stdout }`)        | `variable.other.readwrite`                                | `#F7F7FA` | normal  |
| Original name in an import alias (`Animal -> Creature`) | `comment.line.marker`                                 | `#6A6A75` | normal  |
| Alias target after `->`                            | `entity.name.type` / `variable.other.readwrite`           | type/variable color | normal |
| Operators (`+`, `-`, `=`, `==`, `->`, `is`, `\|>`, `?.`, `??`, …) | `keyword.operator.*`                                      | `#A78BFA` | normal  |
| Interpolation punctuation `{`/`}` inside strings   | `punctuation.section.embedded.*`, `punctuation.definition.placeholder`, `punctuation.separator.placeholder` | `#8E6CFF` | normal  |
| Interpolated content inside `{…}`                  | `string.quoted.double source.zirk`                        | `#F7F7FA` | normal  |
| Line and block comments (`//`, `/* */`)            | `comment.line.double-slash`, `comment.block`              | `#A3A3AF` | italic  |

## Scope mapping notes

- The website theme (`ZIRK_SYNTAX_THEME`) groups scopes more broadly than
  the editor rules above; when a surface does not need the extended
  categories, the canonical base colors are: keywords/operators `#8E6CFF`,
  functions `#39C6FF`, types `#32D4C6`, strings `#FF4FA3`, variables/text
  `#F7F7FA`, numbers/constants `#39C6FF`, comments `#A3A3AF`, decorators
  `#8E6CFF`, modules `#F7F7FA`.
- Numeric literals use specialized scopes (`constant.numeric.integer`,
  `constant.numeric.float`, `constant.numeric.hex`, `constant.numeric.octal`,
  `constant.numeric.binary`, `constant.numeric.duration`); consumers must
  map each one, not only `constant.numeric`.
- `keyword.control.import` is a dedicated scope so `import`/`from` lines
  keep the declaration color instead of the control-flow color.
- Inside string interpolation, `{`/`}` are violet and the embedded
  expression keeps the normal text color (`#F7F7FA`).
- `#override` and other `#` member markers are deliberately muted
  (`#6A6A75`) so they read as annotations, not code.

## Where it is implemented

- VS Code palette rules: `editors/vscode/extension.js` (`BRAND_TEXTMATE_RULES`).
- Grammar scopes: `editors/vscode/syntaxes/zirk.tmLanguage.json`.
- Toggle: `zirk.brandColors` in `editors/vscode/package.json` (default `true`).
- Website theme: `zirk-lang-site/src/app/core/config/zirk-syntax.config.ts`
  (`ZIRK_SYNTAX_PALETTE`, `ZIRK_SYNTAX_THEME`).
- Visual preview: `zirk-palette-preview.html` (repo root).
