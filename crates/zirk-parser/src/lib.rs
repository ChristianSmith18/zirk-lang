//! # zirk-parser
//!
//! **Responsabilidad:** construir el árbol de sintaxis de `zirk-ast` a partir de
//! los tokens de `zirk-lexer`, y reportar errores de gramática.
//!
//! **Límite:** el parser decide si el programa está bien *formado*, no si tiene
//! *sentido*. Un `Int32 + String` es sintaxis válida y error de tipos: eso lo
//! resuelve `zirk-sema`.
//!
//! El parser del lenguaje no se reutiliza para `init.zrk`: `ZIRK_LANGUAGE_SPEC.md`
//! sección 10 lo define como una DSL declarativa con su propio parser, que llega
//! en Fase 6.
//!
//! # Estado
//!
//! Vacío por diseño. La Fase 0 monta el esqueleto del workspace sin implementar
//! sintaxis de Zirk; la gramática llega en Fase 1.
