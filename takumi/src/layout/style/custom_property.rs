use cssparser::{Parser, ParserInput, Token};

use crate::layout::style::{
  Angle, AnimationTime, AnimationTimingFunction, BackgroundImage, ColorInput, Filter, FromCss,
  Length, PercentageNumber, Transform,
};

pub(crate) fn custom_property_syntax_is_supported(raw_syntax: &str) -> bool {
  parse_custom_property_syntax(raw_syntax).is_some_and(|syntax| {
    let syntax = syntax.as_str();

    if syntax == "*" {
      return true;
    }

    let mut alternatives = custom_property_syntax_alternatives(syntax).peekable();
    alternatives.peek().is_some()
      && alternatives.all(|alternative| {
        custom_property_syntax_validator(alternative).is_some()
          || syntax_keyword_is_supported(alternative)
      })
  })
}

pub(crate) fn custom_property_value_matches_syntax(raw_syntax: &str, raw_value: &str) -> bool {
  parse_custom_property_syntax(raw_syntax).is_some_and(|syntax| {
    let syntax = syntax.as_str();

    if syntax == "*" {
      return true;
    }

    let mut alternatives = custom_property_syntax_alternatives(syntax).peekable();
    alternatives.peek().is_some()
      && alternatives.any(|alternative| {
        if let Some(validator) = custom_property_syntax_validator(alternative) {
          return validator(raw_value);
        }

        syntax_keyword_matches_value(alternative, raw_value)
      })
  })
}

fn parse_custom_property_syntax(raw_syntax: &str) -> Option<String> {
  let mut syntax_input = ParserInput::new(raw_syntax);
  let mut syntax_parser = Parser::new(&mut syntax_input);
  let syntax = syntax_parser.expect_string_cloned().ok()?;

  if !syntax_parser.is_exhausted() {
    return None;
  }

  Some(syntax.to_string())
}

fn custom_property_syntax_alternatives(syntax: &str) -> impl Iterator<Item = &str> + '_ {
  syntax
    .split('|')
    .map(str::trim)
    .filter(|part| !part.is_empty())
}

macro_rules! custom_property_syntax_validators {
  ($($syntax:literal => $validator:expr),+ $(,)?) => {
    fn custom_property_syntax_validator(syntax: &str) -> Option<fn(&str) -> bool> {
      match syntax {
        $($syntax => Some($validator),)+
        _ => None,
      }
    }
  };
}

fn value_matches_via_from_str<T>(raw_value: &str) -> bool
where
  T: for<'i> FromCss<'i>,
{
  T::from_str(raw_value).is_ok()
}

fn syntax_keyword_is_supported(keyword: &str) -> bool {
  let mut input = ParserInput::new(keyword);
  let mut parser = Parser::new(&mut input);
  parser.expect_ident().is_ok() && parser.is_exhausted()
}

fn syntax_keyword_matches_value(keyword: &str, raw_value: &str) -> bool {
  let mut input = ParserInput::new(raw_value);
  let mut parser = Parser::new(&mut input);
  parser.expect_ident_matching(keyword).is_ok() && parser.is_exhausted()
}

fn parse_single_token<'a>(raw_value: &'a str) -> Option<Token<'a>> {
  let mut input = ParserInput::new(raw_value);
  let mut parser = Parser::new(&mut input);
  let token = parser.next().ok()?.clone();
  parser.is_exhausted().then_some(token)
}

fn value_matches_custom_ident_syntax(raw_value: &str) -> bool {
  let Some(Token::Ident(ident)) = parse_single_token(raw_value) else {
    return false;
  };

  !matches!(
    ident.as_ref().to_ascii_lowercase().as_str(),
    "initial" | "inherit" | "unset" | "default" | "revert" | "revert-layer"
  )
}

fn value_matches_integer_syntax(raw_value: &str) -> bool {
  matches!(
    parse_single_token(raw_value),
    Some(Token::Number {
      int_value: Some(_),
      ..
    })
  )
}

fn value_matches_length_syntax(raw_value: &str) -> bool {
  match parse_single_token(raw_value) {
    Some(Token::Dimension { .. }) => value_matches_via_from_str::<Length<true>>(raw_value),
    Some(Token::Function(name)) if name.eq_ignore_ascii_case("calc") => {
      !raw_value.contains('%') && value_matches_via_from_str::<Length<true>>(raw_value)
    }
    _ => false,
  }
}

fn value_matches_length_percentage_syntax(raw_value: &str) -> bool {
  match parse_single_token(raw_value) {
    Some(Token::Dimension { .. } | Token::Percentage { .. }) => {
      value_matches_via_from_str::<Length<true>>(raw_value)
    }
    Some(Token::Function(name)) if name.eq_ignore_ascii_case("calc") => {
      value_matches_via_from_str::<Length<true>>(raw_value)
    }
    _ => false,
  }
}

fn value_matches_number_syntax(raw_value: &str) -> bool {
  matches!(parse_single_token(raw_value), Some(Token::Number { .. }))
}

fn value_matches_percentage_syntax(raw_value: &str) -> bool {
  matches!(
    parse_single_token(raw_value),
    Some(Token::Percentage { .. })
  ) && value_matches_via_from_str::<PercentageNumber>(raw_value)
}

custom_property_syntax_validators! {
  "<angle>" => value_matches_via_from_str::<Angle>,
  "<color>" => value_matches_via_from_str::<ColorInput<true>>,
  "<custom-ident>" => value_matches_custom_ident_syntax,
  "<easing-function>" => value_matches_via_from_str::<AnimationTimingFunction>,
  "<filter-function>" => value_matches_filter_function_syntax,
  "<image>" => value_matches_image_syntax,
  "<integer>" => value_matches_integer_syntax,
  "<length>" => value_matches_length_syntax,
  "<length-percentage>" => value_matches_length_percentage_syntax,
  "<number>" => value_matches_number_syntax,
  "<percentage>" => value_matches_percentage_syntax,
  "<time>" => value_matches_via_from_str::<AnimationTime>,
  "<transform-function>" => value_matches_transform_function_syntax,
}

fn value_matches_filter_function_syntax(raw_value: &str) -> bool {
  matches!(parse_single_token(raw_value), Some(Token::Function(_)))
    && value_matches_via_from_str::<Filter>(raw_value)
}

fn value_matches_image_syntax(raw_value: &str) -> bool {
  match parse_single_token(raw_value) {
    Some(Token::Function(_) | Token::UnquotedUrl(_)) => {
      value_matches_via_from_str::<BackgroundImage>(raw_value)
    }
    _ => false,
  }
}

fn value_matches_transform_function_syntax(raw_value: &str) -> bool {
  matches!(parse_single_token(raw_value), Some(Token::Function(_)))
    && value_matches_via_from_str::<Transform>(raw_value)
}
