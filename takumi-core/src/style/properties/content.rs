use std::{fmt, sync::Arc};

use cssparser::{Parser, Token, serialize_identifier, serialize_string};

use crate::style::{
  Animatable, BackgroundImage, Direction, FromCss, ListStyleType, MakeComputed, ParseResult,
  SizingContext, ToCss, properties::counter::counter_name, tw::TailwindPropertyParser,
  unexpected_token,
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
  /// `counter()` or `counters()`, resolved at render-tree-build time against the counters in
  /// scope.
  Counter(CounterFunction),
  /// `open-quote`, `close-quote`, `no-open-quote`, or `no-close-quote`.
  Quote(Quote),
}

/// A quote keyword of `content`, which writes a mark of `quotes` and moves the nesting depth.
/// <https://drafts.csswg.org/css-content-3/#quote-values>
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quote {
  /// `open-quote`: the opening mark of the depth, then one level deeper.
  Open,
  /// `close-quote`: one level shallower, then the closing mark of that depth.
  Close,
  /// `no-open-quote`: one level deeper, with no mark.
  NoOpen,
  /// `no-close-quote`: one level shallower, with no mark.
  NoClose,
}

impl Quote {
  const KEYWORDS: [(&'static str, Quote); 4] = [
    ("open-quote", Quote::Open),
    ("close-quote", Quote::Close),
    ("no-open-quote", Quote::NoOpen),
    ("no-close-quote", Quote::NoClose),
  ];

  fn keyword(self) -> &'static str {
    Self::KEYWORDS
      .iter()
      .find(|(_, quote)| *quote == self)
      .map_or("", |(keyword, _)| keyword)
  }
}

/// `quotes`: the marks `open-quote` and `close-quote` write at each nesting depth.
/// <https://drafts.csswg.org/css-content-3/#quotes>
#[derive(Debug, Clone, Default, PartialEq)]
#[non_exhaustive]
pub enum Quotes {
  /// Approximate: Chromium takes the marks of the element's language, where this takes the
  /// English `“` `”` and `‘` `’` in every language.
  #[default]
  Auto,
  /// No marks, though the quote keywords still move the depth.
  None,
  /// An opening and a closing mark for each depth, the last pair serving every deeper one.
  Pairs(Box<[(Arc<str>, Arc<str>)]>),
}

impl Quotes {
  const AUTO: [(&'static str, &'static str); 2] =
    [("\u{201C}", "\u{201D}"), ("\u{2018}", "\u{2019}")];

  /// The mark that opens, or closes when `open` is false, the quote at `depth`.
  pub fn mark(&self, depth: usize, open: bool) -> &str {
    let pick = |pairs_len: usize| depth.min(pairs_len.saturating_sub(1));

    match self {
      Quotes::Auto => {
        let (opening, closing) = Self::AUTO[pick(Self::AUTO.len())];

        if open { opening } else { closing }
      }
      Quotes::None => "",
      Quotes::Pairs(pairs) => {
        pairs.get(pick(pairs.len())).map_or(
          "",
          |(opening, closing)| if open { opening } else { closing },
        )
      }
    }
  }
}

impl MakeComputed for Quotes {}

impl Animatable for Quotes {}

impl<'i> FromCss<'i> for Quotes {
  fn from_css(input: &mut Parser<'i>) -> ParseResult<'i, Self> {
    if input
      .try_parse(|input| input.expect_ident_matching("auto"))
      .is_ok()
    {
      return Ok(Quotes::Auto);
    }

    if input
      .try_parse(|input| input.expect_ident_matching("none"))
      .is_ok()
    {
      return Ok(Quotes::None);
    }

    let mut pairs = Vec::new();

    loop {
      let opening: Arc<str> = input.expect_string()?.as_ref().into();
      let closing: Arc<str> = input.expect_string()?.as_ref().into();

      pairs.push((opening, closing));

      if input.is_exhausted() {
        break;
      }
    }

    Ok(Quotes::Pairs(pairs.into_boxed_slice()))
  }

  const VALID_TOKENS: &'static [&'static str] = &["auto", "none", "<string> <string>"];
}

