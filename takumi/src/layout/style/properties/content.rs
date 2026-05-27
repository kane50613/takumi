use cssparser::{Parser, Token, match_ignore_ascii_case};
use std::{fmt, sync::Arc};

use crate::layout::style::{
  Animatable, BackgroundImage, CssSyntaxKind, CssToken, FromCss, MakeComputed, ParseResult, ToCss,
  properties::write_css_string, tw::TailwindPropertyParser, unexpected_token,
};

/// CSS `content` property value for `::before` / `::after` pseudo-elements.
#[derive(Debug, Clone, Default, PartialEq)]
#[non_exhaustive]
pub enum ContentValue {
  /// `content: normal`. For `::before` / `::after` this behaves as `None`.
  #[default]
  Normal,
  /// `content: none`. Suppresses pseudo-element box generation.
  None,
  /// A non-empty list of generated content items.
  Items(Box<[ContentItem]>),
}

/// A single item in a `content: ...` list.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum ContentItem {
  /// A literal string, e.g. `content: "Hello"`.
  Text(Arc<str>),
  /// An image value: `url(...)`, `linear-gradient(...)`, etc.
  Image(Box<BackgroundImage>),
  /// `attr(name)` or `attr(name, "fallback")`, resolved at render-tree-build time
  /// against the originating element's attributes.
  AttrRef {
    /// The attribute name (case-insensitive lookup).
    name: Arc<str>,
    /// Fallback string when the attribute is missing.
    fallback: Arc<str>,
  },
}

impl MakeComputed for ContentValue {
  fn make_computed(&mut self, sizing: &crate::rendering::Sizing) {
    if let ContentValue::Items(items) = self {
      for item in items.iter_mut() {
        if let ContentItem::Image(image) = item {
          image.as_mut().make_computed(sizing);
        }
      }
    }
  }
}

impl Animatable for ContentValue {}

impl TailwindPropertyParser for ContentValue {
  fn parse_tw(token: &str) -> Option<Self> {
    match_ignore_ascii_case! {token,
      "none" => Some(ContentValue::None),
      "normal" => Some(ContentValue::Normal),
      _ => None,
    }
  }
}

impl<'i> FromCss<'i> for ContentValue {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    if input
      .try_parse(|input| input.expect_ident_matching("none"))
      .is_ok()
    {
      return Ok(ContentValue::None);
    }

    if input
      .try_parse(|input| input.expect_ident_matching("normal"))
      .is_ok()
    {
      return Ok(ContentValue::Normal);
    }

    let mut items = Vec::new();
    let mut unsupported = false;
    while !input.is_exhausted() {
      match input.try_parse(ContentItem::from_css) {
        Ok(item) => items.push(item),
        // Recognized-but-unsupported tokens (counter(), open-quote, …) advance
        // the parser past them and collapse the whole declaration to `none`.
        Err(_) => {
          skip_unsupported_item(input)?;
          unsupported = true;
        }
      }
    }

    if unsupported {
      return Ok(ContentValue::None);
    }

    if items.is_empty() {
      let location = input.current_source_location();
      return Err(unexpected_token!(Self, location, &Token::WhiteSpace("")));
    }

    Ok(ContentValue::Items(items.into_boxed_slice()))
  }

  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Keyword("none"),
    CssToken::Keyword("normal"),
    CssToken::Syntax(CssSyntaxKind::String),
    CssToken::Syntax(CssSyntaxKind::Image),
  ];
}

impl<'i> FromCss<'i> for ContentItem {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    if let Ok(image) = input.try_parse(BackgroundImage::from_css)
      && !matches!(image, BackgroundImage::None)
    {
      return Ok(ContentItem::Image(Box::new(image)));
    }

    let location = input.current_source_location();
    let token = input.next()?.clone();
    match token {
      Token::QuotedString(value) => Ok(ContentItem::Text(value.as_ref().into())),
      Token::Function(ref name) if name.eq_ignore_ascii_case("attr") => {
        input.parse_nested_block(parse_attr_inner)
      }
      other => Err(unexpected_token!(ContentValue, location, &other)),
    }
  }

  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Syntax(CssSyntaxKind::String),
    CssToken::Syntax(CssSyntaxKind::Image),
  ];
}

