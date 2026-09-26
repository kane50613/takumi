use std::{collections::HashMap, sync::Arc};

use skrifa::{FontRef, MetadataProvider};
use takumi_core::{
  geometry::{ComputedLayout as Layout, Point},
  layout::{
    inline_box::{InlineBoxPaint, resolve_inline_box},
    intercept::skip_ink_spans,
  },
};

use crate::{
  BorderProperties, Canvas, CanvasDevice, DecorationSegmentParams, DeferredOutline, PaintSource,
  RenderContext, Result, SizedFontStyle, collect_background_layers, draw_box_shell,
  draw_decoration, draw_decoration_segment, draw_glyph, draw_glyph_clip_image,
  draw_glyph_text_shadow, draw_node_content,
  layout::inline::{
    BuiltInlineLayout, InlineBoxItem, InlineRunLayout, OutlineIsland, PositionedInlineRun,
    ShapedRun, VisualInlineBox,
  },
  painter::BoxPainter,
  rasterize_layers, render_mask,
  resources::{font::FontError, glyph::ResolvedGlyph},
  stacking_context::ScenePainter,
  style::{Affine, BackgroundClip, BlendMode, Color, TextDecorationLines, TextDecorationSkipInk},
};

fn draw_with_inline_opacity(
  canvas: &mut Canvas,
  opacity: f32,
  draw: impl FnOnce(&mut Canvas) -> Result<()>,
) -> Result<()> {
  if opacity >= 1.0 {
    return draw(canvas);
  }

  if opacity <= 0.0 {
    return Ok(());
  }

  let subcanvas = canvas.begin_subcanvas(canvas.viewport().placement())?;

  draw(canvas)?;
  canvas.composite_subcanvas(subcanvas, BlendMode::Normal, opacity);

  Ok(())
}

#[derive(Clone, Copy)]
struct UnderlineDrawOptions {
  color: Color,
  offset: f32,
  size: f32,
  layout: Layout,
  transform: Affine,
  baseline_shift: f32,
}

#[derive(Clone, Copy)]
struct GlyphRunLineOptions {
  layout: Layout,
  baseline_shift: f32,
  transform: Affine,
}

struct GlyphRunContentOptions<'a> {
  glyph_offset: Point<f32>,
  clip_image: Option<PaintSource<'a>>,
  transform: Affine,
  style: &'a SizedFontStyle<'a>,
}

fn draw_underline_with_skip_ink(
  canvas: &mut Canvas,
  glyph_run: &ShapedRun,
  resolved_glyphs: &HashMap<u32, Arc<ResolvedGlyph>>,
  options: UnderlineDrawOptions,
) {
  let content_offset = options.layout.content_box_offset();
  let run_start_x = content_offset.x + glyph_run.offset;
  let line_top = content_offset.y + options.offset;
  let outlines = glyph_run.glyph_outlines(resolved_glyphs, content_offset, options.baseline_shift);

  for (start_x, end_x) in skip_ink_spans(
    outlines.iter().copied(),
    run_start_x,
    run_start_x + glyph_run.decorated_advance(),
    line_top,
    line_top + options.size,
  ) {
    draw_decoration_segment(
      canvas,
      options.color,
      DecorationSegmentParams {
        offset: options.offset,
        size: options.size,
        start_x,
        end_x,
        layout: options.layout,
        transform: options.transform,
      },
    );
  }
}

fn draw_glyph_run_decorations(
  glyph_run: &ShapedRun,
  resolved_glyphs: &HashMap<u32, Arc<ResolvedGlyph>>,
  canvas: &mut Canvas,
  options: GlyphRunLineOptions,
  lines: TextDecorationLines,
) -> Result<()> {
  let brush = glyph_run.brush;

  for line_kind in [
    TextDecorationLines::UNDERLINE,
    TextDecorationLines::OVERLINE,
    TextDecorationLines::LINE_THROUGH,
  ] {
    if !lines.contains(line_kind) {
      continue;
    }

    let Some((offset, thickness)) = glyph_run.decoration_line(line_kind, options.baseline_shift)
    else {
      continue;
    };

    if line_kind == TextDecorationLines::UNDERLINE
      && options.transform.only_translation()
      && brush.decoration_skip_ink != TextDecorationSkipInk::None
    {
      draw_underline_with_skip_ink(
        canvas,
        glyph_run,
        resolved_glyphs,
        UnderlineDrawOptions {
          color: brush.decoration_color,
          offset,
          size: thickness,
          layout: options.layout,
          transform: options.transform,
          baseline_shift: options.baseline_shift,
        },
      );
    } else {
      draw_decoration(
        canvas,
        glyph_run,
        brush.decoration_color,
        offset,
        thickness,
        options.layout,
        options.transform,
      );
    }
  }

  Ok(())
}

