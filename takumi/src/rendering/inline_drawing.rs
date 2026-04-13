use std::collections::HashMap;

use parley::{GlyphRun, Line, LineMetrics, PositionedInlineBox, PositionedLayoutItem};
use skrifa::{FontRef, MetadataProvider};
use taffy::{Layout, Point};

use crate::{
  Result,
  layout::{
    inline::{InlineBoxItem, InlineBrush, InlineLayout, ProcessedInlineSpan},
    style::{
      Affine, BackgroundClip, BlendMode, BorderStyle, SizedFontStyle, SizedTextDecorationThickness,
      TextDecorationLines,
    },
    tree::LayoutTree,
  },
  rendering::{
    BorderProperties, Canvas, Command, PaintSource, PathBuilder, RenderContext, Stroke,
    collect_background_layers, draw_decoration, draw_glyph, draw_glyph_clip_image,
    draw_glyph_text_shadow, rasterize_layers, release_rasterized_background_tile,
    render::render_node, render_mask,
  },
  resources::font::{FontError, ResolvedGlyph},
};
use taffy::{AvailableSpace, geometry::Size};

#[derive(Clone, Copy)]
struct InlineOutlineRect {
  span_id: u64,
  line_index: usize,
  x: f32,
  y: f32,
  width: f32,
  height: f32,
}

