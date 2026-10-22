//! Blink's [`BlockNode`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/block_node.cc):
//! the algorithm that lays a box out, the layouts and intrinsic sizes it keeps, and where its
//! layout puts its box. Without fragmentation, `contain: size`, or scrollbars.
//!
//! Follows Blink under the notice in LICENSE-CHROMIUM.

use std::{ptr, rc::Rc};

use taffy::{Display as TaffyDisplay, Line};

use crate::{
  geometry::{AvailableSpace, ComputedLayout, NodeId, Point, Size},
  layout::{
    block_layout_algorithm::{BlockLayoutAlgorithm, InlineContentLayout, block_min_max_sizes},
    constraint_space::{AutoSizeBehavior, CacheSlot, ConstraintSpace},
    flex::flex_layout_algorithm::FlexLayoutAlgorithm,
    grid::grid_layout_algorithm::GridLayoutAlgorithm,
    inline::{atomic_inline::BoxAtomicInline, inline_node::InlineNode},
    layout_result::{
      HorizontalEdge, LayoutResult, LayoutStatus, OutOfFlowCandidate, StaticPosition, VerticalEdge,
    },
    length_utils::{
      LengthResolver, NaturalSize, ReplacedSizeMode, default_natural_size,
      inline_size_from_aspect_ratio,
    },
    logical_rect::{AvailableLogicalSize, LogicalBoxStrut, LogicalSize},
    min_max_sizes::{MinMaxSizes, MinMaxSizesResult, SizeType},
    out_of_flow::{ContainingBlock, ContainingBlockClaims},
    table::TableLayoutAlgorithm,
    tree::{LayoutTree, NodeOrigin, RenderNode},
  },
  layout_unit::LayoutUnit,
  style::Display,
};

/// The layouts a box keeps: Blink's layout result cache, one layout that placed its children and
/// a few measures that did not.
#[derive(Default)]
pub(crate) struct LayoutCache {
  layout: Option<(ConstraintSpace, Rc<LayoutResult>)>,
  measures: Vec<(ConstraintSpace, Rc<LayoutResult>)>,
  min_max_sizes: Vec<((MinMaxKey, SizeType), MinMaxSizesResult)>,
}

/// What a box's intrinsic inline sizes depend on in the space it is sized in.
#[derive(Clone, Copy, PartialEq)]
struct MinMaxKey {
  percentage_resolution_inline_size: Option<LayoutUnit>,
  percentage_resolution_block_size: Option<LayoutUnit>,
  available_block_size: Option<LayoutUnit>,
  is_new_formatting_context: bool,
}

impl MinMaxKey {
  fn of(space: &ConstraintSpace) -> Self {
    Self {
      percentage_resolution_inline_size: space.percentage_resolution_inline_size,
      percentage_resolution_block_size: space.percentage_resolution_block_size,
      available_block_size: space.available_block_size,
      is_new_formatting_context: space.is_new_formatting_context,
    }
  }
}

impl LayoutCache {
  const MEASURES: usize = 8;

  /// The layout for `space`: the last layout, which serves a measure too, or a measure.
  fn get(&self, space: &ConstraintSpace) -> Option<Rc<LayoutResult>> {
    if let Some((cached, result)) = &self.layout
      && cached.lays_out_like(space)
    {
      return Some(Rc::clone(result));
    }
    if space.cache_slot == CacheSlot::Layout {
      return None;
    }

    self
      .measures
      .iter()
      .find(|(cached, _)| cached == space)
      .map(|(_, result)| Rc::clone(result))
  }

  fn store(&mut self, space: &ConstraintSpace, result: &Rc<LayoutResult>) {
    if space.cache_slot == CacheSlot::Layout {
      self.layout = Some((space.clone(), Rc::clone(result)));
      return;
    }

    if self.measures.len() == Self::MEASURES {
      self.measures.remove(0);
    }
    self.measures.push((space.clone(), Rc::clone(result)));
  }

