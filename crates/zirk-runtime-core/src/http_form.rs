//! Strict, bounded `application/x-www-form-urlencoded` parsing.

pub const MAX_FORM_BODY_BYTES: usize = 1024 * 1024;
pub const MAX_FORM_FIELDS: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormError {
    BodyTooLarge,
    TooManyFields,
    InvalidEncoding,
}

/// Parses form fields while preserving duplicates and wire order.
/// Empty separators are ignored; a field without `=` has an empty value.
pub fn parse_form(bytes: &[u8]) -> Result<Vec<(String, String)>, FormError> {
    if bytes.len() > MAX_FORM_BODY_BYTES {
        return Err(FormError::BodyTooLarge);
    }
    let mut fields = Vec::new();
    for item in bytes
        .split(|byte| *byte == b'&')
        .filter(|item| !item.is_empty())
    {
        if fields.len() == MAX_FORM_FIELDS {
            return Err(FormError::TooManyFields);
        }
        let separator = item.iter().position(|byte| *byte == b'=');
        let (name, value) = match separator {
            Some(index) => (&item[..index], &item[index + 1..]),
            None => (item, &[][..]),
        };
        fields.push((decode_component(name)?, decode_component(value)?));
    }
    Ok(fields)
}

/// Encodes form fields using application/x-www-form-urlencoded byte rules.
/// Duplicate names and insertion order are preserved.
pub fn encode_form(fields: &[(String, String)]) -> Result<Vec<u8>, FormError> {
    if fields.len() > MAX_FORM_FIELDS {
        return Err(FormError::TooManyFields);
    }
    let encoded_len =
        fields
            .iter()
            .enumerate()
            .try_fold(0usize, |length, (index, (name, value))| {
                let length = if index == 0 {
                    length
                } else {
                    add_encoded_len(length, 1)?
                };
                add_encoded_len(
                    add_encoded_component_len(length, name.as_bytes())?,
                    add_encoded_len(1, encoded_component_len(value.as_bytes())?)?,
                )
            })?;
    let mut encoded = Vec::with_capacity(encoded_len);
    for (index, (name, value)) in fields.iter().enumerate() {
        if index != 0 {
            push_bounded(&mut encoded, b'&')?;
        }
        encode_component(name.as_bytes(), &mut encoded)?;
        push_bounded(&mut encoded, b'=')?;
        encode_component(value.as_bytes(), &mut encoded)?;
    }
    Ok(encoded)
}

fn encoded_component_len(input: &[u8]) -> Result<usize, FormError> {
    input.iter().try_fold(0usize, |length, byte| {
        let byte_len = match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'*' | b'-' | b'.' | b'_' | b' ' => 1,
            _ => 3,
        };
        add_encoded_len(length, byte_len)
    })
}

fn add_encoded_component_len(current: usize, input: &[u8]) -> Result<usize, FormError> {
    add_encoded_len(current, encoded_component_len(input)?)
}

fn add_encoded_len(current: usize, additional: usize) -> Result<usize, FormError> {
    current
        .checked_add(additional)
        .filter(|length| *length <= MAX_FORM_BODY_BYTES)
        .ok_or(FormError::BodyTooLarge)
}

fn encode_component(input: &[u8], output: &mut Vec<u8>) -> Result<(), FormError> {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    for byte in input {
        match *byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'*' | b'-' | b'.' | b'_' => {
                push_bounded(output, *byte)?;
            }
            b' ' => push_bounded(output, b'+')?,
            value => {
                push_bounded(output, b'%')?;
                push_bounded(output, HEX[(value >> 4) as usize])?;
                push_bounded(output, HEX[(value & 0x0f) as usize])?;
            }
        }
    }
    Ok(())
}

fn push_bounded(output: &mut Vec<u8>, byte: u8) -> Result<(), FormError> {
    if output.len() == MAX_FORM_BODY_BYTES {
        return Err(FormError::BodyTooLarge);
    }
    output.push(byte);
    Ok(())
}

fn decode_component(bytes: &[u8]) -> Result<String, FormError> {
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'+' => {
                decoded.push(b' ');
                index += 1;
            }
            b'%' => {
                if index + 2 >= bytes.len() {
                    return Err(FormError::InvalidEncoding);
                }
                let high = hex_value(bytes[index + 1]).ok_or(FormError::InvalidEncoding)?;
                let low = hex_value(bytes[index + 2]).ok_or(FormError::InvalidEncoding)?;
                decoded.push((high << 4) | low);
                index += 3;
            }
            byte => {
                decoded.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8(decoded).map_err(|_| FormError::InvalidEncoding)
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_components_and_preserves_repeated_fields_in_order() {
        assert_eq!(
            parse_form(b"q=hello+world&tag=a%2Bb&tag=two&empty&blank=").unwrap(),
            vec![
                ("q".into(), "hello world".into()),
                ("tag".into(), "a+b".into()),
                ("tag".into(), "two".into()),
                ("empty".into(), "".into()),
                ("blank".into(), "".into()),
            ]
        );
    }

    #[test]
    fn rejects_invalid_escape_and_invalid_utf8() {
        assert_eq!(parse_form(b"x=%"), Err(FormError::InvalidEncoding));
        assert_eq!(parse_form(b"x=%GG"), Err(FormError::InvalidEncoding));
        assert_eq!(parse_form(b"x=%FF"), Err(FormError::InvalidEncoding));
    }

    #[test]
    fn encodes_utf8_spaces_reserved_bytes_and_repeated_names() {
        let fields = vec![
            ("q".into(), "hello world".into()),
            ("tag".into(), "a+b".into()),
            ("tag".into(), "二".into()),
            ("empty".into(), "".into()),
        ];
        let encoded = encode_form(&fields).unwrap();
        assert_eq!(encoded, b"q=hello+world&tag=a%2Bb&tag=%E4%BA%8C&empty=");
        assert_eq!(parse_form(&encoded).unwrap(), fields);
    }

    #[test]
    fn encoder_enforces_field_and_encoded_byte_limits() {
        let too_many = (0..=MAX_FORM_FIELDS)
            .map(|index| (format!("n{index}"), String::new()))
            .collect::<Vec<_>>();
        assert_eq!(encode_form(&too_many), Err(FormError::TooManyFields));
        let encoded_too_large = vec![("x".to_owned(), "/".repeat(MAX_FORM_BODY_BYTES / 3 + 1))];
        assert!(matches!(
            encode_form(&encoded_too_large),
            Err(FormError::BodyTooLarge)
        ));
        let delimiter_overflow = vec![("x".repeat(MAX_FORM_BODY_BYTES), String::new())];
        assert!(matches!(
            encode_form(&delimiter_overflow),
            Err(FormError::BodyTooLarge)
        ));
        let exact_boundary = vec![("x".repeat(MAX_FORM_BODY_BYTES - 1), String::new())];
        let encoded = encode_form(&exact_boundary).unwrap();
        assert_eq!(encoded.len(), MAX_FORM_BODY_BYTES);
        assert!(encoded.capacity() <= MAX_FORM_BODY_BYTES);
    }

    #[test]
    fn enforces_body_and_field_limits() {
        assert_eq!(
            parse_form(&vec![b'x'; MAX_FORM_BODY_BYTES + 1]),
            Err(FormError::BodyTooLarge)
        );
        let joined = (0..=MAX_FORM_FIELDS)
            .flat_map(|_| [b'x', b'&'])
            .collect::<Vec<_>>();
        assert_eq!(parse_form(&joined), Err(FormError::TooManyFields));
    }
}
