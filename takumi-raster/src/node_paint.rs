//! Raster box-decoration painting (backgrounds, borders, outlines, box-shadows).
//!
//! The backend-agnostic geometry — clip regions and the outline ring — lives in
//! [`takumi_core::layout::decoration`]; these functions composite it with
//! tiny-skia, and the SVG backend emits the same geometry as vector paths.

use skrifa::{FontRef, MetadataProvider};
use takumi_core::{
  geometry::{ComputedLayout as Layout, Point, Size},
  layout::{
    decoration::ClipBox,
    inline::{PositionedGlyph, PositionedInlineRun},
  },
  painter::{
    BackgroundClipArea, BoxBorderPainter, BoxFrame, BoxPainter, FillShape, GlyphDevice, GlyphFill,
    PaintDevice, PendingOutline, ShadowShape, StrokeStyle,
  },
  resources::{font::FontError, glyph::ResolvedGlyph},
  shadow::SizedShadow,
  style::{Color, ImageScalingAlgorithm},
};

use super::{
  BackgroundTile, BorderProperties, Canvas, CanvasViewport, ColorTile, Fill, PaintSource,
  RenderContext, SizedFontStyle, TileLayer, TileLayers, background_image_layers,
  collect_background_layers, draw_image, rasterize_layers,
};
use crate::{
  BlurType, CanvasSubcanvas, Command, Error, MaskCompositeColor, MaskSamplingOptions, Placement,
  Result, Stroke, Style, apply_blur_alpha_bytes, attenuate_alpha_by_mask, bitmap_coverage,
  checked_area, draw_glyph, draw_glyph_clip_image, intersect_alpha_masks,
  layout::node::ImageData,
  render_mask,
  style::{Affine, BlendMode},
};

/// Paints a box's own decorations, bottom to top: outset shadows, background,
/// inset shadows, and border.
pub(crate) fn draw_box_shell(
  context: &RenderContext,
  canvas: &mut Canvas,
  layout: Layout,
) -> Result<()> {
  let painter = BoxPainter::new(context, layout);

  CanvasDevice::paint(canvas, context, |device| {
    painter.paint_normal_box_shadows(Point::ZERO, device);
  })?;
  draw_background(context, canvas, layout)?;
  CanvasDevice::paint(canvas, context, |device| {
    painter.paint_inset_box_shadows(Point::ZERO, device);
  })?;
  draw_border(context, canvas, layout)
}

/// The canvas as a [`PaintDevice`]. An unclipped rounded rectangle composites
/// through the same border machinery the tile path uses, so a background colour
/// rasterizes as it always has.
pub(crate) struct CanvasDevice<'c> {
  pub(crate) canvas: &'c mut Canvas,
  pub(crate) transform: Affine,
  pub(crate) algorithm: ImageScalingAlgorithm,
  /// Each open clip.
  pub(crate) clips: Vec<CanvasClip>,
  /// Each open layer and the opacity it composites at, or `None` when it could not open.
  layers: Vec<Option<(CanvasSubcanvas, f32)>>,
  /// The shadow every draw becomes while one is open.
  shadow: Option<SizedShadow>,
  /// The background `background-clip: text` glyphs show.
  pub(crate) text_background: Option<PaintSource<'c>>,
  /// The first error a draw hit.
  error: Option<Error>,
}

/// A clip the canvas device holds: a shape's coverage, and whether draws keep to it or avoid it.
pub(crate) struct CanvasClip {
  coverage: Vec<u8>,
  placement: Placement,
  out: bool,
}

impl<'c> CanvasDevice<'c> {
  pub(crate) fn new(
    canvas: &'c mut Canvas,
    transform: Affine,
    algorithm: ImageScalingAlgorithm,
  ) -> Self {
    Self {
      canvas,
      transform,
      algorithm,
      clips: Vec::new(),
      layers: Vec::new(),
      shadow: None,
      text_background: None,
      error: None,
    }
  }

  /// Whether a solid tile paints a `size` rectangle under `transform` as its coverage would.
  fn tiles_whole_pixels(&self, size: Size<f32>, transform: Affine) -> bool {
    let transform = self.transform * transform;

    size.width.fract() == 0.0
      && size.height.fract() == 0.0
      && (!transform.only_translation()
        || (transform.x.fract() == 0.0 && transform.y.fract() == 0.0))
  }

