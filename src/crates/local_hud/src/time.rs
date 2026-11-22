//! UTC timestamps as the log wrote them. Offsets and prose are rejected so a
//! prompt stuffed into a timestamp field cannot be kept.

use serde_json::Value;

pub fn unix_field(value: Option<&Value>) -> Option<i64> {
    let text = value?.as_str()?;
    unix_rfc3339_z(text)
}

pub fn unix_rfc3339_z(text: &str) -> Option<i64> {
    let bytes = text.as_bytes();
    if bytes.len() < 20 || bytes.len() > 32 || bytes[10] != b'T' || *bytes.last()? != b'Z' {
        return None;
    }
    if bytes[4] != b'-' || bytes[7] != b'-' || bytes[13] != b':' || bytes[16] != b':' {
        return None;
    }
    let year = parse_at(bytes, 0, 4)?;
    let month = parse_at(bytes, 5, 2)?;
    let day = parse_at(bytes, 8, 2)?;
    let hour = parse_at(bytes, 11, 2)?;
    let minute = parse_at(bytes, 14, 2)?;
    let second = parse_at(bytes, 17, 2)?;
    if bytes.len() > 20 {
        let rest = &bytes[19..bytes.len() - 1];
        if rest.first() != Some(&b'.') || rest.len() < 2 || rest.len() > 10 {
            return None;
        }
        if !rest[1..].iter().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
    }
    if !(1..=12).contains(&month) || hour > 23 || minute > 59 || second > 60 {
        return None;
    }
    let days = days_from_civil(year, month, day)?;
    Some(days * 86_400 + i64::from(hour) * 3_600 + i64::from(minute) * 60 + i64::from(second))
}

fn parse_at(bytes: &[u8], start: usize, width: usize) -> Option<i32> {
    let text = std::str::from_utf8(bytes.get(start..start + width)?).ok()?;
    text.parse().ok()
}

/// Howard Hinnant's civil-from-days, inverted. Invalid dates return None.
fn days_from_civil(year: i32, month: i32, day: i32) -> Option<i64> {
    if day < 1 {
        return None;
    }
    let year = year as i64;
    let month = month as i64;
    let day = day as i64;
    let year = if month <= 2 { year - 1 } else { year };
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let yoe = year - era * 400;
    let mp = if month > 2 { month - 3 } else { month + 9 };
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    let (check_year, check_month, check_day) = civil_from_days(days);
    (check_year == year as i32 && check_month == month as i32 && check_day == day as i32)
        .then_some(days)
}

fn civil_from_days(days: i64) -> (i32, i32, i32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { y + 1 } else { y };
    (year as i32, month as i32, day as i32)
}
