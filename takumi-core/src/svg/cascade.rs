//! Applying declarations to a style, after Blink's `StyleBuilder` and the SVG property parsers in
//! `css_parsing_utils.cc`.

use std::sync::Arc;

use cssparser::{Parser, Token};
use tiny_skia::Transform;

use super::{
  length::{Length, LengthUnit},
  name::Property,
  scan::{parse_length, parse_length_list, parse_number},
  style::{
    BaselineShift, ColorInterpolation, Dasharray, Display, FilterValue, ImageRendering, LineCap,
    LineJoin, MaskType, Overflow, Paint, PaintFallback, PaintOrder, Reference, ShapeRendering,
    Spacing, StyleContext, SvgStyle, TextAnchor, TextDecoration, TextRendering, VectorEffect,
  },
  transform::{parse_transform_list, parse_transform_origin},
};
use crate::style::{
  Angle, BlendMode, Color, ColorInput, FillRule, Filter, FontFamily, FontKerning,
  FontOpticalSizing, FontShorthand, FontSize, FontSizeKeyword, FontStretch, FontStyle,
  FontVariantCaps, FontVariationSettings, FontWeight, FromCss, Isolation, PercentageNumber,
  Visibility,
};

/// A CSS-wide keyword.
enum Wide {
  Inherit,
  Initial,
  Unset,
}

impl Wide {
  fn parse(text: &str) -> Option<Self> {
    if text.eq_ignore_ascii_case("inherit") {
      Some(Self::Inherit)
    } else if text.eq_ignore_ascii_case("initial") {
      Some(Self::Initial)
    } else if text.eq_ignore_ascii_case("unset") {
      Some(Self::Unset)
    } else {
      None
    }
  }
}

/// Copies the fields of `$source` that `$property` sets.
macro_rules! copy {
  ($target:expr, $source:expr, $($field:ident),+ $(,)?) => {{
    $($target.$field = $source.$field.clone();)+
  }};
}

impl SvgStyle {
  /// The style of a child that declares nothing: the inherited properties of `self`, and the
  /// initial value of the others.
  pub fn inherit(&self, context: &StyleContext) -> Self {
    let initial = Self::initial(context);
    let mut style = self.clone();

    copy!(
      style,
      initial,
      display,
      opacity,
      background_color,
      clip_path,
      mask,
      mask_type,
      filter,
      flood_color,
      flood_opacity,
      lighting_color,
      stop_color,
      stop_opacity,
      mix_blend_mode,
      isolation,
      overflow,
      vector_effect,
      transform,
      transform_origin,
      text_decoration,
      alignment_baseline,
      baseline_shift,
    );
    style
  }

  /// Applies the declaration `property: value` over `parent`'s inherited values, or returns
  /// `false` and leaves the style unchanged when the value does not parse.
  pub fn apply(
    &mut self,
    property: Property,
    value: &str,
    parent: &SvgStyle,
    context: &StyleContext,
  ) -> bool {
    let value = value.trim();

    match Wide::parse(value) {
      Some(Wide::Inherit) => self.copy_property(property, parent),
      Some(Wide::Initial) => self.copy_property(property, &Self::initial(context)),
      Some(Wide::Unset) if is_inherited(property) => self.copy_property(property, parent),
      Some(Wide::Unset) => self.copy_property(property, &Self::initial(context)),
      None => return self.apply_value(property, value, parent, context),
    }
    true
  }

