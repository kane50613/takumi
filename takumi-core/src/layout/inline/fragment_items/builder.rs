//! Blink's [`FragmentItemsBuilder`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/inline/fragment_items_builder.cc):
//! the items the inline layout port's line boxes place, as fragment items for painting.

use std::{ops::Range, rc::Rc};

use rustc_hash::FxHashMap;
use skrifa::FontRef;

use super::{BackgroundItem, ContentBox, FragmentItems, GlyphStore, Ink, InkBox, RunPlacement};
use crate::{
  context::RenderContext,
  fonts::{
    font::Font,
    shaping::{harfbuzz_shaper::HarfBuzzShaper, shape_result_view::ShapeResultView},
    simple_font_data::SimpleFontData,
  },
  geometry::{AvailableSpace, ComputedLayout, Point, Size},
  layout::inline::{
    atomic_inline::BoxAtomicInline,
    background::{SpanFragment, resolve_span_fragments},
    decorations::{BoxFont, DecorationPlacement},
    inline_item::{InlineItem, InlineItemType},
    inline_layout_algorithm::LineBoxFragment,
    inline_node_data::InlineNodeData,
    items::{DecorationLink, InlineSpans, ProcessedInlineSpan},
    line_breaker::AtomicInlineLayout,
    logical_line_item::{LineFragment, LogicalLineItem},
    metrics::VisualInlineBox,
    runs::{HangingWhitespace, PositionedGlyph, RunMetrics, ShapedRun, glyph_skips_ink},
    text_fit::LineScaleState,
  },
  layout_unit::LayoutUnit,
  resources::glyph::{BOLD_THRESHOLD, synthesis_embolden_strength},
  scene::text_ink_reach,
  style::{Affine, Direction},
};

/// What a line gives the texts on it: where its content box sits, its baseline, its root box's
/// font, and the baselines of the spans on it.
struct TextLine<'l> {
  content: Point<f32>,
  baseline: f32,
  root_font: BoxFont,
  span_baselines: &'l FxHashMap<usize, f32>,
}

/// What the texts of an inline formatting context draw from: its items, their spans, and its
/// context.
#[derive(Clone, Copy)]
struct TextSource<'s, 'c> {
  items_data: &'s InlineNodeData<'c>,
  spans: &'s InlineSpans<'c>,
  context: &'s RenderContext,
}

/// A glyph of a text item, in the order Blink's `ForEachGlyph` visits them.
struct ItemGlyph {
  character_index: u32,
  glyph: u16,
  /// The glyph's origin past the item's start, its offset included.
  x: f32,
  y: f32,
  /// The pen position before the glyph, past the item's start.
  advance: f32,
  font_data: Option<Rc<SimpleFontData>>,
}

