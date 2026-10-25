//! Draws flattened SVG ops onto a tiny-skia pixmap. A layer covers the whole pixmap, except a
//! filter layer, which covers its filter region.

use std::slice::Iter;

use tiny_skia::{
  BlendMode as TinyBlendMode, Color as TinyColor, FillRule, FilterQuality, GradientStop, IntRect,
  LineCap, LineJoin, LinearGradient, Mask, MaskType, Paint, Path, PathBuilder, Pattern, Pixmap,
  PixmapMut, PixmapPaint, PixmapRef, RadialGradient, Rect, SpreadMode, Stroke, StrokeDash,
  Transform,
};

use crate::{
  geometry::PathCommand,
  resources::{image::within_raster_pixel_budget, image_buffer::composite_premultiplied_over_row},
  style::{BlendMode, Color, ImageScalingAlgorithm, math::mul_div_255},
  svg_vector::{
    SvgFill, SvgFilters, SvgGradient, SvgImageData, SvgLineCap, SvgLineJoin, SvgOp, SvgOpPaint,
    SvgSpreadMethod, SvgStrokeStyle,
  },
};

/// Draws SVG ops with tiny-skia.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct SvgDrawing;

/// A pixmap being drawn on, with the transform and clip that apply to what is drawn.
struct Target<'a, 'b> {
  pixmap: &'a mut PixmapMut<'b>,
  transform: Transform,
  clip: Option<Mask>,
  /// The root pixmap grown by twice its size on each side, in this pixmap's pixels, as resvg's `max_bbox`.
  limit: IntRect,
}

impl<'a, 'b> Target<'a, 'b> {
  fn root(pixmap: &'a mut PixmapMut<'b>, transform: Transform) -> Option<Self> {
    let (width, height) = (pixmap.width(), pixmap.height());
    let limit = IntRect::from_xywh(
      i32::try_from(width).ok()?.checked_mul(-2)?,
      i32::try_from(height).ok()?.checked_mul(-2)?,
      width.checked_mul(5)?,
      height.checked_mul(5)?,
    )?;

    Some(Self {
      pixmap,
      transform,
      clip: None,
      limit,
    })
  }

  /// Where a layer sits, a filter's region within `limit` and the raster budget.
  fn layer_bounds(&self, filters: Option<&SvgFilters>) -> Option<IntRect> {
    let (width, height) = (self.pixmap.width(), self.pixmap.height());

    let Some(filters) = filters else {
      return IntRect::from_xywh(0, 0, width, height);
    };
    let region = filters.region()?.transform(self.transform)?;
    let region = IntRect::from_xywh(
      region.x().floor() as i32,
      region.y().floor() as i32,
      region.width().ceil().max(1.0) as u32,
      region.height().ceil().max(1.0) as u32,
    )?
    .intersect(&self.limit)?;

    if within_raster_pixel_budget(region.width(), region.height()) {
      Some(region)
    } else {
      IntRect::from_xywh(0, 0, width, height)
    }
  }
}

/// How a layer joins the target below it.
#[derive(Default)]
struct Composite<'a> {
  opacity: Option<f32>,
  blend: Option<TinyBlendMode>,
  mask: Option<Mask>,
  filters: Option<&'a SvgFilters>,
}

impl SvgDrawing {
  /// Draws `ops` onto `pixmap` with `transform` as the canvas transform.
  pub(crate) fn draw(&self, ops: &[SvgOp], transform: Transform, pixmap: &mut PixmapMut) {
    if let Some(mut target) = Target::root(pixmap, transform) {
      self.run(&mut ops.iter(), &mut target);
    }
  }

