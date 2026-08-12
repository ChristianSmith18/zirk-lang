//! # zirk-lexer
//!
//! **Responsabilidad:** convertir texto fuente `.zrk` en una secuencia de tokens
//! con su ubicación, y reportar los errores léxicos que encuentre.
//!
//! **Límite:** el lexer no conoce la gramática. No decide si una secuencia de
//! tokens es válida, solo si cada token lo es. El punto y coma opcional de
//! `ZIRK_LANGUAGE_SPEC.md` sección 1 es un problema del parser, no de acá.
//!
//! `ZIRK_COMPILER_SPEC.md` sección 2 lo describe como incremental: debe poder
//! reanalizar solo la región afectada por una edición. Esa capacidad llega
//! cuando exista compilación incremental, pero la API no debe cerrarse a ella.
//!
//! # Estado
//!
//! Vacío por diseño. La Fase 0 monta el esqueleto del workspace sin implementar
//! sintaxis de Zirk; la tokenización llega en Fase 1.
