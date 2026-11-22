//! Allowlisted token shapes. Anything else is dropped, not stored.

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
