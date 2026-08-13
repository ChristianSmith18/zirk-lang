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
//! en la Fase 6.
//!
//! # Construcciones de fases posteriores
//!
//! Ante `class`, `for`, `match` o cualquier construcción que exista en el
//! lenguaje pero no en este subset, el parser emite un diagnóstico que la nombra
//! y dice en qué fase llega — no "token inesperado". Es la decisión D6 del
//! design, y es lo que convierte al subset en algo comprensible en vez de en un
//! lenguaje distinto que casualmente se parece a Zirk.

mod parser;

pub use parser::parse;

/// Códigos de diagnóstico del parser.
pub mod codes {
    use zirk_diagnostics::Code;

    /// Se encontró un token donde se esperaba otra cosa.
    pub const TOKEN_INESPERADO: Code = Code::new("E0301");
    /// Construcción del lenguaje que todavía no está implementada.
    pub const NO_IMPLEMENTADO: Code = Code::new("E0302");
    /// Falta el tipo de retorno de una función.
    pub const FALTA_TIPO_RETORNO: Code = Code::new("E0303");
    /// Declaración sin tipo ni inicializador: no hay forma de saber su tipo.
    pub const DECLARACION_SIN_TIPO: Code = Code::new("E0304");
    /// El cuerpo de una construcción debe ir entre llaves.
    pub const FALTAN_LLAVES: Code = Code::new("E0305");
    /// Módulos multi-archivo, que llegan en una fase posterior.
    pub const MODULOS_NO_DISPONIBLES: Code = Code::new("E0306");
}
