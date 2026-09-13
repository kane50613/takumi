//! `@page` rules: the page box's size and margins, per CSS Paged Media Level 3
//! §5 (page selectors) and §5.1 (the `size` descriptor).

use cssparser::{
  AtRuleParser, CowRcStr, DeclarationParser, ParseError, Parser, ParserState, QualifiedRuleParser,
  RuleBodyItemParser, RuleBodyParser, Token, match_ignore_ascii_case, parse_important,
};

use crate::{
  error::StyleSheetParseError,
  geometry::Rect,
  style::{
    CssToken, FromCss, Length, ParseResult, media_query::MediaQueryList, selector::LayerPath,
    unexpected_token,
  },
  viewport::Viewport,
};

/// A page pseudo-class, e.g. the `:first` in `@page :first`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PagePseudoClass {
  /// The first page of the document.
  First,
  /// A left-hand page in a spread.
  Left,
  /// A right-hand page in a spread.
  Right,
  /// A page the pagination added to keep left and right pages in step.
  Blank,
}

impl<'i> FromCss<'i> for PagePseudoClass {
  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Keyword("first"),
    CssToken::Keyword("left"),
    CssToken::Keyword("right"),
    CssToken::Keyword("blank"),
  ];

  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let location = input.current_source_location();
    let token = input.next()?;
    let Token::Ident(ident) = token else {
      return Err(unexpected_token!(location, token));
    };

    match_ignore_ascii_case! {ident,
      "first" => Ok(Self::First),
      "left" => Ok(Self::Left),
      "right" => Ok(Self::Right),
      "blank" => Ok(Self::Blank),
      _ => Err(unexpected_token!(location, token)),
    }
  }
}

/// One page selector: an optional page name and pseudo-classes, e.g.
/// `cover:first`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PageSelector {
  /// The named page this selects, set on elements by the `page` property.
  pub name: Option<String>,
  /// The pseudo-classes in source order.
  pub pseudo_classes: Vec<PagePseudoClass>,
}

impl PageSelector {
  /// Whether the selector applies to every page: no name and no pseudo-class.
  pub fn is_universal(&self) -> bool {
    self.name.is_none() && self.pseudo_classes.is_empty()
  }

  /// css-page-3 §7.2: the page name counts most, then `:first` and `:blank`,
  /// then `:left` and `:right`.
  pub fn specificity(&self) -> u32 {
    let (spread, side) = self
      .pseudo_classes
      .iter()
      .fold((0, 0), |(spread, side), pseudo| match pseudo {
        PagePseudoClass::First | PagePseudoClass::Blank => (spread + 1, side),
        PagePseudoClass::Left | PagePseudoClass::Right => (spread, side + 1),
      });

    (u32::from(self.name.is_some()) << 16) | (spread << 8) | side
  }

  /// Parses the prelude of `@page`: a comma-separated list of selectors, or
  /// nothing for every page.
  pub(crate) fn parse_list<'i>(
    input: &mut Parser<'i, '_>,
  ) -> Result<Vec<Self>, ParseError<'i, StyleSheetParseError>> {
    if input.is_exhausted() {
      return Ok(Vec::new());
    }
    input.parse_comma_separated(Self::parse)
  }

  fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, ParseError<'i, StyleSheetParseError>> {
    let mut selector = Self::default();

    if let Ok(name) = input.try_parse(|input| input.expect_ident().cloned()) {
      selector.name = Some(name.to_string());
    }
    while input.try_parse(Parser::expect_colon).is_ok() {
      let pseudo = PagePseudoClass::from_css(input).map_err(ParseError::into)?;

      selector.pseudo_classes.push(pseudo);
    }
    if selector.is_universal() {
      let location = input.current_source_location();
      let token = input.next()?.clone();

      return Err(location.new_unexpected_token_error(token));
    }
    Ok(selector)
  }
}

/// A page size keyword, portrait as css-page-3 writes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageSizeName {
  /// ISO A3.
  A3,
  /// ISO A4.
  A4,
  /// ISO A5.
  A5,
  /// ISO B4.
  B4,
  /// ISO B5.
  B5,
  /// JIS B4.
  JisB4,
  /// JIS B5.
  JisB5,
  /// US Ledger.
  Ledger,
  /// US Legal.
  Legal,
  /// US Letter.
  Letter,
}

