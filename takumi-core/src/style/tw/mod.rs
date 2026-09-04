mod builder;
/// Utility-prefix to property-parser mapping.
mod map;
/// The `--color-*` style prefixes a utility reads its value from.
mod namespace;
/// Parsers for Tailwind utility-class suffixes.
mod parser;

use std::{
  borrow::Cow,
  cell::RefCell,
  cmp::Ordering,
  collections::HashMap,
  convert::Infallible,
  rc::Rc,
  str::FromStr,
  sync::{Arc, LazyLock},
};

use builder::TailwindDeclarationBuilder;
use cssparser::match_ignore_ascii_case;
pub(crate) use namespace::Namespace;
use quick_cache::sync::Cache;
use serde::{Deserialize, Deserializer, de::Error as DeError};
use smallvec::SmallVec;
use xxhash_rust::xxh3::xxh3_64;

use crate::{
  style::{
    tw::{
      map::{FIXED_PROPERTIES, PREFIX_PARSERS, VAR_TARGETS},
      parser::*,
    },
    *,
  },
  viewport::Viewport,
};

/// Tailwind v4 `--spacing` (rem per unit). Prefer [`Length::from_spacing`].
pub(crate) const TW_VAR_SPACING: f32 = 0.25;

/// The stop list Tailwind compiles gradients to, with each variable's
/// `@property` initial value inlined as its fallback.
const GRADIENT_STOPS: &str = "var(--tw-gradient-via-stops, var(--tw-gradient-from, transparent) var(--tw-gradient-from-position, 0%), var(--tw-gradient-to, transparent) var(--tw-gradient-to-position, 100%))";
const GRADIENT_VIA_STOPS: &str = "var(--tw-gradient-from, transparent) var(--tw-gradient-from-position, 0%), var(--tw-gradient-via) var(--tw-gradient-via-position, 50%), var(--tw-gradient-to, transparent) var(--tw-gradient-to-position, 100%)";

fn push_custom(builder: &mut TailwindDeclarationBuilder, important: bool, name: &str, value: &str) {
  builder.push(
    StyleDeclaration::CustomProperty(name.to_owned(), value.to_owned()),
    important,
  );
}

fn push_gradient_image(builder: &mut TailwindDeclarationBuilder, important: bool, image: String) {
  push_deferred(builder, important, LonghandId::BackgroundImage, image);
}

/// The filter chains Tailwind compiles, in its fixed order. An unset variable
/// collapses to nothing through the empty fallback.
const FILTER_CHAIN: &str = "var(--tw-blur,) var(--tw-brightness,) var(--tw-contrast,) var(--tw-grayscale,) var(--tw-hue-rotate,) var(--tw-invert,) var(--tw-saturate,) var(--tw-sepia,) var(--tw-drop-shadow,)";
const BACKDROP_FILTER_CHAIN: &str = "var(--tw-backdrop-blur,) var(--tw-backdrop-brightness,) var(--tw-backdrop-contrast,) var(--tw-backdrop-grayscale,) var(--tw-backdrop-hue-rotate,) var(--tw-backdrop-invert,) var(--tw-backdrop-opacity,) var(--tw-backdrop-saturate,) var(--tw-backdrop-sepia,)";

const TRANSLATE_PAIR: &str = "var(--tw-translate-x, 0px) var(--tw-translate-y, 0px)";
const SCALE_PAIR: &str = "var(--tw-scale-x, 100%) var(--tw-scale-y, 100%)";

fn push_tw_filter(
  builder: &mut TailwindDeclarationBuilder,
  important: bool,
  backdrop: bool,
  name: &str,
  value: &str,
) {
  let (prefix, longhand, chain) = match backdrop {
    false => ("--tw-", LonghandId::Filter, FILTER_CHAIN),
    true => (
      "--tw-backdrop-",
      LonghandId::BackdropFilter,
      BACKDROP_FILTER_CHAIN,
    ),
  };

  push_custom(builder, important, &format!("{prefix}{name}"), value);
  push_deferred(builder, important, longhand, chain.to_owned());
}

/// `blur(var(--blur-md, 12px))` for a preset, `blur(2px)` for the rest, so a
/// `--blur-*` variable re-shapes the preset the way Tailwind's theme does.
fn blur_css(blur: &TwBlur) -> String {
  match blur.token {
    Some(token) => format!("blur(var(--blur-{token}, {}))", css(&blur.radius)),
    None => css(&Filter::Blur(blur.radius)),
  }
}

