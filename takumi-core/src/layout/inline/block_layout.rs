//! Blink's [`BlockLayoutAlgorithm`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/block_layout_algorithm.cc)
//! for a block whose children form an inline formatting context, which lays its lines out as one
//! child and paints through fragment items.

use std::ptr;

use rustc_hash::FxHashMap;

use super::{
  FragmentItems, InlineMeasurement, StaticPosition,
  atomic_inline::BoxAtomicInline,
  background::{PaddingBox, containing_blocks},
  fragment_items::line_span_fragments,
  inline_layout_algorithm::LineBoxFragment,
  inline_node::InlineNode,
  inline_node_data::InlineNodeData,
  items::{InlineSpans, ProcessedInlineSpan},
  logical_line_item::LineFragment,
  unpositioned_list_marker::UnpositionedListMarker,
};
use crate::{
  geometry::{AvailableSpace, ComputedLayout, Line, Point, Size},
  layout::{
    logical_rect::LogicalOffset,
    margin_strut::MarginStrut,
    tree::{LayoutTree, RenderNode},
  },
  layout_unit::LayoutUnit,
  style::Direction,
};

impl<'c> InlineNode<'c> {
  /// The lines laid out at `inline_size`, atomic inlines in a containing block `inline_size` wide,
  /// below an outside list marker taller than the first line's ascent.
  fn layout_lines_at<'a>(
    self,
    items_data: &'a InlineNodeData<'c>,
    inline_size: f32,
    block_size: AvailableSpace,
    atomic_inline: &mut BoxAtomicInline<'_, 'c>,
  ) -> Vec<LineBoxFragment<'a, 'c>> {
    atomic_inline.available_space = Size {
      width: AvailableSpace::Definite(inline_size),
      height: block_size,
    };

    let mut lines = self.layout_lines(
      items_data,
      LayoutUnit::from_float(inline_size),
      atomic_inline,
    );

    if let Some(marker) = UnpositionedListMarker::of(items_data) {
      marker.push_content_down(&mut lines, atomic_inline);
    }

    lines
  }

  /// The lines laid out in the content box of `layout`.
  fn layout_lines_in_content_box<'a>(
    self,
    items_data: &'a InlineNodeData<'c>,
    layout: ComputedLayout,
    atomic_inline: &mut BoxAtomicInline<'_, 'c>,
  ) -> Vec<LineBoxFragment<'a, 'c>> {
    // Atomic inlines resolve percentages as the measure that sized the lines did.
    let percentage_block_size = self
      .block
      .context
      .definite_height()
      .map_or(AvailableSpace::MaxContent, AvailableSpace::Definite);

    self.layout_lines_at(
      items_data,
      layout.content_box_size().width,
      percentage_block_size,
      atomic_inline,
    )
  }

  /// The min-content, max-content, or fit-content width the available width asks for, as Blink's
  /// `ComputeMinMaxSizes` finds it without laying out any line.
  pub(crate) fn content_inline_size(
    self,
    items_data: &InlineNodeData<'c>,
    available_space: Size<AvailableSpace>,
    atomic_inline: &mut BoxAtomicInline<'_, 'c>,
  ) -> f32 {
    atomic_inline.available_space = available_space;

    let sizes = self.compute_min_max_sizes(items_data, atomic_inline);
    let (min_size, max_size) = (sizes.min_size.to_float(), sizes.max_size.to_float());

    match available_space.width {
      AvailableSpace::MinContent => min_size,
      AvailableSpace::MaxContent => max_size,
      AvailableSpace::Definite(available) => max_size.min(available.max(0.0)).max(min_size),
    }
  }

  /// The content width of a `border_box_width` box, less `insets_width` when its layout resolved
  /// its padding and border, else the padding and border its style gives.
  pub(crate) fn content_width_of(self, border_box_width: f32, insets_width: Option<f32>) -> f32 {
    let insets_width = insets_width.unwrap_or_else(|| {
      let padding = self.block.padding_px();
      let border = self.block.border_px();

      padding.left + padding.right + border.left + border.right
    });

    (border_box_width - insets_width).max(0.0)
  }

  /// The content width, the known one or the min-content, max-content, or fit-content width the
  /// available width asks for, and the width its lines lay out at.
  pub(crate) fn widths(
    self,
    items_data: &InlineNodeData<'c>,
    available_space: Size<AvailableSpace>,
    known_width: Option<f32>,
    atomic_inline: &mut BoxAtomicInline<'_, 'c>,
  ) -> (f32, f32) {
    let width = known_width
      .unwrap_or_else(|| self.content_inline_size(items_data, available_space, atomic_inline));
    // Without a known width the lines lay out at the definite width offered.
    let line_width = known_width.unwrap_or(match available_space.width {
      AvailableSpace::Definite(available) => available.max(width),
      _ => width,
    });

    (width, line_width)
  }

  /// The size and baselines of the lines laid out `line_width` wide in `available_height`: `width`
  /// wide, and as tall as the lines down to a `line-clamp`.
  pub(crate) fn measure_lines(
    self,
    items_data: &InlineNodeData<'c>,
    width: f32,
    line_width: f32,
    available_height: AvailableSpace,
    atomic_inline: &mut BoxAtomicInline<'_, 'c>,
  ) -> InlineMeasurement {
    let lines = self.layout_lines_at(items_data, line_width, available_height, atomic_inline);
    let margins_adjoin = self.block.context.margins_adjoin();
    let margins = margin_struts(&lines, margins_adjoin);
    let (height, clamped) = content_block_size(&lines, margins_adjoin);
    let baseline = |line: &LineBoxFragment<'_, '_>| line.bfc_baseline().map(LayoutUnit::to_float);

    let shown = lines
      .iter()
      .position(|line| line.lines_until_clamp == Some(0))
      .map_or(lines.len(), |clamp_line| clamp_line + 1);

    InlineMeasurement {
      size: Size { width, height },
      first_baseline: lines[..shown].iter().find_map(baseline),
      last_baseline: lines[..shown].iter().rev().find_map(baseline),
      clamped,
      margins,
      is_self_collapsing: lines
        .iter()
        .all(|line| line.line_box_bfc_block_offset.is_none()),
    }
  }
}

