pub(crate) mod map;
pub(crate) mod parser;

use std::{borrow::Cow, cmp::Ordering, ops::Neg, str::FromStr};

use cssparser::match_ignore_ascii_case;
use serde::{Deserializer, de::Error as DeError};

use crate::layout::{
  Viewport,
  style::{
    tw::{
      map::{FIXED_PROPERTIES, PREFIX_PARSERS},
      parser::*,
    },
    *,
  },
};

/// Tailwind `--spacing` variable value.
pub const TW_VAR_SPACING: f32 = 0.25;

/// Represents a collection of tailwind properties.
#[derive(Debug, Clone, PartialEq)]
pub struct TailwindValues {
  inner: Vec<TailwindValue>,
}

impl FromStr for TailwindValues {
  type Err = String;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    let mut collected = s
      .split_whitespace()
      .filter_map(TailwindValue::parse)
      .collect::<Vec<_>>();

    // sort in reverse order by is important, then has breakpoint, then rest is last.
    collected.sort_unstable_by(|a, b| {
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

    Ok(TailwindValues { inner: collected })
  }
}

impl TailwindValues {
  /// Iterate over the tailwind values.
  pub fn iter(&self) -> impl Iterator<Item = &TailwindValue> {
    self.inner.iter()
  }

  pub(crate) fn into_declaration_block(self, viewport: Viewport) -> StyleDeclarationBlock {
    let mut builder = TailwindDeclarationBuilder::default();

    for value in self.inner {
      value.apply(&mut builder, viewport);
    }

    builder.finish()
  }
}

#[derive(Debug, Default)]
struct TailwindDeclarationBuilder {
  declarations: StyleDeclarationBlock,
  gradient_state: TwGradientState,
  transform_state: TwTransformState,
  filter: Option<Filters>,
  filter_important: bool,
  backdrop_filter: Option<Filters>,
  backdrop_filter_important: bool,
  grid_column: Option<GridLine>,
  grid_column_important: bool,
  grid_row: Option<GridLine>,
  grid_row_important: bool,
}

impl TailwindDeclarationBuilder {
  fn push(&mut self, declaration: StyleDeclaration, important: bool) {
    self.declarations.push(declaration, important);
  }

  fn push_filter(&mut self, filter: Filter, important: bool) {
    self.filter.get_or_insert_with(Vec::new).push(filter);
    self.filter_important = important;
  }

  fn push_backdrop_filter(&mut self, filter: Filter, important: bool) {
    self
      .backdrop_filter
      .get_or_insert_with(Vec::new)
      .push(filter);
    self.backdrop_filter_important = important;
  }

  fn set_grid_column(&mut self, grid_line: GridLine, important: bool) {
    self.grid_column = Some(grid_line);
    self.grid_column_important = important;
  }

  fn set_grid_row(&mut self, grid_line: GridLine, important: bool) {
    self.grid_row = Some(grid_line);
    self.grid_row_important = important;
  }

  fn grid_column_mut(&mut self, important: bool) -> &mut GridLine {
    self.grid_column_important = important;
    self.grid_column.get_or_insert_with(GridLine::default)
  }

  fn grid_row_mut(&mut self, important: bool) -> &mut GridLine {
    self.grid_row_important = important;
    self.grid_row.get_or_insert_with(GridLine::default)
  }

  fn finish(mut self) -> StyleDeclarationBlock {
    if let Some(grid_column) = self.grid_column.take() {
      self.push(
        StyleDeclaration::grid_column(Some(grid_column)),
        self.grid_column_important,
      );
    }

    if let Some(grid_row) = self.grid_row.take() {
      self.push(
        StyleDeclaration::grid_row(Some(grid_row)),
        self.grid_row_important,
      );
    }

    if let Some(filter) = self.filter.take() {
      self.push(StyleDeclaration::filter(filter), self.filter_important);
    }

    if let Some(backdrop_filter) = self.backdrop_filter.take() {
      self.push(
        StyleDeclaration::backdrop_filter(backdrop_filter),
        self.backdrop_filter_important,
      );
    }

    self.transform_state.apply(&mut self.declarations);
    self.gradient_state.apply(&mut self.declarations);
    self.declarations
  }
}

#[derive(Debug, Default)]
struct TwTransformState {
  translate: Option<SpacePair<Length>>,
  translate_important: bool,
  scale: Option<SpacePair<PercentageNumber>>,
  scale_important: bool,
}

impl TwTransformState {
  fn set_translate(&mut self, value: SpacePair<Length>, important: bool) {
    self.translate = Some(value);
    self.translate_important = important;
  }

  fn translate_mut(&mut self, important: bool) -> &mut SpacePair<Length> {
    self.translate_important = important;
    self
      .translate
      .get_or_insert_with(SpacePair::<Length>::default)
  }

  fn set_scale(&mut self, value: SpacePair<PercentageNumber>, important: bool) {
    self.scale = Some(value);
    self.scale_important = important;
  }

  fn scale_mut(&mut self, important: bool) -> &mut SpacePair<PercentageNumber> {
    self.scale_important = important;
    self
      .scale
      .get_or_insert_with(SpacePair::<PercentageNumber>::default)
  }

