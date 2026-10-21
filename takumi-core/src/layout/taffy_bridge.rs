//! taffy's grid algorithm, and the table algorithm written in taffy's terms, over the
//! native layout until Blink's replace them: a taffy `LayoutInput` turns into a `ConstraintSpace`
//! and a `LayoutResult` back into a `LayoutOutput`.

use std::{iter::Copied, slice};

use taffy::{
  AvailableSpace as TaffyAvailableSpace, AxisStaticEdge, AxisStaticPosition, Baselines, CacheTree,
  Layout, LayoutInput, LayoutOutput, LayoutPartialTree, Line, NodeId as TaffyNodeId, OofCandidate,
  OofCandidates, Point as TaffyPoint, Position as TaffyPosition, Rect as TaffyRect, RequestedAxis,
  RunMode, Size as TaffySize, SizingMode, Style, TraversePartialTree, TraverseTree,
};

use crate::{
  geometry::{ComputedLayout, NodeId, Point, Size},
  layout::{
    constraint_space::{AutoSizeBehavior, CacheSlot, ConstraintSpace},
    layout_result::{
      HorizontalEdge, LayoutResult, LogicalAlignmentDirection, OutOfFlowCandidate, StaticPosition,
      VerticalEdge,
    },
    length_utils::LengthResolver,
    logical_rect::LogicalSize,
    min_max_sizes::{MinMaxSizes, MinMaxSizesResult, SizeType},
    out_of_flow::ContainingBlock,
    tree::LayoutTree,
  },
  layout_unit::LayoutUnit,
};

/// The taffy algorithm laying a container out.
#[derive(Clone, Copy)]
pub(crate) enum TaffyAlgorithm {
  Table,
}

impl<'r> LayoutTree<'r> {
  /// `node` laid out by `algorithm` in `space`, its inline size the one Blink's
  /// `ComputeInlineSizeForFragment` gives a flex or grid container, and the out-of-flow boxes it
  /// contains laid out in its padding box.
  pub(crate) fn lay_out_with_taffy(
    &mut self,
    node: NodeId,
    space: &ConstraintSpace,
    algorithm: TaffyAlgorithm,
  ) -> LayoutResult {
    let style = self.layout_style_for(node, space);
    let resolver = LengthResolver::new(space, &style);
    let inline_size = match algorithm {
      _ if space.is_fixed_inline_size => Some(space.inline_size()),
      // A table sizes itself, unless its container stretches it explicitly.
      TaffyAlgorithm::Table
        if space.inline_auto_behavior == AutoSizeBehavior::StretchExplicit
          && style.size.width.is_auto() =>
      {
        Some(
          resolver
            .inline_size_for_fragment(&mut |size_type| self.min_max_sizes(node, space, size_type)),
        )
      }
      TaffyAlgorithm::Table => None,
    };
    let block_size = if space.is_fixed_block_size {
      space.available_block_size
    } else {
      match algorithm {
        TaffyAlgorithm::Table
          if space.block_auto_behavior == AutoSizeBehavior::StretchExplicit
            && style.size.height.is_auto() =>
        {
          resolver.initial_block_size_for_fragment(None, inline_size)
        }
        TaffyAlgorithm::Table => None,
      }
    };
    let to_float = |size: Option<LayoutUnit>| size.map(LayoutUnit::to_float);
    let available = |size: Option<LayoutUnit>| {
      size.map_or(TaffyAvailableSpace::MaxContent, |size| {
        TaffyAvailableSpace::Definite(size.to_float())
      })
    };
    let inputs = LayoutInput {
      run_mode: match space.cache_slot {
        CacheSlot::Layout => RunMode::PerformLayout,
        CacheSlot::Measure => RunMode::ComputeSize,
      },
      sizing_mode: if space.sizes_by_content {
        SizingMode::ContentSize
      } else {
        SizingMode::InherentSize
      },
      axis: RequestedAxis::Both,
      known_dimensions: TaffySize {
        width: to_float(inline_size),
        height: to_float(block_size),
      },
      known_dimensions_are_definite: TaffySize {
        width: true,
        height: !space.is_initial_block_size_indefinite,
      },
      parent_size: TaffySize {
        width: to_float(space.percentage_resolution_inline_size),
        height: to_float(space.percentage_resolution_block_size),
      },
      available_space: TaffySize {
        width: available(space.available_inline_size),
        height: available(space.available_block_size),
      },
      vertical_margins_are_collapsible: Line::FALSE,
    };
    let taffy_node = node.into_taffy();
    let output = match algorithm {
      TaffyAlgorithm::Table => self.compute_table_layout(taffy_node, inputs),
    };
    let size = LogicalSize::new(
      LayoutUnit::from_float(output.size.width),
      LayoutUnit::from_float(output.size.height),
    );
    let mut result = LayoutResult::of_box(size, &style, space);

    result.bfc_line_offset = space.bfc_offset.line_offset;
    result.bfc_block_offset = Some(space.bfc_offset.block_offset);
    result.first_baseline = output.baselines.first.map(LayoutUnit::from_float);
    result.last_baseline = output.baselines.last.map(LayoutUnit::from_float);

    let candidates: Vec<OutOfFlowCandidate> = output
      .oof_candidates
      .iter()
      .map(OutOfFlowCandidate::from_taffy)
      .collect();

    result.out_of_flow = if candidates.is_empty() {
      candidates
    } else {
      let claims = self.claims(node);
      let containing_block = ContainingBlock::padding_box(
        Size {
          width: size.inline_size,
          height: size.block_size,
        },
        result.border,
      );

      self.lay_out_out_of_flow(node, claims, candidates, containing_block, space)
    };
    result
  }