  fn min_max_sizes(&self, key: (MinMaxKey, SizeType)) -> Option<MinMaxSizesResult> {
    self
      .min_max_sizes
      .iter()
      .find(|(cached, _)| *cached == key)
      .map(|&(_, sizes)| sizes)
  }

  pub(crate) fn clear(&mut self) {
    *self = Self::default();
  }
}

/// Which algorithm lays a box out, as Blink's `DetermineAlgorithmAndRun` picks it.
enum Algorithm {
  Hidden,
  Table,
  Flex,
  Grid,
  Replaced,
  Block,
}

impl<'r> LayoutTree<'r> {
  /// Blink's `BlockNode::Layout`: the box `node` laid out in `space`, or the layout it kept for it.
  pub(crate) fn layout(&mut self, node: NodeId, space: &ConstraintSpace) -> Rc<LayoutResult> {
    self.restyle_for_container(
      node,
      Size {
        width: space.available_inline_size,
        height: space.available_block_size,
      },
    );

    let cached = self.node_state(node).cache.get(space);
    let result = match cached {
      Some(result) => result,
      None => {
        if space.cache_slot == CacheSlot::Layout {
          self.node_state_mut(node).hoisted_children.clear();
        }

        let result = Rc::new(self.run_algorithm(node, space));

        if result.status == LayoutStatus::Success {
          self.node_state_mut(node).cache.store(space, &result);
        }
        result
      }
    };

    if space.cache_slot == CacheSlot::Layout {
      self.node_state_mut(node).first_baseline_y = result.first_baseline.map(LayoutUnit::to_float);
    }

    result
  }

  /// Blink's `DetermineAlgorithmAndRun`.
  fn run_algorithm(&mut self, node: NodeId, space: &ConstraintSpace) -> LayoutResult {
    match self.algorithm(node) {
      Algorithm::Hidden => {
        if space.cache_slot == CacheSlot::Layout {
          self.hide(node);
        }
        LayoutResult::new(LogicalSize::default())
      }
      Algorithm::Table => TableLayoutAlgorithm::new(self, node, space).map_or_else(
        || LayoutResult::new(LogicalSize::default()),
        TableLayoutAlgorithm::layout,
      ),
      Algorithm::Flex => FlexLayoutAlgorithm::new(self, node, space, false).layout(),
      Algorithm::Grid => GridLayoutAlgorithm::new(self, node, space, false).layout(),
      Algorithm::Replaced => self.lay_out_replaced(node, space),
      Algorithm::Block => BlockLayoutAlgorithm::new(self, node, space).layout(),
    }
  }

  fn algorithm(&self, node: NodeId) -> Algorithm {
    let state = self.node_state(node);

    if state.table_box.hidden || state.layout_style.display == Display::None {
      return Algorithm::Hidden;
    }
    if state.table_box.is_table {
      return Algorithm::Table;
    }
    // An inline replaced box among blocks lays out in the anonymous block CSS 2.2 §9.2.1.1 wraps
    // it in.
    if self.has_inline_children(node) {
      return Algorithm::Block;
    }
    if state.layout_style.is_replaced {
      return Algorithm::Replaced;
    }

    match state.style.display {
      TaffyDisplay::Flex => Algorithm::Flex,
      TaffyDisplay::Grid => Algorithm::Grid,
      _ => Algorithm::Block,
    }
  }

  /// Blink's `BlockNode::ComputeMinMaxSizes`: the min-content and max-content border-box inline
  /// sizes of `node`'s content in `space`.
  pub(crate) fn min_max_sizes(
    &mut self,
    node: NodeId,
    space: &ConstraintSpace,
    size_type: SizeType,
  ) -> MinMaxSizesResult {
    let key = (MinMaxKey::of(space), size_type);

    if let Some(sizes) = self.node_state(node).cache.min_max_sizes(key) {
      return sizes;
    }

    let sizes = self.compute_min_max_sizes(node, space, size_type);

    self
      .node_state_mut(node)
      .cache
      .min_max_sizes
      .push((key, sizes));
    sizes
  }

