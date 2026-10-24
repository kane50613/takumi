//! The computed style of an SVG element, after Blink's `ComputedStyle` and `SVGComputedStyle`.
//!
//! A declaration that does not parse is dropped, so the next lower declaration or the inherited
//! value applies, as in Blink's CSS parser.

use std::sync::Arc;

use bitflags::bitflags;

use super::{length::Length, transform::TransformOrigin};
use crate::style::{
  BlendMode, Color, ColorInput, FillRule, FontFamily, FontKerning, FontOpticalSizing, FontStretch,
  FontStyle, FontVariationSettings, Isolation, Visibility, impl_css_enum,
};

/// A paint to fall back to when a `url()` paint server is missing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PaintFallback {
  /// No paint.
  None,
  /// The `color` property.
  CurrentColor,
  /// A color.
  Color(Color),
}

/// The value of `fill` and `stroke`.
#[derive(Debug, Clone, PartialEq)]
pub enum Paint {
  /// No paint.
  None,
  /// The `color` property.
  CurrentColor,
  /// A color.
  Color(Color),
  /// A paint server by id, with the paint to use when it is missing.
  Url(Arc<str>, Option<PaintFallback>),
  /// The fill of the context element of a marker or `use`.
  ContextFill,
  /// The stroke of the context element of a marker or `use`.
  ContextStroke,
}

/// A part of a shape that `paint-order` orders.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaintOrderPart {
  /// The fill.
  Fill,
  /// The stroke.
  Stroke,
  /// The markers.
  Markers,
}

/// The value of `paint-order`, listing every part once.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaintOrder(pub [PaintOrderPart; 3]);

impl Default for PaintOrder {
  fn default() -> Self {
    Self([
      PaintOrderPart::Fill,
      PaintOrderPart::Stroke,
      PaintOrderPart::Markers,
    ])
  }
}

impl PaintOrder {
  pub(super) fn parse(text: &str) -> Option<Self> {
    let mut parts = Vec::new();

    for word in text.split_ascii_whitespace() {
      let part = match word {
        "normal" if parts.is_empty() => return Some(Self::default()),
        "fill" => PaintOrderPart::Fill,
        "stroke" => PaintOrderPart::Stroke,
        "markers" => PaintOrderPart::Markers,
        _ => return None,
      };

      if parts.contains(&part) {
        return None;
      }
      parts.push(part);
    }
    if parts.is_empty() {
      return None;
    }

    for part in Self::default().0 {
      if !parts.contains(&part) {
        parts.push(part);
      }
    }
    Some(Self([parts[0], parts[1], parts[2]]))
  }
}

/// A resource reference such as `marker-start` or `mask`, by id.
pub type Reference = Option<Arc<str>>;

/// One entry of the `filter` list.
#[derive(Debug, Clone, PartialEq)]
pub enum FilterValue {
  /// A `<filter>` element by id.
  Url(Arc<str>),
  /// A CSS filter function.
  Function(crate::style::Filter),
}

/// The value of `stroke-dasharray`.
#[derive(Debug, Clone, PartialEq)]
pub enum Dasharray {
  /// A solid stroke.
  None,
  /// The dash and gap lengths.
  List(Vec<Length>),
}

/// The value of `letter-spacing` and `word-spacing`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Spacing {
  /// The font's own spacing.
  Normal,
  /// An added length.
  Length(Length),
}

/// The value of `baseline-shift`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BaselineShift {
  /// On the parent's baseline.
  Baseline,
  /// To the subscript position.
  Sub,
  /// To the superscript position.
  Super,
  /// By a length, or by a percentage of the line height.
  Length(Length),
}

bitflags! {
  /// The `text-decoration` lines.
  #[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
  pub struct TextDecoration: u8 {
    /// A line under the text.
    const UNDERLINE = 0b001;
    /// A line through the text.
    const LINE_THROUGH = 0b010;
    /// A line over the text.
    const OVERLINE = 0b100;
  }
}

