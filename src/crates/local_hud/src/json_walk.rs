//! Streaming JSON reader. Allowlisted short values are copied. Every other
//! string is discarded while it is read. A 512-byte buffer can still hold a
//! fragment of those bytes; this is not zero access to the file.

use std::io::{BufReader, Read};

const BUFFER: usize = 512;
const MAX_TEXT: usize = 80;
const MAX_KEY: usize = 64;
const MAX_DEPTH: usize = 12;

#[derive(Clone, Debug)]
pub struct Hit {
    pub path: String,
    pub text: Option<String>,
    pub number: Option<f64>,
    pub flag: Option<bool>,
}

const TEXT: &[&str] = &[
    "type",
    "timestamp",
    "payload.type",
    "payload.id",
    "payload.timestamp",
    "payload.source",
    "payload.originator",
    "payload.model",
    "payload.rate_limits.plan_type",
    "sessionId",
];

const NUMBER: &[&str] = &[
    "ordinal",
    "payload.subagent_history_start_ordinal",
    "payload.info.total_token_usage.input_tokens",
    "payload.info.total_token_usage.output_tokens",
    "payload.info.total_token_usage.cached_input_tokens",
    "payload.info.total_token_usage.cache_write_input_tokens",
    "payload.info.total_token_usage.reasoning_output_tokens",
    "payload.rate_limits.primary.used_percent",
    "payload.rate_limits.primary.window_minutes",
    "payload.rate_limits.primary.resets_at",
    "payload.rate_limits.secondary.used_percent",
    "payload.rate_limits.secondary.window_minutes",
    "payload.rate_limits.secondary.resets_at",
    "usage.input_tokens",
    "usage.output_tokens",
    "usage.cached_input_tokens",
    "usage.cache_write_input_tokens",
    "usage.reasoning_output_tokens",
];

const FLAG: &[&str] = &["isArchived"];

pub fn for_each_value<R: Read>(reader: R, mut on_value: impl FnMut(&[Hit])) {
    let mut input = Input::new(reader);
    loop {
        input.skip_ws();
        match input.peek() {
            None => break,
            Some(b'{') => {
                let mut hits = Vec::new();
                if input.value("", &mut hits, 0) {
                    on_value(&hits);
                } else {
                    input.skip_to_newline();
                }
            }
            Some(_) => input.skip_to_newline(),
        }
    }
}

struct Input<R> {
    inner: BufReader<R>,
    unread: Option<u8>,
}

impl<R: Read> Input<R> {
    fn new(reader: R) -> Self {
        Self {
            inner: BufReader::with_capacity(BUFFER, reader),
            unread: None,
        }
    }

    fn byte(&mut self) -> Option<u8> {
        if let Some(byte) = self.unread.take() {
            return Some(byte);
        }
        let mut buf = [0];
        matches!(self.inner.read(&mut buf), Ok(1)).then_some(buf[0])
    }

    fn peek(&mut self) -> Option<u8> {
        let byte = self.byte()?;
        self.unread = Some(byte);
        Some(byte)
    }

    fn skip_to_newline(&mut self) {
        while let Some(byte) = self.byte() {
            if byte == b'\n' {
                break;
            }
        }
    }

    fn skip_ws(&mut self) {
        while let Some(byte) = self.byte() {
            if !byte.is_ascii_whitespace() {
                self.unread = Some(byte);
                break;
            }
        }
    }

    fn value(&mut self, path: &str, hits: &mut Vec<Hit>, depth: usize) -> bool {
        self.skip_ws();
        let Some(byte) = self.byte() else {
            return false;
        };
        match byte {
            b'{' => self.object(path, hits, depth),
            b'[' => self.array(path, hits, depth),
            b'"' => self.string_value(path, hits),
            b't' | b'f' => self.bool_value(path, hits, byte),
            b'n' => self.literal(b"null", byte),
            b'-' | b'0'..=b'9' => self.number_value(path, hits, byte),
            _ => false,
        }
    }

