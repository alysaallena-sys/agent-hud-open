//! Allowlisted token shapes. Anything else is dropped, not stored.

use serde_json::Value;

pub fn short_token(value: Option<&Value>, max: usize) -> Option<String> {
    let text = value?.as_str()?;
    if text.is_empty() || text.len() > max || !is_token(text) {
        return None;
    }
    Some(text.to_owned())
}

pub fn is_token(text: &str) -> bool {
    text.bytes().all(|byte| {
        byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b':' | b'+')
    })
}

pub fn component_name(name: Option<&std::ffi::OsStr>, width: usize) -> bool {
    name.and_then(|name| name.to_str())
        .is_some_and(|name| name.len() == width && name.bytes().all(|byte| byte.is_ascii_digit()))
}

pub fn uuid(text: &str) -> bool {
    let bytes = text.as_bytes();
    if bytes.len() != 36 {
        return false;
    }
    for (index, byte) in bytes.iter().enumerate() {
        let hyphen = matches!(index, 8 | 13 | 18 | 23);
        if hyphen {
            if *byte != b'-' {
                return false;
            }
        } else if !byte.is_ascii_hexdigit() {
            return false;
        }
    }
    true
}

pub fn u64_field(value: &Value, key: &str) -> Option<u64> {
    value.get(key).and_then(Value::as_u64)
}
