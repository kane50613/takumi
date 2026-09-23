//! Draws flattened SVG vector ops onto a krilla surface, so SVG image sources
//! embed as real paths and gradients instead of bitmaps.

use takumi_core::resources::image::{
  SvgFill, SvgGradient, SvgLineCap, SvgLineJoin, SvgOp, SvgPaint, SvgSpreadMethod, SvgStrokeStyle,
};

use crate::{
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
  paint::{draw_stream, krilla_blend, krilla_path, krilla_transform, normalized},
};

/// Draws `ops` onto `surface` in the current coordinate space and resets the
/// fill/stroke state afterwards.
pub(crate) fn draw_svg_ops(surface: &mut Surface, ops: Vec<SvgOp>) {
  draw_ops(surface, ops);
  surface.set_fill(None);
  surface.set_stroke(None);
}

fn draw_ops(surface: &mut Surface, ops: Vec<SvgOp>) {
  for op in ops {
    match op {
      SvgOp::PushTransform(transform) => surface.push_transform(&krilla_transform(transform)),
      SvgOp::PushClip { path, evenodd } => match krilla_path(&path, 0.0, 0.0) {
        Some(path) => surface.push_clip_path(&path, &fill_rule(evenodd)),
        // The matching `Pop` still comes; keep the layer stack balanced.
        None => surface.push_transform(&Transform::identity()),
      },
      SvgOp::PushBlend(blend) => surface.push_blend_mode(krilla_blend(blend)),
      SvgOp::PushOpacity(opacity) => surface.push_opacity(normalized(opacity)),
      SvgOp::PushMask { ops, luminance } => {
        let stream = draw_stream(surface, |mask| draw_ops(mask, ops));
        let kind = if luminance {
          MaskType::Luminosity
        } else {
          MaskType::Alpha
        };

        surface.push_mask(Mask::new(stream, kind));
      }
      SvgOp::Pop => surface.pop(),
      SvgOp::Draw { path, fill, stroke } => {
        let Some(path) = krilla_path(&path, 0.0, 0.0) else {
          continue;
        };
        let fill = fill.map(|fill| svg_fill(fill, surface));
        let stroke = stroke.map(|stroke| svg_stroke(stroke, surface));

        surface.set_fill(fill);
        surface.set_stroke(stroke);
        surface.draw_path(&path);
      }
      SvgOp::Raster {
        rgba,
        width,
        height,
        rect: (x, y, dest_width, dest_height),
      } => {
        let Some(size) = KrillaSize::from_wh(dest_width, dest_height) else {
          continue;
        };

        surface.push_transform(&Transform::from_translate(x, y));
        surface.draw_image(KrillaImage::from_rgba8(rgba, width, height), size);
        surface.pop();
      }
    }
  }
}

fn svg_fill(fill: SvgFill, surface: &mut Surface) -> Fill {
  Fill {
    paint: svg_paint(fill.paint, surface),
    opacity: normalized(fill.opacity),
    rule: fill_rule(fill.evenodd),
  }
}

fn svg_stroke(stroke: SvgStrokeStyle, surface: &mut Surface) -> Stroke {
  Stroke {
    paint: svg_paint(stroke.paint, surface),
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
    opacity: normalized(stroke.opacity),
    dash: stroke
      .dash
      .map(|(array, offset)| StrokeDash { array, offset }),
  }
}

fn svg_paint(paint: SvgPaint, surface: &mut Surface) -> Paint {
  match paint {
    SvgPaint::Color([red, green, blue]) => rgb::Color::new(red, green, blue).into(),
    SvgPaint::Linear {
      start,
      end,
      gradient,
    } => LinearGradient {
      x1: start.x,
      y1: start.y,
      x2: end.x,
      y2: end.y,
      transform: krilla_transform(gradient.transform),
      spread_method: spread_method(gradient.spread),
      stops: gradient_stops(&gradient),
      anti_alias: false,
    }
    .into(),
    SvgPaint::Radial {
      center,
      radius,
      focal,
      gradient,
    } => RadialGradient {
      cx: center.x,
      cy: center.y,
      cr: radius,
      fx: focal.x,
      fy: focal.y,
      fr: 0.0,
      transform: krilla_transform(gradient.transform),
      spread_method: spread_method(gradient.spread),
      stops: gradient_stops(&gradient),
      anti_alias: false,
    }
    .into(),
    SvgPaint::Pattern {
      ops,
      transform,
      width,
      height,
    } => Pattern {
      stream: draw_stream(surface, |tile| draw_ops(tile, ops)),
      transform: krilla_transform(transform),
      width,
      height,
    }
    .into(),
  }
}

const fn spread_method(spread: SvgSpreadMethod) -> SpreadMethod {
  match spread {
    SvgSpreadMethod::Pad => SpreadMethod::Pad,
    SvgSpreadMethod::Reflect => SpreadMethod::Reflect,
    SvgSpreadMethod::Repeat => SpreadMethod::Repeat,
  }
}

fn gradient_stops(gradient: &SvgGradient) -> Vec<Stop> {
  gradient
    .stops
    .iter()
    .map(|stop| Stop {
      offset: normalized(stop.offset),
      color: rgb::Color::new(stop.color[0], stop.color[1], stop.color[2]).into(),
      opacity: normalized(stop.opacity),
    })
    .collect()
}

const fn fill_rule(evenodd: bool) -> FillRule {
  if evenodd {
    FillRule::EvenOdd
  } else {
    FillRule::NonZero
  }
}
