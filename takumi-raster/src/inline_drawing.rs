use std::collections::HashMap;

use parley::GlyphRun;
use skrifa::{FontRef, MetadataProvider};
use taffy::{Layout, Point};

use crate::{
  BorderProperties, Canvas, Cap, DashPattern, DecorationSegmentParams, PaintSource, Placement,
  RenderContext, Result, SizedFontStyle, Stroke, collect_background_layers, draw_background,
  draw_border, draw_decoration, draw_decoration_segment, draw_glyph, draw_glyph_clip_image,
  draw_glyph_text_shadow, draw_inset_box_shadow, draw_node_content, draw_outline,
  draw_outset_box_shadow,
  layout::{
    inline::{
      BuiltInlineLayout, InlineBoxItem, InlineBrush, InlineOutlineRect, InlineRunLayout,
      PositionedInlineRun, ProcessedInlineSpan, VisualInlineBox, outline_island_contour,
      outline_islands, resolve_inline_runs,
    },
    style::{
      Affine, BackgroundClip, BlendMode, BorderStyle, Color, SizedTextDecorationThickness,
      TextDecorationLines, TextDecorationSkipInk,
    },
    tree::LayoutTree,
  },
  mask_index_from_coord, rasterize_layers, release_rasterized_background_tile,
  render::render_node,
  render_mask,
  resources::font::{FontError, ResolvedGlyph},
};
use taffy::{AvailableSpace, geometry::Size};

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

const UNDERLINE_SKIP_INK_ALPHA_THRESHOLD: u8 = 16;
const SKIP_PADDING_RATIO: f32 = 0.6;
const SKIP_PADDING_MIN: f32 = 1.0;
const SKIP_PADDING_MAX: f32 = 3.0;

#[derive(Clone, Copy)]
struct GlyphLocalBounds {
  left: f32,
  top: f32,
  bottom: f32,
}

