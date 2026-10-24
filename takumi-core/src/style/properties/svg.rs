//! The value types of the SVG presentation properties, named after Blink's.

use std::{fmt, sync::Arc};

use cssparser::{Parser, Token, serialize_string};

use crate::{
  geometry::{PathCommand, Point},
  path_data::PathData,
  style::{
    Animatable, Color, ColorInput, FromCss, Length, MakeComputed, ParseResult, SizingContext,
    ToCss, ValueError, discrete, impl_css_enum, lerp, properties::interpolate_repeated_list,
    unexpected_token,
  },
};

fn accepts_keyword<'i>(input: &mut Parser<'i>, keyword: &str) -> bool {
  input
    .try_parse(|input| input.expect_ident_matching(keyword))
    .is_ok()
}

/// Blink's `StyleSVGResource`: a `url()` naming an SVG resource element.
#[derive(Debug, Clone, PartialEq)]
pub struct SvgResource(Arc<str>);

impl SvgResource {
  /// A resource named by `url`, as written inside `url()`.
  pub fn new(url: impl Into<Arc<str>>) -> Self {
    Self(url.into())
  }

  /// The URL as written.
  pub fn url(&self) -> &str {
    &self.0
  }

  /// The fragment it names in the same document, when it names one.
  pub fn fragment(&self) -> Option<&str> {
    self.0.strip_prefix('#')
  }
}

impl MakeComputed for SvgResource {}

impl Animatable for SvgResource {}

impl<'i> FromCss<'i> for SvgResource {
  fn from_css(input: &mut Parser<'i>) -> ParseResult<'i, Self> {
    let token = input.next()?;

    match token {
      Token::UnquotedUrl(url) => Ok(Self(url.as_ref().into())),
      Token::Function(function) if function.eq_ignore_ascii_case("url") => {
        input.parse_nested_block(|input| Ok(Self(input.expect_string()?.as_ref().into())))
      }
      _ => Err(unexpected_token!(token)),
    }
  }

  const VALID_TOKENS: &'static [&'static str] = &["<url>"];
}

impl ToCss for SvgResource {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    dest.write_str("url(")?;
    serialize_string(&self.0, dest)?;
    dest.write_char(')')
  }
}

/// Blink's `SVGPaint`: what `fill` or `stroke` paints with.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum SvgPaint {
  /// Paints nothing.
  None,
  /// A flat color.
  Color(ColorInput),
  /// A paint server, and the color to paint when it does not resolve.
  Url(SvgResource, Option<ColorInput>),
  /// The fill of the element that references the `marker` or `use` context.
  ContextFill,
  /// The stroke of the element that references the `marker` or `use` context.
  ContextStroke,
}

impl MakeComputed for SvgPaint {}

impl Animatable for SvgPaint {
  fn interpolate(
    &mut self,
    from: &Self,
    to: &Self,
    progress: f32,
    sizing: &SizingContext,
    current_color: Color,
  ) {
    *self = match (from, to) {
      (Self::Color(from), Self::Color(to)) => Self::Color(ColorInput::interpolated(
        from,
        to,
        progress,
        sizing,
        current_color,
      )),
      _ => discrete(from, to, progress),
    };
  }
}

impl<'i> FromCss<'i> for SvgPaint {
  /// Blink's `css_parsing_utils::ParsePaintStroke`.
  fn from_css(input: &mut Parser<'i>) -> ParseResult<'i, Self> {
    if let Ok(resource) = input.try_parse(SvgResource::from_css) {
      let fallback = input
        .try_parse(|input| {
          if accepts_keyword(input, "none") {
            return Ok(None);
          }

          ColorInput::from_css(input).map(Some)
        })
        .unwrap_or(None);

      return Ok(Self::Url(resource, fallback));
    }

    for (keyword, paint) in [
      ("none", Self::None),
      ("context-fill", Self::ContextFill),
      ("context-stroke", Self::ContextStroke),
    ] {
      if accepts_keyword(input, keyword) {
        return Ok(paint);
      }
    }

    ColorInput::from_css(input).map(Self::Color)
  }

  const VALID_TOKENS: &'static [&'static str] =
    &["none", "context-fill", "context-stroke", "<color>", "<url>"];
}