  /// The canvas as a device for the box `context` paints.
  pub(crate) fn of(canvas: &'c mut Canvas, context: &RenderContext) -> Self {
    Self::new(canvas, context.transform, context.style.image_rendering)
  }

  /// Surfaces the first error a draw hit.
  pub(crate) fn finish(self) -> Result<()> {
    self.error.map_or(Ok(()), Err)
  }

  /// Runs `paint` against `canvas` for the box `context` paints, surfacing the first error a
  /// draw hit.
  pub(crate) fn paint(
    canvas: &'c mut Canvas,
    context: &RenderContext,
    paint: impl FnOnce(&mut Self),
  ) -> Result<()> {
    let mut device = Self::of(canvas, context);

    paint(&mut device);
    device.finish()
  }

  /// Rasterizes `shape` under `transform`, culled to the canvas.
  fn coverage(&self, shape: &FillShape, style: Style, transform: Affine) -> (Vec<u8>, Placement) {
    render_mask(
      &shape.to_commands(),
      Some(self.transform * transform),
      Some(style),
      Some(self.canvas.viewport()),
    )
  }

  /// Limits `coverage` to the open clips, or `None` when nothing is left.
  fn clipped(&self, coverage: (Vec<u8>, Placement)) -> Option<(Vec<u8>, Placement)> {
    self
      .clips
      .iter()
      .try_fold(coverage, |(mut mask, placement), clip| {
        if clip.out {
          attenuate_alpha_by_mask(&mut mask, placement, &clip.coverage, clip.placement);

          return Some((mask, placement));
        }

        intersect_alpha_masks(&mask, placement, &clip.coverage, clip.placement)
      })
  }

  /// Opens a clip to `shape`, or out of it when `out` is set.
  fn open_clip(&mut self, shape: &FillShape, transform: Affine, out: bool) {
    let (coverage, placement) = self.coverage(shape, Fill::from(shape.rule()).into(), transform);

    self.clips.push(CanvasClip {
      coverage,
      placement,
      out,
    });
  }

  /// Paints `coverage` in `color`, limited to the open clips.
  fn draw_coverage(&mut self, coverage: (Vec<u8>, Placement), color: Color) {
    if let Some((mask, placement)) = self.clipped(coverage) {
      self
        .canvas
        .draw_mask(&mask, placement, color, BlendMode::Normal);
    }
  }

  /// Paints `commands`, filled or stroked as `style` under `transform`, in `color` blurred as a CSS
  /// shadow of `blur_radius` blurs.
  fn draw_blurred(
    &mut self,
    commands: &[Command],
    style: Style,
    transform: Affine,
    blur_radius: f32,
    color: Color,
  ) {
    let coverage = render_mask(
      commands,
      Some(self.transform * transform),
      Some(style),
      Some(self.shadow_viewport(blur_radius)),
    );

    self.draw_blurred_coverage(coverage, blur_radius, color);
  }

  /// The canvas viewport grown by how far a shadow of `blur_radius` blurs.
  fn shadow_viewport(&self, blur_radius: f32) -> CanvasViewport {
    let reach = BlurType::Shadow.extent(blur_radius);

    self.canvas.viewport().inflate(reach, reach)
  }

  /// Paints `coverage` in `color`, blurred as a CSS shadow of `blur_radius` blurs.
  fn draw_blurred_coverage(
    &mut self,
    (mask, placement): (Vec<u8>, Placement),
    blur_radius: f32,
    color: Color,
  ) {
    if mask.is_empty() {
      return;
    }
    if blur_radius <= 0.0 {
      return self.draw_coverage((mask, placement), color);
    }

    let padding = BlurType::Shadow.extent(blur_radius) as u32;
    let width = placement.width.saturating_add(padding * 2);
    let height = placement.height.saturating_add(padding * 2);
    let Some(area) = checked_area(width, height, 1) else {
      return;
    };
    let mut blurred = vec![0; area];

    for (row, source) in mask.chunks_exact(placement.width as usize).enumerate() {
      let start = (row + padding as usize) * width as usize + padding as usize;

      blurred[start..start + source.len()].copy_from_slice(source);
    }

    if apply_blur_alpha_bytes(&mut blurred, width, height, blur_radius, BlurType::Shadow).is_err() {
      return;
    }

    let placement = Placement {
      left: placement.left - padding as i32,
      top: placement.top - padding as i32,
      width,
      height,
    };

    self.draw_coverage((blurred, placement), color);
  }