  /// Draws ops up to the `Pop` closing the scope they are in, or the end of `ops`.
  fn run(&self, ops: &mut Iter<'_, SvgOp>, target: &mut Target) {
    while let Some(op) = ops.next() {
      match op {
        SvgOp::Pop => return,
        SvgOp::PushTransform(transform) => {
          let outer = target.transform;

          target.transform = outer.pre_concat(Transform::from(*transform));
          self.run(ops, target);
          target.transform = outer;
        }
        SvgOp::PushClip { path, evenodd } => {
          let outer = target.clip.take();

          target.clip = clipped(outer.as_ref(), path, *evenodd, target);
          self.run(ops, target);
          target.clip = outer;
        }
        SvgOp::PushOpacity(opacity) => self.layer(
          ops,
          target,
          Composite {
            opacity: Some(*opacity),
            ..Composite::default()
          },
        ),
        SvgOp::PushBlend(blend) => self.layer(
          ops,
          target,
          Composite {
            blend: Some(tiny_blend_mode(*blend)),
            ..Composite::default()
          },
        ),
        SvgOp::PushIsolation => self.layer(ops, target, Composite::default()),
        SvgOp::PushMask {
          ops: mask_ops,
          luminance,
        } => match self.mask(mask_ops, *luminance, target) {
          Some(mask) => self.layer(
            ops,
            target,
            Composite {
              mask: Some(mask),
              ..Composite::default()
            },
          ),
          None => skip_scope(ops),
        },
        SvgOp::PushFilter(filters) => self.layer(
          ops,
          target,
          Composite {
            filters: Some(filters),
            ..Composite::default()
          },
        ),
        SvgOp::Draw {
          path,
          fill,
          stroke,
          anti_alias,
        } => self.draw_path(target, path, fill.as_ref(), stroke.as_ref(), *anti_alias),
        SvgOp::Image {
          data,
          width,
          height,
          rendering,
        } => self.draw_image(target, data, *width, *height, *rendering),
      }
    }
  }

  /// Draws the scope that follows into a fresh layer and composites the layer onto `target`.
  fn layer(&self, ops: &mut Iter<'_, SvgOp>, target: &mut Target, composite: Composite) {
    let Some(bounds) = target.layer_bounds(composite.filters) else {
      skip_scope(ops);
      return;
    };
    let Some(mut layer) = Pixmap::new(bounds.width(), bounds.height()) else {
      skip_scope(ops);
      return;
    };
    let transform = Transform::from_translate(-bounds.x() as f32, -bounds.y() as f32)
      .pre_concat(target.transform);
    let Some(limit) = target.limit.translate(-bounds.x(), -bounds.y()) else {
      skip_scope(ops);
      return;
    };
    let mut view = layer.as_mut();

    self.run(
      ops,
      &mut Target {
        pixmap: &mut view,
        transform,
        clip: None,
        limit,
      },
    );

    if let Some(filters) = composite.filters {
      filters.apply(transform, &mut layer);
    }
    if let Some(mask) = &composite.mask {
      layer.apply_mask(mask);
    }

    let opacity = composite.opacity.unwrap_or(1.0);
    let blend_mode = composite.blend.unwrap_or(TinyBlendMode::SourceOver);

    if blend_mode == TinyBlendMode::SourceOver {
      draw_over(
        target.pixmap,
        layer.as_ref(),
        (bounds.x(), bounds.y()),
        opacity,
        target.clip.as_ref(),
      );
      return;
    }

    target.pixmap.draw_pixmap(
      bounds.x(),
      bounds.y(),
      layer.as_ref(),
      &PixmapPaint {
        opacity,
        blend_mode,
        quality: FilterQuality::Nearest,
      },
      Transform::identity(),
      target.clip.as_ref(),
    );
  }

  /// The mask `ops` draw, which cuts the layer it is pushed with.
  fn mask(&self, ops: &[SvgOp], luminance: bool, target: &Target) -> Option<Mask> {
    let mut drawn = Pixmap::new(target.pixmap.width(), target.pixmap.height())?;

    self.draw(ops, target.transform, &mut drawn.as_mut());

    Some(Mask::from_pixmap(
      drawn.as_ref(),
      if luminance {
        MaskType::Luminance
      } else {
        MaskType::Alpha
      },
    ))
  }

  fn draw_path(
    &self,
    target: &mut Target,
    commands: &[PathCommand],
    fill: Option<&SvgFill>,
    stroke: Option<&SvgStrokeStyle>,
    anti_alias: bool,
  ) {
    let Some(path) = tiny_path(commands) else {
      return;
    };

    if let Some(fill) = fill {
      // Horizontal and vertical lines have no area to fill.
      if path.bounds().width() != 0.0 && path.bounds().height() != 0.0 {
        self.with_paint(
          &fill.paint,
          fill.opacity,
          target.transform,
          anti_alias,
          |paint| {
            target.pixmap.fill_path(
              &path,
              paint,
              if fill.evenodd {
                FillRule::EvenOdd
              } else {
                FillRule::Winding
              },
              target.transform,
              target.clip.as_ref(),
            );
          },
        );
      }
    }
    if let Some(stroke) = stroke {
      self.with_paint(
        &stroke.paint,
        stroke.opacity,
        target.transform,
        anti_alias,
        |paint| {
          target.pixmap.stroke_path(
            &path,
            paint,
            &Stroke::from(stroke),
            target.transform,
            target.clip.as_ref(),
          );
        },
      );
    }
  }