impl ToCss for SvgPaint {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    match self {
      Self::None => dest.write_str("none"),
      Self::Color(color) => color.to_css(dest),
      Self::Url(resource, fallback) => {
        resource.to_css(dest)?;

        if let Some(fallback) = fallback {
          dest.write_char(' ')?;
          fallback.to_css(dest)?;
        }

        Ok(())
      }
      Self::ContextFill => dest.write_str("context-fill"),
      Self::ContextStroke => dest.write_str("context-stroke"),
    }
  }
}

/// Blink's `SVGDashArray`: the dashes of `stroke-dasharray`.
#[derive(Debug, Clone, PartialEq)]
pub struct StrokeDasharray(Box<[Length]>);

impl StrokeDasharray {
  /// A dash array of `lengths`.
  pub fn new(lengths: impl Into<Box<[Length]>>) -> Self {
    Self(lengths.into())
  }

  /// The dash and gap lengths, in order.
  pub fn lengths(&self) -> &[Length] {
    &self.0
  }

  fn item<'i>(input: &mut Parser<'i>) -> ParseResult<'i, Length> {
    let length = Length::from_css(input)?;

    if matches!(length, Length::Auto) || length.is_negative() {
      return Err(ValueError::invalid("a dash must be a non-negative length"));
    }

    Ok(length)
  }
}

impl MakeComputed for StrokeDasharray {
  fn make_computed(&mut self, sizing: &SizingContext) {
    self.0.make_computed(sizing);
  }
}

impl Animatable for StrokeDasharray {
  /// Repeats both lists to their least common multiple, as Blink's `CSSLengthListInterpolationType`.
  fn interpolate(
    &mut self,
    from: &Self,
    to: &Self,
    progress: f32,
    sizing: &SizingContext,
    current_color: Color,
  ) {
    *self = interpolate_repeated_list(&from.0, &to.0, progress, sizing, current_color).map_or_else(
      || discrete(from, to, progress),
      |lengths| Self(lengths.into()),
    );
  }
}

impl<'i> FromCss<'i> for StrokeDasharray {
  fn from_css(input: &mut Parser<'i>) -> ParseResult<'i, Self> {
    let mut lengths = vec![Self::item(input)?];

    while let Ok(length) = input.try_parse(|input| {
      let _ = input.try_parse(Parser::expect_comma);

      Self::item(input)
    }) {
      lengths.push(length);
    }

    Ok(Self(lengths.into()))
  }

  const VALID_TOKENS: &'static [&'static str] = &["<length-percentage>", "<number>"];
}

impl ToCss for StrokeDasharray {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    self.0.to_css(dest)
  }
}

/// `stroke-miterlimit`, a number of at least one when it draws.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StrokeMiterlimit(f32);

impl StrokeMiterlimit {
  /// A miter limit of `limit`.
  pub const fn new(limit: f32) -> Self {
    Self(limit)
  }

  /// The limit a stroke draws with, which Blink raises to one.
  pub const fn get(self) -> f32 {
    self.0.max(1.0)
  }
}

impl Default for StrokeMiterlimit {
  fn default() -> Self {
    Self(4.0)
  }
}

impl MakeComputed for StrokeMiterlimit {}

impl Animatable for StrokeMiterlimit {
  fn interpolate(
    &mut self,
    from: &Self,
    to: &Self,
    progress: f32,
    _sizing: &SizingContext,
    _current_color: Color,
  ) {
    self.0 = lerp(from.0, to.0, progress);
  }
}

impl<'i> FromCss<'i> for StrokeMiterlimit {
  fn from_css(input: &mut Parser<'i>) -> ParseResult<'i, Self> {
    let limit = input.expect_number()?;

    if limit < 0.0 {
      return Err(ValueError::invalid("a miter limit must not be negative"));
    }

    Ok(Self(limit))
  }

  const VALID_TOKENS: &'static [&'static str] = &["<number>"];
}

impl ToCss for StrokeMiterlimit {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    self.0.to_css(dest)
  }
}

/// Blink's `LineCap`, as `stroke-linecap` sets it.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum LineCap {
  /// The stroke ends flat at the endpoint.
  #[default]
  Butt,
  /// The stroke ends in a half circle past the endpoint.
  Round,
  /// The stroke ends in a half square past the endpoint.
  Square,
}

