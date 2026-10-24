//! Draws flattened SVG ops onto a krilla surface, leaving out a group's SVG filters.

use takumi_core::{
  geometry::Point,
  resources::image::{
    SvgFill, SvgGradient, SvgImageData, SvgLineCap, SvgLineJoin, SvgOp, SvgPaint, SvgSpreadMethod,
    SvgStrokeStyle,
  },
  style::Color,
};

use crate::{
  filter::{ColorFilter, filtered},
  krilla::{
    color::rgb,
    geom::{Size as KrillaSize, Transform},
    image::Image as KrillaImage,
    mask::{Mask, MaskType},
    paint::{
      Fill, FillRule, LineCap, LineJoin, LinearGradient, Paint, Pattern, RadialGradient,
      SpreadMethod, Stop, Stroke, StrokeDash,
    },
    surface::Surface,
  },
  paint::{bitmap_image, draw_stream, krilla_blend, krilla_path, krilla_transform, normalized},
  raster::embedded_image,
};

/// Draws SVG ops in the colors a CSS `filter` leaves them.
pub(crate) struct SvgPainter<'a> {
  filter: Option<&'a ColorFilter>,
}

impl<'a> SvgPainter<'a> {
  pub(crate) const fn new(filter: Option<&'a ColorFilter>) -> Self {
    Self { filter }
  }

  /// Draws `ops` onto `surface`, then resets its fill and stroke.
  pub(crate) fn draw(&self, surface: &mut Surface, ops: Vec<SvgOp>) {
    self.draw_ops(surface, ops);
    surface.set_fill(None);
    surface.set_stroke(None);
  }

  fn draw_ops(&self, surface: &mut Surface, ops: Vec<SvgOp>) {
    for op in ops {
      match op {
        SvgOp::PushTransform(transform) => surface.push_transform(&krilla_transform(transform)),
        SvgOp::PushClip { path, evenodd } => match krilla_path(&path, Point::ZERO) {
          Some(path) => surface.push_clip_path(&path, &fill_rule(evenodd)),
          // The matching `Pop` still comes; keep the layer stack balanced.
          None => surface.push_transform(&Transform::identity()),
        },
        SvgOp::PushBlend(blend) => surface.push_blend_mode(krilla_blend(blend)),
        SvgOp::PushOpacity(opacity) => surface.push_opacity(normalized(opacity)),
        SvgOp::PushMask { ops, luminance } => {
          // CSS filters the element after masking, so the mask's own colors stay as drawn.
          let stream = draw_stream(surface, |mask| SvgPainter::new(None).draw_ops(mask, ops));
          let kind = if luminance {
            MaskType::Luminosity
          } else {
            MaskType::Alpha
          };

          surface.push_mask(Mask::new(stream, kind));
        }
        SvgOp::PushFilter(_) => surface.push_transform(&Transform::identity()),
        SvgOp::Pop => surface.pop(),
        SvgOp::Draw { path, fill, stroke } => {
          let Some(path) = krilla_path(&path, Point::ZERO) else {
            continue;
          };
          let fill = fill.map(|fill| self.fill(fill, surface));
          let stroke = stroke.map(|stroke| self.stroke(stroke, surface));

          surface.set_fill(fill);
          surface.set_stroke(stroke);
          surface.draw_path(&path);
        }
        SvgOp::Image {
          data,
          width,
          height,
          ..
        } => {
          let Some(size) = KrillaSize::from_wh(width, height) else {
            continue;
          };
          let Some(image) = self.image(&data) else {
            continue;
          };

          surface.draw_image(image, size);
        }
      }
    }
  }

  /// An encoded bitmap embeds as it came, unless the filter recolors it or it does not embed.
  fn image(&self, data: &SvgImageData) -> Option<KrillaImage> {
    if let (SvgImageData::Encoded(bytes), None) = (data, self.filter)
      && let Some(image) = embedded_image(bytes)
    {
      return Some(image);
    }

    Some(bitmap_image(&data.decode()?, self.filter))
  }

