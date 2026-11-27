//! Draws flattened SVG ops onto a tiny-skia pixmap. A layer covers its supplied paint bounds, or a
//! filter layer's region when it has none.

use std::slice::Iter;

use tiny_skia::{
  FillRule, IntRect, IntSize, LineCap, LineJoin, Mask, Path, PathBuilder, PathStroker, Pixmap,
  PixmapMut, PixmapRef, Rect, Stroke, StrokeDash, Transform,
};

use super::filter::apply as apply_filter;
use crate::{
  blend::{blend_premultiplied_pixel, scale_premultiplied_pixel},
  coverage,
  shader::{Gradient, GradientShape, GradientStop, ImageFilter, ImageShader, Shader, Spread},
};
use takumi_core::{
  geometry::{PathCommand, Rect as ScopeRect},
  resources::{
    image::within_raster_pixel_budget,
    image_buffer::{composite_premultiplied_over_row, premultiply_pixel},
  },
  resources::{
    image::{
      SvgFill, SvgFilters, SvgGradient, SvgImageData, SvgLineCap, SvgLineJoin, SvgOp, SvgOpPaint,
      SvgRasterizer, SvgSpreadMethod, SvgStrokeStyle,
    },
    image_buffer::ImageBuffer,
  },
  style::{Affine, BlendMode, Color, ImageScalingAlgorithm, math::mul_div_255},
};

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

  /// Where a layer sits, from its paint bounds or a filter region within `limit` and the raster budget.
  fn layer_bounds(
    &self,
    scope: Option<ScopeRect<f32>>,
    filters: Option<&SvgFilters>,
  ) -> Option<IntRect> {
    if let Some(scope) = scope
      && let Some(bounds) = self.scope_bounds(scope)
    {
      return Some(bounds);
    }

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

  /// Draws ops up to the `Pop` closing the scope they are in, or the end of `ops`.
  fn run(&mut self, ops: &mut Iter<'_, SvgOp>) {
    while let Some(op) = ops.next() {
      match op {
        SvgOp::Pop => return,
        SvgOp::PushTransform(transform) => {
          let outer = self.transform;

          self.transform = outer.pre_concat(Transform::from(*transform));
          self.run(ops);
          self.transform = outer;
        }
        SvgOp::PushClip {
          path,
          evenodd,
          bounds,
        } => {
          if self.clip.is_none()
            && !scope_has_unbounded_effects(&mut ops.clone())
            && self.clip_layer(ops, path, *evenodd, *bounds)
          {
            continue;
          }
          let outer = self.clip.take();

          self.clip = clipped(outer.as_ref(), path, *evenodd, self);
          self.run(ops);
          self.clip = outer;
        }
        SvgOp::PushOpacity { opacity, bounds } => self.layer(
          ops,
          Composite {
            opacity: Some(*opacity),
            ..Composite::default()
          },
          *bounds,
        ),
        SvgOp::PushBlend { blend, bounds } => self.layer(
          ops,
          Composite {
            blend: Some(*blend),
            ..Composite::default()
          },
          *bounds,
        ),
        SvgOp::PushIsolation { bounds } => self.layer(ops, Composite::default(), *bounds),
        SvgOp::PushMask {
          ops: mask_ops,
          luminance,
        } => match self.mask(mask_ops, *luminance) {
          Some(mask) => self.layer(
            ops,
            Composite {
              mask: Some(mask),
              ..Composite::default()
            },
            None,
          ),
          None => skip_scope(ops),
        },
        SvgOp::PushFilter(filters) => self.layer(
          ops,
          Composite {
            filters: Some(filters),
            ..Composite::default()
          },
          None,
        ),
        SvgOp::Draw {
          path,
          fill,
          stroke,
          anti_alias,
          rect,
        } => self.draw_path(path, fill.as_ref(), stroke.as_ref(), *anti_alias, *rect),
        SvgOp::Image {
          data,
          width,
          height,
          rendering,
        } => self.draw_image(data, *width, *height, *rendering),
      }
    }
  }

  /// Draws the scope that follows into a fresh layer and composites it onto this target.
  fn layer(
    &mut self,
    ops: &mut Iter<'_, SvgOp>,
    composite: Composite,
    scope: Option<ScopeRect<f32>>,
  ) {
    let Some(bounds) = self.layer_bounds(scope, composite.filters) else {
      skip_scope(ops);
      return;
    };
    let Some(mut layer) = Pixmap::new(bounds.width(), bounds.height()) else {
      skip_scope(ops);
      return;
    };
    let transform =
      Transform::from_translate(-bounds.x() as f32, -bounds.y() as f32).pre_concat(self.transform);
    let Some(limit) = self.limit.translate(-bounds.x(), -bounds.y()) else {
      skip_scope(ops);
      return;
    };
    let mut view = layer.as_mut();

    Target {
      pixmap: &mut view,
      transform,
      clip: None,
      limit,
    }
    .run(ops);

    if let Some(filters) = composite.filters {
      for filter in filters.filters() {
        apply_filter(filter, transform, &mut layer);
      }
    }
    if let Some(mask) = &composite.mask {
      // Skia's `DestinationIn` with the mask's coverage.
      let pixels: &mut [[u8; 4]] = bytemuck::cast_slice_mut(layer.pixels_mut());

      for (pixel, alpha) in pixels.iter_mut().zip(mask.data()) {
        *pixel = scale_premultiplied_pixel(*pixel, *alpha);
      }
    }

    let opacity = composite.opacity.unwrap_or(1.0);

    match composite.blend {
      None | Some(BlendMode::Normal) => draw_over(
        self.pixmap,
        layer.as_ref(),
        (bounds.x(), bounds.y()),
        opacity,
        self.clip.as_ref(),
      ),
      Some(mode) => draw_blended(
        self.pixmap,
        layer.as_ref(),
        (bounds.x(), bounds.y()),
        opacity,
        self.clip.as_ref(),
        mode,
      ),
    }
  }

  /// Draws a clipped scope against a copied backdrop, then replaces that clipped region.
  fn clip_layer(
    &mut self,
    ops: &mut Iter<'_, SvgOp>,
    path: &[PathCommand],
    evenodd: bool,
    scope: Option<ScopeRect<f32>>,
  ) -> bool {
    let Some(scope) = scope.and_then(|scope| self.scope_bounds(scope)) else {
      return false;
    };
    if !within_raster_pixel_budget(scope.width(), scope.height()) {
      return false;
    }
    let Some(mut layer) = Pixmap::new(scope.width(), scope.height()) else {
      return false;
    };

    copy_to_layer(self.pixmap.as_ref(), &mut layer, scope);
    let transform = Transform::from_translate(-(scope.x() as f32), -(scope.y() as f32))
      .pre_concat(self.transform);
    let Some(limit) = self.limit.translate(-scope.x(), -scope.y()) else {
      return false;
    };
    let mut view = layer.as_mut();
    let mut target = Target {
      pixmap: &mut view,
      transform,
      clip: None,
      limit,
    };
    let Some(clip) = clipped(None, path, evenodd, &target) else {
      return false;
    };

    target.clip = Some(clip);
    target.run(ops);
    copy_from_layer(&layer, self.pixmap, scope);
    true
  }

  /// The mask `ops` draw, which cuts the layer it is pushed with: their alpha, or their luminance as
  /// tiny-skia's `Mask::from_pixmap` reads it.
  fn mask(&self, ops: &[SvgOp], luminance: bool) -> Option<Mask> {
    let (width, height) = (self.pixmap.width(), self.pixmap.height());
    let mut drawn = Pixmap::new(width, height)?;

    draw(ops, self.transform, &mut drawn.as_mut());

    let coverage = drawn
      .pixels()
      .iter()
      .map(|pixel| {
        if !luminance {
          return pixel.alpha();
        }

        let alpha = f32::from(pixel.alpha()) / 255.0;
        let channel = |value: u8| {
          let value = f32::from(value) / 255.0;

          if pixel.alpha() == 0 {
            value
          } else {
            value / alpha
          }
        };
        let luma = channel(pixel.red()) * 0.2126
          + channel(pixel.green()) * 0.7152
          + channel(pixel.blue()) * 0.0722;

        ((luma * alpha) * 255.0).clamp(0.0, 255.0).ceil() as u8
      })
      .collect();

    Mask::from_vec(coverage, IntSize::from_wh(width, height)?)
  }

  fn scope_bounds(&self, scope: ScopeRect<f32>) -> Option<IntRect> {
    let bounds = Rect::from_ltrb(scope.left, scope.top, scope.right, scope.bottom)?;

    self.device_bounds(bounds.transform(self.transform)?.outset(2.0, 2.0)?)
  }

  fn draw_path(
    &mut self,
    commands: &[PathCommand],
    fill: Option<&SvgFill>,
    stroke: Option<&SvgStrokeStyle>,
    anti_alias: bool,
    rect: bool,
  ) {
    let Some(path) = tiny_path(commands) else {
      return;
    };

    if let Some(fill) = fill {
      // Horizontal and vertical lines have no area to fill.
      if path.bounds().width() != 0.0 && path.bounds().height() != 0.0 {
        let rule = if fill.evenodd {
          FillRule::EvenOdd
        } else {
          FillRule::Winding
        };
        let mut tile = None;

        if let Some(shader) = shader_of(&fill.paint, fill.opacity, self.transform, &mut tile) {
          if rect && anti_alias {
            self.fill_rect(&path, &shader);
          } else {
            self.fill(&path, rule, &shader, anti_alias);
          }
        }
      }
    }
    if let Some(stroke) = stroke {
      // As tiny-skia's `stroke_path` strokes. Approximate: a stroke thinner than a device pixel
      // outlines and fills, where Skia draws it as a hairline of reduced coverage.
      let resolution = PathStroker::compute_resolution_scale(&self.transform);
      let style = tiny_stroke(stroke);
      let dashed = match &style.dash {
        Some(dash) => path.dash(dash, resolution),
        None => Some(path),
      };
      let mut tile = None;

      if let Some(outline) = dashed.and_then(|path| path.stroke(&style, resolution))
        && let Some(shader) = shader_of(&stroke.paint, stroke.opacity, self.transform, &mut tile)
      {
        self.fill(&outline, FillRule::Winding, &shader, anti_alias);
      }
    }
  }

  /// Fills `path` under the target's transform with `shader`, through the target's clip.
  fn fill(&mut self, path: &Path, rule: FillRule, shader: &Shader, anti_alias: bool) {
    let Some(device) = path.clone().transform(self.transform) else {
      return;
    };
    let Some(bounds) = self.device_bounds(device.bounds()) else {
      return;
    };
    let Some(local) = device.transform(Transform::from_translate(
      -bounds.x() as f32,
      -bounds.y() as f32,
    )) else {
      return;
    };
    let coverage = if anti_alias {
      coverage::fill_path(&local, rule, bounds.size())
    } else {
      coverage::fill_path_aliased(&local, rule, bounds.size())
    };

    if let Some(coverage) = coverage {
      self.blit(&coverage, bounds, shader);
    }
  }

  /// Fills the rect `path` outlines with `shader` as Skia's `SkDraw::drawRect` does: through
  /// `SkScan::AntiFillRect` where the transform keeps it a rect, and as a path otherwise.
  fn fill_rect(&mut self, path: &Path, shader: &Shader) {
    let Transform {
      sx,
      kx,
      ky,
      sy,
      tx,
      ty,
    } = self.transform;

    // `SkMatrix::rectStaysRect`: axis aligned, or turned a quarter.
    if !(kx == 0.0 && ky == 0.0 || sx == 0.0 && sy == 0.0) {
      self.fill(path, FillRule::Winding, shader, true);
      return;
    }

    let map = |x: f32, y: f32| (x * sx + y * kx + tx, x * ky + y * sy + ty);
    let rect = path.bounds();
    let ((x0, y0), (x1, y1)) = (
      map(rect.left(), rect.top()),
      map(rect.right(), rect.bottom()),
    );
    let (left, right) = (x0.min(x1), x0.max(x1));
    let (top, bottom) = (y0.min(y1), y0.max(y1));
    let Some(bounds) =
      Rect::from_ltrb(left, top, right, bottom).and_then(|device| self.device_bounds(device))
    else {
      return;
    };
    let (dx, dy) = (bounds.x() as f32, bounds.y() as f32);

    if let Some(coverage) = coverage::fill_rect(
      [left - dx, top - dy, right - dx, bottom - dy],
      bounds.size(),
    ) {
      self.blit(&coverage, bounds, shader);
    }
  }

  /// The target's pixels `device` touches, none where it misses the target.
  fn device_bounds(&self, device: Rect) -> Option<IntRect> {
    let (width, height) = (self.pixmap.width(), self.pixmap.height());
    // An edge far past the target would saturate as it rounds to `i32`.
    let near_target = Rect::from_ltrb(
      device.left().clamp(-1.0, width as f32 + 1.0),
      device.top().clamp(-1.0, height as f32 + 1.0),
      device.right().clamp(-1.0, width as f32 + 1.0),
      device.bottom().clamp(-1.0, height as f32 + 1.0),
    )?;

    near_target
      .round_out()?
      .intersect(&IntRect::from_xywh(0, 0, width, height)?)
  }

  /// Composites `shader` over the target where `coverage`, a mask over `bounds`, covers it, through
  /// the target's clip.
  fn blit(&mut self, coverage: &[u8], bounds: IntRect, shader: &Shader) {
    let canvas_width = self.pixmap.width() as usize;
    let span = bounds.width() as usize;
    let clip = self.clip.as_ref().map(Mask::data);
    let pixels: &mut [[u8; 4]] = bytemuck::cast_slice_mut(self.pixmap.pixels_mut());
    let mut row = vec![[0; 4]; span];

    for (index, y) in (bounds.top()..bounds.bottom()).enumerate() {
      let start = y as usize * canvas_width + bounds.left() as usize;

      shader.shade_row(bounds.left(), y, &mut row);

      for (offset, pixel) in row.iter_mut().enumerate() {
        let mut alpha = coverage[index * span + offset];

        if let Some(clip) = clip {
          alpha = mul_div_255(alpha, clip[start + offset]);
        }

        *pixel = scale_premultiplied_pixel(*pixel, alpha);
      }

      composite_premultiplied_over_row(&mut pixels[start..start + span], &row);
    }
  }

  fn draw_image(
    &mut self,
    data: &SvgImageData,
    width: f32,
    height: f32,
    rendering: ImageScalingAlgorithm,
  ) {
    let Some(buffer) = data.decode() else {
      return;
    };
    let Some(rect) = Rect::from_xywh(0.0, 0.0, width, height) else {
      return;
    };
    let image_to_device = Affine::from(self.transform)
      * Affine::scale(
        width / buffer.width() as f32,
        height / buffer.height() as f32,
      );
    let Some(image) = ImageShader::new(
      bytemuck::cast_slice(buffer.data()),
      (buffer.width(), buffer.height()),
      image_to_device,
      Spread::Pad,
      if rendering == ImageScalingAlgorithm::Pixelated {
        ImageFilter::Nearest
      } else {
        ImageFilter::Linear
      },
      1.0,
    ) else {
      return;
    };

    self.fill(
      &PathBuilder::from_rect(rect),
      FillRule::Winding,
      &Shader::Image(image),
      true,
    );
  }
}

