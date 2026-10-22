//! What an inline formatting context's lines place: the boxes they hold, the static positions of
//! their out-of-flow boxes, and the fragment items that paint them.

use std::{ptr, rc::Rc};

use rustc_hash::FxHashMap;

use super::{
  FragmentItems, StaticPosition,
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
  geometry::{AvailableSpace, ComputedLayout, Point, Size},
  layout::{
    layout_result::LayoutResult as BoxLayoutResult, logical_rect::LogicalOffset, tree::RenderNode,
  },
  layout_unit::LayoutUnit,
  style::Direction,
};

/// Where an inline formatting context's lines put the boxes it does not paint itself.
pub(crate) struct InlineGeometry<'c> {
  /// The static position of each out-of-flow box of its inline content, in tree order.
  pub(crate) out_of_flow: Vec<StaticPosition>,
  /// Each containing block's padding box, `None` for the node's own.
  pub(crate) containing_blocks: Vec<Option<PaddingBox>>,
  pub(crate) inline_content_boxes: Vec<InlineContentBox<'c>>,
}

impl InlineGeometry<'_> {
  /// The padding box of the `index`th containing block, the node's own laid out as `layout`.
  pub(crate) fn containing_block(&self, index: usize, layout: ComputedLayout) -> PaddingBox {
    self.containing_blocks[index].unwrap_or_else(|| PaddingBox::of(layout))
  }
}

/// A box an inline formatting context's lines place.
pub(crate) struct InlineContentBox<'c> {
  pub(crate) node: &'c RenderNode,
  /// The border box's offset in the content box.
  pub(crate) offset: Point<f32>,
  /// The layout its line gave a block-in-inline or a float.
  pub(crate) line_result: Option<Rc<BoxLayoutResult>>,
}

impl<'c> InlineContentBox<'c> {
  fn new(
    node: &'c RenderNode,
    (line_offset, block_offset): (LayoutUnit, LayoutUnit),
    line_result: Option<Rc<BoxLayoutResult>>,
  ) -> Self {
    Self {
      node,
      offset: Point::new(line_offset.to_float(), block_offset.to_float()),
      line_result,
    }
  }
}

/// An inline formatting context's lines, laid out once for the boxes they place and paint.
pub(crate) struct InlineContent<'c> {
  pub(crate) geometry: InlineGeometry<'c>,
  /// The space its lines laid their inline content boxes out in.
  pub(crate) available_space: Size<AvailableSpace>,
  /// The fragment items paint reads, in a content box whose block size is not known yet.
  pub(crate) fragment_items: Option<FragmentItems>,
}

impl RenderNode {
  /// Its inline content, laid out to `lines` in `layout`'s content box, with fragment items to
  /// paint when `paints`.
  pub(crate) fn inline_content<'c>(
    &'c self,
    (items_data, lines): (&InlineNodeData<'c>, &[LineBoxFragment<'_, 'c>]),
    layout: ComputedLayout,
    containers: &[&RenderNode],
    paints: bool,
    atomic_inline: &mut BoxAtomicInline<'_, 'c>,
  ) -> InlineContent<'c> {
    // Placing an outside marker changes the space, which the boxes lay out in again as here.
    let available_space = atomic_inline.available_space;
    let geometry = self.inline_geometry(items_data, lines, layout, containers, atomic_inline);
    let fragment_items = paints.then(|| {
      FragmentItems::from_lines(
        items_data,
        lines,
        &mut InlineSpans::of(self, items_data),
        layout,
        &self.context,
        atomic_inline,
      )
    });

    InlineContent {
      geometry,
      available_space,
      fragment_items,
    }
  }

  /// The spans `items`, this node's fragment items, paint with.
  pub(crate) fn inline_spans(&self, items: &FragmentItems) -> Vec<ProcessedInlineSpan<'_>> {
    let items_data = InlineNode::new(self).collect_inlines();
    let mut spans = InlineSpans::of(self, &items_data).spans;

    items.size_box_spans(&mut spans);
    spans
  }

  /// Where `lines` put this node's boxes, and each of `containers`' padding box, its own `None`.
  pub(crate) fn inline_geometry<'c>(
    &'c self,
    items_data: &InlineNodeData<'c>,
    lines: &[LineBoxFragment<'_, 'c>],
    layout: ComputedLayout,
    containers: &[&RenderNode],
    atomic_inline: &mut BoxAtomicInline<'_, 'c>,
  ) -> InlineGeometry<'c> {
    let mut placed: FxHashMap<*const RenderNode, StaticPosition> = FxHashMap::default();
    let mut inline_content_boxes = Vec::new();

    for line in lines {
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

        let shown = !child.is_hidden_due_to_layout && !child.is_truncated;
        let inline_content_box = match &child.layout_result {
          Some(LineFragment::AtomicInline { node, .. })
            if shown && node.participates_as_inline_box() =>
          {
            Some(InlineContentBox::new(
              node,
              (
                line_left + child.rect.offset.inline_offset,
                line_top + child.rect.offset.block_offset,
              ),
              None,
            ))
          }
          Some(LineFragment::Float {
            node,
            positioned_float,
          }) if shown => Some(InlineContentBox::new(
            node,
            (
              positioned_float.bfc_offset.line_offset,
              positioned_float.bfc_offset.block_offset,
            ),
            Some(Rc::clone(&positioned_float.layout_result)),
          )),
          Some(LineFragment::BlockInInline { node, result, .. }) if shown => {
            Some(InlineContentBox::new(
              node,
              (
                line_left + child.rect.offset.inline_offset,
                line_top + child.rect.offset.block_offset,
              ),
              Some(Rc::clone(result)),
            ))
          }
          _ => None,
        };

        inline_content_boxes.extend(inline_content_box);

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

    if let Some(marker) = UnpositionedListMarker::of(items_data)
      && let Some((_, line)) = UnpositionedListMarker::content_alignment_line(lines)
    {
      let result = marker.layout(atomic_inline);
      let offset = marker.offset(
        &result,
        line,
        layout,
        self.context.style.inherited_data.direction,
      );

      inline_content_boxes.push(InlineContentBox {
        node: marker.node,
        offset,
        line_result: None,
      });
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

    let spans = if containers.iter().all(|container| ptr::eq(*container, self)) {
      Vec::new()
    } else {
      let spans = InlineSpans::of(self, items_data);
      let fragments = line_span_fragments(lines, &spans, layout.content_box_offset());

      containing_blocks(&fragments, self.context.style.inherited_data.direction)
    };
    let containing_blocks = containers
      .iter()
      .map(|container| {
        spans
          .iter()
          .find(|span| ptr::eq(span.owner, *container))
          .map(|span| span.padding_box)
      })
      .collect();

    InlineGeometry {
      out_of_flow: positions,
      containing_blocks,
      inline_content_boxes,
    }
  }
}