  fn compute_min_max_sizes(
    &mut self,
    node: NodeId,
    space: &ConstraintSpace,
    size_type: SizeType,
  ) -> MinMaxSizesResult {
    let algorithm = self.algorithm(node);
    let style = Rc::clone(self.layout_style_rc(node));
    let resolver = LengthResolver::new(space, &style);
    let depends_on_block_constraints = style.size.height.has_percent_or_stretch()
      || style.min_size.height.has_percent_or_stretch()
      || style.max_size.height.has_percent_or_stretch()
      || (style.size.height.is_auto() && space.is_block_auto_behavior_stretch());

    match algorithm {
      Algorithm::Hidden => return MinMaxSizesResult::default(),
      Algorithm::Replaced => {
        let mut replaced_space = space.clone();

        replaced_space.available_inline_size = None;
        let inline_size = LengthResolver::new(&replaced_space, &style)
          .replaced_size(
            self.natural_size(node),
            ReplacedSizeMode::IgnoreInlineLengths,
          )
          .inline_size;

        return MinMaxSizesResult::new(
          MinMaxSizes::both(inline_size),
          depends_on_block_constraints,
        );
      }
      _ => {}
    }

    // An aspect ratio and a definite block size give the inline size.
    let initial_block_size = resolver.initial_block_size_for_fragment(None, None);

    if let (Some(aspect_ratio), Some(block_size), SizeType::Content) =
      (style.aspect_ratio, initial_block_size, size_type)
    {
      let inline_size = inline_size_from_aspect_ratio(
        resolver.border_padding,
        aspect_ratio,
        style.box_sizing_for_aspect_ratio(),
        block_size,
      );

      return MinMaxSizesResult {
        sizes: MinMaxSizes::both(inline_size),
        depends_on_block_constraints,
        applied_aspect_ratio: true,
      };
    }

    let mut result = match algorithm {
      Algorithm::Flex => FlexLayoutAlgorithm::new(self, node, space, true).compute_min_max_sizes(),
      Algorithm::Grid => GridLayoutAlgorithm::new(self, node, space, true).compute_min_max_sizes(),
      Algorithm::Table => TableLayoutAlgorithm::new(self, node, space).map_or_else(
        MinMaxSizesResult::default,
        TableLayoutAlgorithm::compute_min_max_sizes,
      ),
      _ if self.has_inline_children(node) => {
        let render_node = self.render_node(node);
        let items_data = self.inline_node_data(usize::from(node));
        let content_block_size = initial_block_size.map(|block_size| {
          (block_size - resolver.border_padding.block_sum()).clamp_negative_to_zero()
        });
        let mut atomic_inline = BoxAtomicInline {
          available_space: Size {
            width: AvailableSpace::MaxContent,
            height: content_block_size
              .or(
                space
                  .percentage_resolution_block_size
                  .filter(|_| self.is_anonymous(node)),
              )
              .map_or(AvailableSpace::MaxContent, |height| {
                AvailableSpace::Definite(height.to_float())
              }),
          },
        };
        let sizes =
          InlineNode::new(render_node).compute_min_max_sizes(&items_data, &mut atomic_inline);

        MinMaxSizesResult::new(sizes + resolver.border_padding.inline_sum(), false)
      }
      _ => block_min_max_sizes(self, node, space),
    };

    // A block size the aspect ratio gives keeps to the min and max block sizes.
    // https://drafts.csswg.org/css-sizing-4/#aspect-ratio
    if let (Some(aspect_ratio), None) = (style.aspect_ratio, initial_block_size) {
      let min_max = resolver.min_max_inline_sizes_from_aspect_ratio(aspect_ratio);

      result.sizes.min_size = min_max.clamp_size_to_min_and_max(result.sizes.min_size);
      result.sizes.max_size = min_max.clamp_size_to_min_and_max(result.sizes.max_size);
    }

    // The sizes depend on the percentage block size the box resolves against or passes through.
    result.depends_on_block_constraints = (depends_on_block_constraints || self.is_anonymous(node))
      && (result.depends_on_block_constraints || style.aspect_ratio.is_some());
    result
  }

