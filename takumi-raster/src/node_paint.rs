//! Raster box-decoration painting (backgrounds, borders, outlines, box-shadows).
//!
//! The backend-agnostic geometry — clip regions and the outline ring — lives in
//! [`takumi_core::layout::decoration`]; these functions composite it with
//! tiny-skia, and the SVG backend emits the same geometry as vector paths.

use takumi_core::{
  geometry::{AvailableSpace, ComputedLayout as Layout, Point, Size},
  layout::decoration::{ClipBox, outline_geometry},
};

use super::{
  BackgroundTile, BorderProperties, Canvas, Fill, PaintSource, RenderContext, SizedFontStyle,
  SizedShadow, TileLayer, collect_background_layers, draw_image, draw_inset_shadow_to_canvas,
  draw_outset_shadow, inline_drawing::draw_inline_layout, paint_border, rasterize_layers,
  release_rasterized_background_tile,
};
use crate::{
  Result,
  layout::{
    inline::{
      InlineItem, InlineLayoutMode, InlineLayoutRequest, create_inline_layout,
      resolve_inline_max_height,
    },
    node::{ImageData, Node, NodeKind, TextData},
  },
  style::{Affine, BackgroundClip, BlendMode},
};

pub(crate) fn draw_outset_box_shadow(
  context: &RenderContext,
  canvas: &mut Canvas,
  layout: Layout,
) -> Result<()> {
  let Some(box_shadow) = context.style.box_shadow.as_ref() else {
    return Ok(());
  };

  let element_border_radius = BorderProperties::from_context(context, layout.size, layout.border);

  for shadow in box_shadow.iter() {
    if shadow.inset {
      continue;
    }

    let mut paths = Vec::new();
    let mut element_paths = Vec::new();

    let resolved_spread_radius = shadow
      .spread_radius
      .to_px(&context.sizing, layout.size.width);

    let (border_radius, spread_size) =
      element_border_radius.outset_shadow_box(layout.size, resolved_spread_radius);

    let shadow =
      SizedShadow::from_box_shadow(*shadow, &context.sizing, context.current_color, layout.size);

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
  if let Some(box_shadow) = context.style.box_shadow.as_ref() {
    let border_radius = BorderProperties::from_context(context, layout.size, layout.border);

    for shadow in box_shadow.iter() {
      if !shadow.inset {
        continue;
      }

      let shadow =
        SizedShadow::from_box_shadow(*shadow, &context.sizing, context.current_color, layout.size);
      draw_inset_shadow_to_canvas(&shadow, context.transform, border_radius, canvas, layout)?;
    }
  }
  Ok(())
}

pub(crate) fn draw_background(
  context: &RenderContext,
  canvas: &mut Canvas,
  layout: Layout,
) -> Result<()> {
  let border_radius = BorderProperties::from_context(context, layout.size, layout.border);

  match context.style.background_clip {
    BackgroundClip::BorderBox => {
      let layers = collect_background_layers(context, layout, &mut canvas.buffer_pool)?;

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
        &mut canvas.buffer_pool,
      )? {
        canvas.overlay_image(
          &tile,
          border_radius,
          context.transform,
          context.style.image_rendering,
          BlendMode::Normal,
        );

        release_rasterized_background_tile(tile, &mut canvas.buffer_pool);
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
  let layers = collect_background_layers(context, layout, &mut canvas.buffer_pool)?;

  if let Some(tile) = rasterize_layers(
    layers,
    clip.size.map(|size| size as u32),
    context,
    clip.border,
    Affine::translation(-clip.offset.x, -clip.offset.y),
    &mut canvas.buffer_pool,
  )? {
    canvas.overlay_image(
      &tile,
      BorderProperties::default(),
      context.transform * Affine::translation(clip.offset.x, clip.offset.y),
      context.style.image_rendering,
      BlendMode::Normal,
    );

    release_rasterized_background_tile(tile, &mut canvas.buffer_pool);
  }

  Ok(())
}

pub(crate) fn draw_border(
  context: &RenderContext,
  canvas: &mut Canvas,
  layout: Layout,
) -> Result<()> {
  let clip_image = if context.style.background_clip == BackgroundClip::BorderArea {
    rasterize_layers(
      collect_background_layers(context, layout, &mut canvas.buffer_pool)?,
      layout.size.map(|x| x as u32),
      context,
      BorderProperties::default(),
      Affine::IDENTITY,
      &mut canvas.buffer_pool,
    )?
  } else {
    None
  };

  paint_border(
    BorderProperties::from_context(context, layout.size, layout.border),
    canvas,
    layout.size,
    context.transform,
    clip_image.as_ref().map(PaintSource::from),
  );

  if let Some(tile) = clip_image {
    release_rasterized_background_tile(tile, &mut canvas.buffer_pool);
  }
  Ok(())
}

pub(crate) fn draw_outline(
  context: &RenderContext,
  canvas: &mut Canvas,
  layout: Layout,
) -> Result<()> {
  let outline = outline_geometry(context, layout.size);
  let transform = Affine::translation(-outline.grow, -outline.grow) * context.transform;

  paint_border(outline.border, canvas, outline.size, transform, None);

  Ok(())
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

  let max_height = resolve_inline_max_height(&font_style, size.height);

  let inline_text: InlineItem<'_> = InlineItem::Text {
    text: text.text.as_str().into(),
    context,
  };

  let built = create_inline_layout(InlineLayoutRequest {
    items: vec![inline_text],
    available_space: Size {
      width: AvailableSpace::Definite(size.width),
      height: AvailableSpace::Definite(size.height),
    },
    max_width: size.width,
    max_height,
    style: &font_style,
    context,
    mode: InlineLayoutMode::Draw,
  });

  draw_inline_layout(context, canvas, layout, &built, &font_style)?;

  Ok(())
}
