//! Strict, bounded parser for `multipart/form-data` request bodies.
//!
//! File names are returned as untrusted metadata. This module never maps them
//! to paths or creates files; upload storage belongs to a separate managed
//! resource layer.

use crate::http_form::{MAX_FORM_BODY_BYTES, MAX_FORM_FIELDS};

pub const MAX_MULTIPART_BOUNDARY_BYTES: usize = 70;
pub const MAX_MULTIPART_PART_HEADER_BYTES: usize = 16 * 1024;
pub const MAX_MULTIPART_PARTS: usize = MAX_FORM_FIELDS;
pub const MAX_MULTIPART_ENCODED_BYTES: usize = MAX_FORM_BODY_BYTES;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultipartPart {
    pub name: String,
    /// Original untrusted filename metadata. Never use this directly as a path.
    pub filename: Option<String>,
    pub content_type: Option<String>,
    pub body: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MultipartError {
    BodyTooLarge,
    TooManyParts,
    InvalidBoundary,
    InvalidSyntax,
    InvalidHeaders,
    InvalidDisposition,
}

/// Serializes bounded multipart/form-data using caller-provided part metadata.
/// Metadata is validated before any bytes are emitted, so header injection
/// cannot produce a partially usable request body.
pub fn encode_multipart(
    parts: &[MultipartPart],
    boundary: &str,
) -> Result<Vec<u8>, MultipartError> {
    let boundary_bytes = boundary.as_bytes();
    if !valid_boundary(boundary_bytes) {
        return Err(MultipartError::InvalidBoundary);
    }
    if parts.len() > MAX_MULTIPART_PARTS {
        return Err(MultipartError::TooManyParts);
    }

    let mut encoded = Vec::new();
    for part in parts {
        validate_quoted_parameter(&part.name, false)?;
        if let Some(filename) = &part.filename {
            validate_quoted_parameter(filename, true)?;
        }
        if let Some(content_type) = &part.content_type
            && (content_type.is_empty()
                || content_type.bytes().any(|byte| byte < 0x20 || byte == 0x7f)
                || content_type.parse::<mime::Mime>().is_err())
        {
            return Err(MultipartError::InvalidHeaders);
        }
        if contains_next_part_delimiter(&part.body, boundary_bytes) {
            return Err(MultipartError::InvalidSyntax);
        }

        let mut headers = Vec::with_capacity(128);
        append_part_header_bytes(&mut headers, b"Content-Disposition: form-data; name=\"")?;
        append_quoted_parameter(&mut headers, &part.name)?;
        append_part_header_bytes(&mut headers, b"\"")?;
        if let Some(filename) = &part.filename {
            append_part_header_bytes(&mut headers, b"; filename=\"")?;
            append_quoted_parameter(&mut headers, filename)?;
            append_part_header_bytes(&mut headers, b"\"")?;
        }
        if let Some(content_type) = &part.content_type {
            append_part_header_bytes(&mut headers, b"\r\nContent-Type: ")?;
            append_part_header_bytes(&mut headers, content_type.as_bytes())?;
        }

        append_multipart_bytes(&mut encoded, b"--")?;
        append_multipart_bytes(&mut encoded, boundary_bytes)?;
        append_multipart_bytes(&mut encoded, b"\r\n")?;
        append_multipart_bytes(&mut encoded, &headers)?;
        append_multipart_bytes(&mut encoded, b"\r\n")?;
        append_multipart_bytes(&mut encoded, b"\r\n")?;
        append_multipart_bytes(&mut encoded, &part.body)?;
        append_multipart_bytes(&mut encoded, b"\r\n")?;
    }
    append_multipart_bytes(&mut encoded, b"--")?;
    append_multipart_bytes(&mut encoded, boundary_bytes)?;
    append_multipart_bytes(&mut encoded, b"--\r\n")?;
    Ok(encoded)
}

fn contains_next_part_delimiter(body: &[u8], boundary: &[u8]) -> bool {
    let mut delimiter = Vec::with_capacity(boundary.len() + 4);
    delimiter.extend_from_slice(b"\r\n--");
    delimiter.extend_from_slice(boundary);
    delimiter.extend_from_slice(b"\r\n");
    body.windows(delimiter.len())
        .any(|window| window == delimiter)
}

fn validate_quoted_parameter(value: &str, allow_empty: bool) -> Result<(), MultipartError> {
    if (!allow_empty && value.is_empty()) || value.bytes().any(|byte| byte < 0x20 || byte == 0x7f) {
        return Err(MultipartError::InvalidDisposition);
    }
    Ok(())
}

fn append_quoted_parameter(output: &mut Vec<u8>, value: &str) -> Result<(), MultipartError> {
    for byte in value.bytes() {
        if byte == b'"' || byte == b'\\' {
            append_part_header_bytes(output, b"\\")?;
        }
        append_part_header_bytes(output, &[byte])?;
    }
    Ok(())
}

fn append_part_header_bytes(output: &mut Vec<u8>, bytes: &[u8]) -> Result<(), MultipartError> {
    if bytes.len() > MAX_MULTIPART_PART_HEADER_BYTES.saturating_sub(output.len()) {
        return Err(MultipartError::InvalidHeaders);
    }
    output.extend_from_slice(bytes);
    Ok(())
}

fn append_multipart_bytes(output: &mut Vec<u8>, bytes: &[u8]) -> Result<(), MultipartError> {
    if bytes.len() > MAX_MULTIPART_ENCODED_BYTES.saturating_sub(output.len()) {
        return Err(MultipartError::BodyTooLarge);
    }
    output.extend_from_slice(bytes);
    Ok(())
}

type DispositionParameters = Vec<(Vec<u8>, Vec<u8>)>;

/// Parses a complete multipart body while preserving part order and binary
/// payload bytes. The total body, part count, boundary, and each part's header
/// block are independently bounded.
pub fn parse_multipart(body: &[u8], boundary: &str) -> Result<Vec<MultipartPart>, MultipartError> {
    let boundary = boundary.as_bytes();
    if body.len() > MAX_FORM_BODY_BYTES {
        return Err(MultipartError::BodyTooLarge);
    }
    if !valid_boundary(boundary) {
        return Err(MultipartError::InvalidBoundary);
    }

    let mut marker = Vec::with_capacity(boundary.len() + 2);
    marker.extend_from_slice(b"--");
    marker.extend_from_slice(boundary);
    if !body.starts_with(&marker) {
        return Err(MultipartError::InvalidSyntax);
    }
    let mut cursor = marker.len();
    if body.get(cursor..cursor + 2) == Some(b"--") {
        return if body.get(cursor + 2..).is_some_and(valid_final_crlf) {
            Ok(Vec::new())
        } else {
            Err(MultipartError::InvalidSyntax)
        };
    }
    if body.get(cursor..cursor + 2) != Some(b"\r\n") {
        return Err(MultipartError::InvalidSyntax);
    }
    cursor += 2;

    let mut parts = Vec::new();
    loop {
        if parts.len() == MAX_MULTIPART_PARTS {
            return Err(MultipartError::TooManyParts);
        }
        let header_end =
            find_bytes(body, cursor, b"\r\n\r\n").ok_or(MultipartError::InvalidSyntax)?;
        if header_end - cursor > MAX_MULTIPART_PART_HEADER_BYTES {
            return Err(MultipartError::InvalidHeaders);
        }
        let (name, filename, content_type) = parse_part_headers(&body[cursor..header_end])?;
        let payload_start = header_end + 4;
        let (payload_end, next) = find_delimiter(body, payload_start, &marker)?;
        parts.push(MultipartPart {
            name,
            filename,
            content_type,
            body: body[payload_start..payload_end].to_vec(),
        });
        match next {
            Delimiter::Next(cursor_after) => cursor = cursor_after,
            Delimiter::Final(cursor_after) => {
                return if body.get(cursor_after..).is_some_and(valid_final_crlf) {
                    Ok(parts)
                } else {
                    Err(MultipartError::InvalidSyntax)
                };
            }
        }
    }
}

fn valid_boundary(boundary: &[u8]) -> bool {
    !boundary.is_empty()
        && boundary.len() <= MAX_MULTIPART_BOUNDARY_BYTES
        && boundary.iter().enumerate().all(|(index, byte)| {
            byte.is_ascii_alphanumeric()
                || matches!(
                    byte,
                    b'\''
                        | b'('
                        | b')'
                        | b'+'
                        | b'_'
                        | b','
                        | b'-'
                        | b'.'
                        | b'/'
                        | b':'
                        | b'='
                        | b'?'
                )
                || (*byte == b' ' && index + 1 < boundary.len())
        })
}

fn valid_final_crlf(suffix: &[u8]) -> bool {
    suffix.is_empty() || suffix == b"\r\n"
}

fn parse_part_headers(
    bytes: &[u8],
) -> Result<(String, Option<String>, Option<String>), MultipartError> {
    if bytes.iter().enumerate().any(|(index, byte)| match byte {
        b'\r' => bytes.get(index + 1) != Some(&b'\n'),
        b'\n' => index == 0 || bytes.get(index - 1) != Some(&b'\r'),
        _ => false,
    }) {
        return Err(MultipartError::InvalidHeaders);
    }
    let mut disposition = None;
    let mut content_type = None;
    for line in bytes.split(|byte| *byte == b'\n') {
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        if line.contains(&b'\r') {
            return Err(MultipartError::InvalidHeaders);
        }
        if line.is_empty() || line[0].is_ascii_whitespace() {
            return Err(MultipartError::InvalidHeaders);
        }
        let colon = line
            .iter()
            .position(|byte| *byte == b':')
            .ok_or(MultipartError::InvalidHeaders)?;
        let name = &line[..colon];
        if name.is_empty() || !name.iter().all(|byte| is_token(*byte)) {
            return Err(MultipartError::InvalidHeaders);
        }
        let value = trim_ows(&line[colon + 1..]);
        if value
            .iter()
            .any(|byte| (*byte < 0x20 && *byte != b'\t') || *byte == 0x7f)
        {
            return Err(MultipartError::InvalidHeaders);
        }
        if name.eq_ignore_ascii_case(b"content-disposition") {
            if disposition.replace(value).is_some() {
                return Err(MultipartError::InvalidHeaders);
            }
        } else if name.eq_ignore_ascii_case(b"content-type")
            && content_type.replace(value).is_some()
        {
            return Err(MultipartError::InvalidHeaders);
        }
    }

    let disposition = disposition.ok_or(MultipartError::InvalidDisposition)?;
    let (kind, parameters) = parse_parameters(disposition)?;
    if !kind.eq_ignore_ascii_case(b"form-data") {
        return Err(MultipartError::InvalidDisposition);
    }
    let mut field_name = None;
    let mut filename = None;
    for (key, value) in parameters {
        if key.eq_ignore_ascii_case(b"name") {
            if field_name.replace(value).is_some() {
                return Err(MultipartError::InvalidDisposition);
            }
        } else if key.eq_ignore_ascii_case(b"filename") && filename.replace(value).is_some() {
            return Err(MultipartError::InvalidDisposition);
        }
    }
    let name = String::from_utf8(field_name.ok_or(MultipartError::InvalidDisposition)?)
        .map_err(|_| MultipartError::InvalidDisposition)?;
    let filename = filename
        .map(String::from_utf8)
        .transpose()
        .map_err(|_| MultipartError::InvalidDisposition)?;
    let content_type = content_type
        .map(|value| String::from_utf8(value.to_vec()))
        .transpose()
        .map_err(|_| MultipartError::InvalidHeaders)?;
    if content_type
        .as_deref()
        .is_some_and(|value| value.parse::<mime::Mime>().is_err())
    {
        return Err(MultipartError::InvalidHeaders);
    }
    Ok((name, filename, content_type))
}

fn parse_parameters(value: &[u8]) -> Result<(&[u8], DispositionParameters), MultipartError> {
    let mut pieces = split_parameters(value)?;
    let first = pieces.next().ok_or(MultipartError::InvalidDisposition)?;
    let kind = trim_ows(first);
    if kind.is_empty() {
        return Err(MultipartError::InvalidDisposition);
    }
    let mut parameters = Vec::new();
    for piece in pieces {
        let equals = piece
            .iter()
            .position(|byte| *byte == b'=')
            .ok_or(MultipartError::InvalidDisposition)?;
        let key = trim_ows(&piece[..equals]);
        let raw_value = trim_ows(&piece[equals + 1..]);
        if key.is_empty() || !key.iter().all(|byte| is_token(*byte)) {
            return Err(MultipartError::InvalidDisposition);
        }
        let parsed = if raw_value.starts_with(b"\"") {
            unquote(raw_value)?
        } else {
            if raw_value.is_empty() || !raw_value.iter().all(|byte| is_token(*byte)) {
                return Err(MultipartError::InvalidDisposition);
            }
            raw_value.to_vec()
        };
        parameters.push((key.to_vec(), parsed));
    }
    Ok((kind, parameters))
}

fn split_parameters(value: &[u8]) -> Result<impl Iterator<Item = &[u8]>, MultipartError> {
    let mut pieces = Vec::new();
    let mut start = 0;
    let mut quoted = false;
    let mut escaped = false;
    for (index, byte) in value.iter().copied().enumerate() {
        if escaped {
            escaped = false;
        } else if quoted && byte == b'\\' {
            escaped = true;
        } else if byte == b'"' {
            quoted = !quoted;
        } else if byte == b';' && !quoted {
            pieces.push(&value[start..index]);
            start = index + 1;
        }
    }
    if quoted || escaped {
        return Err(MultipartError::InvalidDisposition);
    }
    pieces.push(&value[start..]);
    Ok(pieces.into_iter())
}

fn unquote(value: &[u8]) -> Result<Vec<u8>, MultipartError> {
    if value.len() < 2 || !value.ends_with(b"\"") {
        return Err(MultipartError::InvalidDisposition);
    }
    let mut result = Vec::with_capacity(value.len() - 2);
    let mut escaped = false;
    for byte in &value[1..value.len() - 1] {
        if escaped {
            if *byte < 0x20 || *byte == 0x7f {
                return Err(MultipartError::InvalidDisposition);
            }
            result.push(*byte);
            escaped = false;
        } else if *byte == b'\\' {
            escaped = true;
        } else if *byte == b'"' || *byte < 0x20 || *byte == 0x7f {
            return Err(MultipartError::InvalidDisposition);
        } else {
            result.push(*byte);
        }
    }
    if escaped {
        return Err(MultipartError::InvalidDisposition);
    }
    Ok(result)
}

fn find_delimiter(
    body: &[u8],
    start: usize,
    marker: &[u8],
) -> Result<(usize, Delimiter), MultipartError> {
    let mut needle = Vec::with_capacity(marker.len() + 2);
    needle.extend_from_slice(b"\r\n");
    needle.extend_from_slice(marker);
    let mut candidate = start;
    while let Some(index) = find_bytes(body, candidate, &needle) {
        let suffix = index + needle.len();
        if body.get(suffix..suffix + 2) == Some(b"\r\n") {
            return Ok((index, Delimiter::Next(suffix + 2)));
        }
        if body.get(suffix..suffix + 2) == Some(b"--") {
            let end = suffix + 2;
            if body.get(end..).is_some_and(valid_final_crlf) {
                return Ok((index, Delimiter::Final(end)));
            }
        }
        candidate = index + 1;
    }
    Err(MultipartError::InvalidSyntax)
}

enum Delimiter {
    Next(usize),
    Final(usize),
}

fn find_bytes(haystack: &[u8], start: usize, needle: &[u8]) -> Option<usize> {
    haystack
        .get(start..)?
        .windows(needle.len())
        .position(|window| window == needle)
        .map(|offset| start + offset)
}

fn trim_ows(mut bytes: &[u8]) -> &[u8] {
    while bytes
        .first()
        .is_some_and(|byte| matches!(byte, b' ' | b'\t'))
    {
        bytes = &bytes[1..];
    }
    while bytes
        .last()
        .is_some_and(|byte| matches!(byte, b' ' | b'\t'))
    {
        bytes = &bytes[..bytes.len() - 1];
    }
    bytes
}

fn is_token(byte: u8) -> bool {
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
    use super::*;

    #[test]
    fn parses_ordered_fields_and_binary_file_content() {
        let body = b"--AaB03x\r\nContent-Disposition: form-data; name=\"caption\"\r\n\r\nhello\r\n--AaB03x\r\nContent-Disposition: form-data; name=\"upload\"; filename=\"../secret.bin\"\r\nContent-Type: application/octet-stream\r\n\r\n\x00\xff\r\n--AaB03x--\r\n";
        let parts = parse_multipart(body, "AaB03x").unwrap();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0].name, "caption");
        assert_eq!(parts[0].body, b"hello");
        assert_eq!(parts[1].name, "upload");
        assert_eq!(parts[1].filename.as_deref(), Some("../secret.bin"));
        assert_eq!(
            parts[1].content_type.as_deref(),
            Some("application/octet-stream")
        );
        assert_eq!(parts[1].body, b"\x00\xff");
    }

    #[test]
    fn ignores_boundary_prefix_inside_binary_payload() {
        let body = b"--b\r\nContent-Disposition: form-data; name=x\r\n\r\na\r\n--bXz\x00\r\n--b--";
        let parts = parse_multipart(body, "b").unwrap();
        assert_eq!(parts[0].body, b"a\r\n--bXz\x00");
    }

    #[test]
    fn rejects_bad_boundary_framing_headers_and_disposition() {
        assert_eq!(
            parse_multipart(b"", ""),
            Err(MultipartError::InvalidBoundary)
        );
        assert_eq!(
            parse_multipart(b"--b\r\n", "b"),
            Err(MultipartError::InvalidSyntax)
        );
        assert_eq!(
            parse_multipart(b"--b\r\nX: y\r\n\r\nz\r\n--b--", "b"),
            Err(MultipartError::InvalidDisposition)
        );
        let folded = b"--b\r\nContent-Disposition: form-data; name=x\r\n folded\r\n\r\nz\r\n--b--";
        assert_eq!(
            parse_multipart(folded, "b"),
            Err(MultipartError::InvalidHeaders)
        );
        let invalid_media_type = b"--b\r\nContent-Disposition: form-data; name=x\r\nContent-Type: not a media type\r\n\r\nz\r\n--b--";
        assert_eq!(
            parse_multipart(invalid_media_type, "b"),
            Err(MultipartError::InvalidHeaders)
        );
        let bare_lf = b"--b\r\nContent-Disposition: form-data; name=x\nX: y\r\n\r\nz\r\n--b--";
        assert_eq!(
            parse_multipart(bare_lf, "b"),
            Err(MultipartError::InvalidHeaders)
        );
        let delimiter_prefix =
            b"--b\r\nContent-Disposition: form-data; name=x\r\n\r\na\r\n--b--x\r\n--b--";
        assert_eq!(
            parse_multipart(delimiter_prefix, "b").unwrap()[0].body,
            b"a\r\n--b--x"
        );
    }

    #[test]
    fn enforces_total_body_and_part_count_bounds() {
        assert_eq!(
            parse_multipart(&vec![b'x'; MAX_FORM_BODY_BYTES + 1], "b"),
            Err(MultipartError::BodyTooLarge)
        );
        let mut body = Vec::new();
        for _ in 0..=MAX_FORM_FIELDS {
            body.extend_from_slice(b"--b\r\nContent-Disposition: form-data; name=x\r\n\r\nx\r\n");
        }
        body.extend_from_slice(b"--b--");
        assert_eq!(
            parse_multipart(&body, "b"),
            Err(MultipartError::TooManyParts)
        );
    }

    #[test]
    fn rejects_oversized_part_headers_and_invalid_utf8_metadata() {
        let huge = format!(
            "--b\r\nContent-Disposition: form-data; name=x\r\nX: {}\r\n\r\nx\r\n--b--",
            "a".repeat(MAX_MULTIPART_PART_HEADER_BYTES)
        );
        assert_eq!(
            parse_multipart(huge.as_bytes(), "b"),
            Err(MultipartError::InvalidHeaders)
        );
        let invalid_name =
            b"--b\r\nContent-Disposition: form-data; name=\"\xff\"\r\n\r\nx\r\n--b--";
        assert_eq!(
            parse_multipart(invalid_name, "b"),
            Err(MultipartError::InvalidDisposition)
        );
    }

    #[test]
    fn encoder_round_trips_binary_parts_and_escapes_metadata() {
        let parts = vec![
            MultipartPart {
                name: "note\"\r?".replace('\r', "x"),
                filename: None,
                content_type: Some("text/plain; charset=utf-8".to_owned()),
                body: "hello".as_bytes().to_vec(),
            },
            MultipartPart {
                name: "upload".to_owned(),
                filename: Some("a\\b\".bin".to_owned()),
                content_type: Some("application/octet-stream".to_owned()),
                body: vec![0, 0xff, b'\r', b'\n'],
            },
            MultipartPart {
                name: "optional-file".to_owned(),
                filename: Some(String::new()),
                content_type: None,
                body: vec![],
            },
        ];
        let encoded = encode_multipart(&parts, "Zirk-boundary").unwrap();
        let parsed = parse_multipart(&encoded, "Zirk-boundary").unwrap();
        assert_eq!(parsed, parts);
        assert!(
            encoded
                .windows(b"filename=\"a\\\\b\\\".bin\"".len())
                .any(|window| window == b"filename=\"a\\\\b\\\".bin\"")
        );
    }

    #[test]
    fn encoder_rejects_header_injection_and_bounds_final_body() {
        let injected = MultipartPart {
            name: "field\r\nX-Evil: yes".to_owned(),
            filename: None,
            content_type: None,
            body: vec![],
        };
        assert_eq!(
            encode_multipart(&[injected], "boundary"),
            Err(MultipartError::InvalidDisposition)
        );

        let huge = MultipartPart {
            name: "file".to_owned(),
            filename: Some("large.bin".to_owned()),
            content_type: None,
            body: vec![b'x'; MAX_MULTIPART_ENCODED_BYTES],
        };
        assert_eq!(
            encode_multipart(&[huge], "boundary"),
            Err(MultipartError::BodyTooLarge)
        );

        let oversized_header = MultipartPart {
            name: "file".to_owned(),
            filename: Some("x".repeat(MAX_MULTIPART_PART_HEADER_BYTES)),
            content_type: None,
            body: vec![],
        };
        assert_eq!(
            encode_multipart(&[oversized_header], "boundary"),
            Err(MultipartError::InvalidHeaders)
        );
    }

    #[test]
    fn encoder_rejects_a_boundary_that_collides_with_payload_framing() {
        let part = MultipartPart {
            name: "field".to_owned(),
            filename: None,
            content_type: None,
            body: b"prefix\r\n--chosen\r\nsuffix".to_vec(),
        };
        assert_eq!(
            encode_multipart(&[part], "chosen"),
            Err(MultipartError::InvalidSyntax)
        );
    }
}
