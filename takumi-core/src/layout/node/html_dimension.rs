//! HTML's rules for parsing dimension values: Blink's `ParseDimensionValue`
//! (`third_party/blink/renderer/core/html/html_dimension.cc`).
//! <https://html.spec.whatwg.org/multipage/common-microsyntaxes.html#rules-for-parsing-dimension-values>

use crate::style::Length;

/// Blink's `ParseDimensionValue`: a number of pixels or a percentage. Approximate: it trims Unicode
/// white space where Blink skips only HTML spaces, reads a trailing `*` as pixels where Blink's
/// `AddHTMLLengthToStyle` maps that relative dimension to nothing, and does not clamp a number
/// past `f32`'s range.
pub(crate) fn parse_dimension_value(value: &str) -> Option<Length> {
  let value = value.trim();
  let integer_end = ascii_digits_len(value);

  if integer_end == 0 {
    return None;
  }
  // A point is consumed even without a digit after it, and anything past the number is ignored.
  let (number_end, unit_start) = match value
    .get(integer_end..)
    .unwrap_or_default()
    .strip_prefix('.')
  {
    Some(fraction) if fraction.starts_with(|character: char| character.is_ascii_digit()) => {
      let end = integer_end + 1 + ascii_digits_len(fraction);

      (end, end)
    }
    Some(_) => (integer_end, integer_end + 1),
    None => (integer_end, integer_end),
  };
  let number: f32 = value.get(..number_end)?.parse().ok()?;
  let unit = value.get(unit_start..).unwrap_or_default();

  Some(if unit.starts_with('%') {
    Length::Percentage(number)
  } else {
    Length::Px(number)
  })
}

/// The length of the ASCII digits `text` starts with.
fn ascii_digits_len(text: &str) -> usize {
  text
    .find(|character: char| !character.is_ascii_digit())
    .unwrap_or(text.len())
}