impl ToCss for Quotes {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    match self {
      Quotes::Auto => dest.write_str("auto"),
      Quotes::None => dest.write_str("none"),
      Quotes::Pairs(pairs) => {
        for (index, (opening, closing)) in pairs.iter().enumerate() {
          if index > 0 {
            dest.write_str(" ")?;
          }

          serialize_string(opening, dest)?;
          dest.write_str(" ")?;
          serialize_string(closing, dest)?;
        }

        Ok(())
      }
    }
  }
}

/// The most bytes one `counters()` writes, a resource limit: nested counters and a long separator
/// would otherwise grow generated text with the square of the nesting depth.
const MAX_COUNTERS_TEXT_BYTES: usize = 16 * 1024;

/// A `counter(<name>, <counter-style>?)` or `counters(<name>, <string>, <counter-style>?)`.
#[derive(Debug, Clone, PartialEq)]
pub struct CounterFunction {
  /// The counter's name.
  pub name: Arc<str>,
  /// The string `counters()` joins every counter of the name in scope with, or `None` for
  /// `counter()`, which reads the innermost one alone.
  pub separator: Option<Arc<str>>,
  /// The counter style the values are written in.
  pub style: ListStyleType,
}

impl CounterFunction {
  /// The text of `values`, the counters in scope from the innermost out, as Blink's
  /// `LayoutCounter::GenerateCounterText` writes them: the outermost first, up to
  /// [`MAX_COUNTERS_TEXT_BYTES`].
  pub(crate) fn text(&self, values: &[i32], direction: Direction) -> String {
    let represent = |value: &i32| {
      self
        .style
        .representation(*value, direction)
        .unwrap_or_default()
    };
    let Some(separator) = &self.separator else {
      return values.first().map(represent).unwrap_or_default();
    };
    let mut text = String::new();

    for (index, value) in values.iter().rev().enumerate() {
      let part = represent(value);
      let joined = if index == 0 { 0 } else { separator.len() };

      if text.len() + joined + part.len() > MAX_COUNTERS_TEXT_BYTES {
        break;
      }

      if index > 0 {
        text.push_str(separator);
      }

      text.push_str(&part);
    }

    text
  }

  /// The arguments of `counters()` when `joined` is set, of `counter()` otherwise.
  fn parse<'i>(input: &mut Parser<'i>, joined: bool) -> ParseResult<'i, Self> {
    let name = counter_name::<ContentItem>(input)?;
    let separator = if joined {
      input.expect_comma()?;

      Some(input.expect_string()?.as_ref().into())
    } else {
      None
    };
    let style = if input.try_parse(Parser::expect_comma).is_ok() {
      let token = input.next()?.clone();

      match &token {
        Token::Ident(ident) => ListStyleType::from_ident(ident),
        _ => None,
      }
      .ok_or_else(|| unexpected_token!(ContentItem, &token))?
    } else {
      ListStyleType::Decimal
    };

    Ok(Self {
      name,
      separator,
      style,
    })
  }
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
  fn make_computed(&mut self, sizing: &SizingContext) {
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

// `content-normal` is Tailwind's `align-content: normal`, so only `none` names a content.
impl TailwindPropertyParser for ContentValue {
  fn parse_tw(token: &str) -> Option<Self> {
    token
      .eq_ignore_ascii_case("none")
      .then_some(ContentValue::None)
  }
}

impl<'i> FromCss<'i> for ContentValue {
  fn from_css(input: &mut Parser<'i>) -> ParseResult<'i, Self> {
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
      return Err(unexpected_token!(Self, &Token::WhiteSpace("")));
    }

    Ok(ContentValue::Items(items.into_boxed_slice()))
  }

  const VALID_TOKENS: &'static [&'static str] = &["none", "normal", "<string>", "<image>"];
}

