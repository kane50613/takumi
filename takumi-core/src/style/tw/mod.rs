mod builder;
/// Utility-prefix to property-parser mapping.
mod map;
/// Parsers for Tailwind utility-class suffixes.
mod parser;
/// Design tokens overriding the built-in utility scales.
mod theme;

use std::{borrow::Cow, cmp::Ordering, str::FromStr};

use builder::TailwindDeclarationBuilder;
pub(crate) use builder::TwGradientType;
use cssparser::match_ignore_ascii_case;
use serde::{Deserialize, Deserializer, de::Error as DeError};
pub(crate) use theme::ThemeLookup;
pub use theme::{Theme, TwNamespace};

use crate::{
  style::{
    tw::{
      map::{FIXED_PROPERTIES, PREFIX_PARSERS},
      parser::*,
    },
    *,
  },
  viewport::Viewport,
};

/// Tailwind v4 `--spacing` (rem per unit). Prefer [`Length::from_spacing`].
pub(crate) const TW_VAR_SPACING: f32 = 0.25;

/// Represents a collection of tailwind properties.
///
/// `inner` holds the built-in resolution, so a render with no theme never
/// re-parses. `source` is kept because a theme can give meaning to a token the
/// built-in scales reject, which `inner` cannot represent.
#[derive(Debug, Clone, PartialEq)]
pub struct TailwindValues {
  source: Box<str>,
  inner: Vec<TailwindValue>,
}

fn parse_values(source: &str, theme: &Theme) -> Vec<TailwindValue> {
  let mut collected = source
    .split_whitespace()
    .filter_map(|token| TailwindValue::parse(token, theme))
    .collect::<Vec<_>>();

  // sort in reverse order by is important, then has breakpoint, then rest is last.
  // Stable sort so equal-priority utilities keep source order (later one wins).
  collected.sort_by(|a, b| {
    // Not important comes before important
    if !a.important && b.important {
      return Ordering::Less;
    }

    if a.important && !b.important {
      return Ordering::Greater;
    }

    // No breakpoint comes before breakpoint
    match (&a.breakpoint, &b.breakpoint) {
      (None, Some(_)) => Ordering::Less,
      (Some(_), None) => Ordering::Greater,
      _ => Ordering::Equal,
    }
  });

  collected
}

impl FromStr for TailwindValues {
  type Err = String;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    Ok(TailwindValues {
      source: s.into(),
      inner: parse_values(s, &Theme::default()),
    })
  }
}

impl TailwindValues {
  /// Collects resource URLs referenced by active Tailwind utilities for the given viewport.
  pub(crate) fn image_urls(&self, viewport: Viewport) -> impl Iterator<Item = &str> {
    self
      .inner
      .iter()
      .filter_map(move |value| value.resource_url(viewport))
  }

  /// Resolves all utilities for the viewport into a declaration block.
  #[inline(never)]
  pub(crate) fn into_declaration_block(
    self,
    viewport: Viewport,
    theme: &Theme,
  ) -> StyleDeclarationBlock {
    let mut builder = TailwindDeclarationBuilder::default();

    let values = if theme.is_empty() {
      self.inner
    } else {
      parse_values(&self.source, theme)
    };

    for value in values {
      value.apply(&mut builder, viewport);
    }

    builder.finish()
  }
}

impl<'de> Deserialize<'de> for TailwindValues {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    let string = String::deserialize(deserializer)?;

    TailwindValues::from_str(&string).map_err(D::Error::custom)
  }
}

/// Represents a tailwind value.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TailwindValue {
  /// The tailwind property.
  pub property: TailwindProperty,
  /// The breakpoint.
  pub breakpoint: Option<Breakpoint>,
  /// Whether the value is important.
  pub important: bool,
}

/// Splits a token at the first top-level variant `:`, mirroring Tailwind's
/// `segment`: a `:` nested in brackets, quotes, or escaped by `\` is not a
/// separator, so `url('https://…')` stays intact.
fn split_variant(token: &str) -> Option<(&str, &str)> {
  let bytes = token.as_bytes();
  let mut stack: Vec<u8> = Vec::new();
  let mut index = 0;
  while index < bytes.len() {
    match bytes[index] {
      b'\\' => index += 1, // skip the escaped character
      quote @ (b'\'' | b'"') => {
        index += 1;
        while index < bytes.len() && bytes[index] != quote {
          index += if bytes[index] == b'\\' { 2 } else { 1 };
        }
      }
      b'(' => stack.push(b')'),
      b'[' => stack.push(b']'),
      b'{' => stack.push(b'}'),
      closing @ (b')' | b']' | b'}') => {
        if stack.last() == Some(&closing) {
          stack.pop();
        }
      }
      b':' if stack.is_empty() => return Some((&token[..index], &token[index + 1..])),
      _ => {}
    }
    index += 1;
  }
  None
}

impl TailwindValue {
  fn resource_url(&self, viewport: Viewport) -> Option<&str> {
    if let Some(breakpoint) = self.breakpoint
      && !breakpoint.matches(viewport)
    {
      return None;
    }

    self.property.resource_url()
  }

  #[inline(never)]
  fn apply(self, builder: &mut TailwindDeclarationBuilder, viewport: Viewport) {
    if let Some(breakpoint) = self.breakpoint
      && !breakpoint.matches(viewport)
    {
      return;
    }

    self.property.apply(builder, self.important);
  }

  /// Parse a tailwind value from a token.
  pub fn parse(mut token: &str, theme: &Theme) -> Option<Self> {
    let mut important = false;
    let mut breakpoint = None;

    // Breakpoint. sm:mt-0
    if let Some((breakpoint_token, rest)) = split_variant(token) {
      breakpoint = Some(Breakpoint::parse(breakpoint_token)?);
      token = rest;
    }

    // Check for important flag. !mt-0
    if let Some(stripped) = token.strip_prefix('!') {
      important = true;
      token = stripped;
    }

    // Check for important flag. mt-0!
    if let Some(stripped) = token.strip_suffix('!') {
      important = true;
      token = stripped;
    }

    Some(TailwindValue {
      property: TailwindProperty::parse(token, theme)?,
      breakpoint,
      important,
    })
  }
}

/// Represents a breakpoint.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Breakpoint(pub Length);

impl Breakpoint {
  /// Parse a breakpoint from a token.
  pub fn parse(token: &str) -> Option<Self> {
    match_ignore_ascii_case! {token,
      "sm" => Some(Breakpoint(Length::Rem(40.0))),
      "md" => Some(Breakpoint(Length::Rem(48.0))),
      "lg" => Some(Breakpoint(Length::Rem(64.0))),
      "xl" => Some(Breakpoint(Length::Rem(80.0))),
      "2xl" => Some(Breakpoint(Length::Rem(96.0))),
      _ => None,
    }
  }

