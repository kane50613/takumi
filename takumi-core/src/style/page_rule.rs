//! `@page` rules as Blink's [`ParsePageSelector`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/css/parser/css_parser_impl.cc)
//! reads them, and the descriptors that win for a page, as `PageRuleCollector` matches them.
//! Only `size` and the margin descriptors are read.

use cssparser::{Parser, Token, match_ignore_ascii_case};

use crate::{
  geometry::{Rect, Size},
  style::{
    CustomProperties, Direction, FromCss, Length, MediaQueryList, PageName, RawDeclaration,
    RawDeclarationParser, SizingContext, contains_var_references, resolve_var_references,
    selector::LayerPath,
  },
  units::{ONE_IN_PX, ONE_MM_IN_PX},
  viewport::Viewport,
};

/// One of the pages Blink's `PageRuleCollector` tells apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageKind {
  /// The first page, a left page under `direction: rtl` and a right page otherwise.
  First,
  /// A left page after the first.
  Left,
  /// A right page after the first.
  Right,
}

impl PageKind {
  /// The kind of page `page`, counted from 0, as Blink's `IsFirstPage` and `IsLeftPage` say under
  /// the root's `direction`.
  pub fn of(page: usize, direction: Direction) -> Self {
    let first_is_left = usize::from(direction == Direction::Rtl);

    if page == 0 {
      Self::First
    } else if (page + first_is_left) % 2 == 1 {
      Self::Left
    } else {
      Self::Right
    }
  }

  /// Whether this kind of page is a left page under `direction`.
  fn is_left(self, direction: Direction) -> bool {
    match self {
      Self::First => direction == Direction::Rtl,
      Self::Left => true,
      Self::Right => false,
    }
  }
}

/// What a page's style inherits from the root element, as Blink's `StyleForPage` takes it: the
/// root's `direction`, font sizes, and custom properties, or the initial style without a root.
#[derive(Clone)]
pub struct PageContext {
  direction: Direction,
  sizing: SizingContext,
  custom_properties: CustomProperties,
}

impl Default for PageContext {
  fn default() -> Self {
    Self {
      direction: Direction::default(),
      sizing: SizingContext::builder()
        .viewport(Viewport::default())
        .build(),
      custom_properties: CustomProperties::default(),
    }
  }
}

impl PageContext {
  /// The context a root element with `direction`, `sizing`, and `custom_properties` gives.
  pub(crate) fn new(
    direction: Direction,
    sizing: SizingContext,
    custom_properties: CustomProperties,
  ) -> Self {
    Self {
      direction,
      sizing,
      custom_properties,
    }
  }

  /// The root's `direction`, which decides which pages are left pages.
  pub fn direction(&self) -> Direction {
    self.direction
  }

  /// What a page's lengths resolve against at `viewport`: the root's font sizes for `em` and
  /// `rem`.
  pub fn sizing(&self, viewport: Viewport) -> SizingContext {
    SizingContext {
      viewport: viewport.with_font_size(self.sizing.viewport.font_size),
      ..self.sizing.clone()
    }
  }
}

/// A page pseudo-class, the one Blink's `ParsePageSelector` takes after a page name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PagePseudoClass {
  /// `:first`.
  First,
  /// `:left`.
  Left,
  /// `:right`.
  Right,
}

/// An `@page` selector: an optional page name and an optional pseudo-class.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct PageSelector {
  /// The page name, which the `page` property gives the pages an element starts.
  name: PageName,
  pseudo_class: Option<PagePseudoClass>,
}

impl PageSelector {
  /// Whether the selector applies to `kind` pages named `name` under the root's `direction`, as
  /// Blink's `CheckPageSelectorComponents` says.
  pub(crate) fn matches(&self, kind: PageKind, name: &PageName, direction: Direction) -> bool {
    (!self.name.is_named() || self.name == *name)
      && match self.pseudo_class {
        None => true,
        Some(PagePseudoClass::First) => kind == PageKind::First,
        Some(PagePseudoClass::Left) => kind.is_left(direction),
        Some(PagePseudoClass::Right) => !kind.is_left(direction),
      }
  }

  /// The page name the selector selects, `auto` for any.
  pub(crate) fn name(&self) -> &PageName {
    &self.name
  }