/// The shader `paint` at `opacity` draws with under `transform`, rasterizing a pattern's tile into
/// `tile` at `transform`'s scale. `None` where it paints nothing.
fn shader_of<'t>(
  paint: &SvgOpPaint,
  opacity: f32,
  transform: Transform,
  tile: &'t mut Option<Pixmap>,
) -> Option<Shader<'t>> {
  let gradient = |shape, gradient: &SvgGradient| match Gradient::new(
    shape,
    &gradient_stops(gradient, opacity),
    spread_mode(gradient.spread),
    Affine::from(transform) * gradient.transform,
  ) {
    Ok(gradient) => Some(Shader::Gradient(gradient)),
    Err(color) => color.map(Shader::Solid),
  };

  match paint {
    SvgOpPaint::Color(Color([red, green, blue, _])) => Some(Shader::Solid(premultiply_pixel([
      *red,
      *green,
      *blue,
      to_u8(opacity),
    ]))),
    SvgOpPaint::Linear {
      start,
      end,
      gradient: stops,
    } => gradient(
      GradientShape::Linear {
        start: [start.x, start.y],
        end: [end.x, end.y],
      },
      stops,
    ),
    SvgOpPaint::Radial {
      center,
      radius,
      focal,
      focal_radius,
      gradient: stops,
    } => gradient(
      GradientShape::Conical {
        start: [focal.x, focal.y],
        start_radius: *focal_radius,
        end: [center.x, center.y],
        end_radius: *radius,
      },
      stops,
    ),
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
        return None;
      }

      let mut pixmap = Pixmap::new(tile_width, tile_height)?;

      draw(
        ops,
        Transform::from_scale(scale_x, scale_y),
        &mut pixmap.as_mut(),
      );

      let tile = tile.insert(pixmap);
      let tile_to_device = Affine::from(transform)
        * Affine::from(pattern_transform)
        * Affine::scale(1.0 / scale_x, 1.0 / scale_y);

      ImageShader::new(
        bytemuck::cast_slice(tile.pixels()),
        (tile.width(), tile.height()),
        tile_to_device,
        Spread::Repeat,
        ImageFilter::Linear,
        opacity,
      )
      .map(Shader::Image)
    }
  }
}