fn draw_glyph_run_under_overline(
  glyph_run: &GlyphRun<'_, InlineBrush>,
  _resolved_glyphs: &HashMap<u32, ResolvedGlyph>,
  canvas: &mut Canvas,
  layout: Layout,
  context: &RenderContext,
  baseline_shift: f32,
) -> Result<()> {
  let brush = &glyph_run.style().brush;

  let run = glyph_run.run();
  let metrics = run.metrics();

  if brush
    .decoration_line
    .contains(TextDecorationLines::UNDERLINE)
  {
    let offset = glyph_run.baseline() + baseline_shift - metrics.underline_offset;
    let size = match brush.decoration_thickness {
      SizedTextDecorationThickness::Value(v) => v,
      SizedTextDecorationThickness::FromFont => metrics.underline_size,
    };

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

  if brush
    .decoration_line
    .contains(TextDecorationLines::OVERLINE)
  {
    draw_decoration(
      canvas,
      glyph_run,
      glyph_run.style().brush.decoration_color,
      glyph_run.baseline() + baseline_shift - metrics.ascent - metrics.underline_offset,
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
  baseline_shift: f32,
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
  let offset = glyph_run.baseline() + baseline_shift - metrics.strikethrough_offset;

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

fn draw_outline_island(
  outline_rects: &[InlineOutlineRect],
  canvas: &mut Canvas,
  spans: &[ProcessedInlineSpan<'_, '_>],
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

fn draw_glyph_run_content(
  style: &SizedFontStyle,
  glyph_run: &GlyphRun<'_, InlineBrush>,
  resolved_glyphs: &HashMap<u32, ResolvedGlyph>,
  canvas: &mut Canvas,
  glyph_offset: Point<f32>,
  context: &RenderContext,
  clip_image: Option<PaintSource<'_>>,
) -> Result<()> {
  let run = glyph_run.run();

  let font = FontRef::from_index(run.font().data.as_ref(), run.font().index)
    .map_err(|_| FontError::InvalidFontIndex)?;
  let palettes = font.color_palettes();
  let palette = palettes.get(0);

  if let Some(clip_image) = clip_image {
    for glyph in glyph_run.positioned_glyphs() {
      let Some(content) = resolved_glyphs.get(&glyph.id) else {
        continue;
      };

      let inline_offset = Point {
        x: glyph_offset.x + glyph.x,
        y: glyph_offset.y + glyph.y,
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
      x: glyph_offset.x + glyph.x,
      y: glyph_offset.y + glyph.y,
    };

    draw_glyph(
      content,
      canvas,
      style,
      context.transform,
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
  layout: Layout,
  context: &RenderContext,
  baseline_shift: f32,
) -> Result<()> {
  for glyph in glyph_run.positioned_glyphs() {
    let Some(content) = resolved_glyphs.get(&glyph.id) else {
      continue;
    };

    let inline_offset = Point {
      x: layout.border.left + layout.padding.left + glyph.x,
      y: layout.border.top + layout.padding.top + glyph.y + baseline_shift,
    };

    draw_glyph_text_shadow(content, canvas, style, context.transform, inline_offset)?;
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
  run.metrics().x_height
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ResolvedLineMetrics {
  pub(crate) resolved_ascent: f32,
  pub(crate) resolved_descent: f32,
  pub(crate) resolved_line_height: f32,
  pub(crate) resolved_baseline: f32,
  pub(crate) resolved_line_top: f32,
  pub(crate) resolved_line_bottom: f32,
  pub(crate) baseline_shift: f32,
}

fn quantized_baseline(line_height: f32, ascent: f32, descent: f32) -> f32 {
  let leading = line_height - (ascent.round() + descent.round());
  ascent.round() + (leading * 0.5).round()
}

#[inline]
fn inline_metrics_debug_enabled() -> bool {
  #[cfg(not(target_arch = "wasm32"))]
  {
    std::env::var_os("TAKUMI_DEBUG_INLINE").is_some()
  }
  #[cfg(target_arch = "wasm32")]
  {
    false
  }
}

fn parent_baseline_offset_for_box(
  line: &Line<'_, InlineBrush>,
  item: &InlineBoxItem<'_, '_>,
  inline_box: &PositionedInlineBox,
  effective_parent_x_height: Option<f32>,
  effective_parent_text_metrics: Option<(f32, f32)>,
) -> f32 {
  let baseline_in_item = item
    .baseline_offset
    .unwrap_or(inline_box.height)
    .clamp(0.0, inline_box.height);
  let mut top = 0.0;
  item.vertical_align.apply(
    &mut top,
    line.metrics(),
    inline_box.height,
    Some(baseline_in_item),
    effective_parent_x_height,
    effective_parent_text_metrics,
  );
  top - (line.metrics().baseline - baseline_in_item)
}

pub(crate) fn effective_parent_x_height_for_line(
  line: &Line<'_, InlineBrush>,
  parent_x_height: Option<f32>,
) -> Option<f32> {
  if parent_x_height.is_some() {
    return parent_x_height;
  }

  let mut text_ascent_max = 0.0_f32;
  for item in line.items() {
    if let PositionedLayoutItem::GlyphRun(glyph_run) = item {
      text_ascent_max = text_ascent_max.max(glyph_run.run().metrics().ascent);
    }
  }

  (text_ascent_max > 0.0).then_some(text_ascent_max * 0.5)
}

pub(crate) fn effective_parent_text_metrics_for_line(
  line: &Line<'_, InlineBrush>,
) -> Option<(f32, f32)> {
  let mut text_ascent_max = 0.0_f32;
  let mut text_descent_max = 0.0_f32;
  let mut has_glyph = false;
  for item in line.items() {
    if let PositionedLayoutItem::GlyphRun(glyph_run) = item {
      let metrics = glyph_run.run().metrics();
      text_ascent_max = text_ascent_max.max(metrics.ascent);
      text_descent_max = text_descent_max.max(metrics.descent);
      has_glyph = true;
    }
  }
  has_glyph.then_some((text_ascent_max, text_descent_max))
}

pub(crate) fn resolve_inline_line_metrics(
  inline_layout: &InlineLayout,
  spans: &[ProcessedInlineSpan<'_, '_>],
  parent_x_height: Option<f32>,
) -> Vec<ResolvedLineMetrics> {
  let mut result = Vec::with_capacity(inline_layout.lines().count());
  let mut cumulative_flow_shift = 0.0_f32;
  let mut next_line_top: Option<f32> = None;

  for (line_index, line) in inline_layout.lines().enumerate() {
    let effective_parent_x_height = effective_parent_x_height_for_line(&line, parent_x_height);
    let effective_parent_text_metrics = effective_parent_text_metrics_for_line(&line);

    let mut resolved_ascent = 0.0_f32;
    let mut resolved_descent = 0.0_f32;
    let mut has_item = false;

    for item in line.items() {
      match item {
        PositionedLayoutItem::GlyphRun(glyph_run) => {
          let metrics = glyph_run.run().metrics();
          resolved_ascent = resolved_ascent.max(metrics.ascent);
          resolved_descent = resolved_descent.max(metrics.descent);
          has_item = true;
        }
        PositionedLayoutItem::InlineBox(inline_box) => {
          let Some(ProcessedInlineSpan::Box(item)) = spans.get(inline_box.id as usize) else {
            continue;
          };
          let baseline_in_item = item
            .baseline_offset
            .unwrap_or(inline_box.height)
            .clamp(0.0, inline_box.height);
          let parent_baseline_offset = parent_baseline_offset_for_box(
            &line,
            item,
            &inline_box,
            effective_parent_x_height,
            effective_parent_text_metrics,
          );
          let ascent_contrib = (baseline_in_item - parent_baseline_offset).max(0.0);
          let descent_contrib =
            (inline_box.height - baseline_in_item + parent_baseline_offset).max(0.0);
          if inline_metrics_debug_enabled() {
            eprintln!(
              "[inline-metrics][line={line_index}][box={}] h={:.3} baseline_in_item={:.3} parent_baseline_offset={:.3} ascent_contrib={:.3} descent_contrib={:.3}",
              inline_box.id,
              inline_box.height,
              baseline_in_item,
              parent_baseline_offset,
              ascent_contrib,
              descent_contrib
            );
          }
          resolved_ascent = resolved_ascent.max(ascent_contrib);
          resolved_descent = resolved_descent.max(descent_contrib);
          has_item = true;
        }
      }
    }

    if !has_item {
      resolved_ascent = line.metrics().ascent;
      resolved_descent = line.metrics().descent;
    }

    let original_line_height = (line.metrics().max_coord - line.metrics().min_coord).max(0.0);
    let resolved_line_height = original_line_height
      .max(line.metrics().line_height)
      .max(resolved_ascent + resolved_descent);
    let resolved_baseline_in_line =
      quantized_baseline(resolved_line_height, resolved_ascent, resolved_descent);
    let resolved_line_top =
      next_line_top.unwrap_or(line.metrics().min_coord + cumulative_flow_shift);
    let resolved_line_bottom = resolved_line_top + resolved_line_height;
    let resolved_baseline = resolved_line_top + resolved_baseline_in_line;
    let baseline_shift = if (resolved_baseline - line.metrics().baseline).is_finite() {
      resolved_baseline - line.metrics().baseline
    } else {
      0.0
    };

    if inline_metrics_debug_enabled() {
      eprintln!(
        "[inline-metrics][line={line_index}] original(min={:.3},max={:.3},baseline={:.3},line_height={:.3},ascent={:.3},descent={:.3}) resolved(top={:.3},bottom={:.3},baseline={:.3},line_height={:.3},ascent={:.3},descent={:.3},baseline_shift={:.3})",
        line.metrics().min_coord,
        line.metrics().max_coord,
        line.metrics().baseline,
        line.metrics().line_height,
        line.metrics().ascent,
        line.metrics().descent,
        resolved_line_top,
        resolved_line_bottom,
        resolved_baseline,
        resolved_line_height,
        resolved_ascent,
        resolved_descent,
        baseline_shift
      );
    }

    result.push(ResolvedLineMetrics {
      resolved_ascent,
      resolved_descent,
      resolved_line_height,
      resolved_baseline,
      resolved_line_top,
      resolved_line_bottom,
      baseline_shift,
    });

    cumulative_flow_shift += resolved_line_height - original_line_height;
    next_line_top = Some(resolved_line_bottom);
  }

  result
}

pub(crate) fn resolved_line_metrics_for_apply(
  line_metrics: &LineMetrics,
  resolved: ResolvedLineMetrics,
) -> LineMetrics {
  let mut adjusted = *line_metrics;
  adjusted.ascent = resolved.resolved_ascent;
  adjusted.descent = resolved.resolved_descent;
  adjusted.baseline = resolved.resolved_baseline;
  adjusted.min_coord = resolved.resolved_line_top;
  adjusted.max_coord = resolved.resolved_line_bottom;
  adjusted.line_height = resolved.resolved_line_height;
  adjusted
}

pub(crate) fn draw_inline_box(
  inline_box: &PositionedInlineBox,
  item: &InlineBoxItem<'_, '_>,
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
      Affine::translation(
        inline_box.x + item.margin.left,
        inline_box.y + item.margin.top,
      ) * transform,
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

  node.draw_outset_box_shadow(&context, canvas, layout)?;
  node.draw_background(&context, canvas, layout)?;
  node.draw_inset_box_shadow(&context, canvas, layout)?;
  node.draw_border(&context, canvas, layout)?;
  node.draw_content(&context, canvas, layout)?;
  node.draw_outline(&context, canvas, layout)?;

  Ok(())
}

pub(crate) fn draw_inline_layout(
  context: &RenderContext,
  canvas: &mut Canvas,
  layout: Layout,
  inline_layout: InlineLayout,
  font_style: &SizedFontStyle,
  spans: &[ProcessedInlineSpan<'_, '_>],
) -> Result<Vec<PositionedInlineBox>> {
  let glyph_runs = collect_glyph_runs(&inline_layout);
  let resolved_glyph_runs = resolve_inline_layout_glyphs(context, &glyph_runs)?;
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
  let parent_x_height = get_parent_x_height(context, font_style);

  let mut positioned_inline_boxes = Vec::new();
  let mut inline_outline_rects = Vec::new();

  let line_vertical_metrics = resolve_inline_line_metrics(&inline_layout, spans, parent_x_height);

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

  // Reference: https://www.w3.org/TR/css-text-decor-3/#painting-order
  for (line_index, line) in inline_layout.lines().enumerate() {
    let baseline_shift = line_vertical_metrics[line_index].baseline_shift;
    let mut resolved_iter = per_line_resolved[line_index].iter();
    for item in line.items() {
      if let PositionedLayoutItem::GlyphRun(glyph_run) = item {
        let Some(resolved_glyphs) = resolved_iter.next() else {
          continue;
        };
        draw_glyph_run_text_shadow(
          font_style,
          &glyph_run,
          resolved_glyphs,
          canvas,
          layout,
          context,
          baseline_shift,
        )?;
      }
    }
  }

  for (line_index, line) in inline_layout.lines().enumerate() {
    let baseline_shift = line_vertical_metrics[line_index].baseline_shift;
    let mut resolved_iter = per_line_resolved[line_index].iter();
    for item in line.items() {
      if let PositionedLayoutItem::GlyphRun(glyph_run) = item {
        let Some(resolved_glyphs) = resolved_iter.next() else {
          continue;
        };
        draw_glyph_run_under_overline(
          &glyph_run,
          resolved_glyphs,
          canvas,
          layout,
          context,
          baseline_shift,
        )?;
      }
    }
  }

  for (line_index, line) in inline_layout.lines().enumerate() {
    let resolved_metrics = line_vertical_metrics[line_index];
    let baseline_shift = resolved_metrics.baseline_shift;
    let line_parent_x_height = effective_parent_x_height_for_line(&line, parent_x_height);
    let line_parent_text_metrics = effective_parent_text_metrics_for_line(&line);
    let mut resolved_iter = per_line_resolved[line_index].iter();

    for item in line.items() {
      match item {
        PositionedLayoutItem::GlyphRun(glyph_run) => {
          let Some(resolved_glyphs) = resolved_iter.next() else {
            continue;
          };
          draw_glyph_run_content(
            font_style,
            &glyph_run,
            resolved_glyphs,
            canvas,
            Point {
              x: layout.border.left + layout.padding.left,
              y: layout.border.top + layout.padding.top + baseline_shift,
            },
            context,
            clip_image_source,
          )?;
          if let Some(outline_rect) = collect_glyph_run_outline_rect(
            &glyph_run,
            layout,
            line_index,
            layout.border.top + layout.padding.top + glyph_run.baseline() + baseline_shift
              - resolved_metrics.resolved_ascent,
            resolved_metrics.resolved_ascent + resolved_metrics.resolved_descent,
          ) {
            inline_outline_rects.push(outline_rect);
          }
        }
        PositionedLayoutItem::InlineBox(mut inline_box) => {
          let item_index = inline_box.id as usize;
          let adjusted_line_metrics =
            resolved_line_metrics_for_apply(line.metrics(), resolved_metrics);

          if let Some(ProcessedInlineSpan::Box(item)) = spans.get(item_index) {
            item.vertical_align.apply(
              &mut inline_box.y,
              &adjusted_line_metrics,
              inline_box.height,
              item.baseline_offset,
              line_parent_x_height,
              line_parent_text_metrics,
            );
            if inline_metrics_debug_enabled() {
              eprintln!(
                "[inline-draw][line={line_index}][box={item_index}] x={:.3} y={:.3} w={:.3} h={:.3} baseline_offset={:?}",
                inline_box.x,
                inline_box.y,
                inline_box.width,
                inline_box.height,
                item.baseline_offset
              );
            }
          }
          positioned_inline_boxes.push(inline_box)
        }
      }
    }
  }

  draw_merged_outline_rects(inline_outline_rects, canvas, spans, context.transform);

  for (line_index, line) in inline_layout.lines().enumerate() {
    let baseline_shift = line_vertical_metrics[line_index].baseline_shift;
    for item in line.items() {
      if let PositionedLayoutItem::GlyphRun(glyph_run) = item {
        draw_glyph_run_line_through(&glyph_run, canvas, layout, context, baseline_shift)?;
      }
    }
  }

  if let Some(tile) = clip_image {
    release_rasterized_background_tile(tile, &mut canvas.buffer_pool);
  }

  Ok(positioned_inline_boxes)
}