    fn object(&mut self, path: &str, hits: &mut Vec<Hit>, depth: usize) -> bool {
        if depth >= MAX_DEPTH {
            return false;
        }
        loop {
            self.skip_ws();
            let Some(byte) = self.byte() else {
                return false;
            };
            if byte == b'}' {
                return true;
            }
            if byte == b',' {
                continue;
            }
            if byte != b'"' {
                return false;
            }
            let Some(key) = self.raw_string(MAX_KEY) else {
                return false;
            };
            self.skip_ws();
            if self.byte() != Some(b':') {
                return false;
            }
            let child = if path.is_empty() {
                key
            } else {
                format!("{path}.{key}")
            };
            if child == "payload.source.subagent" {
                hits.push(Hit {
                    path: child.clone(),
                    text: None,
                    number: None,
                    flag: Some(true),
                });
            }
            if !self.value(&child, hits, depth + 1) {
                return false;
            }
        }
    }

    fn array(&mut self, path: &str, hits: &mut Vec<Hit>, depth: usize) -> bool {
        if depth >= MAX_DEPTH {
            return false;
        }
        loop {
            self.skip_ws();
            let Some(byte) = self.byte() else {
                return false;
            };
            if byte == b']' {
                return true;
            }
            if byte == b',' {
                continue;
            }
            self.unread = Some(byte);
            if !self.value(path, hits, depth + 1) {
                return false;
            }
        }
    }

    fn string_value(&mut self, path: &str, hits: &mut Vec<Hit>) -> bool {
        let keep = TEXT.contains(&path);
        let text = self.raw_string(if keep { MAX_TEXT } else { 0 });
        if keep {
            if let Some(text) = text {
                hits.push(hit(path, Some(text), None, None));
            }
        } else if path.starts_with("usage.") {
            hits.push(hit(path, None, None, None));
        }
        true
    }

    fn bool_value(&mut self, path: &str, hits: &mut Vec<Hit>, first: u8) -> bool {
        let word: &[u8] = if first == b't' { b"true" } else { b"false" };
        if !self.literal(word, first) {
            return false;
        }
        if FLAG.contains(&path) {
            hits.push(hit(path, None, None, Some(first == b't')));
        }
        true
    }

    fn number_value(&mut self, path: &str, hits: &mut Vec<Hit>, first: u8) -> bool {
        let mut raw = [0_u8; 48];
        raw[0] = first;
        let mut len = 1;
        while let Some(byte) = self.byte() {
            if byte.is_ascii_digit() || matches!(byte, b'.' | b'e' | b'E' | b'+' | b'-') {
                if len < raw.len() {
                    raw[len] = byte;
                    len += 1;
                }
            } else {
                self.unread = Some(byte);
                break;
            }
        }
        if NUMBER.contains(&path) {
            if let Some(number) = std::str::from_utf8(&raw[..len])
                .ok()
                .and_then(|text| text.parse().ok())
            {
                hits.push(hit(path, None, Some(number), None));
            }
        }
        true
    }

    fn raw_string(&mut self, max: usize) -> Option<String> {
        let mut out = String::new();
        let mut keep = true;
        loop {
            let byte = self.byte()?;
            if byte == b'\\' {
                self.byte()?;
                keep = false;
                continue;
            }
            if byte == b'"' {
                return keep.then_some(out);
            }
            if out.len() >= max || !byte.is_ascii() || byte < 0x20 {
                keep = false;
                continue;
            }
            out.push(byte as char);
        }
    }

    fn literal(&mut self, word: &[u8], first: u8) -> bool {
        if word.first() != Some(&first) {
            return false;
        }
        for expected in &word[1..] {
            if self.byte() != Some(*expected) {
                return false;
            }
        }
        true
    }
}

fn hit(path: &str, text: Option<String>, number: Option<f64>, flag: Option<bool>) -> Hit {
    Hit {
        path: path.to_owned(),
        text,
        number,
        flag,
    }
}