  /// Paints the shadow the open `shadow` casts from `commands` drawn as `style` under `transform`.
  fn draw_shadow_of(
    &mut self,
    shadow: SizedShadow,
    commands: &[Command],
    style: Style,
    transform: Affine,
  ) {
    self.draw_blurred(
      commands,
      style,
      Affine::translation(shadow.offset_x, shadow.offset_y) * transform,
      shadow.blur_radius,
      shadow.color,
    );
  }

  /// Draws `run`'s glyphs, or their shadow while one is open.
  fn draw_glyphs(
    &mut self,
    run: &PositionedInlineRun,
    style: &SizedFontStyle,
    fill: GlyphFill,
    frame: BoxFrame,
  ) -> Result<()> {
    let glyph_run = &run.glyph_run;
    let local = run.transform(frame.translation());
    let offset = run.glyph_offset(frame.layout);
    // A span may set `-webkit-text-stroke` for itself, so it comes off the run.
    let stroke = (glyph_run.brush.stroke_width, glyph_run.brush.stroke_color);
    let placed = |glyph: &PositionedGlyph| Point {
      x: offset.x + glyph.x,
      y: offset.y + glyph.y,
    };

    if let Some(shadow) = self.shadow {
      for glyph in &glyph_run.glyphs {
        let at = placed(glyph);
        let transform = local * Affine::translation(at.x, at.y);
        let outline = match run.resolved_glyphs.get(&glyph.id).map(AsRef::as_ref) {
          Some(ResolvedGlyph::Outline(outline)) => outline,
          Some(ResolvedGlyph::Bitmap(bitmap)) => {
            if let Some(coverage) = bitmap_coverage(
              bitmap,
              self.transform * Affine::translation(shadow.offset_x, shadow.offset_y) * transform,
              self.shadow_viewport(shadow.blur_radius),
            ) {
              self.draw_blurred_coverage(coverage, shadow.blur_radius, shadow.color);
            }
            continue;
          }
          None => continue,
        };

        self.draw_shadow_of(shadow, outline.paths(), Fill::NonZero.into(), transform);

        if stroke.0 > 0.0 {
          let mut text_stroke = Stroke::new(stroke.0);

          text_stroke.join = style.parent.stroke_linejoin.into();
          self.draw_shadow_of(shadow, outline.paths(), text_stroke.into(), transform);
        }
      }

      return Ok(());
    }

    let transform = self.transform * local;

    if fill == GlyphFill::Background
      && let Some(background) = self.text_background
    {
      for glyph in &glyph_run.glyphs {
        if let Some(content) = run.resolved_glyphs.get(&glyph.id) {
          draw_glyph_clip_image(
            content,
            self.canvas,
            style,
            stroke,
            transform,
            placed(glyph),
            background,
          )?;
        }
      }
    }

    let font = FontRef::from_index(glyph_run.font_data(), glyph_run.font_index)
      .map_err(|_| FontError::InvalidFontIndex)?;
    let palettes = font.color_palettes();
    let palette = palettes.get(0);

    for glyph in &glyph_run.glyphs {
      if let Some(content) = run.resolved_glyphs.get(&glyph.id) {
        draw_glyph(
          content,
          self.canvas,
          style,
          stroke,
          transform,
          placed(glyph),
          glyph_run.brush.color,
          palette.as_ref(),
        )?;
      }
    }

    Ok(())
  }

  /// Fills `shape` with `source`, whose pixels `box_to_source` finds from the box's coordinates,
  /// sampled with `algorithm`.
  pub(crate) fn fill_shape_with_source(
    &mut self,
    shape: &FillShape,
    source: PaintSource<'_>,
    box_to_source: Affine,
    algorithm: ImageScalingAlgorithm,
  ) {
    let Some(canvas_to_box) = self.transform.invert() else {
      return;
    };
    let coverage = self.coverage(shape, Fill::from(shape.rule()).into(), Affine::IDENTITY);
    let Some((mask, placement)) = self.clipped(coverage) else {
      return;
    };

    self.canvas.composite_mask_source(
      &mask,
      placement,
      source,
      MaskCompositeColor::SourceOnly,
      MaskSamplingOptions {
        canvas_to_source: box_to_source * canvas_to_box,
        sample_bias: Point::ZERO,
        algorithm,
      },
      BlendMode::Normal,
    );
  }
}