/// Expands an `@apply` utility list into the declarations it stands for.
/// The list must arrive comment-free; the stylesheet tokenizer drops them.
/// `None` when a token is unknown or carries a variant, which `@apply` rejects.
pub(crate) fn expand_apply(source: &str) -> Option<StyleDeclarationBlock> {
  let mut builder = TailwindDeclarationBuilder::default();

  for token in source.split_whitespace() {
    let value = TailwindValue::parse(token)?;

    if value.breakpoint.is_some() {
      return None;
    }

    value.property.apply(&mut builder, value.important);
  }

  Some(builder.finish())
}

/// `var(--shadow-md, <built-in layers>)`: the variable overrides the whole
/// shape, and the fallback keeps its per-layer colour slots. A custom shape
/// carries its own colours, so `shadow-*` colour utilities only reach the
/// fallback.
fn preset_shadow_css(variable: &str, layers: impl Iterator<Item = String>) -> String {
  let fallback = layers.collect::<Vec<_>>().join(", ");

  format!("var({variable}, {fallback})")
}

/// The animation list as the shorthand text a `var()` fallback re-parses.
fn animation_shorthand_css(animations: &Animations) -> String {
  animations
    .iter()
    .filter_map(|animation| {
      let name = animation.name.as_deref()?;

      Some(format!(
        "{} {} {} {name}",
        css(&animation.duration),
        css(&animation.timing_function),
        css(&animation.iteration_count),
      ))
    })
    .collect::<Vec<_>>()
    .join(", ")
}

/// `drop-shadow(var(--drop-shadow-md, …))` for a preset; the bare `drop-shadow`
/// utility reads `--drop-shadow` itself.
fn drop_shadow_css(drop_shadow: &TwDropShadow) -> String {
  match drop_shadow.token {
    Some("") => format!(
      "drop-shadow(var(--drop-shadow, {}))",
      css(&drop_shadow.shadow)
    ),
    Some(token) => format!(
      "drop-shadow(var(--drop-shadow-{token}, {}))",
      css(&drop_shadow.shadow)
    ),
    None => css(&Filter::DropShadow(drop_shadow.shadow)),
  }
}

/// One shadow layer with its colour behind `var()`, as Tailwind compiles it:
/// the colour utility overrides through the variable, the layer's own colour
/// stays as the fallback.
fn shadow_layer_css(
  prefix: &str,
  offsets: [&Length; 3],
  color: &ColorInput,
  variable: &str,
) -> String {
  let mut out = String::from(prefix);

  for length in offsets {
    out.push_str(&css(length));
    out.push(' ');
  }

  out.push_str(&format!("var({variable}, {})", css(color)));
  out
}

fn box_shadow_css(shadow: &BoxShadow) -> String {
  let mut layer = shadow_layer_css(
    if shadow.inset { "inset " } else { "" },
    [&shadow.offset_x, &shadow.offset_y, &shadow.blur_radius],
    &shadow.color,
    "--tw-shadow-color",
  );

  let spread = format!("{} ", css(&shadow.spread_radius));

  layer.insert_str(layer.rfind("var(").unwrap_or(0), &spread);
  layer
}

fn text_shadow_css(shadow: &TextShadow) -> String {
  shadow_layer_css(
    "",
    [&shadow.offset_x, &shadow.offset_y, &shadow.blur_radius],
    &shadow.color,
    "--tw-text-shadow-color",
  )
}

fn push_deferred(
  builder: &mut TailwindDeclarationBuilder,
  important: bool,
  longhand: LonghandId,
  specified_value: String,
) {
  builder.push(
    StyleDeclaration::Deferred(DeferredDeclaration {
      property: PropertyId::Longhand(longhand),
      specified_value,
    }),
    important,
  );
}

fn css<T: ToCss>(value: &T) -> String {
  let mut output = String::new();

  let _ = value.to_css(&mut output);
  output
}

/// A class list's expansion, split at the importance boundary.
pub(crate) struct TwBlocks {
  pub(crate) normal: StyleDeclarationBlock,
  pub(crate) important: StyleDeclarationBlock,
}

/// Per-render map from a class list to the blocks it expands to. Keyed by the
/// list's hash and byte length as a cheap collision guard, plus the viewport
/// width, which is all a breakpoint reads.
pub(crate) type TwCache = Rc<RefCell<HashMap<(u64, u32, Option<u32>), Rc<TwBlocks>>>>;