/// The value of `display`: whether the element renders.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Display {
  /// The element renders.
  #[default]
  Inline,
  /// The element and its descendants do not render.
  None,
}

/// The value of `stroke-linecap`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LineCap {
  /// A flat edge at the end.
  #[default]
  Butt,
  /// A semicircle past the end.
  Round,
  /// A half square past the end.
  Square,
}

impl_css_enum!(
  LineCap,
  "butt" => LineCap::Butt,
  "round" => LineCap::Round,
  "square" => LineCap::Square,
);

/// The value of `stroke-linejoin`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LineJoin {
  /// A sharp corner up to the miter limit.
  #[default]
  Miter,
  /// A sharp corner clipped at the miter limit.
  MiterClip,
  /// A rounded corner.
  Round,
  /// A flat corner.
  Bevel,
}

impl_css_enum!(
  LineJoin,
  "miter" => LineJoin::Miter,
  "miter-clip" => LineJoin::MiterClip,
  "round" => LineJoin::Round,
  "bevel" => LineJoin::Bevel,
);

/// The value of `text-anchor`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextAnchor {
  /// The chunk starts at the position.
  #[default]
  Start,
  /// The chunk centers on the position.
  Middle,
  /// The chunk ends at the position.
  End,
}

impl_css_enum!(
  TextAnchor,
  "start" => TextAnchor::Start,
  "middle" => TextAnchor::Middle,
  "end" => TextAnchor::End,
);

/// The value of `shape-rendering`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ShapeRendering {
  /// Anti-aliased geometry.
  #[default]
  GeometricPrecision,
  /// No anti-aliasing.
  CrispEdges,
  /// No anti-aliasing, favoring speed.
  OptimizeSpeed,
}

impl ShapeRendering {
  /// Whether shapes are anti-aliased.
  pub fn anti_alias(self) -> bool {
    self == Self::GeometricPrecision
  }
}

impl_css_enum!(
  ShapeRendering,
  "geometricPrecision" | "auto" => ShapeRendering::GeometricPrecision,
  "crispEdges" => ShapeRendering::CrispEdges,
  "optimizeSpeed" => ShapeRendering::OptimizeSpeed,
);

/// The value of `text-rendering`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextRendering {
  /// Anti-aliased outlines.
  #[default]
  OptimizeLegibility,
  /// Anti-aliased outlines, favoring speed.
  OptimizeSpeed,
  /// Anti-aliased outlines, favoring exact geometry.
  GeometricPrecision,
}

impl_css_enum!(
  TextRendering,
  "optimizeLegibility" | "auto" => TextRendering::OptimizeLegibility,
  "optimizeSpeed" => TextRendering::OptimizeSpeed,
  "geometricPrecision" => TextRendering::GeometricPrecision,
);

/// The value of `image-rendering`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ImageRendering {
  /// Smooth resampling.
  #[default]
  OptimizeQuality,
  /// Nearest-neighbor resampling.
  OptimizeSpeed,
  /// Smooth resampling.
  Smooth,
  /// Smooth resampling.
  HighQuality,
  /// Nearest-neighbor resampling.
  CrispEdges,
  /// Nearest-neighbor resampling.
  Pixelated,
}

impl ImageRendering {
  /// Whether the image resamples without smoothing.
  pub fn is_pixelated(self) -> bool {
    matches!(
      self,
      Self::OptimizeSpeed | Self::CrispEdges | Self::Pixelated
    )
  }
}

impl_css_enum!(
  ImageRendering,
  "optimizeQuality" | "auto" => ImageRendering::OptimizeQuality,
  "optimizeSpeed" => ImageRendering::OptimizeSpeed,
  "smooth" => ImageRendering::Smooth,
  "high-quality" => ImageRendering::HighQuality,
  "crisp-edges" => ImageRendering::CrispEdges,
  "pixelated" => ImageRendering::Pixelated,
);

/// The value of `color-interpolation-filters`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorInterpolation {
  /// Filters compute in linear RGB.
  #[default]
  LinearRgb,
  /// Filters compute in sRGB.
  Srgb,
}

