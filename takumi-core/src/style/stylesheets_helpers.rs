use std::{borrow::Cow, fmt::Write};

use cssparser::{
  Delimiter, ParseError, Parser, ParserInput, SourceLocation, Token, parse_important,
};

use super::{LonghandId, ParsedDeclarations, PropertyId, ShorthandId};
use crate::style::{CssInput, CssNumber, CssUnexpected, CssWideKeyword, FromCss};

#[derive(Debug, Clone)]
pub(crate) struct CssInputParseFailure {
  location: SourceLocation,
  detail: Option<String>,
}

pub(crate) enum CssInputParseError<'de> {
  Value {
    value: Cow<'de, str>,
    expected: Cow<'static, str>,
    failure: Option<CssInputParseFailure>,
  },
  NumberType {
    number: CssNumber,
    expected: Cow<'static, str>,
  },
  UnexpectedType {
    unexpected: CssUnexpected,
    expected: Cow<'static, str>,
  },
}

impl CssInputParseError<'_> {
  pub(crate) fn into_serde_error<E>(self, property_name: &str, property: PropertyId) -> E
  where
    E: serde::de::Error,
  {
    E::custom(self.message(property_name, property))
  }

  fn message(&self, property_name: &str, _property: PropertyId) -> String {
    let mut message = String::new();
    let value_kind = match self {
      Self::Value { .. } => "value",
      Self::NumberType { .. } | Self::UnexpectedType { .. } => "type",
    };
    let _ = write!(message, "invalid {} for {}", value_kind, property_name);

    if let Self::Value { failure, .. } = self
      && let Some(failure) = failure
    {
      let _ = write!(
        message,
        ", line {}, column {}",
        failure.location.line + 1,
        failure.location.column
      );
      if let Some(detail) = &failure.detail {
        let _ = write!(message, " near \"{}\"", detail);
      }
    }

    let input_description = match self {
      Self::Value { value, .. } => format!("string {:?}", value),
      Self::NumberType { number, .. } => match number {
        CssNumber::Signed(value) => format!("integer `{value}`"),
        CssNumber::Unsigned(value) => format!("integer `{value}`"),
        CssNumber::Float(value) => format!("float `{value}`"),
      },
      Self::UnexpectedType { unexpected, .. } => match unexpected {
        CssUnexpected::Bool(value) => format!("boolean `{value}`"),
        CssUnexpected::Char(value) => format!("char `{value}`"),
        CssUnexpected::Bytes => "bytes".to_owned(),
        CssUnexpected::Unit => "unit".to_owned(),
        CssUnexpected::Seq => "sequence".to_owned(),
        CssUnexpected::Map => "map".to_owned(),
        CssUnexpected::Other(kind) => (*kind).to_owned(),
      },
    };

    let expected = match self {
      Self::Value { expected, .. }
      | Self::NumberType { expected, .. }
      | Self::UnexpectedType { expected, .. } => expected,
    };

    let _ = write!(
      message,
      ": {}; {}; also accepts 'initial', 'unset' or 'inherit'.",
      input_description, expected
    );

    message
  }
}

pub(crate) fn parse_css_wide_keyword(css_input: &CssInput<'_>) -> Option<CssWideKeyword> {
  match css_input {
    CssInput::Str(value) => {
      let mut parser_input = ParserInput::new(value.as_ref());
      let mut parser = Parser::new(&mut parser_input);
      CssWideKeyword::from_css(&mut parser).ok()
    }
    CssInput::Number(_) | CssInput::Unexpected(_) => None,
  }
}

pub(crate) fn css_input_parse_error<'de>(
  css_input: CssInput<'de>,
  expected: String,
  failure: CssInputParseFailure,
) -> CssInputParseError<'de> {
  match css_input {
    CssInput::Str(value) => CssInputParseError::Value {
      value,
      expected: expected.into(),
      failure: Some(failure),
    },
    CssInput::Number(number) => CssInputParseError::NumberType {
      number,
      expected: expected.into(),
    },
    CssInput::Unexpected(unexpected) => CssInputParseError::UnexpectedType {
      unexpected,
      expected: expected.into(),
    },
  }
}

pub(crate) fn css_input_parse_failure(
  source: &str,
  error: ParseError<'_, Cow<'_, str>>,
) -> CssInputParseFailure {
  let location = error.location;
  let Some(start) = source
    .char_indices()
    .nth(location.column.saturating_sub(1) as usize)
    .map(|(index, _)| index)
  else {
    return CssInputParseFailure {
      location,
      detail: None,
    };
  };

  let snippet = source[start..]
    .trim_start()
    .split([' ', '\t', '\n', '\r', ',', ')', '('])
    .next()
    .unwrap_or_default()
    .trim_matches('"')
    .trim_matches('\'');

  let snippet = snippet.chars().take(24).collect::<String>();
  if snippet.is_empty() {
    CssInputParseFailure {
      location,
      detail: None,
    }
  } else {
    CssInputParseFailure {
      location,
      detail: Some(snippet),
    }
  }
}

impl PropertyId {
  pub(crate) fn from_name(name: &str, normalize: fn(&str) -> Cow<'_, str>) -> PropertyId {
    if name.starts_with("--") {
      return PropertyId::Custom;
    }

    let normalized = normalize(name);

    Self::resolve(normalized.as_ref())
      .or_else(|| strip_vendor_prefix(normalized.as_ref()).and_then(Self::resolve))
      .unwrap_or(PropertyId::Ignored)
  }