  fn copy_property(&mut self, property: Property, source: &SvgStyle) {
    match property {
      Property::AlignmentBaseline => copy!(self, source, alignment_baseline),
      Property::BackgroundColor => copy!(self, source, background_color),
      Property::BaselineShift => copy!(self, source, baseline_shift),
      Property::ClipPath => copy!(self, source, clip_path),
      Property::ClipRule => copy!(self, source, clip_rule),
      Property::Color => copy!(self, source, color),
      Property::ColorInterpolationFilters => copy!(self, source, color_interpolation_filters),
      Property::Display => copy!(self, source, display),
      Property::DominantBaseline => copy!(self, source, dominant_baseline),
      Property::Fill => copy!(self, source, fill),
      Property::FillOpacity => copy!(self, source, fill_opacity),
      Property::FillRule => copy!(self, source, fill_rule),
      Property::Filter => copy!(self, source, filter),
      Property::FloodColor => copy!(self, source, flood_color),
      Property::FloodOpacity => copy!(self, source, flood_opacity),
      Property::Font => copy!(
        self,
        source,
        font_family,
        font_size,
        font_style,
        small_caps,
        font_weight,
        font_stretch,
        font_kerning,
        font_optical_sizing,
        font_variation_settings,
      ),
      Property::FontFamily => copy!(self, source, font_family),
      Property::FontKerning => copy!(self, source, font_kerning),
      Property::FontOpticalSizing => copy!(self, source, font_optical_sizing),
      Property::FontSize => copy!(self, source, font_size),
      Property::FontStretch => copy!(self, source, font_stretch),
      Property::FontStyle => copy!(self, source, font_style),
      Property::FontVariationSettings => copy!(self, source, font_variation_settings),
      Property::FontVariant => copy!(self, source, small_caps),
      Property::FontWeight => copy!(self, source, font_weight),
      Property::ImageRendering => copy!(self, source, image_rendering),
      Property::Isolation => copy!(self, source, isolation),
      Property::LetterSpacing => copy!(self, source, letter_spacing),
      Property::LightingColor => copy!(self, source, lighting_color),
      Property::Marker => copy!(self, source, marker_start, marker_mid, marker_end),
      Property::MarkerEnd => copy!(self, source, marker_end),
      Property::MarkerMid => copy!(self, source, marker_mid),
      Property::MarkerStart => copy!(self, source, marker_start),
      Property::Mask => copy!(self, source, mask),
      Property::MaskType => copy!(self, source, mask_type),
      Property::MixBlendMode => copy!(self, source, mix_blend_mode),
      Property::Opacity => copy!(self, source, opacity),
      Property::Overflow => copy!(self, source, overflow),
      Property::PaintOrder => copy!(self, source, paint_order),
      Property::ShapeRendering => copy!(self, source, shape_rendering),
      Property::StopColor => copy!(self, source, stop_color),
      Property::StopOpacity => copy!(self, source, stop_opacity),
      Property::Stroke => copy!(self, source, stroke),
      Property::StrokeDasharray => copy!(self, source, stroke_dasharray),
      Property::StrokeDashoffset => copy!(self, source, stroke_dashoffset),
      Property::StrokeLinecap => copy!(self, source, stroke_linecap),
      Property::StrokeLinejoin => copy!(self, source, stroke_linejoin),
      Property::StrokeMiterlimit => copy!(self, source, stroke_miterlimit),
      Property::StrokeOpacity => copy!(self, source, stroke_opacity),
      Property::StrokeWidth => copy!(self, source, stroke_width),
      Property::TextAnchor => copy!(self, source, text_anchor),
      Property::TextDecoration => copy!(self, source, text_decoration),
      Property::TextRendering => copy!(self, source, text_rendering),
      Property::Transform => copy!(self, source, transform),
      Property::TransformOrigin => copy!(self, source, transform_origin),
      Property::VectorEffect => copy!(self, source, vector_effect),
      Property::Visibility => copy!(self, source, visibility),
      Property::WhiteSpace => copy!(self, source, preserve_white_space),
      Property::WordSpacing => copy!(self, source, word_spacing),
    }
  }

