//! Character references, decoded as the HTML tokenizer decodes them.
//! <https://html.spec.whatwg.org/multipage/parsing.html#character-reference-state>

use std::borrow::Cow;

use super::named_references::NAMED_REFERENCES;

/// Decodes the character references in text content.
pub(crate) fn decode(text: &str) -> Cow<'_, str> {
  decode_in(text, false)
}

/// Decodes the character references in an attribute value, where an unterminated named reference
/// followed by `=` or an alphanumeric stays as written.
pub(crate) fn decode_attribute(value: &str) -> Cow<'_, str> {
  decode_in(value, true)
}

fn decode_in(text: &str, in_attribute: bool) -> Cow<'_, str> {
  if !text.contains('&') {
    return Cow::Borrowed(text);
  }

  let mut out = String::with_capacity(text.len());
  let mut rest = text;

  while let Some((before, after)) = rest.split_once('&') {
    out.push_str(before);

    let decoded = match after.strip_prefix('#') {
      Some(number) => numeric(number).map(|(character, after)| {
        out.push(character);
        after
      }),
      None => named(after, in_attribute).map(|(characters, after)| {
        out.push_str(characters);
        after
      }),
    };

    rest = decoded.unwrap_or_else(|| {
      out.push('&');
      after
    });
  }

  out.push_str(rest);
  Cow::Owned(out)
}

/// The characters the named reference after the `&` at the start of `text` stands for, and the
/// text after it.
fn named(text: &str, in_attribute: bool) -> Option<(&'static str, &str)> {
  let run = text
    .bytes()
    .take(LONGEST_NAME)
    .take_while(u8::is_ascii_alphanumeric)
    .count();

  if let Some((name, after)) = text.split_at_checked(run + 1)
    && let Some(characters) = name.strip_suffix(';').and(NAMED_REFERENCES.get(name))
  {
    return Some((characters, after));
  }

  (2..=run.min(LONGEST_UNTERMINATED_NAME))
    .rev()
    .find_map(|length| {
      let (name, after) = text.split_at_checked(length)?;

      Some((NAMED_REFERENCES.get(name)?, after))
    })
    .filter(|&(_, after)| {
      !in_attribute
        || !after
          .bytes()
          .next()
          .is_some_and(|next| next == b'=' || next.is_ascii_alphanumeric())
    })
}

/// The character the numeric reference after the `&#` at the start of `text` stands for, and the
/// text after it.
fn numeric(text: &str) -> Option<(char, &str)> {
  let (digits, radix) = match text.strip_prefix(['x', 'X']) {
    Some(hex) => (hex, 16),
    None => (text, 10),
  };
  let count = digits
    .bytes()
    .take_while(|byte| char::from(*byte).is_digit(radix))
    .count();

  if count == 0 {
    return None;
  }

  let (digits, after) = digits.split_at_checked(count)?;
  let code_point = digits
    .chars()
    .filter_map(|digit| digit.to_digit(radix))
    .fold(0_u32, |value, digit| {
      value.saturating_mul(radix).saturating_add(digit)
    });
  let after = after.strip_prefix(';').unwrap_or(after);

  Some((code_point_character(code_point), after))
}

/// <https://html.spec.whatwg.org/multipage/parsing.html#numeric-character-reference-end-state>
fn code_point_character(code_point: u32) -> char {
  match code_point {
    0x80..=0x9f => WINDOWS_1252_C1
      .chars()
      .nth((code_point - 0x80) as usize)
      .unwrap_or(char::REPLACEMENT_CHARACTER),
    _ => char::from_u32(code_point)
      .filter(|&character| character != '\0')
      .unwrap_or(char::REPLACEMENT_CHARACTER),
  }
}

/// The longest name in [`NAMED_REFERENCES`], `;` included.
const LONGEST_NAME: usize = 32;

/// The longest name in [`NAMED_REFERENCES`] the tokenizer decodes without a `;`.
const LONGEST_UNTERMINATED_NAME: usize = 6;

/// The characters a C1 numeric reference resolves to, 0x80 to 0x9f.
const WINDOWS_1252_C1: &str = "€\u{81}‚ƒ„…†‡ˆ‰Š‹Œ\u{8d}Ž\u{8f}\u{90}‘’“”•–—˜™š›œ\u{9d}žŸ";