  /// Blink's `ComputeMinAndMaxContentContribution`: `node`'s border-box inline size at min-content
  /// and max-content, its own sizes applied.
  pub(crate) fn min_and_max_content_contribution(
    &mut self,
    node: NodeId,
    space: &ConstraintSpace,
  ) -> MinMaxSizesResult {
    let style = Rc::clone(self.layout_style_rc(node));
    let resolver = LengthResolver::new(space, &style);

    if matches!(self.algorithm(node), Algorithm::Replaced) {
      return resolver.min_and_max_content_contribution_for_replaced(self.natural_size(node));
    }

    resolver
      .min_and_max_content_contribution(&mut |size_type| self.min_max_sizes(node, space, size_type))
  }

  /// The replaced content `node` lays out, sized in device pixels.
  pub(crate) fn natural_size(&self, node: NodeId) -> NaturalSize {
    let render_node = self.render_node(node);
    let natural = render_node
      .node
      .as_ref()
      .map(|source| source.natural_size(&render_node.context))
      .unwrap_or_default();
    let style = self.layout_style(node);

    NaturalSize {
      aspect_ratio: style.aspect_ratio.or(natural.aspect_ratio),
      default_size: default_natural_size(&render_node.context.sizing),
      ..natural
    }
  }

  /// Blink's `LayoutReplaced` sizing a replaced box in `space`.
  fn lay_out_replaced(&mut self, node: NodeId, space: &ConstraintSpace) -> LayoutResult {
    let style = Rc::clone(self.layout_style_rc(node));
    let size = LengthResolver::new(space, &style)
      .replaced_size(self.natural_size(node), ReplacedSizeMode::Normal);

    LayoutResult {
      bfc_line_offset: space.bfc_offset.line_offset,
      bfc_block_offset: Some(space.bfc_offset.block_offset),
      ..LayoutResult::of_box(size, &style, space)
    }
  }

  /// Blink's `IsReplaced`: whether `node` lays out as replaced content.
  pub(crate) fn is_replaced(&self, node: NodeId) -> bool {
    matches!(self.algorithm(node), Algorithm::Replaced)
  }

  /// Blink's `CreatesNewFormattingContext`.
  pub(crate) fn creates_new_formatting_context(&self, node: NodeId) -> bool {
    let state = self.node_state(node);
    let style = &state.layout_style;

    state.table_box.is_table
      || style.is_replaced
      || style.is_floating()
      || style.is_out_of_flow()
      || style.is_scroll_container()
      || style.applies_layout_containment()
      || !matches!(
        style.display,
        Display::Block | Display::ListItem | Display::Inline
      )
  }

  /// Whether `node` lays inline content out: its children form an inline formatting context, or
  /// it is a childless text or inline box laid out as a block.
  pub(crate) fn has_inline_children(&self, node: NodeId) -> bool {
    let state = self.node_state(node);

    // A replaced root is an atomic inline laid out alone, not the block wrapping one.
    state.is_inline_children
      || (state.children.is_empty()
        && !(state.layout_style.is_replaced && node == NodeId::ROOT)
        && self.render_node(node).measures_as_inline_content())
  }

  /// Whether `node` is an anonymous box, which passes its parent's percentage block size through.
  pub(crate) fn is_anonymous(&self, node: NodeId) -> bool {
    self.render_node(node).origin == NodeOrigin::Anonymous
  }

  pub(crate) fn is_table(&self, node: NodeId) -> bool {
    self.node_state(node).table_box.is_table
  }

  /// Blink's `IsOriginallyInlineLevel`: whether an out-of-flow box was inline-level before it was
  /// taken out of flow.
  pub(crate) fn is_original_display_inline(&self, node: NodeId) -> bool {
    self
      .render_node(node)
      .context
      .style
      .base_data
      .display
      .is_inline_level()
  }

