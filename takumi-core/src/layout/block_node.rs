//! Blink's [`BlockNode`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/block_node.cc):
//! the algorithm that lays a box out, the layouts and intrinsic sizes it keeps, and where its
//! layout puts its box. Without `contain: size` or scrollbars.
//!
//! Follows Blink under the notice in LICENSE-CHROMIUM.

use std::{mem::take, rc::Rc};

#[cfg(feature = "fragmentation")]
use crate::layout::{block_break_token::BlockBreakToken, fragmentation_utils::fragment_index};
use crate::{
  geometry::{AvailableSpace, ComputedLayout, NodeId, Point, Size},
  layout::{
    block_layout_algorithm::{BlockLayoutAlgorithm, block_min_max_sizes},
    constraint_space::{AutoSizeBehavior, CacheSlot, ConstraintSpace},
    flex::flex_layout_algorithm::FlexLayoutAlgorithm,
    fragmentation_layout_input::{ChildBreakToken, FragmentationLayoutInput},
    grid::grid_layout_algorithm::GridLayoutAlgorithm,
    inline::{
      atomic_inline::BoxAtomicInline, inline_layout_algorithm::LineBoxFragment,
      inline_node::InlineNode, inline_node_data::InlineNodeData,
    },
    layout_result::{
      HorizontalEdge, LayoutResult, LayoutStatus, OutOfFlowCandidate, StaticPosition, VerticalEdge,
    },
    length_utils::{
      LengthResolver, NaturalSize, ReplacedSizeMode, default_natural_size,
      inline_size_from_aspect_ratio,
    },
    logical_rect::LogicalSize,
    min_max_sizes::{MinMaxSizes, MinMaxSizesResult, SizeType},
    out_of_flow::{ContainingBlock, ContainingBlockClaims},
    table::TableLayoutAlgorithm,
    tree::{BoxFragment, LayoutTree, NodeOrigin},
  },
  layout_unit::LayoutUnit,
  style::{Display, JustifyContent},
};

/// The layouts a box keeps: Blink's layout result cache, a layout of each fragment that placed its
/// children and a few measures that did not.
#[derive(Default)]
pub(crate) struct LayoutCache {
  /// Blink's `layout_results_`: the last layout of each fragment, with the break token it resumed
  /// at.
  layouts: Vec<(ConstraintSpace, ChildBreakToken, Rc<LayoutResult>)>,
  measures: Vec<(ConstraintSpace, Rc<LayoutResult>)>,
  min_max_sizes: Vec<((MinMaxKey, SizeType), MinMaxSizesResult)>,
}