  fn apply(self, declarations: &mut StyleDeclarationBlock) {
    if let Some(translate) = self.translate {
      declarations.push(
        StyleDeclaration::translate(translate),
        self.translate_important,
      );
    }

    if let Some(scale) = self.scale {
      declarations.push(StyleDeclaration::scale(scale), self.scale_important);
    }
  }
}

#[derive(Debug, Default)]
pub(crate) struct TwGradientState {
  pub gradient_type: TwGradientType,
  pub angle: Option<Angle>,
  pub from: Option<ColorInput>,
  pub to: Option<ColorInput>,
  pub via: Option<ColorInput>,
  pub important: bool,
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub(crate) enum TwGradientType {
  #[default]
  Linear,
  Radial,
  Conic,
}

impl TwGradientState {
  pub(crate) fn apply(self, declarations: &mut StyleDeclarationBlock) {
    if self.from.is_none() && self.to.is_none() && self.via.is_none() && self.angle.is_none() {
      return;
    }

    let angle = self.angle.unwrap_or_else(|| Angle::new(180.0));

    let from_color = self.from.unwrap_or(ColorInput::Value(Color([0, 0, 0, 0])));
    let to_color = self.to.unwrap_or_else(|| {
      if let ColorInput::Value(from_c) = from_color {
        ColorInput::Value(Color([from_c.0[0], from_c.0[1], from_c.0[2], 0]))
      } else {
        ColorInput::Value(Color([0, 0, 0, 0]))
      }
    });

    let mut stops = Vec::new();
    stops.push(GradientStop::ColorHint {
      color: from_color,
      hint: Some(StopPosition(Length::Percentage(0.0))),
    });

    if let Some(via_color) = self.via {
      stops.push(GradientStop::ColorHint {
        color: via_color,
        hint: Some(StopPosition(Length::Percentage(50.0))),
      });
    }

    stops.push(GradientStop::ColorHint {
      color: to_color,
      hint: Some(StopPosition(Length::Percentage(100.0))),
    });

    match self.gradient_type {
      TwGradientType::Linear => {
        let gradient = LinearGradient {
          angle,
          interpolation: ColorInterpolationMethod::default(),
          stops: stops.into_boxed_slice(),
        };

        declarations.push(
          StyleDeclaration::background_image(Some([BackgroundImage::Linear(gradient)].into())),
          self.important,
        );
      }
      TwGradientType::Radial => {
        let gradient = RadialGradient {
          shape: RadialShape::Ellipse,
          size: RadialSize::FarthestCorner,
          center: BackgroundPosition::default(),
          interpolation: ColorInterpolationMethod::default(),
          stops: stops.into_boxed_slice(),
        };

        declarations.push(
          StyleDeclaration::background_image(Some([BackgroundImage::Radial(gradient)].into())),
          self.important,
        );
      }
      TwGradientType::Conic => {
        let gradient = ConicGradient {
          from_angle: angle,
          center: BackgroundPosition::default(),
          interpolation: ColorInterpolationMethod::default(),
          stops: stops.into_boxed_slice(),
        };

        declarations.push(
          StyleDeclaration::background_image(Some([BackgroundImage::Conic(gradient)].into())),
          self.important,
        );
      }
    }
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
pub struct TailwindValue {
  /// The tailwind property.
  pub property: TailwindProperty,
  /// The breakpoint.
  pub breakpoint: Option<Breakpoint>,
  /// Whether the value is important.
  pub important: bool,
}

impl TailwindValue {
  fn apply(self, builder: &mut TailwindDeclarationBuilder, viewport: Viewport) {
    if let Some(breakpoint) = self.breakpoint
      && !breakpoint.matches(viewport)
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
    if let Some((breakpoint_token, rest)) = token.split_once(':') {
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

/// Represents a breakpoint.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Breakpoint(pub(crate) Length);

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
    let Some(viewport_width) = viewport.width else {
      return false;
    };

    let breakpoint_width = match self.0 {
      Length::Rem(value) => value * viewport.font_size * viewport.device_pixel_ratio,
      Length::Px(value) => value * viewport.device_pixel_ratio,
      Length::Vw(value) => (value / 100.0) * viewport_width as f32,
      _ => 0.0,
    };

    viewport_width >= breakpoint_width as u32
  }
}

/// Represents a tailwind property.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum TailwindProperty {
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
  /// `display` property.
  Display(Display),
  /// `object-position` property.
  ObjectPosition(ObjectPosition),
  /// `object-fit` property.
  ObjectFit(ObjectFit),
  /// `background-position` property.
  BackgroundPosition(BackgroundPosition),
  /// `background-size` property.
  BackgroundSize(BackgroundSize),
  /// `background-repeat` property.
  BackgroundRepeat(BackgroundRepeat),
  /// `background-image` property.
  BackgroundImage(BackgroundImage),
  /// `gap` property.
  Gap(Length<false>),
  /// `column-gap` property.
  GapX(Length<false>),
  /// `row-gap` property.
  GapY(Length<false>),
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
  BorderWidth(TwBorderWidth),
  /// `border-style` property.
  BorderStyle(BorderStyle),
  /// `color` property.
  Color(ColorInput),
  /// `opacity` property.
  Opacity(PercentageNumber),
  /// `background-color` property.
  BackgroundColor(ColorInput<false>),
  /// `border-color` property.
  BorderColor(ColorInput),
  /// `border-top-width` property.
  BorderTopWidth(TwBorderWidth),
  /// `border-right-width` property.
  BorderRightWidth(TwBorderWidth),
  /// `border-bottom-width` property.
  BorderBottomWidth(TwBorderWidth),
  /// `border-left-width` property.
  BorderLeftWidth(TwBorderWidth),
  /// `border-inline-width` property.
  BorderXWidth(TwBorderWidth),
  /// `border-block-width` property.
  BorderYWidth(TwBorderWidth),
  /// Tailwind `outline` utility (`outline-width: 1px; outline-style: solid`).
  OutlineDefault,
  /// `outline-width` property.
  OutlineWidth(TwBorderWidth),
  /// `outline-color` property.
  OutlineColor(ColorInput),
  /// `outline-style` property.
  OutlineStyle(BorderStyle),
  /// `outline-offset` property.
  OutlineOffset(TwBorderWidth),
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
  TransformOrigin(TransformOrigin),
  /// `margin` property.
  Margin(Length<false>),
  /// `margin-inline` property.
  MarginX(Length<false>),
  /// `margin-block` property.
  MarginY(Length<false>),
  /// `margin-top` property.
  MarginTop(Length<false>),
  /// `margin-right` property.
  MarginRight(Length<false>),
  /// `margin-bottom` property.
  MarginBottom(Length<false>),
  /// `margin-left` property.
  MarginLeft(Length<false>),
  /// `padding` property.
  Padding(Length<false>),
  /// `padding-inline` property.
  PaddingX(Length<false>),
  /// `padding-block` property.
  PaddingY(Length<false>),
  /// `padding-top` property.
  PaddingTop(Length<false>),
  /// `padding-right` property.
  PaddingRight(Length<false>),
  /// `padding-bottom` property.
  PaddingBottom(Length<false>),
  /// `padding-left` property.
  PaddingLeft(Length<false>),
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
}

fn extract_arbitrary_value(suffix: &str) -> Option<Cow<'_, str>> {
  if suffix.starts_with('[') && suffix.ends_with(']') {
    let value = &suffix[1..suffix.len() - 1];
    if value.contains('_') {
      Some(Cow::Owned(value.replace('_', " ")))
    } else {
      Some(Cow::Borrowed(value))
    }
  } else {
    None
  }
}

/// A trait for parsing tailwind properties.
pub trait TailwindPropertyParser: Sized + for<'i> FromCss<'i> {
  /// Parse a tailwind property from a token.
  fn parse_tw(token: &str) -> Option<Self>;

