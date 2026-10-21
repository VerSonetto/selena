use crate::error::ProtocolError;

const MAX_EVENT_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SseEvent {
    pub event: Option<String>,
    pub data: String,
    pub id: Option<String>,
    pub retry: Option<u64>,
}

impl SseEvent {
    pub fn data(data: impl Into<String>) -> Self {
        Self {
            data: data.into(),
            ..Self::default()
        }
    }

    pub fn named(event: impl Into<String>, data: impl Into<String>) -> Self {
        Self {
            event: Some(event.into()),
            data: data.into(),
            ..Self::default()
        }
    }

    pub fn is_done(&self) -> bool {
        self.data.trim() == "[DONE]"
    }
}

#[derive(Debug, Default)]
pub struct SseParser {
    buffer: Vec<u8>,
}

impl SseParser {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, chunk: &[u8]) -> Vec<Result<SseEvent, ProtocolError>> {
        self.buffer.extend_from_slice(chunk);
        let mut out = Vec::new();

        if self.buffer.len() > MAX_EVENT_BYTES {
            self.buffer.clear();
            out.push(Err(ProtocolError::stream(
                "SSE event exceeded 16 MiB limit",
            )));
            return out;
        }

        while let Some((index, delimiter_len)) = find_boundary(&self.buffer) {
            let block: Vec<u8> = self.buffer.drain(..index + delimiter_len).collect();
            let body = &block[..index];
            match parse_block(body) {
                Ok(Some(event)) => out.push(Ok(event)),
                Ok(None) => {}
                Err(error) => out.push(Err(error)),
            }
        }

        out
    }

    pub fn finish(&mut self) -> Option<Result<SseEvent, ProtocolError>> {
        if self.buffer.is_empty() {
            return None;
        }
        let body = std::mem::take(&mut self.buffer);
        match parse_block(&body) {
            Ok(event) => event.map(Ok),
            Err(error) => Some(Err(error)),
        }
    }

    pub fn buffered_bytes(&self) -> usize {
        self.buffer.len()
    }
}

fn find_boundary(buffer: &[u8]) -> Option<(usize, usize)> {
    let mut i = 0;
    while i < buffer.len() {
        if i + 3 < buffer.len()
            && buffer[i] == b'\r'
            && buffer[i + 1] == b'\n'
            && buffer[i + 2] == b'\r'
            && buffer[i + 3] == b'\n'
        {
            return Some((i, 4));
        }
        if i + 1 < buffer.len() && buffer[i] == b'\n' && buffer[i + 1] == b'\n' {
            return Some((i, 2));
        }
        i += 1;
    }
    None
}

fn split_lines(block: &[u8]) -> Vec<&[u8]> {
    let mut lines = Vec::new();
    let mut start = 0;
    let mut i = 0;
    while i < block.len() {
        if block[i] == b'\n' {
            let end = if i > start && block[i - 1] == b'\r' {
                i - 1
            } else {
                i
            };
            lines.push(&block[start..end]);
            i += 1;
            start = i;
        } else if block[i] == b'\r' {
            lines.push(&block[start..i]);
            if i + 1 < block.len() && block[i + 1] == b'\n' {
                i += 2;
            } else {
                i += 1;
            }
            start = i;
        } else {
            i += 1;
        }
    }
    if start < block.len() {
        lines.push(&block[start..]);
    }
    lines
}

fn parse_block(block: &[u8]) -> Result<Option<SseEvent>, ProtocolError> {
    let text = std::str::from_utf8(block)
        .map_err(|_| ProtocolError::stream("SSE event is not valid UTF-8"))?;
    let mut event = SseEvent::default();
    let mut data_lines: Vec<&str> = Vec::new();

    for line in split_lines(text.as_bytes()) {
        let line = std::str::from_utf8(line)
            .map_err(|_| ProtocolError::stream("SSE line is not valid UTF-8"))?;
        if line.is_empty() || line.starts_with(':') {
            continue;
        }
        let (field, value) = match line.split_once(':') {
            Some((field, value)) => (field, value.strip_prefix(' ').unwrap_or(value)),
            None => (line, ""),
        };
        match field {
            "event" => event.event = Some(value.to_string()),
            "data" => data_lines.push(value),
            "id" => event.id = Some(value.to_string()),
            "retry" => {
                if let Ok(value) = value.parse::<u64>() {
                    event.retry = Some(value);
                }
            }
            _ => {}
        }
    }

    if data_lines.is_empty() && event.event.is_none() && event.id.is_none() {
        return Ok(None);
    }
    event.data = data_lines.join("\n");
    Ok(Some(event))
}

pub fn encode_event(event: &SseEvent) -> String {
    let mut out = String::new();
    if let Some(name) = &event.event {
        out.push_str("event: ");
        out.push_str(name);
        out.push('\n');
    }
    if let Some(id) = &event.id {
        out.push_str("id: ");
        out.push_str(id);
        out.push('\n');
    }
    if let Some(retry) = event.retry {
        out.push_str("retry: ");
        out.push_str(&retry.to_string());
        out.push('\n');
    }
    for line in event.data.split('\n') {
        out.push_str("data: ");
        out.push_str(line);
        out.push('\n');
    }
    out.push('\n');
    out
}

pub fn encode_data(data: impl Into<String>) -> String {
    encode_event(&SseEvent::data(data))
}

pub fn encode_named(event: impl Into<String>, data: impl Into<String>) -> String {
    encode_event(&SseEvent::named(event, data))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_crlf_and_multiline_data() {
        let mut parser = SseParser::new();
        let events = parser.push(b"event: message\r\ndata: hello\r\ndata: world\r\n\r\n");
        assert_eq!(events.len(), 1);
        let event = events[0].as_ref().unwrap();
        assert_eq!(event.event.as_deref(), Some("message"));
        assert_eq!(event.data, "hello\nworld");
    }

    #[test]
    fn parses_multiple_chunks() {
        let mut parser = SseParser::new();
        assert!(parser.push(b"data: {\"a\":").is_empty());
        let events = parser.push(b"1}\n\n");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].as_ref().unwrap().data, "{\"a\":1}");
    }

    #[test]
    fn encodes_multiline() {
        let text = encode_event(&SseEvent::data("a\nb"));
        assert_eq!(text, "data: a\ndata: b\n\n");
    }
}