/// How a layer joins the target below it.
#[derive(Default)]
struct Composite<'a> {
  opacity: Option<f32>,
  blend: Option<BlendMode>,
  mask: Option<Mask>,
  filters: Option<&'a SvgFilters>,
}

/// Draws an SVG image's ops onto pixels, for [`SvgSource::rasterize_cached`](takumi_core::resources::image::SvgSource::rasterize_cached).
pub struct SvgPixmapRasterizer;

impl SvgRasterizer for SvgPixmapRasterizer {
  fn rasterize(
    &self,
    ops: &[SvgOp],
    transform: Affine,
    width: u32,
    height: u32,
  ) -> Option<ImageBuffer> {
    let mut pixmap = Pixmap::new(width, height)?;

    draw(ops, transform.into(), &mut pixmap.as_mut());
    ImageBuffer::from_premultiplied_rgba(pixmap.take(), width, height)
  }
}

/// Draws `ops` onto `pixmap` with `transform` as the canvas transform.
pub(crate) fn draw(ops: &[SvgOp], transform: Transform, pixmap: &mut PixmapMut) {
  if let Some(mut target) = Target::root(pixmap, transform) {
    target.run(&mut ops.iter());
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

/// Blends `source` into `target` under `mode` with its top left at `origin`, at `opacity` and inside
/// `clip` when there is one.
fn draw_blended(
  target: &mut PixmapMut,
  source: PixmapRef,
  (x, y): (i32, i32),
  opacity: f32,
  clip: Option<&Mask>,
  mode: BlendMode,
) {
  let target_width = target.width() as i32;
  let left = x.max(0);
  let right = (x + source.width() as i32).min(target_width);
  let top = y.max(0);
  let bottom = (y + source.height() as i32).min(target.height() as i32);
  let alpha = to_u8(opacity);
  let target_pixels: &mut [[u8; 4]] = bytemuck::cast_slice_mut(target.pixels_mut());
  let source_pixels: &[[u8; 4]] = bytemuck::cast_slice(source.pixels());

  for row in top..bottom {
    for column in left..right {
      let index = (row * target_width + column) as usize;
      let mut pixel = scale_premultiplied_pixel(
        source_pixels[((row - y) * source.width() as i32 + column - x) as usize],
        alpha,
      );

      if let Some(clip) = clip {
        pixel = scale_premultiplied_pixel(pixel, clip.data()[index]);
      }

      blend_premultiplied_pixel(&mut target_pixels[index], pixel, mode);
    }
  }
}

fn copy_to_layer(source: PixmapRef, target: &mut Pixmap, bounds: IntRect) {
  let width = bounds.width() as usize;
  let source_width = source.width() as usize;
  let source_x = bounds.x() as usize;
  let source_y = bounds.y() as usize;

  for row in 0..bounds.height() as usize {
    let source_start = ((source_y + row) * source_width + source_x) * 4;
    let target_start = row * width * 4;

    target.data_mut()[target_start..][..width * 4]
      .copy_from_slice(&source.data()[source_start..][..width * 4]);
  }
}

fn copy_from_layer(source: &Pixmap, target: &mut PixmapMut, bounds: IntRect) {
  let width = bounds.width() as usize;
  let target_width = target.width() as usize;
  let target_x = bounds.x() as usize;
  let target_y = bounds.y() as usize;

  for row in 0..bounds.height() as usize {
    let source_start = row * width * 4;
    let target_start = ((target_y + row) * target_width + target_x) * 4;

    target.data_mut()[target_start..][..width * 4]
      .copy_from_slice(&source.data()[source_start..][..width * 4]);
  }
}

fn scope_has_unbounded_effects(ops: &mut Iter<'_, SvgOp>) -> bool {
  let mut depth = 0;

  for op in ops {
    match op {
      SvgOp::PushFilter(_) | SvgOp::PushMask { .. } => return true,
      SvgOp::Pop if depth == 0 => return false,
      SvgOp::Pop => depth -= 1,
      SvgOp::PushTransform(_)
      | SvgOp::PushClip { .. }
      | SvgOp::PushOpacity { .. }
      | SvgOp::PushBlend { .. }
      | SvgOp::PushIsolation { .. } => depth += 1,
      SvgOp::Draw { .. } | SvgOp::Image { .. } => {}
    }
  }

  false
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
      | SvgOp::PushOpacity { .. }
      | SvgOp::PushBlend { .. }
      | SvgOp::PushIsolation { .. }
      | SvgOp::PushMask { .. }
      | SvgOp::PushFilter(_) => depth += 1,
      SvgOp::Draw { .. } | SvgOp::Image { .. } => {}
    }
  }
}