  fn apply_value(
    &mut self,
    property: Property,
    value: &str,
    parent: &SvgStyle,
    context: &StyleContext,
  ) -> bool {
    match property {
      Property::AlignmentBaseline => set(&mut self.alignment_baseline, parse_css(value)),
      Property::BackgroundColor => set(&mut self.background_color, parse_css(value)),
      Property::BaselineShift => set(&mut self.baseline_shift, parse_baseline_shift(value)),
      Property::ClipPath => set(&mut self.clip_path, parse_reference(value)),
      Property::ClipRule => set(&mut self.clip_rule, parse_css::<FillRule>(value)),
      Property::Color => match parse_css::<ColorInput>(value) {
        Some(ColorInput::Value(color)) => {
          self.color = Some(color);
          true
        }
        Some(ColorInput::CurrentColor) => {
          self.color = parent.color;
          true
        }
        None => false,
      },
      Property::ColorInterpolationFilters => set(
        &mut self.color_interpolation_filters,
        parse_css::<ColorInterpolation>(value),
      ),
      Property::Display => set(&mut self.display, parse_display(value)),
      Property::DominantBaseline => set(&mut self.dominant_baseline, parse_css(value)),
      Property::Fill => set(&mut self.fill, parse_paint(value)),
      Property::FillOpacity => set(&mut self.fill_opacity, parse_opacity(value)),
      Property::FillRule => set(&mut self.fill_rule, parse_css::<FillRule>(value)),
      Property::Filter => set(&mut self.filter, parse_filter_list(value)),
      Property::FloodColor => set(&mut self.flood_color, parse_css(value)),
      Property::FloodOpacity => set(&mut self.flood_opacity, parse_opacity(value)),
      Property::Font => match parse_css::<FontShorthand>(value) {
        Some(font) => {
          self.font_style = font.style;
          self.font_weight = font.weight.resolve_against(parent.font_weight).value();
          self.font_stretch = font.stretch;
          self.font_size = font_size_px(font.size, parent.font_size, context);
          self.font_family = Some(font.family);
          self.small_caps = font.caps == FontVariantCaps::SmallCaps;
          self.font_kerning = FontKerning::Auto;
          self.font_optical_sizing = FontOpticalSizing::default();
          self.font_variation_settings = FontVariationSettings::from([]);
          true
        }
        None => false,
      },
      Property::FontFamily => set(
        &mut self.font_family,
        parse_css::<FontFamily>(value).map(Some),
      ),
      Property::FontKerning => set(&mut self.font_kerning, parse_css::<FontKerning>(value)),
      Property::FontOpticalSizing => set(
        &mut self.font_optical_sizing,
        parse_css::<FontOpticalSizing>(value),
      ),
      Property::FontSize => set(
        &mut self.font_size,
        parse_font_size(value, parent.font_size, context),
      ),
      Property::FontStretch => set(&mut self.font_stretch, parse_css::<FontStretch>(value)),
      Property::FontStyle => set(&mut self.font_style, parse_css::<FontStyle>(value)),
      Property::FontVariationSettings => set(
        &mut self.font_variation_settings,
        parse_css::<FontVariationSettings>(value),
      ),
      Property::FontVariant => set(&mut self.small_caps, parse_font_variant(value)),
      Property::FontWeight => set(
        &mut self.font_weight,
        parse_css::<FontWeight>(value)
          .map(|weight| weight.resolve_against(parent.font_weight).value()),
      ),
      Property::ImageRendering => set(
        &mut self.image_rendering,
        parse_css::<ImageRendering>(value),
      ),
      Property::Isolation => set(&mut self.isolation, parse_css::<Isolation>(value)),
      Property::LetterSpacing => set(&mut self.letter_spacing, parse_spacing(value)),
      Property::LightingColor => set(&mut self.lighting_color, parse_css(value)),
      Property::Marker => match parse_reference(value) {
        Some(reference) => {
          self.marker_start = reference.clone();
          self.marker_mid = reference.clone();
          self.marker_end = reference;
          true
        }
        None => false,
      },
      Property::MarkerEnd => set(&mut self.marker_end, parse_reference(value)),
      Property::MarkerMid => set(&mut self.marker_mid, parse_reference(value)),
      Property::MarkerStart => set(&mut self.marker_start, parse_reference(value)),
      Property::Mask => set(&mut self.mask, parse_reference(value)),
      Property::MaskType => set(&mut self.mask_type, parse_css::<MaskType>(value)),
      Property::MixBlendMode => set(&mut self.mix_blend_mode, parse_css::<BlendMode>(value)),
      Property::Opacity => set(&mut self.opacity, parse_opacity(value)),
      Property::Overflow => set(&mut self.overflow, parse_css::<Overflow>(value)),
      Property::PaintOrder => set(&mut self.paint_order, PaintOrder::parse(value)),
      Property::ShapeRendering => set(
        &mut self.shape_rendering,
        parse_css::<ShapeRendering>(value),
      ),
      Property::StopColor => set(&mut self.stop_color, parse_css(value)),
      Property::StopOpacity => set(&mut self.stop_opacity, parse_opacity(value)),
      Property::Stroke => set(&mut self.stroke, parse_paint(value)),
      Property::StrokeDasharray => set(&mut self.stroke_dasharray, parse_dasharray(value)),
      Property::StrokeDashoffset => set(&mut self.stroke_dashoffset, parse_length(value)),
      Property::StrokeLinecap => set(&mut self.stroke_linecap, parse_css::<LineCap>(value)),
      Property::StrokeLinejoin => set(&mut self.stroke_linejoin, parse_css::<LineJoin>(value)),
      Property::StrokeMiterlimit => set(
        &mut self.stroke_miterlimit,
        parse_number(value).filter(|limit| *limit >= 1.0),
      ),
      Property::StrokeOpacity => set(&mut self.stroke_opacity, parse_opacity(value)),
      Property::StrokeWidth => set(
        &mut self.stroke_width,
        parse_length(value).filter(|width| width.number >= 0.0),
      ),
      Property::TextAnchor => set(&mut self.text_anchor, parse_css::<TextAnchor>(value)),
      Property::TextDecoration => set(&mut self.text_decoration, parse_text_decoration(value)),
      Property::TextRendering => set(&mut self.text_rendering, parse_css::<TextRendering>(value)),
      Property::Transform => set(&mut self.transform, parse_transform(value)),
      Property::TransformOrigin => set(
        &mut self.transform_origin,
        parse_transform_origin(value).map(Some),
      ),
      Property::VectorEffect => set(&mut self.vector_effect, parse_css::<VectorEffect>(value)),
      Property::Visibility => set(&mut self.visibility, parse_css::<Visibility>(value)),
      Property::WhiteSpace => set(&mut self.preserve_white_space, parse_white_space(value)),
      Property::WordSpacing => set(&mut self.word_spacing, parse_spacing(value)),
    }
  }
}