  /// The border-box min-content and max-content widths taffy's algorithm sizes `node`'s content
  /// to, at the block size its style makes definite.
  pub(crate) fn taffy_min_max_sizes(
    &mut self,
    node: NodeId,
    space: &ConstraintSpace,
  ) -> MinMaxSizesResult {
    let block_size = if self.is_table(node) {
      None
    } else {
      let style = self.layout_style_for(node, space);

      LengthResolver::new(space, &style).initial_block_size_for_fragment(None, None)
    };
    let mut width = |available_width| {
      let inputs = LayoutInput {
        run_mode: RunMode::ComputeSize,
        sizing_mode: SizingMode::ContentSize,
        axis: RequestedAxis::Horizontal,
        known_dimensions: TaffySize {
          width: None,
          height: block_size.map(LayoutUnit::to_float),
        },
        known_dimensions_are_definite: TaffySize {
          width: true,
          height: true,
        },
        parent_size: TaffySize {
          width: None,
          height: space
            .percentage_resolution_block_size
            .map(LayoutUnit::to_float),
        },
        available_space: TaffySize {
          width: available_width,
          height: space
            .available_block_size
            .map_or(TaffyAvailableSpace::MaxContent, |height| {
              TaffyAvailableSpace::Definite(height.to_float())
            }),
        },
        vertical_margins_are_collapsible: Line::FALSE,
      };
      let taffy_node = node.into_taffy();
      let output = self.compute_table_layout(taffy_node, inputs);

      LayoutUnit::from_float(output.size.width)
    };

    MinMaxSizesResult::new(
      MinMaxSizes {
        min_size: width(TaffyAvailableSpace::MinContent),
        max_size: width(TaffyAvailableSpace::MaxContent),
      },
      false,
    )
  }