  /// Parse a tailwind property from a token, with support for arbitrary values.
  fn parse_tw_with_arbitrary(token: &str) -> Option<Self> {
    if let Some(value) = extract_arbitrary_value(token) {
      return Self::from_str(&value).ok();
    }

    Self::parse_tw(token)
  }
}

impl Neg for TailwindProperty {
  type Output = Self;

  fn neg(self) -> Self::Output {
    match self {
      TailwindProperty::Margin(length) => TailwindProperty::Margin(-length),
      TailwindProperty::MarginX(length) => TailwindProperty::MarginX(-length),
      TailwindProperty::MarginY(length) => TailwindProperty::MarginY(-length),
      TailwindProperty::MarginTop(length) => TailwindProperty::MarginTop(-length),
      TailwindProperty::MarginRight(length) => TailwindProperty::MarginRight(-length),
      TailwindProperty::MarginBottom(length) => TailwindProperty::MarginBottom(-length),
      TailwindProperty::MarginLeft(length) => TailwindProperty::MarginLeft(-length),
      TailwindProperty::Padding(length) => TailwindProperty::Padding(-length),
      TailwindProperty::PaddingX(length) => TailwindProperty::PaddingX(-length),
      TailwindProperty::PaddingY(length) => TailwindProperty::PaddingY(-length),
      TailwindProperty::PaddingTop(length) => TailwindProperty::PaddingTop(-length),
      TailwindProperty::PaddingRight(length) => TailwindProperty::PaddingRight(-length),
      TailwindProperty::PaddingBottom(length) => TailwindProperty::PaddingBottom(-length),
      TailwindProperty::PaddingLeft(length) => TailwindProperty::PaddingLeft(-length),
      TailwindProperty::Inset(length) => TailwindProperty::Inset(-length),
      TailwindProperty::InsetX(length) => TailwindProperty::InsetX(-length),
      TailwindProperty::InsetY(length) => TailwindProperty::InsetY(-length),
      TailwindProperty::Top(length) => TailwindProperty::Top(-length),
      TailwindProperty::Right(length) => TailwindProperty::Right(-length),
      TailwindProperty::Bottom(length) => TailwindProperty::Bottom(-length),
      TailwindProperty::Left(length) => TailwindProperty::Left(-length),
      TailwindProperty::Translate(length) => TailwindProperty::Translate(-length),
      TailwindProperty::TranslateX(length) => TailwindProperty::TranslateX(-length),
      TailwindProperty::TranslateY(length) => TailwindProperty::TranslateY(-length),
      TailwindProperty::Scale(percentage_number) => TailwindProperty::Scale(-percentage_number),
      TailwindProperty::ScaleX(percentage_number) => TailwindProperty::ScaleX(-percentage_number),
      TailwindProperty::ScaleY(percentage_number) => TailwindProperty::ScaleY(-percentage_number),
      TailwindProperty::Rotate(angle) => TailwindProperty::Rotate(-angle),
      TailwindProperty::LetterSpacing(length) => TailwindProperty::LetterSpacing(-length),
      TailwindProperty::HueRotate(angle) => TailwindProperty::HueRotate(-angle),
      TailwindProperty::BackdropHueRotate(angle) => TailwindProperty::BackdropHueRotate(-angle),
      _ => self,
    }
  }
}

macro_rules! push_decl {
  ($builder:expr, $important:expr, $property:ident($value:expr)) => {
    $builder.push(StyleDeclaration::$property($value), $important)
  };
}

impl TailwindProperty {
  /// Parse a single tailwind property from a token.
  pub fn parse(token: &str) -> Option<TailwindProperty> {
    // Check fixed properties first
    if let Some(property) = FIXED_PROPERTIES.get(token) {
      return Some(property.clone());
    }

    // Handle negative values like "-top-4"
    if let Some(stripped) = token.strip_prefix('-') {
      if let Some(property) = Self::parse_prefix_suffix(stripped) {
        return Some(-property);
      }

      return None;
    }

    Self::parse_prefix_suffix(token)
  }