  fn draw_image(
    &self,
    target: &mut Target,
    data: &SvgImageData,
    width: f32,
    height: f32,
    rendering: ImageScalingAlgorithm,
  ) {
    let Some(buffer) = data.decode() else {
      return;
    };
    let Some(pixmap) = PixmapRef::from_bytes(buffer.data(), buffer.width(), buffer.height()) else {
      return;
    };
    let Some(rect) = Rect::from_xywh(0.0, 0.0, width, height) else {
      return;
    };
    let scale = Transform::from_scale(
      width / pixmap.width() as f32,
      height / pixmap.height() as f32,
    );
    let paint = Paint {
      shader: Pattern::new(
        pixmap,
        SpreadMode::Pad,
        if rendering == ImageScalingAlgorithm::Pixelated {
          FilterQuality::Nearest
        } else {
          FilterQuality::Bicubic
        },
        1.0,
        scale,
      ),
      ..Paint::default()
    };

    target
      .pixmap
      .fill_rect(rect, &paint, target.transform, target.clip.as_ref());
  }

  /// Hands `draw` the tiny-skia paint for `paint`, a pattern tile drawn at `transform`'s scale.
  fn with_paint(
    &self,
    paint: &SvgOpPaint,
    opacity: f32,
    transform: Transform,
    anti_alias: bool,
    draw: impl FnOnce(&Paint),
  ) {
    let tile;
    let mut tiny = Paint {
      anti_alias,
      ..Paint::default()
    };

    match paint {
      SvgOpPaint::Color(Color([red, green, blue, _])) => {
        tiny.set_color_rgba8(*red, *green, *blue, to_u8(opacity));
      }
      SvgOpPaint::Linear {
        start,
        end,
        gradient,
      } => {
        let Some(shader) = LinearGradient::new(
          (start.x, start.y).into(),
          (end.x, end.y).into(),
          gradient_stops(gradient, opacity),
          gradient.spread.into(),
          Transform::from(gradient.transform),
        ) else {
          return;
        };

        tiny.shader = shader;
      }
      SvgOpPaint::Radial {
        center,
        radius,
        focal,
        focal_radius,
        gradient,
      } => {
        let Some(shader) = RadialGradient::new(
          (focal.x, focal.y).into(),
          *focal_radius,
          (center.x, center.y).into(),
          *radius,
          gradient_stops(gradient, opacity),
          gradient.spread.into(),
          Transform::from(gradient.transform),
        ) else {
          return;
        };

        tiny.shader = shader;
      }
      SvgOpPaint::Pattern {
        ops,
        transform: pattern_transform,
        width,
        height,
      } => {
        let pattern_transform = Transform::from(*pattern_transform);
        let (scale_x, scale_y) = transform.pre_concat(pattern_transform).get_scale();
        let tile_width = (width * scale_x).round() as u32;
        let tile_height = (height * scale_y).round() as u32;

        if !within_raster_pixel_budget(tile_width, tile_height) {
          return;
        }

        let Some(mut pixmap) = Pixmap::new(tile_width, tile_height) else {
          return;
        };

        self.draw(
          ops,
          Transform::from_scale(scale_x, scale_y),
          &mut pixmap.as_mut(),
        );
        tile = pixmap;
        tiny.shader = Pattern::new(
          tile.as_ref(),
          SpreadMode::Repeat,
          FilterQuality::Bicubic,
          opacity,
          pattern_transform.pre_scale(1.0 / scale_x, 1.0 / scale_y),
        );
      }
    }
    tiny.blend_mode = TinyBlendMode::SourceOver;
    draw(&tiny);
  }
}

