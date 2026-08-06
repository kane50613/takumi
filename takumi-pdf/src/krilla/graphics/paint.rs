//! Paints that can be used for filling and stroking text or paths.

use std::hash::{Hash, Hasher};
use std::sync::Arc;

use pdf_writer::types::{LineCapStyle, LineJoinStyle};

use crate::krilla::color::SpecialColor;
use crate::krilla::geom::Transform;
use crate::krilla::graphics::color::{Color, cmyk, luma, rgb};
use crate::krilla::num::NormalizedF32;
use crate::krilla::stream::Stream;

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
  pub transform: Transform,
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
  pub transform: Transform,
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
  pub transform: Transform,
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
  pub transform: Transform,
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
      InnerPaint::Color(c) => match c {
        Color::Regular(c) => c.as_rgb(),
        Color::Special(SpecialColor::Separation(c)) => c.space.fallback.as_rgb(),
      },
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

impl From<cmyk::Color> for Paint {
  fn from(value: cmyk::Color) -> Self {
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

/// A spread method.
#[derive(Debug, Hash, Eq, PartialEq, Copy, Clone, Default)]
pub enum SpreadMethod {
  /// The pad spread method.
  #[default]
  Pad,
  /// The reflect spread method.
  Reflect,
  /// The repeat spread method.
  Repeat,
}

/// A color stop in a gradient.
#[derive(Debug, Hash, Eq, PartialEq, Clone)]
#[allow(private_bounds)]
pub struct Stop {
  /// The normalized offset of the stop.
  pub offset: NormalizedF32,
  /// The color of the stop.
  pub color: Color,
  /// The opacity of the stop.
  pub opacity: NormalizedF32,
}

/// A line cap.
#[derive(Eq, PartialEq, Debug, Clone, Copy, Default, Hash)]
pub enum LineCap {
  /// The butt line cap.
  #[default]
  Butt,
  /// The round line cap.
  Round,
  /// The square line cap.
  Square,
}

impl LineCap {
  pub(crate) fn to_pdf_line_cap(self) -> LineCapStyle {
    match self {
      LineCap::Butt => LineCapStyle::ButtCap,
      LineCap::Round => LineCapStyle::RoundCap,
      LineCap::Square => LineCapStyle::ProjectingSquareCap,
    }
  }
}

/// A line join.
#[derive(PartialEq, Eq, Debug, Clone, Copy, Default, Hash)]
pub enum LineJoin {
  /// The miter line join.
  #[default]
  Miter,
  /// The round line join.
  Round,
  /// The bevel line join.
  Bevel,
}

impl LineJoin {
  pub(crate) fn to_pdf_line_join(self) -> LineJoinStyle {
    match self {
      LineJoin::Miter => LineJoinStyle::MiterJoin,
      LineJoin::Round => LineJoinStyle::RoundJoin,
      LineJoin::Bevel => LineJoinStyle::BevelJoin,
    }
  }
}

/// A stroke dash.
#[derive(Debug, Clone, PartialEq)]
pub struct StrokeDash {
  /// The dash array.
  pub array: Vec<f32>,
  /// The offset of the dash.
  pub offset: f32,
}

impl Eq for StrokeDash {}

impl Hash for StrokeDash {
  fn hash<H: Hasher>(&self, state: &mut H) {
    for el in &self.array {
      el.to_bits().hash(state);
    }

    self.offset.to_bits().hash(state);
  }
}

/// A stroke.
#[derive(Debug, Clone, PartialEq)]
pub struct Stroke {
  /// The paint of the stroke.
  pub paint: Paint,
  /// The width of the stroke.
  pub width: f32,
  /// The miter limit of the stroke.
  pub miter_limit: f32,
  /// The line cap of the stroke.
  pub line_cap: LineCap,
  /// The line join of the stroke.
  pub line_join: LineJoin,
  /// The opacity of the stroke.
  pub opacity: NormalizedF32,
  /// The (optional) dash of the stroke.
  pub dash: Option<StrokeDash>,
}

impl Eq for Stroke {}

impl Hash for Stroke {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.paint.hash(state);
    self.width.to_bits().hash(state);
    self.miter_limit.to_bits().hash(state);
    self.line_cap.hash(state);
    self.line_join.hash(state);
    self.opacity.hash(state);
    self.dash.hash(state);
  }
}

impl Default for Stroke {
  fn default() -> Self {
    Stroke {
      paint: luma::Color::black().into(),
      width: 1.0,
      miter_limit: 10.0,
      line_cap: LineCap::default(),
      line_join: LineJoin::default(),
      opacity: NormalizedF32::ONE,
      dash: None,
    }
  }
}

impl Stroke {
  pub(crate) fn into_tiny_skia(self) -> tiny_skia_path::Stroke {
    let mut stroke = tiny_skia_path::Stroke {
      width: self.width,
      miter_limit: self.miter_limit,
      line_cap: match self.line_cap {
        LineCap::Butt => tiny_skia_path::LineCap::Butt,
        LineCap::Round => tiny_skia_path::LineCap::Round,
        LineCap::Square => tiny_skia_path::LineCap::Square,
      },
      line_join: match self.line_join {
        LineJoin::Miter => tiny_skia_path::LineJoin::Miter,
        LineJoin::Round => tiny_skia_path::LineJoin::Round,
        LineJoin::Bevel => tiny_skia_path::LineJoin::Bevel,
      },
      dash: None,
    };

    if let Some(stroke_dash) = self.dash {
      stroke.dash = tiny_skia_path::StrokeDash::new(stroke_dash.array, stroke_dash.offset);
    }

    stroke
  }
}

/// A fill rule.
#[derive(Debug, Eq, PartialEq, Clone, Copy, Hash, Default)]
pub enum FillRule {
  /// The `non-zero` fill rule.
  #[default]
  NonZero,
  /// The `even-odd` fill rule.
  EvenOdd,
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