  /// The space taffy's `inputs` lay `node` out in: a new formatting context sized to fit its
  /// content, a min-content or max-content width resolved to its contribution.
  fn space_for_taffy_input(&mut self, node: NodeId, inputs: &LayoutInput) -> ConstraintSpace {
    let style = self.layout_style(node);
    let direction = style.direction;
    let mut known_dimensions = inputs.known_dimensions;

    // taffy sizes an item's main size intrinsically at the cross size it
    // would stretch to, which a replaced item's aspect ratio would carry into its main size, so a
    // replaced item sizes that axis from its own style and natural size instead.
    let single_axis = match inputs.axis {
      RequestedAxis::Horizontal => {
        known_dimensions.width.is_none() && known_dimensions.height.is_some()
      }
      RequestedAxis::Vertical => {
        known_dimensions.height.is_none() && known_dimensions.width.is_some()
      }
      RequestedAxis::Both => false,
    };
    let sizes_alone = style.is_replaced && inputs.run_mode == RunMode::ComputeSize && single_axis;

    if sizes_alone {
      known_dimensions = TaffySize::NONE;
    }

    let from_float = |size: Option<f32>| size.map(LayoutUnit::from_float);
    let mut space = ConstraintSpace::new(None, direction);

    space.percentage_resolution_inline_size = from_float(inputs.parent_size.width);
    space.percentage_resolution_block_size = from_float(inputs.parent_size.height);
    space.is_new_formatting_context = true;
    space.sizes_by_content = inputs.sizing_mode == SizingMode::ContentSize && !sizes_alone;
    space.cache_slot = match inputs.run_mode {
      RunMode::PerformLayout => CacheSlot::Layout,
      RunMode::ComputeSize | RunMode::PerformHiddenLayout => CacheSlot::Measure,
    };
    space.available_block_size = from_float(
      known_dimensions
        .height
        .or(inputs.available_space.height.into_option()),
    );
    space.is_fixed_block_size = known_dimensions.height.is_some();
    space.is_initial_block_size_indefinite =
      space.is_fixed_block_size && !inputs.known_dimensions_are_definite.height;

    match (known_dimensions.width, inputs.available_space.width) {
      (Some(width), _) => {
        space.available_inline_size = Some(LayoutUnit::from_float(width));
        space.is_fixed_inline_size = true;
      }
      (None, TaffyAvailableSpace::Definite(width)) => {
        space.available_inline_size = Some(LayoutUnit::from_float(width));
      }
      (None, intrinsic) => {
        let sizes = if space.sizes_by_content {
          self.min_max_sizes(node, &space, SizeType::Content).sizes
        } else {
          self.min_and_max_content_contribution(node, &space).sizes
        };

        space.available_inline_size = Some(match intrinsic {
          TaffyAvailableSpace::MinContent => sizes.min_size,
          _ => sizes.max_size,
        });
        space.is_fixed_inline_size = true;
      }
    }

    space
  }

  /// The width alone `node` takes in `space`, which needs no layout.
  fn inline_size_in(&mut self, node: NodeId, space: &ConstraintSpace) -> LayoutUnit {
    let style = self.layout_style_for(node, space);

    if style.is_replaced {
      return self.layout(node, space).size.inline_size;
    }

    LengthResolver::new(space, &style)
      .inline_size_for_fragment(&mut |size_type| self.min_max_sizes(node, space, size_type))
  }

  /// Lays `node` and every box under it out empty, as `display: none` leaves them.
  pub(in crate::layout) fn compute_hidden(&mut self, node: TaffyNodeId) -> LayoutOutput {
    self.hide(NodeId::from_taffy(node));
    LayoutOutput::HIDDEN
  }
}

impl OutOfFlowCandidate {
  /// The candidate a taffy algorithm passed up, aligned to the start, center, or end of the area
  /// taffy gave it. Approximate: a `safe` alignment keeps its keyword when the box overflows the
  /// area.
  fn from_taffy(candidate: &OofCandidate) -> Self {
    let axis = |position: AxisStaticPosition| match position.align.keyword {
      AxisStaticEdge::Start => (position.area.start, 0),
      AxisStaticEdge::Center => ((position.area.start + position.area.end) / 2.0, 1),
      AxisStaticEdge::End => (position.area.end, 2),
    };
    let (x, horizontal) = axis(candidate.static_position.x);
    let (y, vertical) = axis(candidate.static_position.y);

    Self {
      node: NodeId::from_taffy(candidate.node),
      static_position: StaticPosition {
        offset: Point::new(LayoutUnit::from_float(x), LayoutUnit::from_float(y)),
        horizontal_edge: [
          HorizontalEdge::Left,
          HorizontalEdge::Center,
          HorizontalEdge::Right,
        ][horizontal],
        vertical_edge: [
          VerticalEdge::Top,
          VerticalEdge::Center,
          VerticalEdge::Bottom,
        ][vertical],
        align_self_direction: LogicalAlignmentDirection::Block,
      },
    }
  }

