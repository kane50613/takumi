//! Character references, decoded as the HTML tokenizer decodes them.
//! <https://html.spec.whatwg.org/multipage/parsing.html#character-reference-state>

use std::borrow::Cow;

use crate::named_references::NAMED_REFERENCES;

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

  while let Some(start) = rest.find('&') {
    out.push_str(&rest[..start]);
    rest = &rest[start + 1..];

    let decoded = match rest.strip_prefix('#') {
      Some(number) => numeric(number).map(|(character, length)| {
        out.push(character);
        length + 1
      }),
      None => named(rest, in_attribute).map(|(characters, length)| {
        out.push_str(characters);
        length
      }),
    };

    match decoded {
      Some(length) => rest = &rest[length..],
      None => out.push('&'),
    }
  }

  out.push_str(rest);
  Cow::Owned(out)
}

/// The characters the named reference after the `&` at the start of `text` stands for, and its
/// length.
fn named(text: &str, in_attribute: bool) -> Option<(&'static str, usize)> {
  let run = text
    .bytes()
    .take(LONGEST_NAME)
    .take_while(u8::is_ascii_alphanumeric)
    .count();

  if text[run..].starts_with(';')
    && let Some(characters) = lookup(&text[..=run])
  {
    return Some((characters, run + 1));
  }

  (2..=run.min(LONGEST_UNTERMINATED_NAME))
    .rev()
    .find_map(|length| Some((lookup(&text[..length])?, length)))
    .filter(|&(_, length)| {
      !in_attribute
        || !text[length..]
          .bytes()
          .next()
          .is_some_and(|next| next == b'=' || next.is_ascii_alphanumeric())
    })
}

fn lookup(name: &str) -> Option<&'static str> {
  NAMED_REFERENCES
    .binary_search_by(|(each, _)| (*each).cmp(name))
    .ok()
    .map(|index| NAMED_REFERENCES[index].1)
}

/// The character the numeric reference after the `&#` at the start of `text` stands for, and its
/// length.
fn numeric(text: &str) -> Option<(char, usize)> {
  let (digits, radix, prefix) = match text.strip_prefix(['x', 'X']) {
    Some(hex) => (hex, 16, 1),
    None => (text, 10, 0),
  };
  let count = digits
    .bytes()
    .take_while(|byte| char::from(*byte).is_digit(radix))
    .count();

  if count == 0 {
    return None;
  }

  let code_point = digits[..count]
    .chars()
    .filter_map(|digit| digit.to_digit(radix))
    .fold(0_u32, |value, digit| {
      value.saturating_mul(radix).saturating_add(digit)
    });
  let terminator = usize::from(digits[count..].starts_with(';'));

  Some((
    code_point_character(code_point),
    prefix + count + terminator,
  ))
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