/// Draws `source` over `target` with its top left at `origin`, at `opacity` and inside `clip` when
/// there is one, as tiny-skia's source-over `draw_pixmap` does, in integers.
pub(crate) fn draw_over(
  target: &mut PixmapMut,
  source: PixmapRef,
  (x, y): (i32, i32),
  opacity: f32,
  clip: Option<&Mask>,
) {
  let (target_width, target_height) = (target.width() as i32, target.height() as i32);
  let source_width = source.width() as i32;
  let left = x.max(0);
  let right = (x + source_width).min(target_width);
  let top = y.max(0);
  let bottom = (y + source.height() as i32).min(target_height);
  let alpha = (opacity.clamp(0.0, 1.0) * 255.0).round() as u8;

  if left >= right || top >= bottom || alpha == 0 {
    return;
  }

  let span = (right - left) as usize;
  let target_pixels = target.data_mut().as_chunks_mut::<4>().0;
  let source_pixels = source.data().as_chunks::<4>().0;
  let mut covered = Vec::new();

  for row in top..bottom {
    let target_start = (row * target_width + left) as usize;
    let source_row = &source_pixels[((row - y) * source_width + left - x) as usize..][..span];
    let source_row = match (clip, alpha) {
      (None, u8::MAX) => source_row,
      (None, alpha) => {
        covered.clear();
        covered.extend(
          source_row
            .iter()
            .map(|pixel| pixel.map(|channel| mul_div_255(channel, alpha))),
        );
        &covered
      }
      (Some(clip), alpha) => {
        let coverage = &clip.data()[target_start..][..span];

        covered.clear();
        covered.extend(source_row.iter().zip(coverage).map(|(pixel, &coverage)| {
          let coverage = mul_div_255(coverage, alpha);

          pixel.map(|channel| mul_div_255(channel, coverage))
        }));
        &covered
      }
    };

    composite_premultiplied_over_row(&mut target_pixels[target_start..][..span], source_row);
  }
}

/// Skips ops up to the `Pop` closing the scope they are in.
fn skip_scope(ops: &mut Iter<'_, SvgOp>) {
  let mut depth = 0_usize;

  for op in ops {
    match op {
      SvgOp::Pop if depth == 0 => return,
      SvgOp::Pop => depth -= 1,
      SvgOp::PushTransform(_)
      | SvgOp::PushClip { .. }
      | SvgOp::PushOpacity(_)
      | SvgOp::PushBlend(_)
      | SvgOp::PushIsolation
      | SvgOp::PushMask { .. }
      | SvgOp::PushFilter(_) => depth += 1,
      SvgOp::Draw { .. } | SvgOp::Image { .. } => {}
    }
  }
}

/// `outer` narrowed to the inside of `commands`. A path with no area leaves nothing visible.
fn clipped(
  outer: Option<&Mask>,
  commands: &[PathCommand],
  evenodd: bool,
  target: &Target,
) -> Option<Mask> {
  let rule = if evenodd {
    FillRule::EvenOdd
  } else {
    FillRule::Winding
  };
  let path = tiny_path(commands);

  match (outer, path) {
    (Some(outer), Some(path)) => {
      let mut mask = outer.clone();

      mask.intersect_path(&path, rule, true, target.transform);
      Some(mask)
    }
    (None, Some(path)) => {
      let mut mask = Mask::new(target.pixmap.width(), target.pixmap.height())?;

      mask.fill_path(&path, rule, true, target.transform);
      Some(mask)
    }
    (_, None) => Mask::new(target.pixmap.width(), target.pixmap.height()),
  }
}

fn tiny_path(commands: &[PathCommand]) -> Option<Path> {
  let mut builder = PathBuilder::new();

  for command in commands {
    match command {
      PathCommand::MoveTo(p) => builder.move_to(p.x, p.y),
      PathCommand::LineTo(p) => builder.line_to(p.x, p.y),
      PathCommand::QuadTo(c, p) => builder.quad_to(c.x, c.y, p.x, p.y),
      PathCommand::CubicTo(c1, c2, p) => builder.cubic_to(c1.x, c1.y, c2.x, c2.y, p.x, p.y),
      PathCommand::Close => builder.close(),
    }
  }

  builder.finish()
}

impl From<&SvgStrokeStyle> for Stroke {
  fn from(stroke: &SvgStrokeStyle) -> Self {
    Self {
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
      dash: stroke
        .dash
        .as_ref()
        .and_then(|(array, offset)| StrokeDash::new(array.clone(), *offset)),
    }
  }
}

