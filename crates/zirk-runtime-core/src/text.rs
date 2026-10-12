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
    if index >= 0 {
        return text
            .grapheme_indices(true)
            .nth(usize::try_from(index).ok()?)
            .map(|(offset, _)| offset);
    }

    // Negative indexing needs the grapheme count, but still avoids allocating
    // a temporary Vec of every grapheme and byte offset.
    let count = text.graphemes(true).count() as i128;
    let resolved = count + index as i128;
    if resolved < 0 {
        return None;
    }
    text.grapheme_indices(true)
        .nth(usize::try_from(resolved).ok()?)
        .map(|(offset, _)| offset)
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

    #[test]
    fn grapheme_offsets_cover_unicode_negative_and_out_of_range_indexes() {
        let text = "e\u{301}👩‍💻🇨🇱x";
        let offsets: Vec<_> = text.grapheme_indices(true).map(|(i, _)| i).collect();
        assert_eq!(offsets.len(), 4);
        for (index, offset) in offsets.iter().copied().enumerate() {
            assert_eq!(grapheme_offset(text, index as i64), Some(offset));
            assert_eq!(
                grapheme_offset(text, index as i64 - offsets.len() as i64),
                Some(offset)
            );
        }
        assert_eq!(grapheme_offset(text, 4), None);
        assert_eq!(grapheme_offset(text, -5), None);
        assert_eq!(grapheme_offset(text, i64::MAX), None);
        assert_eq!(grapheme_offset(text, i64::MIN), None);
    }

    #[test]
    fn grapheme_offsets_reflect_mutated_and_sliced_text_without_stale_indexes() {
        let mut text = String::from("a👩‍💻b");
        assert_eq!(grapheme_offset(&text, -1), Some("a👩‍💻".len()));
        text = replace_range(&text, 1, "👩‍💻".len() as i64, "e\u{301}🇨🇱").unwrap();
        assert_eq!(grapheme_offset(&text, 1), Some(1));
        assert_eq!(grapheme_offset(&text, -1), Some(text.len() - 1));
        let slice = grapheme_slice(&text, 1, 3, 1);
        assert_eq!(grapheme_offset(&slice, 0), Some(0));
        assert_eq!(grapheme_offset(&slice, -1), Some("e\u{301}".len()));
    }

    #[test]
    #[ignore = "manual microbenchmark; run with --ignored --nocapture"]
    fn grapheme_offset_microbenchmark() {
        fn materialized_baseline(text: &str, index: i64) -> Option<usize> {
            let graphemes: Vec<_> = text.grapheme_indices(true).collect();
            let index = if index < 0 {
                (graphemes.len() as i128 + index as i128).try_into().ok()?
            } else {
                usize::try_from(index).ok()?
            };
            graphemes.get(index).map(|(offset, _)| *offset)
        }

        let datasets = [
            ("short-ascii", "Zirk language".repeat(8)),
            ("long-combining", "e\u{301}a\u{308}o\u{302}".repeat(512)),
            ("emoji-zwj-and-flags", "👩‍💻🇨🇱👨‍👩‍👧‍👦".repeat(256)),
        ];
        for (name, text) in datasets {
            const QUERIES: i64 = 10_000;
            const ROUNDS: usize = 3;
            let grapheme_count = text.graphemes(true).count();
            let mut baseline_times = Vec::with_capacity(ROUNDS);
            let mut iterator_times = Vec::with_capacity(ROUNDS);
            let mut baseline_checksum = 0usize;
            let mut iterator_checksum = 0usize;
            for _ in 0..ROUNDS {
                let start = std::time::Instant::now();
                baseline_checksum = 0;
                for i in 0..QUERIES {
                    let index = if i % 2 == 0 {
                        i % grapheme_count as i64
                    } else {
                        -(i % grapheme_count as i64) - 1
                    };
                    baseline_checksum = baseline_checksum
                        .wrapping_add(materialized_baseline(&text, index).unwrap_or_default());
                }
                baseline_times.push(start.elapsed());

                let start = std::time::Instant::now();
                iterator_checksum = 0;
                for i in 0..QUERIES {
                    let index = if i % 2 == 0 {
                        i % grapheme_count as i64
                    } else {
                        -(i % grapheme_count as i64) - 1
                    };
                    iterator_checksum = iterator_checksum
                        .wrapping_add(grapheme_offset(&text, index).unwrap_or_default());
                }
                iterator_times.push(start.elapsed());
            }
            assert_eq!(
                iterator_checksum, baseline_checksum,
                "{name} checksum differs"
            );
            let baseline_min = baseline_times
                .iter()
                .min()
                .copied()
                .expect("baseline timings");
            let baseline_max = baseline_times
                .iter()
                .max()
                .copied()
                .expect("baseline timings");
            let iterator_min = iterator_times
                .iter()
                .min()
                .copied()
                .expect("iterator timings");
            let iterator_max = iterator_times
                .iter()
                .max()
                .copied()
                .expect("iterator timings");
            baseline_times.sort_unstable();
            iterator_times.sort_unstable();
            eprintln!(
                "{name}: bytes={}, graphemes={grapheme_count}, queries={QUERIES}, rounds={ROUNDS}, materialized_median={:?} range={baseline_min:?}..={baseline_max:?}, iterator_median={:?} range={iterator_min:?}..={iterator_max:?}, checksum={iterator_checksum}",
                text.len(),
                baseline_times[ROUNDS / 2],
                iterator_times[ROUNDS / 2]
            );
        }
    }
}
