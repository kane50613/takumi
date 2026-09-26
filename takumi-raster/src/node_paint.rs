//! Raster box-decoration painting (backgrounds, borders, outlines, box-shadows).
//!
//! The backend-agnostic geometry — clip regions and the outline ring — lives in
//! [`takumi_core::layout::decoration`]; these functions composite it with
//! tiny-skia, and the SVG backend emits the same geometry as vector paths.

use takumi_core::{
  geometry::{ComputedLayout as Layout, Point},
  layout::decoration::{ClipBox, OutlineGeometry},
  painter::{
    BackgroundClipArea, BoxBorderPainter, BoxPainter, FillShape, PaintDevice, StrokeStyle,
  },
  style::{Color, ImageScalingAlgorithm},
};

use super::{
  BackgroundTile, BorderProperties, Canvas, ColorTile, Fill, PaintSource, RenderContext,
  SizedFontStyle, TileLayer, TileLayers, background_image_layers, collect_background_layers,
  draw_image, draw_inset_shadow_to_canvas, draw_outset_shadow, inline_drawing::draw_inline_layout,
  rasterize_layers,
};
use crate::{
  MaskCompositeColor, MaskSamplingOptions, Placement, Result, Style, intersect_alpha_masks,
  layout::{
    inline::{InlineItem, InlineLayoutMode, InlineLayoutRequest, create_inline_layout},
    node::{ImageData, Node, NodeKind, TextData},
  },
  render_mask,
  style::{Affine, BlendMode},
};

pub(crate) fn draw_outset_box_shadow(
  context: &RenderContext,
  canvas: &mut Canvas,
  layout: Layout,
) -> Result<()> {
  let painter = BoxPainter::new(context, layout);
  let shadows = painter.shadows().outer;

  if shadows.is_empty() {
    return Ok(());
  }

  let element_border_radius = *painter.border();
  let mut element_paths = Vec::new();

  element_border_radius.append_mask_commands(&mut element_paths, layout.size, Point::ZERO);

  for shadow in shadows {
    let mut paths = Vec::new();
    let (border_radius, spread_size) =
      element_border_radius.outset_shadow_box(layout.size, shadow.spread_radius);

    border_radius.append_mask_commands(
      &mut paths,
      spread_size,
      Point {
        x: -shadow.spread_radius,
        y: -shadow.spread_radius,
      },
    );

    draw_outset_shadow(
      &shadow,
      canvas,
      &paths,
      context.transform,
      Fill::NonZero.into(),
      Some(&element_paths),
    )?;
  }

  Ok(())
}

pub(crate) fn draw_inset_box_shadow(
  context: &RenderContext,
  canvas: &mut Canvas,
  layout: Layout,
) -> Result<()> {
  let painter = BoxPainter::new(context, layout);
  let border_radius = *painter.border();

  for shadow in painter.shadows().inset {
    draw_inset_shadow_to_canvas(&shadow, context.transform, border_radius, canvas, layout)?;
  }

  Ok(())
}

/// Paints a box's own decorations, bottom to top: outset shadows, background,
/// inset shadows, and border.
pub(crate) fn draw_box_shell(
  context: &RenderContext,
  canvas: &mut Canvas,
  layout: Layout,
) -> Result<()> {
  draw_outset_box_shadow(context, canvas, layout)?;
  draw_background(context, canvas, layout)?;
  draw_inset_box_shadow(context, canvas, layout)?;
  draw_border(context, canvas, layout)
}

/// The canvas as a [`PaintDevice`]. An unclipped rounded rectangle composites
/// through the same border machinery the tile path uses, so a background colour
/// rasterizes as it always has.
pub(crate) struct CanvasDevice<'c> {
  pub(crate) canvas: &'c mut Canvas,
  pub(crate) transform: Affine,
  pub(crate) algorithm: ImageScalingAlgorithm,
  /// The coverage each open [`PaintDevice::save`] clips to, if it clips.
  pub(crate) clips: Vec<Option<(Vec<u8>, Placement)>>,
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
    }
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
      .flatten()
      .try_fold(coverage, |(mask, placement), (clip, clip_placement)| {
        intersect_alpha_masks(&mask, placement, clip, *clip_placement)
      })
  }

  /// Paints `coverage` in `color`, limited to the open clips.
  fn draw_coverage(&mut self, coverage: (Vec<u8>, Placement), color: Color) {
    if let Some((mask, placement)) = self.clipped(coverage) {
      self
        .canvas
        .draw_mask(&mask, placement, color, BlendMode::Normal);
    }
  }

  /// Fills `shape` with `source`, an image laid over the box at the device transform.
  pub(crate) fn fill_shape_with_source(&mut self, shape: &FillShape, source: PaintSource<'_>) {
    let Some(canvas_to_source) = self.transform.invert() else {
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
        canvas_to_source,
        sample_bias: Point::ZERO,
        algorithm: self.algorithm,
      },
      BlendMode::Normal,
    );
  }
}