impl_css_enum!(
  ColorInterpolation,
  "linearRGB" => ColorInterpolation::LinearRgb,
  "sRGB" | "auto" => ColorInterpolation::Srgb,
);

/// The value of `mask-type`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MaskType {
  /// The mask's luminance.
  #[default]
  Luminance,
  /// The mask's alpha.
  Alpha,
}

impl_css_enum!(
  MaskType,
  "luminance" => MaskType::Luminance,
  "alpha" => MaskType::Alpha,
);

/// The value of `overflow`: whether the viewport of the element clips.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Overflow {
  /// Content past the viewport shows.
  #[default]
  Visible,
  /// Content past the viewport is clipped.
  Hidden,
  /// Content past the viewport is clipped.
  Scroll,
  /// Content past the viewport shows.
  Auto,
}

impl Overflow {
  /// Whether content past the viewport is clipped.
  pub fn clips(self) -> bool {
    matches!(self, Self::Hidden | Self::Scroll)
  }
}

impl_css_enum!(
  Overflow,
  "visible" => Overflow::Visible,
  "hidden" => Overflow::Hidden,
  "scroll" => Overflow::Scroll,
  "auto" => Overflow::Auto,
);

/// The value of `vector-effect`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VectorEffect {
  /// Strokes scale with the transform.
  #[default]
  None,
  /// Strokes keep their width on screen.
  NonScalingStroke,
}

impl_css_enum!(
  VectorEffect,
  "none" => VectorEffect::None,
  "non-scaling-stroke" => VectorEffect::NonScalingStroke,
);

/// The value of `dominant-baseline`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DominantBaseline {
  /// The script's default baseline.
  #[default]
  Auto,
  /// The script's default baseline.
  UseScript,
  /// The parent's baseline.
  NoChange,
  /// The parent's baseline.
  ResetSize,
  /// The ideographic baseline.
  Ideographic,
  /// The alphabetic baseline.
  Alphabetic,
  /// The hanging baseline.
  Hanging,
  /// The mathematical baseline.
  Mathematical,
  /// The central baseline.
  Central,
  /// The middle baseline.
  Middle,
  /// The text-after-edge baseline.
  TextAfterEdge,
  /// The text-before-edge baseline.
  TextBeforeEdge,
}

impl_css_enum!(
  DominantBaseline,
  "auto" => DominantBaseline::Auto,
  "use-script" => DominantBaseline::UseScript,
  "no-change" => DominantBaseline::NoChange,
  "reset-size" => DominantBaseline::ResetSize,
  "ideographic" => DominantBaseline::Ideographic,
  "alphabetic" => DominantBaseline::Alphabetic,
  "hanging" => DominantBaseline::Hanging,
  "mathematical" => DominantBaseline::Mathematical,
  "central" => DominantBaseline::Central,
  "middle" => DominantBaseline::Middle,
  "text-after-edge" => DominantBaseline::TextAfterEdge,
  "text-before-edge" => DominantBaseline::TextBeforeEdge,
);

/// The value of `alignment-baseline`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AlignmentBaseline {
  /// The dominant baseline.
  #[default]
  Auto,
  /// The parent's baseline.
  Baseline,
  /// The before edge.
  BeforeEdge,
  /// The text-before-edge baseline.
  TextBeforeEdge,
  /// The middle baseline.
  Middle,
  /// The central baseline.
  Central,
  /// The after edge.
  AfterEdge,
  /// The text-after-edge baseline.
  TextAfterEdge,
  /// The ideographic baseline.
  Ideographic,
  /// The alphabetic baseline.
  Alphabetic,
  /// The hanging baseline.
  Hanging,
  /// The mathematical baseline.
  Mathematical,
}

