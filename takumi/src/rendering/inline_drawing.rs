use std::collections::HashMap;

use image::{GenericImageView, Rgba};
use parley::{GlyphRun, PositionedInlineBox, PositionedLayoutItem};
use swash::FontRef;
use taffy::{Layout, Point};
use zeno::{Command, PathBuilder, Stroke};

use crate::{
  Result,
  layout::{
    inline::{InlineBoxItem, InlineBrush, InlineLayout, ProcessedInlineSpan},
    node::Node,
    style::{
      Affine, BackgroundClip, BlendMode, BorderStyle, Color, ImageScalingAlgorithm, SizedFontStyle,
      SizedTextDecorationThickness, TextDecorationLines, TextDecorationSkipInk,
    },
    tree::LayoutTree,
  },
  rendering::{
    BackgroundTile, BorderProperties, Canvas, ColorTile, RenderContext, collect_background_layers,
    collect_outline_paths, draw_decoration, draw_glyph, draw_glyph_clip_image,
    draw_glyph_text_shadow, mask_index_from_coord, overlay_area, rasterize_layers,
    render::render_node,
  },
  resources::font::{FontError, ResolvedGlyph},
};
use taffy::{AvailableSpace, geometry::Size};

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
  alpha: Box<[u8]>,
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
        let (mask, placement) =
          canvas
            .mask_memory
            .render(&paths, None, None, &mut canvas.buffer_pool);

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
          alpha: mask.to_vec().into_boxed_slice(),
        };
        canvas.buffer_pool.release(mask);
        data
      }
    };

    bounds.insert(*glyph_id, glyph);
  }

  bounds
}

fn draw_decoration_segment(
  canvas: &mut Canvas,
  color: Color,
  start_x: f32,
  end_x: f32,
  y: f32,
  height: f32,
  transform: Affine,
) {
  if end_x <= start_x {
    return;
  }

  let x = start_x.floor();
  let width = (end_x.ceil() - x) as u32;

  let tile = ColorTile {
    color: color.into(),
    width,
    height: height as u32,
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
        canvas, color, current_x, skip_start, line_top, size, transform,
      );
    }
    current_x = current_x.max(skip_end);
  }

  if run_end_x > current_x {
    draw_decoration_segment(
      canvas, color, current_x, run_end_x, line_top, size, transform,
    );
  }
}

fn draw_glyph_run_under_overline(
  glyph_run: &GlyphRun<'_, InlineBrush>,
  resolved_glyphs: &HashMap<u32, ResolvedGlyph>,
  canvas: &mut Canvas,
  layout: Layout,
  context: &RenderContext,
) -> Result<()> {
  let brush = &glyph_run.style().brush;

  let run = glyph_run.run();
  let metrics = run.metrics();

  if brush
    .decoration_line
    .contains(TextDecorationLines::UNDERLINE)
  {
    let offset = glyph_run.baseline() - metrics.underline_offset;
    let size = match brush.decoration_thickness {
      SizedTextDecorationThickness::Value(v) => v,
      SizedTextDecorationThickness::FromFont => metrics.underline_size,
    };

    if context.transform.only_translation()
      && brush.decoration_skip_ink != TextDecorationSkipInk::None
    {
      let glyph_bounds_cache = build_glyph_bounds_cache(canvas, resolved_glyphs);

      draw_underline_with_skip_ink(
        canvas,
        glyph_run,
        &glyph_bounds_cache,
        brush.decoration_color,
        offset,
        size,
        layout,
        context.transform,
      );
    } else {
      draw_decoration(
        canvas,
        glyph_run,
        brush.decoration_color,
        offset,
        size,
        layout,
        context.transform,
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
      glyph_run.baseline() - metrics.ascent - metrics.underline_offset,
      match brush.decoration_thickness {
        SizedTextDecorationThickness::Value(v) => v,
        SizedTextDecorationThickness::FromFont => metrics.underline_size,
      },
      layout,
      context.transform,
    );
  }

  Ok(())
}

fn draw_glyph_run_line_through(
  glyph_run: &GlyphRun<'_, InlineBrush>,
  canvas: &mut Canvas,
  layout: Layout,
  context: &RenderContext,
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

  Ok(())
}

fn collect_glyph_run_outline_rect(
  glyph_run: &GlyphRun<'_, InlineBrush>,
  layout: Layout,
  line_index: usize,
  line_top: f32,
  line_height: f32,
) -> Option<InlineOutlineRect> {
  let span_id = glyph_run.style().brush.source_span_id?;

  Some(InlineOutlineRect {
    span_id,
    line_index,
    x: layout.border.left + layout.padding.left + glyph_run.offset(),
    y: line_top,
    width: glyph_run.advance(),
    height: line_height,
  })
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

fn draw_outline_island<N: Node<N>>(
  outline_rects: &[InlineOutlineRect],
  canvas: &mut Canvas,
  spans: &[ProcessedInlineSpan<'_, '_, N>],
  transform: Affine,
) {
  let Some(first_rect) = outline_rects.first().copied() else {
    return;
  };
  let Some(ProcessedInlineSpan::Text { style, .. }) = spans.get(first_rect.span_id as usize) else {
    return;
  };

  let width = style.outline_width;
  if width == 0.0 || style.outline_style == BorderStyle::None {
    return;
  }

  let expansion = style.outline_offset + width / 2.0;
  let mut path = Vec::with_capacity(outline_rects.len() * 6);
  append_outline_contour(&mut path, outline_rects, expansion);
  if path.is_empty() {
    return;
  }

  let stroke = Stroke::new(width);
  let (mask, placement) = canvas.mask_memory.render(
    &path,
    Some(transform),
    Some(stroke.into()),
    &mut canvas.buffer_pool,
  );

  overlay_area(
    &mut canvas.image,
    Point {
      x: placement.left as f32,
      y: placement.top as f32,
    },
    Size {
      width: placement.width,
      height: placement.height,
    },
    BlendMode::Normal,
    &canvas.constrains,
    |x, y| {
      let alpha = mask[mask_index_from_coord(x, y, placement.width)];
      if alpha == 0 {
        return Color::transparent().into();
      }

      let mut pixel: image::Rgba<u8> = style.outline_color.into();
      pixel.0[3] = ((pixel.0[3] as u16 * alpha as u16) / 255) as u8;
      pixel
    },
  );

  canvas.buffer_pool.release(mask);
}

fn draw_merged_outline_rects<N: Node<N>>(
  mut outline_rects: Vec<InlineOutlineRect>,
  canvas: &mut Canvas,
  spans: &[ProcessedInlineSpan<'_, '_, N>],
  transform: Affine,
) {
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
    draw_outline_island(&island, canvas, spans, transform);
  }
}

fn draw_glyph_run_content<I: GenericImageView<Pixel = Rgba<u8>>>(
  style: &SizedFontStyle,
  glyph_run: &GlyphRun<'_, InlineBrush>,
  resolved_glyphs: &HashMap<u32, ResolvedGlyph>,
  canvas: &mut Canvas,
  layout: Layout,
  context: &RenderContext,
  clip_image: Option<&I>,
) -> Result<()> {
  let run = glyph_run.run();

  let font = FontRef::from_index(run.font().data.as_ref(), run.font().index as usize)
    .ok_or(FontError::InvalidFontIndex)?;
  let palette = font.color_palettes().next();

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
      )?;
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

  Ok(())
}