fn gradient_stops(gradient: &SvgGradient, opacity: f32) -> Vec<GradientStop> {
  gradient
    .stops
    .iter()
    .map(|stop| {
      GradientStop::new(
        stop.offset,
        TinyColor::from_rgba8(
          stop.color.0[0],
          stop.color.0[1],
          stop.color.0[2],
          to_u8(stop.opacity * opacity),
        ),
      )
    })
    .collect()
}

impl From<SvgSpreadMethod> for SpreadMode {
  fn from(spread: SvgSpreadMethod) -> Self {
    match spread {
      SvgSpreadMethod::Pad => Self::Pad,
      SvgSpreadMethod::Reflect => Self::Reflect,
      SvgSpreadMethod::Repeat => Self::Repeat,
    }
  }
}

fn to_u8(opacity: f32) -> u8 {
  (opacity.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// `blend` in tiny-skia, with the dodge and burn modes the canvas blends on its own.
pub(crate) fn tiny_blend_mode(blend: BlendMode) -> TinyBlendMode {
  match blend {
    BlendMode::ColorDodge => TinyBlendMode::ColorDodge,
    BlendMode::ColorBurn => TinyBlendMode::ColorBurn,
    _ => TinyBlendMode::try_from(blend).unwrap_or(TinyBlendMode::SourceOver),
  }
}

#[cfg(test)]
mod tests {
  use std::sync::Arc;

  use tiny_skia::{IntRect, Pixmap, Transform};

  use super::Target;
  use crate::{
    context::RenderContext,
    geometry::Size,
    resources::{
      image::{ImageSource, RenderedImage, SvgSource, to_data_url},
      image_buffer::ImageBuffer,
    },
    style::{Color, ImageScalingAlgorithm},
    svg_vector::SvgOp,
  };

  const NS: &str = r#"xmlns="http://www.w3.org/2000/svg""#;
  const XLINK: &str = r#"xmlns:xlink="http://www.w3.org/1999/xlink""#;

  fn rasterize(svg: &str, size: u32, current_color: Color) -> Arc<ImageBuffer> {
    let source: ImageSource = svg.parse::<SvgSource>().unwrap().into();
    let rendered = source
      .render_for_layout(
        size,
        size,
        ImageScalingAlgorithm::Auto,
        0,
        &RenderContext::with_current_color(current_color),
      )
      .unwrap();
    let RenderedImage::Rasterized(buffer) = rendered else {
      panic!("expected a rasterized svg");
    };

    buffer
  }

  fn draw(body: &str, size: u32) -> Arc<ImageBuffer> {
    rasterize(
      &format!(r#"<svg {NS} {XLINK} width="{size}" height="{size}">{body}</svg>"#),
      size,
      Color::black(),
    )
  }

  /// An SVG in an `image` lays out at the image's rectangle, which its viewport units measure, as
  /// Blink's `SVGImagePainter::ComputeImageViewportSize` gives it.
  #[test]
  fn a_nested_svg_lays_out_at_its_image_rectangle() {
    let image = draw(
      r#"<image width="80" height="40" href="data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg'%3E%3Crect width='50vw' height='50vh' fill='red'/%3E%3C/svg%3E"/>"#,
      100,
    );

    assert_eq!(image.pixel(38, 18), [255, 0, 0, 255]);
    assert_eq!(image.pixel(42, 18), [0, 0, 0, 0]);
    assert_eq!(image.pixel(38, 22), [0, 0, 0, 0]);
  }

  /// A non-scaling stroke keeps its width in the host's units under a transform that scales.
  #[test]
  fn a_non_scaling_stroke_ignores_the_scale_of_its_transform() {
    let image = draw(
      r#"<line x2="10" y1="5" y2="5" stroke="red" stroke-width="2" vector-effect="non-scaling-stroke" transform="scale(5)"/>"#,
      100,
    );

    assert_eq!(image.pixel(20, 25), [255, 0, 0, 255]);
    assert_eq!(image.pixel(20, 22), [0, 0, 0, 0]);
  }

  /// A non-scaling stroke is as wide on both axes under a transform that scales them apart.
  #[test]
  fn a_non_scaling_stroke_is_even_under_a_transform_that_scales_its_axes_apart() {
    let image = draw(
      r#"<line x1="2" x2="2" y2="10" stroke="red" stroke-width="4" vector-effect="non-scaling-stroke" transform="scale(10 1)"/>"#,
      100,
    );

    assert_eq!(image.pixel(21, 5), [255, 0, 0, 255]);
    assert_eq!(image.pixel(22, 5), [0, 0, 0, 0]);
  }

  /// A transform that scales far down still has an inverse a non-scaling stroke draws through.
  #[test]
  fn a_non_scaling_stroke_draws_under_a_tiny_scale() {
    let image = draw(
      r#"<line x2="100000" y1="10000" y2="10000" stroke="red" stroke-width="4" vector-effect="non-scaling-stroke" transform="scale(0.0003)"/>"#,
      100,
    );

    assert_eq!(image.pixel(10, 3), [255, 0, 0, 255]);
  }

  #[test]
  fn clip_and_opacity_apply_to_a_group() {
    let image = draw(
      r##"<defs><clipPath id="c"><rect width="4" height="8"/></clipPath></defs>
      <g clip-path="url(#c)" opacity="0.5"><rect width="8" height="8" fill="#0000ff"/></g>"##,
      8,
    );

    assert_eq!(image.pixel(6, 4), [0, 0, 0, 0]);
    assert_eq!(image.pixel(2, 4), [0, 0, 128, 128]);
  }

  #[test]
  fn luminance_mask_cuts_by_brightness() {
    let image = draw(
      r##"<defs><mask id="m"><rect width="4" height="8" fill="#fff"/><rect x="4" width="4" height="8" fill="#000"/></mask></defs>
      <rect width="8" height="8" fill="#ff0000" mask="url(#m)"/>"##,
      8,
    );

    assert_eq!(image.pixel(2, 4), [255, 0, 0, 255]);
    assert_eq!(image.pixel(6, 4), [0, 0, 0, 0]);
  }

  #[test]
  fn pattern_fill_tiles_its_content() {
    let image = draw(
      r##"<defs><pattern id="p" width="4" height="4" patternUnits="userSpaceOnUse"><rect width="2" height="4" fill="#00ff00"/></pattern></defs>
      <rect width="8" height="8" fill="url(#p)"/>"##,
      8,
    );

    assert_eq!(image.pixel(1, 1), [0, 255, 0, 255]);
    assert_eq!(image.pixel(3, 1), [0, 0, 0, 0]);
    assert_eq!(image.pixel(5, 1), [0, 255, 0, 255]);
  }

  #[test]
  fn pattern_tile_over_the_raster_budget_paints_nothing() {
    let image = draw(
      r##"<defs><pattern id="p" width="9000" height="9000" patternUnits="userSpaceOnUse"><rect width="9000" height="9000" fill="#00ff00"/></pattern></defs>
      <rect width="8" height="8" fill="url(#p)"/>"##,
      8,
    );

    assert_eq!(image.pixel(4, 4), [0, 0, 0, 0]);
  }

  #[test]
  fn nested_filter_layer_stays_within_the_root_limit() {
    let source: SvgSource = format!(
      r##"<svg {NS} width="10" height="10"><filter id="f" filterUnits="userSpaceOnUse" x="-1000" y="-1000" width="5000" height="5000"><feOffset/></filter><rect width="10" height="10" filter="url(#f)"/></svg>"##
    )
    .parse()
    .unwrap();
    let ops = source.vector_ops(
      &RenderContext::with_current_color(Color::black()),
      Size {
        width: 10.0,
        height: 10.0,
      },
    );
    let Some(SvgOp::PushFilter(filters)) = ops.iter().find(|op| matches!(op, SvgOp::PushFilter(_)))
    else {
      panic!("expected a filter op");
    };
    let mut root = Pixmap::new(10, 10).unwrap();
    let mut root_view = root.as_mut();
    let root_target = Target::root(&mut root_view, Transform::identity()).unwrap();
    let bounds = root_target.layer_bounds(Some(filters)).unwrap();

    assert_eq!(bounds, IntRect::from_xywh(-20, -20, 50, 50).unwrap());

    let mut layer = Pixmap::new(bounds.width(), bounds.height()).unwrap();
    let mut layer_view = layer.as_mut();
    let nested = Target {
      pixmap: &mut layer_view,
      transform: Transform::from_translate(20.0, 20.0),
      clip: None,
      limit: root_target.limit.translate(20, 20).unwrap(),
    };

    assert_eq!(
      nested.layer_bounds(Some(filters)),
      IntRect::from_xywh(0, 0, 50, 50)
    );
  }

  #[test]
  fn a_displacement_map_moves_pixels_by_its_scale_once() {
    // A red map displaces by half the scale, so the square at 10..14 reads from 5 pixels on.
    let image = draw(
      r##"<defs><filter id="d" filterUnits="userSpaceOnUse" x="0" y="0" width="30" height="30">
        <feFlood flood-color="#ff0000" result="map"/>
        <feDisplacementMap in="SourceGraphic" in2="map" scale="10" xChannelSelector="R" yChannelSelector="R"/>
      </filter></defs>
      <rect x="10" y="10" width="4" height="4" fill="#000" filter="url(#d)"/>"##,
      30,
    );

    assert_eq!(image.pixel(7, 7)[3], 255);
    assert_eq!(image.pixel(12, 12)[3], 0);
  }

  #[test]
  fn a_result_nothing_reads_again_is_not_kept() {
    let image = draw(
      r##"<defs><filter id="f" filterUnits="userSpaceOnUse" x="0" y="0" width="20" height="20">
        <feFlood flood-color="#00ff00" result="unused"/>
        <feOffset in="SourceGraphic" dx="3"/>
      </filter></defs>
      <rect x="2" y="2" width="4" height="4" fill="#000" filter="url(#f)"/>"##,
      20,
    );

    assert_eq!(image.pixel(6, 3)[3], 255);
    assert_eq!(image.pixel(3, 3)[3], 0);
  }

  #[test]
  fn filter_blurs_its_group() {
    let sharp = draw(
      r##"<rect x="2" y="2" width="4" height="4" fill="#000"/>"##,
      8,
    );
    let blurred = draw(
      r##"<defs><filter id="b"><feGaussianBlur stdDeviation="1"/></filter></defs>
      <rect x="2" y="2" width="4" height="4" fill="#000" filter="url(#b)"/>"##,
      8,
    );

    assert_eq!(sharp.pixel(1, 4)[3], 0);
    assert!(blurred.pixel(1, 4)[3] > 0);
    assert!(blurred.pixel(3, 4)[3] < 255);
  }

  #[test]
  fn embedded_bitmap_draws_and_a_pixelated_one_keeps_its_edges() {
    let png = ImageBuffer::from_rgba_bytes(
      [
        255, 0, 0, 255, 0, 0, 255, 255, 0, 0, 255, 255, 255, 0, 0, 255,
      ]
      .to_vec(),
      2,
      2,
    )
    .unwrap()
    .encode_png()
    .unwrap();
    let href = to_data_url("image/png", &png);
    let image = |style: &str| {
      draw(
        &format!(r#"<image xlink:href="{href}" width="8" height="8" style="{style}"/>"#),
        8,
      )
    };
    let pixelated = image("image-rendering:pixelated");

    assert_eq!(image("").pixel(0, 0), [255, 0, 0, 255]);
    assert_eq!(pixelated.pixel(3, 0), [255, 0, 0, 255]);
    assert_eq!(pixelated.pixel(4, 0), [0, 0, 255, 255]);
  }

  // An `<image>` whose href is a local file path must not be read from disk.
  #[cfg(feature = "png")]
  #[test]
  fn image_href_local_path_is_not_read() {
    let opaque_red = ImageBuffer::from_rgba_bytes([255, 0, 0, 255].repeat(4 * 4), 4, 4)
      .unwrap()
      .encode_png()
      .unwrap();
    let path =
      std::env::temp_dir().join(format!("takumi_lfi_svg_drawing_{}.png", std::process::id()));

    std::fs::write(&path, &opaque_red).unwrap();

    let image = draw(
      &format!(
        r#"<image xlink:href="{}" width="8" height="8"/>"#,
        path.display()
      ),
      8,
    );

    std::fs::remove_file(&path).ok();

    assert!(
      image.data().iter().all(|&byte| byte == 0),
      "local file was read and composited into the SVG output"
    );
  }
}
