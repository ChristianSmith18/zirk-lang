//! Renderizado de diagnósticos.
//!
//! El formato legible reproduce `ZIRK_COMPILER_SPEC.md` sección 8:
//!
//! ```text
//! error[E1234]: precise description
//!   src/users.zrk:18:12
//!    |
//! 18 |     problematic expression
//!    |            ^ localized explanation
//!    |
//!    = cause: semantic reason
//!    = help: concrete action
//!
//! Las etiquetas van en inglés: el spec las ilustra en español porque el
//! documento está en español, pero la salida del compilador se dirige a quien
//! usa Zirk. Ver `docs/decisions/ADR-006-idioma-de-diagnosticos.md`.
//! ```

use crate::Diagnostic;
use std::fmt::Write as _;

/// Forma de salida solicitada.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderStyle {
    /// Formato legible por humanos.
    Human,
    /// Formato estructurado para herramientas (`--json`).
    Json,
}

pub(crate) fn human(diagnostic: &Diagnostic) -> String {
    let mut out = String::new();

    // error[E1234]: descripción precisa
    let _ = writeln!(
        out,
        "{}[{}]: {}",
        diagnostic.severity.as_str(),
        diagnostic.code,
        diagnostic.message
    );

    // El ancho del canal izquierdo depende de cuántos dígitos tiene la línea.
    let gutter = diagnostic
        .location
        .as_ref()
        .map(|l| l.line.to_string().len())
        .unwrap_or(0);
    let pad = " ".repeat(gutter);

    if let Some(location) = &diagnostic.location {
        let _ = writeln!(
            out,
            "{pad} {}:{}:{}",
            location.file, location.line, location.column
        );

        // El fragmento es opcional: sin source, el diagnóstico conserva
        // encabezado, ubicación, causa y ayuda.
        if let Some(snippet) = &diagnostic.snippet {
            let _ = writeln!(out, "{pad} |");
            let _ = writeln!(out, "{} | {}", location.line, snippet.line_text);

            let offset = " ".repeat(location.column.saturating_sub(1) as usize);
            let marker = "^".repeat(snippet.width as usize);
            match &snippet.label {
                Some(label) => {
                    let _ = writeln!(out, "{pad} | {offset}{marker} {label}");
                }
                None => {
                    let _ = writeln!(out, "{pad} | {offset}{marker}");
                }
            }
            let _ = writeln!(out, "{pad} |");
        }
    }

    if let Some(cause) = &diagnostic.cause {
        let _ = writeln!(out, "{pad} = cause: {cause}");
    }
    if let Some(help) = &diagnostic.help {
        let _ = writeln!(out, "{pad} = help: {help}");
    }

    out
}

pub(crate) fn json(diagnostic: &Diagnostic) -> String {
    let mut out = String::from("{");

    let _ = write!(out, "\"severity\":\"{}\"", diagnostic.severity.as_str());
    let _ = write!(out, ",\"code\":\"{}\"", diagnostic.code);
    let _ = write!(out, ",\"message\":{}", quote(&diagnostic.message));

    match &diagnostic.location {
        Some(location) => {
            let _ = write!(
                out,
                ",\"location\":{{\"file\":{},\"line\":{},\"column\":{}}}",
                quote(&location.file),
                location.line,
                location.column
            );
        }
        None => out.push_str(",\"location\":null"),
    }

    match &diagnostic.cause {
        Some(cause) => {
            let _ = write!(out, ",\"cause\":{}", quote(cause));
        }
        None => out.push_str(",\"cause\":null"),
    }

    match &diagnostic.help {
        Some(help) => {
            let _ = write!(out, ",\"help\":{}", quote(help));
        }
        None => out.push_str(",\"help\":null"),
    }

    out.push('}');
    out
}

pub(crate) fn sink(diagnostics: &[Diagnostic], style: RenderStyle) -> String {
    // Se ordenan por ubicación antes de renderizar. Las etapas del pipeline
    // emiten en el orden en que trabajan —primero todo el léxico, después toda
    // la gramática—, así que sin esto un error de la línea 3 puede aparecer
    // antes que uno de la línea 2. Quien lee espera recorrer su archivo de
    // arriba hacia abajo.
    let mut ordenados: Vec<&Diagnostic> = diagnostics.iter().collect();
    ordenados.sort_by_key(|d| clave(d));

    match style {
        RenderStyle::Human => ordenados
            .into_iter()
            .map(human)
            .collect::<Vec<_>>()
            .join("\n"),
        RenderStyle::Json => {
            let items = ordenados
                .into_iter()
                .map(json)
                .collect::<Vec<_>>()
                .join(",");
            format!("[{items}]")
        }
    }
}