impl PaintDevice for CanvasDevice<'_> {
  fn fill_shape(&mut self, shape: &FillShape, color: Color, transform: Affine) {
    let unclipped = self.clips.iter().all(Option::is_none);
    let (border, size, offset) = match shape {
      FillShape::Rect(size) if unclipped => (BorderProperties::default(), *size, Point::ZERO),
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
    let coverage = self.coverage(shape, Style::Stroke(stroke.into()), transform);

    self.draw_coverage(coverage, stroke.color);
  }

  fn save(&mut self, clip: Option<(&FillShape, Affine)>) {
    let clip = clip
      .map(|(shape, transform)| self.coverage(shape, Fill::from(shape.rule()).into(), transform));

    self.clips.push(clip);
  }

  fn restore(&mut self) {
    self.clips.pop();
  }
}

pub(crate) fn draw_background(
  context: &RenderContext,
  canvas: &mut Canvas,
  layout: Layout,
) -> Result<()> {
  let painter = BoxPainter::new(context, layout);
  let background = painter.background();
  let mut device = CanvasDevice::new(canvas, context.transform, context.style.image_rendering);

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
        device.fill_shape_with_source(&shape, tile.into());
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
  let painter = BoxPainter::new(context, layout);
  let mut device = CanvasDevice::new(canvas, context.transform, context.style.image_rendering);

  BoxBorderPainter::new(painter.border(), layout.size).paint(Point::ZERO, &mut device);

  Ok(())
}

/// The outline a box paints, resolved against its layout so nothing but the
/// geometry has to survive until the box's children are done.
pub(crate) fn resolve_outline(
  context: &RenderContext,
  layout: Layout,
) -> Option<(OutlineGeometry, Affine)> {
  let outline = BoxPainter::new(context, layout).outline()?;
  let transform = context.transform * Affine::translation(-outline.grow, -outline.grow);

  Some((outline, transform))
}

pub(crate) fn draw_outline(outline: &OutlineGeometry, transform: Affine, canvas: &mut Canvas) {
  let mut device = CanvasDevice::new(canvas, transform, outline.border.image_rendering);

  BoxBorderPainter::new(&outline.border, outline.size).paint(Point::ZERO, &mut device);
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

pub(crate) fn draw_node_content(
  node: &Node,
  context: &RenderContext,
  canvas: &mut Canvas,
  layout: Layout,
) -> Result<()> {
  match &node.kind {
    NodeKind::Container { .. } => Ok(()),
    NodeKind::Image(image) => draw_image_node_content(image, context, canvas, layout),
    NodeKind::Text(text) => draw_text_node_content(text, context, canvas, layout),
    _ => Ok(()),
  }
}

fn draw_image_node_content(
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

fn draw_text_node_content(
  text: &TextData,
  context: &RenderContext,
  canvas: &mut Canvas,
  layout: Layout,
) -> Result<()> {
  let font_style = SizedFontStyle::from_style(&context.style, context);

  if font_style.sizing.font_size == 0.0 {
    return Ok(());
  }

  let inline_text = InlineItem::Text {
    text: text.text.as_str().into(),
    context,
    link: None,
    decorations: None,
  };

  let built = create_inline_layout(InlineLayoutRequest::in_content_box(
    vec![inline_text],
    layout.unsnapped_content,
    &font_style,
    context,
    InlineLayoutMode::Draw,
  ));

  draw_inline_layout(context, canvas, layout, &built, &font_style)?;

  Ok(())
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
  fn dotted_border_thin_width_uses_zero_dash_length() {
    let stroke = Stroke::from(&StrokeStyle::border(
      Color::black(),
      2.0,
      BorderStyle::Dotted.dash_pattern(2.0, 48.0, false),
    ));
    let Some(dash_pattern) = stroke.dash else {
      unreachable!("thin dotted stroke should produce a dash pattern");
    };
    assert_eq!(stroke.cap, Cap::Round);
    assert_eq!(dash_pattern.intervals[0], 0.0);
    assert!(dash_pattern.intervals[1] > 0.0);
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
