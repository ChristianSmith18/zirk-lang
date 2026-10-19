//! Strict parsing and selection for HTTP `Accept-Encoding` fields.
//!
//! This module deliberately handles negotiation only. Compression codecs are
//! applied by the native transport, where the input/output and CPU budgets can
//! be enforced while bytes are streamed.

use crate::http_values::Headers;

pub const MAX_ACCEPT_ENCODING_BYTES: usize = 4096;
pub const MAX_ACCEPT_ENCODINGS: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContentCoding {
    Brotli,
    Gzip,
    Deflate,
    Identity,
}

impl ContentCoding {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Brotli => "br",
            Self::Gzip => "gzip",
            Self::Deflate => "deflate",
            Self::Identity => "identity",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Preference {
    coding: CodingToken,
    quality: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CodingToken {
    Wildcard,
    Supported(ContentCoding),
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptEncoding {
    field_present: bool,
    preferences: Vec<Preference>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncodingError {
    TooLarge,
    TooManyCodings,
    InvalidSyntax,
    DuplicateCoding,
    InvalidQuality,
}

impl AcceptEncoding {
    /// Parses all field lines as one bounded RFC list. Duplicate codings are
    /// rejected because conflicting weights make representation choice
    /// ambiguous and should not be silently normalized.
    pub fn parse(headers: &Headers) -> Result<Self, EncodingError> {
        let values = headers
            .get_all("accept-encoding")
            .map(|values| values.collect::<Vec<_>>())
            .unwrap_or_default();
        if values.is_empty() {
            return Ok(Self {
                field_present: false,
                preferences: Vec::new(),
            });
        }

        let mut bytes = 0usize;
        let mut preferences = Vec::new();
        for value in values {
            let value = value.to_str().map_err(|_| EncodingError::InvalidSyntax)?;
            bytes = bytes
                .checked_add(value.len())
                .ok_or(EncodingError::TooLarge)?;
            if bytes > MAX_ACCEPT_ENCODING_BYTES {
                return Err(EncodingError::TooLarge);
            }
            if value.trim_matches(is_ows).is_empty() {
                continue;
            }
            for member in value.split(',') {
                if preferences.len() >= MAX_ACCEPT_ENCODINGS {
                    return Err(EncodingError::TooManyCodings);
                }
                let (coding, quality) = parse_member(member)?;
                if preferences
                    .iter()
                    .any(|item: &Preference| item.coding == coding)
                {
                    return Err(EncodingError::DuplicateCoding);
                }
                preferences.push(Preference { coding, quality });
            }
        }
        Ok(Self {
            field_present: true,
            preferences,
        })
    }

    /// Returns the negotiated supported coding with the greatest quality.
    /// Ties follow the server's `available` order. `None` means no offered
    /// coding is acceptable; identity is considered only if listed or implied.
    pub fn negotiate(&self, available: &[ContentCoding]) -> Option<ContentCoding> {
        let mut best: Option<(ContentCoding, u16)> = None;
        for coding in available.iter().copied() {
            let quality = self.quality(coding);
            if quality == 0 {
                continue;
            }
            if best.is_none_or(|(_, best_quality)| quality > best_quality) {
                best = Some((coding, quality));
            }
        }
        best.map(|(coding, _)| coding)
    }

    pub fn quality(&self, coding: ContentCoding) -> u16 {
        if !self.field_present {
            return 1000;
        }
        if let Some(explicit) = self
            .preferences
            .iter()
            .find(|item| item.coding == CodingToken::Supported(coding))
        {
            return explicit.quality;
        }
        if coding == ContentCoding::Identity {
            if self
                .preferences
                .iter()
                .any(|item| item.coding == CodingToken::Wildcard)
            {
                return self
                    .preferences
                    .iter()
                    .find(|item| item.coding == CodingToken::Wildcard)
                    .map_or(1000, |item| if item.quality == 0 { 0 } else { 1000 });
            }
            return 1000;
        }
        self.preferences
            .iter()
            .find(|item| item.coding == CodingToken::Wildcard)
            .map_or(0, |item| item.quality)
    }

    /// Returns a client's quality for a valid registered extension token.
    /// Known codings use their standard identity and wildcard rules.
    pub fn quality_for_token(&self, token: &str) -> u16 {
        if token.is_empty() || token == "*" || !token.bytes().all(is_token_byte) {
            return 0;
        }
        if token.eq_ignore_ascii_case("br") {
            return self.quality(ContentCoding::Brotli);
        }
        if token.eq_ignore_ascii_case("gzip") {
            return self.quality(ContentCoding::Gzip);
        }
        if token.eq_ignore_ascii_case("deflate") {
            return self.quality(ContentCoding::Deflate);
        }
        if token.eq_ignore_ascii_case("identity") {
            return self.quality(ContentCoding::Identity);
        }
        if !self.field_present {
            return 1000;
        }
        if let Some(explicit) = self.preferences.iter().find(|item| {
            matches!(&item.coding, CodingToken::Other(value) if value.eq_ignore_ascii_case(token))
        }) {
            return explicit.quality;
        }
        self.preferences
            .iter()
            .find(|item| item.coding == CodingToken::Wildcard)
            .map_or(0, |item| item.quality)
    }
}

fn parse_member(member: &str) -> Result<(CodingToken, u16), EncodingError> {
    let mut parts = member.split(';');
    let token = parts.next().unwrap_or_default().trim_matches(is_ows);
    if token.is_empty() || !token.bytes().all(is_token_byte) {
        return Err(EncodingError::InvalidSyntax);
    }
    let coding = if token == "*" {
        CodingToken::Wildcard
    } else if token.eq_ignore_ascii_case("br") {
        CodingToken::Supported(ContentCoding::Brotli)
    } else if token.eq_ignore_ascii_case("gzip") {
        CodingToken::Supported(ContentCoding::Gzip)
    } else if token.eq_ignore_ascii_case("deflate") {
        CodingToken::Supported(ContentCoding::Deflate)
    } else if token.eq_ignore_ascii_case("identity") {
        CodingToken::Supported(ContentCoding::Identity)
    } else {
        CodingToken::Other(token.to_ascii_lowercase())
    };
    let quality = parse_weight(parts)?;
    Ok((coding, quality))
}

fn parse_weight<'a>(mut parts: impl Iterator<Item = &'a str>) -> Result<u16, EncodingError> {
    let Some(parameter) = parts.next() else {
        return Ok(1000);
    };
    let parameter = parameter.trim_matches(is_ows);
    let Some((name, value)) = parameter.split_once('=') else {
        return Err(EncodingError::InvalidSyntax);
    };
    if !name.trim_matches(is_ows).eq_ignore_ascii_case("q") {
        return Err(EncodingError::InvalidSyntax);
    }
    if parts.next().is_some() {
        return Err(EncodingError::InvalidSyntax);
    }
    parse_quality(value.trim_matches(is_ows))
}

fn parse_quality(value: &str) -> Result<u16, EncodingError> {
    let bytes = value.as_bytes();
    if bytes == b"0" {
        return Ok(0);
    }
    if bytes == b"1" {
        return Ok(1000);
    }
    if bytes.len() < 2 || bytes[1] != b'.' || (bytes[0] != b'0' && bytes[0] != b'1') {
        return Err(EncodingError::InvalidQuality);
    }
    let fractional = &bytes[2..];
    if fractional.len() > 3 || !fractional.iter().all(u8::is_ascii_digit) {
        return Err(EncodingError::InvalidQuality);
    }
    if bytes[0] == b'1' && fractional.iter().any(|digit| *digit != b'0') {
        return Err(EncodingError::InvalidQuality);
    }
    let mut quality = 0u16;
    for (index, digit) in fractional.iter().enumerate() {
        let place = [100, 10, 1][index];
        quality += u16::from(*digit - b'0') * place;
    }
    if bytes[0] == b'1' {
        Ok(1000)
    } else {
        Ok(quality)
    }
}

fn is_ows(byte: char) -> bool {
    byte == ' ' || byte == '\t'
}

fn is_token_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric()
        || matches!(
            byte,
            b'!' | b'#'
                | b'$'
                | b'%'
                | b'&'
                | b'\''
                | b'*'
                | b'+'
                | b'-'
                | b'.'
                | b'^'
                | b'_'
                | b'`'
                | b'|'
                | b'~'
        )
}

#[cfg(test)]
mod tests {
    use super::{AcceptEncoding, ContentCoding, EncodingError};
    use crate::http_values::Headers;

    fn parse(value: &str) -> AcceptEncoding {
        let mut headers = Headers::new();
        headers.append("Accept-Encoding", value.as_bytes()).unwrap();
        AcceptEncoding::parse(&headers).unwrap()
    }

    #[test]
    fn absent_empty_and_explicit_identity_have_distinct_defaults() {
        let absent = AcceptEncoding::parse(&Headers::new()).unwrap();
        assert_eq!(absent.quality(ContentCoding::Brotli), 1000);
        assert_eq!(
            absent.negotiate(&[ContentCoding::Brotli]),
            Some(ContentCoding::Brotli)
        );

        let empty = parse("");
        assert_eq!(empty.quality(ContentCoding::Brotli), 0);
        assert_eq!(empty.quality(ContentCoding::Identity), 1000);
        assert_eq!(empty.negotiate(&[ContentCoding::Gzip]), None);

        let explicit = parse("identity;q=0, gzip;q=0.5");
        assert_eq!(explicit.quality(ContentCoding::Identity), 0);
        assert_eq!(explicit.quality(ContentCoding::Gzip), 500);
    }

    #[test]
    fn quality_wildcards_and_ties_choose_in_server_preference_order() {
        let preference = parse("gzip;q=0.7, *;q=0.4, br;q=0.9");
        assert_eq!(
            preference.negotiate(&[ContentCoding::Brotli, ContentCoding::Gzip]),
            Some(ContentCoding::Brotli)
        );
        assert_eq!(preference.quality(ContentCoding::Deflate), 400);
        assert_eq!(
            parse("gzip, br").negotiate(&[ContentCoding::Brotli, ContentCoding::Gzip]),
            Some(ContentCoding::Brotli)
        );
        assert_eq!(parse("*;q=0").quality(ContentCoding::Identity), 0);
        assert_eq!(parse("x-custom, *;q=0.5").quality(ContentCoding::Gzip), 500);
        assert_eq!(parse("x-custom;q=0.8").quality_for_token("X-CUSTOM"), 800);
        assert_eq!(parse("*;q=0.25").quality_for_token("zstd"), 250);
        assert_eq!(parse("*;q=0").quality_for_token("zstd"), 0);
        assert_eq!(parse("*").quality_for_token("not a token"), 0);
    }

    #[test]
    fn repeated_field_lines_are_combined_and_duplicates_rejected() {
        let mut headers = Headers::new();
        headers.append("accept-encoding", b"gzip;q=0.4").unwrap();
        headers.append("Accept-Encoding", b"br;q=0.8").unwrap();
        let parsed = AcceptEncoding::parse(&headers).unwrap();
        assert_eq!(
            parsed.negotiate(&[ContentCoding::Gzip, ContentCoding::Brotli]),
            Some(ContentCoding::Brotli)
        );

        headers.append("accept-encoding", b"gzip;q=1").unwrap();
        assert_eq!(
            AcceptEncoding::parse(&headers),
            Err(EncodingError::DuplicateCoding)
        );
    }

    #[test]
    fn malformed_weights_and_syntax_fail_closed() {
        for value in [
            "gzip;q=1.001",
            "gzip;q=0.0000",
            "gzip;q=foo",
            "gzip;q=0;level=1",
            "gzip,,br",
            "*;q=0, *;q=1",
        ] {
            let mut headers = Headers::new();
            headers.append("accept-encoding", value.as_bytes()).unwrap();
            assert!(
                AcceptEncoding::parse(&headers).is_err(),
                "accepted {value:?}"
            );
        }
        assert_eq!(parse("gzip;q=0.125").quality(ContentCoding::Gzip), 125);
    }

    #[test]
    fn field_and_coding_counts_are_bounded() {
        let mut headers = Headers::new();
        headers
            .append("accept-encoding", vec![b'a'; 4097].as_slice())
            .unwrap();
        assert_eq!(
            AcceptEncoding::parse(&headers),
            Err(EncodingError::TooLarge)
        );

        let mut headers = Headers::new();
        let value = (0..17)
            .map(|n| format!("x{n}"))
            .collect::<Vec<_>>()
            .join(",");
        headers.append("accept-encoding", value.as_bytes()).unwrap();
        assert_eq!(
            AcceptEncoding::parse(&headers),
            Err(EncodingError::TooManyCodings)
        );
    }
}