/// Clave de ordenamiento de un diagnóstico: archivo, línea y columna.
///
/// Los diagnósticos sin ubicación van al final: no pertenecen a ningún punto
/// del archivo y anteponerlos desplazaría a los que sí.
fn clave(d: &Diagnostic) -> (bool, String, u32, u32) {
    match &d.location {
        Some(l) => (false, l.file.clone(), l.line, l.column),
        None => (true, String::new(), 0, 0),
    }
}

/// Serializa una cadena como literal JSON, escapando lo que exige RFC 8259.
fn quote(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use crate::{Code, Diagnostic, DiagnosticSink, Location, RenderStyle, Snippet};

    fn diagnostico_completo() -> Diagnostic {
        Diagnostic::error(Code::new("E1234"), "precise description")
            .at(Location::new("src/users.zrk", 18, 12))
            .with_snippet(
                Snippet::new("    problematic expression", 1).with_label("localized explanation"),
            )
            .with_cause("semantic reason")
            .with_help("concrete action")
    }

    #[test]
    fn formato_humano_reproduce_el_spec() {
        let esperado = "\
error[E1234]: precise description
   src/users.zrk:18:12
   |
18 |     problematic expression
   |            ^ localized explanation
   |
   = cause: semantic reason
   = help: concrete action
";
        assert_eq!(diagnostico_completo().render(), esperado);
    }

    #[test]
    fn sin_fragmento_conserva_el_resto_de_la_informacion() {
        let rendered = Diagnostic::error(Code::new("E0002"), "no source available")
            .at(Location::new("src/main.zrk", 3, 5))
            .with_cause("the file could not be read")
            .with_help("check the file permissions")
            .render();

        assert!(rendered.contains("error[E0002]: no source available"));
        assert!(rendered.contains("src/main.zrk:3:5"));
        assert!(rendered.contains("= cause: the file could not be read"));
        assert!(rendered.contains("= help: check the file permissions"));
        // Sin fragmento no debe aparecer el canal de source ni el marcador.
        assert!(!rendered.contains('^'));
    }

    #[test]
    fn sin_ayuda_no_se_inventa_una_generica() {
        let rendered = Diagnostic::error(Code::new("E0003"), "error with no clear fix")
            .with_cause("state is not representable")
            .render();

        assert!(!rendered.contains("help"));
    }

    #[test]
    fn el_marcador_se_alinea_con_la_columna() {
        let rendered = Diagnostic::error(Code::new("E0004"), "unexpected token")
            .at(Location::new("a.zrk", 1, 5))
            .with_snippet(Snippet::new("abcdefgh", 3))
            .render();

        let linea_marcador = rendered
            .lines()
            .find(|l| l.contains('^'))
            .expect("debe haber línea de marcador");

        // Canal "1 | " (4 caracteres) + 4 espacios de la columna 5.
        assert_eq!(linea_marcador, "  |     ^^^");
    }

    #[test]
    fn json_escapa_comillas_y_saltos_de_linea() {
        let rendered =
            Diagnostic::error(Code::new("E0005"), "said \"hello\"\nand left").render_json();

        assert!(rendered.contains(r#""message":"said \"hello\"\nand left""#));
    }

    #[test]
    fn json_conserva_los_campos_obligatorios() {
        let rendered = diagnostico_completo().render_json();

        assert!(rendered.contains(r#""severity":"error""#));
        assert!(rendered.contains(r#""code":"E1234""#));
        assert!(rendered.contains(r#""line":18"#));
        assert!(rendered.contains(r#""column":12"#));
        assert!(rendered.contains(r#""cause":"semantic reason""#));
        assert!(rendered.contains(r#""help":"concrete action""#));
    }

    #[test]
    fn json_representa_los_campos_ausentes_como_null() {
        let rendered = Diagnostic::warning(Code::new("W0001"), "algo").render_json();

        assert!(rendered.contains(r#""location":null"#));
        assert!(rendered.contains(r#""cause":null"#));
        assert!(rendered.contains(r#""help":null"#));
    }

    #[test]
    fn el_sink_json_produce_un_arreglo() {
        let mut sink = DiagnosticSink::new();
        sink.emit(Diagnostic::error(Code::new("E0001"), "uno"));
        sink.emit(Diagnostic::warning(Code::new("W0001"), "dos"));

        let rendered = sink.render(RenderStyle::Json);
        assert!(rendered.starts_with('['));
        assert!(rendered.ends_with(']'));
        assert_eq!(rendered.matches(r#""severity""#).count(), 2);
    }
}
