//! Raster box-decoration painting (backgrounds, borders, outlines, box-shadows).
//!
//! The backend-agnostic geometry — clip regions and the outline ring — lives in
//! [`takumi_core::layout::decoration`]; these functions composite it with
//! tiny-skia, and the SVG backend emits the same geometry as vector paths.

use takumi_core::{
  geometry::{ComputedLayout as Layout, Point, Point as CorePoint},
  layout::decoration::{ClipBox, OutlineGeometry},
  painter::{BoxPainter, FillShape, PaintDevice},
  style::{Color, ImageScalingAlgorithm},
};

use super::{
  BackgroundTile, BorderProperties, Canvas, ColorTile, Fill, PaintSource, RenderContext,
  SizedFontStyle, TileLayer, background_image_layers, collect_background_layers, draw_image,
  draw_inset_shadow_to_canvas, draw_outset_shadow, inline_drawing::draw_inline_layout,
  paint_border, rasterize_layers,
};
use crate::{
  Result,
  layout::{
    inline::{InlineItem, InlineLayoutMode, InlineLayoutRequest, create_inline_layout},
    node::{ImageData, Node, NodeKind, TextData},
  },
  style::{Affine, BackgroundClip, BlendMode},
};

pub(crate) fn draw_outset_box_shadow(
  context: &RenderContext,
  canvas: &mut Canvas,
  layout: Layout,
) -> Result<()> {
  let painter = BoxPainter::new(context, layout);
  let element_border_radius = *painter.border();

  for shadow in painter.shadows().outer {
    let mut paths = Vec::new();
    let mut element_paths = Vec::new();
    let resolved_spread_radius = shadow.spread_radius;
    let (border_radius, spread_size) =
      element_border_radius.outset_shadow_box(layout.size, resolved_spread_radius);

    border_radius.append_mask_commands(
      &mut paths,
      spread_size,
      Point {
        x: -resolved_spread_radius,
        y: -resolved_spread_radius,
      },
    );

    element_border_radius.append_mask_commands(&mut element_paths, layout.size, Point::ZERO);

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
  {
    let painter = BoxPainter::new(context, layout);
    let border_radius = *painter.border();

    for shadow in painter.shadows().inset {
      draw_inset_shadow_to_canvas(&shadow, context.transform, border_radius, canvas, layout)?;
    }
  }
  Ok(())
}

/// The canvas as a [`PaintDevice`]. A rounded rectangle composites through the
/// same border machinery the tile path uses, so nothing rasterizes a path that
/// did not before.
pub(crate) struct CanvasDevice<'c> {
  pub(crate) canvas: &'c mut Canvas,
  pub(crate) transform: Affine,
  pub(crate) algorithm: ImageScalingAlgorithm,
}

impl PaintDevice for CanvasDevice<'_> {
  fn fill_shape(&mut self, shape: &FillShape, color: Color, transform: Affine) {
    let (border, size, offset) = match shape {
      FillShape::Rect(size) => (BorderProperties::default(), *size, CorePoint::ZERO),
      FillShape::RoundedRect {
        border,
        size,
        offset,
      } => (*border, *size, *offset),
      // A path that is not a rectangle never reaches a background colour.
      FillShape::Path { .. } => return,
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
}

pub(crate) fn draw_background(
  context: &RenderContext,
  canvas: &mut Canvas,
  layout: Layout,
) -> Result<()> {
  let border_radius = BorderProperties::from_context(context, layout.size, layout.border);
  let mut device = CanvasDevice {
    canvas,
    transform: context.transform,
    algorithm: context.style.image_rendering,
  };

  BoxPainter::new(context, layout).background_color(Point { x: 0.0, y: 0.0 }, &mut device);

  match context.style.background_clip {
    BackgroundClip::BorderBox => {
      let layers = background_image_layers(context, layout)?;

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
    BackgroundClip::PaddingBox => {
      draw_clipped_background(
        ClipBox::padding_box(border_radius, layout),
        context,
        canvas,
        layout,
      )?;
    }
    BackgroundClip::ContentBox => {
      draw_clipped_background(
        ClipBox::content_box(border_radius, layout),
        context,
        canvas,
        layout,
      )?;
    }
    // Filling the border's own shape with the layers is the clip `border-area`
    // asks for. The border then paints over it, as it does in Blink.
    BackgroundClip::BorderArea => {
      let layers = rasterize_layers(
        collect_background_layers(context, layout)?,
        layout.size.map(|size| size as u32),
        context,
        BorderProperties::default(),
        Affine::IDENTITY,
      )?;

      paint_border(
        border_radius,
        canvas,
        layout.size,
        context.transform,
        layers.as_ref().map(PaintSource::from),
      );
    }
    _ => {}
  }

  Ok(())
}

/// Rasterizes the background layers into `clip`'s rounded region and composites
/// it. Shared by the padding-box and content-box `background-clip` modes.
fn draw_clipped_background(
  clip: ClipBox,
  context: &RenderContext,
  canvas: &mut Canvas,
  layout: Layout,
) -> Result<()> {
  let layers = background_image_layers(context, layout)?;

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
  paint_border(
    BorderProperties::from_context(context, layout.size, layout.border),
    canvas,
    layout.size,
    context.transform,
    None,
  );

  Ok(())
}

/// The outline a box paints, resolved against its layout so nothing but the
/// geometry has to survive until the box's children are done.
pub(crate) fn resolve_outline(
  context: &RenderContext,
  layout: Layout,
) -> Option<(OutlineGeometry, Affine)> {
  let outline = BoxPainter::new(context, layout).outline()?;
  let transform = Affine::translation(-outline.grow, -outline.grow) * context.transform;

  Some((outline, transform))
}

pub(crate) fn draw_outline(outline: &OutlineGeometry, transform: Affine, canvas: &mut Canvas) {
  paint_border(outline.border, canvas, outline.size, transform, None);
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

  draw_image(&image_source, context, canvas, layout)?;
  Ok(())
}

fn draw_text_node_content(
  text: &TextData,
  context: &RenderContext,
  canvas: &mut Canvas,
  layout: Layout,
) -> Result<()> {
  let font_style = SizedFontStyle::from_style(&context.style, context);
  let size = layout.content_box_size();

  if font_style.sizing.font_size == 0.0 {
    return Ok(());
  }

  let inline_text: InlineItem<'_> = InlineItem::Text {
    text: text.text.as_str().into(),
    context,
    link: None,
  };

  let built = create_inline_layout(InlineLayoutRequest::in_content_box(
    vec![inline_text],
    size,
    &font_style,
    context,
    InlineLayoutMode::Draw,
  ));

  draw_inline_layout(context, canvas, layout, &built, &font_style)?;

  Ok(())
}