impl_css_enum!(
  LineCap,
  "butt" => LineCap::Butt,
  "round" => LineCap::Round,
  "square" => LineCap::Square,
);

/// One phase `paint-order` orders.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PaintPhase {
  /// The fill.
  Fill,
  /// The stroke.
  Stroke,
  /// The markers.
  Markers,
}

/// Blink's `EPaintOrder`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum PaintOrder {
  /// Fill, stroke, then markers.
  #[default]
  Normal,
  /// Fill, stroke, then markers, written out.
  FillStrokeMarkers,
  /// Fill, markers, then stroke.
  FillMarkersStroke,
  /// Stroke, fill, then markers.
  StrokeFillMarkers,
  /// Stroke, markers, then fill.
  StrokeMarkersFill,
  /// Markers, fill, then stroke.
  MarkersFillStroke,
  /// Markers, stroke, then fill.
  MarkersStrokeFill,
}

impl PaintOrder {
  /// The three paint phases in painting order.
  pub fn phases(self) -> [PaintPhase; 3] {
    use PaintPhase::{Fill, Markers, Stroke};

    match self {
      Self::Normal | Self::FillStrokeMarkers => [Fill, Stroke, Markers],
      Self::FillMarkersStroke => [Fill, Markers, Stroke],
      Self::StrokeFillMarkers => [Stroke, Fill, Markers],
      Self::StrokeMarkersFill => [Stroke, Markers, Fill],
      Self::MarkersFillStroke => [Markers, Fill, Stroke],
      Self::MarkersStrokeFill => [Markers, Stroke, Fill],
    }
  }

  /// Whether the stroke paints before the fill, with markers wherever they fall.
  #[cfg(feature = "svg")]
  pub(crate) fn strokes_first(self) -> bool {
    matches!(
      self.phases(),
      [PaintPhase::Stroke, ..] | [PaintPhase::Markers, PaintPhase::Stroke, _]
    )
  }

  fn from_phases(phases: [PaintPhase; 3]) -> Self {
    use PaintPhase::{Fill, Markers, Stroke};

    match phases {
      [Fill, Stroke, Markers] => Self::FillStrokeMarkers,
      [Fill, Markers, Stroke] => Self::FillMarkersStroke,
      [Stroke, Fill, Markers] => Self::StrokeFillMarkers,
      [Stroke, Markers, Fill] => Self::StrokeMarkersFill,
      [Markers, Fill, Stroke] => Self::MarkersFillStroke,
      [Markers, Stroke, Fill] => Self::MarkersStrokeFill,
      _ => Self::Normal,
    }
  }
}

impl MakeComputed for PaintOrder {}

impl Animatable for PaintOrder {}

impl<'i> FromCss<'i> for PaintOrder {
  /// Blink's `PaintOrder::ParseSingleValue`: the phases not named follow in natural order.
  fn from_css(input: &mut Parser<'i>) -> ParseResult<'i, Self> {
    if accepts_keyword(input, "normal") {
      return Ok(Self::Normal);
    }

    let mut phases = Vec::with_capacity(3);

    while phases.len() < 3 {
      let Ok(phase) = input.try_parse(|input| {
        let ident = input.expect_ident()?;

        [
          ("fill", PaintPhase::Fill),
          ("stroke", PaintPhase::Stroke),
          ("markers", PaintPhase::Markers),
        ]
        .into_iter()
        .find(|(keyword, _)| ident.eq_ignore_ascii_case(keyword))
        .map(|(_, phase)| phase)
        .ok_or_else(|| ValueError::invalid("expected fill, stroke, or markers"))
      }) else {
        break;
      };

      if phases.contains(&phase) {
        return Err(ValueError::invalid("a paint phase may be named once"));
      }

      phases.push(phase);
    }

    if phases.is_empty() {
      return Err(ValueError::invalid(
        "expected normal, fill, stroke, or markers",
      ));
    }

    for phase in [PaintPhase::Fill, PaintPhase::Stroke, PaintPhase::Markers] {
      if !phases.contains(&phase) {
        phases.push(phase);
      }
    }

    Ok(<[PaintPhase; 3]>::try_from(phases).map_or(Self::Normal, Self::from_phases))
  }

  const VALID_TOKENS: &'static [&'static str] = &["normal", "fill", "stroke", "markers"];
}