impl PaintDevice for CanvasDevice<'_> {
  fn transform(&self) -> Affine {
    self.transform
  }

  fn fill_shape(&mut self, shape: &FillShape, color: Color, transform: Affine) {
    if let Some(shadow) = self.shadow {
      return self.draw_shadow_of(
        shadow,
        &shape.to_commands(),
        Fill::from(shape.rule()).into(),
        transform,
      );
    }

    let unclipped = self.clips.is_empty();
    let (border, size, offset) = match shape {
      FillShape::Rect(size) if unclipped && self.tiles_whole_pixels(*size, transform) => {
        (BorderProperties::default(), *size, Point::ZERO)
      }
      FillShape::RoundedRect {
        border,
        size,
        offset,
      } if unclipped => (*border, *size, *offset),
      _ => {
        let coverage = self.coverage(shape, Fill::from(shape.rule()).into(), transform);

        return self.draw_coverage(coverage, color);
      }
    };
    if size.width <= 0.0 || size.height <= 0.0 {
      return;
    }
    let tile = ColorTile::new(color, size.width as u32, size.height as u32);

    self.canvas.overlay_image(
      &BackgroundTile::Color(tile),
      border,
      self.transform * transform * Affine::translation(offset.x, offset.y),
      self.algorithm,
      BlendMode::Normal,
    );
  }

  fn stroke_shape(&mut self, shape: &FillShape, stroke: &StrokeStyle, transform: Affine) {
    if let Some(shadow) = self.shadow {
      return self.draw_shadow_of(
        shadow,
        &shape.to_commands(),
        Style::Stroke(stroke.into()),
        transform,
      );
    }

    let coverage = self.coverage(shape, Style::Stroke(stroke.into()), transform);

    self.draw_coverage(coverage, stroke.color);
  }

  fn push_clip(&mut self, shape: &FillShape, transform: Affine) {
    self.open_clip(shape, transform, false);
  }

  fn push_clip_out(&mut self, shape: &FillShape, transform: Affine) {
    self.open_clip(shape, transform, true);
  }

  fn with_border_mask(
    &mut self,
    border: &BorderProperties,
    size: Size<f32>,
    origin: Point<f32>,
    content: impl FnOnce(&mut Self),
  ) {
    let placement = self.canvas.viewport().placement();
    let subcanvas = match self.canvas.begin_subcanvas(placement) {
      Ok(subcanvas) => subcanvas,
      Err(error) => {
        self.error.get_or_insert(error);
        return;
      }
    };

    BoxBorderPainter::new(border, size).paint(origin, self);

    let painted = self.canvas.take_subcanvas(subcanvas);

    self.clips.push(CanvasClip {
      coverage: painted.data().iter().skip(3).step_by(4).copied().collect(),
      placement,
      out: false,
    });
    content(self);
    self.clips.pop();
  }

  fn pop_clip(&mut self) {
    self.clips.pop();
  }

  fn begin_layer(&mut self, opacity: f32) {
    let layer = match self
      .canvas
      .begin_subcanvas(self.canvas.viewport().placement())
    {
      Ok(subcanvas) => Some((subcanvas, opacity)),
      Err(error) => {
        self.error.get_or_insert(error);
        None
      }
    };

    self.layers.push(layer);
  }

  fn end_layer(&mut self) {
    if let Some(Some((subcanvas, opacity))) = self.layers.pop() {
      self
        .canvas
        .composite_subcanvas(subcanvas, BlendMode::Normal, opacity);
    }
  }

  fn fill_shadow(&mut self, shape: &ShadowShape, shadow: &SizedShadow, transform: Affine) {
    let fill = shape.fill_shape();

    self.draw_shadow_of(
      *shadow,
      &fill.to_commands(),
      Fill::from(fill.rule()).into(),
      transform,
    );
  }
}

impl GlyphDevice for CanvasDevice<'_> {
  fn begin_shadow(&mut self, shadow: &SizedShadow) {
    self.shadow = Some(*shadow);
  }

  fn end_shadow(&mut self) {
    self.shadow = None;
  }

  fn draw_glyph_run(
    &mut self,
    run: &PositionedInlineRun,
    style: &SizedFontStyle,
    fill: GlyphFill,
    frame: BoxFrame,
  ) {
    if let Err(error) = self.draw_glyphs(run, style, fill, frame) {
      self.error.get_or_insert(error);
    }
  }
}