/// Pushes `item`, allocating room for only it when `entries` is empty: most boxes cache one entry,
/// where a first push would make room for four.
fn push_tight<T>(entries: &mut Vec<T>, item: T) {
  if entries.capacity() == 0 {
    entries.reserve_exact(1);
  }
  entries.push(item);
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

  /// The layout for `space` resumed as `input` says: the last layout of that fragment from the
  /// same break token, which serves a measure too, or a measure of a fresh layout.
  fn get(
    &self,
    space: &ConstraintSpace,
    input: &FragmentationLayoutInput,
  ) -> Option<Rc<LayoutResult>> {
    let index = input.cached_fragment_index()?;

    if let Some((cached, break_token, result)) = self.layouts.get(index)
      && input.resumes_at(break_token)
      && cached.lays_out_like(space)
    {
      return Some(Rc::clone(result));
    }
    if space.cache_slot == CacheSlot::Layout || !input.is_fresh() {
      return None;
    }

    self
      .measures
      .iter()
      .find(|(cached, _)| cached == space)
      .map(|(_, result)| Rc::clone(result))
  }

  fn store(
    &mut self,
    space: &ConstraintSpace,
    input: &FragmentationLayoutInput,
    result: &Rc<LayoutResult>,
  ) {
    let Some(index) = input.cached_fragment_index() else {
      return;
    };

    if space.cache_slot == CacheSlot::Layout {
      // The later fragments resumed from an earlier layout of this one, and lay out anew.
      self.layouts.truncate(index);
      if self.layouts.len() == index {
        push_tight(
          &mut self.layouts,
          (space.clone(), input.child_break_token(), Rc::clone(result)),
        );
      }
      return;
    }
    if !input.is_fresh() {
      return;
    }

    if self.measures.len() == Self::MEASURES {
      self.measures.remove(0);
    }
    push_tight(&mut self.measures, (space.clone(), Rc::clone(result)));
  }

  fn store_min_max_sizes(&mut self, key: (MinMaxKey, SizeType), sizes: MinMaxSizesResult) {
    push_tight(&mut self.min_max_sizes, (key, sizes));
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
    self.layout_fragment(node, space, &FragmentationLayoutInput::default())
  }

  /// [`Self::layout`] resuming the box as `input` says.
  pub(crate) fn layout_fragment(
    &mut self,
    node: NodeId,
    space: &ConstraintSpace,
    input: &FragmentationLayoutInput,
  ) -> Rc<LayoutResult> {
    self.restyle_for_container(
      node,
      Size {
        width: space.available_inline_size,
        height: space.available_block_size,
      },
    );

    if let Some(result) = self.node_state(node).cache.get(space, input) {
      return result;
    }

    if space.cache_slot == CacheSlot::Layout && !input.resumes_box() {
      self.node_state_mut(node).hoisted_children.clear();
    }

    // Blink's `BoxFragmentBuilder` takes the flag from the space whatever algorithm runs.
    let result = Rc::new(LayoutResult {
      is_line_clamp_clipped_float: space.is_line_clamp_clipped_float,
      ..self.run_algorithm(node, space, input)
    });

    if result.status == LayoutStatus::Success {
      self.node_state_mut(node).cache.store(space, input, &result);
    }
    result
  }

  /// Blink's `DetermineAlgorithmAndRun`.
  fn run_algorithm(
    &mut self,
    node: NodeId,
    space: &ConstraintSpace,
    input: &FragmentationLayoutInput,
  ) -> LayoutResult {
    if let Some(table_part) = &space.table_part {
      return table_part.lay_out(self, space, input);
    }

    match self.algorithm(node) {
      Algorithm::Hidden => {
        if space.cache_slot == CacheSlot::Layout {
          self.hide(node);
        }
        LayoutResult::new(LogicalSize::default())
      }
      Algorithm::Table => TableLayoutAlgorithm::new(self, node, space, input).map_or_else(
        || LayoutResult::new(LogicalSize::default()),
        TableLayoutAlgorithm::layout,
      ),
      Algorithm::Flex => FlexLayoutAlgorithm::new(self, node, space, false, input).layout(),
      Algorithm::Grid => GridLayoutAlgorithm::new(self, node, space, false, input).layout(),
      Algorithm::Replaced => self.lay_out_replaced(node, space),
      Algorithm::Block => BlockLayoutAlgorithm::new(self, node, space, input).layout(),
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

    match state.layout_style.display {
      display if display.is_flex() => Algorithm::Flex,
      display if display.is_grid() => Algorithm::Grid,
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
      .store_min_max_sizes(key, sizes);
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
      Algorithm::Flex => FlexLayoutAlgorithm::new(
        self,
        node,
        space,
        true,
        &FragmentationLayoutInput::default(),
      )
      .compute_min_max_sizes(),
      Algorithm::Grid => GridLayoutAlgorithm::new(
        self,
        node,
        space,
        true,
        &FragmentationLayoutInput::default(),
      )
      .compute_min_max_sizes(),
      Algorithm::Table => {
        TableLayoutAlgorithm::new(self, node, space, &FragmentationLayoutInput::default())
          .map_or_else(
            MinMaxSizesResult::default,
            TableLayoutAlgorithm::compute_min_max_sizes,
          )
      }
      _ if self.has_inline_children(node) => {
        let render_node = self.render_node(node);
        let items_data = self.inline_node_data(usize::from(node));
        let content_block_size = initial_block_size.map(|block_size| {
          (block_size - resolver.border_padding.block_sum()).clamp_negative_to_zero()
        });
        let available_height = content_block_size
          .or(
            space
              .percentage_resolution_block_size
              .filter(|_| self.is_anonymous(node)),
          )
          .map_or(AvailableSpace::MaxContent, |height| {
            AvailableSpace::Definite(height.to_float())
          });
        let mut atomic_inline = BoxAtomicInline::new(
          self,
          Size {
            width: AvailableSpace::MaxContent,
            height: available_height,
          },
        );
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
      || self.render_node(node).is_root_element
      || style.is_replaced
      || style.is_floating()
      || style.is_out_of_flow()
      || style.is_scroll_container()
      || style.applies_layout_containment()
      || self.render_node(node).context.style.line_clamp().is_some()
      // https://drafts.csswg.org/css-align/#distribution-block
      || style.align_content != JustifyContent::Normal
      || !matches!(
        style.display,
        Display::Block | Display::ListItem | Display::Inline
      )
  }

  /// Blink's `LayoutBox::IsMonolithic` when printing: replaced content, an atomic inline, or a
  /// line-clamp container, which no page break splits. A float or a block-in-inline of inline
  /// content is laid out as an atomic inline, but is not one.
  #[cfg(feature = "fragmentation")]
  pub(crate) fn is_monolithic(&self, node: NodeId) -> bool {
    let state = self.node_state(node);
    let style = &state.layout_style;

    style.is_replaced
      || (state.is_atomic_inline && style.display.is_inline_level() && !style.is_floating())
      || self.render_node(node).context.style.line_clamp().is_some()
  }

  /// Whether `node` lays inline content out: its children form an inline formatting context, or
  /// it is a childless text or inline box laid out as a block.
  pub(crate) fn has_inline_children(&self, node: NodeId) -> bool {
    let state = self.node_state(node);

    // A replaced atomic inline lays out as itself, not as the block wrapping one.
    state.is_inline_children
      || (state.children.is_empty()
        && !(state.layout_style.is_replaced && (state.is_atomic_inline || node == NodeId::ROOT))
        && self.render_node(node).measures_as_inline_content())
  }

  /// Whether `node` is an anonymous box, which passes its parent's percentage block size through.
  pub(crate) fn is_anonymous(&self, node: NodeId) -> bool {
    self.render_node(node).origin == NodeOrigin::Anonymous
  }

  pub(crate) fn is_table(&self, node: NodeId) -> bool {
    self.node_state(node).table_box.is_table
  }

  /// The out-of-flow boxes `node` is the containing block for.
  pub(crate) fn claims(&self, node: NodeId) -> ContainingBlockClaims {
    self.node_state(node).claims
  }

  /// Places `node`'s fragment `fragment_index`, laid out as `result`, with its border box at
  /// `location` in its parent's fragment `container_fragment_index`. A fragment that does not
  /// break ends the box, so the fragments an earlier layout left after it go, as Blink's
  /// `LayoutBox::SetLayoutResult` drops them.
  pub(crate) fn place_child(
    &mut self,
    node: NodeId,
    fragment_index: usize,
    #[cfg_attr(not(feature = "fragmentation"), allow(unused_variables))]
    container_fragment_index: usize,
    location: Point<LayoutUnit>,
    result: &LayoutResult,
  ) {
    self.set_fragment(
      node,
      fragment_index,
      BoxFragment {
        layout: result.computed_layout(location),
        is_line_clamp_clipped_float: result.is_line_clamp_clipped_float,
        #[cfg(feature = "fragmentation")]
        container_fragment_index,
        is_repeated: result.is_repeated(),
      },
    );
    #[cfg(feature = "fragmentation")]
    if result.break_info.break_token.is_none() {
      self
        .node_state_mut(node)
        .fragments
        .truncate(fragment_index + 1);
    }
  }

  /// Makes the out-of-flow boxes `container` positioned from its hoisted child `first` on belong
  /// to its fragment `fragment_index`.
  #[cfg(feature = "fragmentation")]
  pub(crate) fn hold_hoisted_children(
    &mut self,
    container: NodeId,
    fragment_index: usize,
    first: usize,
  ) {
    let hoisted_children = self.node_state(container).hoisted_children[first..].to_vec();

    for child in hoisted_children {
      self.node_state_mut(child).fragments[0].container_fragment_index = fragment_index;
    }
  }

  /// Blink's `LayoutRepeatableRoot`: a repeated table header or footer laid out once, then the
  /// same layout for each later repetition after `token`, which leaves out the out-of-flow boxes
  /// an ancestor positions.
  #[cfg(feature = "fragmentation")]
  pub(crate) fn lay_out_repeatable_root(
    &mut self,
    node: NodeId,
    space: &ConstraintSpace,
    token: Option<&BlockBreakToken>,
  ) -> Rc<LayoutResult> {
    let is_first = token.is_none_or(|token| !token.is_repeated);
    let first = if is_first {
      None
    } else {
      self.repeated_roots.get(&node).cloned()
    };
    let mut result = match first {
      Some(first) => {
        let mut result = (*first).clone();

        result.break_info.is_first_for_node = false;
        result.break_info.is_repeated = true;
        result.out_of_flow.clear();
        result
      }
      None => {
        let result = self.layout(node, space);

        self.repeated_roots.insert(node, Rc::clone(&result));
        (*result).clone()
      }
    };

    result.break_info.break_token = space.should_repeat.then(|| {
      Rc::new(BlockBreakToken::repeated(
        node,
        fragment_index(token) as u32,
      ))
    });
    Rc::new(result)
  }

  /// Gives every box under `node`'s fragment `source` a copy of its fragment there for `node`'s
  /// fragment `target`, as Blink's `FragmentRepeater` deep-clones a repeated root. An out-of-flow
  /// box repeats with its containing block alone.
  #[cfg(feature = "fragmentation")]
  pub(crate) fn repeat_fragments(&mut self, node: NodeId, source: usize, target: usize) {
    let state = self.node_state(node);
    let children: Vec<NodeId> = state
      .box_children
      .iter()
      .map(|child| child.node_id)
      .filter(|&child| !self.layout_style(child).is_out_of_flow())
      .chain(state.hoisted_children.iter().copied())
      .collect();

    for child in children {
      let fragments = &self.node_state(child).fragments;
      let Some(child_source) = fragments
        .iter()
        .position(|fragment| fragment.container_fragment_index == source)
      else {
        continue;
      };
      // A layout of the same page again replaces the copy it made before.
      let existing = fragments
        .iter()
        .position(|fragment| fragment.container_fragment_index == target);
      let child_target = existing.unwrap_or(fragments.len());
      let fragment = BoxFragment {
        container_fragment_index: target,
        is_repeated: true,
        ..fragments[child_source]
      };
      let fragments = &mut self.node_state_mut(child).fragments;

      match existing {
        Some(index) => fragments[index] = fragment,
        None => fragments.push(fragment),
      }
      self
        .render_node(child)
        .fragment_items
        .repeat(child_source, child_target);
      if self.is_table(child) {
        self.repeat_table_fragment(child, child_source, child_target);
      }
      self.repeat_fragments(child, child_source, child_target);
    }
  }

  /// Records that `container` laid out the out-of-flow `node` as its containing block.
  pub(crate) fn add_hoisted_child(&mut self, container: NodeId, node: NodeId) {
    let hoisted_children = &mut self.node_state_mut(container).hoisted_children;

    if !hoisted_children.contains(&node) {
      hoisted_children.push(node);
    }
  }

  /// Lays `node` and every box under it out empty, as `display: none` leaves them.
  pub(crate) fn hide(&mut self, node: NodeId) {
    let mut stack = vec![node];

    while let Some(node) = stack.pop() {
      let state = self.node_state_mut(node);

      state.fragments[0].layout = ComputedLayout::default();
      state.hoisted_children.clear();
      stack.extend(state.children.iter().copied());
    }
  }

  /// Keeps the inline content of `node`, laid out to `lines` in its content box `inline_size`
  /// wide in `space`, for its boxes and paint to read once its block size is known.
  pub(crate) fn keep_inline_content(
    &mut self,
    node: NodeId,
    space: &ConstraintSpace,
    inline_size: LayoutUnit,
    lines: (&InlineNodeData<'r>, &[LineBoxFragment<'_, 'r>]),
    available_space: Size<AvailableSpace>,
  ) {
    let style = Rc::clone(self.layout_style_rc(node));
    let render_node = self.render_node(node);
    let boxes = self.child_nodes(node);
    let containers = self.inline_containers_of(&boxes);
    let layout = LayoutResult::of_box(
      LogicalSize::new(inline_size, LayoutUnit::ZERO),
      &style,
      space,
    )
    .computed_layout(Point::default());
    let paints = space.cache_slot == CacheSlot::Layout && render_node.has_own_inline_content();
    let mut atomic_inline = BoxAtomicInline::new(self, available_space);

    atomic_inline.cache_slot = space.cache_slot;

    let content =
      render_node.inline_content(lines, layout, &containers, paints, &mut atomic_inline);

    self.inline_contents.insert(node, content);
  }

  /// Places the inline content boxes of `node`, laid out as `result`, where its lines put them,
  /// then lays out the out-of-flow boxes of that content that an inline box contains and returns
  /// the rest for `node` and its ancestors. Blink lays the first out in the inline box's padding
  /// box as `OutOfFlowLayoutPart` does for an inline containing block.
  pub(crate) fn finish_inline_content(
    &mut self,
    node: NodeId,
    (fragment_index, holds_every_line): (usize, bool),
    result: &LayoutResult,
    space: &ConstraintSpace,
  ) -> Vec<OutOfFlowCandidate> {
    let Some(inline_content) = self.inline_contents.remove(&node) else {
      return Vec::new();
    };
    let boxes = self.child_nodes(node);
    let render_node = self.render_node(node);
    let layout = result.computed_layout(Point::default());

    if let Some(items) = inline_content.fragment_items {
      render_node
        .fragment_items
        .keep(fragment_index, Rc::new(items.in_content_box_of(layout)));
    }

    let containers = self.inline_containers_of(&boxes);
    let mut geometry = inline_content.geometry;
    let mut atomic_inline = BoxAtomicInline::new(self, inline_content.available_space);

    atomic_inline.cache_slot = space.cache_slot;

    let content = layout.content_box_offset();
    // The out-of-flow boxes inside an inline content box that it does not contain go on to the
    // containing blocks around this one.
    let mut candidates = Vec::new();

    if space.cache_slot == CacheSlot::Layout {
      for inline_content_box in take(&mut geometry.inline_content_boxes) {
        let child_render_node = inline_content_box.node;
        let offset = inline_content_box.offset;
        let (child, child_result) = atomic_inline
          .lay_out_inline_content_box(child_render_node, inline_content_box.line_result);
        let location = Point::new(
          LayoutUnit::from_float(content.x + offset.x),
          LayoutUnit::from_float(content.y + offset.y),
        );
        let tree = &mut *atomic_inline.tree;

        tree.place_child(
          child,
          child_result.fragment_index(),
          fragment_index,
          location,
          &child_result,
        );
        if let Some(path) = render_node.inline_child_path(child_render_node) {
          tree.add_inline_box_child(node, child, &path);
        }
        candidates.extend(
          child_result
            .out_of_flow
            .iter()
            .map(|candidate| candidate.moved_by(location)),
        );
      }
    }

    for (&child, &position) in boxes.iter().zip(&geometry.out_of_flow) {
      // A box on the lines of another fragment is that fragment's to place; one on no line goes
      // where the content starts.
      let Some(position) = position.or(holds_every_line.then(Default::default)) else {
        continue;
      };
      let candidate = OutOfFlowCandidate::new(
        child,
        StaticPosition {
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
      );
      let Some(container) = self.inline_container(child) else {
        candidates.push(candidate);
        continue;
      };
      let padding_box = containers
        .iter()
        .position(|known| known.context.box_id() == container.context.box_id())
        .map(|index| geometry.containing_block(index, layout));
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
  /// box filling its definite width, in a new formatting context.
  pub(crate) fn root_space(&self, available_space: Size<AvailableSpace>) -> ConstraintSpace {
    let style = self.layout_style(NodeId::ROOT);
    let available_inline_size = match available_space.width {
      AvailableSpace::Definite(width) => LayoutUnit::from_float(width),
      AvailableSpace::MinContent => LayoutUnit::ZERO,
      AvailableSpace::MaxContent => LayoutUnit::MAX,
    };
    let fits_content = self.is_table(NodeId::ROOT)
      || style.is_replaced
      || !matches!(available_space.width, AvailableSpace::Definite(_));
    let mut space = ConstraintSpace::new(Some(available_inline_size), style.direction);

    space.available_block_size = available_space.height.definite();
    space.percentage_resolution_inline_size = available_space.width.definite();
    space.percentage_resolution_block_size = available_space.height.definite();
    space.inline_auto_behavior = if fits_content {
      AutoSizeBehavior::FitContent
    } else {
      AutoSizeBehavior::StretchImplicit
    };
    space.is_new_formatting_context = true;
    space.cache_slot = CacheSlot::Layout;
    space
  }
}

#[cfg(all(test, feature = "fragmentation"))]
mod tests {
  use std::rc::Rc;

  use super::LayoutCache;
  use crate::{
    geometry::NodeId,
    layout::{
      block_break_token::BlockBreakToken, constraint_space::ConstraintSpace,
      fragmentation_layout_input::FragmentationLayoutInput, layout_result::LayoutResult,
      logical_rect::LogicalSize,
    },
    layout_unit::LayoutUnit,
    style::Direction,
  };

  /// A layout resumed after the first fragment, which took `consumed_block_size`.
  fn resumed_after(consumed_block_size: i32) -> FragmentationLayoutInput {
    FragmentationLayoutInput {
      break_token: Some(Rc::new(BlockBreakToken {
        consumed_block_size: LayoutUnit::from_int(consumed_block_size),
        is_break_before: false,
        ..BlockBreakToken::break_before(NodeId::ROOT, false)
      })),
      early_break: None,
    }
  }

  #[test]
  fn a_fragment_layout_is_kept_for_the_break_token_it_resumed_at() {
    let space = ConstraintSpace::new(Some(LayoutUnit::from_int(100)), Direction::Ltr);
    let fresh = FragmentationLayoutInput::default();
    let first = Rc::new(LayoutResult::new(LogicalSize::default()));
    let second = Rc::new(LayoutResult::new(LogicalSize::default()));
    let mut cache = LayoutCache::default();

    cache.store(&space, &fresh, &first);
    cache.store(&space, &resumed_after(200), &second);

    assert!(
      cache
        .get(&space, &resumed_after(200))
        .is_some_and(|result| Rc::ptr_eq(&result, &second))
    );
    // A fragment resumed elsewhere lays out anew.
    assert!(cache.get(&space, &resumed_after(150)).is_none());

    // A new layout of the first fragment leaves the second stale.
    cache.store(&space, &fresh, &first);

    assert!(cache.get(&space, &resumed_after(200)).is_none());
  }
}
