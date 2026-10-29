//! Blink's [`FragmentItemsBuilder`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/inline/fragment_items_builder.cc):
//! the items the inline layout port's line boxes place, as fragment items for painting.

use std::{ops::Range, rc::Rc};

use rustc_hash::FxHashMap;

use super::{BackgroundItem, ContentBox, FragmentItems, GlyphStore, Ink, InkBox, RunPlacement};
use crate::{
  context::RenderContext,
  font_style::SizedFontStyle,
  fonts::{font::Font, simple_font_data::SimpleFontData},
  geometry::{AvailableSpace, ComputedLayout, Point, Size},
  layout::{
    inline::{
      atomic_inline::BoxAtomicInline,
      background::{SpanFragment, resolve_span_fragments},
      decorations::{
        BoxFont, DecoratedLeaf, DecoratedLeaves, DecorationInset, DecorationPlacement,
      },
      inline_item::{InlineItem, LayoutObject},
      inline_layout_algorithm::LineBoxFragment,
      inline_node_data::InlineNodeData,
      items::{DecorationLink, InlineSpans, ProcessedInlineSpan},
      logical_line_item::{LineFragment, LogicalLineItem},
      metrics::VisualInlineBox,
      runs::{PositionedGlyph, RunMetrics, ShapedRun, glyph_skips_ink},
      text_fit::LineScaleState,
      unpositioned_list_marker::UnpositionedListMarker,
    },
    list_marker::SymbolMarker,
  },
  layout_unit::LayoutUnit,
  resources::glyph::ResolvedGlyphs,
  scene::text_ink_reach,
  sort_key::sort_by_key,
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
    atomic_inline: &mut BoxAtomicInline<'_, 'c>,
  ) -> Self {
    let mut items = Self {
      content_box: ContentBox::of(layout),
      text: items_data.text_content.clone(),
      generated_text_start: items_data.text_content.len(),
      ellipsized: false,
      line_boxes: Vec::new(),
      baselines: None,
      ink: Ink::default(),
      span_ink_reach: 0.0,
      text_items: Vec::new(),
      faces: Vec::new(),
      face_indices: FxHashMap::default(),
      face_glyphs: Vec::new(),
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
    let (leaves, leaf_indices) = DecoratedLeaves::of_lines(lines);
    let insets = leaves.insets();

    for (line_index, line) in lines.iter().enumerate() {
      // Blink's `PropagateBaselineFromLineBox` skips a line past the clamp point, and its items
      // are hidden.
      if line.is_hidden_due_to_layout {
        continue;
      }

      let Some(line_top) = line.line_box_bfc_block_offset.or_else(|| {
        line
          .block_in_inline_layout_result()
          .map(|_| line.static_bfc_block_offset)
      }) else {
        continue;
      };
      let line_top = line_top.to_float();
      let line_left = line.bfc_line_offset.to_float();
      let baseline = line_top + line.metrics.ascent.to_float();
      // Where each span on the line has its baseline, for the decorations it paints.
      let mut span_baselines: FxHashMap<usize, f32> = FxHashMap::default();

      if line.line_box_bfc_block_offset.is_some() {
        items
          .line_boxes
          .push((line_top, line_top + line.block_size.to_float()));
      }

      if let Some(line_baseline) = line.bfc_baseline().map(LayoutUnit::to_float) {
        first_baseline.get_or_insert(line_baseline);
        last_baseline = Some(line_baseline);
      }

      for (child_index, child) in line.items.children.iter().enumerate() {
        // Blink's `PhysicalFragment::IsHiddenDueToLayout` also reads the `IsTruncated` that
        // `LineTruncator::HideChild` sets on an atomic inline's layout object.
        if child.is_hidden_due_to_layout || child.is_truncated {
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
          Some(
            LineFragment::AtomicInline {
              node,
              layout_result,
            }
            | LineFragment::BlockInInline {
              node,
              layout_result,
              ..
            },
          ) => {
            let Some(span) = spans.span_of(node) else {
              continue;
            };
            let Some(ProcessedInlineSpan::Box(item)) = spans.spans.get_mut(span) else {
              continue;
            };
            let (paint_offset, paint_block_size) = layout_result.paint_block_extent();
            let size = item.paint_border_box(layout_result.inline_size, paint_block_size);
            let margin = item.margin;

            item.baseline_offset = layout_result
              .baseline
              .map(|baseline| (baseline + paint_offset).to_float() + margin.top);
            items.push_inline_box(
              &mut ink.content,
              VisualInlineBox {
                id: span as u64,
                x: origin.x - margin.left,
                y: origin.y - margin.top - paint_offset.to_float(),
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
            let result = &positioned_float.layout_result;
            let size = item.paint_border_box(result.size.inline_size, result.size.block_size);
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

              let child_insets = leaf_indices[line_index][child_index]
                .map_or(&[][..], |leaf| insets[leaf].as_slice());

              items.push_text(&source, child, origin, &text_line, child_insets, &mut ink);
            }
          }
        }
      }
    }

    items.place_outside_list_marker(
      items_data,
      lines,
      spans,
      (layout, context.style.inherited_data.direction),
      &mut ink.content,
      atomic_inline,
    );
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
    sort_by_key(&mut items.inline_boxes, |inline_box| inline_box.id);
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
    insets: &[DecorationInset],
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
    let box_style;
    let (style, chain) = match spans.spans.get(span) {
      Some(ProcessedInlineSpan::Text {
        style, decorations, ..
      }) => (&**style, decorations.as_deref()),
      // Blink's `PlaceEllipsisNextTo` hangs the ellipsis after an atomic inline on that inline's
      // layout object, whose style paints it.
      Some(ProcessedInlineSpan::Box(item)) if child.text_content.is_some() => {
        box_style = SizedFontStyle::from_style(&object.context().style, object.context());

        (&box_style, item.decorations.as_deref())
      }
      _ => return,
    };
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

    let symbol_marker = if child.inline_item.is_some_and(InlineItem::is_symbol_marker) {
      SymbolMarker::of(object.context())
    } else {
      None
    };
    let index_offset = match &child.text_content {
      // An ellipsis or hyphen counts its offsets from its own start, and takes the text's end.
      Some(generated) => {
        let start = self.text.len() as u32;

        self.text.push_str(generated);
        start
      }
      None => 0,
    };
    let mut glyphs: Vec<ItemGlyph> = Vec::with_capacity(shape_result.num_glyphs());
    let total_advance =
      shape_result.for_each_glyph(0.0, |character_index, glyph, offset, advance, font_data| {
        glyphs.push(ItemGlyph {
          character_index: character_index + index_offset,
          glyph,
          x: advance + offset.x,
          y: offset.y,
          advance,
          font_data: font_data.cloned(),
        });
      });
    let text_end = if child.text_content.is_some() {
      self.text.len()
    } else {
      child.text_offset.end
    };
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
      let face = &font_data.face;
      let is_color = face.typeface.is_color;
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
      let synthesis = font_data.synthesis();
      let run = ShapedRun {
        glyphs: positioned,
        offset: origin.x + start_x,
        baseline,
        advance: end_x - start_x,
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
        font_size: font_data.size,
        font_index: face.typeface.index,
        text_range,
        cluster_ranges,
        variations: font_data.variation_settings(),
        synthetic_bold: synthesis.embolden,
        synthetic_skew: synthesis.skew,
        normalized_coords: font_data.normalized_coords(),
        symbol_marker,
        font_data: face.typeface.blob(),
        brush: brush.clone(),
      };
      let line_scale = LineScaleState {
        scale,
        layout_origin: Point {
          x: line.content.x + origin.x,
          y: line.content.y + baseline,
        },
      };
      let decoration_placement = DecorationPlacement::for_run(
        &run,
        line_scale.transform(Affine::IDENTITY),
        line.content,
        (text_font, scale),
        (line.root_font, line.baseline),
        &decorating,
        // The fragment's ends pull in, which its first and last runs hold.
        insets
          .iter()
          .map(|inset| inset.for_run(start_x <= 0.0, end_x >= total_advance))
          .collect(),
      );

      if !run.brush.decorations.is_empty() {
        for decoration_line in run.decoration_lines(
          &decoration_placement,
          Affine::IDENTITY,
          Affine::IDENTITY,
          Point::ZERO,
          true,
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
      self.push_run(
        context,
        run,
        RunPlacement {
          line_scale,
          decoration_placement,
          relative_offset: Point::ZERO,
        },
        |run, resolved_glyphs| {
          include_glyph_ink(&mut ink.content, run, resolved_glyphs, scaled, scale);
        },
      );
    }
  }
}

/// `ink` grown to take the ink of `run`'s glyphs, placed by `scaled` and sized by `scale`: what
/// the run's metrics box misses, such as synthetic-italic skew, faux-bold outset, negative
/// bearings, and glyphs taller than the font's metrics.
impl DecoratedLeaves {
  /// The leaves of `lines` in order, and the leaf each line's child is, by line and child.
  fn of_lines(lines: &[LineBoxFragment<'_, '_>]) -> (Self, Vec<Vec<Option<usize>>>) {
    let mut leaves = Vec::new();
    let indices = lines
      .iter()
      .enumerate()
      .map(|(line_index, line)| {
        line
          .items
          .children
          .iter()
          .map(|child| {
            let leaf = DecoratedLeaf::of(child, line_index)?;

            leaves.push(leaf);
            Some(leaves.len() - 1)
          })
          .collect()
      })
      .collect();

    (Self(leaves), indices)
  }
}

impl DecoratedLeaf {
  /// `child` of the line at `line` as a leaf of Blink's `InlineCursor`, none for a box that is not
  /// one: an inline box, a float, an out-of-flow box, or one a line hides.
  fn of(child: &LogicalLineItem<'_, '_>, line: usize) -> Option<Self> {
    if child.is_hidden_due_to_layout
      || child.is_truncated
      || child.out_of_flow_positioned_box.is_some()
      || child.unpositioned_float.is_some()
    {
      return None;
    }

    let line_break = child
      .inline_item
      .is_some_and(InlineItem::is_forced_line_break);
    let text = match &child.layout_result {
      Some(LineFragment::InlineBox(_) | LineFragment::Float { .. }) => return None,
      Some(_) => false,
      None if child.shape_result.is_some() || line_break => true,
      None => return None,
    };
    let object = child
      .layout_object
      .or_else(|| child.inline_item?.layout_object);
    let context = object.map(LayoutObject::context);

    Some(Self {
      line,
      text,
      line_break,
      width: child.inline_size.to_float(),
      decorations: context
        .map(|context| context.text_decorations.clone())
        .unwrap_or_default(),
      object: context.map(RenderContext::box_id),
      rtl: context.is_some_and(|context| context.style.inherited_data.direction == Direction::Rtl),
    })
  }
}

fn include_glyph_ink(
  ink: &mut Option<InkBox>,
  run: &ShapedRun,
  resolved_glyphs: &ResolvedGlyphs,
  scaled: impl Fn(Point<f32>) -> Point<f32>,
  scale: f32,
) {
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

  /// Blink's `BlockLayoutAlgorithm::PositionOrPropagateListMarker`: the outside list marker beside
  /// the line it aligns with, in the content box of `layout` laid out in `direction`.
  fn place_outside_list_marker<'c>(
    &mut self,
    items_data: &InlineNodeData<'c>,
    lines: &[LineBoxFragment<'_, 'c>],
    spans: &mut InlineSpans<'c>,
    (layout, direction): (ComputedLayout, Direction),
    ink: &mut Option<InkBox>,
    atomic_inline: &mut BoxAtomicInline<'_, 'c>,
  ) {
    let Some(marker) = UnpositionedListMarker::of(items_data) else {
      return;
    };
    let Some((_, line)) = UnpositionedListMarker::content_alignment_line(lines) else {
      return;
    };
    let Some(span) = spans.span_of(marker.node) else {
      return;
    };
    let Some(ProcessedInlineSpan::Box(item)) = spans.spans.get_mut(span) else {
      return;
    };
    atomic_inline.available_space = Size {
      width: AvailableSpace::Definite(self.content_box.size.width),
      height: AvailableSpace::MaxContent,
    };

    let result = marker.layout(atomic_inline);
    let size = item.paint_border_box(result.inline_size, result.block_size);
    let offset = marker.offset(&result, line, layout, direction);

    self.push_inline_box(
      ink,
      VisualInlineBox {
        id: span as u64,
        x: offset.x,
        y: offset.y,
        width: size.width,
        height: size.height,
        line_baseline: line.bfc_baseline().map(LayoutUnit::to_float),
        kind: marker.node.inline_box_kind(),
      },
    );
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

      if fragment.is_opaque {
        continue;
      }

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

  sort_by_key(&mut fragments, |(_, fragment)| {
    (fragment.id, fragment.line_index)
  });
  fragments
}
