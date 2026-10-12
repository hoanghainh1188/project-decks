use chrono::{DateTime, Datelike, NaiveDate, Timelike, Utc};

pub const MINUTES_PER_DAY: i64 = 8 * 60;
pub const MINUTES_PER_HOUR: i64 = 60;

/// Project wall time encoded as Unix milliseconds. Calendar hours are this clock.
pub fn timestamp(year: i32, month: u32, day: u32, hour: u32, minute: u32) -> i64 {
    NaiveDate::from_ymd_opt(year, month, day)
        .and_then(|d| d.and_hms_opt(hour, minute, 0))
        .map(|n| n.and_utc().timestamp_millis())
        .unwrap_or(0)
}

pub fn civil(ms: i64) -> (NaiveDate, u16) {
    let dt = DateTime::<Utc>::from_timestamp_millis(ms).unwrap_or(DateTime::<Utc>::UNIX_EPOCH);
    let minute = (dt.hour() as u16) * 60 + dt.minute() as u16;
    (dt.date_naive(), minute)
}

pub fn compose(date: NaiveDate, minute: u16) -> i64 {
    let hour = u32::from(minute / 60);
    let min = u32::from(minute % 60);
    date.and_hms_opt(hour.min(23), min.min(59), 0)
        .map(|n| n.and_utc().timestamp_millis())
        .unwrap_or(0)
}

pub fn date_key(date: NaiveDate) -> String {
    format!("{:04}-{:02}-{:02}", date.year(), date.month(), date.day())
}

pub fn parse_date_key(key: &str) -> Option<NaiveDate> {
    let mut parts = key.split('-');
    let year: i32 = parts.next()?.parse().ok()?;
    let month: u32 = parts.next()?.parse().ok()?;
    let day: u32 = parts.next()?.parse().ok()?;
    NaiveDate::from_ymd_opt(year, month, day)
}

/// `2026-01-05T08:00:00` in project wall time.
pub fn format_iso(ms: i64) -> String {
    let dt = DateTime::<Utc>::from_timestamp_millis(ms).unwrap_or(DateTime::<Utc>::UNIX_EPOCH);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:00",
        dt.year(),
        dt.month(),
        dt.day(),
        dt.hour(),
        dt.minute()
    )
}

pub fn parse_iso(text: &str) -> Option<i64> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    let stripped = text.trim_end_matches('Z');
    let (date_part, time_part) = stripped.split_once('T')?;
    let date = parse_date_key(date_part)?;
    let mut time = time_part.split(':');
    let hour: u32 = time.next()?.parse().ok()?;
    let minute: u32 = time.next().unwrap_or("0").parse().ok()?;
    Some(timestamp(date.year(), date.month(), date.day(), hour, minute))
}

/// ISO-8601 duration of working time, e.g. 480 minutes → `PT8H0M0S`.
pub fn format_duration_iso(minutes: i64) -> String {
    let sign = if minutes < 0 { "-" } else { "" };
    let minutes = minutes.abs();
    let hours = minutes / 60;
    let mins = minutes % 60;
    format!("{sign}PT{hours}H{mins}M0S")
}

pub fn parse_duration_iso(text: &str) -> Option<i64> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    let (sign, rest) = if let Some(r) = text.strip_prefix('-') {
        (-1, r)
    } else {
        (1, text)
    };
    let rest = rest.strip_prefix("PT").or_else(|| rest.strip_prefix("P"))?;
    let mut hours: i64 = 0;
    let mut minutes: i64 = 0;
    let mut number = String::new();
    for ch in rest.chars() {
        if ch.is_ascii_digit() {
            number.push(ch);
        } else if ch == 'H' {
            hours = number.parse().ok()?;
            number.clear();
        } else if ch == 'M' {
            minutes = number.parse().ok()?;
            number.clear();
        } else if ch == 'S' {
            number.clear();
        }
    }
    Some(sign * (hours * 60 + minutes))
}
