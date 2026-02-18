use std::collections::HashMap;

use image::{GenericImageView, Rgba};
use parley::{GlyphRun, LineMetrics, PositionedInlineBox, PositionedLayoutItem};
use swash::FontRef;
use taffy::{Layout, Point};

use crate::{
  Result,
  layout::{
    inline::{InlineBoxItem, InlineBrush, InlineLayout},
    node::Node,
    style::{
      Affine, BackgroundClip, BlendMode, Color, ImageScalingAlgorithm, SizedFontStyle,
      TextDecorationLine, TextDecorationSkipInk,
    },
  },
  rendering::{
    BorderProperties, Canvas, ColorTile, RenderContext, collect_background_layers,
    collect_outline_paths, draw_decoration, draw_glyph, draw_glyph_clip_image,
    mask_index_from_coord, rasterize_layers,
  },
  resources::font::{FontError, ResolvedGlyph},
};

const UNDERLINE_SKIP_INK_ALPHA_THRESHOLD: u8 = 16;
/// Empirical multiplier used for extending skip-ink bounds around detected glyph ink.
const SKIP_PADDING_RATIO: f32 = 0.6;
/// Empirical lower bound for skip-ink padding on thin underlines.
const SKIP_PADDING_MIN: f32 = 1.0;
/// Empirical upper bound for skip-ink padding on thick underlines.
const SKIP_PADDING_MAX: f32 = 3.0;
/// Empirical multiplier controlling when adjacent skip ranges are merged.
const SMOOTH_GAP_RATIO: f32 = 0.28;
/// Empirical lower bound for skip-range merge smoothing.
const SMOOTH_GAP_MIN: f32 = 0.5;
/// Empirical upper bound for skip-range merge smoothing.
const SMOOTH_GAP_MAX: f32 = 1.5;
/// Empirical multiplier for minimum visible underline segment between skipped ranges.
const MIN_VISIBLE_SEGMENT_RATIO: f32 = 0.8;
/// Empirical floor for minimum visible underline segment.
const MIN_VISIBLE_SEGMENT_MIN: f32 = 0.75;

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
  alpha: Box<[u8]>,
}

fn build_glyph_bounds_cache(
  canvas: &mut Canvas,
  resolved_glyphs: &HashMap<u32, ResolvedGlyph>,
) -> HashMap<u32, GlyphSkipInkData> {
  let mut bounds = HashMap::with_capacity(resolved_glyphs.len());

  for (glyph_id, content) in resolved_glyphs {
    let glyph = match content {
      ResolvedGlyph::Image(bitmap) => GlyphSkipInkData {
        bounds: GlyphLocalBounds {
          left: bitmap.placement.left as f32,
          top: -bitmap.placement.top as f32,
          bottom: -bitmap.placement.top as f32 + bitmap.placement.height as f32,
        },
        width: bitmap.placement.width,
        height: bitmap.placement.height,
        alpha: bitmap.data.iter().skip(3).step_by(4).copied().collect(),
      },
      ResolvedGlyph::Outline(outline) => {
        let paths = collect_outline_paths(outline);
        let (mask, placement) = canvas.mask_memory.render(&paths, None, None);

        if placement.width == 0 || placement.height == 0 {
          continue;
        }

        GlyphSkipInkData {
          bounds: GlyphLocalBounds {
            left: placement.left as f32,
            top: placement.top as f32,
            bottom: placement.top as f32 + placement.height as f32,
          },
          width: placement.width,
          height: placement.height,
          alpha: mask.to_vec().into_boxed_slice(),
        }
      }
    };

    bounds.insert(*glyph_id, glyph);
  }

  bounds
}

fn draw_decoration_segment(
  canvas: &mut Canvas,
  color: Color,
  x: f32,
  y: f32,
  width: f32,
  height: f32,
  transform: Affine,
) {
  let tile = ColorTile {
    color: color.into(),
    width: width.ceil() as u32,
    height: height.ceil() as u32,
  };

  if tile.width == 0 || tile.height == 0 {
    return;
  }

  canvas.overlay_image(
    &tile,
    BorderProperties::default(),
    transform * Affine::translation(x, y),
    ImageScalingAlgorithm::Auto,
    BlendMode::Normal,
  );
}

