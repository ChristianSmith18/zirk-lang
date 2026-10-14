//! Platform-neutral regular-expression construction and content operations.
//!
//! Caches, handles, and resource policy belong to the embedding host.

pub use regex::Regex;

/// Maximum compiled-program size accepted for a single pattern.
pub const MAX_COMPILED_REGEX_BYTES: usize = 256 * 1024;

pub fn compile(pattern: &str) -> Result<Regex, regex::Error> {
    let mut builder = regex::RegexBuilder::new(pattern);
    builder.size_limit(MAX_COMPILED_REGEX_BYTES);
    builder.build()
}
pub fn is_valid(pattern: &str) -> bool {
    !pattern.is_empty() && compile(pattern).is_ok()
}
pub fn is_match(regex: &Regex, text: &str) -> bool {
    regex.is_match(text)
}
pub fn replace(regex: &Regex, text: &str, replacement: &str) -> String {
    regex.replace_all(text, replacement).into_owned()
}
pub fn split(regex: &Regex, text: &str) -> Vec<String> {
    regex.split(text).map(str::to_owned).collect()
}
pub fn find(regex: &Regex, text: &str) -> Option<(usize, usize)> {
    regex
        .find(text)
        .map(|matched| (matched.start(), matched.end()))
}
pub fn find_all(regex: &Regex, text: &str) -> Vec<(usize, usize)> {
    regex
        .find_iter(text)
        .map(|matched| (matched.start(), matched.end()))
        .collect()
}

/// The text of positional capture group `n` of the match whose whole-match
/// span starts at `match_start` within `haystack`, or an empty string when
/// the group is missing. Re-running captures is how `Regex.Match.group(n)`
/// resolves groups without storing them.
pub fn match_group_pos(regex: &Regex, haystack: &str, match_start: usize, n: usize) -> String {
    for caps in regex.captures_iter(haystack) {
        let Some(whole) = caps.get(0) else { continue };
        if whole.start() == match_start {
            return caps
                .get(n)
                .map(|group| group.as_str().to_owned())
                .unwrap_or_default();
        }
    }
    String::new()
}

/// The text of the named capture group of the match whose whole-match span
/// starts at `match_start`, or an empty string when missing.
pub fn match_group_name(regex: &Regex, haystack: &str, match_start: usize, name: &str) -> String {
    for caps in regex.captures_iter(haystack) {
        let Some(whole) = caps.get(0) else { continue };
        if whole.start() == match_start {
            return caps
                .name(name)
                .map(|group| group.as_str().to_owned())
                .unwrap_or_default();
        }
    }
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructs_matches_finds_and_replaces() {
        let regex = compile("a+").unwrap();
        assert!(is_match(&regex, "baac"));
        assert_eq!(find(&regex, "baac"), Some((1, 3)));
        assert_eq!(replace(&regex, "baacaa", "x"), "bxcx");
        assert_eq!(split(&regex, "baac"), ["b", "c"]);
        assert!(!is_valid("["));
    }

    #[test]
    fn rejects_patterns_whose_compiled_program_exceeds_the_limit() {
        // The alternation has a compact source but expands into a large
        // compiled automaton. The builder limit must reject it before runtime
        // handles can retain the compiled program.
        let pattern = format!(
            "(?:{}){{100}}",
            (0..40)
                .map(|i| format!("x{i}"))
                .collect::<Vec<_>>()
                .join("|")
        );
        assert!(compile(&pattern).is_err());
        assert!(compile("(?:ab|cd)+").is_ok());
    }
}