  /// The out-of-flow boxes `node` is the containing block for.
  pub(crate) fn claims(&self, node: NodeId) -> ContainingBlockClaims {
    self.node_state(node).claims
  }

  /// Places `node`, laid out as `result`, with its border box at `location` in its parent's.
  pub(crate) fn place_child(
    &mut self,
    node: NodeId,
    location: Point<LayoutUnit>,
    result: &LayoutResult,
  ) {
    self.set_layout(
      node,
      ComputedLayout::new(
        location.map(LayoutUnit::to_float),
        Size {
          width: result.size.inline_size.to_float(),
          height: result.size.block_size.to_float(),
        },
        result.border,
        result.padding,
      ),
    );
  }

  /// Records that `container` laid out the out-of-flow `node` as its containing block.
  pub(crate) fn add_hoisted_child(&mut self, container: NodeId, node: NodeId) {
    self.node_state_mut(container).hoisted_children.push(node);
  }

  /// Lays `node` and every box under it out empty, as `display: none` leaves them.
  pub(crate) fn hide(&mut self, node: NodeId) {
    let mut stack = vec![node];

    while let Some(node) = stack.pop() {
      let state = self.node_state_mut(node);

      state.layout = ComputedLayout::default();
      state.first_baseline_y = None;
      state.hoisted_children.clear();
      stack.extend(
        state
          .children
          .iter()
          .map(|&child| NodeId::from_taffy(child)),
      );
    }
  }

  /// The inline content of the inline formatting context root `node` laid out in its content box,
  /// `inline_size` wide in `space`, as Blink's `InlineNode::Layout` gives it to the block holding
  /// it.
  pub(crate) fn lay_out_inline_content(
    &mut self,
    node: NodeId,
    space: &ConstraintSpace,
    inline_size: LayoutUnit,
    border_padding: LogicalBoxStrut,
    child_percentage_size: AvailableLogicalSize,
  ) -> InlineContentLayout {
    let style = Rc::clone(self.layout_style_rc(node));
    let has_definite_block_size = space.is_fixed_block_size
      || LengthResolver::new(space, &style)
        .initial_block_size_for_fragment(None, Some(inline_size))
        .is_some();
    let render_node = self.render_node(node);
    let collapses_through = !space.is_new_formatting_context;

    render_node.context.set_margins_adjoin(Line {
      start: collapses_through && border_padding.block_start == LayoutUnit::ZERO,
      end: collapses_through
        && border_padding.block_end == LayoutUnit::ZERO
        && !has_definite_block_size,
    });
    render_node
      .context
      .set_definite_height(child_percentage_size.block_size.map(LayoutUnit::to_float));

    let available_height = child_percentage_size
      .block_size
      .map_or(AvailableSpace::MaxContent, |height| {
        AvailableSpace::Definite(height.to_float())
      });
    let measurement = self.inline_measurement(
      usize::from(node),
      Size {
        width: AvailableSpace::Definite(inline_size.to_float()),
        height: available_height,
      },
      Size {
        width: Some(inline_size.to_float()),
        height: None,
      },
      border_padding.inline_sum().to_float(),
    );

    InlineContentLayout {
      is_self_collapsing: measurement.is_self_collapsing,
      block_size: LayoutUnit::from_float(measurement.size.height),
      start_margin: measurement.margins.start,
      end_margin: measurement.margins.end,
      first_baseline: measurement.first_baseline.map(LayoutUnit::from_float),
      last_baseline: measurement.last_baseline.map(LayoutUnit::from_float),
    }
  }

