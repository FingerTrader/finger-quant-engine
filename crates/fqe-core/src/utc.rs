//! Explicit Gregorian UTC conversion. No wall clock, offset inference, timezone DB or leap seconds.
use std::fmt;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimeError {
    Syntax,
    Calendar,
    Precision,
    Overflow,
}
impl fmt::Display for TimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "UTC timestamp error: {self:?}")
    }
}
impl std::error::Error for TimeError {}
fn leap(y: u64) -> bool {
    y.is_multiple_of(4) && (!y.is_multiple_of(100) || y.is_multiple_of(400))
}
fn before_year(y: u64) -> u64 {
    let n = y - 1;
    365 * n + n / 4 - n / 100 + n / 400
}
fn month_days(y: u64, m: u64) -> u64 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if leap(y) {
                29
            } else {
                28
            }
        }
        _ => 0,
    }
}
pub fn micro_to_ns(micro: u64) -> Result<u64, TimeError> {
    micro.checked_mul(1000).ok_or(TimeError::Overflow)
}
pub fn ns_to_micro_exact(ns: u64) -> Result<u64, TimeError> {
    if !ns.is_multiple_of(1000) {
        Err(TimeError::Precision)
    } else {
        Ok(ns / 1000)
    }
}
/// YYYY-MM-DDTHH:MM:SS[.1..6 digits]Z; unsigned post-epoch UTC only.
pub fn parse_utc_ns(text: &str) -> Result<u64, TimeError> {
    let b = text.as_bytes();
    if !(20..=27).contains(&b.len()) || !text.is_ascii() {
        return Err(TimeError::Syntax);
    }
    if b[4] != b'-'
        || b[7] != b'-'
        || b[10] != b'T'
        || b[13] != b':'
        || b[16] != b':'
        || *b.last().unwrap() != b'Z'
    {
        return Err(TimeError::Syntax);
    }
    let part = |a: usize, z: usize| -> Result<u64, TimeError> {
        let s = &text[a..z];
        if !s.bytes().all(|b| b.is_ascii_digit()) {
            return Err(TimeError::Syntax);
        }
        s.parse().map_err(|_| TimeError::Syntax)
    };
    let y = part(0, 4)?;
    let m = part(5, 7)?;
    let d = part(8, 10)?;
    let h = part(11, 13)?;
    let minute = part(14, 16)?;
    let second = part(17, 19)?;
    if y < 1970
        || !(1..=12).contains(&m)
        || d == 0
        || d > month_days(y, m)
        || h > 23
        || minute > 59
        || second > 59
    {
        return Err(TimeError::Calendar);
    }
    let micros = if b.len() == 20 {
        0
    } else {
        if b[19] != b'.' || b.len() < 22 {
            return Err(TimeError::Syntax);
        }
        let count = b.len() - 21;
        part(20, b.len() - 1)? * 10_u64.pow((6 - count) as u32)
    };
    let days =
        before_year(y) - before_year(1970) + (1..m).map(|m| month_days(y, m)).sum::<u64>() + d - 1;
    let seconds = days
        .checked_mul(86400)
        .and_then(|n| n.checked_add(h * 3600 + minute * 60 + second))
        .ok_or(TimeError::Overflow)?;
    let micros = seconds
        .checked_mul(1_000_000)
        .and_then(|n| n.checked_add(micros))
        .ok_or(TimeError::Overflow)?;
    micro_to_ns(micros)
}
