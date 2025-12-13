use image::RgbaImage;
use parley::{GlyphRun, PositionedInlineBox, PositionedLayoutItem};
use swash::FontRef;
use taffy::{Layout, Point, Size};

use crate::{
  Result,
  layout::{
    inline::{InlineBrush, InlineLayout},
    node::Node,
    style::{Affine, BackgroundClip, SizedFontStyle, TextDecorationLine},
  },
  rendering::{Canvas, RenderContext, create_background_image, draw_decoration, draw_glyph},
  resources::font::FontError,
};

fn draw_glyph_run(
  style: &SizedFontStyle,
  glyph_run: &GlyphRun<'_, InlineBrush>,
  canvas: &mut Canvas,
  layout: Layout,
  context: &RenderContext,
  image_fill: Option<&RgbaImage>,
) -> Result<()> {
  let decoration_line = style
    .parent
    .text_decoration_line
    .as_ref()
    .unwrap_or(&style.parent.text_decoration.line);

  let run = glyph_run.run();
  let metrics = run.metrics();

  // decoration underline should not overlap with the glyph descent part,
  // as a temporary workaround, we draw the decoration under the glyph.
  if decoration_line.contains(&TextDecorationLine::Underline) {
    draw_decoration(
      canvas,
      glyph_run,
      style.text_decoration_color,
      glyph_run.baseline() - metrics.underline_offset,
      glyph_run.run().font_size() / 18.0,
      layout,
      context.transform,
    );
  }

  // Collect all glyph IDs for batch processing
  let glyph_ids = glyph_run.positioned_glyphs().map(|glyph| glyph.id);

  let font = FontRef::from_index(run.font().data.as_ref(), run.font().index as usize)
    .ok_or(FontError::InvalidFontIndex)?;
  let resolved_glyphs = context
    .global
    .font_context
    .resolve_glyphs(run, font, glyph_ids);

  let palette = font.color_palettes().next();

  // Draw each glyph using the batch-resolved cache
  for glyph in glyph_run.positioned_glyphs() {
    if let Some(cached_glyph) = resolved_glyphs.get(&glyph.id) {
      draw_glyph(
        glyph,
        cached_glyph,
        canvas,
        style,
        layout,
        image_fill,
        context.transform,
        context.opacity,
        glyph_run.style(),
        palette,
      )?;
    }
  }

  if decoration_line.contains(&TextDecorationLine::LineThrough) {
    let size = glyph_run.run().font_size() / 18.0;
    let offset = glyph_run.baseline() - metrics.strikethrough_offset;

    draw_decoration(
      canvas,
      glyph_run,
      style.text_decoration_color,
      offset,
      size,
      layout,
      context.transform,
    );
  }

  if decoration_line.contains(&TextDecorationLine::Overline) {
    draw_decoration(
      canvas,
      glyph_run,
      style.text_decoration_color,
      glyph_run.baseline() - metrics.ascent - metrics.underline_offset,
      glyph_run.run().font_size() / 18.0,
      layout,
      context.transform,
    );
  }

  Ok(())
}

pub(crate) fn draw_inline_box<N: Node<N>>(
  inline_box: &PositionedInlineBox,
  node: &N,
  context: &RenderContext,
  canvas: &mut Canvas,
  transform: Affine,
) -> Result<()> {
  if context.opacity == 0 {
    return Ok(());
  }

  let context = RenderContext {
    transform: Affine::translation(inline_box.x, inline_box.y) * transform,
    ..context.clone()
  };

  node.draw_content(
    &context,
    canvas,
    Layout {
      size: Size {
        width: inline_box.width,
        height: inline_box.height,
      },
      ..Default::default()
    },
  )
}

pub(crate) fn draw_inline_layout(
  context: &RenderContext,
  canvas: &mut Canvas,
  layout: Layout,
  inline_layout: InlineLayout,
  font_style: &SizedFontStyle,
) -> Result<Vec<PositionedInlineBox>> {
  let fill_image = if context.style.background_clip == BackgroundClip::Text {
    create_background_image(
      context,
      layout.size,
      Size {
        width: layout.content_box_width(),
        height: layout.content_box_height(),
      },
      Point {
        x: layout.padding.left + layout.border.left,
        y: layout.padding.top + layout.border.top,
      },
      &mut canvas.mask_memory,
    )?
  } else {
    None
  };

  let mut positioned_inline_boxes = Vec::new();

  for line in inline_layout.lines() {
    for item in line.items() {
      match item {
        PositionedLayoutItem::GlyphRun(glyph_run) => {
          draw_glyph_run(
            font_style,
            &glyph_run,
            canvas,
            layout,
            context,
            fill_image.as_ref(),
          )?;
        }
        PositionedLayoutItem::InlineBox(inline_box) => positioned_inline_boxes.push(inline_box),
      }
    }
  }

  Ok(positioned_inline_boxes)
}