fn draw_glyph_run_text_shadow(
  style: &SizedFontStyle,
  glyph_run: &GlyphRun<'_, InlineBrush>,
  resolved_glyphs: &HashMap<u32, ResolvedGlyph>,
  canvas: &mut Canvas,
  layout: Layout,
  context: &RenderContext,
) -> Result<()> {
  for glyph in glyph_run.positioned_glyphs() {
    let Some(content) = resolved_glyphs.get(&glyph.id) else {
      continue;
    };

    let inline_offset = Point {
      x: layout.border.left + layout.padding.left + glyph.x,
      y: layout.border.top + layout.padding.top + glyph.y,
    };

    draw_glyph_text_shadow(content, canvas, style, context.transform, inline_offset)?;
  }

  Ok(())
}

fn glyph_runs(
  inline_layout: &InlineLayout,
) -> impl Iterator<Item = GlyphRun<'_, InlineBrush>> + '_ {
  inline_layout.lines().flat_map(|line| {
    line.items().filter_map(|item| {
      if let PositionedLayoutItem::GlyphRun(glyph_run) = item {
        Some(glyph_run)
      } else {
        None
      }
    })
  })
}

fn glyph_runs_with_resolved<'a>(
  inline_layout: &'a InlineLayout,
  resolved_glyph_runs: &'a [HashMap<u32, ResolvedGlyph>],
) -> impl Iterator<Item = (GlyphRun<'a, InlineBrush>, &'a HashMap<u32, ResolvedGlyph>)> + 'a {
  glyph_runs(inline_layout).zip(resolved_glyph_runs.iter())
}

fn resolve_inline_layout_glyphs(
  context: &RenderContext,
  inline_layout: &InlineLayout,
) -> Result<Vec<HashMap<u32, ResolvedGlyph>>> {
  glyph_runs(inline_layout)
    .map(|glyph_run| {
      let run = glyph_run.run();
      let glyph_ids = glyph_run.positioned_glyphs().map(|glyph| glyph.id);
      let font = FontRef::from_index(run.font().data.as_ref(), run.font().index as usize)
        .ok_or(FontError::InvalidFontIndex)?;

      Ok(
        context
          .global
          .font_context
          .resolve_glyphs(&glyph_run, font, glyph_ids),
      )
    })
    .collect()
}