impl FragmentItems {
  /// Blink's `FragmentItemsBuilder::AddLine` for each of `lines`, laid out in the content box of
  /// `layout`, its runs and boxes painting with `spans`, which takes the boxes' painted sizes.
  pub(crate) fn from_lines<'c>(
    items_data: &InlineNodeData<'c>,
    lines: &[LineBoxFragment<'_, 'c>],
    spans: &mut InlineSpans<'c>,
    layout: ComputedLayout,
    context: &RenderContext,
  ) -> Self {
    let mut items = Self {
      content_box: ContentBox::of(layout),
      text: items_data.text_content.clone(),
      ellipsized: false,
      line_boxes: Vec::new(),
      baselines: None,
      ink: Ink::default(),
      span_ink_reach: 0.0,
      text_items: Vec::new(),
      faces: Vec::new(),
      glyphs: GlyphStore::default(),
      inline_boxes: Vec::new(),
      outline_rects: Vec::new(),
      backgrounds: Vec::new(),
    };
    let mut ink = Ink::default();
    let mut first_baseline = None;
    let mut last_baseline = None;
    let content = layout.content_box_offset();
    let root_font = BoxFont::of(context);

    for line in lines {
      let Some(line_top) = line.line_box_bfc_block_offset else {
        continue;
      };
      let line_top = line_top.to_float();
      let line_left = line.bfc_line_offset.to_float();
      let baseline = line_top + line.metrics.ascent.to_float();
      // Where each span on the line has its baseline, for the decorations it paints.
      let mut span_baselines: FxHashMap<usize, f32> = FxHashMap::default();

      items
        .line_boxes
        .push((line_top, line_top + line.block_size.to_float()));
      first_baseline.get_or_insert(baseline);
      last_baseline = Some(baseline);

      for child in &line.items.children {
        if child.is_hidden_due_to_layout {
          continue;
        }

        let origin = Point {
          x: line_left + child.rect.offset.inline_offset.to_float(),
          y: line_top + child.rect.offset.block_offset.to_float(),
        };

        if let Some(node) = child.out_of_flow_positioned_box
          && let Some(span) = spans.span_of(node)
        {
          items.inline_boxes.push(VisualInlineBox {
            id: span as u64,
            x: origin.x,
            y: origin.y,
            width: 0.0,
            height: 0.0,
            line_baseline: Some(baseline),
            kind: node.inline_box_kind(),
          });

          continue;
        }

        match &child.layout_result {
          Some(LineFragment::AtomicInline {
            node,
            layout_result,
          }) => {
            let Some(span) = spans.span_of(node) else {
              continue;
            };
            let Some(ProcessedInlineSpan::Box(item)) = spans.spans.get_mut(span) else {
              continue;
            };
            let size = item.paint_border_box(layout_result.inline_size, layout_result.block_size);
            let margin = item.margin;

            item.baseline_offset = layout_result
              .baseline
              .map(|baseline| baseline.to_float() + margin.top);
            items.push_inline_box(
              &mut ink.content,
              VisualInlineBox {
                id: span as u64,
                x: origin.x - margin.left,
                y: origin.y - margin.top,
                width: size.width,
                height: size.height,
                line_baseline: Some(baseline),
                kind: node.inline_box_kind(),
              },
            );
          }
          Some(LineFragment::Float {
            node,
            positioned_float,
          }) => {
            let Some(span) = spans.span_of(node) else {
              continue;
            };
            let Some(ProcessedInlineSpan::Box(item)) = spans.spans.get_mut(span) else {
              continue;
            };
            let result = positioned_float.layout_result;
            let size = item.paint_border_box(result.inline_size, result.block_size);
            let margin = item.margin;

            items.push_inline_box(
              &mut ink.content,
              VisualInlineBox {
                id: span as u64,
                x: positioned_float.bfc_offset.line_offset.to_float() - margin.left,
                y: positioned_float.bfc_offset.block_offset.to_float() - margin.top,
                width: size.width,
                height: size.height,
                line_baseline: None,
                kind: node.inline_box_kind(),
              },
            );
          }
          Some(LineFragment::InlineBox(fragment)) => {
            let Some(decoration) = spans.decoration_of(fragment.node) else {
              continue;
            };
            let ascent = items_data
              .fonts
              .get(&fragment.node.context)
              .primary_font_metrics()
              .map_or(0.0, |metrics| metrics.font_height().ascent.to_float());

            span_baselines.insert(
              decoration.decoration.id,
              origin.y
                + (fragment.borders.line_over + fragment.padding.line_over).to_float()
                + ascent,
            );
          }
          None => {
            if child.shape_result.is_some() {
              items.ellipsized |= child.inline_item.is_none();
              let text_line = TextLine {
                content,
                baseline,
                root_font,
                span_baselines: &span_baselines,
              };

              let source = TextSource {
                items_data,
                spans,
                context,
              };

              items.push_text(&source, child, origin, &text_line, &mut ink);
            }
          }
        }
      }
    }

    items.place_outside_list_markers(items_data, lines, spans, &mut ink.content);
    let span_fragments = line_span_fragments(lines, spans, content);

    let (backgrounds, outline_rects) = resolve_span_fragments(&span_fragments);

    for background in &backgrounds {
      InkBox::include(
        &mut ink.content,
        Point::new(background.x - content.x, background.y - content.y),
        Size::new(background.width, background.height),
      );
    }

    items.backgrounds = backgrounds.into_iter().map(BackgroundItem::of).collect();
    items.outline_rects = outline_rects;
    items.span_ink_reach = spans
      .spans
      .iter()
      .filter_map(ProcessedInlineSpan::chain)
      .flat_map(|chain| chain.ancestors())
      .map(|link| link.decoration.reach())
      .chain(spans.spans.iter().filter_map(|span| match span {
        ProcessedInlineSpan::Text { style, .. } => Some(text_ink_reach(style)),
        _ => None,
      }))
      .fold(0.0, f32::max);
    items.baselines = first_baseline.zip(last_baseline);
    items.ink = ink;
    items.inline_boxes.sort_by_key(|inline_box| inline_box.id);
    items.shrink_to_fit();
    items
  }

  /// Blink's `FragmentItemsBuilder::AddText`: `child`'s glyphs at `origin`, the top of its text,
  /// a run per font.
  fn push_text<'c>(
    &mut self,
    source: &TextSource<'_, 'c>,
    child: &LogicalLineItem<'_, 'c>,
    origin: Point<f32>,
    line: &TextLine<'_>,
    ink: &mut Ink,
  ) {
    let TextSource {
      items_data,
      spans,
      context,
    } = *source;
    let Some(shape_result) = &child.shape_result else {
      return;
    };
    let Some(object) = child
      .layout_object
      .or_else(|| child.inline_item?.layout_object)
    else {
      return;
    };
    let node = object.node();
    let Some(span) = child
      .inline_item
      .and_then(|item| spans.item_spans[item.index])
      .or_else(|| spans.span_of(node))
    else {
      return;
    };
    let Some(ProcessedInlineSpan::Text {
      style, decorations, ..
    }) = spans.spans.get(span)
    else {
      return;
    };
    let chain = decorations.as_deref();
    let mut decorating: Vec<(BoxFont, f32)> = chain
      .into_iter()
      .flat_map(DecorationLink::ancestors)
      .filter(|link| {
        link
          .decoration
          .owner
          .context
          .style
          .visual_data
          .text_decoration_line
          .is_some_and(|line| !line.is_empty())
      })
      .map(|link| {
        (
          link.decoration.font,
          line
            .span_baselines
            .get(&link.decoration.id)
            .copied()
            .unwrap_or(line.baseline),
        )
      })
      .collect();

    decorating.reverse();

    let text_font = chain.map_or(line.root_font, |link| link.decoration.font);
    let scale = child
      .text_fit_scale
      .as_ref()
      .map_or(1.0, |text_fit_scale| text_fit_scale.scale);
    let font: Rc<Font> = child
      .text_fit_scale
      .as_ref()
      .and_then(|text_fit_scale| text_fit_scale.font.clone())
      .unwrap_or_else(|| items_data.fonts.get(object.context()));
    let ascent = font
      .primary_font_metrics()
      .map_or(0.0, |metrics| metrics.font_height().ascent.to_float());
    let baseline = origin.y + ascent * scale;
    let mut brush = style.brush();

    brush.source_span_id = Some(span as u64);

    let symbol;
    // Approximate: Blink paints a symbol marker as a shape in the spaces `WidthOfSymbol` wide it
    // shapes as; takumi draws the symbol's glyph centered in them.
    let (shape_result, index_offset, x_shift) =
      if child.inline_item.is_some_and(InlineItem::is_symbol_marker) {
        let text = &items_data.text_content[child.text_offset.clone()];
        let shaped =
          HarfBuzzShaper::new(text.to_owned()).shape_all(&font, shape_result.direction());
        let x_shift = (shape_result.width() - shaped.width()) / 2.0;

        symbol = ShapeResultView::create_from_result(&Rc::new(shaped));
        (&symbol, child.text_offset.start as u32, x_shift)
      } else {
        (shape_result.as_ref(), 0, 0.0)
      };
    let mut glyphs: Vec<ItemGlyph> = Vec::with_capacity(shape_result.num_glyphs());
    let total_advance =
      shape_result.for_each_glyph(0.0, |character_index, glyph, offset, advance, font_data| {
        glyphs.push(ItemGlyph {
          character_index: character_index + index_offset,
          glyph,
          x: advance + offset.x + x_shift,
          y: offset.y,
          advance: advance + x_shift,
          font_data: font_data.cloned(),
        });
      });
    let text_end = child.text_offset.end;
    let mut starts: Vec<u32> = glyphs.iter().map(|glyph| glyph.character_index).collect();

    starts.sort_unstable();
    starts.dedup();

    let cluster_of = |character_index: u32| {
      let start = character_index as usize;
      let end = starts
        .get(starts.partition_point(|&start| start <= character_index))
        .map_or(text_end, |&next| next as usize);

      start..end
    };
    let mut group_start = 0;

    while group_start < glyphs.len() {
      let font_data = glyphs[group_start].font_data.clone();
      let group_end = glyphs[group_start..]
        .iter()
        .position(|glyph| !same_font(&glyph.font_data, &font_data))
        .map_or(glyphs.len(), |length| group_start + length);
      let group = &glyphs[group_start..group_end];
      let start_x = group[0].advance;
      let end_x = glyphs
        .get(group_end)
        .map_or(total_advance, |next| next.advance);

      group_start = group_end;

      let Some(font_data) = font_data else {
        continue;
      };
      let face = font_data.face();
      let is_color = face.is_color;
      let cluster_ranges: Vec<Range<usize>> = group
        .iter()
        .map(|glyph| cluster_of(glyph.character_index))
        .collect();
      let text_range = cluster_ranges
        .iter()
        .map(|range| range.start)
        .min()
        .unwrap_or(0)
        ..cluster_ranges
          .iter()
          .map(|range| range.end)
          .max()
          .unwrap_or(0);
      let positioned = group
        .iter()
        .zip(&cluster_ranges)
        .map(|(glyph, cluster)| PositionedGlyph {
          id: u32::from(glyph.glyph),
          x: origin.x + glyph.x,
          y: baseline + glyph.y,
          skips_ink: !is_color && glyph_skips_ink(&self.text, cluster.clone()),
        })
        .collect();
      let metrics = font_data.font_metrics();
      let weight = font.description.attributes.weight.value();
      let run = ShapedRun {
        glyphs: positioned,
        offset: origin.x + start_x,
        baseline,
        advance: end_x - start_x,
        hanging: HangingWhitespace::default(),
        metrics: RunMetrics {
          ascent: metrics.map_or(0.0, |metrics| metrics.exact.ascent),
          descent: metrics.map_or(0.0, |metrics| metrics.exact.descent),
          line_height: child.rect.size.block_size.to_float(),
          underline_offset: metrics
            .and_then(|metrics| metrics.underline)
            .map_or(0.0, |underline| -underline.position),
          underline_size: metrics
            .and_then(|metrics| metrics.underline)
            .map_or(0.0, |underline| underline.thickness),
        },
        font_size: font_data.size(),
        font_index: face.index,
        text_range,
        cluster_ranges,
        variations: font_data.variation_settings(),
        synthetic_bold: (!is_color
          && face.synthesis.embolden()
          && weight >= BOLD_THRESHOLD
          && brush.font_synthesis.weight.is_allowed())
        .then(|| synthesis_embolden_strength(font_data.size())),
        synthetic_skew: face
          .synthesis
          .skew()
          .filter(|_| !is_color && brush.font_synthesis.style.is_allowed())
          .map(|degrees| -degrees),
        normalized_coords: font_data.normalized_coords(),
        font_data: face.blob.clone(),
        brush: brush.clone(),
      };
      let line_scale = LineScaleState {
        scale,
        alignment_correction: 0.0,
        layout_origin: Point {
          x: line.content.x + origin.x,
          y: line.content.y + baseline,
        },
      };
      let decoration_placement = DecorationPlacement::for_run(
        &run,
        line_scale.transform(Affine::IDENTITY, 0.0),
        line.content,
        (text_font, scale),
        (line.root_font, line.baseline),
        &decorating,
      );

      if !run.brush.decorations.is_empty() {
        for decoration_line in run.decoration_lines(
          &decoration_placement,
          Affine::IDENTITY,
          Affine::IDENTITY,
          Point::ZERO,
        ) {
          let area = decoration_line.bounds();

          InkBox::include(
            &mut ink.decorations,
            Point::new(area.left, area.top),
            Size::new(area.right - area.left, area.bottom - area.top),
          );
        }
      }

      let RunMetrics {
        ascent, descent, ..
      } = run.metrics;
      let scaled = |point: Point<f32>| Point {
        x: origin.x + (point.x - origin.x) * scale,
        y: baseline + (point.y - baseline) * scale,
      };

      InkBox::include(
        &mut ink.content,
        scaled(Point {
          x: run.offset,
          y: baseline - ascent,
        }),
        Size {
          width: run.advance * scale,
          height: (ascent + descent) * scale,
        },
      );
      include_glyph_ink(&mut ink.content, &run, context, scaled, scale);
      self.push_run(
        context,
        run,
        RunPlacement {
          line_scale,
          static_inline_prefix: 0.0,
          baseline_shift: 0.0,
          decoration_placement,
          relative_offset: Point::ZERO,
        },
      );
    }
  }
}

