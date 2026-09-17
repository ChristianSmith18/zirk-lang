//! Platform-neutral Unicode and string-content operations.
//!
//! These helpers operate solely on owned Rust strings.  Handle identity,
//! allocation, and mutation publication remain responsibilities of a runtime
//! host such as `zirk-runtime` or the IR interpreter.

use unicode_normalization::UnicodeNormalization;
use unicode_segmentation::UnicodeSegmentation;

/// Normalization forms accepted by Zirk's text APIs.
pub fn normalize(text: &str, form: &str) -> String {
    match form {
        "NFC" => text.nfc().collect(),
        "NFD" => text.nfd().collect(),
        "NFKC" => text.nfkc().collect(),
        "NFKD" => text.nfkd().collect(),
        _ => text.to_owned(),
    }
}

/// Compares text after canonical NFC normalization.
pub fn equal(left: &str, right: &str) -> bool {
    left == right || normalize(left, "NFC") == normalize(right, "NFC")
}

/// Byte offset of a grapheme index, including negative indexing.
pub fn grapheme_offset(text: &str, index: i64) -> Option<usize> {
    let graphemes: Vec<_> = text.grapheme_indices(true).collect();
    let index = if index < 0 {
        graphemes.len() as i64 + index
    } else {
        index
    };
    graphemes
        .get(usize::try_from(index).ok()?)
        .map(|(offset, _)| *offset)
}

/// Byte length of the grapheme starting at a valid UTF-8 boundary.
pub fn grapheme_len_at(text: &str, offset: usize) -> Option<usize> {
    text.get(offset..)?.graphemes(true).next().map(str::len)
}

/// Pure content helpers used by both runtime hosts.
pub fn concat(left: &str, right: &str) -> String {
    format!("{left}{right}")
}

/// Repeats content without exposing allocator-specific failures.
pub fn repeat(text: &str, count: usize) -> Option<String> {
    text.len().checked_mul(count)?;
    Some(text.repeat(count))
}

pub fn contains(text: &str, pattern: &str) -> bool {
    text.contains(pattern)
}
pub fn starts_with(text: &str, pattern: &str) -> bool {
    text.starts_with(pattern)
}
pub fn ends_with(text: &str, pattern: &str) -> bool {
    text.ends_with(pattern)
}
pub fn replace(text: &str, needle: &str, replacement: &str) -> String {
    text.replace(needle, replacement)
}
pub fn grapheme_count(text: &str) -> usize {
    text.graphemes(true).count()
}
pub fn grapheme_find(text: &str, needle: &str) -> Option<usize> {
    text.find(needle)
        .map(|offset| grapheme_count(&text[..offset]))
}
pub fn substring(text: &str, start: i64, end: i64) -> &str {
    let length = text.len() as i64;
    let start = start.clamp(0, length) as usize;
    let end = end.clamp(0, length) as usize;
    if start > end {
        return "";
    }
    text.get(start..end).unwrap_or("")
}
pub fn replace_range(text: &str, offset: i64, length: i64, replacement: &str) -> Option<String> {
    let start = usize::try_from(offset).ok()?;
    let end = start.checked_add(usize::try_from(length).ok()?)?;
    if end > text.len() || !text.is_char_boundary(start) || !text.is_char_boundary(end) {
        return None;
    }
    Some(format!("{}{}{}", &text[..start], replacement, &text[end..]))
}

/// Returns a grapheme-safe slice by grapheme indexes using Zirk slice rules.
pub fn grapheme_slice(text: &str, start: i64, end: i64, step: i64) -> String {
    const MISSING: i64 = i64::MIN;
    let graphemes: Vec<_> = text.graphemes(true).collect();
    let count = graphemes.len() as i64;
    let step = if step == MISSING { 1 } else { step };
    if step == 0 || count == 0 {
        return String::new();
    }
    let forward = |bound: i64| {
        if bound < 0 {
            (count + bound).max(0)
        } else {
            bound.min(count)
        }
    };
    let mut out = String::with_capacity(text.len());
    if step > 0 {
        let mut index = if start == MISSING { 0 } else { forward(start) };
        let end = if end == MISSING { count } else { forward(end) };
        while index < end {
            out.push_str(graphemes[index as usize]);
            index += step;
        }
    } else {
        let mut index = if start == MISSING {
            count - 1
        } else {
            (if start < 0 { count + start } else { start }).clamp(-1, count - 1)
        };
        let end = if end == MISSING {
            -1
        } else {
            (if end < 0 { count + end } else { end }).clamp(-1, count - 1)
        };
        while index > end {
            out.push_str(graphemes[index as usize]);
            index += step;
        }
    }
    out
}

/// The grapheme clusters of `text`, in order.
pub fn graphemes(text: &str) -> Vec<&str> {
    text.graphemes(true).collect()
}

/// `text.split(separator)` — an empty separator splits into grapheme
/// clusters, matching `zirk_str_split`.
pub fn split<'a>(text: &'a str, separator: &str) -> Vec<&'a str> {
    if separator.is_empty() {
        text.graphemes(true).collect()
    } else {
        text.split(separator).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_unicode_content_semantics() {
        assert!(equal("ho\u{301}", "hó"));
        assert_eq!(grapheme_offset("a👩‍💻b", -1), Some("a👩‍💻".len()));
        assert_eq!(grapheme_len_at("a👩‍💻b", 1), Some("👩‍💻".len()));
        assert_eq!(grapheme_slice("a👩‍💻b", i64::MIN, i64::MIN, -1), "b👩‍💻a");
        assert_eq!(concat("ñ", "andú"), "ñandú");
        assert_eq!(repeat("é", 3).as_deref(), Some("ééé"));
        assert_eq!(grapheme_find("a👩‍💻b", "b"), Some(2));
        assert_eq!(substring("ñandú", 0, 2), "ñ");
        assert_eq!(
            replace_range("a👩‍💻b", 1, "👩‍💻".len() as i64, "x").as_deref(),
            Some("axb")
        );
    }
}