  /// Check if the breakpoint matches the viewport width.
  pub fn matches(&self, viewport: Viewport) -> bool {
    let Some(viewport_width) = viewport.size.width else {
      return false;
    };

    let breakpoint_width = match self.0 {
      Length::Rem(value) => viewport.to_device(value * viewport.font_size),
      Length::Px(value) => viewport.to_device(value),
      Length::Vw(value) => (value / 100.0) * viewport_width as f32,
      _ => return false,
    };

    viewport_width >= breakpoint_width as u32
  }
}

/// Represents a tailwind property.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum TailwindProperty {
  /// `background-clip` property.
  BackgroundClip(BackgroundClip),
  /// `box-sizing` property.
  BoxSizing(BoxSizing),
  /// `flex-grow` property.
  FlexGrow(FlexGrow),
  /// `flex-shrink` property.
  FlexShrink(FlexGrow),
  /// `aspect-ratio` property.
  Aspect(AspectRatio),
  /// `align-items` property.
  Items(AlignItems),
  /// `justify-content` property.
  Justify(JustifyContent),
  /// `align-content` property.
  Content(JustifyContent),
  /// `align-self` property.
  JustifySelf(AlignItems),
  /// `justify-items` property.
  JustifyItems(AlignItems),
  /// `flex-direction` property.
  AlignSelf(AlignItems),
  /// `flex-direction` property.
  FlexDirection(FlexDirection),
  /// `flex-wrap` property.
  FlexWrap(FlexWrap),
  /// `flex` property.
  Flex(Flex),
  /// `flex-basis` property.
  FlexBasis(Length),
  /// `overflow` property.
  Overflow(Overflow),
  /// `overflow-x` property.
  OverflowX(Overflow),
  /// `overflow-y` property.
  OverflowY(Overflow),
  /// `position` property.
  Position(Position),
  /// `font-style` property.
  FontStyle(FontStyle),
  /// `font-weight` property.
  FontWeight(FontWeight),
  /// `font-stretch` property.
  FontStretch(FontStretch),
  /// `font-family` property.
  FontFamily(FontFamily),
  /// `line-clamp` property.
  LineClamp(LineClamp),
  /// `text-overflow` property.
  TextOverflow(TextOverflow),
  /// `text-wrap` property.
  TextWrap(TextWrap),
  /// `white-space` property.
  WhiteSpace(WhiteSpace),
  /// `word-break` property.
  WordBreak(WordBreak),
  /// `overflow-wrap` property.
  OverflowWrap(OverflowWrap),
  /// Set `text-overflow: ellipsis`, `white-space: nowrap` and `overflow: hidden`.
  Truncate,
  /// `text-align` property.
  TextAlign(TextAlign),
  /// `text-decoration` property.
  TextDecorationLine(TextDecorationLines),
  /// `text-decoration-color` property.
  TextDecorationColor(ColorInput),
  /// `text-decoration-thickness` property.
  TextDecorationThickness(TextDecorationThickness),
  /// `text-transform` property.
  TextTransform(TextTransform),
  /// `width` and `height` property.
  Size(Length),
  /// `width` property.
  Width(Length),
  /// `height` property.
  Height(Length),
  /// `min-width` property.
  MinWidth(Length),
  /// `min-height` property.
  MinHeight(Length),
  /// `max-width` property.
  MaxWidth(Length),
  /// `max-height` property.
  MaxHeight(Length),
  /// `box-shadow` property.
  Shadow(BoxShadow),
  /// `box-shadow` color override.
  ShadowColor(ColorInput),
  /// `display` property.
  Display(Display),
  /// `list-style-type` property.
  ListStyleType(ListStyleType),
  /// `list-style-position` property.
  ListStylePosition(ListStylePosition),
  /// `list-style-image` property.
  ListStyleImage(ListStyleImage),
  /// `object-position` property.
  ObjectPosition(PositionValue),
  /// `object-fit` property.
  ObjectFit(ObjectFit),
  /// `background-position` property.
  BackgroundPosition(PositionValue),
  /// `background-size` property.
  BackgroundSize(BackgroundSize),
  /// `background-repeat` property.
  BackgroundRepeat(BackgroundRepeat),
  /// `background-image` property.
  BackgroundImage(BackgroundImage),
  /// `mask-image` property.
  MaskImage(BackgroundImage),
  /// `gap` property.
  Gap(Length),
  /// `column-gap` property.
  GapX(Length),
  /// `row-gap` property.
  GapY(Length),
  /// `grid-auto-flow` property.
  GridAutoFlow(GridAutoFlow),
  /// `grid-auto-columns` property.
  GridAutoColumns(GridTrackSize),
  /// `grid-auto-rows` property.
  GridAutoRows(GridTrackSize),
  /// `grid-column` property.
  GridColumn(GridLine),
  /// `grid-row` property.
  GridRow(GridLine),
  /// `grid-column: span <number> / span <number>` property.
  GridColumnSpan(GridPlacementSpan),
  /// `grid-row: span <number> / span <number>` property.
  GridRowSpan(GridPlacementSpan),
  /// `grid-column-start` property.
  GridColumnStart(GridPlacement),
  /// `grid-column-end` property.
  GridColumnEnd(GridPlacement),
  /// `grid-row-start` property.
  GridRowStart(GridPlacement),
  /// `grid-row-end` property.
  GridRowEnd(GridPlacement),
  /// `grid-template-columns` property.
  GridTemplateColumns(TwGridTemplate),
  /// `grid-template-rows` property.
  GridTemplateRows(TwGridTemplate),
  /// `letter-spacing` property.
  LetterSpacing(TwLetterSpacing),
  /// Tailwind `border` utility (`border-width: 1px; border-style: solid`).
  BorderDefault,
  /// `border-width` property.
  BorderWidth(LineWidth),
  /// `border-style` property.
  BorderStyle(BorderStyle),
  /// `color` property.
  Color(ColorInput),
  /// `opacity` property.
  Opacity(PercentageNumber),
  /// `background-color` property.
  BackgroundColor(ColorInput),
  /// `border-color` property.
  BorderColor(ColorInput),
  /// `border-top-width` property.
  BorderTopWidth(LineWidth),
  /// `border-right-width` property.
  BorderRightWidth(LineWidth),
  /// `border-bottom-width` property.
  BorderBottomWidth(LineWidth),
  /// `border-left-width` property.
  BorderLeftWidth(LineWidth),
  /// `border-inline-width` property.
  BorderXWidth(LineWidth),
  /// `border-block-width` property.
  BorderYWidth(LineWidth),
  /// `border-top-color` property.
  BorderTopColor(ColorInput),
  /// `border-right-color` property.
  BorderRightColor(ColorInput),
  /// `border-bottom-color` property.
  BorderBottomColor(ColorInput),
  /// `border-left-color` property.
  BorderLeftColor(ColorInput),
  /// `border-inline-color` property.
  BorderXColor(ColorInput),
  /// `border-block-color` property.
  BorderYColor(ColorInput),
  /// Tailwind `outline` utility (`outline-width: 1px; outline-style: solid`).
  OutlineDefault,
  /// `outline-width` property.
  OutlineWidth(LineWidth),
  /// `outline-color` property.
  OutlineColor(ColorInput),
  /// `outline-style` property.
  OutlineStyle(BorderStyle),
  /// `outline-offset` property.
  OutlineOffset(LineWidth),
  /// `border-radius` property.
  Rounded(TwRounded),
  /// `border-top-left-radius` property.
  RoundedTopLeft(TwRounded),
  /// `border-top-right-radius` property.
  RoundedTopRight(TwRounded),
  /// `border-bottom-right-radius` property.
  RoundedBottomRight(TwRounded),
  /// `border-bottom-left-radius` property.
  RoundedBottomLeft(TwRounded),
  /// `border-top-left-radius`, `border-top-right-radius` property.
  RoundedTop(TwRounded),
  /// `border-top-right-radius`, `border-bottom-right-radius` property.
  RoundedRight(TwRounded),
  /// `border-bottom-left-radius`, `border-bottom-right-radius` property.
  RoundedBottom(TwRounded),
  /// `border-top-left-radius`, `border-bottom-left-radius` property.
  RoundedLeft(TwRounded),
  /// `font-size` property.
  FontSize(TwFontSize),
  /// `line-height` property.
  LineHeight(LineHeight),
  /// `translate` property.
  Translate(Length),
  /// `translate-x` property.
  TranslateX(Length),
  /// `translate-y` property.
  TranslateY(Length),
  /// `rotate` property.
  Rotate(Angle),
  /// `scale` property.
  Scale(PercentageNumber),
  /// `scale-x` property.
  ScaleX(PercentageNumber),
  /// `scale-y` property.
  ScaleY(PercentageNumber),
  /// `transform-origin` property.
  TransformOrigin(PositionValue),
  /// `margin` property.
  Margin(Length),
  /// `margin-inline` property.
  MarginX(Length),
  /// `margin-block` property.
  MarginY(Length),
  /// `margin-top` property.
  MarginTop(Length),
  /// `margin-right` property.
  MarginRight(Length),
  /// `margin-bottom` property.
  MarginBottom(Length),
  /// `margin-left` property.
  MarginLeft(Length),
  /// `margin-inline-start` property.
  MarginInlineStart(Length),
  /// `margin-inline-end` property.
  MarginInlineEnd(Length),
  /// `padding` property.
  Padding(Length),
  /// `padding-inline` property.
  PaddingX(Length),
  /// `padding-block` property.
  PaddingY(Length),
  /// `padding-top` property.
  PaddingTop(Length),
  /// `padding-right` property.
  PaddingRight(Length),
  /// `padding-bottom` property.
  PaddingBottom(Length),
  /// `padding-left` property.
  PaddingLeft(Length),
  /// `padding-inline-start` property.
  PaddingInlineStart(Length),
  /// `padding-inline-end` property.
  PaddingInlineEnd(Length),
  /// `inset` property.
  Inset(Length),
  /// `inset-inline` property.
  InsetX(Length),
  /// `inset-block` property.
  InsetY(Length),
  /// `top` property.
  Top(Length),
  /// `right` property.
  Right(Length),
  /// `bottom` property.
  Bottom(Length),
  /// `left` property.
  Left(Length),
  /// `filter: blur()` property.
  Blur(TwBlur),
  /// `filter: brightness()` property.
  Brightness(PercentageNumber),
  /// `filter: contrast()` property.
  Contrast(PercentageNumber),
  /// `filter: drop-shadow()` property.
  DropShadow(TextShadow),
  /// `filter: grayscale()` property.
  Grayscale(PercentageNumber),
  /// `filter: hue-rotate()` property.
  HueRotate(Angle),
  /// `filter: invert()` property.
  Invert(PercentageNumber),
  /// `filter: saturate()` property.
  Saturate(PercentageNumber),
  /// `filter: sepia()` property.
  Sepia(PercentageNumber),
  /// `filter` property.
  Filter(Filters),
  /// `backdrop-filter: blur()` property.
  BackdropBlur(TwBlur),
  /// `backdrop-filter: brightness()` property.
  BackdropBrightness(PercentageNumber),
  /// `backdrop-filter: contrast()` property.
  BackdropContrast(PercentageNumber),
  /// `backdrop-filter: grayscale()` property.
  BackdropGrayscale(PercentageNumber),
  /// `backdrop-filter: hue-rotate()` property.
  BackdropHueRotate(Angle),
  /// `backdrop-filter: invert()` property.
  BackdropInvert(PercentageNumber),
  /// `backdrop-filter: opacity()` property.
  BackdropOpacity(PercentageNumber),
  /// `backdrop-filter: saturate()` property.
  BackdropSaturate(PercentageNumber),
  /// `backdrop-filter: sepia()` property.
  BackdropSepia(PercentageNumber),
  /// `backdrop-filter` property.
  BackdropFilter(Filters),
  /// `text-shadow` property.
  TextShadow(TextShadow),
  /// `text-shadow` color override.
  TextShadowColor(ColorInput),
  /// `box-shadow` layer set.
  ShadowList(&'static [BoxShadow]),
  /// `text-shadow` layer set.
  TextShadowList(&'static [TextShadow]),
  /// `isolation` property.
  Isolation(Isolation),
  /// `mix-blend-mode` property.
  MixBlendMode(BlendMode),
  /// `background-blend-mode` property.
  BackgroundBlendMode(BlendMode),
  /// `visibility` property.
  Visibility(Visibility),
  /// `vertical-align` property.
  VerticalAlign(VerticalAlign),
  /// `animation` shorthand.
  Animation(Animations),
  /// `bg-linear` property.
  BgLinearAngle(Angle),
  /// `bg-radial` property.
  BgRadial,
  /// `bg-conic` property.
  BgConicAngle(Angle),
  /// `from` property.
  GradientFrom(ColorInput),
  /// `to` property.
  GradientTo(ColorInput),
  /// `via` property.
  GradientVia(ColorInput),
  /// Gradient `from` stop position.
  GradientFromPosition(Length),
  /// Gradient `via` stop position.
  GradientViaPosition(Length),
  /// Gradient `to` stop position.
  GradientToPosition(Length),
}

fn extract_arbitrary_value(suffix: &str) -> Option<Cow<'_, str>> {
  let value = suffix.strip_prefix('[')?.strip_suffix(']')?;
  Some(decode_arbitrary_value(value))
}