fn compute_skip_padding(size: f32) -> f32 {
  (size * SKIP_PADDING_RATIO).clamp(SKIP_PADDING_MIN, SKIP_PADDING_MAX)
}

fn compute_smooth_gap(size: f32) -> f32 {
  (size * SMOOTH_GAP_RATIO).clamp(SMOOTH_GAP_MIN, SMOOTH_GAP_MAX)
}

fn compute_min_visible_segment(size: f32) -> f32 {
  (size * MIN_VISIBLE_SEGMENT_RATIO).max(MIN_VISIBLE_SEGMENT_MIN)
}

#[allow(clippy::too_many_arguments)]
fn draw_underline_with_skip_ink(
  canvas: &mut Canvas,
  glyph_run: &GlyphRun<'_, InlineBrush>,
  glyph_bounds_cache: &HashMap<u32, GlyphSkipInkData>,
  color: Color,
  offset: f32,
  size: f32,
  layout: Layout,
  transform: Affine,
) {
  let run_start_x = layout.border.left + layout.padding.left + glyph_run.offset();
  let run_end_x = run_start_x + glyph_run.advance();
  let line_top = layout.border.top + layout.padding.top + offset;
  let line_bottom = line_top + size;
  let skip_padding = compute_skip_padding(size);
  let smooth_gap = compute_smooth_gap(size);
  let min_visible_segment = compute_min_visible_segment(size);

  let mut skip_ranges = Vec::new();

  for glyph in glyph_run.positioned_glyphs() {
    let Some(glyph_data) = glyph_bounds_cache.get(&glyph.id) else {
      continue;
    };
    let local_bounds = glyph_data.bounds;

    let inline_x = layout.border.left + layout.padding.left + glyph.x;
    let inline_y = layout.border.top + layout.padding.top + glyph.y;

    let glyph_top = inline_y + local_bounds.top;
    let glyph_bottom = inline_y + local_bounds.bottom;

    let intersects_underline = glyph_bottom > line_top && glyph_top < line_bottom;
    if !intersects_underline {
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
    draw_decoration(canvas, glyph_run, color, offset, size, layout, transform);
    return;
  }

  skip_ranges.sort_unstable_by(|a, b| a.0.total_cmp(&b.0));

  let mut merged_ranges = Vec::with_capacity(skip_ranges.len());
  for (start, end) in skip_ranges {
    let Some(last) = merged_ranges.last_mut() else {
      merged_ranges.push((start, end));
      continue;
    };

    if start <= last.1 + smooth_gap {
      last.1 = last.1.max(end);
    } else {
      merged_ranges.push((start, end));
    }
  }

  let mut current_x = run_start_x;
  for (skip_start, skip_end) in merged_ranges {
    if skip_start - current_x >= min_visible_segment {
      draw_decoration_segment(
        canvas,
        color,
        current_x,
        line_top,
        skip_start - current_x,
        size,
        transform,
      );
    }
    current_x = current_x.max(skip_end);
  }

  if run_end_x - current_x >= min_visible_segment {
    draw_decoration_segment(
      canvas,
      color,
      current_x,
      line_top,
      run_end_x - current_x,
      size,
      transform,
    );
  }
}

fn draw_glyph_run<I: GenericImageView<Pixel = Rgba<u8>>>(
  style: &SizedFontStyle,
  glyph_run: &GlyphRun<'_, InlineBrush>,
  canvas: &mut Canvas,
  layout: Layout,
  context: &RenderContext,
  clip_image: Option<&I>,
) -> Result<()> {
  let decoration_line = style
    .parent
    .text_decoration_line
    .as_ref()
    .unwrap_or(&style.parent.text_decoration.line);

  let run = glyph_run.run();
  let metrics = run.metrics();

  if decoration_line.contains(&TextDecorationLine::Overline) {
    draw_decoration(
      canvas,
      glyph_run,
      glyph_run.style().brush.decoration_color,
      glyph_run.baseline() - metrics.ascent - metrics.underline_offset,
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
    .resolve_glyphs(glyph_run, font, glyph_ids);

  let palette = font.color_palettes().next();

  if decoration_line.contains(&TextDecorationLine::Underline) {
    let offset = glyph_run.baseline() - metrics.underline_offset;
    let size = glyph_run.run().font_size() / 18.0;

    if context.transform.only_translation()
      && style.parent.text_decoration_skip_ink != TextDecorationSkipInk::None
    {
      let glyph_bounds_cache = build_glyph_bounds_cache(canvas, &resolved_glyphs);

      draw_underline_with_skip_ink(
        canvas,
        glyph_run,
        &glyph_bounds_cache,
        glyph_run.style().brush.decoration_color,
        offset,
        size,
        layout,
        context.transform,
      );
    } else {
      draw_decoration(
        canvas,
        glyph_run,
        glyph_run.style().brush.decoration_color,
        offset,
        size,
        layout,
        context.transform,
      );
    }
  }

  if let Some(clip_image) = clip_image {
    for glyph in glyph_run.positioned_glyphs() {
      let Some(content) = resolved_glyphs.get(&glyph.id) else {
        continue;
      };

      let inline_offset = Point {
        x: layout.border.left + layout.padding.left + glyph.x,
        y: layout.border.top + layout.padding.top + glyph.y,
      };

      draw_glyph_clip_image(
        content,
        canvas,
        style,
        context.transform,
        inline_offset,
        clip_image,
      );
    }
  }

  for glyph in glyph_run.positioned_glyphs() {
    let Some(content) = resolved_glyphs.get(&glyph.id) else {
      continue;
    };

    let inline_offset = Point {
      x: layout.border.left + layout.padding.left + glyph.x,
      y: layout.border.top + layout.padding.top + glyph.y,
    };

    draw_glyph(
      content,
      canvas,
      style,
      context.transform,
      inline_offset,
      glyph_run.style().brush.color,
      palette,
    )?;
  }

  if decoration_line.contains(&TextDecorationLine::LineThrough) {
    let size = glyph_run.run().font_size() / 18.0;
    let offset = glyph_run.baseline() - metrics.strikethrough_offset;

    draw_decoration(
      canvas,
      glyph_run,
      glyph_run.style().brush.decoration_color,
      offset,
      size,
      layout,
      context.transform,
    );
  }

  Ok(())
}

pub(crate) fn draw_inline_box<N: Node<N>>(
  inline_box: &PositionedInlineBox,
  item: &InlineBoxItem<'_, '_, N>,
  canvas: &mut Canvas,
  transform: Affine,
) -> Result<()> {
  if item.context.style.opacity.0 == 0.0 {
    return Ok(());
  }

  let context = RenderContext {
    transform: transform * Affine::translation(inline_box.x, inline_box.y),
    ..item.context.clone()
  };
  let layout = item.into();

  item.node.draw_outset_box_shadow(&context, canvas, layout)?;
  item.node.draw_background(&context, canvas, layout)?;
  item.node.draw_inset_box_shadow(&context, canvas, layout)?;
  item.node.draw_border(&context, canvas, layout)?;
  item.node.draw_content(&context, canvas, layout)?;
  item.node.draw_outline(&context, canvas, layout)?;

  Ok(())
}

pub(crate) fn draw_inline_layout(
  context: &RenderContext,
  canvas: &mut Canvas,
  layout: Layout,
  inline_layout: InlineLayout,
  font_style: &SizedFontStyle,
) -> Result<Vec<PositionedInlineBox>> {
  let clip_image = if context.style.background_clip == BackgroundClip::Text {
    let layers = collect_background_layers(context, layout.size)?;

    rasterize_layers(
      layers,
      layout.size.map(|x| x as u32),
      context,
      BorderProperties::default(),
      Affine::IDENTITY,
      &mut canvas.mask_memory,
    )
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
            clip_image.as_ref(),
          )?;
        }
        PositionedLayoutItem::InlineBox(mut inline_box) => {
          fix_inline_box_y(&mut inline_box.y, line.metrics());
          positioned_inline_boxes.push(inline_box)
        }
      }
    }
  }

  Ok(positioned_inline_boxes)
}

// https://github.com/linebender/parley/blob/d7ed9b1ec844fa5a9ed71b84552c603dae3cab18/parley/src/layout/line.rs#L261C28-L261C61
pub(crate) fn fix_inline_box_y(y: &mut f32, metrics: &LineMetrics) {
  *y += metrics.line_height - metrics.baseline;
}