impl_css_enum!(
  AlignmentBaseline,
  "auto" => AlignmentBaseline::Auto,
  "baseline" => AlignmentBaseline::Baseline,
  "before-edge" => AlignmentBaseline::BeforeEdge,
  "text-before-edge" => AlignmentBaseline::TextBeforeEdge,
  "middle" => AlignmentBaseline::Middle,
  "central" => AlignmentBaseline::Central,
  "after-edge" => AlignmentBaseline::AfterEdge,
  "text-after-edge" => AlignmentBaseline::TextAfterEdge,
  "ideographic" => AlignmentBaseline::Ideographic,
  "alphabetic" => AlignmentBaseline::Alphabetic,
  "hanging" => AlignmentBaseline::Hanging,
  "mathematical" => AlignmentBaseline::Mathematical,
);

/// What lengths and font sizes resolve against while styles compute.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StyleContext {
  /// The font size of the root, in pixels.
  pub font_size: f32,
  /// The pixels per inch that absolute units convert with.
  pub dpi: f32,
}

impl Paint {
  fn uses_current_color(&self) -> bool {
    matches!(
      self,
      Self::CurrentColor | Self::Url(_, Some(PaintFallback::CurrentColor))
    )
  }
}

/// The style properties of one element, after the cascade and inheritance.
#[derive(Debug, Clone, PartialEq)]
pub struct SvgStyle {
  /// `display`
  pub display: Display,
  /// `visibility`
  pub visibility: Visibility,
  /// `opacity`
  pub opacity: f32,
  /// `color`, or `None` for the color of the host the SVG is drawn in.
  pub color: Option<Color>,
  /// `background-color`
  pub background_color: ColorInput,
  /// `fill`
  pub fill: Paint,
  /// `fill-opacity`
  pub fill_opacity: f32,
  /// `fill-rule`
  pub fill_rule: FillRule,
  /// `clip-rule`
  pub clip_rule: FillRule,
  /// `stroke`
  pub stroke: Paint,
  /// `stroke-opacity`
  pub stroke_opacity: f32,
  /// `stroke-width`
  pub stroke_width: Length,
  /// `stroke-linecap`
  pub stroke_linecap: LineCap,
  /// `stroke-linejoin`
  pub stroke_linejoin: LineJoin,
  /// `stroke-miterlimit`
  pub stroke_miterlimit: f32,
  /// `stroke-dasharray`
  pub stroke_dasharray: Dasharray,
  /// `stroke-dashoffset`
  pub stroke_dashoffset: Length,
  /// `paint-order`
  pub paint_order: PaintOrder,
  /// `marker-start`
  pub marker_start: Reference,
  /// `marker-mid`
  pub marker_mid: Reference,
  /// `marker-end`
  pub marker_end: Reference,
  /// `clip-path`
  pub clip_path: Reference,
  /// `mask`
  pub mask: Reference,
  /// `mask-type`
  pub mask_type: MaskType,
  /// `filter`
  pub filter: Vec<FilterValue>,
  /// `flood-color`
  pub flood_color: ColorInput,
  /// `flood-opacity`
  pub flood_opacity: f32,
  /// `lighting-color`
  pub lighting_color: ColorInput,
  /// `stop-color`
  pub stop_color: ColorInput,
  /// `stop-opacity`
  pub stop_opacity: f32,
  /// `color-interpolation-filters`
  pub color_interpolation_filters: ColorInterpolation,
  /// `shape-rendering`
  pub shape_rendering: ShapeRendering,
  /// `image-rendering`
  pub image_rendering: ImageRendering,
  /// `text-rendering`
  pub text_rendering: TextRendering,
  /// `mix-blend-mode`
  pub mix_blend_mode: BlendMode,
  /// `isolation`
  pub isolation: Isolation,
  /// `overflow`
  pub overflow: Overflow,
  /// `vector-effect`
  pub vector_effect: VectorEffect,
  /// `transform`
  pub transform: Option<tiny_skia::Transform>,
  /// `transform-origin`
  pub transform_origin: Option<TransformOrigin>,
  /// `font-family`
  pub font_family: Option<FontFamily>,
  /// `font-size`, in pixels.
  pub font_size: f32,
  /// `font-style`
  pub font_style: FontStyle,
  /// `font-weight`, with `bolder` and `lighter` resolved.
  pub font_weight: f32,
  /// `font-stretch`
  pub font_stretch: FontStretch,
  /// `font-variant`, whether it asks for small caps.
  pub small_caps: bool,
  /// `font-kerning`
  pub font_kerning: FontKerning,
  /// `font-optical-sizing`
  pub font_optical_sizing: FontOpticalSizing,
  /// `font-variation-settings`
  pub font_variation_settings: FontVariationSettings,
  /// `letter-spacing`
  pub letter_spacing: Spacing,
  /// `word-spacing`
  pub word_spacing: Spacing,
  /// `text-anchor`
  pub text_anchor: TextAnchor,
  /// `text-decoration`
  pub text_decoration: TextDecoration,
  /// `dominant-baseline`
  pub dominant_baseline: DominantBaseline,
  /// `alignment-baseline`
  pub alignment_baseline: AlignmentBaseline,
  /// `baseline-shift`
  pub baseline_shift: BaselineShift,
  /// Whether white space is preserved, from `xml:space` and `white-space`.
  pub preserve_white_space: bool,
}