struct GlyphSkipInkData {
  bounds: GlyphLocalBounds,
  width: u32,
  height: u32,
  alpha: Vec<u8>,
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

fn build_glyph_bounds_cache(
  canvas: &mut Canvas,
  resolved_glyphs: &HashMap<u32, ResolvedGlyph>,
) -> HashMap<u32, GlyphSkipInkData> {
  let mut bounds = HashMap::with_capacity(resolved_glyphs.len());

  for (glyph_id, content) in resolved_glyphs {
    let glyph = match content {
      ResolvedGlyph::Bitmap(bitmap) => GlyphSkipInkData {
        bounds: GlyphLocalBounds {
          left: bitmap.placement.left as f32,
          top: -bitmap.placement.top as f32,
          bottom: -bitmap.placement.top as f32 + bitmap.placement.height as f32,
        },
        width: bitmap.placement.width,
        height: bitmap.placement.height,
        alpha: {
          let mut alpha = vec![0; (bitmap.placement.width * bitmap.placement.height) as usize];
          bitmap.write_alpha_mask(&mut alpha);
          alpha
        },
      },
      ResolvedGlyph::Outline(outline) => {
        let (mask, placement) = render_mask(outline.paths(), None, None, &mut canvas.buffer_pool);

        if placement.width == 0 || placement.height == 0 {
          continue;
        }

        let data = GlyphSkipInkData {
          bounds: GlyphLocalBounds {
            left: placement.left as f32,
            top: placement.top as f32,
            bottom: placement.top as f32 + placement.height as f32,
          },
          width: placement.width,
          height: placement.height,
          alpha: mask.to_vec(),
        };
        canvas.buffer_pool.release(mask);
        data
      }
    };

    bounds.insert(*glyph_id, glyph);
  }

  bounds
}

fn compute_skip_padding(size: f32) -> f32 {
  (size * SKIP_PADDING_RATIO).clamp(SKIP_PADDING_MIN, SKIP_PADDING_MAX)
}

fn draw_underline_with_skip_ink(
  canvas: &mut Canvas,
  glyph_run: &GlyphRun<'_, InlineBrush>,
  glyph_bounds_cache: &HashMap<u32, GlyphSkipInkData>,
  options: UnderlineDrawOptions,
) {
  let run_start_x = options.layout.border.left + options.layout.padding.left + glyph_run.offset();
  let run_end_x = run_start_x + glyph_run.advance();
  let line_top = options.layout.border.top + options.layout.padding.top + options.offset;
  let line_bottom = line_top + options.size;
  let skip_padding = compute_skip_padding(options.size);
  let mut skip_ranges = Vec::new();

  for glyph in glyph_run.positioned_glyphs() {
    let Some(glyph_data) = glyph_bounds_cache.get(&glyph.id) else {
      continue;
    };
    let local_bounds = glyph_data.bounds;
    let inline_x = options.layout.border.left + options.layout.padding.left + glyph.x;
    let inline_y =
      options.layout.border.top + options.layout.padding.top + glyph.y + options.baseline_shift;
    let glyph_top = inline_y + local_bounds.top;
    let glyph_bottom = inline_y + local_bounds.bottom;

    if glyph_bottom <= line_top || glyph_top >= line_bottom {
      continue;
    }

    let local_line_top = line_top - inline_y;
    let local_line_bottom = line_bottom - inline_y;
    let mask_y_start = (local_line_top - local_bounds.top).floor() as i32;
    let mask_y_end = (local_line_bottom - local_bounds.top).ceil() as i32;
    let y_start = mask_y_start.clamp(0, glyph_data.height as i32);
    let y_end = mask_y_end.clamp(0, glyph_data.height as i32);
    if y_start >= y_end {
      continue;
    }

    let mut hit_min_x: Option<u32> = None;
    let mut hit_max_x: Option<u32> = None;
    for y in y_start as u32..y_end as u32 {
      let mut row_min_x: Option<u32> = None;
      for x in 0..glyph_data.width {
        let alpha = glyph_data.alpha[mask_index_from_coord(x, y, glyph_data.width)];
        if alpha > UNDERLINE_SKIP_INK_ALPHA_THRESHOLD {
          row_min_x = Some(x);
          break;
        }
      }
      let Some(row_min_x) = row_min_x else {
        continue;
      };

      let mut row_max_x = row_min_x;
      for x in (row_min_x..glyph_data.width).rev() {
        let alpha = glyph_data.alpha[mask_index_from_coord(x, y, glyph_data.width)];
        if alpha > UNDERLINE_SKIP_INK_ALPHA_THRESHOLD {
          row_max_x = x;
          break;
        }
      }
      hit_min_x = Some(hit_min_x.map_or(row_min_x, |min_x| min_x.min(row_min_x)));
      hit_max_x = Some(hit_max_x.map_or(row_max_x, |max_x| max_x.max(row_max_x)));
    }

    let (hit_min_x, hit_max_x) = match (hit_min_x, hit_max_x) {
      (Some(min_x), Some(max_x)) => (min_x, max_x),
      _ => continue,
    };

    let skip_start =
      (inline_x + local_bounds.left + hit_min_x as f32 - skip_padding).max(run_start_x);
    let skip_end =
      (inline_x + local_bounds.left + hit_max_x as f32 + 1.0 + skip_padding).min(run_end_x);
    if skip_end > skip_start {
      skip_ranges.push((skip_start, skip_end));
    }
  }

  if skip_ranges.is_empty() {
    draw_decoration_segment(
      canvas,
      options.color,
      DecorationSegmentParams {
        offset: options.offset,
        size: options.size,
        start_x: run_start_x,
        end_x: run_end_x,
        layout: options.layout,
        transform: options.transform,
      },
    );
    return;
  }

  skip_ranges.sort_unstable_by(|a, b| a.0.total_cmp(&b.0));
  let mut merged_ranges = Vec::with_capacity(skip_ranges.len());
  for (start, end) in skip_ranges {
    let Some(last) = merged_ranges.last_mut() else {
      merged_ranges.push((start, end));
      continue;
    };
    if start <= last.1 {
      last.1 = last.1.max(end);
    } else {
      merged_ranges.push((start, end));
    }
  }

  let mut current_x = run_start_x;
  for (skip_start, skip_end) in merged_ranges {
    if skip_start > current_x {
      draw_decoration_segment(
        canvas,
        options.color,
        DecorationSegmentParams {
          offset: options.offset,
          size: options.size,
          start_x: current_x,
          end_x: skip_start,
          layout: options.layout,
          transform: options.transform,
        },
      );
    }
    current_x = current_x.max(skip_end);
  }

  if run_end_x > current_x {
    draw_decoration_segment(
      canvas,
      options.color,
      DecorationSegmentParams {
        offset: options.offset,
        size: options.size,
        start_x: current_x,
        end_x: run_end_x,
        layout: options.layout,
        transform: options.transform,
      },
    );
  }
}

fn draw_glyph_run_under_overline(
  glyph_run: &GlyphRun<'_, InlineBrush>,
  resolved_glyphs: &HashMap<u32, ResolvedGlyph>,
  canvas: &mut Canvas,
  options: GlyphRunLineOptions,
) -> Result<()> {
  let brush = &glyph_run.style().brush;

  let run = glyph_run.run();
  let metrics = run.metrics();

  if brush
    .decoration_line
    .contains(TextDecorationLines::UNDERLINE)
  {
    let offset = glyph_run.baseline() + options.baseline_shift - metrics.underline_offset
      + brush.underline_offset;
    let size = match brush.decoration_thickness {
      SizedTextDecorationThickness::Value(v) => v,
      SizedTextDecorationThickness::FromFont => metrics.underline_size,
    };

    if options.transform.only_translation()
      && brush.decoration_skip_ink != TextDecorationSkipInk::None
    {
      let glyph_bounds_cache = build_glyph_bounds_cache(canvas, resolved_glyphs);
      draw_underline_with_skip_ink(
        canvas,
        glyph_run,
        &glyph_bounds_cache,
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
      glyph_run.style().brush.decoration_color,
      glyph_run.baseline() + options.baseline_shift - metrics.ascent - metrics.underline_offset,
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
  glyph_run: &GlyphRun<'_, InlineBrush>,
  canvas: &mut Canvas,
  options: GlyphRunLineOptions,
) -> Result<()> {
  let brush = &glyph_run.style().brush;
  let decoration_line = brush.decoration_line;

  if !decoration_line.contains(TextDecorationLines::LINE_THROUGH) {
    return Ok(());
  }

  let metrics = glyph_run.run().metrics();
  let size = match brush.decoration_thickness {
    SizedTextDecorationThickness::Value(v) => v,
    SizedTextDecorationThickness::FromFont => metrics.strikethrough_size,
  };
  let offset = glyph_run.baseline() + options.baseline_shift - metrics.strikethrough_offset;

  draw_decoration(
    canvas,
    glyph_run,
    glyph_run.style().brush.decoration_color,
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

  let width = style.outline_width;
  if width == 0.0 || !style.outline_style.is_rendered() {
    return Ok(());
  }

  let opacity = style.parent.opacity.0;
  draw_with_inline_opacity(canvas, opacity, |canvas| {
    draw_outline_island_content(outline_rects, canvas, style, transform);
    Ok(())
  })
}

fn draw_outline_island_content(
  outline_rects: &[InlineOutlineRect],
  canvas: &mut Canvas,
  style: &SizedFontStyle,
  transform: Affine,
) {
  let width = style.outline_width;

  let path = outline_island_contour(outline_rects, style.outline_offset + width / 2.0);
  if path.is_empty() {
    return;
  }

  let mut stroke = Stroke::new(width);
  match style.outline_style {
    BorderStyle::Dotted => {
      stroke.cap = Cap::Round;
      stroke.dash = Some(DashPattern {
        intervals: [0.0, width * 2.0],
        offset: 0.0,
      });
    }
    BorderStyle::Dashed => {
      stroke.dash = Some(DashPattern {
        intervals: [width * 3.0, width * 2.0],
        offset: 0.0,
      });
    }
    BorderStyle::Hidden
    | BorderStyle::Double
    | BorderStyle::Groove
    | BorderStyle::Ridge
    | BorderStyle::Inset
    | BorderStyle::Outset => return,
    _ => {}
  }
  let (mask, placement) = render_mask(
    &path,
    Some(transform),
    Some(stroke.into()),
    &mut canvas.buffer_pool,
  );
  canvas.draw_mask(&mask, placement, style.outline_color, BlendMode::Normal);

  canvas.buffer_pool.release(mask);
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
  glyph_run: &GlyphRun<'_, InlineBrush>,
  resolved_glyphs: &HashMap<u32, ResolvedGlyph>,
  canvas: &mut Canvas,
  options: GlyphRunContentOptions<'_>,
) -> Result<()> {
  let run = glyph_run.run();

  let font = FontRef::from_index(run.font().data.as_ref(), run.font().index)
    .map_err(|_| FontError::InvalidFontIndex)?;
  let palettes = font.color_palettes();
  let palette = palettes.get(0);

  if let Some(clip_image) = options.clip_image {
    for glyph in glyph_run.positioned_glyphs() {
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
        options.transform,
        inline_offset,
        clip_image,
      )?;
    }
  }

  for glyph in glyph_run.positioned_glyphs() {
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
      options.transform,
      inline_offset,
      glyph_run.style().brush.color,
      palette.as_ref(),
    )?;
  }

  Ok(())
}

fn draw_glyph_run_text_shadow(
  style: &SizedFontStyle,
  glyph_run: &GlyphRun<'_, InlineBrush>,
  resolved_glyphs: &HashMap<u32, ResolvedGlyph>,
  canvas: &mut Canvas,
  options: GlyphRunLineOptions,
) -> Result<()> {
  for glyph in glyph_run.positioned_glyphs() {
    let Some(content) = resolved_glyphs.get(&glyph.id) else {
      continue;
    };

    let inline_offset = Point {
      x: options.layout.border.left + options.layout.padding.left + glyph.x,
      y: options.layout.border.top + options.layout.padding.top + glyph.y + options.baseline_shift,
    };

    draw_glyph_text_shadow(content, canvas, style, options.transform, inline_offset)?;
  }

  Ok(())
}

pub(crate) fn draw_inline_box(
  inline_box: &VisualInlineBox,
  item: &InlineBoxItem<'_>,
  canvas: &mut Canvas,
  transform: Affine,
) -> Result<()> {
  if item.render_node.context.style.opacity.0 == 0.0 {
    return Ok(());
  }

  if item.render_node.participates_as_inline_box() {
    let mut subtree_root = item.render_node.clone();
    let mut layout_tree = LayoutTree::from_render_node(&subtree_root);

    let inline_width =
      (inline_box.width - item.margin.grid_axis_sum(taffy::AbsoluteAxis::Horizontal)).max(0.0);
    let inline_height =
      (inline_box.height - item.margin.grid_axis_sum(taffy::AbsoluteAxis::Vertical)).max(0.0);

    layout_tree.compute_layout(Size {
      width: AvailableSpace::Definite(inline_width),
      height: AvailableSpace::Definite(inline_height),
    });
    let layout_results = layout_tree.into_results();
    let root_node_id = layout_results.root_node_id();

    render_node(
      &mut subtree_root,
      &layout_results,
      root_node_id,
      canvas,
      Affine::translation(inline_box.x, inline_box.y)
        * transform
        * Affine::translation(item.margin.left, item.margin.top),
      Size {
        width: Some(inline_width),
        height: Some(inline_height),
      },
    )?;
    return Ok(());
  }

  let Some(node) = &item.render_node.node else {
    return Ok(());
  };

  let mut context = item.render_node.context.clone();
  context.transform = Affine::translation(inline_box.x, inline_box.y) * transform;

  let layout = item.into();

  draw_outset_box_shadow(&context, canvas, layout)?;
  draw_background(&context, canvas, layout)?;
  draw_inset_box_shadow(&context, canvas, layout)?;
  draw_border(&context, canvas, layout)?;
  draw_node_content(node, &context, canvas, layout)?;
  draw_outline(&context, canvas, layout)?;

  Ok(())
}

pub(crate) fn draw_inline_layout(
  context: &RenderContext,
  canvas: &mut Canvas,
  layout: Layout,
  built: &BuiltInlineLayout<'_>,
  font_style: &SizedFontStyle,
) -> Result<Vec<VisualInlineBox>> {
  let spans = &built.spans;
  let InlineRunLayout {
    runs,
    inline_boxes,
    outline_rects,
    ..
  } = resolve_inline_runs(built, context, layout)?;

  let decoration_mask = runs.iter().fold(TextDecorationLines::empty(), |acc, run| {
    acc | run.glyph_run.style().brush.decoration_line
  });
  let need_text_shadow = !font_style.text_shadow.is_empty();
  let need_under_overline =
    decoration_mask.intersects(TextDecorationLines::UNDERLINE | TextDecorationLines::OVERLINE);
  let need_line_through = decoration_mask.contains(TextDecorationLines::LINE_THROUGH);

  let clip_image = if context.style.background_clip == BackgroundClip::Text {
    let layers = collect_background_layers(context, layout, &mut canvas.buffer_pool)?;

    rasterize_layers(
      layers,
      layout.size.map(|x| x as u32),
      context,
      BorderProperties::default(),
      Affine::IDENTITY,
      &mut canvas.buffer_pool,
    )?
  } else {
    None
  };
  let clip_image_source = clip_image.as_ref().map(PaintSource::from);

  let line_options = |run: &PositionedInlineRun<'_>| GlyphRunLineOptions {
    layout,
    baseline_shift: run.baseline_shift,
    transform: run.transform(context.transform),
  };

  // Reference: https://www.w3.org/TR/css-text-decor-3/#painting-order
  if need_text_shadow {
    for run in &runs {
      let opts = line_options(run);
      draw_with_inline_opacity(canvas, run.glyph_run.style().brush.opacity, |canvas| {
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
      draw_with_inline_opacity(canvas, run.glyph_run.style().brush.opacity, |canvas| {
        draw_glyph_run_under_overline(&run.glyph_run, &run.resolved_glyphs, canvas, opts)
      })?;
    }
  }

  for run in &runs {
    let transform = run.transform(context.transform);
    draw_with_inline_opacity(canvas, run.glyph_run.style().brush.opacity, |canvas| {
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
      draw_with_inline_opacity(canvas, run.glyph_run.style().brush.opacity, |canvas| {
        draw_glyph_run_line_through(&run.glyph_run, canvas, opts)
      })?;
    }
  }

  if let Some(tile) = clip_image {
    release_rasterized_background_tile(tile, &mut canvas.buffer_pool);
  }

  Ok(inline_boxes)
}