fn set<T>(field: &mut T, value: Option<T>) -> bool {
  match value {
    Some(value) => {
      *field = value;
      true
    }
    None => false,
  }
}

/// Whether `property` inherits, per Blink's `css_properties.json5`.
fn is_inherited(property: Property) -> bool {
  !matches!(
    property,
    Property::AlignmentBaseline
      | Property::BackgroundColor
      | Property::BaselineShift
      | Property::ClipPath
      | Property::Display
      | Property::Filter
      | Property::FloodColor
      | Property::FloodOpacity
      | Property::Isolation
      | Property::LightingColor
      | Property::Mask
      | Property::MaskType
      | Property::MixBlendMode
      | Property::Opacity
      | Property::Overflow
      | Property::StopColor
      | Property::StopOpacity
      | Property::TextDecoration
      | Property::Transform
      | Property::TransformOrigin
      | Property::VectorEffect
  )
}

/// Parses all of `text` as one `T`.
fn parse_css<T>(text: &str) -> Option<T>
where
  T: for<'i> FromCss<'i>,
{
  let mut parser = Parser::new(text);
  let value = T::from_css(&mut parser).ok()?;

  parser.expect_exhausted().ok()?;
  Some(value)
}

fn parse_display(text: &str) -> Option<Display> {
  let mut parser = Parser::new(text);
  let token = parser.next().ok()?.clone();

  parser.expect_exhausted().ok()?;
  match token {
    Token::Ident(name) if name.eq_ignore_ascii_case("none") => Some(Display::None),
    Token::Ident(_) => Some(Display::Inline),
    _ => None,
  }
}

