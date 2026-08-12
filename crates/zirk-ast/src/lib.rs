//! # zirk-ast
//!
//! **Responsabilidad:** definir los nodos del árbol de sintaxis de Zirk y su
//! correspondencia con ubicaciones del source.
//!
//! **Límite:** este crate solo define la *forma* del árbol. No lo construye
//! (eso es `zirk-parser`), no lo interpreta ni lo valida (eso es `zirk-sema`).
//!
//! Según `ZIRK_COMPILER_SPEC.md` sección 3, la AST semántica interna es privada
//! y puede evolucionar con el compilador. Las herramientas externas no la
//! consumen directamente: usan la Syntax API pública, que llegará en una fase
//! posterior y es un contrato distinto de este crate.
//!
//! # Estado
//!
//! Vacío por diseño. La Fase 0 monta el esqueleto del workspace sin implementar
//! sintaxis de Zirk; los nodos llegan en Fase 1.