  /// Blink's `CSSSelector::Specificity` for a page selector.
  fn specificity(&self) -> u32 {
    let pseudo_class = match self.pseudo_class {
      None => 0,
      Some(PagePseudoClass::First) => 2,
      Some(PagePseudoClass::Left | PagePseudoClass::Right) => 1,
    };

    u32::from(self.name.is_named()) * 4 + pseudo_class
  }

  /// Blink's `ParsePageSelector`: a page name, a pseudo-class, both, or nothing.
  pub(crate) fn parse(input: &mut Parser) -> Option<Self> {
    let name = input
      .try_parse(|input| input.expect_ident().map(|name| PageName::named(name)))
      .unwrap_or_default();
    let pseudo_class = if input.try_parse(Parser::expect_colon).is_ok() {
      let ident = input.expect_ident().ok()?.clone();

      Some(match_ignore_ascii_case! { &ident,
        "first" => PagePseudoClass::First,
        "left" => PagePseudoClass::Left,
        "right" => PagePseudoClass::Right,
        _ => return None,
      })
    } else {
      None
    };

    input.expect_exhausted().ok()?;

    Some(Self { name, pseudo_class })
  }
}

/// A page size keyword, portrait as css-page-3 writes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PageSizeName {
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

impl PageSizeName {
  /// The sheet's portrait size in px.
  const fn size(self) -> Size<f32> {
    let (width, height) = match self {
      Self::A3 => (297.0 * ONE_MM_IN_PX, 420.0 * ONE_MM_IN_PX),
      Self::A4 => (210.0 * ONE_MM_IN_PX, 297.0 * ONE_MM_IN_PX),
      Self::A5 => (148.0 * ONE_MM_IN_PX, 210.0 * ONE_MM_IN_PX),
      Self::B4 => (250.0 * ONE_MM_IN_PX, 353.0 * ONE_MM_IN_PX),
      Self::B5 => (176.0 * ONE_MM_IN_PX, 250.0 * ONE_MM_IN_PX),
      Self::JisB4 => (257.0 * ONE_MM_IN_PX, 364.0 * ONE_MM_IN_PX),
      Self::JisB5 => (182.0 * ONE_MM_IN_PX, 257.0 * ONE_MM_IN_PX),
      Self::Ledger => (11.0 * ONE_IN_PX, 17.0 * ONE_IN_PX),
      Self::Legal => (8.5 * ONE_IN_PX, 14.0 * ONE_IN_PX),
      Self::Letter => (8.5 * ONE_IN_PX, 11.0 * ONE_IN_PX),
    };

    Size { width, height }
  }
}

/// The orientation keyword in a `size` value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PageOrientation {
  /// The shorter side is the width.
  Portrait,
  /// The longer side is the width.
  Landscape,
}

/// The sheet part of a `size` value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum PageSheet {
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

/// A `size` value: `auto`, a sheet, an orientation, or a sheet with an orientation.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PageSize {
  /// The sheet, or `None` to keep the size the renderer was given.
  sheet: Option<PageSheet>,
  /// The orientation, or `None` to keep the sheet's own.
  orientation: Option<PageOrientation>,
}

impl PageSize {
  /// The page this value gives a `base` page: the sheet it names, or `base`, turned to the
  /// orientation it asks for.
  pub fn resolve(self, base: Size<f32>, sizing: &SizingContext) -> Size<f32> {
    let Size { width, height } = match self.sheet {
      None => base,
      Some(PageSheet::Named(name)) => name.size(),
      Some(PageSheet::Lengths { width, height }) => Size {
        width: width.to_px(sizing, 0.0),
        height: height.to_px(sizing, 0.0),
      },
    };
    let (width, height) = match self.orientation {
      Some(PageOrientation::Landscape) if width < height => (height, width),
      Some(PageOrientation::Portrait) if width > height => (height, width),
      _ => (width, height),
    };

    Size { width, height }
  }