impl<'i> FromCss<'i> for PageSizeName {
  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Keyword("A3"),
    CssToken::Keyword("A4"),
    CssToken::Keyword("A5"),
    CssToken::Keyword("B4"),
    CssToken::Keyword("B5"),
    CssToken::Keyword("JIS-B4"),
    CssToken::Keyword("JIS-B5"),
    CssToken::Keyword("ledger"),
    CssToken::Keyword("legal"),
    CssToken::Keyword("letter"),
  ];

  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let location = input.current_source_location();
    let token = input.next()?;
    let Token::Ident(ident) = token else {
      return Err(unexpected_token!(location, token));
    };

    match_ignore_ascii_case! {ident,
      "a3" => Ok(Self::A3),
      "a4" => Ok(Self::A4),
      "a5" => Ok(Self::A5),
      "b4" => Ok(Self::B4),
      "b5" => Ok(Self::B5),
      "jis-b4" => Ok(Self::JisB4),
      "jis-b5" => Ok(Self::JisB5),
      "ledger" => Ok(Self::Ledger),
      "legal" => Ok(Self::Legal),
      "letter" => Ok(Self::Letter),
      _ => Err(unexpected_token!(location, token)),
    }
  }
}

/// The orientation keyword in a `size` value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageOrientation {
  /// The shorter side is the width.
  Portrait,
  /// The longer side is the width.
  Landscape,
}

impl<'i> FromCss<'i> for PageOrientation {
  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Keyword("portrait"),
    CssToken::Keyword("landscape"),
  ];

  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let location = input.current_source_location();
    let token = input.next()?;
    let Token::Ident(ident) = token else {
      return Err(unexpected_token!(location, token));
    };

    match_ignore_ascii_case! {ident,
      "portrait" => Ok(Self::Portrait),
      "landscape" => Ok(Self::Landscape),
      _ => Err(unexpected_token!(location, token)),
    }
  }
}

/// The sheet part of a `size` value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PageSheet {
  /// A keyword such as `A4`.
  Named(PageSizeName),
  /// Explicit lengths; one length names a square.
  Lengths {
    /// The page width.
    width: Length,
    /// The page height.
    height: Length,
  },
}

/// A `size` descriptor value: `auto`, a sheet, an orientation, or a sheet
/// with an orientation.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PageSize {
  /// The sheet, or `None` to keep the size the renderer was given.
  pub sheet: Option<PageSheet>,
  /// The orientation, or `None` to keep the sheet's own.
  pub orientation: Option<PageOrientation>,
}

/// `size: auto | <length>{1,2} | [ <page-size> || [ portrait | landscape ] ]`.
impl<'i> FromCss<'i> for PageSize {
  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Keyword("auto"),
    CssToken::Keyword("portrait"),
    CssToken::Keyword("landscape"),
    CssToken::Syntax(crate::style::CssSyntaxKind::Length),
  ];

  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    if input
      .try_parse(|input| input.expect_ident_matching("auto"))
      .is_ok()
    {
      return Ok(Self::default());
    }
    if let Ok(width) = input.try_parse(parse_sheet_length) {
      let height = input.try_parse(parse_sheet_length).unwrap_or(width);

      return Ok(Self {
        sheet: Some(PageSheet::Lengths { width, height }),
        orientation: None,
      });
    }
    let mut size = Self::default();

    for _ in 0..2 {
      if size.orientation.is_none()
        && let Ok(orientation) = input.try_parse(PageOrientation::from_css)
      {
        size.orientation = Some(orientation);
      } else if size.sheet.is_none()
        && let Ok(name) = input.try_parse(PageSizeName::from_css)
      {
        size.sheet = Some(PageSheet::Named(name));
      } else {
        break;
      }
    }
    if size.sheet.is_none() && size.orientation.is_none() {
      let location = input.current_source_location();
      let token = input.next()?;

      return Err(unexpected_token!(location, token));
    }
    Ok(size)
  }
}

/// A length for `size`, which takes no `auto`, no percentage, and no
/// negative value.
fn parse_sheet_length<'i>(input: &mut Parser<'i, '_>) -> ParseResult<'i, Length> {
  let location = input.current_source_location();
  let start = input.state();

  if let Token::Dimension { value, .. } | Token::Number { value, .. } = input.next()?.clone()
    && value < 0.0
  {
    input.reset(&start);
    let token = input.next()?;

    return Err(unexpected_token!(PageSize, location, token));
  }
  input.reset(&start);
  match Length::from_css(input)? {
    Length::Auto => Err(location.new_unexpected_token_error(Token::Ident("auto".into()))),
    Length::Percentage(value) => Err(location.new_unexpected_token_error(Token::Percentage {
      has_sign: false,
      unit_value: value / 100.0,
      int_value: None,
    })),
    length => Ok(length),
  }
}