impl SvgStyle {
  /// Whether a paint of this element reads the `color` property.
  pub fn uses_current_color(&self) -> bool {
    self.fill.uses_current_color()
      || self.stroke.uses_current_color()
      || self.flood_color == ColorInput::CurrentColor
      || self.lighting_color == ColorInput::CurrentColor
      || self.stop_color == ColorInput::CurrentColor
      || self.background_color == ColorInput::CurrentColor
  }

  /// The initial values of every property.
  pub fn initial(context: &StyleContext) -> Self {
    Self {
      display: Display::Inline,
      visibility: Visibility::Visible,
      opacity: 1.0,
      color: None,
      background_color: ColorInput::transparent(),
      fill: Paint::Color(Color([0, 0, 0, 255])),
      fill_opacity: 1.0,
      fill_rule: FillRule::NonZero,
      clip_rule: FillRule::NonZero,
      stroke: Paint::None,
      stroke_opacity: 1.0,
      stroke_width: Length::number(1.0),
      stroke_linecap: LineCap::Butt,
      stroke_linejoin: LineJoin::Miter,
      stroke_miterlimit: 4.0,
      stroke_dasharray: Dasharray::None,
      stroke_dashoffset: Length::number(0.0),
      paint_order: PaintOrder::default(),
      marker_start: None,
      marker_mid: None,
      marker_end: None,
      clip_path: None,
      mask: None,
      mask_type: MaskType::Luminance,
      filter: Vec::new(),
      flood_color: ColorInput::Value(Color([0, 0, 0, 255])),
      flood_opacity: 1.0,
      lighting_color: ColorInput::Value(Color([255, 255, 255, 255])),
      stop_color: ColorInput::Value(Color([0, 0, 0, 255])),
      stop_opacity: 1.0,
      color_interpolation_filters: ColorInterpolation::LinearRgb,
      shape_rendering: ShapeRendering::GeometricPrecision,
      image_rendering: ImageRendering::OptimizeQuality,
      text_rendering: TextRendering::OptimizeLegibility,
      mix_blend_mode: BlendMode::Normal,
      isolation: Isolation::Auto,
      overflow: Overflow::Visible,
      vector_effect: VectorEffect::None,
      transform: None,
      transform_origin: None,
      font_family: None,
      font_size: context.font_size,
      font_style: FontStyle::default(),
      font_weight: 400.0,
      font_stretch: FontStretch::default(),
      small_caps: false,
      font_kerning: FontKerning::Auto,
      font_optical_sizing: FontOpticalSizing::default(),
      font_variation_settings: FontVariationSettings::from([]),
      letter_spacing: Spacing::Normal,
      word_spacing: Spacing::Normal,
      text_anchor: TextAnchor::Start,
      text_decoration: TextDecoration::empty(),
      dominant_baseline: DominantBaseline::Auto,
      alignment_baseline: AlignmentBaseline::Auto,
      baseline_shift: BaselineShift::Baseline,
      preserve_white_space: false,
    }
  }
}