/// The clip `commands` cut under the target's transform, intersected with `outer`.
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
  let size = IntSize::from_wh(target.pixmap.width(), target.pixmap.height())?;
  let mut coverage = match tiny_path(commands).and_then(|path| path.transform(target.transform)) {
    Some(path) => coverage::fill_path(&path, rule, size)?,
    None => vec![0; size.width() as usize * size.height() as usize],
  };

  if let Some(outer) = outer {
    for (alpha, outer) in coverage.iter_mut().zip(outer.data()) {
      *alpha = mul_div_255(*alpha, *outer);
    }
  }

  Mask::from_vec(coverage, size)
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

/// The tiny-skia stroke of `stroke`.
fn tiny_stroke(stroke: &SvgStrokeStyle) -> Stroke {
  Stroke {
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

fn gradient_stops(gradient: &SvgGradient, opacity: f32) -> Vec<GradientStop> {
  gradient
    .stops
    .iter()
    .map(|stop| {
      let [red, green, blue, _] = stop.color.0;
      let alpha = to_u8(stop.opacity * opacity);

      GradientStop {
        position: stop.offset,
        color: [red, green, blue, alpha].map(|channel| f32::from(channel) / 255.0),
      }
    })
    .collect()
}

/// The spread of `spread`.
fn spread_mode(spread: SvgSpreadMethod) -> Spread {
  match spread {
    SvgSpreadMethod::Pad => Spread::Pad,
    SvgSpreadMethod::Reflect => Spread::Reflect,
    SvgSpreadMethod::Repeat => Spread::Repeat,
  }
}

fn to_u8(opacity: f32) -> u8 {
  (opacity.clamp(0.0, 1.0) * 255.0).round() as u8
}

#[cfg(test)]
mod tests {
  use std::sync::Arc;

  use tiny_skia::{IntRect, Pixmap, Transform};

  use super::{ScopeRect, SvgPixmapRasterizer, Target};
  use takumi_core::{
    Fonts,
    context::RenderContext,
    geometry::Size,
    resources::{
      cache::Cache,
      image::{ImageCacheMode, ImageError, ImageSource, SvgOp, SvgRaster, SvgSource, to_data_url},
      image_buffer::ImageBuffer,
    },
    style::{Color, ImageScalingAlgorithm, SizingContext},
    viewport::Viewport,
  };

  const NS: &str = r#"xmlns="http://www.w3.org/2000/svg""#;
  const XLINK: &str = r#"xmlns:xlink="http://www.w3.org/1999/xlink""#;

  fn context(current_color: Color) -> RenderContext {
    RenderContext::builder()
      .fonts(Fonts::default().snapshot())
      .sizing(
        SizingContext::builder()
          .viewport(Viewport::new((800, 600)))
          .build(),
      )
      .current_color(current_color)
      .build()
  }

  fn rasterize(svg: &str, size: u32, current_color: Color) -> Arc<ImageBuffer> {
    let viewport = Size {
      width: size as f32,
      height: size as f32,
    };

    svg
      .parse::<SvgSource>()
      .unwrap()
      .rasterize_cached(
        SvgRaster::whole(viewport, size, size),
        ImageScalingAlgorithm::Auto,
        &context(current_color),
        &SvgPixmapRasterizer,
      )
      .unwrap()
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
  fn a_fill_past_the_i32_range_still_covers_the_target() {
    let image = draw(
      r#"<g opacity="0.5"><path d="M-3e9 -3e9 H3e9 V3e9 H-3e9 Z" fill="red"/></g>"#,
      10,
    );

    assert_eq!(image.pixel(5, 5), [128, 0, 0, 128]);
  }

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
  fn clip_applies_to_a_direct_group() {
    let image = draw(
      r##"<defs><clipPath id="c"><rect width="4" height="8"/></clipPath></defs>
      <g clip-path="url(#c)"><rect width="8" height="8" fill="#0000ff"/></g>"##,
      8,
    );

    assert_eq!(image.pixel(6, 4), [0, 0, 0, 0]);
    assert_eq!(image.pixel(2, 4), [0, 0, 255, 255]);
  }

  #[test]
  fn a_clipped_blend_uses_its_backdrop() {
    let image = draw(
      r##"<defs><clipPath id="c"><rect width="4" height="8"/></clipPath></defs>
      <rect width="8" height="8" fill="#ff0000"/>
      <g clip-path="url(#c)"><rect width="8" height="8" fill="#0000ff" style="mix-blend-mode:multiply"/></g>"##,
      8,
    );

    assert_eq!(image.pixel(2, 4), [0, 0, 0, 255]);
    assert_eq!(image.pixel(6, 4), [255, 0, 0, 255]);
  }

  #[test]
  fn isolation_applies_to_a_solid_group() {
    let image = draw(
      r##"<g style="isolation:isolate"><rect x="2" y="3" width="4" height="2" fill="#0000ff"/></g>"##,
      8,
    );

    assert_eq!(image.pixel(1, 4), [0, 0, 0, 0]);
    assert_eq!(image.pixel(3, 4), [0, 0, 255, 255]);
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
    let picture = source.picture(
      &context(Color::black()),
      Size {
        width: 10.0,
        height: 10.0,
      },
    );
    let Some(SvgOp::PushFilter(filters)) = picture
      .ops()
      .iter()
      .find(|op| matches!(op, SvgOp::PushFilter(_)))
    else {
      panic!("expected a filter op");
    };
    let mut root = Pixmap::new(10, 10).unwrap();
    let mut root_view = root.as_mut();
    let root_target = Target::root(&mut root_view, Transform::identity()).unwrap();
    let bounds = root_target.layer_bounds(None, Some(filters)).unwrap();

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
      nested.layer_bounds(None, Some(filters)),
      IntRect::from_xywh(0, 0, 50, 50)
    );
  }

  #[test]
  fn an_isolation_layer_covers_its_paint_bounds() {
    let mut pixmap = Pixmap::new(100, 100).unwrap();
    let mut view = pixmap.as_mut();
    let target = Target::root(&mut view, Transform::identity()).unwrap();

    assert_eq!(
      target.scope_bounds(ScopeRect {
        left: 20.0,
        top: 30.0,
        right: 30.0,
        bottom: 50.0,
      }),
      IntRect::from_xywh(18, 28, 14, 24)
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

  #[test]
  fn a_spotlight_clamps_its_exponent_as_blink_does() {
    let spot = |exponent: &str| {
      draw(
        &format!(
          r#"<filter id="f" x="0" y="0" width="1" height="1"><feSpecularLighting specularExponent="4"><feSpotLight x="20" y="0" z="30" pointsAtX="20" pointsAtY="40" specularExponent="{exponent}"/></feSpecularLighting></filter><rect width="40" height="40" filter="url(#f)"/>"#
        ),
        40,
      )
    };

    assert_eq!(spot("0.5").data(), spot("1").data());
    assert_eq!(spot("500").data(), spot("128").data());
    assert_ne!(spot("2").data(), spot("1").data());
  }

  /// `image`, an SVG, rasterized at `width` by `height` for `host`.
  fn rasterized_at(
    image: &ImageSource,
    width: u32,
    height: u32,
    host: &RenderContext,
  ) -> Result<Arc<ImageBuffer>, ImageError> {
    let ImageSource::Svg(svg) = image else {
      panic!("expected an svg");
    };
    let viewport = Size {
      width: width as f32,
      height: height as f32,
    };

    svg.rasterize_cached(
      SvgRaster::whole(viewport, width, height),
      ImageScalingAlgorithm::Auto,
      host,
      &SvgPixmapRasterizer,
    )
  }

  /// `image`, an SVG, rasterized at `size` by `size` for `host`.
  fn rasterized(
    image: &ImageSource,
    size: u32,
    host: &RenderContext,
  ) -> Result<Arc<ImageBuffer>, ImageError> {
    rasterized_at(image, size, size, host)
  }

  // A namespace-less root is accepted, so markup without `xmlns` is still
  // detected and rendered as SVG.
  #[test]
  fn svg_without_xmlns_renders() -> Result<(), ImageError> {
    let svg = r##"<svg width="4" height="4"><rect width="4" height="4" fill="#ff0000"/></svg>"##;
    let image = ImageSource::from_bytes(svg.as_bytes())?;

    assert!(matches!(image, ImageSource::Svg(_)));
    let rendered = rasterized(&image, 4, &context(Color::black()))?;
    assert_eq!(rendered.pixel(2, 2), [255, 0, 0, 255]);
    Ok(())
  }

  // `currentColor` resolves against the host color in an `svg` element, as an inline `svg`
  // inherits `color`, and to black in an image, a document isolated from its host.
  #[test]
  fn svg_current_color_resolves_to_host_color_only_in_an_svg_element() -> Result<(), ImageError> {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4"><rect x="0" y="0" width="4" height="4" fill="currentColor"/></svg>"#;
    let image = ImageSource::from_bytes(svg.as_bytes())?;
    let host = context(Color([255, 0, 0, 255]));
    let render = |host: &RenderContext| rasterized(&image, 4, host);

    assert_eq!(
      render(&host.for_svg_element())?.pixel(2, 2),
      [255, 0, 0, 255]
    );
    assert_eq!(render(&host)?.pixel(2, 2), [0, 0, 0, 255]);
    Ok(())
  }

  // The dependency on the host color is detected on decoded attribute values,
  // so an entity-encoded `currentColor` inherits too.
  #[test]
  fn svg_entity_encoded_current_color_resolves_to_host_color() -> Result<(), ImageError> {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4"><rect x="0" y="0" width="4" height="4" fill="current&#67;olor"/></svg>"#;
    let image = ImageSource::from_bytes(svg.as_bytes())?;

    let rendered = rasterized(
      &image,
      4,
      &context(Color([255, 0, 0, 255])).for_svg_element(),
    )?;

    assert_eq!(rendered.pixel(2, 2), [255, 0, 0, 255]);
    Ok(())
  }

  // Two hosts of different colors drawing one source through one cache each get their own color.
  #[test]
  fn a_cached_raster_keeps_each_host_color() -> Result<(), ImageError> {
    let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4" color="currentColor"><rect width="4" height="4" fill="currentColor"/></svg>"#;
    let cache = Cache::default();
    let image = cache.decode_image(svg, ImageCacheMode::Auto)?;
    let draw = |color| {
      let host = RenderContext::builder()
        .fonts(Fonts::default().snapshot())
        .sizing(
          SizingContext::builder()
            .viewport(Viewport::new((4, 4)))
            .build(),
        )
        .current_color(color)
        .cache(cache.clone())
        .build()
        .for_svg_element();

      rasterized(&image, 4, &host)
    };

    draw(Color([255, 0, 0, 255]))?;

    assert_eq!(draw(Color([0, 0, 255, 255]))?.pixel(2, 2), [0, 0, 255, 255]);
    Ok(())
  }

  // A `color` attribute on the root wins over the host color, so the host
  // never overrides what the SVG defines itself.
  #[test]
  fn svg_own_color_attribute_beats_host_color() -> Result<(), ImageError> {
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4" color="#00ff00"><rect x="0" y="0" width="4" height="4" fill="currentColor"/></svg>"##;
    let image = ImageSource::from_bytes(svg.as_bytes())?;

    let rendered = rasterized(&image, 4, &context(Color([255, 0, 0, 255])))?;

    assert_eq!(rendered.pixel(2, 2), [0, 255, 0, 255]);
    Ok(())
  }

  #[test]
  fn svg_rasterization_rejects_oversized_target() {
    let source: ImageSource = "<svg width=\"1\" height=\"1\"/>"
      .parse::<SvgSource>()
      .unwrap()
      .into();

    let result = rasterized_at(&source, 8193, 8192, &context(Color::black()));

    assert!(matches!(result, Err(ImageError::InvalidPixmapSize)));
  }
}