/// `margin: <length>{1,4}`, expanded the way the `margin` shorthand is.
fn parse_margin_shorthand<'i>(input: &mut Parser<'i, '_>) -> ParseResult<'i, Rect<Option<Length>>> {
  let mut values = Vec::with_capacity(4);

  while values.len() < 4
    && let Ok(length) = input.try_parse(Length::from_css)
  {
    values.push(length);
  }
  let (top, right, bottom, left) = match values.as_slice() {
    [all] => (*all, *all, *all, *all),
    [vertical, horizontal] => (*vertical, *horizontal, *vertical, *horizontal),
    [top, horizontal, bottom] => (*top, *horizontal, *bottom, *horizontal),
    [top, right, bottom, left] => (*top, *right, *bottom, *left),
    _ => {
      let location = input.current_source_location();
      let token = input.next()?;

      return Err(unexpected_token!(Length, location, token));
    }
  };

  Ok(Rect {
    top: Some(top),
    right: Some(right),
    bottom: Some(bottom),
    left: Some(left),
  })
}

/// The descriptors an `@page` rule sets, or the ones that win a cascade.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PageDescriptors {
  /// The `size` descriptor, when set.
  pub size: Option<PageSize>,
  /// The margin descriptors that are set, per side.
  pub margin: Rect<Option<Length>>,
}

impl PageDescriptors {
  /// The descriptors that win for a page: rules whose `@media` holds for
  /// `viewport` and whose selectors are empty or pass `selects`, applied in
  /// cascade order (importance, layer, specificity, source order).
  pub(crate) fn cascade<'r>(
    rules: impl IntoIterator<Item = &'r PageRule>,
    layer_count: usize,
    viewport: Viewport,
    selects: impl Fn(&PageSelector) -> bool,
  ) -> Self {
    let mut ranked: Vec<(u64, &PageDescriptors)> = Vec::new();

    for (order, rule) in rules.into_iter().enumerate() {
      let Some(specificity) = rule.specificity(viewport, &selects) else {
        continue;
      };
      let rank = |important: bool, layer: usize| {
        (u64::from(important) << 63)
          | ((layer as u64) << 40)
          | (u64::from(specificity) << 20)
          | order as u64
      };
      let normal = rank(false, rule.layer_order.unwrap_or(layer_count));
      let important = rank(
        true,
        rule.layer_order.map_or(0, |layer| layer_count - layer),
      );

      // Rules are few, so an insertion keeps the order without pulling in a
      // sort for this element type.
      for entry in [(normal, &rule.descriptors), (important, &rule.important)] {
        let at = ranked.partition_point(|&(rank, _)| rank <= entry.0);

        ranked.insert(at, entry);
      }
    }

    let mut winners = Self::default();

    for (_, descriptors) in ranked {
      winners.apply(descriptors);
    }
    winners
  }

  fn apply(&mut self, other: &Self) {
    if other.size.is_some() {
      self.size = other.size;
    }
    for (side, value) in [
      (&mut self.margin.top, other.margin.top),
      (&mut self.margin.right, other.margin.right),
      (&mut self.margin.bottom, other.margin.bottom),
      (&mut self.margin.left, other.margin.left),
    ] {
      if value.is_some() {
        *side = value;
      }
    }
  }

  fn set(&mut self, descriptor: PageDescriptor) {
    match descriptor {
      PageDescriptor::Size(size) => self.size = Some(size),
      PageDescriptor::Margin(margin) => self.apply(&Self { size: None, margin }),
    }
  }
}

/// One `@page` rule.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PageRule {
  /// The selectors this rule applies to; empty selects every page.
  pub selectors: Vec<PageSelector>,
  /// The descriptors without `!important`.
  pub descriptors: PageDescriptors,
  /// The descriptors with `!important`.
  pub important: PageDescriptors,
  pub(crate) media_queries: Vec<MediaQueryList>,
  pub(crate) layer: Option<LayerPath>,
  pub(crate) layer_order: Option<usize>,
}

