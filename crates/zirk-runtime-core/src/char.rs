//! Platform-neutral `Char` classification and conversion.

/// The first Unicode scalar determines classification for a Zirk grapheme.
fn first(text: &str) -> Option<char> {
    text.chars().next()
}

pub fn is_uppercase(text: &str) -> bool {
    first(text).is_some_and(char::is_uppercase)
}
pub fn is_lowercase(text: &str) -> bool {
    first(text).is_some_and(char::is_lowercase)
}
pub fn is_digit(text: &str) -> bool {
    first(text).is_some_and(char::is_numeric)
}
pub fn is_whitespace(text: &str) -> bool {
    first(text).is_some_and(char::is_whitespace)
}
pub fn is_letter(text: &str) -> bool {
    first(text).is_some_and(char::is_alphabetic)
}
pub fn is_alphanumeric(text: &str) -> bool {
    first(text).is_some_and(char::is_alphanumeric)
}
pub fn uppercase(text: &str) -> String {
    text.chars().flat_map(char::to_uppercase).collect()
}
pub fn lowercase(text: &str) -> String {
    text.chars().flat_map(char::to_lowercase).collect()
}
pub fn ascii_code(text: &str) -> i32 {
    first(text)
        .filter(char::is_ascii)
        .map_or(-1, |value| value as i32)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn classifies_and_converts_unicode_scalars() {
        assert!(is_uppercase("Ñ"));
        assert!(is_letter("ñ"));
        assert!(is_digit("٣"));
        assert_eq!(uppercase("ß"), "SS");
        assert_eq!(lowercase("İ"), "i̇");
        assert_eq!(ascii_code("ñ"), -1);
    }
}
