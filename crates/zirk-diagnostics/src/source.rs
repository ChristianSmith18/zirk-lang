//! Manejo de source: spans y conversión a ubicaciones legibles.
//!
//! Vive en este crate porque es lo que convierte una posición del compilador en
//! la [`Location`](crate::Location) que necesita un diagnóstico, y porque todas
//! las etapas del pipeline lo necesitan — la misma razón por la que
//! `zirk-diagnostics` es la dependencia transversal permitida.

use crate::{Location, Snippet};

/// Rango de bytes dentro de un archivo fuente.
///
/// Se guardan offsets de byte y no línea/columna porque es lo barato de
/// producir y propagar; la conversión a coordenadas legibles ocurre solo cuando
/// hay que emitir un diagnóstico.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Span {
    pub start: u32,
    pub end: u32,
}

impl Span {
    pub const fn new(start: u32, end: u32) -> Self {
        Self { start, end }
    }

    /// Span vacío en una posición, útil para señalar "acá falta algo".
    pub const fn empty(at: u32) -> Self {
        Self { start: at, end: at }
    }

    /// Span que cubre desde el inicio de este hasta el final del otro.
    pub fn to(self, other: Span) -> Span {
        Span::new(self.start.min(other.start), self.end.max(other.end))
    }

    pub const fn len(self) -> u32 {
        self.end.saturating_sub(self.start)
    }

    pub const fn is_empty(self) -> bool {
        self.start >= self.end
    }
}

/// Un archivo fuente con su índice de líneas.
///
/// El índice se calcula una vez al construirlo: convertir offsets a línea y
/// columna es una operación frecuente en el camino de errores, y recorrer el
/// texto entero en cada diagnóstico no escala.
#[derive(Debug, Clone)]
pub struct SourceFile {
    name: String,
    text: String,
    /// Offset de byte donde comienza cada línea. Siempre arranca con 0.
    line_starts: Vec<u32>,
}

impl SourceFile {
    pub fn new(name: impl Into<String>, text: impl Into<String>) -> Self {
        let text = text.into();
        let mut line_starts = vec![0u32];
        for (offset, byte) in text.bytes().enumerate() {
            if byte == b'\n' {
                line_starts.push(offset as u32 + 1);
            }
        }
        Self {
            name: name.into(),
            text,
            line_starts,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    /// Texto cubierto por un span.
    pub fn slice(&self, span: Span) -> &str {
        let start = (span.start as usize).min(self.text.len());
        let end = (span.end as usize).min(self.text.len());
        // Un span mal formado no debe abortar el compilador en pleno error.
        if start > end {
            return "";
        }
        &self.text[start..end]
    }

    /// Índice 0-based de la línea que contiene un offset.
    fn line_index(&self, offset: u32) -> usize {
        match self.line_starts.binary_search(&offset) {
            Ok(exacto) => exacto,
            Err(siguiente) => siguiente - 1,
        }
    }

    /// Convierte un offset en línea y columna 1-based.
    ///
    /// La columna cuenta caracteres Unicode, no bytes, como espera un editor.
    pub fn line_column(&self, offset: u32) -> (u32, u32) {
        let linea = self.line_index(offset);
        let inicio = self.line_starts[linea] as usize;
        let hasta = (offset as usize).min(self.text.len());
        let columna = self.text[inicio..hasta].chars().count() as u32 + 1;
        (linea as u32 + 1, columna)
    }

    /// Texto de una línea 1-based, sin el salto final.
    pub fn line_text(&self, line: u32) -> &str {
        let indice = (line.saturating_sub(1)) as usize;
        let Some(&inicio) = self.line_starts.get(indice) else {
            return "";
        };
        let fin = self
            .line_starts
            .get(indice + 1)
            .map(|&s| s as usize)
            .unwrap_or(self.text.len());
        self.text[inicio as usize..fin].trim_end_matches(['\n', '\r'])
    }

    /// Ubicación legible del inicio de un span.
    pub fn location(&self, span: Span) -> Location {
        let (linea, columna) = self.line_column(span.start);
        Location::new(self.name.clone(), linea, columna)
    }

    /// Fragmento de source para un span, acotado a su primera línea.
    ///
    /// Un span multilínea se recorta al final de la línea inicial: el formato de
    /// `COMPILER_SPEC` sección 8 muestra una línea con un marcador debajo, y un
    /// marcador que abarque varias líneas no tendría dónde dibujarse.
    pub fn snippet(&self, span: Span) -> Snippet {
        let (linea, columna) = self.line_column(span.start);
        let texto = self.line_text(linea);

        let caracteres_hasta_fin = texto.chars().count() as u32 + 1 - columna;
        let ancho = self
            .slice(span)
            .chars()
            .count()
            .max(1)
            .min(caracteres_hasta_fin.max(1) as usize) as u32;

        Snippet::new(texto, ancho)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn archivo() -> SourceFile {
        SourceFile::new(
            "test.zrk",
            "fn main(): Void {\n    stdout.println(\"hola\");\n}\n",
        )
    }

    #[test]
    fn la_primera_posicion_es_linea_uno_columna_uno() {
        assert_eq!(archivo().line_column(0), (1, 1));
    }

    #[test]
    fn el_offset_se_convierte_en_linea_y_columna() {
        let f = archivo();
        // El primer byte de la segunda línea está tras "fn main(): Void {\n".
        assert_eq!(f.line_column(18), (2, 1));
    }

    #[test]
    fn la_columna_cuenta_caracteres_no_bytes() {
        let f = SourceFile::new("t.zrk", "mut café = 1;");
        // 'é' ocupa dos bytes; el '=' está en el byte 10 pero en la columna 10.
        let offset = f.text().find('=').unwrap() as u32;
        assert_eq!(offset, 10);
        assert_eq!(f.line_column(offset), (1, 10));
    }

    #[test]
    fn se_recupera_el_texto_de_una_linea() {
        assert_eq!(archivo().line_text(2), "    stdout.println(\"hola\");");
    }

    #[test]
    fn una_linea_inexistente_no_aborta() {
        assert_eq!(archivo().line_text(99), "");
    }

    #[test]
    fn el_span_recorta_el_texto_correspondiente() {
        let f = SourceFile::new("t.zrk", "fn main");
        assert_eq!(f.slice(Span::new(3, 7)), "main");
    }

    #[test]
    fn un_span_fuera_de_rango_no_aborta() {
        let f = SourceFile::new("t.zrk", "abc");
        assert_eq!(f.slice(Span::new(1, 999)), "bc");
    }

    #[test]
    fn el_marcador_no_se_pasa_del_fin_de_linea() {
        let f = SourceFile::new("t.zrk", "ab\ncd");
        // Span que arranca en la primera línea y se extiende más allá de ella.
        let s = f.snippet(Span::new(1, 5));
        assert_eq!(s.line_text, "ab");
        assert!(s.width <= 2);
    }

    #[test]
    fn dos_spans_se_combinan_cubriendo_ambos() {
        assert_eq!(Span::new(2, 5).to(Span::new(8, 10)), Span::new(2, 10));
    }
}