impl PageRule {
  /// The specificity the rule matches a page with: that of its most specific
  /// selector passing `selects`, 0 for a bare `@page`, or `None` when no
  /// selector passes or an `@media` condition fails for `viewport`.
  pub fn specificity(
    &self,
    viewport: Viewport,
    selects: impl Fn(&PageSelector) -> bool,
  ) -> Option<u32> {
    if !self
      .media_queries
      .iter()
      .all(|queries| queries.matches(viewport))
    {
      return None;
    }
    if self.selectors.is_empty() {
      return Some(0);
    }
    self
      .selectors
      .iter()
      .filter(|selector| selects(selector))
      .map(PageSelector::specificity)
      .max()
  }

  /// Parses the block of `@page`. In lossy mode a descriptor that fails is
  /// dropped and the rest kept; otherwise the first failure is the error.
  pub(crate) fn parse_block<'i>(
    selectors: Vec<PageSelector>,
    layer: Option<LayerPath>,
    lossy: bool,
    input: &mut Parser<'i, '_>,
  ) -> Result<Self, ParseError<'i, StyleSheetParseError>> {
    let mut rule = Self {
      selectors,
      layer,
      ..Self::default()
    };

    for result in RuleBodyParser::new(input, &mut PageDescriptorParser) {
      match result {
        Ok((descriptor, false)) => rule.descriptors.set(descriptor),
        Ok((descriptor, true)) => rule.important.set(descriptor),
        Err((error, _)) if !lossy => return Err(error),
        Err(_) => {}
      }
    }
    Ok(rule)
  }
}

enum PageDescriptor {
  Size(PageSize),
  Margin(Rect<Option<Length>>),
}

struct PageDescriptorParser;

impl<'i> DeclarationParser<'i> for PageDescriptorParser {
  type Declaration = (PageDescriptor, bool);
  type Error = StyleSheetParseError;

  fn parse_value<'t>(
    &mut self,
    name: CowRcStr<'i>,
    input: &mut Parser<'i, 't>,
    _state: &ParserState,
  ) -> Result<Self::Declaration, ParseError<'i, Self::Error>> {
    let side = |input: &mut Parser<'i, 't>| Length::from_css(input).map(Some);
    let descriptor = match_ignore_ascii_case! {&name,
      "size" => PageSize::from_css(input).map(PageDescriptor::Size),
      "margin" => parse_margin_shorthand(input).map(PageDescriptor::Margin),
      "margin-top" => side(input).map(|top| PageDescriptor::Margin(Rect { top, ..Rect::default() })),
      "margin-right" => side(input).map(|right| PageDescriptor::Margin(Rect { right, ..Rect::default() })),
      "margin-bottom" => side(input).map(|bottom| PageDescriptor::Margin(Rect { bottom, ..Rect::default() })),
      "margin-left" => side(input).map(|left| PageDescriptor::Margin(Rect { left, ..Rect::default() })),
      _ => {
        return Err(input.new_custom_error(StyleSheetParseError::invalid_reason(format!(
          "unsupported @page descriptor `{name}`"
        ))));
      }
    }
    .map_err(ParseError::into)?;
    let important = input.try_parse(parse_important).is_ok();

    input.expect_exhausted()?;
    Ok((descriptor, important))
  }
}

impl<'i> QualifiedRuleParser<'i> for PageDescriptorParser {
  type Prelude = ();
  type QualifiedRule = (PageDescriptor, bool);
  type Error = StyleSheetParseError;
}

impl<'i> AtRuleParser<'i> for PageDescriptorParser {
  type Prelude = ();
  type AtRule = (PageDescriptor, bool);
  type Error = StyleSheetParseError;
}

