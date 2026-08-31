use std::{collections::HashMap, sync::Arc};

use skrifa::{FontRef, MetadataProvider};
use takumi_core::{
  geometry::{ComputedLayout as Layout, NodeId, Point, Size},
  layout::{
    inline_box::{InlineBoxPaint, resolve_inline_box},
    intercept::skip_ink_spans,
  },
};

use crate::{
  BorderProperties, Canvas, Cap, DashPattern, DecorationSegmentParams, PaintSource, Placement,
  RenderContext, Result, SizedFontStyle, Stroke, collect_background_layers, draw_background,
  draw_border, draw_decoration, draw_decoration_segment, draw_glyph, draw_glyph_clip_image,
  draw_glyph_text_shadow, draw_inset_box_shadow, draw_node_content, draw_outline,
  draw_outset_box_shadow,
  layout::inline::{
    BuiltInlineLayout, InlineBoxItem, InlineOutlineRect, InlineRunLayout, PositionedInlineRun,
    ProcessedInlineSpan, ShapedRun, VisualInlineBox, glyph_outlines, inline_background_fragments,
    inline_background_path, outline_island_contour, outline_islands, resolve_inline_runs,
  },
  painter::StrokeStyle,
  rasterize_layers,
  render::render_node,
  render_mask, resolve_outline,
  resources::{font::FontError, glyph::ResolvedGlyph},
  style::{
    Affine, BackgroundClip, BlendMode, Color, SizedTextDecorationThickness, TextDecorationLines,
    TextDecorationSkipInk,
  },
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

  let viewport = canvas.viewport();
  let subcanvas = canvas.begin_subcanvas(Placement {
    left: viewport.origin.x as i32,
    top: viewport.origin.y as i32,
    width: viewport.size.width,
    height: viewport.size.height,
  })?;

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
  let outlines = glyph_outlines(
    glyph_run,
    resolved_glyphs,
    content_offset,
    options.baseline_shift,
  );

  for (start_x, end_x) in skip_ink_spans(
    outlines.iter().copied(),
    run_start_x,
    run_start_x + glyph_run.advance,
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

fn draw_glyph_run_under_overline(
  glyph_run: &ShapedRun,
  resolved_glyphs: &HashMap<u32, Arc<ResolvedGlyph>>,
  canvas: &mut Canvas,
  options: GlyphRunLineOptions,
) -> Result<()> {
  let brush = glyph_run.brush;
  let metrics = glyph_run.metrics;

  if brush
    .decoration_line
    .contains(TextDecorationLines::UNDERLINE)
  {
    let offset =
      glyph_run.baseline + options.baseline_shift + glyph_run.underline_offset_from_baseline();
    let size = match brush.decoration_thickness {
      SizedTextDecorationThickness::Value(v) => v,
      SizedTextDecorationThickness::FromFont => metrics.underline_size,
    };

    if options.transform.only_translation()
      && brush.decoration_skip_ink != TextDecorationSkipInk::None
    {
      draw_underline_with_skip_ink(
        canvas,
        glyph_run,
        resolved_glyphs,
        UnderlineDrawOptions {
          color: brush.decoration_color,
          offset,
          size,
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
        size,
        options.layout,
        options.transform,
      );
    }
  }

  if brush
    .decoration_line
    .contains(TextDecorationLines::OVERLINE)
  {
    draw_decoration(
      canvas,
      glyph_run,
      glyph_run.brush.decoration_color,
      glyph_run.baseline + options.baseline_shift - metrics.ascent - metrics.underline_offset,
      match brush.decoration_thickness {
        SizedTextDecorationThickness::Value(v) => v,
        SizedTextDecorationThickness::FromFont => metrics.underline_size,
      },
      options.layout,
      options.transform,
    );
  }

  Ok(())
}

fn draw_glyph_run_line_through(
  glyph_run: &ShapedRun,
  canvas: &mut Canvas,
  options: GlyphRunLineOptions,
) -> Result<()> {
  let brush = glyph_run.brush;
  let decoration_line = brush.decoration_line;

  if !decoration_line.contains(TextDecorationLines::LINE_THROUGH) {
    return Ok(());
  }

  let metrics = glyph_run.metrics;
  let size = match brush.decoration_thickness {
    SizedTextDecorationThickness::Value(v) => v,
    SizedTextDecorationThickness::FromFont => metrics.strikethrough_size,
  };
  let offset = glyph_run.baseline + options.baseline_shift - metrics.strikethrough_offset;

  draw_decoration(
    canvas,
    glyph_run,
    glyph_run.brush.decoration_color,
    offset,
    size,
    options.layout,
    options.transform,
  );

  Ok(())
}

fn draw_outline_island(
  outline_rects: &[InlineOutlineRect],
  canvas: &mut Canvas,
  spans: &[ProcessedInlineSpan<'_>],
  transform: Affine,
) -> Result<()> {
  let Some(first_rect) = outline_rects.first().copied() else {
    return Ok(());
  };
  let Some(ProcessedInlineSpan::Text { style, .. }) = spans.get(first_rect.span_id as usize) else {
    return Ok(());
  };

  let Some(stroke) = style.outline_stroke() else {
    return Ok(());
  };
  let opacity = style.parent.opacity.0;
  draw_with_inline_opacity(canvas, opacity, |canvas| {
    draw_outline_island_content(outline_rects, canvas, style, &stroke, transform);
    Ok(())
  })
}

fn draw_outline_island_content(
  outline_rects: &[InlineOutlineRect],
  canvas: &mut Canvas,
  style: &SizedFontStyle,
  outline: &StrokeStyle,
  transform: Affine,
) {
  let path = outline_island_contour(outline_rects, style.outline_offset + outline.width / 2.0);
  if path.is_empty() {
    return;
  }

  let mut stroke = Stroke::new(outline.width);

  if let Some(intervals) = outline.dash {
    stroke.dash = Some(DashPattern {
      intervals,
      offset: 0.0,
    });
  }
  if outline.round_cap {
    stroke.cap = Cap::Round;
  }

  let (mask, placement) = render_mask(
    &path,
    Some(transform),
    Some(stroke.into()),
    Some(canvas.viewport()),
  );
  canvas.draw_mask(&mask, placement, outline.color, BlendMode::Normal);
}

fn draw_merged_outline_rects(
  outline_rects: Vec<InlineOutlineRect>,
  canvas: &mut Canvas,
  spans: &[ProcessedInlineSpan<'_>],
  transform: Affine,
) -> Result<()> {
  for island in outline_islands(outline_rects) {
    draw_outline_island(&island, canvas, spans, transform)?;
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
      let mut root = subtree.root;

      render_node(
        &mut root,
        &subtree.results,
        NodeId::ROOT,
        canvas,
        transform
          * Affine::translation(
            origin.x + subtree.margin_offset.x,
            origin.y + subtree.margin_offset.y,
          ),
        Size {
          width: Some(subtree.size.width),
          height: Some(subtree.size.height),
        },
      )
    }
    InlineBoxPaint::Replaced { node, layout } => {
      let Some(source) = &node.node else {
        return Ok(());
      };
      let mut context = node.context.clone();
      context.transform = transform * Affine::translation(origin.x, origin.y);

      draw_outset_box_shadow(&context, canvas, layout)?;
      draw_background(&context, canvas, layout)?;
      draw_inset_box_shadow(&context, canvas, layout)?;
      draw_border(&context, canvas, layout)?;
      draw_node_content(source, &context, canvas, layout)?;
      if let Some((outline, transform)) = resolve_outline(&context, layout) {
        draw_outline(&outline, transform, canvas);
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
  let spans = &built.spans;
  let resolved = resolve_inline_runs(built, context, layout)?;

  // Inline-span backgrounds fill under every glyph of the formatting context.
  for fragment in inline_background_fragments(&resolved, spans) {
    let path = inline_background_path(&fragment);
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
    let layers = collect_background_layers(context, layout)?;

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
        draw_glyph_run_under_overline(&run.glyph_run, &run.resolved_glyphs, canvas, opts)
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
    draw_merged_outline_rects(outline_rects, canvas, spans, context.transform)?;
  }

  if need_line_through {
    for run in &runs {
      let opts = line_options(run);
      draw_with_inline_opacity(canvas, run.glyph_run.brush.opacity, |canvas| {
        draw_glyph_run_line_through(&run.glyph_run, canvas, opts)
      })?;
    }
  }

  Ok(inline_boxes)
}
