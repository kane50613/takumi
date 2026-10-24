//! The fill and stroke of a shape, after Blink's `SVGLayoutSupport::ResolveSVGPaint` and
//! `ApplyStrokeStyleToStrokeData`.

use super::{
  Scope, Styled, SvgLayout,
  object::{ContextSource, Fill, Paint, Stroke, Units},
  paint_server::PaintServer,
};
use crate::{
  resources::image::{SvgLineCap, SvgLineJoin},
  style::{Color, FillRule, LineCap, LineJoin, SvgPaint},
  svg::length::LengthAxis,
};

impl SvgLayout<'_> {
  /// The fill of the shape, none when it has none to paint.
  pub(super) fn resolve_fill(
    &mut self,
    styled: &Styled,
    has_bbox: bool,
    scope: &Scope,
  ) -> Option<Fill> {
    let style = styled.style();
    let evenodd = |rule: FillRule| rule == FillRule::EvenOdd;

    if scope.parent_clip_path.is_some() {
      // A `clipPath` child can be filled only with a black color.
      return Some(Fill {
        paint: Paint::Color(Color::black()),
        opacity: 1.0,
        evenodd: evenodd(style.misc_inherited_data.clip_rule),
        context: None,
      });
    }

    let mut color_opacity = 1.0;
    let (paint, context) = self.convert_paint(
      &style.svg_fill_data.fill,
      styled,
      has_bbox,
      scope,
      &mut color_opacity,
    )?;

    Some(Fill {
      paint,
      opacity: color_opacity * style.svg_fill_data.fill_opacity.0.clamp(0.0, 1.0),
      evenodd: evenodd(style.svg_inherited_data.fill_rule),
      context,
    })
  }

  /// The stroke of the shape, none when it has none to paint.
  pub(super) fn resolve_stroke(
    &mut self,
    styled: &Styled,
    has_bbox: bool,
    scope: &Scope,
  ) -> Option<Stroke> {
    if scope.parent_clip_path.is_some() {
      // A `clipPath` child cannot be stroked.
      return None;
    }

    let style = styled.style();
    let data = &style.svg_stroke_data;
    let mut color_opacity = 1.0;
    let (paint, context) =
      self.convert_paint(&data.stroke, styled, has_bbox, scope, &mut color_opacity)?;
    let width = self.px(styled, scope, data.stroke_width, LengthAxis::Other);

    if !(width > 0.0 && width.is_finite()) {
      return None;
    }

    Some(Stroke {
      paint,
      opacity: color_opacity * data.stroke_opacity.0.clamp(0.0, 1.0),
      width,
      miter_limit: data.stroke_miterlimit.get(),
      cap: match data.stroke_linecap {
        LineCap::Butt => SvgLineCap::Butt,
        LineCap::Round => SvgLineCap::Round,
        LineCap::Square => SvgLineCap::Square,
      },
      join: match style.misc_inherited_data.stroke_linejoin {
        LineJoin::Miter => SvgLineJoin::Miter,
        LineJoin::Round => SvgLineJoin::Round,
        LineJoin::Bevel => SvgLineJoin::Bevel,
      },
      dash: self.dash_pattern(styled, scope).map(|dashes| {
        (
          dashes,
          self.px(styled, scope, data.stroke_dashoffset, LengthAxis::Other),
        )
      }),
      context,
    })
  }

  /// The paint a `fill` or `stroke` value names, with the opacity its color carries.
  fn convert_paint(
    &mut self,
    paint: &SvgPaint,
    styled: &Styled,
    has_bbox: bool,
    scope: &Scope,
    opacity: &mut f32,
  ) -> Option<(Paint, Option<ContextSource>)> {
    let color_paint = |color: Color, opacity: &mut f32| {
      *opacity = f32::from(color.0[3]) / 255.0;

      Some((
        Paint::Color(Color([color.0[0], color.0[1], color.0[2], 255])),
        None,
      ))
    };
    let current_color = styled.context.current_color;

    match paint {
      SvgPaint::None => None,
      SvgPaint::ContextFill => scope
        .context_paints
        .clone()
        .and_then(|(fill, _)| fill)
        .map(|fill| (fill.paint, fill.context)),
      SvgPaint::ContextStroke => scope
        .context_paints
        .clone()
        .and_then(|(_, stroke)| stroke)
        .map(|stroke| (stroke.paint, stroke.context)),
      SvgPaint::Color(color) => color_paint(color.resolve(current_color), opacity),
      SvgPaint::Url(resource, fallback) => {
        let from_fallback = |opacity: &mut f32| {
          fallback.and_then(|color| color_paint(color.resolve(current_color), opacity))
        };
        let Some(link) = self.reference(resource) else {
          return from_fallback(opacity);
        };

        if !self.document.elements[link].kind.is_paint_server() {
          return from_fallback(opacity);
        }

        match self.paint_server(link, scope) {
          Some(PaintServer::Server(paint)) => {
            // A server in bounding box units paints only a shape that has a bounding box.
            // https://www.w3.org/TR/SVG11/painting.html#SpecifyingPaint
            if !has_bbox && paint.units() == Units::ObjectBoundingBox {
              from_fallback(opacity)
            } else {
              Some((paint, None))
            }
          }
          Some(PaintServer::Color {
            color,
            opacity: server_opacity,
          }) => {
            *opacity = server_opacity;
            Some((Paint::Color(color), None))
          }
          None => from_fallback(opacity),
        }
      }
    }
  }

  /// `stroke-dasharray` as an even list of user units, none where it paints a solid line.
  /// https://www.w3.org/TR/SVG11/painting.html#StrokeDasharrayProperty
  fn dash_pattern(&self, styled: &Styled, scope: &Scope) -> Option<Vec<f32>> {
    let lengths = styled.style().svg_stroke_data.stroke_dasharray.as_ref()?;
    let list: Vec<f32> = lengths
      .lengths()
      .iter()
      .map(|length| self.px(styled, scope, *length, LengthAxis::Other))
      .collect();

    // A negative value is an error.
    if list.iter().any(|dash| dash.is_sign_negative()) {
      return None;
    }

    // A sum of zero renders as if there were none.
    let mut sum: f32 = 0.0;

    for dash in &list {
      sum += *dash;
    }

    if sum.to_bits() <= 4 {
      return None;
    }

    // An odd list repeats to an even one.
    if !list.len().is_multiple_of(2) {
      let mut doubled = list.clone();

      doubled.extend_from_slice(&list);
      return Some(doubled);
    }

    Some(list)
  }
}
