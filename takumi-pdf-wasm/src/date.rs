//! Parsing of the `YYYY-MM-DD[THH:MM:SS]` dates accepted for metadata and
//! attachments.

use std::str::FromStr;

use takumi_pdf::PdfDate;

fn parse_field<T: FromStr>(part: Option<&str>, width: usize) -> Option<T> {
  let part = part?;

  if part.len() != width || !part.bytes().all(|byte| byte.is_ascii_digit()) {
    return None;
  }
  part.parse().ok()
}

pub(crate) fn parse_date(value: &str) -> Option<PdfDate> {
  let (date, time) = match value.split_once('T') {
    Some((date, time)) => (date, Some(time.strip_suffix('Z').unwrap_or(time))),
    None => (value, None),
  };
  let mut parts = date.splitn(3, '-');
  let year: u16 = parse_field(parts.next(), 4)?;
  let month: u8 = parse_field(parts.next(), 2)?;
  let day: u8 = parse_field(parts.next(), 2)?;
  let (hour, minute, second) = match time {
    Some(time) => {
      let mut parts = time.splitn(3, ':');
      (
        parse_field(parts.next(), 2)?,
        parse_field(parts.next(), 2)?,
        parse_field(parts.next(), 2)?,
      )
    }
    None => (0u8, 0u8, 0u8),
  };
  let leap = year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
  let days_in_month = match month {
    1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
    4 | 6 | 9 | 11 => 30,
    2 if leap => 29,
    2 => 28,
    _ => return None,
  };

  if day < 1 || day > days_in_month || hour > 23 || minute > 59 || second > 59 {
    return None;
  }
  Some(PdfDate {
    year,
    month,
    day,
    hour,
    minute,
    second,
  })
}

#[cfg(test)]
mod tests {
  use super::parse_date;

  #[test]
  fn parse_date_accepts_documented_formats() {
    assert!(parse_date("2026-08-06").is_some());
    assert!(parse_date("2026-08-06T01:02:03").is_some());
    assert!(parse_date("2026-08-06T01:02:03Z").is_some());
    assert!(parse_date("2028-02-29").is_some());
  }

  #[test]
  fn parse_date_rejects_invalid_input() {
    assert!(parse_date("2026-08-06T01:02:03ZZ").is_none());
    assert!(parse_date("2026-02-30").is_none());
    assert!(parse_date("2026-13-01").is_none());
    assert!(parse_date("26-08-06").is_none());
  }
}