enum FnKind {
  Url,
  VarTheme,
  Other,
}

impl FnKind {
  fn from_name(name: &str) -> FnKind {
    if name == "url" || name.ends_with("_url") {
      FnKind::Url
    } else if matches!(name, "var" | "theme") || name.ends_with("_var") || name.ends_with("_theme")
    {
      FnKind::VarTheme
    } else {
      FnKind::Other
    }
  }
}

fn is_ident_byte(byte: u8) -> bool {
  byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_'
}

/// Mirrors Tailwind's `decodeArbitraryValue`: `_` becomes a space and `\_` a
/// literal `_`, but underscores inside `url(...)` and the first argument of
/// `var()`/`theme()` are preserved.
fn decode_arbitrary_value(value: &str) -> Cow<'_, str> {
  if !value.contains('_') {
    return Cow::Borrowed(value);
  }

  let bytes = value.as_bytes();
  let mut out = String::with_capacity(value.len());
  let mut stack: Vec<(FnKind, bool)> = Vec::new();
  let mut ident_start = 0;
  let mut index = 0;
  while index < bytes.len() {
    let byte = bytes[index];
    if byte == b'\\' && bytes.get(index + 1) == Some(&b'_') {
      out.push('_');
      index += 2;
      ident_start = index;
    } else if byte == b'_' {
      let preserved = stack.iter().any(|(kind, _)| matches!(kind, FnKind::Url))
        || matches!(stack.last(), Some((FnKind::VarTheme, true)));
      out.push(if preserved { '_' } else { ' ' });
      index += 1;
    } else if byte == b'(' {
      stack.push((FnKind::from_name(&value[ident_start..index]), true));
      out.push('(');
      index += 1;
      ident_start = index;
    } else if byte == b')' {
      stack.pop();
      out.push(')');
      index += 1;
      ident_start = index;
    } else if byte == b',' {
      if let Some((_, first_arg)) = stack.last_mut() {
        *first_arg = false;
      }
      out.push(',');
      index += 1;
      ident_start = index;
    } else if is_ident_byte(byte) {
      out.push(byte as char);
      index += 1;
    } else {
      let char_len = value[index..].chars().next().map_or(1, char::len_utf8);
      out.push_str(&value[index..index + char_len]);
      index += char_len;
      ident_start = index;
    }
  }
  Cow::Owned(out)
}