pub(crate) fn draw_background(
  context: &RenderContext,
  canvas: &mut Canvas,
  layout: Layout,
) -> Result<()> {
  let painter = BoxPainter::new(context, layout);
  let background = painter.background();
  let mut device = CanvasDevice::of(canvas, context);

  // A blending layer mixes with the layers and color beneath it and nothing behind the box, so
  // the whole background composites in one tile, color included.
  let isolated = background
    .layers
    .iter()
    .any(|layer| layer.blend_mode != BlendMode::Normal);

  if !isolated && !matches!(background.clip, BackgroundClipArea::BorderArea(_)) {
    painter.background_color(Point::ZERO, &mut device);
  }

  match background.clip {
    BackgroundClipArea::BorderBox(border_radius) if isolated => {
      if let Some(tile) = rasterize_layers(
        collect_background_layers(&background, context)?,
        layout.size.map(|x| x as u32),
        context,
        BorderProperties::default(),
        Affine::IDENTITY,
      )? {
        canvas.overlay_image(
          &tile,
          border_radius,
          context.transform,
          context.style.image_rendering,
          BlendMode::Normal,
        );
      }
    }
    BackgroundClipArea::BorderBox(border_radius) => {
      let layers = background_image_layers(&background, context)?;

      if border_radius.is_zero() {
        for tile in layers {
          for y in &tile.ys {
            for x in &tile.xs {
              let transform = context.transform * Affine::translation(*x as f32, *y as f32);
              if transform.only_translation()
                && canvas.overlay_background_tile_direct(
                  &tile.tile,
                  Point {
                    x: transform.x,
                    y: transform.y,
                  },
                  tile.blend_mode,
                )
              {
                continue;
              }

              canvas.overlay_image(
                &tile.tile,
                border_radius,
                transform,
                context.style.image_rendering,
                tile.blend_mode,
              );
            }
          }
        }
      } else if let Some(layer) = single_solid_color_layer(&layers, canvas) {
        let transform = context.transform * Affine::translation(layer.x as f32, layer.y as f32);
        canvas.overlay_image(
          layer.tile,
          border_radius,
          transform,
          context.style.image_rendering,
          layer.blend_mode,
        );
      } else if let Some(tile) = rasterize_layers(
        layers,
        layout.size.map(|x| x as u32),
        context,
        BorderProperties::default(),
        Affine::IDENTITY,
      )? {
        canvas.overlay_image(
          &tile,
          border_radius,
          context.transform,
          context.style.image_rendering,
          BlendMode::Normal,
        );
      }
    }
    BackgroundClipArea::Inner(clip) => {
      let layers = if isolated {
        collect_background_layers(&background, context)?
      } else {
        background_image_layers(&background, context)?
      };

      draw_clipped_background(clip, layers, context, canvas)?;
    }
    BackgroundClipArea::BorderArea(_) => {
      let tile = rasterize_layers(
        collect_background_layers(&background, context)?,
        layout.size.map(|size| size as u32),
        context,
        BorderProperties::default(),
        Affine::IDENTITY,
      )?;

      if let Some(tile) = &tile
        && let Some(shape) = background.clip.shape(layout.size)
      {
        let algorithm = context.style.image_rendering;

        match background.clip.border_mask() {
          Some(mask) => device.with_border_mask(&mask, layout.size, Point::ZERO, |device| {
            device.fill_shape_with_source(
              &FillShape::Rect(layout.size),
              tile.into(),
              Affine::IDENTITY,
              algorithm,
            );
          }),
          None => device.fill_shape_with_source(&shape, tile.into(), Affine::IDENTITY, algorithm),
        }
      }
    }
    BackgroundClipArea::Text => {}
  }

  Ok(())
}

/// Rasterizes the background layers into `clip`'s rounded region and composites
/// it. Shared by the padding-box and content-box `background-clip` modes.
fn draw_clipped_background(
  clip: ClipBox,
  layers: TileLayers,
  context: &RenderContext,
  canvas: &mut Canvas,
) -> Result<()> {
  if let Some(tile) = rasterize_layers(
    layers,
    clip.size.map(|size| size as u32),
    context,
    clip.border,
    Affine::translation(-clip.offset.x, -clip.offset.y),
  )? {
    canvas.overlay_image(
      &tile,
      BorderProperties::default(),
      context.transform * Affine::translation(clip.offset.x, clip.offset.y),
      context.style.image_rendering,
      BlendMode::Normal,
    );
  }

  Ok(())
}