/// Blink's `BlockLayoutAlgorithm` passing out the margins that adjoin the content's block-start
/// and block-end edges where `margins_adjoin` says: the ones before the first line box, and the
/// `EndMarginStrut` of the last line.
fn margin_struts(
  lines: &[LineBoxFragment<'_, '_>],
  margins_adjoin: Line<bool>,
) -> Line<MarginStrut> {
  let end = lines
    .last()
    .map_or(MarginStrut::default(), |line| line.end_margin_strut);
  let start = lines
    .iter()
    .enumerate()
    .find_map(|(index, line)| {
      line.line_box_bfc_block_offset?;

      let before = index
        .checked_sub(1)
        .map_or(MarginStrut::default(), |previous| {
          lines[previous].end_margin_strut
        });

      Some(
        match line
          .block_in_inline_layout_result()
          .and_then(|result| result.collapsed_margins)
        {
          Some(margins) => before.collapse_with(margins.start),
          None => before,
        },
      )
    })
    .unwrap_or(end);

  Line {
    start: if margins_adjoin.start {
      start
    } else {
      MarginStrut::default()
    },
    end: if margins_adjoin.end {
      end
    } else {
      MarginStrut::default()
    },
  }
}

/// The block size the lines take, down to the clamp line of a `line-clamp`, floats included, and
/// the margins after the last line unless they collapse out through an edge `margins_adjoin`, and
/// whether a clamp hid lines.
fn content_block_size(
  lines: &[LineBoxFragment<'_, '_>],
  margins_adjoin: Line<bool>,
) -> (f32, bool) {
  let mut bottom = LayoutUnit::ZERO;
  let mut clamped = false;
  let mut end = LayoutUnit::ZERO;
  let mut resolved = false;

  for line in lines {
    if let Some(top) = line.line_box_bfc_block_offset {
      resolved = true;

      if clamped {
        continue;
      }

      end = top + line.block_size + line.clearance_after_line;
      bottom = bottom.max(top + line.block_size);
      clamped = line.lines_until_clamp == Some(0);
    }

    // Floats grow the inline formatting context, which keeps its floats to itself.
    for child in &line.items.children {
      if let Some(LineFragment::Float {
        positioned_float, ..
      }) = &child.layout_result
      {
        bottom = bottom.max(
          positioned_float.bfc_offset.block_offset + positioned_float.layout_result.block_size,
        );
      }
    }
  }

  // Blink's `BlockLayoutAlgorithm` passes an unresolved BFC block offset's margin strut out
  // through the block-start edge, so the block size leaves it out.
  if !clamped
    && !margins_adjoin.end
    && (resolved || !margins_adjoin.start)
    && let Some(last) = lines.last()
  {
    bottom = bottom.max(end + last.end_margin_strut.sum());
  }

  let hides_lines = clamped
    && lines
      .iter()
      .skip_while(|line| line.lines_until_clamp != Some(0))
      .skip(1)
      .any(|line| line.line_box_bfc_block_offset.is_some());

  (bottom.to_float(), hides_lines)
}