/// Represents a collection of tailwind properties.
#[derive(Debug, Clone, PartialEq)]
pub struct TailwindValues {
  inner: Vec<TailwindValue>,
  /// Hash and byte length of the class list this parsed from.
  fingerprint: (u64, u32),
}

/// How many parsed class lists to keep. A list is small and a document reuses
/// a handful of them; the bound is what keeps a long-lived process flat.
const PARSED_CACHE_ENTRIES: usize = 2048;

static PARSED: LazyLock<Cache<String, Arc<TailwindValues>>> =
  LazyLock::new(|| Cache::new(PARSED_CACHE_ENTRIES));

impl FromStr for TailwindValues {
  type Err = String;

  fn from_str(source: &str) -> Result<Self, Self::Err> {
    Ok(Self::parse(source))
  }
}

impl TailwindValues {
  /// The parsed form of `source`, shared with every node carrying the same
  /// class list. Parsing is pure, so one cache serves every render.
  pub fn interned(source: &str) -> Arc<Self> {
    PARSED
      .get_or_insert_with::<str, Infallible>(source, || Ok(Arc::new(Self::parse(source))))
      .unwrap_or_else(|never| match never {})
  }

  fn parse(source: &str) -> Self {
    let mut collected = source
      .split_whitespace()
      .filter_map(TailwindValue::parse)
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

    TailwindValues {
      inner: collected,
      fingerprint: (xxh3_64(source.as_bytes()), source.len() as u32),
    }
  }
}

impl TailwindValues {
  /// Collects resource URLs referenced by active Tailwind utilities for the given viewport.
  pub(crate) fn image_urls(
    &self,
    viewport: Viewport,
    breakpoints: &BreakpointOverrides,
  ) -> impl Iterator<Item = &str> {
    self
      .inner
      .iter()
      .filter_map(|value| value.resource_url(viewport, breakpoints))
      .collect::<Vec<_>>()
      .into_iter()
  }

  /// The blocks this class list expands to, resolved once per render.
  pub(crate) fn declaration_blocks(
    &self,
    viewport: Viewport,
    breakpoints: &BreakpointOverrides,
    cache: &TwCache,
  ) -> Rc<TwBlocks> {
    let key = (self.fingerprint.0, self.fingerprint.1, viewport.size.width);

    if let Some(blocks) = cache.borrow().get(&key) {
      return blocks.clone();
    }

    let (normal, important) = self
      .clone()
      .into_declaration_block(viewport, breakpoints)
      .split_importance();
    let blocks = Rc::new(TwBlocks { normal, important });

    cache.borrow_mut().insert(key, blocks.clone());

    blocks
  }

  /// Resolves all utilities for the viewport into a declaration block.
  #[inline(never)]
  pub(crate) fn into_declaration_block(
    self,
    viewport: Viewport,
    breakpoints: &BreakpointOverrides,
  ) -> StyleDeclarationBlock {
    let mut builder = TailwindDeclarationBuilder::with_capacity(self.inner.len());

    for value in self.inner {
      value.apply(&mut builder, viewport, breakpoints);
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
  fn resource_url(&self, viewport: Viewport, breakpoints: &BreakpointOverrides) -> Option<&str> {
    if let Some(breakpoint) = &self.breakpoint
      && !breakpoint.matches(viewport, breakpoints)
    {
      return None;
    }

    self.property.resource_url()
  }

  #[inline(never)]
  fn apply(
    self,
    builder: &mut TailwindDeclarationBuilder,
    viewport: Viewport,
    breakpoints: &BreakpointOverrides,
  ) {
    if let Some(breakpoint) = &self.breakpoint
      && !breakpoint.matches(viewport, breakpoints)
    {
      return;
    }

    self.property.apply(builder, self.important);
  }

  /// Parse a tailwind value from a token.
  pub fn parse(mut token: &str) -> Option<Self> {
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
      property: TailwindProperty::parse(token)?,
      breakpoint,
      important,
    })
  }
}

/// Widths a stylesheet's `--breakpoint-*` variables assign, keyed by token.
pub(crate) type BreakpointOverrides = HashMap<String, Length>;

/// Represents a breakpoint.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Breakpoint {
  /// Built-in width; `None` for a token only a `--breakpoint-*` variable defines.
  pub width: Option<Length>,
  /// The variant token, which a `--breakpoint-<token>` variable re-sizes.
  pub token: Arc<str>,
}

