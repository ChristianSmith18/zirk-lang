//! Platform-neutral regular-expression construction and content operations.
//!
//! Caches, handles, and resource policy belong to the embedding host.

pub use regex::Regex;

pub fn compile(pattern: &str) -> Result<Regex, regex::Error> {
    Regex::new(pattern)
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
}