  /// `auto | <length [0,∞]>{1,2} | [ <page-size> || [ portrait | landscape ] ]`.
  fn parse(input: &mut Parser) -> Option<Self> {
    if input
      .try_parse(|input| input.expect_ident_matching("auto"))
      .is_ok()
    {
      return Some(Self::default());
    }

    if let Ok(width) = input.try_parse(sheet_length) {
      let height = input.try_parse(sheet_length).unwrap_or(width);

      return Some(Self {
        sheet: Some(PageSheet::Lengths { width, height }),
        orientation: None,
      });
    }

    let mut size = Self::default();

    while let Ok(ident) = input.try_parse(|input| input.expect_ident().cloned()) {
      let orientation = match_ignore_ascii_case! { &ident,
        "portrait" => Some(PageOrientation::Portrait),
        "landscape" => Some(PageOrientation::Landscape),
        _ => None,
      };

      match orientation {
        Some(orientation) if size.orientation.is_none() => size.orientation = Some(orientation),
        None if size.sheet.is_none() => {
          size.sheet = Some(PageSheet::Named(page_size_name(&ident)?))
        }
        _ => return None,
      }
    }

    (size.sheet.is_some() || size.orientation.is_some()).then_some(size)
  }
}

fn page_size_name(ident: &str) -> Option<PageSizeName> {
  Some(match_ignore_ascii_case! { ident,
    "a3" => PageSizeName::A3,
    "a4" => PageSizeName::A4,
    "a5" => PageSizeName::A5,
    "b4" => PageSizeName::B4,
    "b5" => PageSizeName::B5,
    "jis-b4" => PageSizeName::JisB4,
    "jis-b5" => PageSizeName::JisB5,
    "ledger" => PageSizeName::Ledger,
    "legal" => PageSizeName::Legal,
    "letter" => PageSizeName::Letter,
    _ => return None,
  })
}

/// A length for `size`, which takes no `auto`, no percentage, and no negative value.
fn sheet_length(input: &mut Parser) -> Result<Length, ()> {
  let start = input.state();
  let negative = matches!(
    input.next(),
    Ok(Token::Dimension { value, .. } | Token::Number { value, .. }) if *value < 0.0
  );

  input.reset(&start);

  match Length::from_css(input) {
    Ok(Length::Auto) | Err(_) => Err(()),
    Ok(length) if negative || length.has_percentage() => Err(()),
    Ok(length) => Ok(length),
  }
}

/// `margin: <length>{1,4}`, expanded as the `margin` shorthand is.
fn margin_shorthand(input: &mut Parser) -> Option<Rect<Option<Length>>> {
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
    _ => return None,
  };

  Some(Rect {
    top: Some(top),
    right: Some(right),
    bottom: Some(bottom),
    left: Some(left),
  })
}

/// The descriptors an `@page` rule sets, or those that win the cascade.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PageDescriptors {
  /// The `size` descriptor.
  pub size: Option<PageSize>,
  /// The margin descriptors, per side.
  pub margin: Rect<Option<Length>>,
}

impl PageDescriptors {
  /// Overrides each descriptor `other` sets.
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

  /// The descriptors one declaration sets, or `None` for one Blink's `ParsePageDescriptor` drops.
  fn parse(name: &str, value: &str) -> Option<Self> {
    let input = &mut Parser::new(value);
    let side = |input: &mut Parser| input.try_parse(Length::from_css).ok().map(Some);
    let margin = |margin| Self { size: None, margin };
    let parsed = match_ignore_ascii_case! { name,
      "size" => PageSize::parse(input).map(|size| Self { size: Some(size), ..Self::default() }),
      "margin" => margin_shorthand(input).map(margin),
      "margin-top" => side(input).map(|top| margin(Rect { top, ..Rect::default() })),
      "margin-right" => side(input).map(|right| margin(Rect { right, ..Rect::default() })),
      "margin-bottom" => side(input).map(|bottom| margin(Rect { bottom, ..Rect::default() })),
      "margin-left" => side(input).map(|left| margin(Rect { left, ..Rect::default() })),
      _ => None,
    }?;

    input.is_exhausted().then_some(parsed)
  }

  /// The descriptors `name` sets when its `var()` substitution fails: invalid at computed-value
  /// time, the declaration is `unset`, which gives `size` and each margin its initial value.
  fn unset(name: &str) -> Option<Self> {
    let initial = if name.eq_ignore_ascii_case("size") {
      "auto"
    } else {
      "0"
    };

    Self::parse(name, initial)
  }
}

