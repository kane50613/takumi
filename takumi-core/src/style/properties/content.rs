use std::{fmt, sync::Arc};

use cssparser::{Parser, Token, match_ignore_ascii_case, serialize_string};

use crate::style::{
  Animatable, BackgroundImage, CssSyntaxKind, CssToken, FromCss, MakeComputed, ParseResult, ToCss,
  tw::TailwindPropertyParser, unexpected_token,
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
pub enum ContentItem {
  /// A literal string, e.g. `content: "Hello"`.
  Text(Arc<str>),
  /// An image value: `url(...)`, `linear-gradient(...)`, etc.
  Image(Box<BackgroundImage>),
  /// `attr(name [, "fallback"])`, resolved at render-tree-build time against
  /// the originating element's attributes.
  Attr(AttrRef),
}

/// A parsed `attr(<name> [, <fallback>])` expression.
#[derive(Debug, Clone, PartialEq)]
pub struct AttrRef {
  /// The attribute name (case-insensitive lookup).
  pub name: Arc<str>,
  /// Fallback string when the attribute is missing.
  pub fallback: Arc<str>,
}

impl MakeComputed for ContentValue {
  fn make_computed(&mut self, sizing: &crate::style::SizingContext) {
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
    while !input.is_exhausted() {
      items.push(ContentItem::from_css(input)?);
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
    let start = input.state();
    if let Ok(image) = input.try_parse(BackgroundImage::from_css) {
      if !matches!(image, BackgroundImage::None) {
        return Ok(ContentItem::Image(Box::new(image)));
      }
      // Bare `none` ident inside a list isn't a valid item; report it from
      // the same position rather than letting it pass as an image.
      input.reset(&start);
    }

    let location = input.current_source_location();
    let token = input.next()?.clone();
    match token {
      Token::QuotedString(value) => Ok(ContentItem::Text(value.as_ref().into())),
      Token::Function(ref name) if name.eq_ignore_ascii_case("attr") => input
        .parse_nested_block(AttrRef::from_css)
        .map(ContentItem::Attr),
      other => Err(unexpected_token!(Self, location, &other)),
    }
  }

  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Syntax(CssSyntaxKind::String),
    CssToken::Syntax(CssSyntaxKind::Image),
  ];
}

impl<'i> FromCss<'i> for AttrRef {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let name: Arc<str> = input.expect_ident()?.as_ref().into();
    let fallback: Arc<str> = if input.try_parse(Parser::expect_comma).is_ok() {
      input.expect_string()?.as_ref().into()
    } else {
      "".into()
    };
    Ok(Self { name, fallback })
  }

  const VALID_TOKENS: &'static [CssToken] = &[CssToken::Syntax(CssSyntaxKind::Ident)];
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
      ContentItem::Text(value) => serialize_string(value, dest),
      ContentItem::Image(image) => image.to_css(dest),
      ContentItem::Attr(attr) => attr.to_css(dest),
    }
  }
}

impl ToCss for AttrRef {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    dest.write_str("attr(")?;
    dest.write_str(&self.name)?;
    if !self.fallback.is_empty() {
      dest.write_str(", ")?;
      serialize_string(&self.fallback, dest)?;
    }
    dest.write_char(')')
  }
}

#[cfg(test)]
#[allow(clippy::panic, clippy::expect_used)]
mod tests {
  use std::assert_matches;

  use super::*;
  use crate::style::FromCssStr;

  fn parse(input: &str) -> ContentValue {
    ContentValue::from_css_str(input).expect("parse")
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
  fn round_trips_control_characters() {
    let parsed = parse("\"a\\a b\"");

    assert_eq!(parse(&parsed.to_css_string()), parsed);
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
    let ContentItem::Attr(attr) = &items[0] else {
      panic!("expected attr");
    };
    assert_eq!(&*attr.name, "label");
    assert_eq!(&*attr.fallback, "");
  }

  #[test]
  fn parses_attr_with_fallback() {
    let ContentValue::Items(items) = parse("attr(label, \"unknown\")") else {
      panic!("expected items");
    };
    let ContentItem::Attr(attr) = &items[0] else {
      panic!("expected attr");
    };
    assert_eq!(&*attr.fallback, "unknown");
  }

  #[test]
  fn parses_url_image() {
    let ContentValue::Items(items) = parse("url(\"icon.png\")") else {
      panic!("expected items");
    };
    assert_matches!(
      &items[0],
      ContentItem::Image(image) if matches!(**image, BackgroundImage::Url(_))
    );
  }

  #[test]
  fn parses_mixed_list() {
    let ContentValue::Items(items) = parse("\"Prefix: \" attr(name) url(\"icon.png\")") else {
      panic!("expected items");
    };
    assert_eq!(items.len(), 3);
  }

  #[test]
  fn unsupported_function_is_rejected() {
    assert!(ContentValue::from_css_str("counter(foo)").is_err());
    assert!(ContentValue::from_css_str("\"prefix\" counter(foo)").is_err());
  }

  #[test]
  fn unsupported_keyword_is_rejected() {
    assert!(ContentValue::from_css_str("open-quote").is_err());
    assert!(ContentValue::from_css_str("\"x\" close-quote").is_err());
  }
}