/// A number or a percentage, clamped to `0..=1`.
fn parse_opacity(text: &str) -> Option<f32> {
  let length = parse_length(text)?;

  match length.unit {
    LengthUnit::None => Some(length.number.clamp(0.0, 1.0)),
    LengthUnit::Percent => Some((length.number / 100.0).clamp(0.0, 1.0)),
    _ => None,
  }
}

fn parse_spacing(text: &str) -> Option<Spacing> {
  if text.eq_ignore_ascii_case("normal") {
    Some(Spacing::Normal)
  } else {
    parse_length(text).map(Spacing::Length)
  }
}

fn parse_baseline_shift(text: &str) -> Option<BaselineShift> {
  match text {
    "baseline" => Some(BaselineShift::Baseline),
    "sub" => Some(BaselineShift::Sub),
    "super" => Some(BaselineShift::Super),
    _ => parse_length(text).map(BaselineShift::Length),
  }
}

fn parse_dasharray(text: &str) -> Option<Dasharray> {
  if text.eq_ignore_ascii_case("none") {
    return Some(Dasharray::None);
  }

  let list = parse_length_list(text)?;

  (!list.is_empty() && list.iter().all(|length| length.number >= 0.0))
    .then_some(Dasharray::List(list))
}

fn parse_text_decoration(text: &str) -> Option<TextDecoration> {
  let mut lines = TextDecoration::empty();

  for word in text.split_ascii_whitespace() {
    match word {
      "none" => {}
      "underline" => lines |= TextDecoration::UNDERLINE,
      "overline" => lines |= TextDecoration::OVERLINE,
      "line-through" => lines |= TextDecoration::LINE_THROUGH,
      _ => return None,
    }
  }
  Some(lines)
}

fn parse_font_variant(text: &str) -> Option<bool> {
  match text {
    "normal" => Some(false),
    "small-caps" => Some(true),
    _ => None,
  }
}

fn parse_white_space(text: &str) -> Option<bool> {
  match text {
    "normal" | "nowrap" | "pre-line" => Some(false),
    "pre" | "pre-wrap" | "break-spaces" => Some(true),
    _ => None,
  }
}

fn parse_transform(text: &str) -> Option<Option<Transform>> {
  if text.eq_ignore_ascii_case("none") {
    return Some(None);
  }
  parse_transform_list(text).map(Some)
}

/// `none`, or `url(#id)`.
fn parse_reference(text: &str) -> Option<Reference> {
  if text.eq_ignore_ascii_case("none") {
    return Some(None);
  }

  let mut parser = Parser::new(text);
  let id = url_fragment(&mut parser)?;

  parser.expect_exhausted().ok()?;
  Some(Some(id))
}

/// The id of a `url(#id)` token.
fn url_fragment(input: &mut Parser) -> Option<Arc<str>> {
  let url = match input.next().ok()?.clone() {
    Token::UnquotedUrl(url) => url.to_string(),
    Token::Function(name) if name.eq_ignore_ascii_case("url") => input
      .parse_nested_block::<_, _, ()>(|block| {
        block
          .expect_string()
          .map(ToString::to_string)
          .map_err(Into::into)
      })
      .ok()?,
    _ => return None,
  };
  let id = url.trim().strip_prefix('#')?;

  (!id.is_empty()).then(|| Arc::from(id))
}

fn parse_paint(text: &str) -> Option<Paint> {
  let mut parser = Parser::new(text);
  let paint = paint_from(&mut parser)?;

  parser.expect_exhausted().ok()?;
  Some(paint)
}

fn paint_from(input: &mut Parser) -> Option<Paint> {
  let start = input.state();

  if let Some(id) = url_fragment(input) {
    let fallback = if input.is_exhausted() {
      None
    } else {
      Some(fallback_from(input)?)
    };

    return Some(Paint::Url(id, fallback));
  }
  input.reset(&start);

  if let Ok(Token::Ident(name)) = input.next().cloned() {
    let keyword = match name.to_ascii_lowercase().as_str() {
      "none" => Some(Paint::None),
      "currentcolor" => Some(Paint::CurrentColor),
      "context-fill" => Some(Paint::ContextFill),
      "context-stroke" => Some(Paint::ContextStroke),
      _ => None,
    };

    if keyword.is_some() {
      return keyword;
    }
  }
  input.reset(&start);

  Color::from_css(input).ok().map(Paint::Color)
}