/// A declaration in an `@page` rule: its descriptors, or, holding a `var()`, its text until the
/// page context's custom properties substitute it, as Blink's pending substitution value.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum PageDeclaration {
  Parsed(PageDescriptors),
  Pending { name: String, value: String },
}

impl PageDeclaration {
  /// The declaration `name: value`, or `None` for one Blink drops.
  fn parse(name: &str, value: &str) -> Option<Self> {
    if contains_var_references(value) {
      return PageDescriptors::unset(name).map(|_| Self::Pending {
        name: name.to_owned(),
        value: value.to_owned(),
      });
    }

    PageDescriptors::parse(name, value).map(Self::Parsed)
  }

  /// The descriptors this declaration sets in a page context with `custom_properties`.
  fn descriptors(&self, custom_properties: &CustomProperties) -> PageDescriptors {
    match self {
      Self::Parsed(descriptors) => descriptors.clone(),
      Self::Pending { name, value } => resolve_var_references(value, custom_properties)
        .and_then(|value| PageDescriptors::parse(name, &value))
        .or_else(|| PageDescriptors::unset(name))
        .unwrap_or_default(),
    }
  }
}

/// One `@page` rule.
#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct PageRule {
  /// The selector, universal for a bare `@page`.
  pub(crate) selector: PageSelector,
  /// The declarations without `!important`, in source order.
  pub(crate) normal: Vec<PageDeclaration>,
  /// The declarations with `!important`, in source order.
  pub(crate) important: Vec<PageDeclaration>,
  /// Media queries gating this rule.
  pub(crate) media_queries: Vec<MediaQueryList>,
  /// Cascade layer this rule belongs to.
  pub(crate) layer: Option<LayerPath>,
  /// Resolved ordinal of the rule's layer.
  pub(crate) layer_order: Option<usize>,
}

impl PageRule {
  /// The rule `@page <selector> { ... }` makes.
  pub(crate) fn parse(selector: PageSelector, input: &mut Parser) -> Self {
    let mut rule = Self {
      selector,
      ..Self::default()
    };

    for declaration in cssparser::RuleBodyParser::new(input, &mut RawDeclarationParser) {
      let Ok(RawDeclaration {
        name,
        value,
        important,
      }) = declaration
      else {
        continue;
      };
      let declarations = if important {
        &mut rule.important
      } else {
        &mut rule.normal
      };

      declarations.extend(PageDeclaration::parse(&name, value));
    }

    rule
  }