impl Breakpoint {
  /// Parse a breakpoint from a token.
  pub fn parse(token: &str) -> Option<Self> {
    let width = match_ignore_ascii_case! {token,
      "sm" => Some(Length::Rem(40.0)),
      "md" => Some(Length::Rem(48.0)),
      "lg" => Some(Length::Rem(64.0)),
      "xl" => Some(Length::Rem(80.0)),
      "2xl" => Some(Length::Rem(96.0)),
      _ => None,
    };

    if width.is_none() && !(!token.is_empty() && token.bytes().all(is_ident_byte)) {
      return None;
    }

    Some(Breakpoint {
      width,
      token: token.to_ascii_lowercase().into(),
    })
  }

  /// Check if the breakpoint matches the viewport width.
  pub fn matches(&self, viewport: Viewport, overrides: &BreakpointOverrides) -> bool {
    let Some(viewport_width) = viewport.size.width else {
      return false;
    };
    let Some(width) = overrides.get(&*self.token).copied().or(self.width) else {
      return false;
    };

    let breakpoint_width = match width {
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
  ShadowColor(TwVarColor),
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
  DropShadow(TwDropShadow),
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
  TextShadowColor(TwVarColor),
  /// `box-shadow` layer set.
  ShadowList(&'static [BoxShadow]),
  /// A shadow preset whose shape a `--shadow-*` / `--inset-shadow-*`
  /// variable overrides wholesale.
  ShadowPreset {
    /// The variable holding the override, e.g. `--shadow-md`.
    variable: &'static str,
    /// Built-in layers serving as the `var()` fallback.
    layers: &'static [BoxShadow],
  },
  /// `text-shadow` layer set.
  TextShadowList(&'static [TextShadow]),
  /// A text-shadow preset whose shape a `--text-shadow-*` variable overrides.
  TextShadowPreset {
    /// The variable holding the override, e.g. `--text-shadow-md`.
    variable: &'static str,
    /// Built-in layers serving as the `var()` fallback.
    layers: &'static [TextShadow],
  },
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
  Animation(TwAnimation),
  /// `bg-linear` property.
  BgLinearAngle(Angle),
  /// `bg-radial` property.
  BgRadial,
  /// `bg-conic` property.
  BgConicAngle(Angle),
  /// `from` property.
  GradientFrom(TwVarColor),
  /// `to` property.
  GradientTo(TwVarColor),
  /// `via` property.
  GradientVia(TwVarColor),
  /// Gradient `from` stop position.
  GradientFromPosition(Length),
  /// Gradient `via` stop position.
  GradientViaPosition(Length),
  /// Gradient `to` stop position.
  GradientToPosition(Length),
  /// A CSS variable standing in for the built-in value it falls back to.
  /// Tailwind compiles a utility to `var(--color-red-500)`, not to the colour.
  VarUtility(VarUtility),
}

/// A utility resolved from custom properties: one [`TwVarRef`] declaration per
/// longhand it writes.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct VarUtility {
  targets: SmallVec<[StyleDeclaration; 2]>,
}

impl VarUtility {
  /// One declaration per longhand, sharing the variable the prefix reads.
  /// `text-lg` spells its line height in a companion variable, so one token can
  /// set the size without dragging the leading with it.
  /// `None` when a declaration has no single longhand to defer (it composes
  /// through custom properties instead), leaving the utility on its built-in value.
  fn from_builtin(
    name: &Arc<str>,
    expression: &Arc<str>,
    declarations: impl IntoIterator<Item = StyleDeclaration>,
  ) -> Option<Self> {
    let mut targets = SmallVec::new();

    for declaration in declarations {
      if matches!(
        declaration,
        StyleDeclaration::CustomProperty(..)
          | StyleDeclaration::Deferred(..)
          | StyleDeclaration::VarRef(..)
      ) {
        return None;
      }

      let longhand = declaration.longhand_id();

      targets.push(var_ref(name, expression, longhand, Some(declaration)));
    }

    Some(Self { targets })
  }

  #[cfg(test)]
  pub(crate) fn builtin_declarations(&self) -> Vec<StyleDeclaration> {
    self
      .targets
      .iter()
      .filter_map(|target| match target {
        StyleDeclaration::VarRef(var_ref) => var_ref.fallback.as_deref().cloned(),
        _ => None,
      })
      .collect()
  }
}

fn var_ref(
  name: &Arc<str>,
  expression: &Arc<str>,
  longhand: LonghandId,
  fallback: Option<StyleDeclaration>,
) -> StyleDeclaration {
  let (name, expression) = match companion_variable(name, longhand) {
    Some(companion) => (companion.clone(), format!("var({companion})")),
    None => (name.clone(), expression.to_string()),
  };

  StyleDeclaration::VarRef(TwVarRef {
    name,
    deferred: DeferredDeclaration {
      property: PropertyId::Longhand(longhand),
      specified_value: expression,
    },
    fallback: fallback.map(Box::new),
  })
}

/// The variable a longhand reads when the token spells it separately from its
/// primary value, as `--text-lg--line-height` does for `--text-lg`.
fn companion_variable(name: &str, longhand: LonghandId) -> Option<Arc<str>> {
  (longhand == LonghandId::LineHeight && name.starts_with(Namespace::Text.prefix()))
    .then(|| format!("{name}--line-height").into())
}

/// The `var()` expression a utility suffix reads, or `None` when the value is
/// spelled in the class itself. Numeric spacing multiplies the `--spacing` step
/// the way Tailwind's own `calc(var(--spacing) * 4)` does.
fn var_expression(
  namespaces: &[Namespace],
  suffix: &str,
  negative: bool,
) -> Option<(Arc<str>, Arc<str>)> {
  if suffix.starts_with('[') {
    return None;
  }

  // `bg-brand-500/50` mixes the variable with transparent, the way Tailwind's
  // own opacity modifier compiles.
  if let Some((token, opacity)) = suffix.split_once('/') {
    let &namespace = namespaces.first()?;
    let percentage = opacity.parse::<f32>().ok()?;

    if namespace != Namespace::Color || !(0.0..=100.0).contains(&percentage) {
      return None;
    }

    let name = format!("{}{token}", namespace.prefix());
    let expression = format!("color-mix(in oklab, var({name}) {percentage}%, transparent)");

    return Some((name.into(), expression.into()));
  }

  // `max-w-4` multiplies `--spacing` where `max-w-prose` reads `--container-prose`.
  if suffix.parse::<f32>().is_ok() && namespaces.contains(&Namespace::Spacing) {
    let sign = if negative { "-" } else { "" };

    return Some((
      "--spacing".into(),
      format!("calc(var(--spacing, {TW_VAR_SPACING}rem) * {sign}{suffix})").into(),
    ));
  }

  let &namespace = namespaces.first()?;

  let name = format!("{}{suffix}", namespace.prefix());
  let expression = match negative {
    true => format!("calc(var({name}) * -1)"),
    false => format!("var({name})"),
  };

  Some((name.into(), expression.into()))
}

pub(crate) fn extract_arbitrary_value(suffix: &str) -> Option<Cow<'_, str>> {
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

pub(crate) fn is_ident_byte(byte: u8) -> bool {
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

  /// Variable namespaces this value type reads, tried in order. The utility keeps
  /// the built-in value as the fallback behind `var()`, so this only decides
  /// which variable name the utility reads.
  const NAMESPACES: &'static [Namespace] = &[];

  /// Parse a tailwind property from a token, with support for arbitrary values.
  fn parse_tw_with_arbitrary(token: &str) -> Option<Self> {
    if let Some(value) = extract_arbitrary_value(token) {
      return Self::from_css_str(&value).ok();
    }

    Self::parse_tw(token)
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
  pub fn parse(token: &str) -> Option<TailwindProperty> {
    // Check fixed properties first
    if let Some(property) = FIXED_PROPERTIES.get(token) {
      return Some(property.clone());
    }

    match token.strip_prefix('-') {
      Some(stripped) => Self::parse_prefix_suffix(stripped, true),
      None => Self::parse_prefix_suffix(token, false),
    }
  }

  /// The longhands a property writes, paired with the value each falls back to.
  /// Expanding it once here keeps the builder out of the per-node apply path.
  pub(crate) fn expand_targets(self) -> SmallVec<[StyleDeclaration; 2]> {
    let mut probe = TailwindDeclarationBuilder::default();

    self.apply(&mut probe, false);

    probe.finish().iter().cloned().collect()
  }

  fn parse_prefix_suffix(token: &str, negative: bool) -> Option<TailwindProperty> {
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
        let Some(property) = parser.parse(suffix) else {
          continue;
        };

        let property = if negative {
          property.try_neg()?
        } else {
          property
        };

        if let Some((name, expression)) = var_expression(parser.namespaces(), suffix, negative)
          && let Some(var_utility) =
            VarUtility::from_builtin(&name, &expression, property.clone().expand_targets())
        {
          return Some(TailwindProperty::VarUtility(var_utility));
        }

        return Some(property);
      }

      // No built-in value, but the prefix still names what the utility writes.
      if let Some(groups) = VAR_TARGETS.get(prefix) {
        let targets: SmallVec<[StyleDeclaration; 2]> = groups
          .iter()
          .filter_map(|(namespace, longhands)| {
            let (name, expression) = var_expression(&[*namespace], suffix, negative)?;

            Some(
              longhands
                .iter()
                .map(move |&longhand| var_ref(&name, &expression, longhand, None)),
            )
          })
          .flatten()
          .collect();

        if !targets.is_empty() {
          return Some(TailwindProperty::VarUtility(VarUtility { targets }));
        }
      }
    }

    None
  }

  #[inline(never)]
  fn apply(self, builder: &mut TailwindDeclarationBuilder, important: bool) {
    match self {
      TailwindProperty::VarUtility(var_utility) => {
        for target in var_utility.targets {
          builder.push(target, important);
        }
      }
      TailwindProperty::BgLinearAngle(angle) => {
        push_gradient_image(
          builder,
          important,
          format!(
            "linear-gradient({}deg in oklab, var(--tw-gradient-stops))",
            *angle
          ),
        );
      }
      TailwindProperty::BgRadial => {
        push_gradient_image(
          builder,
          important,
          "radial-gradient(in oklab, var(--tw-gradient-stops))".to_owned(),
        );
      }
      TailwindProperty::BgConicAngle(angle) => {
        let image = if *angle == 0.0 {
          "conic-gradient(in oklab, var(--tw-gradient-stops))".to_owned()
        } else {
          format!(
            "conic-gradient(from {}deg in oklab, var(--tw-gradient-stops))",
            *angle
          )
        };

        push_gradient_image(builder, important, image);
      }
      TailwindProperty::GradientFrom(color) => {
        push_custom(builder, important, "--tw-gradient-from", &color.0);
        push_custom(builder, important, "--tw-gradient-stops", GRADIENT_STOPS);
      }
      TailwindProperty::GradientTo(color) => {
        push_custom(builder, important, "--tw-gradient-to", &color.0);
        push_custom(builder, important, "--tw-gradient-stops", GRADIENT_STOPS);
      }
      TailwindProperty::GradientVia(color) => {
        push_custom(builder, important, "--tw-gradient-via", &color.0);
        push_custom(
          builder,
          important,
          "--tw-gradient-via-stops",
          GRADIENT_VIA_STOPS,
        );
        push_custom(
          builder,
          important,
          "--tw-gradient-stops",
          "var(--tw-gradient-via-stops)",
        );
      }
      TailwindProperty::GradientFromPosition(pos) => {
        push_custom(
          builder,
          important,
          "--tw-gradient-from-position",
          &css(&pos),
        );
      }
      TailwindProperty::GradientViaPosition(pos) => {
        push_custom(builder, important, "--tw-gradient-via-position", &css(&pos));
      }
      TailwindProperty::GradientToPosition(pos) => {
        push_custom(builder, important, "--tw-gradient-to-position", &css(&pos));
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
      TailwindProperty::Shadow(box_shadow) => {
        push_deferred(
          builder,
          important,
          LonghandId::BoxShadow,
          box_shadow_css(&box_shadow),
        );
      }
      TailwindProperty::ShadowList(&[]) => {
        push_deferred(builder, important, LonghandId::BoxShadow, "none".to_owned());
      }
      TailwindProperty::ShadowList(layers) => {
        let layers: Vec<String> = layers.iter().map(box_shadow_css).collect();

        push_deferred(builder, important, LonghandId::BoxShadow, layers.join(", "));
      }
      TailwindProperty::ShadowPreset { variable, layers } => {
        push_deferred(
          builder,
          important,
          LonghandId::BoxShadow,
          preset_shadow_css(variable, layers.iter().map(box_shadow_css)),
        );
      }
      TailwindProperty::ShadowColor(color) => {
        push_custom(builder, important, "--tw-shadow-color", &color.0);
      }
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
        push_custom(builder, important, "--tw-translate-x", &css(&length));
        push_custom(builder, important, "--tw-translate-y", &css(&length));
        push_deferred(
          builder,
          important,
          LonghandId::Translate,
          TRANSLATE_PAIR.to_owned(),
        );
      }
      TailwindProperty::TranslateX(length) => {
        push_custom(builder, important, "--tw-translate-x", &css(&length));
        push_deferred(
          builder,
          important,
          LonghandId::Translate,
          TRANSLATE_PAIR.to_owned(),
        );
      }
      TailwindProperty::TranslateY(length) => {
        push_custom(builder, important, "--tw-translate-y", &css(&length));
        push_deferred(
          builder,
          important,
          LonghandId::Translate,
          TRANSLATE_PAIR.to_owned(),
        );
      }
      TailwindProperty::Rotate(angle) => push_decl!(builder, important, rotate(Some(angle))),
      TailwindProperty::Scale(percentage_number) => {
        push_custom(builder, important, "--tw-scale-x", &css(&percentage_number));
        push_custom(builder, important, "--tw-scale-y", &css(&percentage_number));
        push_deferred(builder, important, LonghandId::Scale, SCALE_PAIR.to_owned());
      }
      TailwindProperty::ScaleX(percentage_number) => {
        push_custom(builder, important, "--tw-scale-x", &css(&percentage_number));
        push_deferred(builder, important, LonghandId::Scale, SCALE_PAIR.to_owned());
      }
      TailwindProperty::ScaleY(percentage_number) => {
        push_custom(builder, important, "--tw-scale-y", &css(&percentage_number));
        push_deferred(builder, important, LonghandId::Scale, SCALE_PAIR.to_owned());
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
      TailwindProperty::GridColumn(grid_line) => push_decl!(
        builder,
        important,
        grid_column_start(grid_line.start),
        grid_column_end(grid_line.end)
      ),
      TailwindProperty::GridRow(grid_line) => push_decl!(
        builder,
        important,
        grid_row_start(grid_line.start),
        grid_row_end(grid_line.end)
      ),
      TailwindProperty::GridColumnStart(tw_grid_placement) => {
        push_decl!(builder, important, grid_column_start(tw_grid_placement))
      }
      TailwindProperty::GridColumnEnd(tw_grid_placement) => {
        push_decl!(builder, important, grid_column_end(tw_grid_placement))
      }
      TailwindProperty::GridRowStart(tw_grid_placement) => {
        push_decl!(builder, important, grid_row_start(tw_grid_placement))
      }
      TailwindProperty::GridRowEnd(tw_grid_placement) => {
        push_decl!(builder, important, grid_row_end(tw_grid_placement))
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
        let line = GridLine::span(grid_placement_span);

        push_decl!(
          builder,
          important,
          grid_column_start(line.start),
          grid_column_end(line.end)
        )
      }
      TailwindProperty::GridRowSpan(grid_placement_span) => {
        let line = GridLine::span(grid_placement_span);

        push_decl!(
          builder,
          important,
          grid_row_start(line.start),
          grid_row_end(line.end)
        )
      }
      TailwindProperty::Blur(tw_blur) => {
        push_tw_filter(builder, important, false, "blur", &blur_css(&tw_blur));
      }
      TailwindProperty::Brightness(percentage_number) => {
        push_tw_filter(
          builder,
          important,
          false,
          "brightness",
          &css(&Filter::Brightness(percentage_number)),
        );
      }
      TailwindProperty::Contrast(percentage_number) => {
        push_tw_filter(
          builder,
          important,
          false,
          "contrast",
          &css(&Filter::Contrast(percentage_number)),
        );
      }
      TailwindProperty::DropShadow(drop_shadow) => {
        push_tw_filter(
          builder,
          important,
          false,
          "drop-shadow",
          &drop_shadow_css(&drop_shadow),
        );
      }
      TailwindProperty::Grayscale(percentage_number) => {
        push_tw_filter(
          builder,
          important,
          false,
          "grayscale",
          &css(&Filter::Grayscale(percentage_number)),
        );
      }
      TailwindProperty::HueRotate(angle) => {
        push_tw_filter(
          builder,
          important,
          false,
          "hue-rotate",
          &css(&Filter::HueRotate(angle)),
        );
      }
      TailwindProperty::Invert(percentage_number) => {
        push_tw_filter(
          builder,
          important,
          false,
          "invert",
          &css(&Filter::Invert(percentage_number)),
        );
      }
      TailwindProperty::Saturate(percentage_number) => {
        push_tw_filter(
          builder,
          important,
          false,
          "saturate",
          &css(&Filter::Saturate(percentage_number)),
        );
      }
      TailwindProperty::Sepia(percentage_number) => {
        push_tw_filter(
          builder,
          important,
          false,
          "sepia",
          &css(&Filter::Sepia(percentage_number)),
        );
      }
      TailwindProperty::Filter(filters) => {
        if filters.is_empty() {
          push_deferred(builder, important, LonghandId::Filter, "none".to_owned());
        } else {
          push_decl!(builder, important, filter(filters));
        }
      }
      TailwindProperty::BackdropBlur(tw_blur) => {
        push_tw_filter(builder, important, true, "blur", &blur_css(&tw_blur));
      }
      TailwindProperty::BackdropBrightness(percentage_number) => {
        push_tw_filter(
          builder,
          important,
          true,
          "brightness",
          &css(&Filter::Brightness(percentage_number)),
        );
      }
      TailwindProperty::BackdropContrast(percentage_number) => {
        push_tw_filter(
          builder,
          important,
          true,
          "contrast",
          &css(&Filter::Contrast(percentage_number)),
        );
      }
      TailwindProperty::BackdropGrayscale(percentage_number) => {
        push_tw_filter(
          builder,
          important,
          true,
          "grayscale",
          &css(&Filter::Grayscale(percentage_number)),
        );
      }
      TailwindProperty::BackdropHueRotate(angle) => {
        push_tw_filter(
          builder,
          important,
          true,
          "hue-rotate",
          &css(&Filter::HueRotate(angle)),
        );
      }
      TailwindProperty::BackdropInvert(percentage_number) => {
        push_tw_filter(
          builder,
          important,
          true,
          "invert",
          &css(&Filter::Invert(percentage_number)),
        );
      }
      TailwindProperty::BackdropOpacity(percentage_number) => {
        push_tw_filter(
          builder,
          important,
          true,
          "opacity",
          &css(&Filter::Opacity(percentage_number)),
        );
      }
      TailwindProperty::BackdropSaturate(percentage_number) => {
        push_tw_filter(
          builder,
          important,
          true,
          "saturate",
          &css(&Filter::Saturate(percentage_number)),
        );
      }
      TailwindProperty::BackdropSepia(percentage_number) => {
        push_tw_filter(
          builder,
          important,
          true,
          "sepia",
          &css(&Filter::Sepia(percentage_number)),
        );
      }
      TailwindProperty::BackdropFilter(filters) => {
        if filters.is_empty() {
          push_deferred(
            builder,
            important,
            LonghandId::BackdropFilter,
            "none".to_owned(),
          );
        } else {
          push_decl!(builder, important, backdrop_filter(filters));
        }
      }
      TailwindProperty::TextShadow(text_shadow) => {
        push_deferred(
          builder,
          important,
          LonghandId::TextShadow,
          text_shadow_css(&text_shadow),
        );
      }
      TailwindProperty::TextShadowList(&[]) => {
        push_deferred(
          builder,
          important,
          LonghandId::TextShadow,
          "none".to_owned(),
        );
      }
      TailwindProperty::TextShadowList(layers) => {
        let layers: Vec<String> = layers.iter().map(text_shadow_css).collect();

        push_deferred(
          builder,
          important,
          LonghandId::TextShadow,
          layers.join(", "),
        );
      }
      TailwindProperty::TextShadowPreset { variable, layers } => {
        push_deferred(
          builder,
          important,
          LonghandId::TextShadow,
          preset_shadow_css(variable, layers.iter().map(text_shadow_css)),
        );
      }
      TailwindProperty::TextShadowColor(color) => {
        push_custom(builder, important, "--tw-text-shadow-color", &color.0);
      }
      TailwindProperty::Visibility(visibility) => {
        push_decl!(builder, important, visibility(visibility))
      }
      TailwindProperty::Animation(tw_animation) => {
        let TwAnimation { animations, token } = tw_animation;

        if let Some(token) = token {
          let fallback = animation_shorthand_css(&animations);
          let specified_value = if fallback.is_empty() {
            format!("var(--animate-{token})")
          } else {
            format!("var(--animate-{token}, {fallback})")
          };

          builder.push(
            StyleDeclaration::Deferred(DeferredDeclaration {
              property: PropertyId::Shorthand(ShorthandId::Animation),
              specified_value,
            }),
            important,
          );
          return;
        }

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