fn fallback_from(input: &mut Parser) -> Option<PaintFallback> {
  match paint_from(input)? {
    Paint::None => Some(PaintFallback::None),
    Paint::CurrentColor => Some(PaintFallback::CurrentColor),
    Paint::Color(color) => Some(PaintFallback::Color(color)),
    _ => None,
  }
}

fn parse_filter_list(text: &str) -> Option<Vec<FilterValue>> {
  if text.eq_ignore_ascii_case("none") {
    return Some(Vec::new());
  }

  let mut parser = Parser::new(text);
  let mut filters = Vec::new();

  while !parser.is_exhausted() {
    let start = parser.state();

    if let Some(id) = url_fragment(&mut parser) {
      filters.push(FilterValue::Url(id));
      continue;
    }
    parser.reset(&start);
    filters.push(FilterValue::Function(function_from(&mut parser)?));
  }
  (!filters.is_empty()).then_some(filters)
}

/// A filter function, with the default amount when the parentheses are empty.
fn function_from(input: &mut Parser) -> Option<Filter> {
  let start = input.state();

  if let Ok(Token::Function(name)) = input.next().cloned() {
    let empty = input
      .parse_nested_block::<_, _, ()>(|block| block.expect_exhausted().map_err(Into::into))
      .is_ok();
    let whole = PercentageNumber(1.0);
    let default = match name.to_ascii_lowercase().as_str() {
      "brightness" => Some(Filter::Brightness(whole)),
      "contrast" => Some(Filter::Contrast(whole)),
      "grayscale" => Some(Filter::Grayscale(whole)),
      "invert" => Some(Filter::Invert(whole)),
      "opacity" => Some(Filter::Opacity(whole)),
      "saturate" => Some(Filter::Saturate(whole)),
      "sepia" => Some(Filter::Sepia(whole)),
      "hue-rotate" => Some(Filter::HueRotate(Angle::zero())),
      "blur" => Some(Filter::Blur(crate::style::Length::zero())),
      _ => None,
    };

    if empty && default.is_some() {
      return default;
    }
  }
  input.reset(&start);
  Filter::from_css(input).ok()
}

fn parse_font_size(text: &str, parent: f32, context: &StyleContext) -> Option<f32> {
  if let Some(length) = parse_length(text) {
    return Some(length_to_font_size(length, parent, context));
  }
  parse_css::<FontSize>(text).map(|size| font_size_px(size, parent, context))
}

fn font_size_px(size: FontSize, parent: f32, context: &StyleContext) -> f32 {
  match size {
    FontSize::Keyword(keyword) => {
      let factor = match keyword {
        FontSizeKeyword::XXSmall => 0.6,
        FontSizeKeyword::XSmall => 0.75,
        FontSizeKeyword::Small => 8.0 / 9.0,
        FontSizeKeyword::Medium => 1.0,
        FontSizeKeyword::Large => 1.2,
        FontSizeKeyword::XLarge => 1.5,
        FontSizeKeyword::XXLarge => 2.0,
        _ => 3.0,
      };

      factor * context.font_size
    }
    FontSize::Larger => parent * 1.2,
    FontSize::Smaller => parent / 1.2,
    FontSize::Length(length) => match length {
      crate::style::Length::Px(px) => px,
      crate::style::Length::Em(em) => em * parent,
      crate::style::Length::Percentage(percent) => parent * percent / 100.0,
      crate::style::Length::Rem(rem) => rem * context.font_size,
      _ => parent,
    },
  }
}