fn draw_glyph_run_content(
  glyph_run: &ShapedRun,
  resolved_glyphs: &HashMap<u32, Arc<ResolvedGlyph>>,
  canvas: &mut Canvas,
  options: GlyphRunContentOptions<'_>,
) -> Result<()> {
  let font = FontRef::from_index(glyph_run.font_data(), glyph_run.font_index)
    .map_err(|_| FontError::InvalidFontIndex)?;
  let palettes = font.color_palettes();
  let palette = palettes.get(0);
  // A span may set `-webkit-text-stroke` for itself, so it comes off the run.
  let stroke = (glyph_run.brush.stroke_width, glyph_run.brush.stroke_color);

  if let Some(clip_image) = options.clip_image {
    for glyph in &glyph_run.glyphs {
      let Some(content) = resolved_glyphs.get(&glyph.id) else {
        continue;
      };

      let inline_offset = Point {
        x: options.glyph_offset.x + glyph.x,
        y: options.glyph_offset.y + glyph.y,
      };

      draw_glyph_clip_image(
        content,
        canvas,
        options.style,
        stroke,
        options.transform,
        inline_offset,
        clip_image,
      )?;
    }
  }

  for glyph in &glyph_run.glyphs {
    let Some(content) = resolved_glyphs.get(&glyph.id) else {
      continue;
    };

    let inline_offset = Point {
      x: options.glyph_offset.x + glyph.x,
      y: options.glyph_offset.y + glyph.y,
    };

    draw_glyph(
      content,
      canvas,
      options.style,
      stroke,
      options.transform,
      inline_offset,
      glyph_run.brush.color,
      palette.as_ref(),
    )?;
  }

  Ok(())
}

fn draw_glyph_run_text_shadow(
  style: &SizedFontStyle,
  glyph_run: &ShapedRun,
  resolved_glyphs: &HashMap<u32, Arc<ResolvedGlyph>>,
  canvas: &mut Canvas,
  options: GlyphRunLineOptions,
) -> Result<()> {
  let content_offset = options.layout.content_box_offset();

  for glyph in &glyph_run.glyphs {
    let Some(content) = resolved_glyphs.get(&glyph.id) else {
      continue;
    };

    let inline_offset = Point {
      x: content_offset.x + glyph.x,
      y: content_offset.y + glyph.y + options.baseline_shift,
    };

    draw_glyph_text_shadow(content, canvas, style, options.transform, inline_offset)?;
  }

  Ok(())
}

pub(crate) fn draw_inline_box(
  inline_box: &VisualInlineBox,
  item: &InlineBoxItem<'_>,
  container: Layout,
  canvas: &mut Canvas,
  transform: Affine,
) -> Result<()> {
  let Some((origin, paint)) = resolve_inline_box(inline_box, item, container) else {
    return Ok(());
  };

  match paint {
    InlineBoxPaint::Container(subtree) => {
      let at = subtree.border_box_origin(origin);
      let mut scene = subtree.into_scene(transform * Affine::translation(at.x, at.y), true)?;

      ScenePainter::new(&mut scene, canvas).paint_context(0)
    }
    InlineBoxPaint::Replaced { node, layout } => {
      let Some(source) = &node.node else {
        return Ok(());
      };
      let mut context = node.context.clone();
      context.transform = transform * Affine::translation(origin.x, origin.y);

      draw_box_shell(&context, canvas, layout)?;
      draw_node_content(source, &context, canvas, layout)?;
      if let Some(outline) = DeferredOutline::of(&context, layout) {
        outline.paint(canvas);
      }
      Ok(())
    }
  }
}

