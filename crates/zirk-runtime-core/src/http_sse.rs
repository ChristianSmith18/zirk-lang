//! Incremental, bounded parsing and encoding for Server-Sent Events.

pub const MAX_SSE_LINE_BYTES: usize = 16 * 1024;
pub const MAX_SSE_EVENT_BYTES: usize = 64 * 1024;
pub const MAX_SSE_DATA_FIELDS: usize = 256;
pub const MAX_SSE_EVENTS_PER_FEED: usize = 256;
pub const MAX_SSE_ENCODED_EVENT_BYTES: usize = 128 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SseEvent {
    pub event: String,
    pub data: String,
    pub last_event_id: String,
    pub retry_ms: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SseError {
    LineTooLarge,
    EventTooLarge,
    TooManyDataFields,
    TooManyEvents,
    InvalidUtf8,
    InvalidEvent,
}

/// Incremental WHATWG-style event-stream parser with bounded retained state.
/// Comments are exposed as heartbeat counts; retry and event ID state persist
/// across feeds and dispatched events for reconnection by the caller.
#[derive(Debug, Default)]
pub struct SseParser {
    line: Vec<u8>,
    data: Vec<String>,
    data_bytes: usize,
    event_type: Option<String>,
    last_event_id: String,
    retry_ms: Option<u64>,
    heartbeat_count: u64,
    skip_lf: bool,
    first_line: bool,
}

impl SseParser {
    pub fn new() -> Self {
        Self {
            first_line: true,
            ..Self::default()
        }
    }

    /// Feeds one network fragment and emits complete events only. The line
    /// buffer retains at most `MAX_SSE_LINE_BYTES`; event data is similarly
    /// bounded even when a field spans multiple input fragments.
    pub fn feed(&mut self, bytes: &[u8]) -> Result<Vec<SseEvent>, SseError> {
        let mut events = Vec::new();
        for byte in bytes {
            if self.skip_lf {
                self.skip_lf = false;
                if *byte == b'\n' {
                    continue;
                }
            }
            match byte {
                b'\r' => {
                    self.process_line(&mut events)?;
                    self.skip_lf = true;
                }
                b'\n' => self.process_line(&mut events)?,
                _ => {
                    if self.line.len() == MAX_SSE_LINE_BYTES {
                        return Err(SseError::LineTooLarge);
                    }
                    self.line.push(*byte);
                }
            }
            if events.len() > MAX_SSE_EVENTS_PER_FEED {
                return Err(SseError::TooManyEvents);
            }
        }
        Ok(events)
    }

    /// End-of-stream discards an unterminated line/event, matching event
    /// stream dispatch rules; callers can still inspect reconnection state.
    pub fn finish(&mut self) {
        self.line.clear();
        self.data.clear();
        self.data_bytes = 0;
        self.event_type = None;
    }

    pub fn last_event_id(&self) -> &str {
        &self.last_event_id
    }

    pub fn retry_ms(&self) -> Option<u64> {
        self.retry_ms
    }

    /// Number of comment lines parsed, commonly used as a heartbeat signal.
    pub fn heartbeat_count(&self) -> u64 {
        self.heartbeat_count
    }

    fn process_line(&mut self, events: &mut Vec<SseEvent>) -> Result<(), SseError> {
        let mut line = std::mem::take(&mut self.line);
        if self.first_line {
            self.first_line = false;
            if line.starts_with(&[0xef, 0xbb, 0xbf]) {
                line.drain(..3);
            }
        }
        if line.is_empty() {
            if !self.data.is_empty() {
                events.push(SseEvent {
                    event: self
                        .event_type
                        .take()
                        .filter(|value| !value.is_empty())
                        .unwrap_or_else(|| "message".to_owned()),
                    data: self.data.join("\n"),
                    last_event_id: self.last_event_id.clone(),
                    retry_ms: self.retry_ms,
                });
                self.data.clear();
                self.data_bytes = 0;
            } else {
                self.event_type = None;
            }
            return Ok(());
        }
        if line[0] == b':' {
            self.heartbeat_count = self.heartbeat_count.saturating_add(1);
            return Ok(());
        }

        let separator = line.iter().position(|byte| *byte == b':');
        let (field, value) = match separator {
            Some(index) => {
                let mut value = &line[index + 1..];
                if value.first() == Some(&b' ') {
                    value = &value[1..];
                }
                (&line[..index], value)
            }
            None => (line.as_slice(), &[][..]),
        };
        let field = std::str::from_utf8(field).map_err(|_| SseError::InvalidUtf8)?;
        let value = std::str::from_utf8(value).map_err(|_| SseError::InvalidUtf8)?;
        match field {
            "data" => {
                if self.data.len() == MAX_SSE_DATA_FIELDS {
                    return Err(SseError::TooManyDataFields);
                }
                self.data_bytes = self
                    .data_bytes
                    .checked_add(value.len() + usize::from(!self.data.is_empty()))
                    .filter(|length| *length <= MAX_SSE_EVENT_BYTES)
                    .ok_or(SseError::EventTooLarge)?;
                self.data.push(value.to_owned());
            }
            "event" => {
                if value.len() > MAX_SSE_LINE_BYTES {
                    return Err(SseError::EventTooLarge);
                }
                self.event_type = Some(value.to_owned());
            }
            "id" if !value.contains('\0') => {
                self.last_event_id = value.to_owned();
            }
            "retry" if !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()) => {
                if let Ok(delay) = value.parse::<u64>() {
                    self.retry_ms = Some(delay);
                }
            }
            _ => {}
        }
        Ok(())
    }
}

