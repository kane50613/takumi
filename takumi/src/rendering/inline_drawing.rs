use std::collections::HashMap;

use parley::{GlyphRun, InlineBoxKind, Line, PositionedInlineBox, PositionedLayoutItem};
use skrifa::{FontRef, MetadataProvider};
use taffy::{Layout, Point};

use crate::{
  Result,
  layout::{
    inline::{
      InlineBoxItem, InlineBrush, InlineLayout, ProcessedInlineSpan, ResolvedLineMetrics,
      VisualInlineBox, get_parent_font_metrics, resolve_inline_line_metrics,
      resolve_inline_line_states, resolve_visual_inline_box, text_fit_line_alignment_correction,
    },
    style::{
      Affine, BackgroundClip, BlendMode, BorderStyle, Color, SizedTextDecorationThickness,
      TextDecorationLines, TextDecorationSkipInk,
    },
    tree::LayoutTree,
  },
  rendering::{
    BorderProperties, Canvas, Cap, Command, DashPattern, DecorationSegmentParams, PaintSource,
    PathBuilder, Placement, RenderContext, SizedFontStyle, Stroke, collect_background_layers,
    draw_background, draw_border, draw_decoration, draw_decoration_segment, draw_glyph,
    draw_glyph_clip_image, draw_glyph_text_shadow, draw_inset_box_shadow, draw_node_content,
    draw_outline, draw_outset_box_shadow, mask_index_from_coord, rasterize_layers,
    release_rasterized_background_tile, render::render_node, render_mask, scale_text_fit_x,
    text_fit_x_correction,
  },
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
struct InlineOutlineRect {
  span_id: u64,
  line_index: usize,
  x: f32,
  y: f32,
  width: f32,
  height: f32,
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

#[derive(Clone, Copy)]
struct LineScaleState {
  scale: f32,
  alignment_correction: f32,
  layout_origin: Point<f32>,
}

pub(crate) struct InlineLayoutDrawData<'a, 'c, 'g> {
  pub(crate) spans: &'a [ProcessedInlineSpan<'c, 'g>],
  pub(crate) custom_inline_boxes: &'a [PositionedInlineBox],
  pub(crate) line_scales: &'a [f32],
}

fn line_scale_transform_with_static_prefix(
  base: Affine,
  state: LineScaleState,
  static_inline_prefix: f32,
) -> Affine {
  let x_correction = text_fit_x_correction(
    state.scale,
    static_inline_prefix,
    state.alignment_correction,
  );

  base
    * Affine::translation(x_correction, 0.0)
    * Affine::translation(state.layout_origin.x, state.layout_origin.y)
    * Affine::scale(state.scale, state.scale)
    * Affine::translation(-state.layout_origin.x, -state.layout_origin.y)
}

fn scale_outline_rect(
  rect: InlineOutlineRect,
  state: LineScaleState,
  static_inline_prefix: f32,
) -> InlineOutlineRect {
  if (state.scale - 1.0).abs() <= f32::EPSILON {
    return rect;
  }

  let x_correction = text_fit_x_correction(
    state.scale,
    static_inline_prefix,
    state.alignment_correction,
  );

  InlineOutlineRect {
    x: x_correction + state.layout_origin.x + (rect.x - state.layout_origin.x) * state.scale,
    y: state.layout_origin.y + (rect.y - state.layout_origin.y) * state.scale,
    width: rect.width * state.scale,
    height: rect.height * state.scale,
    ..rect
  }
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
    let offset = glyph_run.baseline() + options.baseline_shift - metrics.underline_offset;
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

fn collect_glyph_run_outline_rect(
  glyph_run: &GlyphRun<'_, InlineBrush>,
  layout: Layout,
  line_index: usize,
  line_top: f32,
  line_height: f32,
  line_scale: LineScaleState,
  static_inline_prefix: f32,
) -> Option<InlineOutlineRect> {
  let span_id = glyph_run.style().brush.source_span_id?;

  Some(scale_outline_rect(
    InlineOutlineRect {
      span_id,
      line_index,
      x: layout.border.left + layout.padding.left + glyph_run.offset(),
      y: line_top,
      width: glyph_run.advance(),
      height: line_height,
    },
    line_scale,
    static_inline_prefix,
  ))
}

const OUTLINE_COORD_TOLERANCE: f32 = 1e-3;

fn x_ranges_touch(left: InlineOutlineRect, right: InlineOutlineRect) -> bool {
  left.x <= right.x + right.width + OUTLINE_COORD_TOLERANCE
    && right.x <= left.x + left.width + OUTLINE_COORD_TOLERANCE
}

fn append_outline_contour(
  path: &mut Vec<Command>,
  outline_rects: &[InlineOutlineRect],
  amount: f32,
) {
  let mut expanded_rects = outline_rects
    .iter()
    .filter_map(|r| expand_outline_rect(*r, amount));

  let Some(first_rect) = expanded_rects.next() else {
    return;
  };

  path.move_to((first_rect.x, first_rect.y));
  path.line_to((first_rect.x + first_rect.width, first_rect.y));

  let mut current_rect = first_rect;

  for next_rect in expanded_rects {
    path.line_to((current_rect.x + current_rect.width, next_rect.y));
    path.line_to((next_rect.x + next_rect.width, next_rect.y));
    current_rect = next_rect;
  }
  let last_rect = current_rect;

  path.line_to((
    last_rect.x + last_rect.width,
    last_rect.y + last_rect.height,
  ));
  path.line_to((last_rect.x, last_rect.y + last_rect.height));

  let mut expanded_rev = outline_rects
    .iter()
    .rev()
    .filter_map(|r| expand_outline_rect(*r, amount));
  let Some(mut lower_rect) = expanded_rev.next() else {
    return;
  };

  for upper_rect in expanded_rev {
    path.line_to((lower_rect.x, upper_rect.y + upper_rect.height));
    path.line_to((upper_rect.x, upper_rect.y + upper_rect.height));
    lower_rect = upper_rect;
  }

  path.close();
}

fn expand_outline_rect(outline_rect: InlineOutlineRect, amount: f32) -> Option<InlineOutlineRect> {
  let width = outline_rect.width + amount * 2.0;
  let height = outline_rect.height + amount * 2.0;
  if width <= 0.0 || height <= 0.0 {
    return None;
  }

  Some(InlineOutlineRect {
    x: outline_rect.x - amount,
    y: outline_rect.y - amount,
    width,
    height,
    ..outline_rect
  })
}

fn draw_outline_island(
  outline_rects: &[InlineOutlineRect],
  canvas: &mut Canvas,
  spans: &[ProcessedInlineSpan<'_, '_>],
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

  let expansion = style.outline_offset + width / 2.0;
  let mut path = Vec::with_capacity(outline_rects.len() * 6);
  append_outline_contour(&mut path, outline_rects, expansion);
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
  mut outline_rects: Vec<InlineOutlineRect>,
  canvas: &mut Canvas,
  spans: &[ProcessedInlineSpan<'_, '_>],
  transform: Affine,
) -> Result<()> {
  outline_rects.sort_by(|left, right| {
    left
      .span_id
      .cmp(&right.span_id)
      .then(left.line_index.cmp(&right.line_index))
      .then(left.x.total_cmp(&right.x))
  });

  let mut merged_rects = Vec::with_capacity(outline_rects.len());
  for outline_rect in outline_rects {
    let Some(previous_rect) = merged_rects.last_mut() else {
      merged_rects.push(outline_rect);
      continue;
    };

    let same_group = previous_rect.span_id == outline_rect.span_id
      && previous_rect.line_index == outline_rect.line_index;
    let touching =
      outline_rect.x <= previous_rect.x + previous_rect.width + OUTLINE_COORD_TOLERANCE;
    let same_band = (outline_rect.y - previous_rect.y).abs() <= OUTLINE_COORD_TOLERANCE
      && (outline_rect.height - previous_rect.height).abs() <= OUTLINE_COORD_TOLERANCE;

    if same_group && same_band && touching {
      let right_edge =
        (previous_rect.x + previous_rect.width).max(outline_rect.x + outline_rect.width);
      previous_rect.x = previous_rect.x.min(outline_rect.x);
      previous_rect.y = previous_rect.y.min(outline_rect.y);
      previous_rect.width = right_edge - previous_rect.x;
      previous_rect.height = previous_rect.height.max(outline_rect.height);
    } else {
      merged_rects.push(outline_rect);
    }
  }

  let mut line_rect_counts = HashMap::new();
  for outline_rect in &merged_rects {
    *line_rect_counts
      .entry((outline_rect.span_id, outline_rect.line_index))
      .or_insert(0usize) += 1;
  }

  let mut islands: Vec<Vec<InlineOutlineRect>> = Vec::new();
  for outline_rect in merged_rects {
    let mut matched_island = None;

    for (index, island) in islands.iter().enumerate() {
      let Some(previous_rect) = island.last().copied() else {
        continue;
      };
      if previous_rect.span_id != outline_rect.span_id {
        continue;
      }
      if outline_rect.line_index != previous_rect.line_index + 1 {
        continue;
      }

      let previous_is_unique =
        line_rect_counts.get(&(previous_rect.span_id, previous_rect.line_index)) == Some(&1);
      let current_is_unique =
        line_rect_counts.get(&(outline_rect.span_id, outline_rect.line_index)) == Some(&1);
      if (previous_is_unique && current_is_unique) || x_ranges_touch(previous_rect, outline_rect) {
        matched_island = Some(index);
        break;
      }
    }

    if let Some(index) = matched_island {
      islands[index].push(outline_rect);
    } else {
      islands.push(vec![outline_rect]);
    }
  }

  for island in islands {
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

fn collect_glyph_runs(inline_layout: &InlineLayout) -> Vec<GlyphRun<'_, InlineBrush>> {
  let mut glyph_runs = Vec::new();

  for line in inline_layout.lines() {
    for item in line.items() {
      if let PositionedLayoutItem::GlyphRun(glyph_run) = item {
        glyph_runs.push(glyph_run);
      }
    }
  }

  glyph_runs
}

fn resolve_inline_layout_glyphs(
  context: &RenderContext,
  glyph_runs: &[GlyphRun<'_, InlineBrush>],
) -> Result<Vec<HashMap<u32, ResolvedGlyph>>> {
  let mut resolved_glyph_runs = Vec::with_capacity(glyph_runs.len());

  for glyph_run in glyph_runs {
    let run = glyph_run.run();
    let glyph_ids = glyph_run.positioned_glyphs().map(|glyph| glyph.id);
    let font = FontRef::from_index(run.font().data.as_ref(), run.font().index)
      .map_err(|_| FontError::InvalidFontIndex)?;

    resolved_glyph_runs.push(
      context
        .global
        .font_context
        .resolve_glyphs(glyph_run, font, glyph_ids),
    );
  }

  Ok(resolved_glyph_runs)
}

pub(crate) fn draw_inline_box(
  inline_box: &VisualInlineBox,
  item: &InlineBoxItem<'_, '_>,
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

  let context = RenderContext {
    transform: Affine::translation(inline_box.x, inline_box.y) * transform,
    ..item.render_node.context.clone()
  };
  let layout = item.into();

  draw_outset_box_shadow(&context, canvas, layout)?;
  draw_background(&context, canvas, layout)?;
  draw_inset_box_shadow(&context, canvas, layout)?;
  draw_border(&context, canvas, layout)?;
  draw_node_content(node, &context, canvas, layout)?;
  draw_outline(&context, canvas, layout)?;

  Ok(())
}

struct LineSetup {
  state: LineScaleState,
  baseline_shift: f32,
  line_scale_origin_x: f32,
  resolved_metrics: ResolvedLineMetrics,
}

fn line_setup(
  line: &Line<'_, InlineBrush>,
  layout: Layout,
  line_vertical_metrics: &[ResolvedLineMetrics],
  line_scales: &[f32],
  line_index: usize,
) -> LineSetup {
  let resolved_metrics = line_vertical_metrics[line_index];
  let line_scale = line_scales.get(line_index).copied().unwrap_or(1.0);
  let (line_scale_origin_x, line_alignment_correction) =
    text_fit_line_alignment_correction(line, line_scale, layout.content_box_size().width);
  LineSetup {
    state: LineScaleState {
      scale: line_scale,
      alignment_correction: line_alignment_correction,
      layout_origin: Point {
        x: layout.border.left + layout.padding.left + line_scale_origin_x,
        y: layout.border.top + layout.padding.top + resolved_metrics.resolved_baseline,
      },
    },
    baseline_shift: resolved_metrics.baseline_shift,
    line_scale_origin_x,
    resolved_metrics,
  }
}

struct LinePassContext<'a> {
  inline_layout: &'a InlineLayout,
  layout: Layout,
  base_transform: Affine,
  line_vertical_metrics: &'a [ResolvedLineMetrics],
  line_scales: &'a [f32],
  per_line_resolved: &'a [&'a [HashMap<u32, ResolvedGlyph>]],
}

fn for_each_glyph_run_pass(
  ctx: &LinePassContext<'_>,
  canvas: &mut Canvas,
  mut visit: impl FnMut(
    &mut Canvas,
    &GlyphRun<'_, InlineBrush>,
    &HashMap<u32, ResolvedGlyph>,
    GlyphRunLineOptions,
  ) -> Result<()>,
) -> Result<()> {
  for (line_index, line) in ctx.inline_layout.lines().enumerate() {
    let setup = line_setup(
      &line,
      ctx.layout,
      ctx.line_vertical_metrics,
      ctx.line_scales,
      line_index,
    );
    let mut resolved_iter = ctx.per_line_resolved[line_index].iter();
    let mut static_inline_prefix = 0.0_f32;
    for item in line.items() {
      match item {
        PositionedLayoutItem::GlyphRun(glyph_run) => {
          let Some(resolved_glyphs) = resolved_iter.next() else {
            continue;
          };
          let opts = GlyphRunLineOptions {
            layout: ctx.layout,
            baseline_shift: setup.baseline_shift,
            transform: line_scale_transform_with_static_prefix(
              ctx.base_transform,
              setup.state,
              static_inline_prefix,
            ),
          };
          draw_with_inline_opacity(canvas, glyph_run.style().brush.opacity, |canvas| {
            visit(canvas, &glyph_run, resolved_glyphs, opts)
          })?;
        }
        PositionedLayoutItem::InlineBox(inline_box) if inline_box.kind == InlineBoxKind::InFlow => {
          static_inline_prefix += inline_box.width;
        }
        PositionedLayoutItem::InlineBox(_) => {}
      }
    }
  }
  Ok(())
}

pub(crate) fn draw_inline_layout(
  context: &RenderContext,
  canvas: &mut Canvas,
  layout: Layout,
  inline_layout: InlineLayout,
  font_style: &SizedFontStyle,
  data: InlineLayoutDrawData<'_, '_, '_>,
) -> Result<Vec<VisualInlineBox>> {
  let InlineLayoutDrawData {
    spans,
    custom_inline_boxes,
    line_scales,
  } = data;
  let glyph_runs = collect_glyph_runs(&inline_layout);
  let resolved_glyph_runs = resolve_inline_layout_glyphs(context, &glyph_runs)?;
  let decoration_mask = glyph_runs
    .iter()
    .fold(TextDecorationLines::empty(), |acc, run| {
      acc | run.style().brush.decoration_line
    });
  let need_text_shadow = !font_style.text_shadow.is_empty();
  let need_under_overline =
    decoration_mask.intersects(TextDecorationLines::UNDERLINE | TextDecorationLines::OVERLINE);
  let need_line_through = decoration_mask.contains(TextDecorationLines::LINE_THROUGH);
  let need_outline_collect = spans.iter().any(|span| match span {
    ProcessedInlineSpan::Text { style, .. } => {
      style.outline_width > 0.0 && style.outline_style.is_rendered()
    }
    _ => false,
  });
  let clip_image = if context.style.background_clip == BackgroundClip::Text {
    let layers = collect_background_layers(context, layout.size, &mut canvas.buffer_pool)?;

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
  let parent_font_metrics = get_parent_font_metrics(&inline_layout);

  let mut positioned_inline_boxes = HashMap::new();
  let mut inline_outline_rects = Vec::new();

  let line_vertical_metrics =
    resolve_inline_line_metrics(&inline_layout, spans, parent_font_metrics, line_scales);
  let line_states =
    resolve_inline_line_states(&inline_layout, spans, parent_font_metrics, line_scales);

  // Pre-slice resolved glyph runs per line so each CSS painting phase can index
  // directly instead of maintaining fragile in-sync iterator state across separate loops.
  let per_line_resolved: Vec<&[HashMap<u32, ResolvedGlyph>]> = {
    let mut slices = Vec::with_capacity(line_vertical_metrics.len());
    let mut offset = 0;
    for line in inline_layout.lines() {
      let run_count = line
        .items()
        .filter(|i| matches!(i, PositionedLayoutItem::GlyphRun(_)))
        .count();
      slices.push(&resolved_glyph_runs[offset..offset + run_count]);
      offset += run_count;
    }
    slices
  };

  let pass_ctx = LinePassContext {
    inline_layout: &inline_layout,
    layout,
    base_transform: context.transform,
    line_vertical_metrics: &line_vertical_metrics,
    line_scales,
    per_line_resolved: &per_line_resolved,
  };

  // Reference: https://www.w3.org/TR/css-text-decor-3/#painting-order
  if need_text_shadow {
    for_each_glyph_run_pass(
      &pass_ctx,
      canvas,
      |canvas, glyph_run, resolved_glyphs, opts| {
        draw_glyph_run_text_shadow(font_style, glyph_run, resolved_glyphs, canvas, opts)
      },
    )?;
  }

  if need_under_overline {
    for_each_glyph_run_pass(
      &pass_ctx,
      canvas,
      |canvas, glyph_run, resolved_glyphs, opts| {
        draw_glyph_run_under_overline(glyph_run, resolved_glyphs, canvas, opts)
      },
    )?;
  }

  for (line_index, line) in inline_layout.lines().enumerate() {
    let setup = line_setup(
      &line,
      layout,
      &line_vertical_metrics,
      line_scales,
      line_index,
    );
    let mut resolved_iter = per_line_resolved[line_index].iter();
    let mut static_inline_prefix = 0.0_f32;

    for item in line.items() {
      match item {
        PositionedLayoutItem::GlyphRun(glyph_run) => {
          let Some(resolved_glyphs) = resolved_iter.next() else {
            continue;
          };
          draw_with_inline_opacity(canvas, glyph_run.style().brush.opacity, |canvas| {
            draw_glyph_run_content(
              &glyph_run,
              resolved_glyphs,
              canvas,
              GlyphRunContentOptions {
                glyph_offset: Point {
                  x: layout.border.left + layout.padding.left,
                  y: layout.border.top + layout.padding.top + setup.baseline_shift,
                },
                clip_image: clip_image_source,
                transform: line_scale_transform_with_static_prefix(
                  context.transform,
                  setup.state,
                  static_inline_prefix,
                ),
                style: font_style,
              },
            )
          })?;
          if need_outline_collect
            && let Some(outline_rect) = collect_glyph_run_outline_rect(
              &glyph_run,
              layout,
              line_index,
              layout.border.top + layout.padding.top + glyph_run.baseline() + setup.baseline_shift
                - setup.resolved_metrics.resolved_ascent,
              setup.resolved_metrics.resolved_line_height,
              setup.state,
              static_inline_prefix,
            )
          {
            inline_outline_rects.push(outline_rect);
          }
        }
        PositionedLayoutItem::InlineBox(inline_box) => {
          if inline_box.kind != InlineBoxKind::InFlow {
            continue;
          }
          let Some(inline_box) =
            resolve_visual_inline_box(inline_box, Some(line_states[line_index]), spans)
          else {
            continue;
          };
          let inline_box = VisualInlineBox {
            x: scale_text_fit_x(
              inline_box.layout_x,
              setup.line_scale_origin_x,
              setup.state.scale,
              static_inline_prefix,
              setup.state.alignment_correction,
            ),
            ..inline_box
          };
          let replaced = positioned_inline_boxes.insert(inline_box.id, inline_box);
          debug_assert!(replaced.is_none());
          static_inline_prefix += inline_box.layout_advance;
        }
      }
    }
  }

  for inline_box in custom_inline_boxes {
    let Some(inline_box) = resolve_visual_inline_box(inline_box.clone(), None, spans) else {
      continue;
    };
    let replaced = positioned_inline_boxes.insert(inline_box.id, inline_box);
    debug_assert!(replaced.is_none());
  }

  if need_outline_collect {
    draw_merged_outline_rects(inline_outline_rects, canvas, spans, context.transform)?;
  }

  if need_line_through {
    for_each_glyph_run_pass(&pass_ctx, canvas, |canvas, glyph_run, _, opts| {
      draw_glyph_run_line_through(glyph_run, canvas, opts)
    })?;
  }

  if let Some(tile) = clip_image {
    release_rasterized_background_tile(tile, &mut canvas.buffer_pool);
  }

  let mut positioned_inline_boxes: Vec<_> = positioned_inline_boxes.into_values().collect();
  positioned_inline_boxes.sort_by_key(|inline_box| inline_box.id);
  Ok(positioned_inline_boxes)
}
