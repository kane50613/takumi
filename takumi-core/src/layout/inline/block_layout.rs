//! Blink's [`BlockLayoutAlgorithm`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/block_layout_algorithm.cc)
//! for a block whose children form an inline formatting context, which taffy sizes as a leaf and
//! paints through fragment items.

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
};
use crate::{
  geometry::{AvailableSpace, ComputedLayout, Point, Size},
  layout::logical_rect::LogicalOffset,
  layout::tree::RenderNode,
  layout_unit::LayoutUnit,
  style::Direction,
};

impl<'c> InlineNode<'c> {
  /// The lines laid out at `inline_size`, atomic inlines in a containing block `inline_size` wide.
  fn layout_lines_at<'a>(
    self,
    items_data: &'a InlineNodeData<'c>,
    inline_size: f32,
    block_size: AvailableSpace,
  ) -> Vec<LineBoxFragment<'a, 'c>> {
    let mut atomic_inline = BoxAtomicInline {
      available_space: Size {
        width: AvailableSpace::Definite(inline_size),
        height: block_size,
      },
    };

    self.layout_lines(
      items_data,
      LayoutUnit::from_float(inline_size),
      &mut atomic_inline,
    )
  }

  /// The lines laid out in the content box of `layout`.
  fn layout_lines_in_content_box<'a>(
    self,
    items_data: &'a InlineNodeData<'c>,
    layout: ComputedLayout,
  ) -> Vec<LineBoxFragment<'a, 'c>> {
    let content = layout.content_box_size();

    self.layout_lines_at(
      items_data,
      content.width,
      AvailableSpace::Definite(content.height),
    )
  }

  /// The size and baselines of the lines in taffy's constraint: the known width, or the
  /// min-content, max-content, or fit-content width the available width asks for, and the lines'
  /// height down to a `line-clamp`.
  pub(crate) fn measure(
    self,
    available_space: Size<AvailableSpace>,
    known_dimensions: Size<Option<f32>>,
  ) -> InlineMeasurement {
    let items_data = self.prepare_layout();
    // taffy knows a border-box width whatever `box-sizing` says, and offers a content-box one.
    let padding = self.block.padding_px();
    let border = self.block.border_px();
    let known_width = known_dimensions
      .width
      .map(|width| (width - padding.left - padding.right - border.left - border.right).max(0.0));
    let width = known_width.unwrap_or_else(|| {
      let mut atomic_inline = BoxAtomicInline { available_space };
      let sizes = self.compute_min_max_sizes(&items_data, &mut atomic_inline);
      let (min_size, max_size) = (sizes.min_size.to_float(), sizes.max_size.to_float());

      match available_space.width {
        AvailableSpace::MinContent => min_size,
        AvailableSpace::MaxContent => max_size,
        AvailableSpace::Definite(available) => max_size.min(available.max(0.0)).max(min_size),
      }
    });
    // taffy lays a leaf out with no known width, at the definite width it offers.
    let line_width = known_width.unwrap_or(match available_space.width {
      AvailableSpace::Definite(available) => available.max(width),
      _ => width,
    });
    let lines = self.layout_lines_at(&items_data, line_width, available_space.height);
    let (height, clamped) = content_block_size(&lines);
    let baseline = |line: &LineBoxFragment<'_, '_>| {
      line
        .line_box_bfc_block_offset
        .map(|top| (top + line.metrics.ascent).to_float())
    };

    let shown = lines
      .iter()
      .position(|line| line.lines_until_clamp == Some(0))
      .map_or(lines.len(), |clamp_line| clamp_line + 1);

    InlineMeasurement {
      size: Size { width, height },
      first_baseline: lines[..shown].iter().find_map(baseline),
      last_baseline: lines[..shown].iter().rev().find_map(baseline),
      clamped,
    }
  }
}

/// The block size the lines take, down to the clamp line of a `line-clamp`, floats included,
/// and whether a clamp hid lines.
fn content_block_size(lines: &[LineBoxFragment<'_, '_>]) -> (f32, bool) {
  let mut bottom = LayoutUnit::ZERO;
  let mut clamped = false;

  for line in lines {
    if let Some(top) = line.line_box_bfc_block_offset {
      if clamped {
        continue;
      }

      bottom = bottom.max(top + line.block_size);
      clamped = line.lines_until_clamp == Some(0);
    }

    // Floats grow the formatting context, which takumi lays out as a leaf of its own.
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

  let hides_lines = clamped
    && lines
      .iter()
      .skip_while(|line| line.lines_until_clamp != Some(0))
      .skip(1)
      .any(|line| line.line_box_bfc_block_offset.is_some());

  (bottom.to_float(), hides_lines)
}

impl RenderNode {
  /// The fragment items of this node's inline content laid out in the content box of `layout`.
  pub(crate) fn lay_out_fragment_items(&self, layout: ComputedLayout) -> FragmentItems {
    let node = InlineNode::new(self);
    let items_data = node.prepare_layout();
    let lines = node.layout_lines_in_content_box(&items_data, layout);
    let mut spans = InlineSpans::of(self, &items_data);

    FragmentItems::from_lines(&items_data, &lines, &mut spans, layout, &self.context)
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
  pub(crate) fn inline_out_of_flow_geometry(
    &self,
    layout: ComputedLayout,
    containers: &[&RenderNode],
  ) -> (Vec<StaticPosition>, Vec<PaddingBox>) {
    let node = InlineNode::new(self);
    let items_data = node.prepare_layout();
    let lines = node.layout_lines_in_content_box(&items_data, layout);
    let mut placed: FxHashMap<*const RenderNode, StaticPosition> = FxHashMap::default();
    let mut next_line_top = LayoutUnit::ZERO;

    for line in &lines {
      // An empty line holds its out-of-flow boxes where the next line would start.
      let line_top = line.line_box_bfc_block_offset.unwrap_or(next_line_top);

      if line.line_box_bfc_block_offset.is_some() {
        next_line_top = line_top + line.block_size + line.clearance_after_line;
      }

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
          for &(node, offset, _) in &fragment.out_of_flow_children {
            place(
              node,
              child.rect.offset + offset,
              fragment.node.context.style.inherited_data.direction,
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

  /// The first or last line's baseline below the content box's top, laid out at the border box
  /// width `width` in `available_space`.
  pub(crate) fn inline_content_baseline(
    &self,
    available_space: Size<AvailableSpace>,
    width: f32,
    use_last_line: bool,
  ) -> Option<f32> {
    let measurement = InlineNode::new(self).measure(
      available_space,
      Size {
        width: Some(width),
        height: None,
      },
    );

    if use_last_line {
      measurement.last_baseline
    } else {
      measurement.first_baseline
    }
  }
}
