//! Box-decoration painting (backgrounds, borders, outlines, box-shadows).
//!
//! These functions depend only on the resolved [`RenderContext`] and computed
//! [`Layout`], not on the node tree. They are candidates for promotion to a
//! backend-agnostic scene layer when an alternative (e.g. SVG) backend lands.

use taffy::{AvailableSpace, Layout, Point, Size};

use crate::{
  Result,
  layout::{
    inline::{
      InlineItem, InlineLayoutMode, InlineLayoutRequest, create_inline_layout,
      resolve_inline_max_height,
    },
    node::{ImageData, Node, NodeKind, TextData},
    style::{Affine, BackgroundClip, BlendMode, Sides},
  },
};

use super::{
  BackgroundTile, BorderProperties, Canvas, Fill, PaintSource, RenderContext, SizedFontStyle,
  SizedShadow, TileLayer, collect_background_layers, draw_image, draw_inset_shadow_to_canvas,
  draw_outset_shadow,
  inline_drawing::{InlineLayoutDrawData, draw_inline_layout},
  rasterize_layers, release_rasterized_background_tile,
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

    let mut border_radius = element_border_radius;
    let resolved_spread_radius = shadow
      .spread_radius
      .to_px(&context.sizing, layout.size.width);

    border_radius.expand_by(Sides([resolved_spread_radius; 4]).into());

    let shadow =
      SizedShadow::from_box_shadow(*shadow, &context.sizing, context.current_color, layout.size);

    let spread_size = Size {
      width: (layout.size.width + 2.0 * resolved_spread_radius).max(0.0),
      height: (layout.size.height + 2.0 * resolved_spread_radius).max(0.0),
    };

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
  let mut border_radius = BorderProperties::from_context(context, layout.size, layout.border);

  match context.style.background_clip {
    BackgroundClip::BorderBox => {
      let layers = collect_background_layers(context, layout.size, &mut canvas.buffer_pool)?;

      if border_radius.is_zero() {
        for tile in layers {
          for y in &tile.ys {
            for x in &tile.xs {
              let transform = context.transform * Affine::translation(*x as f32, *y as f32);
              if transform.only_translation()
                && canvas.overlay_background_tile_direct(
                  &tile.tile,
                  transform.decompose_translation(),
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
      border_radius.inset_by_border_width();

      let layers = collect_background_layers(context, layout.size, &mut canvas.buffer_pool)?;

      if let Some(tile) = rasterize_layers(
        layers,
        Size {
          width: (layout.size.width - layout.border.left - layout.border.right) as u32,
          height: (layout.size.height - layout.border.top - layout.border.bottom) as u32,
        },
        context,
        border_radius,
        Affine::translation(-layout.border.left, -layout.border.top),
        &mut canvas.buffer_pool,
      )? {
        canvas.overlay_image(
          &tile,
          BorderProperties::default(),
          context.transform * Affine::translation(layout.border.left, layout.border.top),
          context.style.image_rendering,
          BlendMode::Normal,
        );

        release_rasterized_background_tile(tile, &mut canvas.buffer_pool);
      }
    }
    BackgroundClip::ContentBox => {
      border_radius.inset_by_border_width();
      border_radius.expand_by(layout.padding.map(|size| -size));

      let layers = collect_background_layers(context, layout.size, &mut canvas.buffer_pool)?;

      if let Some(tile) = rasterize_layers(
        layers,
        layout.content_box_size().map(|x| x as u32),
        context,
        border_radius,
        Affine::translation(
          -layout.padding.left - layout.border.left,
          -layout.padding.top - layout.border.top,
        ),
        &mut canvas.buffer_pool,
      )? {
        canvas.overlay_image(
          &tile,
          BorderProperties::default(),
          context.transform
            * Affine::translation(
              layout.padding.left + layout.border.left,
              layout.padding.top + layout.border.top,
            ),
          context.style.image_rendering,
          BlendMode::Normal,
        );

        release_rasterized_background_tile(tile, &mut canvas.buffer_pool);
      }
    }
    _ => {}
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
      collect_background_layers(context, layout.size, &mut canvas.buffer_pool)?,
      layout.size.map(|x| x as u32),
      context,
      BorderProperties::default(),
      Affine::IDENTITY,
      &mut canvas.buffer_pool,
    )?
  } else {
    None
  };

  BorderProperties::from_context(context, layout.size, layout.border).draw(
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
  let width = context
    .style
    .outline_width
    .to_px(&context.sizing, layout.size.width)
    .max(0.0);

  let offset = context
    .style
    .outline_offset
    .to_px(&context.sizing, layout.size.width);

  let mut border = BorderProperties {
    width: Sides([width; 4]).into(),
    color: Sides([context.style.outline_color.resolve(context.current_color); 4]).into(),
    style: Sides([context.style.outline_style; 4]).into(),
    image_rendering: context.style.image_rendering,
    radius: BorderProperties::resolve_radius_part(context, layout.size),
  };

  border.expand_by(Sides([offset + width; 4]).into());

  let transform = Affine::translation(-offset - width, -offset - width) * context.transform;
  let size = layout.size.map(|x| x + (offset + width) * 2.0);

  border.draw(canvas, size, transform, None);

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

  let inline_text: InlineItem<'_, '_> = InlineItem::Text {
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
    global: context.global,
    mode: InlineLayoutMode::Draw,
  });

  draw_inline_layout(
    context,
    canvas,
    layout,
    built.layout,
    &font_style,
    InlineLayoutDrawData {
      spans: &built.spans,
      custom_inline_boxes: &built.custom_inline_boxes,
      line_scales: &built.line_scales,
    },
  )?;

  Ok(())
}