/// A trait for parsing tailwind properties.
pub(crate) trait TailwindPropertyParser: Sized + for<'i> FromCss<'i> {
  /// Parse a tailwind property from a token. Defaults to the type's `FromCss`
  /// parser; override for keywords Tailwind spells differently from CSS.
  fn parse_tw(token: &str) -> Option<Self> {
    Self::from_css_str(token).ok()
  }

  /// Theme namespaces this value type reads, tried in order.
  const NAMESPACES: &'static [TwNamespace] = &[];

  /// Arbitrary value, then theme token, then the built-in scale. A theme token
  /// is spelled as a CSS value, so it takes the arbitrary-value path rather
  /// than the utility-suffix grammar.
  fn parse_tw_themed(token: &str, theme: &Theme, namespaces: &[TwNamespace]) -> Option<Self> {
    if let Some(value) = extract_arbitrary_value(token) {
      return Self::from_css_str(&value).ok();
    }

    for &namespace in namespaces {
      match theme.lookup(namespace, token) {
        ThemeLookup::Value(value) => return Self::from_css_str(value).ok(),
        ThemeLookup::Removed => return None,
        ThemeLookup::Missing => {}
      }
    }

    Self::parse_tw(token)
  }

  /// The entry point utility dispatch calls. `namespaces` is the utility's own
  /// list, which differs from `NAMESPACES` where one value grammar serves two
  /// namespaces: `max-w-*` reads `--container-*` where `w-*` reads `--spacing-*`.
  /// Override to handle a modifier the suffix carries, such as a color's `/50`.
  fn parse_tw_with_arbitrary(
    token: &str,
    theme: &Theme,
    namespaces: &[TwNamespace],
  ) -> Option<Self> {
    Self::parse_tw_themed(token, theme, namespaces)
  }
}

macro_rules! try_neg {
  ($self:expr;
    try_negative: $($neg:ident),+ $(,)?;
    unary: $($un:ident),+ $(,)?;
    grid: $($grid:ident),+ $(,)?
  ) => {
    Some(match $self {
      $(TailwindProperty::$neg(v) => TailwindProperty::$neg(v.try_negative()?),)+
      $(TailwindProperty::$un(v) => TailwindProperty::$un(-v),)+
      $(TailwindProperty::$grid(p) => TailwindProperty::$grid(p.try_negative()?),)+
      _ => return None,
    })
  };
}

impl TailwindProperty {
  fn try_neg(self) -> Option<Self> {
    try_neg!(self;
      try_negative:
        Margin, MarginX, MarginY, MarginTop, MarginRight, MarginBottom, MarginLeft,
        MarginInlineStart, MarginInlineEnd, Inset, InsetX, InsetY, Top, Right, Bottom, Left,
        Translate, TranslateX, TranslateY;
      unary: Scale, ScaleX, ScaleY, Rotate, LetterSpacing, HueRotate, BackdropHueRotate;
      grid: GridColumnStart, GridColumnEnd, GridRowStart, GridRowEnd
    )
  }
}

macro_rules! push_decl {
  ($builder:expr, $important:expr $(, $property:ident($value:expr))* $(,)?) => {{
    $(
      $builder.push(StyleDeclaration::$property($value), $important);
    )*
  }};
}

macro_rules! rounded_corners {
  ($builder:expr, $important:expr, $rounded:expr $(, $corner:ident)+ $(,)?) => {{
    let value = SpacePair::from_single($rounded.0);
    push_decl!($builder, $important $(, $corner(value))+);
  }};
}

impl TailwindProperty {
  fn resource_url(&self) -> Option<&str> {
    match self {
      TailwindProperty::BackgroundImage(BackgroundImage::Url(url))
      | TailwindProperty::MaskImage(BackgroundImage::Url(url)) => Some(url.as_ref()),
      _ => None,
    }
  }

  /// Parse a single tailwind property from a token.
  pub fn parse(token: &str, theme: &Theme) -> Option<TailwindProperty> {
    // Check fixed properties first
    if let Some(property) = FIXED_PROPERTIES.get(token) {
      return Some(property.clone());
    }

    if let Some(stripped) = token.strip_prefix('-') {
      return Self::parse_prefix_suffix(stripped, theme).and_then(Self::try_neg);
    }

    Self::parse_prefix_suffix(token, theme)
  }

  fn parse_prefix_suffix(token: &str, theme: &Theme) -> Option<TailwindProperty> {
    let bytes = token.as_bytes();

    for dash_pos in (0..bytes.len()).rev() {
      if bytes[dash_pos] != b'-' {
        continue;
      }

      let prefix = &token[..dash_pos];
      let Some(parsers) = PREFIX_PARSERS.get(prefix) else {
        continue;
      };

      let suffix = &token[dash_pos + 1..];
      for parser in *parsers {
        if let Some(property) = parser.parse(suffix, theme) {
          return Some(property);
        }
      }
    }

    None
  }