/// `ink` grown to take the ink of `run`'s glyphs, placed by `scaled` and sized by `scale`: what
/// the run's metrics box misses, such as synthetic-italic skew, faux-bold outset, negative
/// bearings, and glyphs taller than the font's metrics.
fn include_glyph_ink(
  ink: &mut Option<InkBox>,
  run: &ShapedRun,
  context: &RenderContext,
  scaled: impl Fn(Point<f32>) -> Point<f32>,
  scale: f32,
) {
  let Ok(font) = FontRef::from_index(run.font_data(), run.font_index) else {
    return;
  };
  let resolved_glyphs = context.fonts().with_context(|fonts| {
    fonts.resolve_glyphs(run.face(), font, run.glyphs.iter().map(|glyph| glyph.id))
  });

  for glyph in &run.glyphs {
    let Some((min_x, min_y, max_x, max_y)) = resolved_glyphs
      .get(&glyph.id)
      .and_then(|resolved| resolved.ink_extents())
    else {
      continue;
    };

    InkBox::include(
      ink,
      scaled(Point {
        x: glyph.x + min_x,
        y: glyph.y + min_y,
      }),
      Size {
        width: (max_x - min_x) * scale,
        height: (max_y - min_y) * scale,
      },
    );
  }
}

/// Whether two glyphs draw in the same font data.
fn same_font(left: &Option<Rc<SimpleFontData>>, right: &Option<Rc<SimpleFontData>>) -> bool {
  match (left, right) {
    (Some(left), Some(right)) => Rc::ptr_eq(left, right),
    (None, None) => true,
    _ => false,
  }
}