  fn parse_prefix_suffix(token: &str) -> Option<TailwindProperty> {
    let dash_positions = token.match_indices('-').map(|(i, _)| i);

    // Try different prefix lengths (longest first)
    for dash_pos in dash_positions.rev() {
      let prefix = &token[..dash_pos];

      let Some(parsers) = PREFIX_PARSERS.get(prefix) else {
        continue;
      };

      let suffix = &token[dash_pos + 1..];

      for parser in *parsers {
        if let Some(property) = parser.parse(suffix) {
          return Some(property);
        }
      }
    }

    None
  }

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
      TailwindProperty::BackgroundClip(background_clip) => {
        push_decl!(builder, important, background_clip(background_clip));
      }
      TailwindProperty::Gap(gap) => {
        push_decl!(builder, important, row_gap(gap));
        push_decl!(builder, important, column_gap(gap));
      }
      TailwindProperty::GapX(gap_x) => push_decl!(builder, important, column_gap(gap_x)),
      TailwindProperty::GapY(gap_y) => push_decl!(builder, important, row_gap(gap_y)),
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
        push_decl!(builder, important, flex_grow(Some(FlexGrow(flex.grow))));
        push_decl!(builder, important, flex_shrink(Some(FlexGrow(flex.shrink))));
        push_decl!(builder, important, flex_basis(Some(flex.basis)));
      }
      TailwindProperty::FlexBasis(flex_basis) => {
        push_decl!(builder, important, flex_basis(Some(flex_basis)))
      }
      TailwindProperty::Overflow(overflow) => {
        push_decl!(builder, important, overflow_x(overflow));
        push_decl!(builder, important, overflow_y(overflow));
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
        push_decl!(builder, important, font_family(Some(font_family)))
      }
      TailwindProperty::LineClamp(line_clamp) => {
        push_decl!(builder, important, line_clamp(Some(line_clamp)))
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
        push_decl!(builder, important, width(size));
        push_decl!(builder, important, height(size));
      }
      TailwindProperty::Width(width) => push_decl!(builder, important, width(width)),
      TailwindProperty::Height(height) => push_decl!(builder, important, height(height)),
      TailwindProperty::MinWidth(min_width) => push_decl!(builder, important, min_width(min_width)),
      TailwindProperty::MinHeight(min_height) => {
        push_decl!(builder, important, min_height(min_height))
      }
      TailwindProperty::MaxWidth(max_width) => push_decl!(builder, important, max_width(max_width)),
      TailwindProperty::MaxHeight(max_height) => {
        push_decl!(builder, important, max_height(max_height))
      }
      TailwindProperty::Shadow(box_shadow) => {
        push_decl!(builder, important, box_shadow(Some([box_shadow].into())))
      }
      TailwindProperty::Display(display) => push_decl!(builder, important, display(display)),
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
      TailwindProperty::BorderDefault => {
        push_decl!(builder, important, border_top_width(Length::Px(1.0)));
        push_decl!(builder, important, border_right_width(Length::Px(1.0)));
        push_decl!(builder, important, border_bottom_width(Length::Px(1.0)));
        push_decl!(builder, important, border_left_width(Length::Px(1.0)));
        push_decl!(builder, important, border_style(BorderStyle::Solid));
      }
      TailwindProperty::BorderWidth(tw_border_width) => {
        push_decl!(builder, important, border_top_width(tw_border_width.0));
        push_decl!(builder, important, border_right_width(tw_border_width.0));
        push_decl!(builder, important, border_bottom_width(tw_border_width.0));
        push_decl!(builder, important, border_left_width(tw_border_width.0));
      }
      TailwindProperty::BorderStyle(border_style) => {
        push_decl!(builder, important, border_style(border_style))
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
        push_decl!(builder, important, border_color(color_input))
      }
      TailwindProperty::BorderTopWidth(tw_border_width) => {
        push_decl!(builder, important, border_top_width(tw_border_width.0))
      }
      TailwindProperty::BorderRightWidth(tw_border_width) => {
        push_decl!(builder, important, border_right_width(tw_border_width.0))
      }
      TailwindProperty::BorderBottomWidth(tw_border_width) => {
        push_decl!(builder, important, border_bottom_width(tw_border_width.0))
      }
      TailwindProperty::BorderLeftWidth(tw_border_width) => {
        push_decl!(builder, important, border_left_width(tw_border_width.0))
      }
      TailwindProperty::BorderXWidth(tw_border_width) => {
        push_decl!(builder, important, border_left_width(tw_border_width.0));
        push_decl!(builder, important, border_right_width(tw_border_width.0));
      }
      TailwindProperty::BorderYWidth(tw_border_width) => {
        push_decl!(builder, important, border_top_width(tw_border_width.0));
        push_decl!(builder, important, border_bottom_width(tw_border_width.0));
      }
      TailwindProperty::OutlineDefault => {
        push_decl!(builder, important, outline_width(Length::Px(1.0)));
        push_decl!(builder, important, outline_style(BorderStyle::Solid));
      }
      TailwindProperty::OutlineWidth(tw_border_width) => {
        push_decl!(builder, important, outline_width(tw_border_width.0))
      }
      TailwindProperty::OutlineColor(color_input) => {
        push_decl!(builder, important, outline_color(color_input))
      }
      TailwindProperty::OutlineStyle(outline_style) => {
        push_decl!(builder, important, outline_style(outline_style))
      }
      TailwindProperty::OutlineOffset(outline_offset) => {
        push_decl!(builder, important, outline_offset(outline_offset.0))
      }
      TailwindProperty::Rounded(rounded) => {
        push_decl!(
          builder,
          important,
          border_top_left_radius(SpacePair::from_single(rounded.0))
        );
        push_decl!(
          builder,
          important,
          border_top_right_radius(SpacePair::from_single(rounded.0))
        );
        push_decl!(
          builder,
          important,
          border_bottom_right_radius(SpacePair::from_single(rounded.0))
        );
        push_decl!(
          builder,
          important,
          border_bottom_left_radius(SpacePair::from_single(rounded.0))
        );
      }
      TailwindProperty::VerticalAlign(vertical_align) => {
        push_decl!(builder, important, vertical_align(vertical_align))
      }
      TailwindProperty::RoundedTopLeft(rounded) => push_decl!(
        builder,
        important,
        border_top_left_radius(SpacePair::from_single(rounded.0))
      ),
      TailwindProperty::RoundedTopRight(rounded) => push_decl!(
        builder,
        important,
        border_top_right_radius(SpacePair::from_single(rounded.0))
      ),
      TailwindProperty::RoundedBottomRight(rounded) => push_decl!(
        builder,
        important,
        border_bottom_right_radius(SpacePair::from_single(rounded.0))
      ),
      TailwindProperty::RoundedBottomLeft(rounded) => push_decl!(
        builder,
        important,
        border_bottom_left_radius(SpacePair::from_single(rounded.0))
      ),
      TailwindProperty::RoundedTop(rounded) => {
        push_decl!(
          builder,
          important,
          border_top_left_radius(SpacePair::from_single(rounded.0))
        );
        push_decl!(
          builder,
          important,
          border_top_right_radius(SpacePair::from_single(rounded.0))
        );
      }
      TailwindProperty::RoundedRight(rounded) => {
        push_decl!(
          builder,
          important,
          border_top_right_radius(SpacePair::from_single(rounded.0))
        );
        push_decl!(
          builder,
          important,
          border_bottom_right_radius(SpacePair::from_single(rounded.0))
        );
      }
      TailwindProperty::RoundedBottom(rounded) => {
        push_decl!(
          builder,
          important,
          border_bottom_left_radius(SpacePair::from_single(rounded.0))
        );
        push_decl!(
          builder,
          important,
          border_bottom_right_radius(SpacePair::from_single(rounded.0))
        );
      }
      TailwindProperty::RoundedLeft(rounded) => {
        push_decl!(
          builder,
          important,
          border_top_left_radius(SpacePair::from_single(rounded.0))
        );
        push_decl!(
          builder,
          important,
          border_bottom_left_radius(SpacePair::from_single(rounded.0))
        );
      }
      TailwindProperty::TextOverflow(text_overflow) => {
        push_decl!(builder, important, text_overflow(text_overflow))
      }
      TailwindProperty::Truncate => {
        push_decl!(builder, important, text_overflow(TextOverflow::Ellipsis));
        push_decl!(builder, important, text_wrap_mode(TextWrapMode::NoWrap));
        push_decl!(
          builder,
          important,
          white_space_collapse(WhiteSpaceCollapse::Collapse)
        );
        push_decl!(builder, important, overflow_x(Overflow::Hidden));
        push_decl!(builder, important, overflow_y(Overflow::Hidden));
      }
      TailwindProperty::TextWrap(text_wrap) => {
        push_decl!(
          builder,
          important,
          text_wrap_mode(text_wrap.mode.unwrap_or_default())
        );
        push_decl!(builder, important, text_wrap_style(text_wrap.style));
      }
      TailwindProperty::WhiteSpace(white_space) => {
        push_decl!(
          builder,
          important,
          text_wrap_mode(white_space.text_wrap_mode)
        );
        push_decl!(
          builder,
          important,
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
        push_decl!(builder, important, margin_top(length));
        push_decl!(builder, important, margin_right(length));
        push_decl!(builder, important, margin_bottom(length));
        push_decl!(builder, important, margin_left(length));
      }
      TailwindProperty::MarginX(length) => {
        push_decl!(builder, important, margin_left(length));
        push_decl!(builder, important, margin_right(length));
      }
      TailwindProperty::MarginY(length) => {
        push_decl!(builder, important, margin_top(length));
        push_decl!(builder, important, margin_bottom(length));
      }
      TailwindProperty::MarginTop(length) => push_decl!(builder, important, margin_top(length)),
      TailwindProperty::MarginRight(length) => push_decl!(builder, important, margin_right(length)),
      TailwindProperty::MarginBottom(length) => {
        push_decl!(builder, important, margin_bottom(length))
      }
      TailwindProperty::MarginLeft(length) => push_decl!(builder, important, margin_left(length)),
      TailwindProperty::Padding(length) => {
        push_decl!(builder, important, padding_top(length));
        push_decl!(builder, important, padding_right(length));
        push_decl!(builder, important, padding_bottom(length));
        push_decl!(builder, important, padding_left(length));
      }
      TailwindProperty::PaddingX(length) => {
        push_decl!(builder, important, padding_left(length));
        push_decl!(builder, important, padding_right(length));
      }
      TailwindProperty::PaddingY(length) => {
        push_decl!(builder, important, padding_top(length));
        push_decl!(builder, important, padding_bottom(length));
      }
      TailwindProperty::PaddingTop(length) => push_decl!(builder, important, padding_top(length)),
      TailwindProperty::PaddingRight(length) => {
        push_decl!(builder, important, padding_right(length))
      }
      TailwindProperty::PaddingBottom(length) => {
        push_decl!(builder, important, padding_bottom(length))
      }
      TailwindProperty::PaddingLeft(length) => push_decl!(builder, important, padding_left(length)),
      TailwindProperty::Inset(length) => {
        push_decl!(builder, important, top(length));
        push_decl!(builder, important, right(length));
        push_decl!(builder, important, bottom(length));
        push_decl!(builder, important, left(length));
      }
      TailwindProperty::InsetX(length) => {
        push_decl!(builder, important, left(length));
        push_decl!(builder, important, right(length));
      }
      TailwindProperty::InsetY(length) => {
        push_decl!(builder, important, top(length));
        push_decl!(builder, important, bottom(length));
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
        builder.grid_column_mut(important).start = tw_grid_placement;
      }
      TailwindProperty::GridColumnEnd(tw_grid_placement) => {
        builder.grid_column_mut(important).end = tw_grid_placement;
      }
      TailwindProperty::GridRowStart(tw_grid_placement) => {
        builder.grid_row_mut(important).start = tw_grid_placement;
      }
      TailwindProperty::GridRowEnd(tw_grid_placement) => {
        builder.grid_row_mut(important).end = tw_grid_placement;
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
        for filter in filters {
          builder.push_filter(filter, important);
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
        for filter in filters {
          builder.push_backdrop_filter(filter, important);
        }
      }
      TailwindProperty::TextShadow(text_shadow) => {
        push_decl!(builder, important, text_shadow(Some([text_shadow].into())))
      }
      TailwindProperty::Visibility(visibility) => {
        push_decl!(builder, important, visibility(visibility))
      }
      TailwindProperty::Animation(animations) => {
        let has_animation_name = animations.iter().any(|animation| animation.name.is_some());

        push_decl!(
          builder,
          important,
          animation_duration(AnimationDurations(
            animations
              .iter()
              .map(|animation| animation.duration)
              .collect()
          ))
        );
        push_decl!(
          builder,
          important,
          animation_delay(AnimationDurations(
            animations.iter().map(|animation| animation.delay).collect()
          ))
        );
        push_decl!(
          builder,
          important,
          animation_timing_function(AnimationTimingFunctions(
            animations
              .iter()
              .map(|animation| animation.timing_function)
              .collect()
          ))
        );
        push_decl!(
          builder,
          important,
          animation_iteration_count(AnimationIterationCounts(
            animations
              .iter()
              .map(|animation| animation.iteration_count)
              .collect()
          ))
        );
        push_decl!(
          builder,
          important,
          animation_direction(AnimationDirections(
            animations
              .iter()
              .map(|animation| animation.direction)
              .collect()
          ))
        );
        push_decl!(
          builder,
          important,
          animation_fill_mode(AnimationFillModes(
            animations
              .iter()
              .map(|animation| animation.fill_mode)
              .collect()
          ))
        );
        push_decl!(
          builder,
          important,
          animation_play_state(AnimationPlayStates(
            animations
              .iter()
              .map(|animation| animation.play_state)
              .collect()
          ))
        );
        push_decl!(
          builder,
          important,
          animation_name(if has_animation_name {
            AnimationNames(
              animations
                .into_iter()
                .map(|animation| animation.name.unwrap_or_default())
                .collect(),
            )
          } else {
            AnimationNames::default()
          })
        );
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use crate::layout::style::{ComputedStyle, Style, properties::BackgroundImage};

  use super::*;

  #[test]
  fn test_box_sizing() {
    assert_eq!(
      TailwindProperty::parse("box-border"),
      Some(TailwindProperty::BoxSizing(BoxSizing::BorderBox))
    );
  }

  #[test]
  fn test_parse_width() {
    assert_eq!(
      TailwindProperty::parse("w-64"),
      Some(TailwindProperty::Width(Length::Rem(64.0 * TW_VAR_SPACING)))
    );
    assert_eq!(
      TailwindProperty::parse("h-32"),
      Some(TailwindProperty::Height(Length::Rem(32.0 * TW_VAR_SPACING)))
    );
    assert_eq!(
      TailwindProperty::parse("justify-self-center"),
      Some(TailwindProperty::JustifySelf(AlignItems::Center))
    );
  }

  #[test]
  fn test_parse_color() {
    assert_eq!(
      TailwindProperty::parse("text-black/30"),
      Some(TailwindProperty::Color(ColorInput::Value(Color([
        0,
        0,
        0,
        (0.3_f32 * 255.0).round() as u8
      ]))))
    );
  }

  #[test]
  fn test_parse_decoration_color() {
    assert_eq!(
      TailwindProperty::parse("decoration-red-500"),
      Some(TailwindProperty::TextDecorationColor(ColorInput::Value(
        Color([239, 68, 68, 255])
      )))
    );
  }

  #[test]
  fn test_parse_text_decoration_lines() {
    assert_eq!(
      TailwindProperty::parse("underline"),
      Some(TailwindProperty::TextDecorationLine(
        TextDecorationLines::UNDERLINE
      ))
    );
    assert_eq!(
      TailwindProperty::parse("no-underline"),
      Some(TailwindProperty::TextDecorationLine(
        TextDecorationLines::empty()
      ))
    );
  }

  #[test]
  fn test_parse_arbitrary_color() {
    assert_eq!(
      TailwindProperty::parse("text-[rgb(0, 191, 255)]"),
      Some(TailwindProperty::Color(ColorInput::Value(Color([
        0, 191, 255, 255
      ]))))
    );
  }

  #[test]
  fn test_parse_arbitrary_flex_with_spaces() {
    assert_eq!(
      TailwindProperty::parse("flex-[3_1_auto]"),
      Some(TailwindProperty::Flex(Flex {
        grow: 3.0,
        shrink: 1.0,
        basis: Length::Auto,
      }))
    );
  }

  #[test]
  fn test_parse_tailwind_animation_preset() {
    assert!(matches!(
      TailwindProperty::parse("animate-spin"),
      Some(TailwindProperty::Animation(animations))
        if animations.as_ref() == [Animation {
          duration: AnimationTime::from_milliseconds(1000.0),
          timing_function: AnimationTimingFunction::Linear,
          iteration_count: AnimationIterationCount::Infinite,
          name: Some("spin".to_string()),
          ..Animation::default()
        }]
    ));
  }

  #[test]
  fn test_parse_tailwind_animation_arbitrary_value() {
    assert!(matches!(
      TailwindProperty::parse("animate-[wiggle_1s_ease-in-out_infinite]"),
      Some(TailwindProperty::Animation(animations))
        if animations.as_ref() == [Animation {
          duration: AnimationTime::from_milliseconds(1000.0),
          timing_function: AnimationTimingFunction::EaseInOut,
          iteration_count: AnimationIterationCount::Infinite,
          name: Some("wiggle".to_string()),
          ..Animation::default()
        }]
    ));
  }

  #[test]
  fn test_parse_negative_margin() {
    assert_eq!(
      TailwindProperty::parse("-ml-4"),
      Some(TailwindProperty::MarginLeft(Length::Rem(
        -4.0 * TW_VAR_SPACING
      )))
    );
  }

  #[test]
  fn test_parse_border_radius() {
    assert_eq!(
      TailwindProperty::parse("rounded-xs"),
      Some(TailwindProperty::Rounded(TwRounded(Length::Rem(0.125))))
    );
    assert_eq!(
      TailwindProperty::parse("rounded-full"),
      Some(TailwindProperty::Rounded(TwRounded(Length::Px(9999.0))))
    );
  }

  #[test]
  fn test_parse_font_size_with_arbitrary_line_height() {
    assert_eq!(
      TailwindProperty::parse("text-base/[12.34]"),
      Some(TailwindProperty::FontSize(TwFontSize {
        font_size: (Length::Rem(1.0).into()),
        line_height: Some(LineHeight::Unitless(12.34)),
      }))
    );
  }

  #[test]
  fn test_parse_border_width() {
    assert_eq!(
      TailwindProperty::parse("border"),
      Some(TailwindProperty::BorderDefault)
    );
    assert_eq!(
      TailwindProperty::parse("border-t-2"),
      Some(TailwindProperty::BorderTopWidth(TwBorderWidth(Length::Px(
        2.0
      ))))
    );
    assert_eq!(
      TailwindProperty::parse("border-x-4"),
      Some(TailwindProperty::BorderXWidth(TwBorderWidth(Length::Px(
        4.0
      ))))
    );
    assert_eq!(
      TailwindProperty::parse("border-solid"),
      Some(TailwindProperty::BorderStyle(BorderStyle::Solid))
    );
    assert_eq!(
      TailwindProperty::parse("border-none"),
      Some(TailwindProperty::BorderStyle(BorderStyle::None))
    );
  }

  #[test]
  fn test_parse_outline() {
    assert_eq!(
      TailwindProperty::parse("outline"),
      Some(TailwindProperty::OutlineDefault)
    );
    assert_eq!(
      TailwindProperty::parse("outline-2"),
      Some(TailwindProperty::OutlineWidth(TwBorderWidth(Length::Px(
        2.0
      ))))
    );
    assert_eq!(
      TailwindProperty::parse("outline-red-500"),
      Some(TailwindProperty::OutlineColor(ColorInput::Value(Color([
        239, 68, 68, 255
      ]))))
    );
    assert_eq!(
      TailwindProperty::parse("outline-solid"),
      Some(TailwindProperty::OutlineStyle(BorderStyle::Solid))
    );
    assert_eq!(
      TailwindProperty::parse("outline-offset-4"),
      Some(TailwindProperty::OutlineOffset(TwBorderWidth(Length::Px(
        4.0
      ))))
    );
    assert_eq!(
      TailwindProperty::parse("outline-none"),
      Some(TailwindProperty::OutlineStyle(BorderStyle::None))
    );
  }

  #[test]
  fn test_parse_col_end() {
    assert_eq!(
      TailwindProperty::parse("col-end-1"),
      Some(TailwindProperty::GridColumnEnd(GridPlacement::Line(1)))
    );
  }

  #[test]
  fn test_parse_overflow_clip() {
    assert_eq!(
      TailwindProperty::parse("overflow-clip"),
      Some(TailwindProperty::Overflow(Overflow::Clip))
    );
    assert_eq!(
      TailwindProperty::parse("overflow-x-clip"),
      Some(TailwindProperty::OverflowX(Overflow::Clip))
    );
    assert_eq!(
      TailwindProperty::parse("overflow-y-clip"),
      Some(TailwindProperty::OverflowY(Overflow::Clip))
    );
  }

  #[test]
  fn test_comprehensive_mappings() {
    // Test various prefix mappings to ensure they're working
    let should_parse = vec![
      // Layout
      "flex",
      "grid",
      "hidden",
      "block",
      "inline",
      // Sizing
      "w-4",
      "h-8",
      "size-12",
      "min-w-0",
      "max-h-96",
      // Spacing
      "m-2",
      "mx-4",
      "my-auto",
      "mt-8",
      "mr-6",
      "mb-4",
      "ml-2",
      "p-3",
      "px-5",
      "py-2",
      "pt-1",
      "pr-4",
      "pb-3",
      "pl-2",
      // Colors
      "text-red-500",
      "bg-blue-200",
      "border-gray-300",
      // Typography
      "text-sm",
      "font-bold",
      "font-stretch-condensed",
      "font-stretch-ultra-expanded",
      "font-stretch-75%",
      "uppercase",
      "tracking-wide",
      "animate-spin",
      "animate-[wiggle_1s_ease-in-out_infinite]",
      // Flexbox
      "justify-center",
      "items-end",
      "self-start",
      "flex-grow",
      "shrink",
      // Borders
      "border",
      "border-t-2",
      "border-solid",
      "border-none",
      "outline",
      "outline-2",
      "outline-red-500",
      "outline-solid",
      "outline-offset-2",
      "rounded-lg",
      // Transforms
      "rotate-45",
      "scale-75",
      "translate-x-4",
      // Grid
      "grid-cols-3",
      "col-span-2",
      // Backdrop Filters
      "backdrop-blur-md",
      "backdrop-brightness-50",
      "backdrop-contrast-125",
      "backdrop-grayscale",
      "backdrop-hue-rotate-90",
      "backdrop-invert",
      "backdrop-opacity-50",
      "backdrop-saturate-200",
      "backdrop-sepia",
      "backdrop-filter-[blur(4px)_brightness(0.5)]",
    ];

    let should_not_parse = vec!["nonexistent-class", "invalid-prefix-1", "random-string"];

    for class in should_parse {
      assert!(
        TailwindProperty::parse(class).is_some(),
        "Expected '{}' to parse successfully",
        class
      );
    }

    for class in should_not_parse {
      assert!(
        TailwindProperty::parse(class).is_none(),
        "Expected '{}' to fail parsing",
        class
      );
    }
  }

  #[test]
  fn test_breakpoint_matches() {
    let viewport = (1000, 1000).into();

    assert!(Breakpoint::parse("sm").is_some_and(|bp| bp.matches(viewport)));
  }

  #[test]
  fn test_breakpoint_does_not_match() {
    let viewport = (1000, 1000).into();

    // 80 * 16 = 1280 > 1000
    assert!(Breakpoint::parse("xl").is_some_and(|bp| !bp.matches(viewport)));
  }

  #[test]
  fn test_value_parsing() {
    assert_eq!(
      TailwindValue::parse("md:!mt-4"),
      Some(TailwindValue {
        property: TailwindProperty::MarginTop(Length::Rem(1.0)),
        breakpoint: Some(Breakpoint(Length::Rem(48.0))),
        important: true,
      })
    );
  }

  #[test]
  fn test_values_sorting() {
    assert_eq!(
      TailwindValues::from_str("md:!mt-4 sm:mt-8 !mt-12 mt-16"),
      Ok(TailwindValues {
        inner: vec![
          // mt-16
          TailwindValue {
            property: TailwindProperty::MarginTop(Length::Rem(4.0)),
            breakpoint: None,
            important: false,
          },
          // sm:mt-8
          TailwindValue {
            property: TailwindProperty::MarginTop(Length::Rem(2.0)),
            breakpoint: Some(Breakpoint(Length::Rem(40.0))),
            important: false,
          },
          // !mt-12
          TailwindValue {
            property: TailwindProperty::MarginTop(Length::Rem(3.0)),
            breakpoint: None,
            important: true,
          },
          // md:!mt-4
          TailwindValue {
            property: TailwindProperty::MarginTop(Length::Rem(1.0)),
            breakpoint: Some(Breakpoint(Length::Rem(48.0))),
            important: true,
          },
        ]
      })
    )
  }

  #[test]
  fn test_filters_append() {
    use crate::layout::style::properties::Filter;

    let Ok(values) = TailwindValues::from_str("blur-sm brightness-150 contrast-125") else {
      unreachable!()
    };
    let viewport = (100, 100).into();

    let style =
      Style::from(values.into_declaration_block(viewport)).inherit(&ComputedStyle::default());

    assert_eq!(
      style.filter,
      vec![
        Filter::Blur(Length::Px(8.0)),
        Filter::Brightness(PercentageNumber(1.5)),
        Filter::Contrast(PercentageNumber(1.25))
      ]
    )
  }

  #[test]
  fn test_transform_utilities_resolve_to_standard_longhands() {
    let Ok(values) = TailwindValues::from_str("translate-x-4 translate-y-8 scale-75 scale-x-50")
    else {
      unreachable!()
    };
    let viewport = (100, 100).into();

    let style =
      Style::from(values.into_declaration_block(viewport)).inherit(&ComputedStyle::default());

    assert_eq!(
      style.translate,
      SpacePair::from_pair(Length::Rem(1.0), Length::Rem(2.0))
    );
    assert_eq!(
      style.scale,
      SpacePair::from_pair(PercentageNumber(0.5), PercentageNumber(0.75))
    );
  }

  #[test]
  fn test_parse_blend_mode() {
    assert_eq!(
      TailwindProperty::parse("mix-blend-multiply"),
      Some(TailwindProperty::MixBlendMode(BlendMode::Multiply))
    );
    assert_eq!(
      TailwindProperty::parse("bg-blend-screen"),
      Some(TailwindProperty::BackgroundBlendMode(BlendMode::Screen))
    );
  }
  #[test]
  fn test_parse_vertical_align() {
    assert_eq!(
      TailwindProperty::parse("align-baseline"),
      Some(TailwindProperty::VerticalAlign(VerticalAlign::Keyword(
        VerticalAlignKeyword::Baseline
      )))
    );
    assert_eq!(
      TailwindProperty::parse("align-top"),
      Some(TailwindProperty::VerticalAlign(VerticalAlign::Keyword(
        VerticalAlignKeyword::Top
      )))
    );
    assert_eq!(
      TailwindProperty::parse("align-middle"),
      Some(TailwindProperty::VerticalAlign(VerticalAlign::Keyword(
        VerticalAlignKeyword::Middle
      )))
    );
    assert_eq!(
      TailwindProperty::parse("align-bottom"),
      Some(TailwindProperty::VerticalAlign(VerticalAlign::Keyword(
        VerticalAlignKeyword::Bottom
      )))
    );
    assert_eq!(
      TailwindProperty::parse("align-text-top"),
      Some(TailwindProperty::VerticalAlign(VerticalAlign::Keyword(
        VerticalAlignKeyword::TextTop
      )))
    );
    assert_eq!(
      TailwindProperty::parse("align-text-bottom"),
      Some(TailwindProperty::VerticalAlign(VerticalAlign::Keyword(
        VerticalAlignKeyword::TextBottom
      )))
    );
    assert_eq!(
      TailwindProperty::parse("align-sub"),
      Some(TailwindProperty::VerticalAlign(VerticalAlign::Keyword(
        VerticalAlignKeyword::Sub
      )))
    );
    assert_eq!(
      TailwindProperty::parse("align-super"),
      Some(TailwindProperty::VerticalAlign(VerticalAlign::Keyword(
        VerticalAlignKeyword::Super
      )))
    );
    assert_eq!(
      TailwindProperty::parse("align-[10px]"),
      Some(TailwindProperty::VerticalAlign(VerticalAlign::Length(
        Length::Px(10.0)
      )))
    );
    assert_eq!(
      TailwindProperty::parse("align-[25%]"),
      Some(TailwindProperty::VerticalAlign(VerticalAlign::Length(
        Length::Percentage(25.0)
      )))
    );
    assert_eq!(
      TailwindProperty::parse("align-[-0.5em]"),
      Some(TailwindProperty::VerticalAlign(VerticalAlign::Length(
        Length::Em(-0.5)
      )))
    );
  }

  #[test]
  fn test_parse_decoration_thickness() {
    assert_eq!(
      TailwindProperty::parse("decoration-4"),
      Some(TailwindProperty::TextDecorationThickness(
        TextDecorationThickness::Length(Length::Px(4.0))
      ))
    );
    assert_eq!(
      TailwindProperty::parse("decoration-auto"),
      Some(TailwindProperty::TextDecorationThickness(
        TextDecorationThickness::Length(Length::Auto)
      ))
    );
    assert_eq!(
      TailwindProperty::parse("decoration-from-font"),
      Some(TailwindProperty::TextDecorationThickness(
        TextDecorationThickness::FromFont
      ))
    );
    assert_eq!(
      TailwindProperty::parse("decoration-[3px]"),
      Some(TailwindProperty::TextDecorationThickness(
        TextDecorationThickness::Length(Length::Px(3.0))
      ))
    );
  }

  #[test]
  fn test_linear_gradient_apply() {
    let viewport = (100, 100).into();
    let Ok(values) =
      TailwindValues::from_str("bg-linear-to-r from-red-500 via-green-500 to-blue-500")
    else {
      unreachable!()
    };

    let style =
      Style::from(values.into_declaration_block(viewport)).inherit(&ComputedStyle::default());

    assert_eq!(
      style.background_image,
      Some(
        [BackgroundImage::Linear(LinearGradient {
          angle: Angle::new(90.0),
          interpolation: ColorInterpolationMethod::default(),
          stops: [
            GradientStop::ColorHint {
              color: ColorInput::Value(Color([239, 68, 68, 255])),
              hint: Some(StopPosition(Length::Percentage(0.0))),
            },
            GradientStop::ColorHint {
              color: ColorInput::Value(Color([34, 197, 94, 255])),
              hint: Some(StopPosition(Length::Percentage(50.0))),
            },
            GradientStop::ColorHint {
              color: ColorInput::Value(Color([59, 130, 246, 255])),
              hint: Some(StopPosition(Length::Percentage(100.0))),
            },
          ]
          .into(),
        })]
        .into()
      )
    );
  }
}