  #[inline(never)]
  fn apply(self, builder: &mut TailwindDeclarationBuilder, important: bool) {
    match self {
      TailwindProperty::BgLinearAngle(angle) => {
        builder.gradient_state.gradient_type = TwGradientType::Linear;
        builder.gradient_state.angle = Some(angle);
        builder.gradient_state.important = important;
      }
      TailwindProperty::BgRadial => {
        builder.gradient_state.gradient_type = TwGradientType::Radial;
        builder.gradient_state.important = important;
      }
      TailwindProperty::BgConicAngle(angle) => {
        builder.gradient_state.gradient_type = TwGradientType::Conic;
        builder.gradient_state.angle = Some(angle);
        builder.gradient_state.important = important;
      }
      TailwindProperty::GradientFrom(color) => {
        builder.gradient_state.from = Some(color);
        builder.gradient_state.important = important;
      }
      TailwindProperty::GradientTo(color) => {
        builder.gradient_state.to = Some(color);
        builder.gradient_state.important = important;
      }
      TailwindProperty::GradientVia(color) => {
        builder.gradient_state.via = Some(color);
        builder.gradient_state.important = important;
      }
      TailwindProperty::GradientFromPosition(pos) => {
        builder.gradient_state.from_position = Some(pos);
        builder.gradient_state.important = important;
      }
      TailwindProperty::GradientViaPosition(pos) => {
        builder.gradient_state.via_position = Some(pos);
        builder.gradient_state.important = important;
      }
      TailwindProperty::GradientToPosition(pos) => {
        builder.gradient_state.to_position = Some(pos);
        builder.gradient_state.important = important;
      }
      TailwindProperty::BackgroundClip(background_clip) => {
        push_decl!(builder, important, background_clip(background_clip));
      }
      TailwindProperty::Gap(gap) => {
        push_decl!(
          builder,
          important,
          row_gap(gap.into()),
          column_gap(gap.into())
        );
      }
      TailwindProperty::GapX(gap_x) => {
        push_decl!(builder, important, column_gap(gap_x.into()))
      }
      TailwindProperty::GapY(gap_y) => push_decl!(builder, important, row_gap(gap_y.into())),
      TailwindProperty::BoxSizing(box_sizing) => {
        push_decl!(builder, important, box_sizing(box_sizing))
      }
      TailwindProperty::FlexGrow(flex_grow) => {
        push_decl!(builder, important, flex_grow(Some(flex_grow)))
      }
      TailwindProperty::FlexShrink(flex_shrink) => {
        push_decl!(builder, important, flex_shrink(Some(flex_shrink)))
      }
      TailwindProperty::Aspect(ratio) => push_decl!(builder, important, aspect_ratio(ratio)),
      TailwindProperty::Items(align_items) => {
        push_decl!(builder, important, align_items(align_items))
      }
      TailwindProperty::Justify(justify_content) => {
        push_decl!(builder, important, justify_content(justify_content))
      }
      TailwindProperty::Content(align_content) => {
        push_decl!(builder, important, align_content(align_content))
      }
      TailwindProperty::AlignSelf(align_self) => {
        push_decl!(builder, important, align_self(align_self))
      }
      TailwindProperty::FlexDirection(flex_direction) => {
        push_decl!(builder, important, flex_direction(flex_direction))
      }
      TailwindProperty::FlexWrap(flex_wrap) => push_decl!(builder, important, flex_wrap(flex_wrap)),
      TailwindProperty::Flex(flex) => {
        push_decl!(
          builder,
          important,
          flex_grow(Some(FlexGrow(flex.grow))),
          flex_shrink(Some(FlexGrow(flex.shrink))),
          flex_basis(Some(flex.basis))
        );
      }
      TailwindProperty::FlexBasis(flex_basis) => {
        push_decl!(builder, important, flex_basis(Some(flex_basis)))
      }
      TailwindProperty::Overflow(overflow) => {
        push_decl!(
          builder,
          important,
          overflow_x(overflow),
          overflow_y(overflow)
        );
      }
      TailwindProperty::Position(position) => push_decl!(builder, important, position(position)),
      TailwindProperty::FontStyle(font_style) => {
        push_decl!(builder, important, font_style(font_style))
      }
      TailwindProperty::FontWeight(font_weight) => {
        push_decl!(builder, important, font_weight(font_weight))
      }
      TailwindProperty::FontStretch(font_stretch) => {
        push_decl!(builder, important, font_stretch(font_stretch))
      }
      TailwindProperty::FontFamily(font_family) => {
        push_decl!(builder, important, font_family(font_family))
      }
      TailwindProperty::LineClamp(value) => {
        push_decl!(
          builder,
          important,
          max_lines(value.max_lines),
          block_ellipsis(value.block_ellipsis),
          r#continue(value.line_continue)
        )
      }
      TailwindProperty::TextAlign(text_align) => {
        push_decl!(builder, important, text_align(text_align))
      }
      TailwindProperty::TextDecorationLine(text_decoration) => push_decl!(
        builder,
        important,
        text_decoration_line(Some(text_decoration))
      ),
      TailwindProperty::TextDecorationColor(color_input) => {
        push_decl!(builder, important, text_decoration_color(color_input))
      }
      TailwindProperty::TextDecorationThickness(thickness) => {
        push_decl!(builder, important, text_decoration_thickness(thickness))
      }
      TailwindProperty::TextTransform(text_transform) => {
        push_decl!(builder, important, text_transform(text_transform))
      }
      TailwindProperty::Size(size) => {
        push_decl!(builder, important, width(size), height(size));
      }
      TailwindProperty::Width(width) => push_decl!(builder, important, width(width)),
      TailwindProperty::Height(height) => push_decl!(builder, important, height(height)),
      TailwindProperty::MinWidth(min_width) => push_decl!(builder, important, min_width(min_width)),
      TailwindProperty::MinHeight(min_height) => {
        push_decl!(builder, important, min_height(min_height))
      }
      TailwindProperty::MaxWidth(max_width) => {
        push_decl!(builder, important, max_width(max_width.into()))
      }
      TailwindProperty::MaxHeight(max_height) => {
        push_decl!(builder, important, max_height(max_height.into()))
      }
      TailwindProperty::Shadow(box_shadow) => builder.set_shadow_layers([box_shadow], important),
      TailwindProperty::ShadowList(&[]) => builder.reset_shadow(important),
      TailwindProperty::ShadowList(layers) => {
        builder.set_shadow_layers(layers.iter().copied(), important)
      }
      TailwindProperty::ShadowColor(color) => builder.set_shadow_color(color, important),
      TailwindProperty::Display(display) => {
        push_decl!(builder, important, display(display));
      }
      TailwindProperty::ListStyleType(style_type) => {
        push_decl!(builder, important, list_style_type(style_type));
      }
      TailwindProperty::ListStylePosition(position) => {
        push_decl!(builder, important, list_style_position(position));
      }
      TailwindProperty::ListStyleImage(image) => {
        push_decl!(builder, important, list_style_image(image));
      }
      TailwindProperty::OverflowX(overflow) => push_decl!(builder, important, overflow_x(overflow)),
      TailwindProperty::OverflowY(overflow) => push_decl!(builder, important, overflow_y(overflow)),
      TailwindProperty::ObjectPosition(background_position) => {
        push_decl!(builder, important, object_position(background_position))
      }
      TailwindProperty::ObjectFit(object_fit) => {
        push_decl!(builder, important, object_fit(object_fit))
      }
      TailwindProperty::BackgroundPosition(background_position) => push_decl!(
        builder,
        important,
        background_position([background_position].into())
      ),
      TailwindProperty::BackgroundSize(background_size) => push_decl!(
        builder,
        important,
        background_size([background_size].into())
      ),
      TailwindProperty::BackgroundRepeat(background_repeat) => push_decl!(
        builder,
        important,
        background_repeat([background_repeat].into())
      ),
      TailwindProperty::BackgroundImage(background_image) => push_decl!(
        builder,
        important,
        background_image(Some([background_image].into()))
      ),
      TailwindProperty::MaskImage(mask_image) => {
        push_decl!(builder, important, mask_image(Some([mask_image].into())))
      }
      TailwindProperty::BorderDefault => {
        push_decl!(
          builder,
          important,
          border_top_width(LineWidth::Length(Length::Px(1.0))),
          border_right_width(LineWidth::Length(Length::Px(1.0))),
          border_bottom_width(LineWidth::Length(Length::Px(1.0))),
          border_left_width(LineWidth::Length(Length::Px(1.0)))
        );
      }
      TailwindProperty::BorderWidth(tw_border_width) => {
        push_decl!(
          builder,
          important,
          border_top_width(tw_border_width),
          border_right_width(tw_border_width),
          border_bottom_width(tw_border_width),
          border_left_width(tw_border_width)
        );
      }
      TailwindProperty::BorderStyle(border_style) => {
        push_decl!(
          builder,
          important,
          border_top_style(border_style),
          border_right_style(border_style),
          border_bottom_style(border_style),
          border_left_style(border_style)
        )
      }
      TailwindProperty::JustifySelf(align_items) => {
        push_decl!(builder, important, justify_self(align_items))
      }
      TailwindProperty::JustifyItems(align_items) => {
        push_decl!(builder, important, justify_items(align_items))
      }
      TailwindProperty::Color(color_input) => push_decl!(builder, important, color(color_input)),
      TailwindProperty::Opacity(percentage_number) => {
        push_decl!(builder, important, opacity(percentage_number))
      }
      TailwindProperty::BackgroundColor(color_input) => {
        push_decl!(builder, important, background_color(color_input))
      }
      TailwindProperty::BorderColor(color_input) => {
        push_decl!(
          builder,
          important,
          border_top_color(color_input),
          border_right_color(color_input),
          border_bottom_color(color_input),
          border_left_color(color_input)
        )
      }
      TailwindProperty::BorderTopWidth(tw_border_width) => {
        push_decl!(builder, important, border_top_width(tw_border_width))
      }
      TailwindProperty::BorderRightWidth(tw_border_width) => {
        push_decl!(builder, important, border_right_width(tw_border_width))
      }
      TailwindProperty::BorderBottomWidth(tw_border_width) => {
        push_decl!(builder, important, border_bottom_width(tw_border_width))
      }
      TailwindProperty::BorderLeftWidth(tw_border_width) => {
        push_decl!(builder, important, border_left_width(tw_border_width))
      }
      TailwindProperty::BorderXWidth(tw_border_width) => {
        push_decl!(
          builder,
          important,
          border_left_width(tw_border_width),
          border_right_width(tw_border_width)
        );
      }
      TailwindProperty::BorderYWidth(tw_border_width) => {
        push_decl!(
          builder,
          important,
          border_top_width(tw_border_width),
          border_bottom_width(tw_border_width)
        );
      }
      TailwindProperty::BorderTopColor(color_input) => {
        push_decl!(builder, important, border_top_color(color_input))
      }
      TailwindProperty::BorderRightColor(color_input) => {
        push_decl!(builder, important, border_right_color(color_input))
      }
      TailwindProperty::BorderBottomColor(color_input) => {
        push_decl!(builder, important, border_bottom_color(color_input))
      }
      TailwindProperty::BorderLeftColor(color_input) => {
        push_decl!(builder, important, border_left_color(color_input))
      }
      TailwindProperty::BorderXColor(color_input) => {
        push_decl!(
          builder,
          important,
          border_left_color(color_input),
          border_right_color(color_input)
        );
      }
      TailwindProperty::BorderYColor(color_input) => {
        push_decl!(
          builder,
          important,
          border_top_color(color_input),
          border_bottom_color(color_input)
        );
      }
      TailwindProperty::OutlineDefault => {
        push_decl!(
          builder,
          important,
          outline_width(LineWidth::Length(Length::Px(1.0))),
          outline_style(BorderStyle::Solid)
        );
      }
      TailwindProperty::OutlineWidth(tw_border_width) => {
        push_decl!(builder, important, outline_width(tw_border_width))
      }
      TailwindProperty::OutlineColor(color_input) => {
        push_decl!(builder, important, outline_color(color_input))
      }
      TailwindProperty::OutlineStyle(outline_style) => {
        push_decl!(builder, important, outline_style(outline_style))
      }
      TailwindProperty::OutlineOffset(outline_offset) => {
        push_decl!(
          builder,
          important,
          outline_offset(Length::from(outline_offset))
        )
      }
      TailwindProperty::Rounded(rounded) => rounded_corners!(
        builder,
        important,
        rounded,
        border_top_left_radius,
        border_top_right_radius,
        border_bottom_right_radius,
        border_bottom_left_radius
      ),
      TailwindProperty::VerticalAlign(vertical_align) => {
        push_decl!(builder, important, vertical_align(vertical_align))
      }
      TailwindProperty::RoundedTopLeft(rounded) => {
        rounded_corners!(builder, important, rounded, border_top_left_radius)
      }
      TailwindProperty::RoundedTopRight(rounded) => {
        rounded_corners!(builder, important, rounded, border_top_right_radius)
      }
      TailwindProperty::RoundedBottomRight(rounded) => {
        rounded_corners!(builder, important, rounded, border_bottom_right_radius)
      }
      TailwindProperty::RoundedBottomLeft(rounded) => {
        rounded_corners!(builder, important, rounded, border_bottom_left_radius)
      }
      TailwindProperty::RoundedTop(rounded) => rounded_corners!(
        builder,
        important,
        rounded,
        border_top_left_radius,
        border_top_right_radius
      ),
      TailwindProperty::RoundedRight(rounded) => rounded_corners!(
        builder,
        important,
        rounded,
        border_top_right_radius,
        border_bottom_right_radius
      ),
      TailwindProperty::RoundedBottom(rounded) => rounded_corners!(
        builder,
        important,
        rounded,
        border_bottom_left_radius,
        border_bottom_right_radius
      ),
      TailwindProperty::RoundedLeft(rounded) => rounded_corners!(
        builder,
        important,
        rounded,
        border_top_left_radius,
        border_bottom_left_radius
      ),
      TailwindProperty::TextOverflow(text_overflow) => {
        push_decl!(builder, important, text_overflow(text_overflow))
      }
      TailwindProperty::Truncate => {
        push_decl!(
          builder,
          important,
          text_overflow(TextOverflow::Ellipsis),
          text_wrap_mode(TextWrapMode::NoWrap),
          white_space_collapse(WhiteSpaceCollapse::Collapse),
          overflow_x(Overflow::Hidden),
          overflow_y(Overflow::Hidden)
        );
      }
      TailwindProperty::TextWrap(text_wrap) => {
        push_decl!(
          builder,
          important,
          text_wrap_mode(text_wrap.mode),
          text_wrap_style(text_wrap.style)
        );
      }
      TailwindProperty::WhiteSpace(white_space) => {
        push_decl!(
          builder,
          important,
          text_wrap_mode(white_space.text_wrap_mode),
          white_space_collapse(white_space.white_space_collapse)
        );
      }
      TailwindProperty::WordBreak(word_break) => {
        push_decl!(builder, important, word_break(word_break))
      }
      TailwindProperty::Isolation(isolation) => {
        push_decl!(builder, important, isolation(isolation))
      }
      TailwindProperty::MixBlendMode(blend_mode) => {
        push_decl!(builder, important, mix_blend_mode(blend_mode))
      }
      TailwindProperty::BackgroundBlendMode(blend_mode) => push_decl!(
        builder,
        important,
        background_blend_mode([blend_mode].into())
      ),
      TailwindProperty::OverflowWrap(overflow_wrap) => {
        push_decl!(builder, important, overflow_wrap(overflow_wrap))
      }
      TailwindProperty::FontSize(font_size) => {
        push_decl!(builder, important, font_size(font_size.font_size));
        if let Some(line_height) = font_size.line_height {
          push_decl!(builder, important, line_height(line_height));
        }
      }
      TailwindProperty::LineHeight(line_height) => {
        push_decl!(builder, important, line_height(line_height))
      }
      TailwindProperty::Translate(length) => {
        builder
          .transform_state
          .set_translate(SpacePair::from_single(length), important);
      }
      TailwindProperty::TranslateX(length) => {
        builder.transform_state.translate_mut(important).x = length;
      }
      TailwindProperty::TranslateY(length) => {
        builder.transform_state.translate_mut(important).y = length;
      }
      TailwindProperty::Rotate(angle) => push_decl!(builder, important, rotate(Some(angle))),
      TailwindProperty::Scale(percentage_number) => {
        builder
          .transform_state
          .set_scale(SpacePair::from_single(percentage_number), important);
      }
      TailwindProperty::ScaleX(percentage_number) => {
        builder.transform_state.scale_mut(important).x = percentage_number;
      }
      TailwindProperty::ScaleY(percentage_number) => {
        builder.transform_state.scale_mut(important).y = percentage_number;
      }
      TailwindProperty::TransformOrigin(background_position) => {
        push_decl!(builder, important, transform_origin(background_position))
      }
      TailwindProperty::Margin(length) => {
        push_decl!(
          builder,
          important,
          margin_top(length),
          margin_right(length),
          margin_bottom(length),
          margin_left(length)
        );
      }
      TailwindProperty::MarginX(length) => {
        push_decl!(
          builder,
          important,
          margin_left(length),
          margin_right(length)
        );
      }
      TailwindProperty::MarginY(length) => {
        push_decl!(
          builder,
          important,
          margin_top(length),
          margin_bottom(length)
        );
      }
      TailwindProperty::MarginTop(length) => push_decl!(builder, important, margin_top(length)),
      TailwindProperty::MarginRight(length) => push_decl!(builder, important, margin_right(length)),
      TailwindProperty::MarginBottom(length) => {
        push_decl!(builder, important, margin_bottom(length))
      }
      TailwindProperty::MarginLeft(length) => push_decl!(builder, important, margin_left(length)),
      TailwindProperty::MarginInlineStart(length) => {
        push_decl!(builder, important, margin_inline_start(length))
      }
      TailwindProperty::MarginInlineEnd(length) => {
        push_decl!(builder, important, margin_inline_end(length))
      }
      TailwindProperty::Padding(length) => {
        push_decl!(
          builder,
          important,
          padding_top(length),
          padding_right(length),
          padding_bottom(length),
          padding_left(length)
        );
      }
      TailwindProperty::PaddingX(length) => {
        push_decl!(
          builder,
          important,
          padding_left(length),
          padding_right(length)
        );
      }
      TailwindProperty::PaddingY(length) => {
        push_decl!(
          builder,
          important,
          padding_top(length),
          padding_bottom(length)
        );
      }
      TailwindProperty::PaddingTop(length) => push_decl!(builder, important, padding_top(length)),
      TailwindProperty::PaddingRight(length) => {
        push_decl!(builder, important, padding_right(length))
      }
      TailwindProperty::PaddingBottom(length) => {
        push_decl!(builder, important, padding_bottom(length))
      }
      TailwindProperty::PaddingLeft(length) => push_decl!(builder, important, padding_left(length)),
      TailwindProperty::PaddingInlineStart(length) => {
        push_decl!(builder, important, padding_inline_start(length))
      }
      TailwindProperty::PaddingInlineEnd(length) => {
        push_decl!(builder, important, padding_inline_end(length))
      }
      TailwindProperty::Inset(length) => {
        push_decl!(
          builder,
          important,
          top(length),
          right(length),
          bottom(length),
          left(length)
        );
      }
      TailwindProperty::InsetX(length) => {
        push_decl!(builder, important, left(length), right(length));
      }
      TailwindProperty::InsetY(length) => {
        push_decl!(builder, important, top(length), bottom(length));
      }
      TailwindProperty::Top(length) => push_decl!(builder, important, top(length)),
      TailwindProperty::Right(length) => push_decl!(builder, important, right(length)),
      TailwindProperty::Bottom(length) => push_decl!(builder, important, bottom(length)),
      TailwindProperty::Left(length) => push_decl!(builder, important, left(length)),
      TailwindProperty::GridAutoColumns(grid_auto_size) => push_decl!(
        builder,
        important,
        grid_auto_columns(Some([grid_auto_size].into()))
      ),
      TailwindProperty::GridAutoRows(grid_auto_size) => push_decl!(
        builder,
        important,
        grid_auto_rows(Some([grid_auto_size].into()))
      ),
      TailwindProperty::GridColumn(tw_grid_span) => {
        builder.set_grid_column(tw_grid_span, important)
      }
      TailwindProperty::GridRow(tw_grid_span) => builder.set_grid_row(tw_grid_span, important),
      TailwindProperty::GridColumnStart(tw_grid_placement) => {
        let start = builder
          .grid_column
          .start
          .get_or_insert_with(GridPlacement::auto);
        *start = tw_grid_placement;
        builder.grid_column.start_important = important;
      }
      TailwindProperty::GridColumnEnd(tw_grid_placement) => {
        let end = builder
          .grid_column
          .end
          .get_or_insert_with(GridPlacement::auto);
        *end = tw_grid_placement;
        builder.grid_column.end_important = important;
      }
      TailwindProperty::GridRowStart(tw_grid_placement) => {
        let start = builder
          .grid_row
          .start
          .get_or_insert_with(GridPlacement::auto);
        *start = tw_grid_placement;
        builder.grid_row.start_important = important;
      }
      TailwindProperty::GridRowEnd(tw_grid_placement) => {
        let end = builder.grid_row.end.get_or_insert_with(GridPlacement::auto);
        *end = tw_grid_placement;
        builder.grid_row.end_important = important;
      }
      TailwindProperty::GridTemplateColumns(tw_grid_template) => push_decl!(
        builder,
        important,
        grid_template_columns(Some(tw_grid_template.0))
      ),
      TailwindProperty::GridTemplateRows(tw_grid_template) => push_decl!(
        builder,
        important,
        grid_template_rows(Some(tw_grid_template.0))
      ),
      TailwindProperty::LetterSpacing(tw_letter_spacing) => {
        push_decl!(builder, important, letter_spacing(tw_letter_spacing.0))
      }
      TailwindProperty::GridAutoFlow(grid_auto_flow) => {
        push_decl!(builder, important, grid_auto_flow(grid_auto_flow))
      }
      TailwindProperty::GridColumnSpan(grid_placement_span) => {
        builder.set_grid_column(GridLine::span(grid_placement_span), important)
      }
      TailwindProperty::GridRowSpan(grid_placement_span) => {
        builder.set_grid_row(GridLine::span(grid_placement_span), important)
      }
      TailwindProperty::Blur(tw_blur) => builder.push_filter(Filter::Blur(tw_blur.0), important),
      TailwindProperty::Brightness(percentage_number) => {
        builder.push_filter(Filter::Brightness(percentage_number), important)
      }
      TailwindProperty::Contrast(percentage_number) => {
        builder.push_filter(Filter::Contrast(percentage_number), important)
      }
      TailwindProperty::DropShadow(text_shadow) => {
        builder.push_filter(Filter::DropShadow(text_shadow), important)
      }
      TailwindProperty::Grayscale(percentage_number) => {
        builder.push_filter(Filter::Grayscale(percentage_number), important)
      }
      TailwindProperty::HueRotate(angle) => {
        builder.push_filter(Filter::HueRotate(angle), important)
      }
      TailwindProperty::Invert(percentage_number) => {
        builder.push_filter(Filter::Invert(percentage_number), important)
      }
      TailwindProperty::Saturate(percentage_number) => {
        builder.push_filter(Filter::Saturate(percentage_number), important)
      }
      TailwindProperty::Sepia(percentage_number) => {
        builder.push_filter(Filter::Sepia(percentage_number), important)
      }
      TailwindProperty::Filter(filters) => {
        if filters.is_empty() {
          builder.set_filter_reset(important, false);
        } else {
          for filter in filters {
            builder.push_filter(filter, important);
          }
        }
      }
      TailwindProperty::BackdropBlur(tw_blur) => {
        builder.push_backdrop_filter(Filter::Blur(tw_blur.0), important)
      }
      TailwindProperty::BackdropBrightness(percentage_number) => {
        builder.push_backdrop_filter(Filter::Brightness(percentage_number), important)
      }
      TailwindProperty::BackdropContrast(percentage_number) => {
        builder.push_backdrop_filter(Filter::Contrast(percentage_number), important)
      }
      TailwindProperty::BackdropGrayscale(percentage_number) => {
        builder.push_backdrop_filter(Filter::Grayscale(percentage_number), important)
      }
      TailwindProperty::BackdropHueRotate(angle) => {
        builder.push_backdrop_filter(Filter::HueRotate(angle), important)
      }
      TailwindProperty::BackdropInvert(percentage_number) => {
        builder.push_backdrop_filter(Filter::Invert(percentage_number), important)
      }
      TailwindProperty::BackdropOpacity(percentage_number) => {
        builder.push_backdrop_filter(Filter::Opacity(percentage_number), important)
      }
      TailwindProperty::BackdropSaturate(percentage_number) => {
        builder.push_backdrop_filter(Filter::Saturate(percentage_number), important)
      }
      TailwindProperty::BackdropSepia(percentage_number) => {
        builder.push_backdrop_filter(Filter::Sepia(percentage_number), important)
      }
      TailwindProperty::BackdropFilter(filters) => {
        if filters.is_empty() {
          builder.set_filter_reset(important, true);
        } else {
          for filter in filters {
            builder.push_backdrop_filter(filter, important);
          }
        }
      }
      TailwindProperty::TextShadow(text_shadow) => {
        builder.set_text_shadow_layers([text_shadow], important)
      }
      TailwindProperty::TextShadowList(&[]) => builder.reset_text_shadow(important),
      TailwindProperty::TextShadowList(layers) => {
        builder.set_text_shadow_layers(layers.iter().copied(), important)
      }
      TailwindProperty::TextShadowColor(color) => builder.set_text_shadow_color(color, important),
      TailwindProperty::Visibility(visibility) => {
        push_decl!(builder, important, visibility(visibility))
      }
      TailwindProperty::Animation(animations) => {
        push_decl!(
          builder,
          important,
          animation_duration(
            animations
              .iter()
              .map(|animation| animation.duration)
              .collect()
          ),
          animation_delay(animations.iter().map(|animation| animation.delay).collect()),
          animation_timing_function(
            animations
              .iter()
              .map(|animation| animation.timing_function)
              .collect()
          ),
          animation_iteration_count(
            animations
              .iter()
              .map(|animation| animation.iteration_count)
              .collect()
          ),
          animation_direction(
            animations
              .iter()
              .map(|animation| animation.direction)
              .collect()
          ),
          animation_fill_mode(
            animations
              .iter()
              .map(|animation| animation.fill_mode)
              .collect()
          ),
          animation_play_state(
            animations
              .iter()
              .map(|animation| animation.play_state)
              .collect()
          ),
          animation_name(
            animations
              .into_iter()
              .map(|animation| animation.name)
              .collect()
          )
        );
      }
    }
  }
}

#[cfg(test)]
#[allow(clippy::panic, clippy::unwrap_used)]
mod tests;