fn skip_unsupported_item<'i>(input: &mut Parser<'i, '_>) -> ParseResult<'i, ()> {
  let location = input.current_source_location();
  let token = input.next()?.clone();
  match token {
    Token::Function(_) => input.parse_nested_block(|input| -> ParseResult<'i, ()> {
      while input.next().is_ok() {}
      Ok(())
    }),
    Token::Ident(_) => Ok(()),
    other => Err(unexpected_token!(ContentValue, location, &other)),
  }
}

fn parse_attr_inner<'i>(input: &mut Parser<'i, '_>) -> ParseResult<'i, ContentItem> {
  let name: Arc<str> = input.expect_ident()?.as_ref().into();
  let fallback: Arc<str> = if input.try_parse(Parser::expect_comma).is_ok() {
    input.expect_string()?.as_ref().into()
  } else {
    "".into()
  };
  Ok(ContentItem::AttrRef { name, fallback })
}

impl ToCss for ContentValue {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    match self {
      ContentValue::Normal => dest.write_str("normal"),
      ContentValue::None => dest.write_str("none"),
      ContentValue::Items(items) => {
        for (i, item) in items.iter().enumerate() {
          if i > 0 {
            dest.write_char(' ')?;
          }
          item.to_css(dest)?;
        }
        Ok(())
      }
    }
  }
}

impl ToCss for ContentItem {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    match self {
      ContentItem::Text(value) => write_css_string(dest, value),
      ContentItem::Image(image) => image.to_css(dest),
      ContentItem::AttrRef { name, fallback } => {
        dest.write_str("attr(")?;
        dest.write_str(name)?;
        if !fallback.is_empty() {
          dest.write_str(", ")?;
          write_css_string(dest, fallback)?;
        }
        dest.write_char(')')
      }
    }
  }
}

#[cfg(test)]
#[allow(clippy::panic, clippy::expect_used)]
mod tests {
  use super::*;

  fn parse(input: &str) -> ContentValue {
    ContentValue::from_str(input).expect("parse")
  }

  #[test]
  fn parses_none_and_normal() {
    assert_eq!(parse("none"), ContentValue::None);
    assert_eq!(parse("normal"), ContentValue::Normal);
  }

  #[test]
  fn parses_single_string() {
    let ContentValue::Items(items) = parse("\"hello\"") else {
      panic!("expected items");
    };
    assert_eq!(items.len(), 1);
    assert_eq!(items[0], ContentItem::Text("hello".into()));
  }

  #[test]
  fn parses_multiple_strings_as_list() {
    let ContentValue::Items(items) = parse("\"a\" \"b\"") else {
      panic!("expected items");
    };
    assert_eq!(items.len(), 2);
  }

  #[test]
  fn parses_attr_without_fallback() {
    let ContentValue::Items(items) = parse("attr(label)") else {
      panic!("expected items");
    };
    let ContentItem::AttrRef { name, fallback } = &items[0] else {
      panic!("expected attr");
    };
    assert_eq!(&**name, "label");
    assert_eq!(&**fallback, "");
  }

  #[test]
  fn parses_attr_with_fallback() {
    let ContentValue::Items(items) = parse("attr(label, \"unknown\")") else {
      panic!("expected items");
    };
    let ContentItem::AttrRef { fallback, .. } = &items[0] else {
      panic!("expected attr");
    };
    assert_eq!(&**fallback, "unknown");
  }

  #[test]
  fn parses_url_image() {
    let ContentValue::Items(items) = parse("url(\"icon.png\")") else {
      panic!("expected items");
    };
    assert!(matches!(
      &items[0],
      ContentItem::Image(image) if matches!(**image, BackgroundImage::Url(_)),
    ));
  }

  #[test]
  fn parses_mixed_list() {
    let ContentValue::Items(items) = parse("\"Prefix: \" attr(name) url(\"icon.png\")") else {
      panic!("expected items");
    };
    assert_eq!(items.len(), 3);
  }

  #[test]
  fn counter_drops_whole_value_to_none() {
    assert_eq!(parse("counter(foo)"), ContentValue::None);
    assert_eq!(parse("\"prefix\" counter(foo)"), ContentValue::None);
  }

  #[test]
  fn quote_keywords_drop_whole_value_to_none() {
    assert_eq!(parse("open-quote"), ContentValue::None);
    assert_eq!(parse("\"x\" close-quote"), ContentValue::None);
  }
}