impl RenderNode {
  /// The fragment items of this node's inline content laid out in the content box of `layout`,
  /// its atomic inlines in a layout tree of their own.
  pub(crate) fn lay_out_fragment_items(&self, layout: ComputedLayout) -> FragmentItems {
    let mut tree = LayoutTree::from_render_node(self);
    let mut atomic_inline = BoxAtomicInline::new(&mut tree, Size::all(AvailableSpace::MaxContent));

    self.lay_out_fragment_items_from(
      &InlineNode::new(self).prepare_layout(),
      layout,
      &mut atomic_inline,
    )
  }

  /// [`Self::lay_out_fragment_items`] from its prepared `items_data`.
  pub(crate) fn lay_out_fragment_items_from<'c>(
    &'c self,
    items_data: &InlineNodeData<'c>,
    layout: ComputedLayout,
    atomic_inline: &mut BoxAtomicInline<'_, 'c>,
  ) -> FragmentItems {
    let lines =
      InlineNode::new(self).layout_lines_in_content_box(items_data, layout, atomic_inline);
    let mut spans = InlineSpans::of(self, items_data);

    FragmentItems::from_lines(
      items_data,
      &lines,
      &mut spans,
      layout,
      &self.context,
      atomic_inline,
    )
  }

  /// The spans `items`, this node's fragment items, paint with.
  pub(crate) fn inline_spans(&self, items: &FragmentItems) -> Vec<ProcessedInlineSpan<'_>> {
    let items_data = InlineNode::new(self).collect_inlines();
    let mut spans = InlineSpans::of(self, &items_data).spans;

    items.size_box_spans(&mut spans);
    spans
  }

  /// Where each out-of-flow box in this node's inline content sits, in its content box, in tree
  /// order, and the padding box each of `containers` bounds the boxes it contains with, in
  /// `layout`'s border box: an inline span's from its line fragments, this node's its own.
  pub(crate) fn inline_out_of_flow_geometry<'c>(
    &'c self,
    layout: ComputedLayout,
    containers: &[&RenderNode],
    atomic_inline: &mut BoxAtomicInline<'_, 'c>,
  ) -> (Vec<StaticPosition>, Vec<PaddingBox>) {
    let node = InlineNode::new(self);
    let items_data = node.prepare_layout();
    let lines = node.layout_lines_in_content_box(&items_data, layout, atomic_inline);
    let mut placed: FxHashMap<*const RenderNode, StaticPosition> = FxHashMap::default();

    for line in &lines {
      let line_top = line
        .line_box_bfc_block_offset
        .unwrap_or(line.static_bfc_block_offset);
      let line_left = line.bfc_line_offset;
      let mut place = |node: &RenderNode, offset: LogicalOffset, direction: Direction| {
        placed.insert(
          ptr::from_ref(node),
          StaticPosition {
            point: Point {
              x: (line_left + offset.inline_offset).to_float(),
              y: (line_top + offset.block_offset).to_float(),
            },
            from_end: direction == Direction::Rtl,
          },
        );
      };

      for child in &line.items.children {
        if let Some(node) = child.out_of_flow_positioned_box {
          place(node, child.rect.offset, child.container_direction);
        }

        // An inline box's fragment took the out-of-flow boxes inside it.
        if let Some(LineFragment::InlineBox(fragment)) = &child.layout_result {
          for out_of_flow in &fragment.out_of_flow_children {
            place(
              out_of_flow.node,
              child.rect.offset + out_of_flow.offset,
              out_of_flow.container_direction,
            );
          }
        }
      }
    }

    let positions = self
      .inline_out_of_flow()
      .iter()
      .map(|out_of_flow| {
        placed
          .get(&ptr::from_ref(out_of_flow.node))
          .copied()
          .unwrap_or(StaticPosition {
            point: Point::ZERO,
            from_end: false,
          })
      })
      .collect();

    let own_padding_box = PaddingBox::of(layout);
    let spans = if containers.iter().all(|container| ptr::eq(*container, self)) {
      Vec::new()
    } else {
      let spans = InlineSpans::of(self, &items_data);
      let fragments = line_span_fragments(&lines, &spans, layout.content_box_offset());

      containing_blocks(&fragments, self.context.style.inherited_data.direction)
    };
    let containing_blocks = containers
      .iter()
      .map(|container| {
        spans
          .iter()
          .find(|span| ptr::eq(span.owner, *container))
          .map_or(own_padding_box, |span| span.padding_box)
      })
      .collect();

    (positions, containing_blocks)
  }
}