impl<'i> FromCss<'i> for ContentItem {
  fn from_css(input: &mut Parser<'i>) -> ParseResult<'i, Self> {
    let start = input.state();
    if let Ok(image) = input.try_parse(BackgroundImage::from_css) {
      if !matches!(image, BackgroundImage::None) {
        return Ok(ContentItem::Image(Box::new(image)));
      }
      // Bare `none` ident inside a list isn't a valid item; report it from
      // the same position rather than letting it pass as an image.
      input.reset(&start);
    }

    let token = input.next()?.clone();
    match token {
      Token::QuotedString(value) => Ok(ContentItem::Text(value.as_ref().into())),
      Token::Ident(ref name) => Quote::KEYWORDS
        .iter()
        .find(|(keyword, _)| name.eq_ignore_ascii_case(keyword))
        .map(|&(_, quote)| ContentItem::Quote(quote))
        .ok_or_else(|| unexpected_token!(Self, &token)),
      Token::Function(ref name) if name.eq_ignore_ascii_case("attr") => input
        .parse_nested_block(AttrRef::from_css)
        .map(ContentItem::Attr),
      Token::Function(ref name) if name.eq_ignore_ascii_case("counter") => input
        .parse_nested_block(|input| CounterFunction::parse(input, false))
        .map(ContentItem::Counter),
      Token::Function(ref name) if name.eq_ignore_ascii_case("counters") => input
        .parse_nested_block(|input| CounterFunction::parse(input, true))
        .map(ContentItem::Counter),
      other => Err(unexpected_token!(Self, &other)),
    }
  }

  const VALID_TOKENS: &'static [&'static str] = &[
    "<string>",
    "<image>",
    "counter()",
    "counters()",
    "open-quote",
    "close-quote",
    "no-open-quote",
    "no-close-quote",
  ];
}

impl<'i> FromCss<'i> for AttrRef {
  fn from_css(input: &mut Parser<'i>) -> ParseResult<'i, Self> {
    let name: Arc<str> = input.expect_ident()?.as_ref().into();
    let fallback: Arc<str> = if input.try_parse(Parser::expect_comma).is_ok() {
      input.expect_string()?.as_ref().into()
    } else {
      "".into()
    };
    Ok(Self { name, fallback })
  }

  const VALID_TOKENS: &'static [&'static str] = &["<ident>"];
}

impl ToCss for ContentValue {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    match self {
      ContentValue::Normal => dest.write_str("normal"),
      ContentValue::None => dest.write_str("none"),
      ContentValue::Items(items) => items.to_css(dest),
    }
  }
}

impl ToCss for ContentItem {
  const LIST_SEPARATOR: &'static str = " ";

  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    match self {
      ContentItem::Text(value) => serialize_string(value, dest),
      ContentItem::Image(image) => image.to_css(dest),
      ContentItem::Attr(attr) => attr.to_css(dest),
      ContentItem::Counter(counter) => counter.to_css(dest),
      ContentItem::Quote(quote) => dest.write_str(quote.keyword()),
    }
  }
}

impl ToCss for CounterFunction {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    dest.write_str(if self.separator.is_some() {
      "counters("
    } else {
      "counter("
    })?;
    serialize_identifier(&self.name, dest)?;

    if let Some(separator) = &self.separator {
      dest.write_str(", ")?;
      serialize_string(separator, dest)?;
    }

    if self.style != ListStyleType::Decimal {
      dest.write_str(", ")?;
      self.style.to_css(dest)?;
    }

    dest.write_char(')')
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

  fn attr(name: &str, fallback: &str) -> ContentItem {
    ContentItem::Attr(AttrRef {
      name: name.into(),
      fallback: fallback.into(),
    })
  }

  #[test]
  fn parses_single_string() {
    assert_eq!(
      parse("\"hello\""),
      ContentValue::Items(vec![ContentItem::Text("hello".into())].into())
    );
  }