impl PaintOrder {
  /// How many phases Blink's `PaintOrderArray::CanonicalLength` writes: the rest follow in natural
  /// order.
  fn canonical_length(self) -> usize {
    match self {
      Self::Normal
      | Self::FillStrokeMarkers
      | Self::StrokeFillMarkers
      | Self::MarkersFillStroke => 1,
      Self::FillMarkersStroke | Self::StrokeMarkersFill | Self::MarkersStrokeFill => 2,
    }
  }
}

impl ToCss for PaintOrder {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    if matches!(self, Self::Normal) {
      return dest.write_str("normal");
    }

    for (index, phase) in self
      .phases()
      .into_iter()
      .take(self.canonical_length())
      .enumerate()
    {
      if index > 0 {
        dest.write_char(' ')?;
      }

      dest.write_str(match phase {
        PaintPhase::Fill => "fill",
        PaintPhase::Stroke => "stroke",
        PaintPhase::Markers => "markers",
      })?;
    }

    Ok(())
  }
}

/// Blink's `ETextAnchor`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum TextAnchor {
  /// Text starts at the anchor.
  #[default]
  Start,
  /// Text is centered on the anchor.
  Middle,
  /// Text ends at the anchor.
  End,
}

impl_css_enum!(
  TextAnchor,
  "start" => TextAnchor::Start,
  "middle" => TextAnchor::Middle,
  "end" => TextAnchor::End,
);

/// Blink's `EDominantBaseline`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum DominantBaseline {
  /// The baseline the writing mode implies.
  #[default]
  Auto,
  /// The alphabetic baseline.
  Alphabetic,
  /// The ideographic baseline.
  Ideographic,
  /// The middle baseline.
  Middle,
  /// The central baseline.
  Central,
  /// The mathematical baseline.
  Mathematical,
  /// The hanging baseline.
  Hanging,
  /// The baseline of the dominant script.
  UseScript,
  /// The baseline of the parent.
  NoChange,
  /// The baseline of the parent, with the font size reset.
  ResetSize,
  /// The text-after-edge baseline.
  TextAfterEdge,
  /// The text-before-edge baseline.
  TextBeforeEdge,
}

impl_css_enum!(
  DominantBaseline,
  "auto" => DominantBaseline::Auto,
  "alphabetic" => DominantBaseline::Alphabetic,
  "ideographic" => DominantBaseline::Ideographic,
  "middle" => DominantBaseline::Middle,
  "central" => DominantBaseline::Central,
  "mathematical" => DominantBaseline::Mathematical,
  "hanging" => DominantBaseline::Hanging,
  "use-script" => DominantBaseline::UseScript,
  "no-change" => DominantBaseline::NoChange,
  "reset-size" => DominantBaseline::ResetSize,
  "text-after-edge" => DominantBaseline::TextAfterEdge,
  "text-before-edge" => DominantBaseline::TextBeforeEdge,
);

/// Blink's `EAlignmentBaseline`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum AlignmentBaseline {
  /// The baseline the parent implies.
  #[default]
  Auto,
  /// The dominant baseline.
  Baseline,
  /// The alphabetic baseline.
  Alphabetic,
  /// The ideographic baseline.
  Ideographic,
  /// The middle baseline.
  Middle,
  /// The central baseline.
  Central,
  /// The mathematical baseline.
  Mathematical,
  /// The before-edge baseline.
  BeforeEdge,
  /// The text-before-edge baseline.
  TextBeforeEdge,
  /// The after-edge baseline.
  AfterEdge,
  /// The text-after-edge baseline.
  TextAfterEdge,
  /// The hanging baseline.
  Hanging,
}

impl_css_enum!(
  AlignmentBaseline,
  "auto" => AlignmentBaseline::Auto,
  "baseline" => AlignmentBaseline::Baseline,
  "alphabetic" => AlignmentBaseline::Alphabetic,
  "ideographic" => AlignmentBaseline::Ideographic,
  "middle" => AlignmentBaseline::Middle,
  "central" => AlignmentBaseline::Central,
  "mathematical" => AlignmentBaseline::Mathematical,
  "before-edge" => AlignmentBaseline::BeforeEdge,
  "text-before-edge" => AlignmentBaseline::TextBeforeEdge,
  "after-edge" => AlignmentBaseline::AfterEdge,
  "text-after-edge" => AlignmentBaseline::TextAfterEdge,
  "hanging" => AlignmentBaseline::Hanging,
);