  /// Lays out the out-of-flow boxes in the inline content of `node`, laid out as `result`, that an
  /// inline box contains, and returns the rest for `node` and its ancestors. Blink lays the first
  /// out in the inline box's padding box as `OutOfFlowLayoutPart` does for an inline containing
  /// block.
  pub(crate) fn lay_out_inline_out_of_flow(
    &mut self,
    node: NodeId,
    result: &LayoutResult,
    space: &ConstraintSpace,
  ) -> Vec<OutOfFlowCandidate> {
    let boxes = self.child_nodes(node);

    if boxes.is_empty() {
      return Vec::new();
    }

    let layout = ComputedLayout::new(
      Point::new(0.0, 0.0),
      Size {
        width: result.size.inline_size.to_float(),
        height: result.size.block_size.to_float(),
      },
      result.border,
      result.padding,
    );
    let render_node = self.render_node(node);
    let mut containers: Vec<&RenderNode> = Vec::new();

    for &child in &boxes {
      if let Some(container) = self.inline_container(child)
        && !containers.iter().any(|known| ptr::eq(*known, container))
      {
        containers.push(container);
      }
    }

    let (positions, padding_boxes) = render_node.inline_out_of_flow_geometry(layout, &containers);
    let content = layout.content_box_offset();
    let mut candidates = Vec::new();

    for (child, position) in boxes.into_iter().zip(positions) {
      let candidate = OutOfFlowCandidate {
        node: child,
        static_position: StaticPosition {
          offset: Point::new(
            LayoutUnit::from_float(content.x + position.point.x),
            LayoutUnit::from_float(content.y + position.point.y),
          ),
          horizontal_edge: if position.from_end {
            HorizontalEdge::Right
          } else {
            HorizontalEdge::Left
          },
          vertical_edge: VerticalEdge::Top,
          ..StaticPosition::default()
        },
      };
      let Some(container) = self.inline_container(child) else {
        candidates.push(candidate);
        continue;
      };
      let padding_box = containers
        .iter()
        .position(|known| ptr::eq(*known, container))
        .map(|index| padding_boxes[index]);
      let Some(padding_box) = padding_box else {
        candidates.push(candidate);
        continue;
      };
      let containing_block = ContainingBlock {
        origin: padding_box.origin.map(LayoutUnit::from_float),
        size: padding_box.size.map(LayoutUnit::from_float),
      };
      let claims = ContainingBlockClaims {
        absolute: container.contains_absolute_as_inline(),
        fixed: container.contains_fixed_as_inline(),
      };

      candidates.extend(self.lay_out_out_of_flow(
        node,
        claims,
        vec![candidate],
        containing_block,
        space,
      ));
    }

    candidates
  }

  /// The space the root lays out in, `available_space` its initial containing block: a block-level
  /// box filling its definite width, a new formatting context unless `in_flow`.
  pub(crate) fn root_space(
    &self,
    available_space: Size<AvailableSpace>,
    in_flow: bool,
  ) -> ConstraintSpace {
    let style = self.layout_style(NodeId::ROOT);
    let definite = |space: AvailableSpace| match space {
      AvailableSpace::Definite(size) => Some(LayoutUnit::from_float(size)),
      AvailableSpace::MinContent | AvailableSpace::MaxContent => None,
    };
    let available_inline_size = match available_space.width {
      AvailableSpace::Definite(width) => LayoutUnit::from_float(width),
      AvailableSpace::MinContent => LayoutUnit::ZERO,
      AvailableSpace::MaxContent => LayoutUnit::MAX,
    };
    let fits_content = self.is_table(NodeId::ROOT)
      || style.is_replaced
      || !matches!(available_space.width, AvailableSpace::Definite(_));
    let mut space = ConstraintSpace::new(Some(available_inline_size), style.direction);

    space.available_block_size = definite(available_space.height);
    space.percentage_resolution_inline_size = definite(available_space.width);
    space.percentage_resolution_block_size = definite(available_space.height);
    space.inline_auto_behavior = if fits_content {
      AutoSizeBehavior::FitContent
    } else {
      AutoSizeBehavior::StretchImplicit
    };
    space.is_new_formatting_context = !in_flow;
    space.cache_slot = CacheSlot::Layout;
    space
  }
}