  #[test]
  fn round_trips_control_characters() {
    let parsed = parse("\"a\\a b\"");

    assert_eq!(parse(&parsed.to_css_string()), parsed);
  }

  #[test]
  fn parses_multiple_strings_as_list() {
    assert_eq!(
      parse("\"a\" \"b\""),
      ContentValue::Items(
        vec![ContentItem::Text("a".into()), ContentItem::Text("b".into())].into()
      )
    );
  }

  #[test]
  fn parses_attr_without_fallback() {
    assert_eq!(
      parse("attr(label)"),
      ContentValue::Items(vec![attr("label", "")].into())
    );
  }

  #[test]
  fn parses_attr_with_fallback() {
    assert_eq!(
      parse("attr(label, \"unknown\")"),
      ContentValue::Items(vec![attr("label", "unknown")].into())
    );
  }

  #[test]
  fn parses_url_image() {
    assert_matches!(
      parse("url(\"icon.png\")"),
      ContentValue::Items(items)
        if matches!(&items[0], ContentItem::Image(image) if matches!(**image, BackgroundImage::Url(_)))
    );
  }

  #[test]
  fn parses_mixed_list() {
    assert_matches!(
      parse("\"Prefix: \" attr(name) url(\"icon.png\")"),
      ContentValue::Items(items) if items.len() == 3
    );
  }

  #[test]
  fn unsupported_function_is_rejected() {
    assert!(ContentValue::from_css_str("target-counter(attr(href), page)").is_err());
    assert!(ContentValue::from_css_str("\"prefix\" target-counter(attr(href), page)").is_err());
  }

  #[test]
  fn counter_functions_round_trip() {
    for css in [
      "counter(chapter)",
      "counter(chapter, upper-roman)",
      "counters(item, \".\")",
      "counters(item, \".\", lower-alpha)",
    ] {
      assert_eq!(parse(css).to_css_string(), css);
    }

    assert!(ContentValue::from_css_str("counter(none)").is_err());
    assert!(ContentValue::from_css_str("counters(item)").is_err());
  }

  #[test]
  fn unsupported_keyword_is_rejected() {
    assert!(ContentValue::from_css_str("contents").is_err());
    assert!(ContentValue::from_css_str("\"x\" open-quotes").is_err());
  }

  #[test]
  fn parses_quote_keywords() {
    assert_eq!(
      parse("open-quote \"x\" no-close-quote"),
      ContentValue::Items(
        [
          ContentItem::Quote(Quote::Open),
          ContentItem::Text("x".into()),
          ContentItem::Quote(Quote::NoClose),
        ]
        .into()
      )
    );
    assert_eq!(
      parse("close-quote no-open-quote").to_css_string(),
      "close-quote no-open-quote"
    );
  }

  #[test]
  fn parses_quotes() {
    assert_eq!(Quotes::from_css_str("auto"), Ok(Quotes::Auto));
    assert_eq!(Quotes::from_css_str("none"), Ok(Quotes::None));
    assert_eq!(
      Quotes::from_css_str("\"«\" \"»\" \"<\" \">\""),
      Ok(Quotes::Pairs(
        [("«".into(), "»".into()), ("<".into(), ">".into())].into()
      ))
    );
    assert!(Quotes::from_css_str("\"«\"").is_err());
  }

  #[test]
  fn deeper_quotes_take_the_last_pair() {
    let quotes = Quotes::from_css_str("\"P\" \"S\" \"A\" \"S\"").expect("parse");

    assert_eq!(quotes.mark(0, true), "P");
    assert_eq!(quotes.mark(1, true), "A");
    assert_eq!(quotes.mark(5, false), "S");
    assert_eq!(Quotes::Auto.mark(1, false), "\u{2019}");
    assert_eq!(Quotes::None.mark(0, true), "");
  }
}