impl FragmentItems {
  /// Gives the box spans of `spans` the painted sizes of the boxes these items place.
  pub(crate) fn size_box_spans(&self, spans: &mut [ProcessedInlineSpan<'_>]) {
    for inline_box in &self.inline_boxes {
      if let Some(ProcessedInlineSpan::Box(item)) = spans.get_mut(inline_box.id as usize) {
        item.paint_width = inline_box.width;
        item.paint_height = inline_box.height;
      }
    }
  }

  /// Blink's `BlockLayoutAlgorithm::PositionOrPropagateListMarker`: each outside list marker
  /// beside the first line, at its content's start edge, on the line's baseline.
  fn place_outside_list_markers<'c>(
    &mut self,
    items_data: &InlineNodeData<'c>,
    lines: &[LineBoxFragment<'_, 'c>],
    spans: &mut InlineSpans<'c>,
    ink: &mut Option<InkBox>,
  ) {
    let Some(line) = lines
      .iter()
      .find(|line| line.line_box_bfc_block_offset.is_some())
    else {
      return;
    };
    let line_top = line
      .line_box_bfc_block_offset
      .map_or(0.0, LayoutUnit::to_float);
    let baseline = line_top + line.metrics.ascent.to_float();

    for item in &items_data.items {
      if item.item_type != InlineItemType::ListMarker {
        continue;
      }

      let Some(object) = item.layout_object else {
        continue;
      };
      let node = object.node();
      let Some(span) = spans.span_of(node) else {
        continue;
      };
      let Some(ProcessedInlineSpan::Box(marker)) = spans.spans.get_mut(span) else {
        continue;
      };
      let mut atomic_inline = BoxAtomicInline {
        available_space: Size {
          width: AvailableSpace::Definite(self.content_box.size.width),
          height: AvailableSpace::MaxContent,
        },
      };
      let result = atomic_inline.layout(node);
      let size = marker.paint_border_box(result.inline_size, result.block_size);
      let marker_baseline = result.baseline.map_or(size.height, |baseline| {
        baseline.to_float() + marker.margin.top
      });
      let x = match node.context.style.inherited_data.direction {
        Direction::Rtl => self.content_box.size.width - size.width,
        Direction::Ltr => 0.0,
      };

      self.push_inline_box(
        ink,
        VisualInlineBox {
          id: span as u64,
          x,
          y: baseline - marker_baseline,
          width: size.width,
          height: size.height,
          line_baseline: Some(baseline),
          kind: node.inline_box_kind(),
        },
      );
    }
  }