  fn resolve(normalized: &str) -> Option<PropertyId> {
    if let Some(property) = legacy_alias_property_id(normalized) {
      return Some(property);
    }

    match PropertyId::from_normalized_name(normalized) {
      PropertyId::Ignored => None,
      property => Some(property),
    }
  }
}

// Ref: https://developer.mozilla.org/en-US/docs/Web/CSS/Reference/Properties/row-gap
fn legacy_alias_property_id(name: &str) -> Option<PropertyId> {
  match name {
    "grid_gap" => Some(PropertyId::Shorthand(ShorthandId::Gap)),
    "grid_row_gap" => Some(PropertyId::Longhand(LonghandId::RowGap)),
    "grid_column_gap" => Some(PropertyId::Longhand(LonghandId::ColumnGap)),
    // `continue` is a Rust keyword; its longhand field is `r#continue`, so the
    // name-derived lookup can't reach it.
    "continue" => Some(PropertyId::Longhand(LonghandId::Continue)),
    "page_break_before" => Some(PropertyId::Longhand(LonghandId::BreakBefore)),
    "page_break_after" => Some(PropertyId::Longhand(LonghandId::BreakAfter)),
    "page_break_inside" => Some(PropertyId::Longhand(LonghandId::BreakInside)),
    _ => None,
  }
}

fn strip_vendor_prefix(normalized: &str) -> Option<&str> {
  ["webkit_", "moz_", "ms_", "o_"]
    .into_iter()
    .find_map(|prefix| normalized.strip_prefix(prefix))
}

pub(crate) fn expand_shorthand<T>(
  value: T,
  expand: impl FnOnce(T, &mut ParsedDeclarations),
) -> ParsedDeclarations {
  let mut declarations = ParsedDeclarations::new();
  expand(value, &mut declarations);
  declarations
}

pub(crate) fn normalize_kebab_property_name(name: &str) -> Cow<'_, str> {
  if !name
    .bytes()
    .any(|byte| byte == b'-' || byte.is_ascii_uppercase())
  {
    return Cow::Borrowed(name);
  }

  let mut normalized: String = name
    .chars()
    .map(|ch| match ch {
      '-' => '_',
      _ => ch.to_ascii_lowercase(),
    })
    .collect();

  let leading = normalized.len() - normalized.trim_start_matches('_').len();

  if leading > 0 {
    normalized.drain(..leading);
  }

  Cow::Owned(normalized)
}

pub(crate) fn normalize_camel_property_name(name: &str) -> Cow<'_, str> {
  if !name.starts_with('_') && !name.bytes().any(|byte| byte.is_ascii_uppercase()) {
    return Cow::Borrowed(name);
  }

  let mut normalized = String::with_capacity(name.len() + 4);
  for ch in name.chars() {
    if ch.is_ascii_uppercase() {
      normalized.push('_');
      normalized.push(ch.to_ascii_lowercase());
    } else {
      normalized.push(ch);
    }
  }

  let leading = normalized.len() - normalized.trim_start_matches('_').len();

  if leading > 0 {
    normalized.drain(..leading);
  }

  Cow::Owned(normalized)
}

pub(crate) fn contains_var_function(specified_value: &str) -> bool {
  fn contains_in_parser(input: &mut Parser<'_, '_>) -> bool {
    loop {
      let should_check_nested_block = match input.next_including_whitespace_and_comments() {
        Ok(Token::Function(name)) => {
          if name.eq_ignore_ascii_case("var") {
            return true;
          }

          true
        }
        Ok(Token::ParenthesisBlock | Token::SquareBracketBlock | Token::CurlyBracketBlock) => true,
        Ok(_) => false,
        Err(_) => break,
      };

      if should_check_nested_block
        && input
          .parse_nested_block(|input| {
            Ok::<_, ParseError<'_, Cow<'_, str>>>(contains_in_parser(input))
          })
          .unwrap_or(true)
      {
        return true;
      }
    }

    false
  }

  let mut parser_input = ParserInput::new(specified_value);
  let mut parser = Parser::new(&mut parser_input);
  contains_in_parser(&mut parser)
}

/// Advances to the `!` a trailing `!important` starts with, leaving the marker
/// itself unread.
pub(crate) fn skip_to_important(parser: &mut Parser<'_, '_>) {
  let _ = parser.parse_until_before(Delimiter::Bang, |parser| {
    while parser.next_including_whitespace_and_comments().is_ok() {}

    Ok::<_, ParseError<'_, ()>>(())
  });
}

/// The byte index where a trailing `!important` starts, if the value ends with one.
fn important_start(value: &str) -> Option<usize> {
  if !value.contains('!') {
    return None;
  }

  let mut parser_input = ParserInput::new(value);
  let mut parser = Parser::new(&mut parser_input);

  skip_to_important(&mut parser);
  let end = parser.position().byte_index();

  (parse_important(&mut parser).is_ok() && parser.is_exhausted()).then_some(end)
}

/// Splits a trailing `!important` off a declaration value.
pub(crate) fn split_important(css_input: CssInput<'_>) -> (CssInput<'_>, bool) {
  let CssInput::Str(value) = css_input else {
    return (css_input, false);
  };

  let Some(end) = important_start(value.as_ref()) else {
    return (CssInput::Str(value), false);
  };

  let value = match value {
    Cow::Borrowed(value) => Cow::Borrowed(&value[..end]),
    Cow::Owned(mut value) => {
      value.truncate(end);
      Cow::Owned(value)
    }
  };

  (CssInput::Str(value), true)
}
