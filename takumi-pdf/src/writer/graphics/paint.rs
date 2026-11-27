//! Paints that can be used for filling and stroking text or paths.

use std::hash::{Hash, Hasher};
use std::sync::Arc;
use takumi_core::paint::SpreadMethod;
use takumi_core::style::Affine;

use pdf_writer::types::{LineCapStyle, LineJoinStyle};
use takumi_core::{
  path::Stroke as PathStroke,
  style::{FillRule, LineCap, LineJoin},
};

use crate::writer::graphics::color::{Color, luma, rgb};
use crate::writer::num::NormalizedF32;
use crate::writer::stream::Stream;

/// A linear gradient.
#[derive(Debug, Clone, PartialEq)]
pub struct LinearGradient {
  /// The x coordinate of the first point.
  pub x1: f32,
  /// The y coordinate of the first point.
  pub y1: f32,
  /// The x coordinate of the second point.
  pub x2: f32,
  /// The y coordinate of the second point.
  pub y2: f32,
  /// A transform that should be applied to the linear gradient.
  pub transform: Affine,
  /// The spread method of the linear gradient.
  pub spread_method: SpreadMethod,
  /// The color stops of the linear gradient.
  ///
  /// Note that all stops need to be in the same color space.
  pub stops: Vec<Stop>,
  /// Whether the gradient should be anti-aliased.
  pub anti_alias: bool,
}

impl Eq for LinearGradient {}

impl Hash for LinearGradient {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.x1.to_bits().hash(state);
    self.y1.to_bits().hash(state);
    self.x2.to_bits().hash(state);
    self.y2.to_bits().hash(state);
    self.transform.hash(state);
    self.spread_method.hash(state);
    self.stops.hash(state);
    self.anti_alias.hash(state);
  }
}

/// A radial gradient.
#[derive(Debug, Clone, PartialEq)]
pub struct RadialGradient {
  /// The x coordinate of the start circle.
  pub fx: f32,
  /// The y coordinate of the start circle.
  pub fy: f32,
  /// The radius of the start circle.
  pub fr: f32,
  /// The x coordinate of the end circle.
  pub cx: f32,
  /// The y coordinate of the end circle.
  pub cy: f32,
  /// The radius of the end circle.
  pub cr: f32,
  /// A transform that should be applied to the radial gradient.
  pub transform: Affine,
  /// The spread method of the radial gradient.
  ///
  /// _Note_: The spread methods `Repeat`/`Reflect` are currently not supported
  /// for radial gradients, and will fall back to `Pad`.
  pub spread_method: SpreadMethod,
  /// The color stops of the radial gradient.
  ///
  /// Note that all stops need to be in the same color space.
  pub stops: Vec<Stop>,
  /// Whether the gradient should be anti-aliased.
  pub anti_alias: bool,
}

impl Eq for RadialGradient {}

impl Hash for RadialGradient {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.fx.to_bits().hash(state);
    self.fy.to_bits().hash(state);
    self.fr.to_bits().hash(state);
    self.cx.to_bits().hash(state);
    self.cy.to_bits().hash(state);
    self.cr.to_bits().hash(state);
    self.transform.hash(state);
    self.spread_method.hash(state);
    self.stops.hash(state);
    self.anti_alias.hash(state);
  }
}

/// A sweep gradient.
///
/// Angles start from the right and go counter-clockwise with increasing values.
#[derive(Debug, Clone, PartialEq)]
pub struct SweepGradient {
  /// The x coordinate of the center.
  pub cx: f32,
  /// The y coordinate of the center.
  pub cy: f32,
  /// The start angle.
  pub start_angle: f32,
  /// The end angle.
  pub end_angle: f32,
  /// A transform that should be applied to the sweep gradient.
  pub transform: Affine,
  /// The spread method of the sweep gradient.
  pub spread_method: SpreadMethod,
  /// The color stops of the sweep gradient.
  ///
  /// Note that all stops need to be in the same color space.
  pub stops: Vec<Stop>,
  /// Whether the gradient should be anti-aliased.
  pub anti_alias: bool,
}

impl Eq for SweepGradient {}

impl Hash for SweepGradient {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.cx.to_bits().hash(state);
    self.cy.to_bits().hash(state);
    self.start_angle.to_bits().hash(state);
    self.end_angle.to_bits().hash(state);
    self.transform.hash(state);
    self.spread_method.hash(state);
    self.stops.hash(state);
    self.anti_alias.hash(state);
  }
}