pub(crate) fn draw_inline_layout(
  context: &RenderContext,
  canvas: &mut Canvas,
  layout: Layout,
  built: &BuiltInlineLayout<'_>,
  font_style: &SizedFontStyle,
) -> Result<Vec<VisualInlineBox>> {
  let resolved = built.resolve_runs(context, layout)?;

  // Inline-span backgrounds fill under every glyph of the formatting context.
  for fragment in &resolved.background_fragments {
    let path = fragment.path();
    let (mask, placement) = render_mask(
      &path,
      Some(context.transform),
      None,
      Some(canvas.viewport()),
    );

    draw_with_inline_opacity(canvas, fragment.opacity, |canvas| {
      canvas.draw_mask(&mask, placement, fragment.color, BlendMode::Normal);
      Ok(())
    })?;
  }

  let InlineRunLayout {
    runs,
    inline_boxes,
    outline_rects,
    ..
  } = resolved;

  let decoration_mask = runs.iter().fold(TextDecorationLines::empty(), |acc, run| {
    acc | run.glyph_run.brush.decoration_line
  });
  let need_text_shadow = !font_style.text_shadow.is_empty();
  let need_under_overline =
    decoration_mask.intersects(TextDecorationLines::UNDERLINE | TextDecorationLines::OVERLINE);
  let need_line_through = decoration_mask.contains(TextDecorationLines::LINE_THROUGH);

  let clip_image = if context.style.background_clip == BackgroundClip::Text {
    let layers =
      collect_background_layers(&BoxPainter::new(context, layout).background(), context)?;

    rasterize_layers(
      layers,
      layout.size.map(|x| x as u32),
      context,
      BorderProperties::default(),
      Affine::IDENTITY,
    )?
  } else {
    None
  };
  let clip_image_source = clip_image.as_ref().map(PaintSource::from);

  let line_options = |run: &PositionedInlineRun| GlyphRunLineOptions {
    layout,
    baseline_shift: run.baseline_shift,
    transform: run.transform(context.transform),
  };

  // Reference: https://www.w3.org/TR/css-text-decor-3/#painting-order
  if need_text_shadow {
    for run in &runs {
      let opts = line_options(run);
      draw_with_inline_opacity(canvas, run.glyph_run.brush.opacity, |canvas| {
        draw_glyph_run_text_shadow(
          font_style,
          &run.glyph_run,
          &run.resolved_glyphs,
          canvas,
          opts,
        )
      })?;
    }
  }

  if need_under_overline {
    for run in &runs {
      let opts = line_options(run);
      draw_with_inline_opacity(canvas, run.glyph_run.brush.opacity, |canvas| {
        draw_glyph_run_decorations(
          &run.glyph_run,
          &run.resolved_glyphs,
          canvas,
          opts,
          TextDecorationLines::UNDERLINE | TextDecorationLines::OVERLINE,
        )
      })?;
    }
  }

  for run in &runs {
    let transform = run.transform(context.transform);
    draw_with_inline_opacity(canvas, run.glyph_run.brush.opacity, |canvas| {
      draw_glyph_run_content(
        &run.glyph_run,
        &run.resolved_glyphs,
        canvas,
        GlyphRunContentOptions {
          glyph_offset: run.glyph_offset(layout),
          clip_image: clip_image_source,
          transform,
          style: font_style,
        },
      )
    })?;
  }

  if !outline_rects.is_empty() {
    let mut device = CanvasDevice::of(canvas, context);

    for island in OutlineIsland::of(outline_rects) {
      island.paint(&built.spans, Point::ZERO, &mut device);
    }
    device.finish()?;
  }

  if need_line_through {
    for run in &runs {
      let opts = line_options(run);
      draw_with_inline_opacity(canvas, run.glyph_run.brush.opacity, |canvas| {
        draw_glyph_run_decorations(
          &run.glyph_run,
          &run.resolved_glyphs,
          canvas,
          opts,
          TextDecorationLines::LINE_THROUGH,
        )
      })?;
    }
  }

  Ok(inline_boxes)
}