pub(crate) fn draw_border(
  context: &RenderContext,
  canvas: &mut Canvas,
  layout: Layout,
) -> Result<()> {
  CanvasDevice::paint(canvas, context, |device| {
    BoxPainter::new(context, layout).paint_border(Point::ZERO, device);
  })
}

/// A box's outline and the device state it paints with, kept until the box's children are done.
pub(crate) struct DeferredOutline {
  outline: PendingOutline,
  transform: Affine,
  algorithm: ImageScalingAlgorithm,
}

impl DeferredOutline {
  /// The outline of the box `context` paints at `layout`, or `None` when it paints none.
  pub(crate) fn of(context: &RenderContext, layout: Layout) -> Option<Self> {
    Some(Self {
      outline: BoxPainter::new(context, layout).pending_outline(Point::ZERO)?,
      transform: context.transform,
      algorithm: context.style.image_rendering,
    })
  }

  pub(crate) fn paint(&self, canvas: &mut Canvas) -> Result<()> {
    let mut device = CanvasDevice::new(canvas, self.transform, self.algorithm);

    self.outline.paint(&mut device);
    device.finish()
  }
}

struct SolidColorLayer<'a> {
  tile: &'a BackgroundTile,
  x: i32,
  y: i32,
  blend_mode: BlendMode,
}

fn single_solid_color_layer<'a>(
  layers: &'a [TileLayer],
  canvas: &Canvas,
) -> Option<SolidColorLayer<'a>> {
  if !canvas.has_no_constraint_mask() {
    return None;
  }
  let [layer] = layers else {
    return None;
  };
  if !matches!(layer.tile, BackgroundTile::Color(_)) {
    return None;
  }
  if layer.xs.len() != 1 || layer.ys.len() != 1 {
    return None;
  }
  Some(SolidColorLayer {
    tile: &layer.tile,
    x: layer.xs[0],
    y: layer.ys[0],
    blend_mode: layer.blend_mode,
  })
}

pub(crate) fn draw_image_node_content(
  image: &ImageData,
  context: &RenderContext,
  canvas: &mut Canvas,
  layout: Layout,
) -> Result<()> {
  let Ok(image_source) = image.src.resolve(context) else {
    return Ok(());
  };

  draw_image(&image_source, context, canvas, layout)
}

#[cfg(test)]
mod tests {
  use takumi_core::{
    geometry::{Point, Rect, Size},
    layout::border::BorderProperties,
    painter::{BoxBorderPainter, StrokeStyle},
    style::{Affine, BorderStyle, Color, ImageScalingAlgorithm, Sides, SpacePair},
  };

  use super::CanvasDevice;
  use crate::{Canvas, Cap, Stroke};

  fn paint_border(
    border: BorderProperties,
    canvas: &mut Canvas,
    size: Size<f32>,
    transform: Affine,
  ) {
    let mut device = CanvasDevice::new(canvas, transform, ImageScalingAlgorithm::Auto);

    BoxBorderPainter::new(&border, size).paint(Point::ZERO, &mut device);
  }

  fn test_border(style: BorderStyle, width: f32) -> BorderProperties {
    BorderProperties {
      width: Rect {
        top: width,
        right: width,
        bottom: width,
        left: width,
      },
      color: Rect {
        top: Color([255, 0, 0, 255]),
        right: Color([255, 0, 0, 255]),
        bottom: Color([255, 0, 0, 255]),
        left: Color([255, 0, 0, 255]),
      },
      radius: Sides([SpacePair::from_single(0.0); 4]),
      style: Rect {
        top: style,
        right: style,
        bottom: style,
        left: style,
      },
      image_rendering: ImageScalingAlgorithm::Auto,
      collapsed: false,
      shape: Sides::default(),
    }
  }

  #[test]
  fn solid_border_draws_continuous_edge() {
    let mut canvas = Canvas::new(Size {
      width: 48,
      height: 48,
    });

    paint_border(
      test_border(BorderStyle::Solid, 4.0),
      &mut canvas,
      Size {
        width: 48.0,
        height: 48.0,
      },
      Affine::IDENTITY,
    );

    let image = canvas
      .into_inner()
      .unwrap_or_else(|error| unreachable!("test canvas should be readable: {error}"));
    assert!((8..40).all(|x| image.get_pixel(x, 2).0[3] > 0));
  }

