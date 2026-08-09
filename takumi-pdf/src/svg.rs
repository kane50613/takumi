//! Draws flattened SVG vector ops onto a krilla surface, so SVG image sources
//! embed as real paths and gradients instead of bitmaps.

use takumi_core::{
  geometry::PathCommand,
  resources::image::{
    SvgFill, SvgGradient, SvgLineCap, SvgLineJoin, SvgOp, SvgPaint, SvgSpreadMethod, SvgStrokeStyle,
  },
};

use crate::{
  krilla::{
    color::rgb,
    geom::{Path as KrillaPath, Size as KrillaSize, Transform},
    image::Image as KrillaImage,
    mask::{Mask, MaskType},
    num::NormalizedF32,
    paint::{
      Fill, FillRule, LineCap, LineJoin, LinearGradient, Paint, Pattern, RadialGradient,
      SpreadMethod, Stop, Stroke, StrokeDash,
    },
    surface::Surface,
  },
  paint::{krilla_blend, krilla_path},
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
      SvgOp::PushTransform([a, b, c, d, e, f]) => {
        surface.push_transform(&Transform::from_row(a, b, c, d, e, f));
      }
      SvgOp::PushClip { path, evenodd } => match svg_path(&path) {
        Some(path) => surface.push_clip_path(&path, &fill_rule(evenodd)),
        // The matching `Pop` still comes; keep the layer stack balanced.
        None => surface.push_transform(&Transform::identity()),
      },
      SvgOp::PushBlend(blend) => surface.push_blend_mode(krilla_blend(blend)),
      SvgOp::PushOpacity(opacity) => {
        surface.push_opacity(normalized(opacity));
      }
      SvgOp::PushMask { ops, luminance } => {
        let mut stream_builder = surface.stream_builder();
        let mut sub_surface = stream_builder.surface();

        draw_ops(&mut sub_surface, ops);
        sub_surface.finish();
        let stream = stream_builder.finish();
        let kind = if luminance {
          MaskType::Luminosity
        } else {
          MaskType::Alpha
        };

        surface.push_mask(Mask::new(stream, kind));
      }
      SvgOp::Pop => surface.pop(),
      SvgOp::Draw { path, fill, stroke } => {
        let Some(path) = svg_path(&path) else {
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

fn svg_path(commands: &[PathCommand]) -> Option<KrillaPath> {
  krilla_path(commands, 0.0, 0.0)
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
      transform: gradient_transform(&gradient),
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
      transform: gradient_transform(&gradient),
      spread_method: spread_method(gradient.spread),
      stops: gradient_stops(&gradient),
      anti_alias: false,
    }
    .into(),
    SvgPaint::Pattern {
      ops,
      transform: [a, b, c, d, e, f],
      width,
      height,
    } => {
      let mut stream_builder = surface.stream_builder();
      let mut tile = stream_builder.surface();

      draw_ops(&mut tile, ops);
      tile.finish();

      Pattern {
        stream: stream_builder.finish(),
        transform: Transform::from_row(a, b, c, d, e, f),
        width,
        height,
      }
      .into()
    }
  }
}

fn gradient_transform(gradient: &SvgGradient) -> Transform {
  let [a, b, c, d, e, f] = gradient.transform;

  Transform::from_row(a, b, c, d, e, f)
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

fn normalized(value: f32) -> NormalizedF32 {
  NormalizedF32::new(value.clamp(0.0, 1.0)).unwrap_or(NormalizedF32::ONE)
}