pub(crate) fn get_parent_x_height(
  context: &RenderContext,
  font_style: &SizedFontStyle,
) -> Option<f32> {
  let (layout, _) = context
    .global
    .font_context
    .tree_builder(font_style.into(), |builder| {
      builder.push_text("x");
    });

  let run = layout.lines().next()?.runs().next()?;
  let font = run.font();
  let font_ref = FontRef::from_index(font.data.as_ref(), font.index as usize)?;

  let metrics = font_ref.metrics(run.normalized_coords());
  let units_per_em = metrics.units_per_em as f32;
  if units_per_em == 0.0 {
    return None;
  }
  let scale = run.font_size() / units_per_em;
  Some(metrics.x_height * scale)
}

pub(crate) fn draw_inline_box<N: Node<N>>(
  inline_box: &PositionedInlineBox,
  item: &InlineBoxItem<'_, '_, N>,
  canvas: &mut Canvas,
  transform: Affine,
) -> Result<()> {
  if item.render_node.context.style.opacity.0 == 0.0 {
    return Ok(());
  }

  if item.render_node.is_inline_atomic_container() {
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
      transform
        * Affine::translation(
          inline_box.x + item.margin.left,
          inline_box.y + item.margin.top,
        ),
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
    transform: transform * Affine::translation(inline_box.x, inline_box.y),
    ..item.render_node.context.clone()
  };
  let layout = item.into();

  node.draw_outset_box_shadow(&context, canvas, layout)?;
  node.draw_background(&context, canvas, layout)?;
  node.draw_inset_box_shadow(&context, canvas, layout)?;
  node.draw_border(&context, canvas, layout)?;
  node.draw_content(&context, canvas, layout)?;
  node.draw_outline(&context, canvas, layout)?;

  Ok(())
}

pub(crate) fn draw_inline_layout<N: Node<N>>(
  context: &RenderContext,
  canvas: &mut Canvas,
  layout: Layout,
  inline_layout: InlineLayout,
  font_style: &SizedFontStyle,
  spans: &[ProcessedInlineSpan<'_, '_, N>],
) -> Result<Vec<PositionedInlineBox>> {
  let resolved_glyph_runs = resolve_inline_layout_glyphs(context, &inline_layout)?;
  let clip_image = if context.style.background_clip == BackgroundClip::Text {
    let layers = collect_background_layers(context, layout.size, &mut canvas.buffer_pool)?;

    rasterize_layers(
      layers,
      layout.size.map(|x| x as u32),
      context,
      BorderProperties::default(),
      Affine::IDENTITY,
      &mut canvas.mask_memory,
      &mut canvas.buffer_pool,
    )?
  } else {
    None
  };

  let mut positioned_inline_boxes = Vec::new();
  let mut inline_outline_rects = Vec::new();

  // Reference: https://www.w3.org/TR/css-text-decor-3/#painting-order
  for (glyph_run, resolved_glyphs) in glyph_runs_with_resolved(&inline_layout, &resolved_glyph_runs)
  {
    draw_glyph_run_text_shadow(
      font_style,
      &glyph_run,
      resolved_glyphs,
      canvas,
      layout,
      context,
    )?;
  }

  for (glyph_run, resolved_glyphs) in glyph_runs_with_resolved(&inline_layout, &resolved_glyph_runs)
  {
    draw_glyph_run_under_overline(&glyph_run, resolved_glyphs, canvas, layout, context)?;
  }

  let parent_x_height = get_parent_x_height(context, font_style);
  let mut glyph_runs_with_resolved = glyph_runs_with_resolved(&inline_layout, &resolved_glyph_runs);
  for (line_index, line) in inline_layout.lines().enumerate() {
    let line_metrics = line.metrics();

    for item in line.items() {
      match item {
        PositionedLayoutItem::GlyphRun(glyph_run) => {
          let Some((_, resolved_glyphs)) = glyph_runs_with_resolved.next() else {
            continue;
          };
          draw_glyph_run_content(
            font_style,
            &glyph_run,
            resolved_glyphs,
            canvas,
            layout,
            context,
            clip_image.as_ref(),
          )?;
          if let Some(outline_rect) = collect_glyph_run_outline_rect(
            &glyph_run,
            layout,
            line_index,
            layout.border.top + layout.padding.top + glyph_run.baseline() - line_metrics.ascent,
            line_metrics.line_height,
          ) {
            inline_outline_rects.push(outline_rect);
          }
        }
        PositionedLayoutItem::InlineBox(mut inline_box) => {
          let item_index = inline_box.id as usize;

          if let Some(ProcessedInlineSpan::Box(item)) = spans.get(item_index) {
            item.vertical_align.apply(
              &mut inline_box.y,
              line.metrics(),
              inline_box.height,
              parent_x_height,
            );
          }
          positioned_inline_boxes.push(inline_box)
        }
      }
    }
  }

  draw_merged_outline_rects(inline_outline_rects, canvas, spans, context.transform);

  for glyph_run in glyph_runs(&inline_layout) {
    draw_glyph_run_line_through(&glyph_run, canvas, layout, context)?;
  }

  if let Some(BackgroundTile::Image(image)) = clip_image {
    canvas.buffer_pool.release_image(image);
  }

  Ok(positioned_inline_boxes)
}
