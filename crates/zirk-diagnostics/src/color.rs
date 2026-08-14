//! Colouring of diagnostics.
//!
//! `ZIRK_COMPILER_SPEC.md` section 9 requires the CLI to produce **deterministic
//! output**. Colour is therefore opt-in at the rendering call: the library never
//! decides on its own, and whoever renders states whether the destination is a
//! terminal.
//!
//! That is what keeps a piped diagnostic byte-identical to an uncoloured one,
//! which matters because the same text is what tooling and tests read.
//!
//! # Why raw ANSI and no dependency
//!
//! The whole compiler has one external dependency, `inkwell`, and it is there
//! because writing LLVM bindings by hand is unreasonable. Colour is a handful of
//! escape sequences; adding a crate for it would trade a real cost —one more
//! thing to audit, version and keep working on three platforms— for very little.

/// Whether a rendering emits colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Color {
    /// No escape sequences. The output is exactly the text.
    #[default]
    Never,
    /// ANSI escape sequences.
    Ansi,
}

impl Color {
    /// Wraps a text in a style, or returns it untouched when colour is off.
    pub(crate) fn paint(self, style: Style, text: &str) -> String {
        match self {
            Color::Never => text.to_string(),
            // The reset is emitted even for an empty text: leaving a style open
            // would bleed into whatever the terminal prints next.
            Color::Ansi => format!("{}{text}{RESET}", style.prefix()),
        }
    }

    pub(crate) fn enabled(self) -> bool {
        self == Color::Ansi
    }
}

const RESET: &str = "\x1b[0m";

/// A role in a diagnostic, with its style.
///
/// Roles are named after what they mean, not after their colour: a future theme
/// changes the palette without touching the rendering code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Style {
    /// `error[E0403]` and the message next to it.
    Error,
    /// `warning[W0001]` and its message.
    Warning,
    /// The `file:line:column` line.
    Location,
    /// The `|` gutter and the line numbers.
    Gutter,
    /// The fragment of the source line the diagnostic points at.
    Marked,
    /// The `^^^` marker and the label beside it.
    Marker,
    /// The `= cause:` label.
    Cause,
    /// The `= help:` label.
    Help,
}

impl Style {
    /// The escape sequence that opens the style.
    ///
    /// The 256-colour palette is used rather than the 8 basic colours because
    /// the basic red is aggressive on most themes, and a diagnostic that shouts
    /// is harder to read, not easier.
    fn prefix(self) -> &'static str {
        match self {
            // Muted red (#d75f5f) in bold: it stands out without shouting.
            Style::Error => "\x1b[1;38;5;167m",
            // Warm amber, clearly distinct from the error red.
            Style::Warning => "\x1b[1;38;5;179m",
            // Grey: the location matters, but it is not what is read first.
            Style::Location => "\x1b[38;5;245m",
            Style::Gutter => "\x1b[38;5;245m",
            // No colour, just bold: the source keeps its own reading, and the
            // emphasis marks the exact spot.
            Style::Marked => "\x1b[1m",
            Style::Marker => "\x1b[38;5;167m",
            // Soft blue for the reason.
            Style::Cause => "\x1b[38;5;75m",
            // Soft violet for the fix. Distinguishing them matters: cause and
            // help answer different questions.
            Style::Help => "\x1b[38;5;141m",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn without_colour_the_text_is_untouched() {
        assert_eq!(Color::Never.paint(Style::Error, "error"), "error");
    }

    #[test]
    fn with_colour_the_text_is_wrapped_and_closed() {
        let painted = Color::Ansi.paint(Style::Error, "error");
        assert!(painted.starts_with("\x1b["));
        assert!(painted.ends_with(RESET));
        assert!(painted.contains("error"));
    }

    #[test]
    fn every_style_closes_what_it_opens() {
        // A style left open bleeds into whatever the terminal prints next.
        for style in [
            Style::Error,
            Style::Warning,
            Style::Location,
            Style::Gutter,
            Style::Marked,
            Style::Marker,
            Style::Cause,
            Style::Help,
        ] {
            let painted = Color::Ansi.paint(style, "x");
            assert_eq!(
                painted.matches(RESET).count(),
                1,
                "{style:?} must close exactly once"
            );
        }
    }

    #[test]
    fn the_roles_that_must_differ_do_differ() {
        // Cause and help answer different questions, and the error must not be
        // confused with either.
        let distinct = [Style::Error, Style::Cause, Style::Help, Style::Warning];
        for (i, a) in distinct.iter().enumerate() {
            for b in &distinct[i + 1..] {
                assert_ne!(a.prefix(), b.prefix(), "{a:?} and {b:?} share a colour");
            }
        }
    }
}