/// Blink's `EShapeRendering`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ShapeRendering {
  /// Anti-aliased.
  #[default]
  Auto,
  /// Favors speed over fidelity, which draws without anti-aliasing.
  OptimizeSpeed,
  /// Favors sharp edges, which draws without anti-aliasing.
  CrispEdges,
  /// Favors fidelity over speed.
  GeometricPrecision,
}

impl_css_enum!(
  ShapeRendering,
  "auto" => ShapeRendering::Auto,
  "optimizeSpeed" => ShapeRendering::OptimizeSpeed,
  "crispEdges" => ShapeRendering::CrispEdges,
  "geometricPrecision" => ShapeRendering::GeometricPrecision,
);

/// Blink's `EColorInterpolation`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ColorInterpolation {
  /// Interpolates in sRGB.
  #[default]
  Srgb,
  /// Interpolates in linear RGB.
  LinearRgb,
  /// Leaves the choice to the renderer.
  Auto,
}

impl_css_enum!(
  ColorInterpolation,
  "sRGB" => ColorInterpolation::Srgb,
  "linearRGB" => ColorInterpolation::LinearRgb,
  "auto" => ColorInterpolation::Auto,
);

/// Blink's `EVectorEffect`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum VectorEffect {
  /// The stroke scales with the transform.
  #[default]
  None,
  /// The stroke keeps its width in the host coordinate system.
  NonScalingStroke,
}

impl_css_enum!(
  VectorEffect,
  "none" => VectorEffect::None,
  "non-scaling-stroke" => VectorEffect::NonScalingStroke,
);

/// Blink's `EMaskType`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum MaskType {
  /// The mask is the luminance of its content.
  #[default]
  Luminance,
  /// The mask is the alpha of its content.
  Alpha,
}

impl_css_enum!(
  MaskType,
  "luminance" => MaskType::Luminance,
  "alpha" => MaskType::Alpha,
);

/// Blink's `baseline-shift`: a length, or `sub`/`super`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BaselineShift {
  /// Raises the baseline by the length, lowers it when negative.
  Length(Length),
  /// Lowers the baseline to the parent's subscript position.
  Sub,
  /// Raises the baseline to the parent's superscript position.
  Super,
}

impl Default for BaselineShift {
  fn default() -> Self {
    Self::Length(Length::zero())
  }
}

impl MakeComputed for BaselineShift {
  fn make_computed(&mut self, sizing: &SizingContext) {
    if let Self::Length(length) = self {
      length.make_computed(sizing);
    }
  }
}

impl Animatable for BaselineShift {
  fn interpolate(
    &mut self,
    from: &Self,
    to: &Self,
    progress: f32,
    sizing: &SizingContext,
    current_color: Color,
  ) {
    *self = match (from, to) {
      (Self::Length(from), Self::Length(to)) => Self::Length(Length::interpolated(
        from,
        to,
        progress,
        sizing,
        current_color,
      )),
      _ => discrete(from, to, progress),
    };
  }
}

impl<'i> FromCss<'i> for BaselineShift {
  fn from_css(input: &mut Parser<'i>) -> ParseResult<'i, Self> {
    for (keyword, shift) in [
      ("baseline", Self::default()),
      ("sub", Self::Sub),
      ("super", Self::Super),
    ] {
      if accepts_keyword(input, keyword) {
        return Ok(shift);
      }
    }

    let length = Length::from_css(input)?;

    if matches!(length, Length::Auto) {
      return Err(ValueError::invalid("baseline-shift takes no `auto`"));
    }

    Ok(Self::Length(length))
  }

  const VALID_TOKENS: &'static [&'static str] =
    &["baseline", "sub", "super", "<length-percentage>"];
}

impl ToCss for BaselineShift {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    match self {
      Self::Length(length) => length.to_css(dest),
      Self::Sub => dest.write_str("sub"),
      Self::Super => dest.write_str("super"),
    }
  }
}

/// Blink's `StylePath`: the outline `d` sets, none when it has no commands.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SvgPath(Arc<[PathCommand]>);

impl SvgPath {
  /// A path of `commands`.
  pub fn new(commands: impl Into<Arc<[PathCommand]>>) -> Self {
    Self(commands.into())
  }