  #[test]
  fn hidden_border_does_not_draw() {
    let mut canvas = Canvas::new(Size {
      width: 24,
      height: 24,
    });

    paint_border(
      test_border(BorderStyle::Hidden, 4.0),
      &mut canvas,
      Size {
        width: 24.0,
        height: 24.0,
      },
      Affine::IDENTITY,
    );

    let image = canvas
      .into_inner()
      .unwrap_or_else(|error| unreachable!("test canvas should be readable: {error}"));
    assert!(image.pixels().all(|pixel| pixel.0[3] == 0));
  }

  #[test]
  fn dashed_border_draws_pattern() {
    let mut canvas = Canvas::new(Size {
      width: 48,
      height: 48,
    });

    paint_border(
      test_border(BorderStyle::Dashed, 4.0),
      &mut canvas,
      Size {
        width: 48.0,
        height: 48.0,
      },
      Affine::IDENTITY,
    );

    let image = canvas
      .into_inner()
      .unwrap_or_else(|error| unreachable!("test canvas should be readable: {error}"));

    let row: Vec<u8> = (0..48).map(|x| image.get_pixel(x, 2).0[3]).collect();
    let has_opaque = row.iter().any(|&a| a > 0);
    let has_transparent = row.iter().skip(8).take(32).any(|&a| a == 0);

    assert!(has_opaque, "Dashed border should have opaque pixels");
    assert!(
      has_transparent,
      "Dashed border should have transparent gaps"
    );
  }

  #[test]
  fn dotted_border_draws_pattern() {
    let mut canvas = Canvas::new(Size {
      width: 48,
      height: 48,
    });

    paint_border(
      test_border(BorderStyle::Dotted, 4.0),
      &mut canvas,
      Size {
        width: 48.0,
        height: 48.0,
      },
      Affine::IDENTITY,
    );

    let image = canvas
      .into_inner()
      .unwrap_or_else(|error| unreachable!("test canvas should be readable: {error}"));

    let row: Vec<u8> = (0..48).map(|x| image.get_pixel(x, 2).0[3]).collect();
    let has_opaque = row.iter().any(|&a| a > 0);
    let has_transparent = row.iter().skip(8).take(32).any(|&a| a == 0);

    assert!(has_opaque, "Dotted border should have opaque pixels");
    assert!(
      has_transparent,
      "Dotted border should have transparent gaps"
    );
  }

  #[test]
  fn thin_dotted_border_draws_square_dots() {
    let stroke = Stroke::from(&StrokeStyle::border(
      Color::black(),
      2.0,
      BorderStyle::Dotted.dash_pattern(2.0, 48.0, false),
    ));
    let Some(dash_pattern) = stroke.dash else {
      unreachable!("thin dotted stroke should produce a dash pattern");
    };

    assert_eq!(stroke.cap, Cap::Butt);
    assert_eq!(dash_pattern.intervals, [2.0, 2.0]);
  }

  #[test]
  fn dashed_border_top_only_draws_pattern() {
    let mut canvas = Canvas::new(Size {
      width: 48,
      height: 48,
    });
    let mut border = test_border(BorderStyle::Dashed, 0.0);
    border.width.top = 4.0;

    paint_border(
      border,
      &mut canvas,
      Size {
        width: 48.0,
        height: 48.0,
      },
      Affine::IDENTITY,
    );

    let image = canvas
      .into_inner()
      .unwrap_or_else(|error| unreachable!("test canvas should be readable: {error}"));
    let top_row: Vec<u8> = (8..40).map(|x| image.get_pixel(x, 2).0[3]).collect();

    assert!(
      top_row.iter().any(|&alpha| alpha > 0),
      "Top dashed side should contain opaque pixels"
    );
    assert!(
      top_row.contains(&0),
      "Top dashed side should contain transparent gaps"
    );
    assert_eq!(
      image.get_pixel(24, 45).0[3],
      0,
      "Bottom side should stay transparent for top-only dashed border"
    );
    assert_eq!(
      image.get_pixel(2, 24).0[3],
      0,
      "Left side should stay transparent for top-only dashed border"
    );
    assert_eq!(
      image.get_pixel(45, 24).0[3],
      0,
      "Right side should stay transparent for top-only dashed border"
    );
  }

