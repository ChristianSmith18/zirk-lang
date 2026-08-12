//! # zirk-diagnostics
//!
//! **Responsabilidad:** definir la forma de un diagnóstico del compilador y
//! renderizarlo, tanto para humanos como para herramientas.
//!
//! Implementa el contrato de `ZIRK_COMPILER_SPEC.md` sección 8: todo diagnóstico
//! lleva severidad, código estable, ubicación, causa y —cuando existe una
//! reparación clara— una ayuda accionable.
//!
//! **Límite:** este crate no conoce la sintaxis de Zirk, no lee archivos y no
//! decide cuándo emitir un diagnóstico. Solo lo representa y lo formatea. Quien
//! detecta el error es responsable de construirlo y de proveer el fragmento de
//! source si lo tiene.
//!
//! Es la única dependencia transversal permitida del workspace: cualquier etapa
//! del pipeline puede depender de él, independientemente de su posición.

mod render;

pub use render::RenderStyle;

/// Severidad de un diagnóstico.
///
/// Los warnings nunca alteran la semántica del programa; solo informan. La
/// elevación a error es una decisión de configuración, no del sitio que emite.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    Error,
    Warning,
}

impl Severity {
    /// Etiqueta usada en la salida legible y en la estructurada.
    pub const fn as_str(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
        }
    }
}

/// Código estable de un diagnóstico.
///
/// Un código publicado no se reutiliza para un error semánticamente distinto:
/// herramientas, documentación y supresiones dependen de esa estabilidad.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Code(&'static str);

impl Code {
    pub const fn new(code: &'static str) -> Self {
        Self(code)
    }

    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

impl std::fmt::Display for Code {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

/// Ubicación de un diagnóstico en el source.
///
/// `line` y `column` son 1-based, como espera cualquier editor. `column` cuenta
/// caracteres Unicode, no bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location {
    pub file: String,
    pub line: u32,
    pub column: u32,
}

impl Location {
    pub fn new(file: impl Into<String>, line: u32, column: u32) -> Self {
        Self {
            file: file.into(),
            line,
            column,
        }
    }
}

/// Fragmento de source que acompaña al diagnóstico.
///
/// Es opcional a propósito: un diagnóstico emitido sin acceso al source sigue
/// siendo válido y debe renderizarse conservando el resto de la información.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snippet {
    /// Texto completo de la línea señalada, sin el salto de línea final.
    pub line_text: String,
    /// Cuántos caracteres abarca el marcador. Siempre al menos 1.
    pub width: u32,
    /// Explicación localizada que se imprime junto al marcador.
    pub label: Option<String>,
}

impl Snippet {
    pub fn new(line_text: impl Into<String>, width: u32) -> Self {
        Self {
            line_text: line_text.into(),
            width: width.max(1),
            label: None,
        }
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
}

/// Un diagnóstico del compilador.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: Code,
    /// Descripción precisa, en el encabezado.
    pub message: String,
    pub location: Option<Location>,
    pub snippet: Option<Snippet>,
    /// Motivo semántico del error.
    pub cause: Option<String>,
    /// Acción concreta de reparación. Se omite si no hay una reparación clara:
    /// una ayuda genérica sin valor accionable es peor que ninguna.
    pub help: Option<String>,
}

impl Diagnostic {
    pub fn error(code: Code, message: impl Into<String>) -> Self {
        Self::new(Severity::Error, code, message)
    }

    pub fn warning(code: Code, message: impl Into<String>) -> Self {
        Self::new(Severity::Warning, code, message)
    }

    pub fn new(severity: Severity, code: Code, message: impl Into<String>) -> Self {
        Self {
            severity,
            code,
            message: message.into(),
            location: None,
            snippet: None,
            cause: None,
            help: None,
        }
    }

    pub fn at(mut self, location: Location) -> Self {
        self.location = Some(location);
        self
    }

    pub fn with_snippet(mut self, snippet: Snippet) -> Self {
        self.snippet = Some(snippet);
        self
    }

    pub fn with_cause(mut self, cause: impl Into<String>) -> Self {
        self.cause = Some(cause.into());
        self
    }

    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }

    /// Renderiza en el formato legible de `COMPILER_SPEC` sección 8.
    pub fn render(&self) -> String {
        render::human(self)
    }

    /// Renderiza como objeto JSON para consumo por herramientas.
    pub fn render_json(&self) -> String {
        render::json(self)
    }

    /// Mueve el diagnóstico al heap para devolverlo en un [`DiagnosticResult`].
    pub fn boxed(self) -> Box<Self> {
        Box::new(self)
    }
}

/// Resultado de una operación que puede fallar con un diagnóstico.
///
/// El error va en el heap a propósito: `Diagnostic` ronda los 200 bytes y este
/// alias aparece en el retorno de casi toda operación del compilador. Sin el
/// `Box`, el camino de éxito paga ese tamaño en cada llamada.
pub type DiagnosticResult<T> = Result<T, Box<Diagnostic>>;

/// Acumula diagnósticos y aplica la política de severidad.
///
/// Es el único punto donde un warning puede convertirse en error: los sitios que
/// emiten declaran la severidad natural del hallazgo y no conocen la política.
#[derive(Debug, Clone, Default)]
pub struct DiagnosticSink {
    diagnostics: Vec<Diagnostic>,
    warnings_as_errors: bool,
}

impl DiagnosticSink {
    pub fn new() -> Self {
        Self::default()
    }

    /// Eleva los warnings a errores, según `--warnings-as-errors`.
    pub fn warnings_as_errors(mut self, enabled: bool) -> Self {
        self.warnings_as_errors = enabled;
        self
    }

    pub fn emit(&mut self, mut diagnostic: Diagnostic) {
        if self.warnings_as_errors && diagnostic.severity == Severity::Warning {
            diagnostic.severity = Severity::Error;
        }
        self.diagnostics.push(diagnostic);
    }

    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Si hay al menos un error, la compilación no puede continuar.
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|d| d.severity == Severity::Error)
    }

    pub fn is_empty(&self) -> bool {
        self.diagnostics.is_empty()
    }

    pub fn len(&self) -> usize {
        self.diagnostics.len()
    }

    /// Renderiza todos los diagnósticos acumulados.
    pub fn render(&self, style: RenderStyle) -> String {
        render::sink(&self.diagnostics, style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn warnings_as_errors_eleva_la_severidad() {
        let mut sink = DiagnosticSink::new().warnings_as_errors(true);
        sink.emit(Diagnostic::warning(Code::new("W0001"), "símbolo sin uso"));

        assert_eq!(sink.diagnostics()[0].severity, Severity::Error);
        assert!(sink.has_errors());
    }

    #[test]
    fn sin_la_bandera_un_warning_no_frena_la_compilacion() {
        let mut sink = DiagnosticSink::new();
        sink.emit(Diagnostic::warning(Code::new("W0001"), "símbolo sin uso"));

        assert_eq!(sink.diagnostics()[0].severity, Severity::Warning);
        assert!(!sink.has_errors());
    }

    #[test]
    fn los_errores_siempre_frenan_la_compilacion() {
        let mut sink = DiagnosticSink::new();
        sink.emit(Diagnostic::error(Code::new("E0001"), "tipo incompatible"));

        assert!(sink.has_errors());
    }

    #[test]
    fn un_snippet_tiene_marcador_de_al_menos_un_caracter() {
        assert_eq!(Snippet::new("texto", 0).width, 1);
    }
}
