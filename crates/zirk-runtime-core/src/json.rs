//! Shared JSON input limits and duplicate-object-key validation.

use std::collections::HashSet;

pub const MAX_DOCUMENT_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_NESTING_DEPTH: usize = 128;

/// Rejects oversized documents, excessive nesting, and duplicate object keys.
/// Keys are compared after JSON escape decoding, so `"x"` and `"\\u0078"`
/// are considered the same key.
pub fn validate_document(document: &str) -> Result<(), ()> {
    if document.len() > MAX_DOCUMENT_BYTES {
        return Err(());
    }

    struct Scanner<'a> {
        bytes: &'a [u8],
        cursor: usize,
    }

    impl Scanner<'_> {
        fn whitespace(&mut self) {
            while self
                .bytes
                .get(self.cursor)
                .is_some_and(u8::is_ascii_whitespace)
            {
                self.cursor += 1;
            }
        }

        fn string(&mut self) -> Result<String, ()> {
            let start = self.cursor;
            if self.bytes.get(self.cursor) != Some(&b'"') {
                return Err(());
            }
            self.cursor += 1;
            let mut escaped = false;
            while let Some(&byte) = self.bytes.get(self.cursor) {
                self.cursor += 1;
                if escaped {
                    escaped = false;
                    continue;
                }
                match byte {
                    b'\\' => escaped = true,
                    b'"' => {
                        let text =
                            std::str::from_utf8(&self.bytes[start..self.cursor]).map_err(|_| ())?;
                        return serde_json::from_str(text).map_err(|_| ());
                    }
                    _ => {}
                }
            }
            Err(())
        }

        fn value(&mut self, depth: usize) -> Result<(), ()> {
            if depth > MAX_NESTING_DEPTH {
                return Err(());
            }
            self.whitespace();
            match self.bytes.get(self.cursor).copied() {
                Some(b'{') => {
                    self.cursor += 1;
                    self.whitespace();
                    let mut keys = HashSet::new();
                    if self.bytes.get(self.cursor) == Some(&b'}') {
                        self.cursor += 1;
                        return Ok(());
                    }
                    loop {
                        self.whitespace();
                        if !keys.insert(self.string()?) {
                            return Err(());
                        }
                        self.whitespace();
                        if self.bytes.get(self.cursor) != Some(&b':') {
                            return Err(());
                        }
                        self.cursor += 1;
                        self.value(depth + 1)?;
                        self.whitespace();
                        match self.bytes.get(self.cursor) {
                            Some(b',') => self.cursor += 1,
                            Some(b'}') => {
                                self.cursor += 1;
                                return Ok(());
                            }
                            _ => return Err(()),
                        }
                    }
                }
                Some(b'[') => {
                    self.cursor += 1;
                    self.whitespace();
                    if self.bytes.get(self.cursor) == Some(&b']') {
                        self.cursor += 1;
                        return Ok(());
                    }
                    loop {
                        self.value(depth + 1)?;
                        self.whitespace();
                        match self.bytes.get(self.cursor) {
                            Some(b',') => self.cursor += 1,
                            Some(b']') => {
                                self.cursor += 1;
                                return Ok(());
                            }
                            _ => return Err(()),
                        }
                    }
                }
                Some(b'"') => {
                    self.string()?;
                    Ok(())
                }
                Some(_) => {
                    while self.bytes.get(self.cursor).is_some_and(|byte| {
                        !byte.is_ascii_whitespace() && !matches!(byte, b',' | b']' | b'}')
                    }) {
                        self.cursor += 1;
                    }
                    Ok(())
                }
                None => Err(()),
            }
        }
    }

    let mut scanner = Scanner {
        bytes: document.as_bytes(),
        cursor: 0,
    };
    scanner.value(0)?;
    scanner.whitespace();
    (scanner.cursor == scanner.bytes.len())
        .then_some(())
        .ok_or(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_duplicate_keys_after_escape_decoding() {
        assert!(validate_document(r#"{"x":1,"\u0078":2}"#).is_err());
        assert!(validate_document(r#"{"outer":{"x":1,"x":2}}"#).is_err());
        assert!(validate_document(r#"{"x":1,"y":2}"#).is_ok());
    }

    #[test]
    fn enforces_document_size_and_nesting_limits() {
        assert!(validate_document(&" ".repeat(MAX_DOCUMENT_BYTES + 1)).is_err());
        let nested = format!(
            "{}0{}",
            "[".repeat(MAX_NESTING_DEPTH + 1),
            "]".repeat(MAX_NESTING_DEPTH + 1)
        );
        assert!(validate_document(&nested).is_err());
    }
}