  /// Places `inline_box`, growing `ink` to take it.
  fn push_inline_box(&mut self, ink: &mut Option<InkBox>, inline_box: VisualInlineBox) {
    InkBox::include(
      ink,
      Point::new(inline_box.x, inline_box.y),
      Size::new(inline_box.width, inline_box.height),
    );
    self.inline_boxes.push(inline_box);
  }
}

/// Each span's line fragments, in the border box whose content box sits at `content`, grouped by
/// span in line order.
pub(in crate::layout::inline) fn line_span_fragments<'c>(
  lines: &[LineBoxFragment<'_, 'c>],
  spans: &InlineSpans<'c>,
  content: Point<f32>,
) -> Vec<(Rc<DecorationLink<'c>>, SpanFragment)> {
  let mut fragments = Vec::new();

  for (line_index, line) in lines.iter().enumerate() {
    let Some(line_top) = line.line_box_bfc_block_offset else {
      continue;
    };
    let line_top = line_top.to_float();
    let line_left = line.bfc_line_offset.to_float();
    let baseline = line_top + line.metrics.ascent.to_float();

    for child in &line.items.children {
      let Some(LineFragment::InlineBox(fragment)) = &child.layout_result else {
        continue;
      };
      let Some(decoration) = spans.decoration_of(fragment.node) else {
        continue;
      };
      let (has_start, has_end) = match decoration.decoration.direction {
        Direction::Rtl => (fragment.has_line_right_edge, fragment.has_line_left_edge),
        Direction::Ltr => (fragment.has_line_left_edge, fragment.has_line_right_edge),
      };

      fragments.push((
        Rc::clone(decoration),
        SpanFragment {
          id: decoration.decoration.id,
          line_index,
          x: content.x + line_left + child.rect.offset.inline_offset.to_float(),
          y: content.y + line_top + child.rect.offset.block_offset.to_float(),
          width: fragment.size.inline_size.to_float(),
          height: fragment.size.block_size.to_float(),
          baseline: content.y + baseline,
          has_start,
          has_end,
        },
      ));
    }
  }

  fragments.sort_by_key(|(_, fragment)| (fragment.id, fragment.line_index));
  fragments
}