  /// The candidate as a taffy algorithm carries it up, its static position a point.
  fn to_taffy(self, order: usize, position: TaffyPosition) -> OofCandidate {
    let edge = |index: usize| {
      [
        AxisStaticEdge::Start,
        AxisStaticEdge::Center,
        AxisStaticEdge::End,
      ][index]
    };
    let horizontal = match self.static_position.horizontal_edge {
      HorizontalEdge::Left => 0,
      HorizontalEdge::Center => 1,
      HorizontalEdge::Right => 2,
    };
    let vertical = match self.static_position.vertical_edge {
      VerticalEdge::Top => 0,
      VerticalEdge::Center => 1,
      VerticalEdge::Bottom => 2,
    };

    OofCandidate {
      node: self.node.into_taffy(),
      order: order as u32,
      position,
      static_position: TaffyPoint {
        x: AxisStaticPosition::from_edge(
          self.static_position.offset.x.to_float(),
          edge(horizontal),
        ),
        y: AxisStaticPosition::from_edge(self.static_position.offset.y.to_float(), edge(vertical)),
      },
    }
  }
}

impl TraversePartialTree for LayoutTree<'_> {
  type ChildIter<'a>
    = Copied<slice::Iter<'a, TaffyNodeId>>
  where
    Self: 'a;

  fn child_ids(&self, parent_node_id: TaffyNodeId) -> Self::ChildIter<'_> {
    self.layout_children(parent_node_id).iter().copied()
  }

  fn child_count(&self, parent_node_id: TaffyNodeId) -> usize {
    self.layout_children(parent_node_id).len()
  }

  fn get_child_id(&self, parent_node_id: TaffyNodeId, child_index: usize) -> TaffyNodeId {
    self.layout_children(parent_node_id)[child_index]
  }
}

impl TraverseTree for LayoutTree<'_> {}

impl LayoutPartialTree for LayoutTree<'_> {
  type CoreContainerStyle<'a>
    = &'a Style
  where
    Self: 'a;
  type CustomIdent = String;

  fn get_core_container_style(&self, node_id: TaffyNodeId) -> Self::CoreContainerStyle<'_> {
    self.taffy_style(node_id)
  }

  fn set_unrounded_layout(&mut self, node_id: TaffyNodeId, layout: &Layout) {
    self.set_layout(
      NodeId::from_taffy(node_id),
      ComputedLayout::from_taffy(layout),
    );
  }

  fn resolve_calc_value(&self, val: *const (), basis: f32) -> f32 {
    self
      .render_node(NodeId::ROOT)
      .context
      .sizing
      .resolve_calc(val, basis)
  }

  fn compute_child_layout(&mut self, node: TaffyNodeId, inputs: LayoutInput) -> LayoutOutput {
    if inputs.run_mode == RunMode::PerformHiddenLayout {
      return self.compute_hidden(node);
    }

    let child = NodeId::from_taffy(node);
    let space = self.space_for_taffy_input(child, &inputs);

    if inputs.run_mode == RunMode::ComputeSize
      && inputs.axis == RequestedAxis::Horizontal
      && inputs.known_dimensions.width.is_none()
    {
      let width = if space.is_fixed_inline_size {
        space.inline_size()
      } else {
        self.inline_size_in(child, &space)
      };

      return LayoutOutput::from_outer_size(TaffySize {
        width: width.to_float(),
        height: 0.0,
      });
    }

    let result = self.layout(child, &space);
    let mut output = LayoutOutput::from_sizes_and_baselines(
      TaffySize {
        width: result.size.inline_size.to_float(),
        height: result.size.block_size.to_float(),
      },
      TaffyRect::ZERO,
      Baselines {
        first: result.first_baseline.map(LayoutUnit::to_float),
        last: result.last_baseline.map(LayoutUnit::to_float),
      },
    );
    let mut candidates = OofCandidates::new();

    for (order, candidate) in result.out_of_flow.iter().enumerate() {
      let position = self.taffy_style(candidate.node.into_taffy()).position;

      candidates.push(candidate.to_taffy(order, position));
    }
    output.oof_candidates = candidates;
    output
  }
}

impl CacheTree for LayoutTree<'_> {
  fn cache_get(&mut self, _node_id: TaffyNodeId, _input: &LayoutInput) -> Option<LayoutOutput> {
    None
  }

  fn cache_store(&mut self, _node_id: TaffyNodeId, _input: &LayoutInput, _output: LayoutOutput) {}

  fn cache_clear(&mut self, node_id: TaffyNodeId) {
    self
      .node_state_mut(NodeId::from_taffy(node_id))
      .cache
      .clear();
  }
}