  /// The absolute commands of the outline.
  pub fn commands(&self) -> &[PathCommand] {
    &self.0
  }

  /// Whether the outline has no commands, which is `d: none`.
  pub fn is_empty(&self) -> bool {
    self.0.is_empty()
  }

  /// Interpolates the commands when both paths have the same shape.
  fn lerped(&self, other: &Self, progress: f32) -> Option<Self> {
    if self.0.len() != other.0.len() {
      return None;
    }

    let lerp_point = |from: Point<f32>, to: Point<f32>| {
      Point::new(lerp(from.x, to.x, progress), lerp(from.y, to.y, progress))
    };

    self
      .0
      .iter()
      .zip(other.0.iter())
      .map(|pair| match pair {
        (PathCommand::MoveTo(from), PathCommand::MoveTo(to)) => {
          Some(PathCommand::MoveTo(lerp_point(*from, *to)))
        }
        (PathCommand::LineTo(from), PathCommand::LineTo(to)) => {
          Some(PathCommand::LineTo(lerp_point(*from, *to)))
        }
        (PathCommand::QuadTo(from_control, from), PathCommand::QuadTo(to_control, to)) => {
          Some(PathCommand::QuadTo(
            lerp_point(*from_control, *to_control),
            lerp_point(*from, *to),
          ))
        }
        (
          PathCommand::CubicTo(from_first, from_second, from),
          PathCommand::CubicTo(to_first, to_second, to),
        ) => Some(PathCommand::CubicTo(
          lerp_point(*from_first, *to_first),
          lerp_point(*from_second, *to_second),
          lerp_point(*from, *to),
        )),
        (PathCommand::Close, PathCommand::Close) => Some(PathCommand::Close),
        _ => None,
      })
      .collect::<Option<Vec<_>>>()
      .map(|commands| Self(commands.into()))
  }
}

impl MakeComputed for SvgPath {}

impl Animatable for SvgPath {
  /// Interpolates the absolute commands the path normalizes to, where Blink interpolates the
  /// authored segments, so arcs, which become cubics here, differ.
  fn interpolate(
    &mut self,
    from: &Self,
    to: &Self,
    progress: f32,
    _sizing: &SizingContext,
    _current_color: Color,
  ) {
    *self = from
      .lerped(to, progress)
      .unwrap_or_else(|| discrete(from, to, progress));
  }
}

impl<'i> FromCss<'i> for SvgPath {
  /// Blink's `ConsumePathOrNone`: `none`, or a `path()` of complete path data, where an empty
  /// string is `none`.
  fn from_css(input: &mut Parser<'i>) -> ParseResult<'i, Self> {
    if accepts_keyword(input, "none") {
      return Ok(Self::default());
    }

    let token = input.next()?.clone();

    let Token::Function(function) = &token else {
      return Err(unexpected_token!(&token));
    };

    if !function.eq_ignore_ascii_case("path") {
      return Err(unexpected_token!(&token));
    }

    input.parse_nested_block(|input| {
      let (commands, complete) = PathData::parse(input.expect_string()?);

      if !complete {
        return Err(ValueError::invalid("expected complete path data"));
      }

      Ok(Self(commands.into()))
    })
  }

  const VALID_TOKENS: &'static [&'static str] = &["none", "<path()>"];
}

impl ToCss for SvgPath {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    if self.is_empty() {
      return dest.write_str("none");
    }

    dest.write_str("path(\"")?;

    for (index, command) in self.0.iter().enumerate() {
      if index > 0 {
        dest.write_char(' ')?;
      }

      match command {
        PathCommand::MoveTo(point) => write!(dest, "M {} {}", point.x, point.y)?,
        PathCommand::LineTo(point) => write!(dest, "L {} {}", point.x, point.y)?,
        PathCommand::QuadTo(control, point) => {
          write!(
            dest,
            "Q {} {} {} {}",
            control.x, control.y, point.x, point.y
          )?;
        }
        PathCommand::CubicTo(first, second, point) => write!(
          dest,
          "C {} {} {} {} {} {}",
          first.x, first.y, second.x, second.y, point.x, point.y
        )?,
        PathCommand::Close => dest.write_char('Z')?,
      }
    }

    dest.write_str("\")")
  }
}