/// A pattern.
///
/// IMPORTANT: Note that you must only use a mask in the document that you created it with!
/// If you use it in a different document, you will end up with an invalid PDF file.
#[derive(Debug, PartialEq, Clone)]
pub struct Pattern {
  /// The stream of the pattern.
  pub stream: Stream,
  /// A transform that should be applied to the pattern.
  pub transform: Affine,
  /// The width of the pattern.
  pub width: f32,
  /// The height of the pattern.
  pub height: f32,
}

impl Eq for Pattern {}

impl Hash for Pattern {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.stream.hash(state);
    self.transform.hash(state);
    self.width.to_bits().hash(state);
    self.height.to_bits().hash(state);
  }
}

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub(crate) enum InnerPaint {
  Color(Color),
  LinearGradient(LinearGradient),
  RadialGradient(RadialGradient),
  SweepGradient(SweepGradient),
  Pattern(Arc<Pattern>),
}

/// A paint.
///
/// You cannot construct this type directly, but instead can convert
/// into it by calling `into` on the various types of paint, such as linear
/// gradients and patterns.
#[derive(Debug, Hash, Eq, PartialEq, Clone)]
pub struct Paint(pub(crate) InnerPaint);

impl Paint {
  pub(crate) fn as_rgb(&self) -> Option<rgb::Color> {
    match &self.0 {
      InnerPaint::Color(c) => Some(c.as_rgb()),
      _ => None,
    }
  }
}

impl From<rgb::Color> for Paint {
  fn from(value: rgb::Color) -> Self {
    Paint(InnerPaint::Color(value.into()))
  }
}

impl From<luma::Color> for Paint {
  fn from(value: luma::Color) -> Self {
    Paint(InnerPaint::Color(value.into()))
  }
}

impl From<Color> for Paint {
  fn from(value: Color) -> Self {
    Paint(InnerPaint::Color(value))
  }
}

impl From<LinearGradient> for Paint {
  fn from(value: LinearGradient) -> Self {
    Paint(InnerPaint::LinearGradient(value))
  }
}

impl From<RadialGradient> for Paint {
  fn from(value: RadialGradient) -> Self {
    Paint(InnerPaint::RadialGradient(value))
  }
}

impl From<SweepGradient> for Paint {
  fn from(value: SweepGradient) -> Self {
    Paint(InnerPaint::SweepGradient(value))
  }
}

impl From<Pattern> for Paint {
  fn from(value: Pattern) -> Self {
    Paint(InnerPaint::Pattern(Arc::new(value)))
  }
}

/// A color stop in a gradient.
#[derive(Debug, Hash, Eq, PartialEq, Clone)]
pub struct Stop {
  /// The normalized offset of the stop.
  pub offset: NormalizedF32,
  /// The color of the stop.
  pub color: Color,
  /// The opacity of the stop.
  pub opacity: NormalizedF32,
}

/// The PDF line cap a stroke's cap writes as.
pub(crate) fn line_cap_style(cap: LineCap) -> LineCapStyle {
  match cap {
    LineCap::Butt => LineCapStyle::ButtCap,
    LineCap::Round => LineCapStyle::RoundCap,
    LineCap::Square => LineCapStyle::ProjectingSquareCap,
  }
}

/// The PDF line join a stroke's join writes as.
pub(crate) fn line_join_style(join: LineJoin) -> LineJoinStyle {
  match join {
    LineJoin::Miter => LineJoinStyle::MiterJoin,
    LineJoin::Round => LineJoinStyle::RoundJoin,
    LineJoin::Bevel => LineJoinStyle::BevelJoin,
  }
}

/// The miter limit PDF assumes when a content stream sets none.
pub(crate) const PDF_MITER_LIMIT: f32 = 10.0;

/// A stroke: its paint and opacity, and the outline it draws.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Stroke {
  /// The paint of the stroke.
  pub paint: Paint,
  /// The opacity of the stroke.
  pub opacity: NormalizedF32,
  /// The width, caps, joins, miter limit, and dashes.
  pub outline: PathStroke,
}

impl Stroke {
  /// A solid stroke of `width` with PDF's default miter limit.
  pub(crate) fn solid(paint: Paint, opacity: NormalizedF32, width: f32) -> Self {
    Self {
      paint,
      opacity,
      outline: PathStroke {
        width,
        miter_limit: PDF_MITER_LIMIT,
        ..PathStroke::default()
      },
    }
  }
}

/// A fill.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Fill {
  /// The paint of the fill.
  pub paint: Paint,
  /// The opacity of the fill.
  pub opacity: NormalizedF32,
  /// The fill rule that should be used when applying the fill.
  pub rule: FillRule,
}

impl Default for Fill {
  fn default() -> Self {
    Fill {
      paint: luma::Color::black().into(),
      opacity: NormalizedF32::ONE,
      rule: FillRule::default(),
    }
  }
}
