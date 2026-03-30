use std::{borrow::Cow, fmt::Write};

use cssparser::{ParseError, Parser, ParserInput, SourceLocation, Token};

use crate::layout::style::{
  CssInput, CssNumber, CssUnexpected, CssWideKeyword, FromCss, merge_enum_values,
};

use super::{LonghandId, ParsedDeclarations, PropertyId, ShorthandId};

#[derive(Debug, Clone)]
pub(super) struct CssInputParseFailure {
  location: SourceLocation,
  detail: Option<String>,
}

pub(super) enum CssInputParseError<'de> {
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
  pub(super) fn into_serde_error<E>(self, property_name: &str, property: PropertyId) -> E
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

pub(super) fn parse_css_wide_keyword(css_input: &CssInput<'_>) -> Option<CssWideKeyword> {
  match css_input {
    CssInput::Str(value) => {
      let mut parser_input = ParserInput::new(value.as_ref());
      let mut parser = Parser::new(&mut parser_input);
      CssWideKeyword::from_css(&mut parser).ok()
    }
    CssInput::Number(_) | CssInput::Unexpected(_) => None,
  }
}

pub(super) fn parse_css_input_value<'de, T>(
  css_input: CssInput<'de>,
) -> Result<T, CssInputParseError<'de>>
where
  T: for<'i> FromCss<'i>,
{
  match css_input {
    CssInput::Str(value) => {
      let source = value.to_string();
      let failure = match T::from_str(source.as_str()) {
        Ok(parsed_value) => return Ok(parsed_value),
        Err(error) => css_input_parse_failure(source.as_str(), error),
      };

      Err(CssInputParseError::Value {
        value,
        expected: T::EXPECT_MESSAGE
          .build_message(source.as_str(), merge_enum_values(T::VALID_TOKENS))
          .into(),
        failure: Some(failure),
      })
    }
    CssInput::Number(number) => {
      let source = number.to_string();

      T::from_str(&source).map_err(|_| CssInputParseError::NumberType {
        number,
        expected: T::EXPECT_MESSAGE
          .build_message(&source, merge_enum_values(T::VALID_TOKENS))
          .into(),
      })
    }
    CssInput::Unexpected(unexpected) => Err(CssInputParseError::UnexpectedType {
      unexpected,
      expected: T::EXPECT_MESSAGE
        .build_message("input", merge_enum_values(T::VALID_TOKENS))
        .into(),
    }),
  }
}

fn css_input_parse_failure(
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

pub(super) fn property_id_from_name(name: &str, normalize: fn(&str) -> Cow<'_, str>) -> PropertyId {
  if name.starts_with("--") {
    return PropertyId::Custom;
  }

  if let Some(property) = webkit_property_id_from_name(name) {
    return property;
  }

  let normalized = normalize(name);

  if let Some(property) = legacy_alias_property_id(normalized.as_ref()) {
    return property;
  }

  PropertyId::from_normalized_name(normalized.as_ref())
}

// Ref: https://developer.mozilla.org/en-US/docs/Web/CSS/Reference/Properties/row-gap
fn legacy_alias_property_id(name: &str) -> Option<PropertyId> {
  match name {
    "grid_gap" => Some(PropertyId::Shorthand(ShorthandId::Gap)),
    "grid_row_gap" => Some(PropertyId::Longhand(LonghandId::RowGap)),
    "grid_column_gap" => Some(PropertyId::Longhand(LonghandId::ColumnGap)),
    _ => None,
  }
}

fn webkit_property_id_from_name(name: &str) -> Option<PropertyId> {
  let suffix = name
    .strip_prefix("-webkit-")
    .or_else(|| name.strip_prefix("Webkit"))
    .or_else(|| name.strip_prefix("WebKit"))?;

  match suffix {
    "text-stroke" => Some(PropertyId::Shorthand(ShorthandId::WebkitTextStroke)),
    "text-stroke-width" => Some(PropertyId::Longhand(LonghandId::WebkitTextStrokeWidth)),
    "text-stroke-color" => Some(PropertyId::Longhand(LonghandId::WebkitTextStrokeColor)),
    "text-fill-color" => Some(PropertyId::Longhand(LonghandId::WebkitTextFillColor)),
    "TextStroke" => Some(PropertyId::Shorthand(ShorthandId::WebkitTextStroke)),
    "TextStrokeWidth" => Some(PropertyId::Longhand(LonghandId::WebkitTextStrokeWidth)),
    "TextStrokeColor" => Some(PropertyId::Longhand(LonghandId::WebkitTextStrokeColor)),
    "TextFillColor" => Some(PropertyId::Longhand(LonghandId::WebkitTextFillColor)),
    _ => None,
  }
}

pub(super) fn expand_shorthand<T>(
  value: T,
  expand: impl FnOnce(T, &mut ParsedDeclarations),
) -> ParsedDeclarations {
  let mut declarations = ParsedDeclarations::new();
  expand(value, &mut declarations);
  declarations
}

pub(super) fn normalize_kebab_property_name(name: &str) -> Cow<'_, str> {
  if !name
    .bytes()
    .any(|byte| byte == b'-' || byte.is_ascii_uppercase())
  {
    return Cow::Borrowed(name);
  }

  Cow::Owned(
    name
      .chars()
      .map(|ch| match ch {
        '-' => '_',
        _ => ch.to_ascii_lowercase(),
      })
      .collect(),
  )
}

pub(super) fn normalize_camel_property_name(name: &str) -> Cow<'_, str> {
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

  Cow::Owned(normalized.trim_start_matches('_').to_owned())
}

pub(super) fn contains_var_function(specified_value: &str) -> bool {
  fn contains_in_parser(input: &mut Parser<'_, '_>) -> bool {
    while let Ok(token) = input.next_including_whitespace_and_comments() {
      match token {
        Token::Function(name) if name.eq_ignore_ascii_case("var") => return true,
        Token::Function(_)
        | Token::ParenthesisBlock
        | Token::SquareBracketBlock
        | Token::CurlyBracketBlock => {
          if input
            .parse_nested_block(|input| {
              Ok::<_, ParseError<'_, Cow<'_, str>>>(contains_in_parser(input))
            })
            .unwrap_or(true)
          {
            return true;
          }
        }
        _ => {}
      }
    }

    false
  }

  let mut parser_input = ParserInput::new(specified_value);
  let mut parser = Parser::new(&mut parser_input);
  contains_in_parser(&mut parser)
}