impl<'i> RuleBodyItemParser<'i, (PageDescriptor, bool), StyleSheetParseError>
  for PageDescriptorParser
{
  fn parse_qualified(&self) -> bool {
    false
  }

  fn parse_declarations(&self) -> bool {
    true
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::{
    style::StyleSheet,
    viewport::{MediaTarget, Viewport},
  };

  fn page_rules(css: &str) -> Vec<PageRule> {
    let sheet = StyleSheet::parse(css).expect("parse @page");

    sheet.page_rules().to_vec()
  }

  fn winners(css: &str) -> PageDescriptors {
    let sheet = StyleSheet::parse(css).expect("parse @page");
    let print = Viewport::default().with_media_target(MediaTarget::Print);

    sheet.page_descriptors(print, PageSelector::is_universal)
  }

  #[test]
  fn a_universal_page_rule_reads_size_and_margin() {
    let rules = page_rules("@page { size: A4 landscape; margin: 1in 2cm; margin-top: auto }");

    assert_eq!(rules.len(), 1);
    assert!(rules[0].selectors.is_empty());
    assert_eq!(
      rules[0].descriptors,
      PageDescriptors {
        size: Some(PageSize {
          sheet: Some(PageSheet::Named(PageSizeName::A4)),
          orientation: Some(PageOrientation::Landscape),
        }),
        margin: Rect {
          top: Some(Length::Auto),
          right: Some(Length::Cm(2.0)),
          bottom: Some(Length::In(1.0)),
          left: Some(Length::Cm(2.0)),
        },
      }
    );
  }

  #[test]
  fn size_accepts_lengths_and_a_lone_orientation() {
    let rules =
      page_rules("@page { size: 10cm 20cm } @page { size: landscape } @page { size: 8in }");

    assert_eq!(
      rules[0].descriptors.size,
      Some(PageSize {
        sheet: Some(PageSheet::Lengths {
          width: Length::Cm(10.0),
          height: Length::Cm(20.0),
        }),
        orientation: None,
      })
    );
    assert_eq!(
      rules[1].descriptors.size,
      Some(PageSize {
        sheet: None,
        orientation: Some(PageOrientation::Landscape),
      })
    );
    assert_eq!(
      rules[2].descriptors.size,
      Some(PageSize {
        sheet: Some(PageSheet::Lengths {
          width: Length::In(8.0),
          height: Length::In(8.0),
        }),
        orientation: None,
      })
    );
  }

  #[test]
  fn page_selectors_read_names_and_pseudo_classes() {
    let rules = page_rules("@page cover:first, :left { margin: 0 }");
    let print = Viewport::default().with_media_target(MediaTarget::Print);

    assert_eq!(
      rules[0].selectors,
      vec![
        PageSelector {
          name: Some("cover".into()),
          pseudo_classes: vec![PagePseudoClass::First],
        },
        PageSelector {
          name: None,
          pseudo_classes: vec![PagePseudoClass::Left],
        },
      ]
    );
    assert_eq!(rules[0].selectors[0].specificity(), (1 << 16) | (1 << 8));
    assert_eq!(rules[0].selectors[1].specificity(), 1);
    assert_eq!(
      rules[0].specificity(print, PageSelector::is_universal),
      None
    );
    assert_eq!(
      rules[0].specificity(print, |_| true),
      Some((1 << 16) | (1 << 8))
    );
  }

  #[test]
  fn a_media_query_gates_the_rule() {
    let winners =
      winners("@media print { @page { margin: 1mm } } @media screen { @page { margin: 1cm } }");

    assert_eq!(winners.margin.top, Some(Length::Mm(1.0)));
  }

  #[test]
  fn the_cascade_orders_importance_then_layer_then_source() {
    assert_eq!(
      winners("@page { margin: 1mm !important } @page { margin: 2mm }")
        .margin
        .top,
      Some(Length::Mm(1.0))
    );
    assert_eq!(
      winners("@layer base { @page { margin: 1mm } } @page { margin: 2mm } @layer base { @page { margin: 3mm } }")
        .margin
        .top,
      Some(Length::Mm(2.0))
    );
    assert_eq!(
      winners("@layer base { @page { margin: 1mm !important } } @page { margin: 2mm !important }")
        .margin
        .top,
      Some(Length::Mm(1.0))
    );
    assert_eq!(
      winners("@page { size: A5; margin-top: 1mm } @page { size: auto }"),
      PageDescriptors {
        size: Some(PageSize::default()),
        margin: Rect {
          top: Some(Length::Mm(1.0)),
          ..Rect::default()
        },
      }
    );
  }

  #[test]
  fn an_unknown_descriptor_fails_strict_parsing_and_is_dropped_loosely() {
    assert!(StyleSheet::parse("@page { color: red; margin: 0 }").is_err());

    let sheet = StyleSheet::parse_loosy("@page { color: red; margin: 0 }");

    assert_eq!(
      sheet.page_rules()[0].descriptors.margin.top,
      Some(Length::Px(0.0))
    );
  }

  #[test]
  fn an_invalid_prelude_or_size_is_an_error() {
    assert!(StyleSheet::parse("@page :middle { margin: 0 }").is_err());
    assert!(StyleSheet::parse("@page { size: 50% }").is_err());
    assert!(StyleSheet::parse("@page { size: -1px }").is_err());
    assert!(StyleSheet::parse("@page { size: 10cm -1px }").is_err());
    assert!(StyleSheet::parse("@page { size: A4 letter }").is_err());
    assert!(StyleSheet::parse(".a { @page { margin: 0 } }").is_err());
  }
}