#[cfg(test)]
mod tests {
  use crate::geometry::Line;

  use super::content_block_size;
  use crate::geometry::{AvailableSpace, Size};
  use crate::layout::{
    inline::{atomic_inline::BoxAtomicInline, inline_node::InlineNode, testing::render_tree},
    node::Node,
    tree::LayoutTree,
  };
  use crate::layout_unit::LayoutUnit;

  /// The block size of lines holding only a self-collapsing block in a span, its 20px top margin
  /// collapsing out where `margins_adjoin`.
  fn self_collapsing_block_size(margins_adjoin: Line<bool>) -> f32 {
    let block = render_tree(
      [Node::container([Node::container([]).with_class_name("inner")]).with_class_name("span")],
      ".inner { display: block; margin-top: 20px }",
    );
    let mut tree = LayoutTree::from_render_node(&block);
    let mut atomic_inline = BoxAtomicInline::new(
      &mut tree,
      Size {
        width: AvailableSpace::Definite(60.0),
        height: AvailableSpace::MaxContent,
      },
    );

    block.context.set_margins_adjoin(margins_adjoin);

    let node = InlineNode::new(&block);
    let data = node.prepare_layout();
    let lines = node.layout_lines(&data, LayoutUnit::from_float(60.0), &mut atomic_inline);

    content_block_size(&lines, margins_adjoin).0
  }

  /// Blink's `BlockLayoutAlgorithm` passes the margins of content that resolves no BFC block offset
  /// out through the block start, so they do not also grow a block whose end they cannot pass.
  #[test]
  fn margins_passed_out_through_the_start_leave_the_block_size() {
    assert_eq!(
      self_collapsing_block_size(Line {
        start: true,
        end: false,
      }),
      0.0
    );
    assert_eq!(self_collapsing_block_size(Line::FALSE), 20.0);
  }
}