/// Encodes one event as bounded UTF-8 event-stream framing.
pub fn encode_sse_event(
    event: Option<&str>,
    id: Option<&str>,
    data: &str,
) -> Result<Vec<u8>, SseError> {
    encode_sse_event_with_retry(event, id, None, data)
}

/// Encodes one event, including an optional reconnection delay, as bounded
/// UTF-8 event-stream framing.
pub fn encode_sse_event_with_retry(
    event: Option<&str>,
    id: Option<&str>,
    retry_ms: Option<u64>,
    data: &str,
) -> Result<Vec<u8>, SseError> {
    if event.is_some_and(|value| {
        value.contains(['\r', '\n']) || value.len().saturating_add(6) > MAX_SSE_LINE_BYTES
    }) || id.is_some_and(|value| {
        value.contains(['\r', '\n', '\0']) || value.len().saturating_add(3) > MAX_SSE_LINE_BYTES
    }) {
        return Err(SseError::InvalidEvent);
    }
    let normalized_data = data.replace("\r\n", "\n").replace('\r', "\n");
    let data = normalized_data.as_str();
    if data.len() > MAX_SSE_EVENT_BYTES {
        return Err(SseError::EventTooLarge);
    }
    let data_line_count = data.bytes().filter(|byte| *byte == b'\n').count() + 1;
    if data_line_count > MAX_SSE_DATA_FIELDS {
        return Err(SseError::TooManyDataFields);
    }
    let mut encoded_len = data
        .len()
        .checked_add(data_line_count.saturating_mul(7))
        .ok_or(SseError::EventTooLarge)?;
    if let Some(event) = event.filter(|value| !value.is_empty()) {
        encoded_len = encoded_len
            .checked_add(7 + event.len())
            .ok_or(SseError::EventTooLarge)?;
    }
    if let Some(id) = id {
        encoded_len = encoded_len
            .checked_add(4 + id.len())
            .ok_or(SseError::EventTooLarge)?;
    }
    let retry_text = retry_ms.map(|value| value.to_string());
    if let Some(retry) = &retry_text {
        encoded_len = encoded_len
            .checked_add(7 + retry.len())
            .ok_or(SseError::EventTooLarge)?;
    }
    encoded_len = encoded_len.checked_add(1).ok_or(SseError::EventTooLarge)?;
    if encoded_len > MAX_SSE_ENCODED_EVENT_BYTES {
        return Err(SseError::EventTooLarge);
    }
    let mut encoded = Vec::with_capacity(encoded_len);
    if let Some(event) = event.filter(|value| !value.is_empty()) {
        encoded.extend_from_slice(b"event:");
        encoded.extend_from_slice(event.as_bytes());
        encoded.extend_from_slice(b"\n");
    }
    if let Some(id) = id {
        encoded.extend_from_slice(b"id:");
        encoded.extend_from_slice(id.as_bytes());
        encoded.extend_from_slice(b"\n");
    }
    if let Some(retry) = retry_text {
        encoded.extend_from_slice(b"retry:");
        encoded.extend_from_slice(retry.as_bytes());
        encoded.extend_from_slice(b"\n");
    }
    for line in data.split('\n') {
        encoded.extend_from_slice(b"data:");
        if !line.is_empty() {
            encoded.push(b' ');
            encoded.extend_from_slice(line.as_bytes());
        }
        encoded.push(b'\n');
    }
    encoded.push(b'\n');
    Ok(encoded)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_incremental_crlf_multiline_events_and_persistent_reconnect_state() {
        let mut parser = SseParser::new();
        assert!(
            parser
                .feed(b"\xef\xbb\xbf: keep-alive\r")
                .unwrap()
                .is_empty()
        );
        let first = parser
            .feed(b"\nid: cursor-7\nevent: update\ndata: first\ndata: second\nretry: 2500\n\n")
            .unwrap();
        let events = parser.feed(b"data: next\n\n").unwrap();
        assert_eq!(first.len(), 1);
        assert_eq!(
            [first, events].concat(),
            vec![
                SseEvent {
                    event: "update".into(),
                    data: "first\nsecond".into(),
                    last_event_id: "cursor-7".into(),
                    retry_ms: Some(2500),
                },
                SseEvent {
                    event: "message".into(),
                    data: "next".into(),
                    last_event_id: "cursor-7".into(),
                    retry_ms: Some(2500),
                },
            ]
        );
        assert_eq!(parser.retry_ms(), Some(2500));
        assert_eq!(parser.last_event_id(), "cursor-7");
        assert_eq!(parser.heartbeat_count(), 1);
    }

    #[test]
    fn ignores_invalid_retry_and_nul_ids_and_dispatches_empty_data() {
        let mut parser = SseParser::new();
        let events = parser
            .feed(b"id: initial\nretry: 12x\ndata\n\nid: bad\0id\nretry: 45\ndata:\n\n")
            .unwrap();
        assert_eq!(events[0].data, "");
        assert_eq!(events[0].last_event_id, "initial");
        assert_eq!(events[1].last_event_id, "initial");
        assert_eq!(parser.retry_ms(), Some(45));
    }

    #[test]
    fn rejects_invalid_utf8_and_bounds_lines_events_and_feed_dispatch() {
        assert_eq!(
            SseParser::new().feed(b"data: \xff\n\n"),
            Err(SseError::InvalidUtf8)
        );
        assert_eq!(
            SseParser::new().feed(&vec![b'x'; MAX_SSE_LINE_BYTES + 1]),
            Err(SseError::LineTooLarge)
        );
        let mut parser = SseParser::new();
        let mut huge_event = Vec::new();
        for _ in 0..5 {
            huge_event.extend_from_slice(b"data: ");
            huge_event.extend(std::iter::repeat_n(b'x', MAX_SSE_LINE_BYTES - 6));
            huge_event.push(b'\n');
        }
        huge_event.push(b'\n');
        assert_eq!(parser.feed(&huge_event), Err(SseError::EventTooLarge));
        let mut parser = SseParser::new();
        let many_events = b"data:x\n\n".repeat(MAX_SSE_EVENTS_PER_FEED + 1);
        assert_eq!(parser.feed(&many_events), Err(SseError::TooManyEvents));
    }

    #[test]
    fn encoder_preserves_event_fields_and_roundtrips_multiline_data() {
        let encoded = encode_sse_event(Some("update"), Some("cursor-8"), "one\ntwo").unwrap();
        assert_eq!(
            encoded,
            b"event:update\nid:cursor-8\ndata: one\ndata: two\n\n"
        );
        let events = SseParser::new().feed(&encoded).unwrap();
        assert_eq!(events[0].event, "update");
        assert_eq!(events[0].last_event_id, "cursor-8");
        assert_eq!(events[0].data, "one\ntwo");
        assert_eq!(
            encode_sse_event(Some("bad\nevent"), None, "data"),
            Err(SseError::InvalidEvent)
        );
        assert_eq!(
            encode_sse_event(None, Some("bad\0id"), "data"),
            Err(SseError::InvalidEvent)
        );
        assert_eq!(
            encode_sse_event(Some(&"x".repeat(MAX_SSE_LINE_BYTES)), None, "data"),
            Err(SseError::InvalidEvent)
        );
    }

    #[test]
    fn encoder_preserves_retry_and_empty_id_reset_metadata() {
        let encoded =
            encode_sse_event_with_retry(None, Some(""), Some(2_500), "first\nsecond").unwrap();
        assert_eq!(encoded, b"id:\nretry:2500\ndata: first\ndata: second\n\n");
        let events = SseParser::new().feed(&encoded).unwrap();
        assert_eq!(events[0].event, "message");
        assert_eq!(events[0].last_event_id, "");
        assert_eq!(events[0].retry_ms, Some(2_500));
        assert_eq!(events[0].data, "first\nsecond");
    }

    #[test]
    fn encoder_normalizes_crlf_cr_and_lf_data_lines() {
        let encoded = encode_sse_event(None, None, "one\r\ntwo\rthree\nfour").unwrap();
        assert_eq!(
            encoded,
            b"data: one\ndata: two\ndata: three\ndata: four\n\n"
        );
        let events = SseParser::new().feed(&encoded).unwrap();
        assert_eq!(events[0].data, "one\ntwo\nthree\nfour");

        let trailing_cr = encode_sse_event(None, None, "last\r").unwrap();
        assert_eq!(trailing_cr, b"data: last\ndata:\n\n");
    }
}