  /// The descriptors that win among `rules` whose `@media` holds at `viewport` and whose selector
  /// matches `kind` pages named `name` in `context`, in cascade order: importance, layer,
  /// specificity, then source order.
  pub(crate) fn cascade<'r>(
    rules: impl IntoIterator<Item = &'r Self>,
    layer_count: usize,
    viewport: Viewport,
    kind: PageKind,
    name: &PageName,
    context: &PageContext,
  ) -> PageDescriptors {
    let mut ranked: Vec<(u64, &[PageDeclaration])> = Vec::new();

    for (order, rule) in rules.into_iter().enumerate() {
      if !rule.selector.matches(kind, name, context.direction)
        || !rule
          .media_queries
          .iter()
          .all(|queries| queries.matches(viewport))
      {
        continue;
      }

      let rank = |important: bool, layer: usize| {
        (u64::from(important) << 63)
          | ((layer as u64) << 40)
          | (u64::from(rule.selector.specificity()) << 20)
          | order as u64
      };

      ranked.push((
        rank(false, rule.layer_order.unwrap_or(layer_count)),
        &rule.normal,
      ));
      ranked.push((
        rank(
          true,
          rule.layer_order.map_or(0, |layer| layer_count - layer),
        ),
        &rule.important,
      ));
    }

    ranked.sort_by_key(|&(rank, _)| rank);

    let mut winners = PageDescriptors::default();

    for declaration in ranked
      .into_iter()
      .flat_map(|(_, declarations)| declarations)
    {
      winners.apply(&declaration.descriptors(&context.custom_properties));
    }

    winners
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::{
    style::StyleSheet,
    viewport::{MediaTarget, Viewport},
  };

  fn winners(css: &str, kind: PageKind) -> PageDescriptors {
    winners_in(css, kind, &PageContext::default())
  }

  fn winners_in(css: &str, kind: PageKind, context: &PageContext) -> PageDescriptors {
    let print = Viewport::default().with_media_target(MediaTarget::Print);

    StyleSheet::parse(css)
      .expect("parse @page")
      .page_descriptors(print, kind, &PageName::default(), context)
  }

  fn context_with(direction: Direction, custom_properties: &[(&str, &str)]) -> PageContext {
    let mut properties = CustomProperties::default();

    for (name, value) in custom_properties {
      properties.set((*name).to_owned(), (*value).to_owned());
    }

    PageContext::new(
      direction,
      SizingContext::builder()
        .viewport(Viewport::default())
        .build(),
      properties,
    )
  }

  #[test]
  fn a_page_rule_reads_size_and_margin() {
    assert_eq!(
      winners(
        "@page { size: A4 landscape; margin: 1in 2cm; margin-top: auto }",
        PageKind::Right
      ),
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
  fn size_takes_lengths_a_keyword_and_an_orientation() {
    let size = |css: &str| winners(&format!("@page {{ size: {css} }}"), PageKind::Right).size;

    assert_eq!(
      size("10cm 20cm"),
      Some(PageSize {
        sheet: Some(PageSheet::Lengths {
          width: Length::Cm(10.0),
          height: Length::Cm(20.0),
        }),
        orientation: None,
      })
    );
    assert_eq!(
      size("landscape letter").map(|size| (size.sheet, size.orientation)),
      Some((
        Some(PageSheet::Named(PageSizeName::Letter)),
        Some(PageOrientation::Landscape)
      ))
    );
    assert_eq!(size("auto"), Some(PageSize::default()));

    for invalid in [
      "50%",
      "calc(50% + 10px)",
      "-1px",
      "10cm -1px",
      "A4 letter",
      "portrait landscape",
    ] {
      assert_eq!(size(invalid), None, "{invalid}");
    }
  }

  #[test]
  fn pseudo_classes_select_the_first_left_and_right_pages() {
    let css = "@page { margin: 1px } @page :right { margin: 2px } @page :left { margin: 3px } @page :first { margin: 4px }";
    let top = |kind| winners(css, kind).margin.top;

    assert_eq!(top(PageKind::First), Some(Length::Px(4.0)));
    assert_eq!(top(PageKind::Left), Some(Length::Px(3.0)));
    assert_eq!(top(PageKind::Right), Some(Length::Px(2.0)));
  }

  #[test]
  fn the_first_page_is_a_right_page_unless_the_root_is_rtl() {
    let css = "@page :left { margin: 1px } @page :right { margin: 2px }";
    let first = |direction| {
      winners_in(css, PageKind::First, &context_with(direction, &[]))
        .margin
        .top
    };

    assert_eq!(first(Direction::Ltr), Some(Length::Px(2.0)));
    assert_eq!(first(Direction::Rtl), Some(Length::Px(1.0)));
  }

  #[test]
  fn pages_alternate_from_the_side_the_direction_starts_on() {
    let kinds = |direction| {
      (0..4)
        .map(|page| PageKind::of(page, direction))
        .collect::<Vec<_>>()
    };

    assert_eq!(
      kinds(Direction::Ltr),
      [
        PageKind::First,
        PageKind::Left,
        PageKind::Right,
        PageKind::Left
      ]
    );
    assert_eq!(
      kinds(Direction::Rtl),
      [
        PageKind::First,
        PageKind::Right,
        PageKind::Left,
        PageKind::Right
      ]
    );
  }

  #[test]
  fn a_var_takes_the_root_custom_property() {
    let context = context_with(Direction::Ltr, &[("--m", "3px"), ("--size", "A5")]);
    let descriptors = winners_in(
      "@page { size: var(--size); margin: var(--m) 1px }",
      PageKind::Right,
      &context,
    );

    assert_eq!(
      descriptors.size,
      Some(PageSize {
        sheet: Some(PageSheet::Named(PageSizeName::A5)),
        orientation: None,
      })
    );
    assert_eq!(descriptors.margin.top, Some(Length::Px(3.0)));
    assert_eq!(descriptors.margin.right, Some(Length::Px(1.0)));
  }

  #[test]
  fn a_var_that_fails_unsets_its_descriptor_over_earlier_rules() {
    let context = context_with(Direction::Ltr, &[("--m", "red")]);
    let descriptors = winners_in(
      "@page { size: A4; margin: 2px } @page { size: var(--missing); margin-top: var(--m) }",
      PageKind::Right,
      &context,
    );

    assert_eq!(descriptors.size, Some(PageSize::default()));
    assert_eq!(descriptors.margin.top, Some(Length::Px(0.0)));
    assert_eq!(descriptors.margin.left, Some(Length::Px(2.0)));
  }

  #[test]
  fn a_var_in_an_unknown_descriptor_is_dropped() {
    assert_eq!(
      winners("@page { color: var(--c) }", PageKind::Right),
      PageDescriptors::default()
    );
  }

  #[test]
  fn a_named_selector_selects_the_pages_of_that_name() {
    let sheet = StyleSheet::parse(
      "@page { margin: 1px } @page cover { margin: 2px } @page cover:first { margin: 3px }",
    )
    .expect("parse @page");
    let print = Viewport::default().with_media_target(MediaTarget::Print);
    let top = |kind, name: &PageName| {
      sheet
        .page_descriptors(print, kind, name, &PageContext::default())
        .margin
        .top
    };
    let cover = PageName::named("cover");

    assert_eq!(top(PageKind::First, &cover), Some(Length::Px(3.0)));
    assert_eq!(top(PageKind::Left, &cover), Some(Length::Px(2.0)));
    assert_eq!(
      top(PageKind::Left, &PageName::named("index")),
      Some(Length::Px(1.0))
    );
    assert_eq!(
      top(PageKind::Left, &PageName::default()),
      Some(Length::Px(1.0))
    );
    assert_eq!(sheet.page_names(), [cover]);
  }

  #[test]
  fn a_named_page_or_an_unknown_selector_selects_nothing_on_an_unnamed_page() {
    for css in [
      "@page cover { margin: 1px }",
      "@page :blank { margin: 1px }",
      "@page :first, :left { margin: 1px }",
      "@page :first:left { margin: 1px }",
    ] {
      let sheet = StyleSheet::parse(css).expect("a rule Blink drops leaves the sheet valid");
      let print = Viewport::default().with_media_target(MediaTarget::Print);

      assert_eq!(
        sheet
          .page_descriptors(
            print,
            PageKind::First,
            &PageName::default(),
            &PageContext::default()
          )
          .margin
          .top,
        None,
        "{css}"
      );
    }
  }

  #[test]
  fn a_media_query_gates_the_rule() {
    assert_eq!(
      winners(
        "@media print { @page { margin: 1mm } } @media screen { @page { margin: 1cm } }",
        PageKind::Right
      )
      .margin
      .top,
      Some(Length::Mm(1.0))
    );
  }

  #[test]
  fn the_cascade_orders_importance_then_layer_then_specificity_then_source() {
    let top = |css| winners(css, PageKind::First).margin.top;

    assert_eq!(
      top("@page { margin: 1mm !important } @page { margin: 2mm }"),
      Some(Length::Mm(1.0))
    );
    assert_eq!(
      top(
        "@layer base { @page { margin: 1mm } } @page { margin: 2mm } @layer base { @page { margin: 3mm } }"
      ),
      Some(Length::Mm(2.0))
    );
    assert_eq!(
      top("@layer base { @page { margin: 1mm !important } } @page { margin: 2mm !important }"),
      Some(Length::Mm(1.0))
    );
    assert_eq!(
      top("@page :first { margin: 1mm } @page :right { margin: 2mm }"),
      Some(Length::Mm(1.0))
    );
  }

  #[test]
  fn an_invalid_descriptor_is_dropped_and_the_rest_kept() {
    assert_eq!(
      winners(
        "@page { color: red; margin: 0; margin-left: 1px 2px }",
        PageKind::Right
      )
      .margin,
      Rect {
        top: Some(Length::Px(0.0)),
        right: Some(Length::Px(0.0)),
        bottom: Some(Length::Px(0.0)),
        left: Some(Length::Px(0.0)),
      }
    );
  }
}