fn length_to_font_size(length: Length, parent: f32, context: &StyleContext) -> f32 {
  match length.unit {
    LengthUnit::Percent => parent * length.number / 100.0,
    _ => {
      let relative = super::length::LengthContext {
        font_size: parent,
        width: 0.0,
        height: 0.0,
        dpi: context.dpi,
      };

      length.to_fraction_or_user_units(&relative)
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn style_after(declarations: &[(Property, &str)]) -> SvgStyle {
    let context = StyleContext {
      font_size: 12.0,
      dpi: 96.0,
    };
    let initial = SvgStyle::initial(&context);
    let mut style = initial.inherit(&context);

    for (property, value) in declarations {
      style.apply(*property, value, &initial, &context);
    }
    style
  }

  #[test]
  fn paints_parse_urls_fallbacks_and_keywords() {
    assert_eq!(
      parse_paint("url(#a) red"),
      Some(Paint::Url(
        Arc::from("a"),
        Some(PaintFallback::Color(Color([255, 0, 0, 255])))
      ))
    );
    assert_eq!(
      parse_paint("url('#a')"),
      Some(Paint::Url(Arc::from("a"), None))
    );
    assert_eq!(parse_paint("currentColor"), Some(Paint::CurrentColor));
    assert_eq!(
      parse_paint("#0f08"),
      Some(Paint::Color(Color([0, 255, 0, 136])))
    );
    assert_eq!(parse_paint("url(a)"), None);
    assert_eq!(parse_paint("red blue"), None);
  }

  #[test]
  fn an_invalid_declaration_leaves_the_earlier_one() {
    let style = style_after(&[
      (Property::StrokeWidth, "4"),
      (Property::StrokeWidth, "wide"),
      (Property::Opacity, "50%"),
      (Property::FillOpacity, "2"),
    ]);

    assert_eq!(style.stroke_width, Length::number(4.0));
    assert_eq!(style.opacity, 0.5);
    assert_eq!(style.fill_opacity, 1.0);
  }

  #[test]
  fn css_wide_keywords_pick_the_parent_or_the_initial_value() {
    let context = StyleContext {
      font_size: 12.0,
      dpi: 96.0,
    };
    let mut parent = SvgStyle::initial(&context);

    parent.opacity = 0.25;
    parent.fill_rule = FillRule::EvenOdd;

    let mut style = parent.inherit(&context);

    style.apply(Property::Opacity, "inherit", &parent, &context);
    style.apply(Property::FillRule, "initial", &parent, &context);

    assert_eq!(style.opacity, 0.25);
    assert_eq!(style.fill_rule, FillRule::NonZero);
    assert_eq!(parent.inherit(&context).opacity, 1.0);
    assert_eq!(parent.inherit(&context).fill_rule, FillRule::EvenOdd);
  }

  #[test]
  fn font_sizes_resolve_against_the_parent() {
    let context = StyleContext {
      font_size: 12.0,
      dpi: 96.0,
    };
    let mut parent = SvgStyle::initial(&context);

    parent.font_size = 20.0;

    let mut style = parent.inherit(&context);

    style.apply(Property::FontSize, "2em", &parent, &context);
    assert_eq!(style.font_size, 40.0);
    style.apply(Property::FontSize, "50%", &parent, &context);
    assert_eq!(style.font_size, 10.0);
    style.apply(Property::FontSize, "larger", &parent, &context);
    assert_eq!(style.font_size, 24.0);
    style.apply(Property::FontSize, "large", &parent, &context);
    assert!((style.font_size - 14.4).abs() < 1e-4);
  }

  #[test]
  fn lists_and_references_parse() {
    assert_eq!(
      parse_dasharray("5, 3"),
      Some(Dasharray::List(vec![
        Length::number(5.0),
        Length::number(3.0)
      ]))
    );
    assert_eq!(parse_dasharray("5 -3"), None);
    assert_eq!(parse_reference("url(#m)"), Some(Some(Arc::from("m"))));
    assert_eq!(parse_reference("none"), Some(None));
    assert_eq!(parse_reference("m"), None);
    assert!(parse_transform("rotate(90)").is_some());
    assert_eq!(parse_transform("rotate(90deg)"), None);
  }
}