  #[test]
  fn dotted_border_left_only_draws_pattern() {
    let mut canvas = Canvas::new(Size {
      width: 48,
      height: 48,
    });
    let mut border = test_border(BorderStyle::Dotted, 0.0);
    border.width.left = 4.0;

    paint_border(
      border,
      &mut canvas,
      Size {
        width: 48.0,
        height: 48.0,
      },
      Affine::IDENTITY,
    );

    let image = canvas
      .into_inner()
      .unwrap_or_else(|error| unreachable!("test canvas should be readable: {error}"));
    let left_column: Vec<u8> = (8..40).map(|y| image.get_pixel(2, y).0[3]).collect();

    assert!(
      left_column.iter().any(|&alpha| alpha > 0),
      "Left dotted side should contain opaque pixels"
    );
    assert!(
      left_column.contains(&0),
      "Left dotted side should contain transparent gaps"
    );
    assert_eq!(
      image.get_pixel(24, 2).0[3],
      0,
      "Top side should stay transparent for left-only dotted border"
    );
    assert_eq!(
      image.get_pixel(45, 24).0[3],
      0,
      "Right side should stay transparent for left-only dotted border"
    );
    assert_eq!(
      image.get_pixel(24, 45).0[3],
      0,
      "Bottom side should stay transparent for left-only dotted border"
    );
  }

  #[test]
  fn solid_fast_path_skips_hidden_side_with_positive_width() {
    let mut canvas = Canvas::new(Size {
      width: 48,
      height: 48,
    });
    let mut border = test_border(BorderStyle::Solid, 4.0);
    border.style.top = BorderStyle::Hidden;

    paint_border(
      border,
      &mut canvas,
      Size {
        width: 48.0,
        height: 48.0,
      },
      Affine::IDENTITY,
    );

    let image = canvas
      .into_inner()
      .unwrap_or_else(|error| unreachable!("test canvas should be readable: {error}"));

    assert_eq!(
      image.get_pixel(24, 2).0[3],
      0,
      "Hidden top side should stay transparent"
    );
    let right_band_has_ink = (44..48).any(|x| image.get_pixel(x, 24).0[3] > 0);
    assert!(
      right_band_has_ink,
      "Visible right side should still be painted"
    );
  }

  #[test]
  fn double_fast_path_skips_hidden_side_with_positive_width() {
    let mut canvas = Canvas::new(Size {
      width: 48,
      height: 48,
    });
    let mut border = test_border(BorderStyle::Double, 6.0);
    border.style.top = BorderStyle::Hidden;

    paint_border(
      border,
      &mut canvas,
      Size {
        width: 48.0,
        height: 48.0,
      },
      Affine::IDENTITY,
    );

    let image = canvas
      .into_inner()
      .unwrap_or_else(|error| unreachable!("test canvas should be readable: {error}"));

    assert_eq!(
      image.get_pixel(24, 2).0[3],
      0,
      "Hidden top side should stay transparent"
    );
    let right_band_has_ink = (42..48).any(|x| image.get_pixel(x, 24).0[3] > 0);
    assert!(
      right_band_has_ink,
      "Visible right side should still be painted"
    );
  }

  #[test]
  fn solid_fallback_ignores_hidden_neighbor_widths() {
    let mut canvas = Canvas::new(Size {
      width: 64,
      height: 64,
    });
    let mut border = test_border(BorderStyle::Hidden, 0.0);
    border.style.top = BorderStyle::Solid;
    border.width.top = 8.0;
    border.style.right = BorderStyle::Dashed;
    border.width.right = 8.0;
    border.width.left = 24.0;

    paint_border(
      border,
      &mut canvas,
      Size {
        width: 64.0,
        height: 64.0,
      },
      Affine::IDENTITY,
    );

    let image = canvas
      .into_inner()
      .unwrap_or_else(|error| unreachable!("test canvas should be readable: {error}"));

    assert!(
      image.get_pixel(4, 3).0[3] > 0,
      "Visible top side should not be clipped by hidden left width"
    );
    assert_eq!(
      image.get_pixel(3, 32).0[3],
      0,
      "Hidden left side should stay transparent"
    );
  }

  #[test]
  fn oversized_solid_border_fills_without_panicking() {
    let mut canvas = Canvas::new(Size {
      width: 20,
      height: 20,
    });
    let border = test_border(BorderStyle::Solid, 40.0);

    paint_border(
      border,
      &mut canvas,
      Size {
        width: 20.0,
        height: 20.0,
      },
      Affine::IDENTITY,
    );

    let image = canvas
      .into_inner()
      .unwrap_or_else(|error| unreachable!("test canvas should be readable: {error}"));

    assert!(
      image.get_pixel(10, 10).0[3] > 0,
      "Oversized border should still render a valid filled mask"
    );
  }
}