  fn fill(&self, fill: SvgFill, surface: &mut Surface) -> Fill {
    let (paint, opacity) = self.paint(fill.paint, fill.opacity, surface);

    Fill {
      paint,
      opacity: normalized(opacity),
      rule: fill_rule(fill.evenodd),
    }
  }

  fn stroke(&self, stroke: SvgStrokeStyle, surface: &mut Surface) -> Stroke {
    let (paint, opacity) = self.paint(stroke.paint, stroke.opacity, surface);

    Stroke {
      paint,
      width: stroke.width,
      miter_limit: stroke.miter_limit,
      line_cap: match stroke.cap {
        SvgLineCap::Butt => LineCap::Butt,
        SvgLineCap::Round => LineCap::Round,
        SvgLineCap::Square => LineCap::Square,
      },
      line_join: match stroke.join {
        SvgLineJoin::Miter => LineJoin::Miter,
        SvgLineJoin::Round => LineJoin::Round,
        SvgLineJoin::Bevel => LineJoin::Bevel,
      },
      opacity: normalized(opacity),
      dash: stroke
        .dash
        .map(|(array, offset)| StrokeDash { array, offset }),
    }
  }

  /// `color` and `opacity` as the filter leaves them.
  fn filtered_color(&self, [red, green, blue]: [u8; 3], opacity: f32) -> (rgb::Color, f32) {
    let [red, green, blue, alpha] = filtered(self.filter, Color([red, green, blue, 255]));

    (
      rgb::Color::new(red, green, blue),
      opacity * f32::from(alpha) / 255.0,
    )
  }

  /// `paint` for a fill or stroke of `opacity`, with the opacity the filter leaves it.
  fn paint(&self, paint: SvgPaint, opacity: f32, surface: &mut Surface) -> (Paint, f32) {
    match paint {
      SvgPaint::Color(color) => {
        let (color, opacity) = self.filtered_color(color, opacity);

        (color.into(), opacity)
      }
      SvgPaint::Linear {
        start,
        end,
        gradient,
      } => (
        LinearGradient {
          x1: start.x,
          y1: start.y,
          x2: end.x,
          y2: end.y,
          transform: krilla_transform(gradient.transform),
          spread_method: spread_method(gradient.spread),
          stops: self.gradient_stops(&gradient),
          anti_alias: false,
        }
        .into(),
        opacity,
      ),
      SvgPaint::Radial {
        center,
        radius,
        focal,
        gradient,
      } => (
        RadialGradient {
          cx: center.x,
          cy: center.y,
          cr: radius,
          fx: focal.x,
          fy: focal.y,
          fr: 0.0,
          transform: krilla_transform(gradient.transform),
          spread_method: spread_method(gradient.spread),
          stops: self.gradient_stops(&gradient),
          anti_alias: false,
        }
        .into(),
        opacity,
      ),
      SvgPaint::Pattern {
        ops,
        transform,
        width,
        height,
      } => (
        Pattern {
          stream: draw_stream(surface, |tile| self.draw_ops(tile, ops)),
          transform: krilla_transform(transform),
          width,
          height,
        }
        .into(),
        opacity,
      ),
    }
  }

  fn gradient_stops(&self, gradient: &SvgGradient) -> Vec<Stop> {
    gradient
      .stops
      .iter()
      .map(|stop| {
        let (color, opacity) = self.filtered_color(stop.color, stop.opacity);

        Stop {
          offset: normalized(stop.offset),
          color: color.into(),
          opacity: normalized(opacity),
        }
      })
      .collect()
  }
}

const fn spread_method(spread: SvgSpreadMethod) -> SpreadMethod {
  match spread {
    SvgSpreadMethod::Pad => SpreadMethod::Pad,
    SvgSpreadMethod::Reflect => SpreadMethod::Reflect,
    SvgSpreadMethod::Repeat => SpreadMethod::Repeat,
  }
}

const fn fill_rule(evenodd: bool) -> FillRule {
  if evenodd {
    FillRule::EvenOdd
  } else {
    FillRule::NonZero
  }
}
