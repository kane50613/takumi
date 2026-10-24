//! Fill and stroke resolution, after Blink's `SVGObjectPainter` paint server lookup.

use super::{State, paint_server, resolve_current_color, split_alpha};
use crate::{
  resvg::usvg::{
    ApproxEqUlps, Color, ContextElement, Fill, FillRule, LineCap, LineJoin, NonZeroPositiveF32,
    Opacity, Paint, Stroke, StrokeMiterlimit, Units,
  },
  style::FillRule as StyleFillRule,
  svg::{
    Dasharray, Length, LineCap as StyleLineCap, LineJoin as StyleLineJoin, Node as SvgNode,
    Paint as StylePaint, PaintFallback,
  },
};

fn fill_rule(rule: StyleFillRule) -> FillRule {
  match rule {
    StyleFillRule::NonZero => FillRule::NonZero,
    StyleFillRule::EvenOdd => FillRule::EvenOdd,
  }
}

pub(crate) fn resolve_fill(
  node: SvgNode,
  has_bbox: bool,
  state: &State,
  cache: &mut super::Cache,
) -> Option<Fill> {
  let style = node.style()?;

  if state.parent_clip_path.is_some() {
    // A `clipPath` child can be filled only with a black color.
    return Some(Fill {
      paint: Paint::Color(Color::black()),
      opacity: Opacity::ONE,
      rule: fill_rule(style.clip_rule),
      context_element: None,
    });
  }

  let mut sub_opacity = Opacity::ONE;
  let (paint, context_element) =
    convert_paint(node, &style.fill, has_bbox, state, &mut sub_opacity, cache)?;

  Some(Fill {
    paint,
    opacity: sub_opacity * Opacity::new_clamped(style.fill_opacity),
    rule: fill_rule(style.fill_rule),
    context_element,
  })
}

pub(crate) fn resolve_stroke(
  node: SvgNode,
  has_bbox: bool,
  state: &State,
  cache: &mut super::Cache,
) -> Option<Stroke> {
  let style = node.style()?;

  if state.parent_clip_path.is_some() {
    // A `clipPath` child cannot be stroked.
    return None;
  }

  let mut sub_opacity = Opacity::ONE;
  let (paint, context_element) = convert_paint(
    node,
    &style.stroke,
    has_bbox,
    state,
    &mut sub_opacity,
    cache,
  )?;

  let width = super::units::convert_other_length(style.stroke_width, node, state);
  let width = NonZeroPositiveF32::new(width)?;

  let miterlimit = StrokeMiterlimit::new(style.stroke_miterlimit.max(1.0));

  Some(Stroke {
    paint,
    dasharray: conv_dasharray(node, &style.stroke_dasharray, state),
    dashoffset: super::units::convert_other_length(style.stroke_dashoffset, node, state),
    miterlimit,
    opacity: sub_opacity * Opacity::new_clamped(style.stroke_opacity),
    width,
    linecap: match style.stroke_linecap {
      StyleLineCap::Butt => LineCap::Butt,
      StyleLineCap::Round => LineCap::Round,
      StyleLineCap::Square => LineCap::Square,
    },
    linejoin: match style.stroke_linejoin {
      StyleLineJoin::Miter => LineJoin::Miter,
      StyleLineJoin::MiterClip => LineJoin::MiterClip,
      StyleLineJoin::Round => LineJoin::Round,
      StyleLineJoin::Bevel => LineJoin::Bevel,
    },
    context_element,
  })
}

fn convert_paint(
  node: SvgNode,
  paint: &StylePaint,
  has_bbox: bool,
  state: &State,
  opacity: &mut Opacity,
  cache: &mut super::Cache,
) -> Option<(Paint, Option<ContextElement>)> {
  match paint {
    StylePaint::None => None,
    StylePaint::ContextFill => state
      .context_element
      .clone()
      .and_then(|(f, _)| f)
      .map(|f| (f.paint, f.context_element)),
    StylePaint::ContextStroke => state
      .context_element
      .clone()
      .and_then(|(_, s)| s)
      .map(|s| (s.paint, s.context_element)),
    StylePaint::CurrentColor => {
      let (color, alpha) = split_alpha(resolve_current_color(node, state));
      *opacity = alpha;
      Some((Paint::Color(color), None))
    }
    StylePaint::Color(svg_color) => {
      let (color, alpha) = split_alpha(*svg_color);
      *opacity = alpha;
      Some((Paint::Color(color), None))
    }
    StylePaint::Url(func_iri, fallback) => {
      if let Some(link) = node.document().element_by_id(func_iri) {
        let tag_name = link.tag()?;

        if tag_name.is_paint_server() {
          match paint_server::convert(link, state, cache) {
            Some(paint_server::ServerOrColor::Server(paint)) => {
              // We can use a paint server node with ObjectBoundingBox units
              // for painting only when the shape itself has a bbox.
              //
              // See SVG spec 7.11 for details.

              if !has_bbox && paint.units() == Units::ObjectBoundingBox {
                from_fallback(node, *fallback, state, opacity).map(|p| (p, None))
              } else {
                Some((paint, None))
              }
            }
            Some(paint_server::ServerOrColor::Color { color, opacity: so }) => {
              *opacity = so;
              Some((Paint::Color(color), None))
            }
            None => from_fallback(node, *fallback, state, opacity).map(|p| (p, None)),
          }
        } else {
          log::warn!("'{}' cannot be used to paint a shape.", tag_name.name());
          from_fallback(node, *fallback, state, opacity).map(|p| (p, None))
        }
      } else {
        from_fallback(node, *fallback, state, opacity).map(|p| (p, None))
      }
    }
  }
}

fn from_fallback(
  node: SvgNode,
  fallback: Option<PaintFallback>,
  state: &State,
  opacity: &mut Opacity,
) -> Option<Paint> {
  match fallback? {
    PaintFallback::None => None,
    PaintFallback::CurrentColor => {
      let (color, alpha) = split_alpha(resolve_current_color(node, state));
      *opacity = alpha;
      Some(Paint::Color(color))
    }
    PaintFallback::Color(svg_color) => {
      let (color, alpha) = split_alpha(svg_color);
      *opacity = alpha;
      Some(Paint::Color(color))
    }
  }
}

// Prepare the 'stroke-dasharray' according to:
// https://www.w3.org/TR/SVG11/painting.html#StrokeDasharrayProperty
fn conv_dasharray(node: SvgNode, dasharray: &Dasharray, state: &State) -> Option<Vec<f32>> {
  let Dasharray::List(lengths) = dasharray else {
    return None;
  };
  let list: Vec<f32> = lengths
    .iter()
    .map(|length: &Length| super::units::convert_other_length(*length, node, state))
    .collect();

  // `If the sum of the values is zero, then the stroke is rendered
  // as if a value of none were specified.`
  {
    // no Iter::sum(), because of f64

    let mut sum: f32 = 0.0;
    for n in list.iter() {
      sum += *n;
    }

    if sum.approx_eq_ulps(&0.0, 4) {
      return None;
    }
  }

  // `If an odd number of values is provided, then the list of values
  // is repeated to yield an even number of values.`
  if !list.len().is_multiple_of(2) {
    let mut tmp_list = list.clone();
    tmp_list.extend_from_slice(&list);
    return Some(tmp_list);
  }

  Some(list)
}
