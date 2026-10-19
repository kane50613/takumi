use std::{
  borrow::Cow,
  cell::RefCell,
  collections::{HashMap, HashSet},
  hash::Hasher,
  iter::Copied,
  mem::{replace, take},
  ptr,
  rc::Rc,
  slice,
  vec::IntoIter,
};

use smallvec::SmallVec;
use taffy::{
  AvailableSpace as TaffyAvailableSpace, AxisStaticEdge, AxisStaticPosition, BlockContext,
  BoxSizing, Cache, CacheTree, ContainingBlockClaims, CoreStyle, DetailedGridInfo,
  DetailedLayoutInfo, Dimension, Display as TaffyDisplay, Layout, LayoutBlockContainer,
  LayoutContainingBlock, LayoutFlexboxContainer, LayoutGridContainer, LayoutInput, LayoutOutput,
  LayoutPartialTree, LengthPercentageAuto, Line, MaybeMath, MaybeResolve, NodeId as TaffyNodeId,
  OofCandidate, OofPositioningArea, Position as TaffyPosition, RequestedAxis, ResolveOrZero,
  RoundTree, RunMode, Size as TaffySize, SizingMode, Style, TraversePartialTree, TraverseTree,
  compute_block_layout, compute_cached_layout, compute_flexbox_layout, compute_grid_layout,
  compute_hidden_layout, compute_leaf_layout, compute_oof_layout_for_area, compute_root_layout,
};
use xxhash_rust::xxh3::Xxh3;

use crate::{
  Error,
  context::RenderContext,
  font_style::SizedFontStyle,
  geometry::{AvailableSpace, ComputedLayout, NodeId, Point, Rect, Size},
  layout::{
    inline::{
      ContentBox, FragmentItems, InlineContentKind, InlineOutOfFlow, PaddingBox, StaticPosition,
      inline_node::InlineNode,
    },
    list_marker::{ListCounter, is_list_element, list_marker, owns_list_counter},
    node::{Node, NodeKind, NodeStyleLayers},
    table::MAX_GRID_ROWS,
    table_columns::TableColumns,
  },
  matching::{MatchedDeclarationsView, NodeMatchedDeclarations, match_stylesheets_view},
  painter::OwnContent,
  resources::font::PrimaryFontMetrics,
  sort_key::sort_by_key,
  style::{
    Affine, BackgroundImage, BackgroundImages, Color, ComputedStyle, ContentItem, ContentValue,
    Display, Float, GridPlacement, Length, LineHeight, ListStylePosition, Position, SizingContext,
    Style as NodeStyle, StyleDeclaration, StyleDeclarationBlock, StyleSheet, TwBlocks,
    WhiteSpaceCollapse, apply_stylesheet_animations,
  },
  viewport::Viewport,
};

/// A render-tree child paired with its layout node id.
#[derive(Debug, Clone)]
pub struct OrderedChild {
  /// Index of the child, or of the inline box holding it, in its parent's render order.
  pub render_index: usize,
  /// The rest of the path to an out-of-flow box inside the parent's inline content.
  pub inline_path: Option<Box<[usize]>>,
  /// Layout node id.
  pub node_id: NodeId,
  /// Containing block the child was hoisted to, if out-of-flow.
  pub hoisted_cb: Option<NodeId>,
}

impl OrderedChild {
  /// Extends `path`, its parent's, to the child.
  pub(crate) fn extend_path(&self, path: &mut Vec<usize>) {
    path.push(self.render_index);
    path.extend(self.inline_path.iter().flatten());
  }
}

/// Each visited node's placement and content box, kept so a hoisted out-of-flow child resolves
/// against its containing block instead of its box-tree parent. A placement is what a child
/// starts from: a device transform, or one with the paint offset beside it.
pub(crate) struct ContainingBlocks<P = Affine> {
  placements: HashMap<NodeId, P>,
  content_boxes: HashMap<NodeId, Size<Option<f32>>>,
}

impl<P> Default for ContainingBlocks<P> {
  fn default() -> Self {
    Self {
      placements: HashMap::new(),
      content_boxes: HashMap::new(),
    }
  }
}

impl<P: Copy> ContainingBlocks<P> {
  /// Records the placement a node's children start from.
  pub(crate) fn record_placement(&mut self, node_id: NodeId, placement: P) {
    self.placements.insert(node_id, placement);
  }

  /// Records the content box a node lays its children out in.
  pub fn record_content_box(&mut self, node_id: NodeId, content_box: Size<Option<f32>>) {
    self.content_boxes.insert(node_id, content_box);
  }

  /// The placement and container size `child` resolves against: its containing block's when
  /// hoisted, otherwise the parent's.
  pub fn base_for(
    &self,
    child: &OrderedChild,
    parent_placement: P,
    parent_content_box: Size<Option<f32>>,
  ) -> (P, Size<Option<f32>>) {
    match child.hoisted_cb {
      Some(cb) => (
        self
          .placements
          .get(&cb)
          .copied()
          .unwrap_or(parent_placement),
        self
          .content_boxes
          .get(&cb)
          .copied()
          .unwrap_or(parent_content_box),
      ),
      None => (parent_placement, parent_content_box),
    }
  }

  /// The placement `child` resolves against, or `None` when it was hoisted to a containing block
  /// none of the recorded nodes is.
  pub(crate) fn recorded_placement_for(
    &self,
    child: &OrderedChild,
    parent_placement: P,
  ) -> Option<P> {
    match child.hoisted_cb {
      Some(cb) => self.placements.get(&cb).copied(),
      None => Some(parent_placement),
    }
  }
}

/// Immutable per-node layout output after computing a tree.
pub struct LayoutResults {
  nodes: Vec<LayoutResultNode>,
}

struct LayoutResultNode {
  layout: Layout,
  first_baseline_y: Option<f32>,
  box_children: Box<[OrderedChild]>,
}

impl LayoutResults {
  /// Lays out the tree under `root`, a box its parent placed, in `available_space`. The root's
  /// margins shrink the space but leave its border box at the origin, where the parent expects it.
  pub fn compute(root: &RenderNode, available_space: Size<AvailableSpace>) -> Self {
    let mut results = Self::lay_out(root, available_space, false);

    if let Some(root) = results.nodes.first_mut() {
      root.layout.location = taffy::Point::ZERO;
    }
    results
  }

  /// [`Self::compute`] for paint: each inline formatting context keeps the fragment items it
  /// lays out to, and lets go of the shaped text no other box shares.
  pub(crate) fn compute_for_paint(
    root: &RenderNode,
    available_space: Size<AvailableSpace>,
  ) -> Self {
    Self::lay_out(root, available_space, true)
  }

  fn lay_out(
    root: &RenderNode,
    available_space: Size<AvailableSpace>,
    keeps_fragment_items: bool,
  ) -> Self {
    let mut tree = LayoutTree::from_render_node(root);

    tree.keeps_fragment_items = keeps_fragment_items;
    tree.compute_layout(available_space);
    tree.into_results()
  }

  /// Computed layout of a node.
  pub fn layout(&self, node_id: NodeId) -> crate::Result<ComputedLayout> {
    self
      .node(node_id)
      .map(|node| ComputedLayout::from_taffy(&node.layout))
  }

  /// Paint-ordered children of a node.
  pub fn box_children(&self, node_id: NodeId) -> crate::Result<&[OrderedChild]> {
    self.node(node_id).map(|node| node.box_children.as_ref())
  }

  pub(crate) fn first_baseline_y(&self, node_id: NodeId) -> crate::Result<Option<f32>> {
    self.node(node_id).map(|node| node.first_baseline_y)
  }

  /// The root's border-box size, zero when the tree is empty.
  pub(super) fn root_size(&self) -> Size<f32> {
    self
      .node(NodeId::ROOT)
      .map_or(Size::ZERO, |node| Size::from_taffy(node.layout.size))
  }

  fn node(&self, node_id: NodeId) -> crate::Result<&LayoutResultNode> {
    self
      .nodes
      .get(usize::from(node_id))
      .ok_or(Error::InvalidLayoutNode(node_id.into()))
  }
}

/// A measurement's constraint as bits: each axis's available space as a kind and a size, then
/// each known size, the height ones left out unless `with_height`.
fn inline_measure_bits(
  available_space: Size<AvailableSpace>,
  known_dimensions: Size<Option<f32>>,
  with_height: bool,
) -> [u32; 6] {
  let available = |space: AvailableSpace| match space {
    AvailableSpace::Definite(value) => (0, value.to_bits()),
    AvailableSpace::MinContent => (1, 0),
    AvailableSpace::MaxContent => (2, 0),
  };
  let known = |value: Option<f32>| value.unwrap_or(f32::NAN).to_bits();
  let (width_kind, width) = available(available_space.width);
  let (height_kind, height, known_height) = if with_height {
    let (kind, height) = available(available_space.height);

    (kind, height, known(known_dimensions.height))
  } else {
    (u32::MAX, 0, 0)
  };

  [
    width_kind,
    width,
    height_kind,
    height,
    known(known_dimensions.width),
    known_height,
  ]
}

/// Mutable taffy tree wrapping render nodes during layout.
pub struct LayoutTree<'r> {
  nodes: Vec<LayoutNodeState>,
  render_nodes: Vec<&'r RenderNode>,
  /// The nodes standing in for inline containing blocks.
  inline_containing_blocks: Vec<TaffyNodeId>,
  /// Each inline formatting context's measured size per constraint. taffy keys its own cache on
  /// both axes, while only inline boxes see the available height, so a context without them is
  /// keyed on its width constraint alone.
  inline_measures: RefCell<HashMap<InlineMeasureKey, Size<f32>>>,
  /// Whether each inline formatting context keeps the fragment items it lays out to, as a layout
  /// that paint reads does.
  keeps_fragment_items: bool,
}

/// A node and the constraint it is measured under, as bits.
type InlineMeasureKey = (usize, [u32; 6]);

struct LayoutNodeState {
  style: Style,
  /// Whether the style is the same whatever query container it resolves
  /// against, so taffy's repeated passes can reuse it.
  container_independent: bool,
  cache: Cache,
  unrounded_layout: Layout,
  final_layout: Layout,
  first_baseline_y: Option<f32>,
  is_inline_children: bool,
  /// Whether the node is a flex or grid item, which its container may stretch.
  flex_or_grid_item: bool,
  /// How far a table cell's content moves down to its row's baseline.
  row_baseline_shift: f32,
  children: Box<[TaffyNodeId]>,
  box_children: Box<[OrderedChild]>,
  /// The out-of-flow boxes this node positioned as their containing block, from its last layout.
  hoisted_children: Vec<TaffyNodeId>,
  /// The grid this node last laid out, which places its out-of-flow children in their areas.
  detailed_layout_info: DetailedLayoutInfo<String>,
  /// The out-of-flow boxes this node is the containing block for.
  claims: ContainingBlockClaims,
}

impl LayoutNodeState {
  fn new(
    style: Style,
    container_independent: bool,
    is_inline_children: bool,
    claims: ContainingBlockClaims,
  ) -> Self {
    Self {
      style,
      container_independent,
      cache: Cache::new(),
      unrounded_layout: Layout::new(),
      final_layout: Layout::new(),
      first_baseline_y: None,
      is_inline_children,
      flex_or_grid_item: false,
      row_baseline_shift: 0.0,
      children: Box::new([]),
      box_children: Box::new([]),
      hoisted_children: Vec::new(),
      detailed_layout_info: DetailedLayoutInfo::None,
      claims,
    }
  }
}

/// Who created a box: the author's node tree, or a layout-generated construct.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeOrigin {
  /// An authored node, at its document-order position in the source tree.
  Authored {
    /// Position in the source tree, counted in document order.
    source_order: usize,
  },
  /// A generated `::marker` box.
  Marker,
  /// A generated `::before`/`::after` box.
  Pseudo,
  /// An anonymous box layout invented, such as an inline-text wrapper.
  Anonymous,
}

/// A box's source-table role before lowering.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TablePart {
  /// The lowered `display: table` box itself.
  Table,
  /// A `table-caption` box.
  Caption,
  /// A cell from a `table-header-group` row.
  HeaderCell,
  /// A cell from a body row.
  BodyCell,
  /// A cell from a `table-footer-group` row.
  FooterCell,
  /// A run of body rows lowered onto a grid of its own, for a table taller than one grid.
  BodyRows,
}

impl TablePart {
  /// Whether the box is a cell.
  pub(crate) fn is_cell(self) -> bool {
    matches!(self, Self::HeaderCell | Self::BodyCell | Self::FooterCell)
  }
}

/// A styled node plus its children, ready for layout.
#[derive(Clone)]
pub struct RenderNode {
  /// Resolved style and rendering context.
  pub context: RenderContext,
  /// Source node, absent for anonymous wrappers.
  pub node: Option<Node>,
  /// Who created this box.
  pub origin: NodeOrigin,
  /// Child render nodes.
  pub children: Option<Box<[RenderNode]>>,
  pub(crate) layout_style_override: Option<Rc<Style>>,
  /// Text for an anonymous inline-text wrapper.
  pub anonymous_text_content: Option<String>,
  /// Generated marker box, emitted before this box's own inline content.
  pub(crate) marker: Option<Box<RenderNode>>,
  pub(crate) force_inline_layout: bool,
  /// Grid lines a lowered table's header rows cover, as `[start, end)`, for
  /// paged output to repeat per css-tables-3 §repeated-headers.
  pub table_header_lines: Option<(i16, i16)>,
  /// Grid lines a lowered table's footer rows cover, as `[start, end)`, for
  /// paged output to repeat per css-tables-3 §repeated-headers.
  pub table_footer_lines: Option<(i16, i16)>,
  /// The role this box had in a source table, kept through table lowering.
  pub table_part: Option<TablePart>,
  /// A lowered table's columns, sized once its width is known.
  pub(crate) table_columns: Option<Box<TableColumns>>,
  /// The fragment items its inline content last laid out to.
  pub(crate) fragment_items: FragmentItemsCache,
}

/// Drops the render tree iteratively; recursive drop glue overflows the stack
/// on deep user trees, same reason `Node` has an iterative `Drop`.
impl Drop for RenderNode {
  fn drop(&mut self) {
    let mut stack: Vec<RenderNode> = Vec::new();
    let collect = |node: &mut RenderNode, stack: &mut Vec<RenderNode>| {
      if let Some(children) = node.children.take() {
        stack.extend(children.into_vec());
      }
      if let Some(marker) = node.marker.take() {
        stack.push(*marker);
      }
    };

    collect(self, &mut stack);
    while let Some(mut node) = stack.pop() {
      collect(&mut node, &mut stack);
    }
  }
}

/// The fragment items a node's inline content last laid out to. They belong to that node, so a
/// clone, which may change its style or content, starts without them.
#[derive(Default)]
pub(crate) struct FragmentItemsCache(RefCell<Option<Rc<FragmentItems>>>);

impl FragmentItemsCache {
  pub(crate) fn keep(&self, items: Rc<FragmentItems>) {
    self.0.replace(Some(items));
  }
}

impl Clone for FragmentItemsCache {
  fn clone(&self) -> Self {
    Self::default()
  }
}

thread_local! {
  /// The layout an anonymous block box takes in place of a computed style, shared by all of them.
  static BLOCK_STYLE: Rc<Style> = Rc::new(Style {
    display: TaffyDisplay::Block,
    ..Style::default()
  });
}

fn block_style() -> Rc<Style> {
  BLOCK_STYLE.with(Rc::clone)
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct AtomicInlineMetrics {
  pub(crate) size: Size<f32>,
  pub(crate) baseline_offset: Option<f32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum InlineBaselineSource {
  InlineContentLastLine,
  LayoutFirstBaseline,
}

/// An element's own important declarations by cascade tier.
struct ElementImportant {
  tw: Option<Rc<TwBlocks>>,
  inline: Option<StyleDeclarationBlock>,
}

fn registered_custom_property_parent_style<'a>(
  parent_style: &'a ComputedStyle,
  stylesheets: &[StyleSheet],
  viewport: Viewport,
) -> Cow<'a, ComputedStyle> {
  if stylesheets
    .iter()
    .all(|sheet| sheet.property_rules().is_empty())
  {
    return Cow::Borrowed(parent_style);
  }

  let mut adjusted_parent = parent_style.clone();

  for sheet in stylesheets {
    for property_rule in sheet.property_rules() {
      if !property_rule
        .media_queries
        .iter()
        .all(|media_query| media_query.matches(viewport))
      {
        continue;
      }

      adjusted_parent
        .custom_properties
        .register_in_scope(property_rule, &parent_style.custom_properties);
    }
  }

  Cow::Owned(adjusted_parent)
}

/// Whether a node claiming `claims` is the containing block of a box with `position`.
fn claims_box(claims: ContainingBlockClaims, position: Position) -> bool {
  match position {
    Position::Absolute => claims.absolute,
    Position::Fixed => claims.fixed,
    Position::Static | Position::Relative => false,
  }
}

impl<'r> LayoutTree<'r> {
  /// Builds a layout tree from a render-node root.
  pub fn from_render_node(render_root: &'r RenderNode) -> Self {
    let mut tree = Self {
      nodes: Vec::with_capacity(1),
      render_nodes: Vec::with_capacity(1),
      inline_containing_blocks: Vec::new(),
      inline_measures: RefCell::default(),
      keeps_fragment_items: false,
    };
    let root_id = tree.push_subtree(render_root);

    debug_assert_eq!(root_id, TaffyNodeId::from(0usize));

    tree
  }

  /// Appends the layout nodes of the subtree under `render_root`, returning its root id.
  fn push_subtree(&mut self, render_root: &'r RenderNode) -> TaffyNodeId {
    /// What the walk descends into: a node's render children, or for an inline formatting
    /// context the out-of-flow boxes inside its inline content.
    enum PendingChildren<'r> {
      Render(&'r [RenderNode]),
      OutOfFlow(Vec<InlineOutOfFlow<'r>>),
    }

    impl<'r> PendingChildren<'r> {
      fn get(&self, index: usize) -> Option<InlineOutOfFlow<'r>> {
        match self {
          Self::Render(children) => children.get(index).map(|child| InlineOutOfFlow {
            path: SmallVec::from_elem(index, 1),
            node: child,
            container: None,
          }),
          Self::OutOfFlow(children) => children.get(index).cloned(),
        }
      }

      fn len(&self) -> usize {
        match self {
          Self::Render(children) => children.len(),
          Self::OutOfFlow(children) => children.len(),
        }
      }
    }

    struct PendingNode<'r> {
      node_id: TaffyNodeId,
      render_node: &'r RenderNode,
      /// The path from its parent, as its parent's [`OrderedChild`] records it.
      render_path: SmallVec<[usize; 2]>,
      /// The inline span containing it, when it is out of flow.
      inline_container: Option<&'r RenderNode>,
      /// For an inline formatting context, the node standing in for each inline span, or the
      /// context itself, that contains out-of-flow boxes.
      inline_containing_blocks: Vec<(&'r RenderNode, TaffyNodeId)>,
      position: Position,
      next_child_index: usize,
      children: PendingChildren<'r>,
      taffy_child_ids: Vec<TaffyNodeId>,
      box_children: Vec<OrderedChild>,
    }

    fn push_node_state<'r>(
      nodes: &mut Vec<LayoutNodeState>,
      render_nodes: &mut Vec<&'r RenderNode>,
      InlineOutOfFlow {
        path: render_path,
        node: render_node,
        container: inline_container,
      }: InlineOutOfFlow<'r>,
    ) -> PendingNode<'r> {
      let node_index = nodes.len();
      let node_id = TaffyNodeId::from(node_index);
      let is_inline_children = render_node.should_create_inline_layout();
      let children = if is_inline_children {
        PendingChildren::OutOfFlow(render_node.inline_out_of_flow())
      } else {
        PendingChildren::Render(render_node.children.as_deref().unwrap_or_default())
      };
      let position = render_node.context.style.base_data.position;
      let contains_fixed = render_node.context.style.contains_fixed_descendants();

      render_nodes.push(render_node);

      let (style, container_independent) = {
        let sizing = &render_node.context.sizing;

        // Resolution reports whether it read the query container, which is
        // exact. Comparing two resolved sizes is not: `min(10px, 100cqw)`
        // agrees across two large containers and disagrees with a small one.
        sizing.container_read.set(false);
        let style = render_node.layout_style(sizing);
        // A table's width follows the space it is offered.
        let independent = !sizing.container_read.get() && render_node.table_columns.is_none();

        (style, independent)
      };

      nodes.push(LayoutNodeState::new(
        style,
        container_independent,
        is_inline_children,
        ContainingBlockClaims {
          absolute: position.is_positioned() || contains_fixed,
          fixed: contains_fixed,
        },
      ));

      let capacity = children.len();
      PendingNode {
        node_id,
        render_node,
        render_path,
        inline_container,
        inline_containing_blocks: Vec::new(),
        position,
        next_child_index: 0,
        children,
        taffy_child_ids: Vec::with_capacity(capacity),
        box_children: Vec::with_capacity(capacity),
      }
    }

    /// Appends a block standing in for `container`, laid out over the padding box its fragments
    /// bound.
    fn push_inline_containing_block<'r>(
      nodes: &mut Vec<LayoutNodeState>,
      render_nodes: &mut Vec<&'r RenderNode>,
      container: &'r RenderNode,
    ) -> TaffyNodeId {
      let node_id = TaffyNodeId::from(nodes.len());

      render_nodes.push(container);
      nodes.push(LayoutNodeState::new(
        Style {
          display: TaffyDisplay::Block,
          ..Style::DEFAULT
        },
        true,
        false,
        ContainingBlockClaims::ALL,
      ));
      node_id
    }

    let Self {
      nodes,
      render_nodes,
      inline_containing_blocks,
      ..
    } = self;
    let mut proxy_boxes: HashMap<TaffyNodeId, Vec<TaffyNodeId>> = HashMap::new();

    let root = push_node_state(
      nodes,
      render_nodes,
      InlineOutOfFlow {
        path: SmallVec::new(),
        node: render_root,
        container: None,
      },
    );
    let root_id = root.node_id;
    let mut stack = vec![root];

    while let Some(current) = stack.last_mut() {
      if let Some(child) = current.children.get(current.next_child_index) {
        current.next_child_index += 1;
        let pending = push_node_state(nodes, render_nodes, child);

        stack.push(pending);
        continue;
      }

      let Some(finished) = stack.pop() else {
        break;
      };
      let fid = finished.node_id;

      let mut taffy_children = finished.taffy_child_ids;
      for &(_, proxy) in &finished.inline_containing_blocks {
        nodes[usize::from(proxy)].children = proxy_boxes
          .remove(&proxy)
          .unwrap_or_default()
          .into_boxed_slice();
      }
      let idx: usize = fid.into();
      if matches!(
        nodes[idx].style.display,
        TaffyDisplay::Flex | TaffyDisplay::Grid
      ) {
        for &child in &taffy_children {
          let child = &mut nodes[usize::from(child)];

          child.flex_or_grid_item = !child.style.position.is_out_of_flow();
        }
        sort_children_by_order(&mut taffy_children, |child_id| {
          let child_idx: usize = child_id.into();
          render_nodes
            .get(child_idx)
            .map_or(0, |child| child.context.style.misc3_data.order.0)
        });
      }
      nodes[idx].children = taffy_children.into_boxed_slice();
      nodes[idx].box_children = finished.box_children.into_boxed_slice();

      if let Some(parent) = stack.last_mut() {
        let parent_state = &nodes[usize::from(parent.node_id)];
        let inline_container = finished.inline_container.or_else(|| {
          (parent_state.is_inline_children && claims_box(parent_state.claims, finished.position))
            .then_some(parent.render_node)
        });

        // A box an inline span contains, or the inline formatting context it sits in, goes under
        // a node standing in for that span, which the context lays out once its lines are known.
        // Any other box an inline formatting context holds escapes it as a taffy candidate, and
        // the rest stay with their parent while taffy carries them to their containing block.
        if let Some(container) = inline_container {
          let proxy = parent
            .inline_containing_blocks
            .iter()
            .find(|(span, _)| ptr::eq(*span, container))
            .map(|&(_, proxy)| proxy)
            .unwrap_or_else(|| {
              let proxy = push_inline_containing_block(nodes, render_nodes, container);

              inline_containing_blocks.push(proxy);
              parent.inline_containing_blocks.push((container, proxy));
              parent.taffy_child_ids.push(proxy);
              proxy
            });

          proxy_boxes.entry(proxy).or_default().push(fid);
        } else if !parent_state.is_inline_children {
          parent.taffy_child_ids.push(fid);
        }

        let (&render_index, inline_path) = finished.render_path.split_first().unwrap_or((&0, &[]));

        parent.box_children.push(OrderedChild {
          render_index,
          inline_path: (!inline_path.is_empty()).then(|| inline_path.into()),
          node_id: NodeId::from_taffy(fid),
          hoisted_cb: None,
        });
      }
    }

    root_id
  }

  /// Computes the layout for the whole tree.
  pub fn compute_layout(&mut self, available_space: Size<AvailableSpace>) {
    let root_node_id = NodeId::ROOT.into_taffy();

    self.stretch_root(available_space.width);
    compute_root_layout(
      self,
      root_node_id,
      available_space.map(AvailableSpace::into_taffy).into_taffy(),
    );
    self.finalize_layout();
  }

  /// Gives a block-level flex, grid or flow-root root with an `auto` width the width a definite
  /// `available_width` stretches it to, as CSS sizes every block-level box but a table. Taffy
  /// stretches only a block root, and an atomic inline laid out as a root shrinks to fit.
  fn stretch_root(&mut self, available_width: AvailableSpace) {
    let AvailableSpace::Definite(available) = available_width else {
      return;
    };
    let stretches = self.render_nodes.first().is_some_and(|root| {
      matches!(
        root.context.style.base_data.display,
        Display::Flex | Display::WebkitBox | Display::Grid | Display::FlowRoot
      )
    });
    let style = &self.nodes[0].style;

    if !stretches || !style.size.width.is_auto() {
      return;
    }

    let calc = |value, basis| self.resolve_calc_value(value, basis);
    let margin = style.margin.resolve_or_zero(Some(available), calc);
    let mut width = available - margin.horizontal_axis_sum();

    if style.box_sizing == BoxSizing::ContentBox {
      width -= style
        .padding
        .resolve_or_zero(Some(available), calc)
        .horizontal_axis_sum()
        + style
          .border
          .resolve_or_zero(Some(available), calc)
          .horizontal_axis_sum();
    }
    self.nodes[0].style.size.width = Dimension::length(width.max(0.0));
  }

  /// Places the out-of-flow boxes inside the inline formatting context `node` lays out as
  /// `layout`: the ones an inline span or the context contains under the nodes standing in for
  /// them, and the rest as candidates in `output` for taffy to carry to their containing block.
  /// Kept out of the recursive layout closure, whose frame every level of the tree pays for.
  #[inline(never)]
  fn lay_out_inline_out_of_flow(
    &mut self,
    node: TaffyNodeId,
    _available_space: Size<AvailableSpace>,
    layout: ComputedLayout,
    output: &mut LayoutOutput,
  ) {
    let state = &self.nodes[usize::from(node)];
    let render_node = self.render_nodes[usize::from(node)];
    let proxies = state.children.clone();
    let boxes: Vec<TaffyNodeId> = state
      .box_children
      .iter()
      .map(|child| child.node_id.into_taffy())
      .collect();
    let containers: Vec<&RenderNode> = proxies
      .iter()
      .map(|&proxy| self.render_nodes[usize::from(proxy)])
      .collect();
    let (positions, containing_blocks) =
      render_node.inline_out_of_flow_geometry(layout, &containers);
    // Static positions arrive in the content box; candidates sit in the border box.
    let content = Point {
      x: layout.border.left + layout.padding.left,
      y: layout.border.top + layout.padding.top,
    };
    let positions: HashMap<TaffyNodeId, StaticPosition> = boxes
      .iter()
      .zip(positions)
      .map(|(&node, position)| {
        (
          node,
          StaticPosition {
            point: content + position.point,
            ..position
          },
        )
      })
      .collect();

    for (&proxy, padding_box) in proxies.iter().zip(containing_blocks) {
      self.lay_out_inline_containing_block(proxy, padding_box, &positions);
    }

    for (order, &node) in boxes.iter().enumerate() {
      let held = proxies
        .iter()
        .any(|&proxy| self.nodes[usize::from(proxy)].children.contains(&node));

      if let (false, Some(position)) = (held, positions.get(&node)) {
        output.oof_candidates.push(self.out_of_flow_candidate(
          node,
          order,
          position.point,
          position.from_end,
        ));
      }
    }
  }

  /// Lays out `proxy`, standing in for an inline containing block, over its padding box, and
  /// positions the out-of-flow boxes it contains from their static positions in `positions`.
  fn lay_out_inline_containing_block(
    &mut self,
    proxy: TaffyNodeId,
    padding_box: PaddingBox,
    positions: &HashMap<TaffyNodeId, StaticPosition>,
  ) {
    let PaddingBox { origin, size } = padding_box;
    let candidates: Vec<OofCandidate> = self.nodes[usize::from(proxy)]
      .children
      .iter()
      .enumerate()
      .filter_map(|(order, &node)| {
        let position = positions.get(&node)?;

        Some(self.out_of_flow_candidate(node, order, position.point - origin, position.from_end))
      })
      .collect();
    let direction = self.render_nodes[usize::from(proxy)]
      .context
      .style
      .inherited_data
      .direction
      .into_taffy();
    let result = compute_oof_layout_for_area(
      self,
      proxy,
      &candidates,
      OofPositioningArea {
        size: size.into_taffy(),
        offset: taffy::Point::ZERO,
      },
      direction,
      ContainingBlockClaims::ALL,
    );

    self.clear_hoisted_children(proxy);
    self.add_hoisted_children(proxy, &result.hoisted);
    self.set_unrounded_layout(
      proxy,
      &Layout {
        location: origin.into_taffy(),
        size: size.into_taffy(),
        ..Layout::new()
      },
    );
  }

  /// The taffy candidate the out-of-flow `node` is, its margin box's inline-start corner at
  /// `point` in the border box of the node holding it.
  fn out_of_flow_candidate(
    &self,
    node: TaffyNodeId,
    order: usize,
    point: Point<f32>,
    from_end: bool,
  ) -> OofCandidate {
    OofCandidate {
      node,
      order: order as u32,
      position: self.nodes[usize::from(node)].style.position,
      static_position: taffy::Point {
        x: AxisStaticPosition::from_edge(
          point.x,
          if from_end {
            AxisStaticEdge::End
          } else {
            AxisStaticEdge::Start
          },
        ),
        y: AxisStaticPosition::from_edge(point.y, AxisStaticEdge::Start),
      },
    }
  }

  /// Keeps every box where it laid out, unrounded: Blink holds layout in layout units and snaps a
  /// box to pixels only as it paints it.
  fn finalize_layout(&mut self) {
    for node in &mut self.nodes {
      node.final_layout = node.unrounded_layout;
    }
  }

  /// The root's border-box min-content and max-content widths, sized without laying out its
  /// height, as Blink's `ComputeMinMaxSizes` sizes a box's inline axis alone.
  pub(crate) fn root_min_max_widths(&mut self) -> (f32, f32) {
    let mut width = |available_width: TaffyAvailableSpace| {
      let available_space = TaffySize {
        width: available_width,
        height: TaffyAvailableSpace::MaxContent,
      };

      self
        .compute_child_layout(
          NodeId::ROOT.into_taffy(),
          LayoutInput {
            run_mode: RunMode::ComputeSize,
            sizing_mode: SizingMode::InherentSize,
            axis: RequestedAxis::Horizontal,
            known_dimensions: TaffySize::NONE,
            known_dimensions_are_definite: TaffySize {
              width: true,
              height: true,
            },
            parent_size: available_space.into_options(),
            available_space,
            vertical_margins_are_collapsible: Line::FALSE,
          },
        )
        .size
        .width
    };

    (
      width(TaffyAvailableSpace::MinContent),
      width(TaffyAvailableSpace::MaxContent),
    )
  }

  /// Consumes the tree into immutable per-node layout results.
  pub(crate) fn into_results(mut self) -> LayoutResults {
    // A box an inline containing block holds is placed in the inline formatting context, its
    // box-tree parent, rather than in the node standing in for the containing block.
    for &proxy in &self.inline_containing_blocks {
      let proxy = &self.nodes[usize::from(proxy)];
      let (location, unrounded) = (proxy.final_layout.location, proxy.unrounded_layout.location);

      for child in proxy.hoisted_children.clone() {
        let child = &mut self.nodes[usize::from(child)];

        child.final_layout.location = child.final_layout.location + location;
        child.unrounded_layout.location = child.unrounded_layout.location + unrounded;
      }
    }

    // A box another node than its box-tree parent positioned resolves against that containing
    // block. Inline containing blocks are left out: their boxes now sit in their parent's space.
    let mut containing_blocks: HashMap<TaffyNodeId, TaffyNodeId> = HashMap::new();
    let proxies: HashSet<TaffyNodeId> = self.inline_containing_blocks.iter().copied().collect();

    for (index, node) in self.nodes.iter().enumerate() {
      if !proxies.contains(&TaffyNodeId::from(index)) {
        for &child in &node.hoisted_children {
          containing_blocks.insert(child, TaffyNodeId::from(index));
        }
      }
    }
    for (index, node) in self.nodes.iter_mut().enumerate() {
      for child in node.box_children.iter_mut() {
        child.hoisted_cb = containing_blocks
          .get(&child.node_id.into_taffy())
          .filter(|&&cb| cb != TaffyNodeId::from(index))
          .map(|&cb| NodeId::from_taffy(cb));
      }
    }

    // A fresh allocation: collecting in place keeps the node states' much larger one alive.
    let mut nodes = Vec::with_capacity(self.nodes.len());

    nodes.extend(self.nodes.into_iter().map(|node| LayoutResultNode {
      layout: node.final_layout,
      first_baseline_y: node.first_baseline_y,
      box_children: node.box_children,
    }));
    LayoutResults { nodes }
  }

  fn get_layout_node_ref(&self, node_id: TaffyNodeId) -> Option<&LayoutNodeState> {
    self.nodes.get(usize::from(node_id))
  }

  fn get_layout_node_mut_ref(&mut self, node_id: TaffyNodeId) -> Option<&mut LayoutNodeState> {
    self.nodes.get_mut(usize::from(node_id))
  }

  fn update_node_style_for_available_space(
    &mut self,
    node_id: TaffyNodeId,
    available_space: Size<AvailableSpace>,
    known_dimensions: Size<Option<f32>>,
  ) {
    let idx = usize::from(node_id);
    let (Some(node), Some(render_node)) = (self.nodes.get_mut(idx), self.render_nodes.get(idx))
    else {
      return;
    };

    if node.container_independent {
      return;
    }

    let mut sizing = render_node.context.sizing.clone();

    sizing.container_size = Size {
      width: known_dimensions
        .width
        .or(available_space.width.into_option()),
      height: known_dimensions
        .height
        .or(available_space.height.into_option()),
    };
    node.style = render_node.layout_style(&sizing);
    if node.row_baseline_shift != 0.0 {
      node.style.margin.top = LengthPercentageAuto::length(node.row_baseline_shift);
    }
    render_node.size_table(
      &mut node.style,
      available_space.width,
      known_dimensions.width,
      node.flex_or_grid_item,
      &sizing,
    );

    if render_node.table_columns.is_some() {
      self.share_column_tracks(node_id);
    }
  }

  /// Gives a lowered table's [`TablePart::BodyRows`] grids the column tracks it just sized.
  fn share_column_tracks(&mut self, table: TaffyNodeId) {
    let mut changed = false;

    let Some(state) = self.get_layout_node_ref(table) else {
      return;
    };
    let tracks = state.style.grid_template_columns.clone();
    let children = state.children.clone();

    for child in children {
      let index = usize::from(child);

      if self
        .render_nodes
        .get(index)
        .is_none_or(|rows| rows.table_part != Some(TablePart::BodyRows))
      {
        continue;
      }
      if let Some(rows) = self.nodes.get_mut(index)
        && rows.style.grid_template_columns != tracks
      {
        rows.style.grid_template_columns.clone_from(&tracks);
        rows.cache.clear();
        changed = true;
      }
    }

    // taffy keys the table's cache on its inputs, which can stay the same while the tracks move:
    // a known width with percentage padding resolves against the available width.
    if changed && let Some(state) = self.get_layout_node_mut_ref(table) {
      state.cache.clear();
    }
  }
}

impl RenderNode {
  // Taffy may inject a flex stretch-derived cross-size into leaf `known_dimensions`
  // during intrinsic single-axis sizing (`ComputeSize` with `InherentSize` or `ContentSize`). For replaced
  // elements, letting that value participate in aspect-ratio transfer can
  // incorrectly inflate the measured main-size. Strip that hint at the leaf boundary.
  fn should_strip_flex_intrinsic_stretch_known_dimension(
    &self,
    inputs: LayoutInput,
    known_dimensions: Size<Option<f32>>,
  ) -> bool {
    if inputs.run_mode != RunMode::ComputeSize
      || !matches!(
        inputs.sizing_mode,
        SizingMode::InherentSize | SizingMode::ContentSize
      )
    {
      return false;
    }

    if !matches!(
      inputs.axis,
      RequestedAxis::Horizontal | RequestedAxis::Vertical
    ) {
      return false;
    }

    let Some(node) = self.node.as_ref() else {
      return false;
    };

    if !node.is_replaced_element() {
      return false;
    }

    match inputs.axis {
      RequestedAxis::Horizontal => {
        known_dimensions.width.is_none() && known_dimensions.height.is_some()
      }
      RequestedAxis::Vertical => {
        known_dimensions.height.is_none() && known_dimensions.width.is_some()
      }
      RequestedAxis::Both => false,
    }
  }
}

/// Stable-sorts flex/grid children by `order`, keeping source order for ties.
fn sort_children_by_order(
  children: &mut [TaffyNodeId],
  mut child_order: impl FnMut(TaffyNodeId) -> i32,
) {
  if children.iter().all(|&child_id| child_order(child_id) == 0) {
    return;
  }

  sort_by_key(children, |&child_id| child_order(child_id));
}

impl TraversePartialTree for LayoutTree<'_> {
  type ChildIter<'a>
    = Copied<slice::Iter<'a, TaffyNodeId>>
  where
    Self: 'a;

  fn child_ids(&self, parent_node_id: TaffyNodeId) -> Self::ChildIter<'_> {
    let Some(node) = self.get_layout_node_ref(parent_node_id) else {
      return [].iter().copied();
    };

    node.children.iter().copied()
  }

  fn child_count(&self, parent_node_id: TaffyNodeId) -> usize {
    let Some(node) = self.get_layout_node_ref(parent_node_id) else {
      return 0;
    };

    node.children.len()
  }

  fn get_child_id(&self, parent_node_id: TaffyNodeId, child_index: usize) -> TaffyNodeId {
    let Some(node) = self.get_layout_node_ref(parent_node_id) else {
      return TaffyNodeId::from(0usize);
    };

    node.children[child_index]
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
    if let Some(node) = self.get_layout_node_ref(node_id) {
      return &node.style;
    }
    &self.nodes[0].style
  }

  fn set_unrounded_layout(&mut self, node_id: TaffyNodeId, layout: &Layout) {
    let Some(node) = self.get_layout_node_mut_ref(node_id) else {
      return;
    };

    node.unrounded_layout = *layout;
    if self.keeps_fragment_items
      && let Some(render_node) = self.render_nodes.get(usize::from(node_id))
    {
      render_node.keep_fragment_items(ComputedLayout::from_taffy(layout));
    }
  }

  fn resolve_calc_value(&self, val: *const (), basis: f32) -> f32 {
    let Some(root) = self.render_nodes.first() else {
      return 0.0;
    };

    root.context.sizing.resolve_calc(val, basis)
  }

  fn compute_child_layout(&mut self, node: TaffyNodeId, inputs: LayoutInput) -> LayoutOutput {
    self.compute_child_layout_inner(node, inputs, None)
  }
}

impl<'r> LayoutTree<'r> {
  /// Positions the out-of-flow boxes `output` collected that `node` is the containing block for,
  /// leaving the rest to bubble up. Claims follow takumi's style, which taffy's cannot express.
  fn lay_out_out_of_flow(&mut self, node: TaffyNodeId, output: &mut LayoutOutput) {
    let (Some(area), Some(claims)) = (
      output.oof_positioning_area,
      self.get_layout_node_ref(node).map(|state| state.claims),
    ) else {
      return;
    };
    let direction = self.get_core_container_style(node).direction();
    let candidates = output.oof_candidates.take();
    let result =
      compute_oof_layout_for_area(self, node, candidates.as_slice(), area, direction, claims);

    self.clear_hoisted_children(node);
    self.add_hoisted_children(node, &result.hoisted);
    output.oof_candidates = result.unclaimed;
  }

  /// Lays out a grid, and the rows of a table lowered to one as Blink's `RowBaselineTabulator` aligns
  /// them: once to read its baseline-aligned cells' first baselines, then again with each one's
  /// content moved down to the deepest of its row. The deepest ascent plus descent then sizes the
  /// row, as the moved content grows the cells.
  fn compute_table_layout(&mut self, node: TaffyNodeId, inputs: LayoutInput) -> LayoutOutput {
    let cells: Vec<(TaffyNodeId, TaffyNodeId, i16)> = self
      .get_layout_node_ref(node)
      .map(|state| state.children.clone())
      .unwrap_or_default()
      .iter()
      .filter_map(|&cell| {
        let render_node = self.render_nodes.get(usize::from(cell))?;
        let GridPlacement::Line(row) = render_node.context.style.misc2_data.grid_row_start else {
          return None;
        };
        let content = *self.get_layout_node_ref(cell)?.children.first()?;

        render_node
          .aligns_to_row_baseline()
          .then_some((cell, content, row))
      })
      .collect();

    if cells.is_empty() {
      let output = compute_grid_layout(self, node, inputs);

      return self.with_first_row_baseline(node, inputs, output);
    }

    for &(cell, content, _) in &cells {
      self.shift_to_row_baseline(cell, content, 0.0);
    }
    compute_grid_layout(
      self,
      node,
      LayoutInput {
        run_mode: RunMode::PerformLayout,
        ..inputs
      },
    );

    let baselines: Vec<Option<f32>> = cells
      .iter()
      .map(|&(cell, ..)| self.get_layout_node_ref(cell)?.first_baseline_y)
      .collect();
    let mut rows: HashMap<i16, f32> = HashMap::new();

    for (&(.., row), baseline) in cells.iter().zip(&baselines) {
      if let Some(baseline) = *baseline {
        let deepest = rows.entry(row).or_insert(baseline);

        *deepest = deepest.max(baseline);
      }
    }
    for (&(cell, content, row), baseline) in cells.iter().zip(baselines) {
      if let (Some(baseline), Some(deepest)) = (baseline, rows.get(&row)) {
        self.shift_to_row_baseline(cell, content, deepest - baseline);
      }
    }

    let output = compute_grid_layout(self, node, inputs);

    self.with_first_row_baseline(node, inputs, output)
  }

  /// `output` with the grid's first baseline its first row's first item's, which taffy takes from
  /// the item's bottom edge unless the item aligns to the baseline.
  /// <https://drafts.csswg.org/css-grid-2/#grid-baselines>
  fn with_first_row_baseline(
    &self,
    node: TaffyNodeId,
    inputs: LayoutInput,
    mut output: LayoutOutput,
  ) -> LayoutOutput {
    if inputs.run_mode != RunMode::PerformLayout || output.baselines.first.is_none() {
      return output;
    }

    let Some(state) = self.get_layout_node_ref(node) else {
      return output;
    };
    let first_item = state
      .children
      .iter()
      .filter_map(|&child| self.get_layout_node_ref(child))
      .filter(|child| {
        child.style.position != TaffyPosition::Absolute && child.style.display != TaffyDisplay::None
      })
      .min_by(|left, right| {
        left
          .unrounded_layout
          .location
          .y
          .total_cmp(&right.unrounded_layout.location.y)
      });

    if let Some(item) = first_item
      && let Some(baseline) = item.first_baseline_y
    {
      output.baselines.first = Some(item.unrounded_layout.location.y + baseline);
    }

    output
  }

  /// Moves `cell`'s `content` down by `shift`, dropping what the cell laid out before.
  fn shift_to_row_baseline(&mut self, cell: TaffyNodeId, content: TaffyNodeId, shift: f32) {
    let Some(state) = self.get_layout_node_mut_ref(content) else {
      return;
    };

    if state.row_baseline_shift == shift {
      return;
    }
    state.row_baseline_shift = shift;
    state.style.margin.top = LengthPercentageAuto::length(shift);
    if let Some(cell) = self.get_layout_node_mut_ref(cell) {
      cell.cache.clear();
    }
  }

  /// The inputs a block-level `auto`-width table lays out with: without the width a block
  /// container stretches its children to, since a table shrinks to fit it instead.
  fn shrunk_table_inputs(&self, node: TaffyNodeId, mut inputs: LayoutInput) -> LayoutInput {
    let idx = usize::from(node);
    let (Some(state), Some(render_node)) = (self.nodes.get(idx), self.render_nodes.get(idx)) else {
      return inputs;
    };

    if render_node.shrinks_to_fit_as_table() && !state.flex_or_grid_item {
      inputs.known_dimensions.width = None;
    }
    inputs
  }

  /// Lays `node` and every box under it out empty, as `display: none` leaves them, without
  /// keeping fragment items no paint reads.
  fn compute_hidden(&mut self, node: TaffyNodeId) -> LayoutOutput {
    let keeps_fragment_items = replace(&mut self.keeps_fragment_items, false);
    let output = compute_hidden_layout(self, node);

    self.keeps_fragment_items = keeps_fragment_items;
    output
  }

  fn compute_child_layout_inner(
    &mut self,
    node: TaffyNodeId,
    inputs: LayoutInput,
    block_ctx: Option<&mut BlockContext<'_>>,
  ) -> LayoutOutput {
    self.update_node_style_for_available_space(
      node,
      Size::from_taffy(inputs.available_space).map(AvailableSpace::from_taffy),
      Size::from_taffy(inputs.known_dimensions),
    );

    let inputs = self.shrunk_table_inputs(node, inputs);

    if inputs.run_mode == RunMode::PerformHiddenLayout {
      return self.compute_hidden(node);
    }

    let output = compute_cached_layout(self, node, inputs, |tree, node, inputs| {
      let Some(node_data) = tree.get_layout_node_ref(node) else {
        return tree.compute_hidden(node);
      };

      let display_mode = node_data.style.display;
      // An inline formatting context's only children stand in for its inline containing blocks,
      // which it lays out itself.
      let has_children = !node_data.children.is_empty() && !node_data.is_inline_children;

      let mut output = match (display_mode, has_children) {
        (TaffyDisplay::None, _) => tree.compute_hidden(node),
        (TaffyDisplay::Block, true) => compute_block_layout(tree, node, inputs, block_ctx),
        // A flow-root box is a block formatting context root, so it never joins
        // its parent's context. <https://drafts.csswg.org/css-display-3/#valdef-display-flow-root>
        (TaffyDisplay::FlowRoot, true) => compute_block_layout(tree, node, inputs, None),
        (TaffyDisplay::Flex, true) => compute_flexbox_layout(tree, node, inputs),
        (TaffyDisplay::Grid, true) => tree.compute_table_layout(node, inputs),
        (_, false) => {
          let idx: usize = node.into();
          let Some(&render_node) = tree.render_nodes.get(idx) else {
            return tree.compute_hidden(node);
          };

          let stripped_known_dimensions = |known_dimensions: TaffySize<Option<f32>>| {
            if render_node.should_strip_flex_intrinsic_stretch_known_dimension(
              inputs,
              Size::from_taffy(known_dimensions),
            ) {
              TaffySize::NONE
            } else {
              known_dimensions
            }
          };
          let calc = |value, basis| tree.resolve_calc_value(value, basis);
          let padding = node_data
            .style
            .padding
            .resolve_or_zero(inputs.parent_size.width, calc);
          let border = node_data
            .style
            .border
            .resolve_or_zero(inputs.parent_size.width, calc);
          let insets = padding.sum_axes() + border.sum_axes();
          // The border-box size `compute_leaf_layout` gives the box before its content.
          let fixed_size =
            stripped_known_dimensions(inputs.known_dimensions).or(match inputs.sizing_mode {
              SizingMode::InherentSize => node_data
                .style
                .size
                .maybe_resolve(inputs.parent_size, calc)
                .maybe_apply_aspect_ratio(node_data.style.aspect_ratio)
                .maybe_add(if node_data.style.box_sizing == BoxSizing::ContentBox {
                  insets
                } else {
                  TaffySize::ZERO
                }),
              SizingMode::ContentSize => TaffySize::NONE,
            });

          render_node.context.set_definite_height(
            fixed_size
              .height
              .map(|height| (height - insets.height).max(0.0)),
          );

          let mut output = compute_leaf_layout(
            inputs,
            &node_data.style,
            |val, basis| tree.resolve_calc_value(val, basis),
            |known_dimensions, available_space| {
              let known_dimensions = stripped_known_dimensions(known_dimensions);

              if let TaffySize {
                width: Some(width),
                height: Some(height),
              } = known_dimensions.maybe_apply_aspect_ratio(node_data.style.aspect_ratio)
              {
                return TaffySize { width, height };
              }

              let available_space =
                Size::from_taffy(available_space).map(AvailableSpace::from_taffy);
              let known_dimensions = Size::from_taffy(known_dimensions);
              if !node_data.is_inline_children {
                return render_node
                  .measure(available_space, known_dimensions, &node_data.style)
                  .into_taffy();
              }

              let across_heights = (
                idx,
                inline_measure_bits(available_space, known_dimensions, false),
              );
              let exact = (
                idx,
                inline_measure_bits(available_space, known_dimensions, true),
              );
              let measured = {
                let measures = tree.inline_measures.borrow();

                measures
                  .get(&across_heights)
                  .or_else(|| measures.get(&exact))
                  .copied()
              };

              if let Some(size) = measured {
                return size.into_taffy();
              }

              let (size, sizes_boxes) =
                render_node.measure_inline_content(available_space, known_dimensions);

              tree
                .inline_measures
                .borrow_mut()
                .insert(if sizes_boxes { exact } else { across_heights }, size);
              size.into_taffy()
            },
          );

          let lays_out_text = node_data.is_inline_children
            || matches!(
              render_node.node.as_ref().and_then(Node::inline_content),
              Some(InlineContentKind::Text(_))
            );

          let border_box = || {
            ComputedLayout::new(
              Point::ZERO,
              Size::from_taffy(output.size),
              Rect::from_taffy(border),
              Rect::from_taffy(padding),
            )
          };

          // `compute_leaf_layout` reports no baseline, which leaves flexbox
          // baseline alignment on its bottom margin edge fallback.
          if lays_out_text && inputs.run_mode == RunMode::PerformLayout {
            let kept = tree.keeps_fragment_items.then(border_box);

            if let Some(layout) = kept {
              render_node.keep_fragment_items(layout);
            }
            output.baselines.first = render_node.inline_content_border_box_baseline(
              Size::from_taffy(inputs.available_space).map(AvailableSpace::from_taffy),
              Size::from_taffy(output.size),
              false,
              kept,
            );
          }

          // A block-level box measured whole at a definite width lays out at that size, so its
          // fragment items laid out now let its shaped text go before the boxes after it shape
          // theirs. A flex or grid item is measured at sizes its container then changes.
          if tree.keeps_fragment_items
            && lays_out_text
            && !node_data.flex_or_grid_item
            && inputs.run_mode == RunMode::ComputeSize
            && inputs.axis == RequestedAxis::Both
            && matches!(
              inputs.available_space.width,
              TaffyAvailableSpace::Definite(_)
            )
          {
            render_node.keep_fragment_items(border_box());
          }

          if node_data.is_inline_children
            && !node_data.box_children.is_empty()
            && inputs.run_mode == RunMode::PerformLayout
          {
            tree.lay_out_inline_out_of_flow(
              node,
              Size::from_taffy(inputs.available_space).map(AvailableSpace::from_taffy),
              border_box(),
              &mut output,
            );
          }

          output
        }
      };

      // Only a full layout writes the boxes it positions, which a later cache hit must still hold.
      if inputs.run_mode == RunMode::PerformLayout {
        tree.lay_out_out_of_flow(node, &mut output);
      }

      output
    });

    if let Some(node_data) = self.get_layout_node_mut_ref(node) {
      node_data.first_baseline_y = output.baselines.first;
    }

    output
  }
}

impl CacheTree for LayoutTree<'_> {
  fn cache_get(&mut self, node_id: TaffyNodeId, input: &LayoutInput) -> Option<LayoutOutput> {
    let node = self.get_layout_node_mut_ref(node_id)?;
    node.cache.get(input)
  }

  fn cache_store(
    &mut self,
    node_id: TaffyNodeId,
    input: &LayoutInput,
    layout_output: LayoutOutput,
  ) {
    let Some(node) = self.get_layout_node_mut_ref(node_id) else {
      return;
    };

    node.cache.store(input, layout_output);
  }

  fn cache_clear(&mut self, node_id: TaffyNodeId) {
    let Some(node) = self.get_layout_node_mut_ref(node_id) else {
      return;
    };

    node.cache.clear();
  }
}

impl LayoutBlockContainer for LayoutTree<'_> {
  type BlockContainerStyle<'a>
    = &'a Style
  where
    Self: 'a;
  type BlockItemStyle<'a>
    = &'a Style
  where
    Self: 'a;

  fn get_block_container_style(&self, node_id: TaffyNodeId) -> Self::BlockContainerStyle<'_> {
    self.get_core_container_style(node_id)
  }

  fn get_block_child_style(&self, child_node_id: TaffyNodeId) -> Self::BlockItemStyle<'_> {
    self.get_core_container_style(child_node_id)
  }

  fn compute_block_child_layout(
    &mut self,
    node: TaffyNodeId,
    inputs: LayoutInput,
    block_ctx: Option<&mut BlockContext<'_>>,
  ) -> LayoutOutput {
    self.compute_child_layout_inner(node, inputs, block_ctx)
  }
}

impl LayoutFlexboxContainer for LayoutTree<'_> {
  type FlexboxContainerStyle<'a>
    = &'a Style
  where
    Self: 'a;
  type FlexboxItemStyle<'a>
    = &'a Style
  where
    Self: 'a;

  fn get_flexbox_container_style(&self, node_id: TaffyNodeId) -> Self::FlexboxContainerStyle<'_> {
    self.get_core_container_style(node_id)
  }

  fn get_flexbox_child_style(&self, child_node_id: TaffyNodeId) -> Self::FlexboxItemStyle<'_> {
    self.get_core_container_style(child_node_id)
  }
}

impl LayoutGridContainer for LayoutTree<'_> {
  type GridContainerStyle<'a>
    = &'a Style
  where
    Self: 'a;
  type GridItemStyle<'a>
    = &'a Style
  where
    Self: 'a;

  fn get_grid_container_style(&self, node_id: TaffyNodeId) -> Self::GridContainerStyle<'_> {
    self.get_core_container_style(node_id)
  }

  fn get_grid_child_style(&self, child_node_id: TaffyNodeId) -> Self::GridItemStyle<'_> {
    self.get_core_container_style(child_node_id)
  }

  fn set_detailed_grid_info(&mut self, node_id: TaffyNodeId, info: DetailedGridInfo<String>) {
    if let Some(node) = self.get_layout_node_mut_ref(node_id) {
      node.detailed_layout_info = DetailedLayoutInfo::Grid(Box::new(info));
    }
  }
}

impl LayoutContainingBlock for LayoutTree<'_> {
  type OofItemStyle<'a>
    = &'a Style
  where
    Self: 'a;

  fn get_oof_item_style(&self, node_id: TaffyNodeId) -> Self::OofItemStyle<'_> {
    self.get_core_container_style(node_id)
  }

  fn clear_hoisted_children(&mut self, node_id: TaffyNodeId) {
    if let Some(node) = self.get_layout_node_mut_ref(node_id) {
      node.hoisted_children.clear();
    }
  }

  fn add_hoisted_children(&mut self, node_id: TaffyNodeId, hoisted: &[TaffyNodeId]) {
    if let Some(node) = self.get_layout_node_mut_ref(node_id) {
      node.hoisted_children.extend_from_slice(hoisted);
    }
  }

  fn get_detailed_layout_info(&self, node_id: TaffyNodeId) -> &DetailedLayoutInfo<String> {
    self
      .get_layout_node_ref(node_id)
      .map_or(&DetailedLayoutInfo::None, |node| &node.detailed_layout_info)
  }
}

impl RoundTree for LayoutTree<'_> {
  fn get_unrounded_layout(&self, node_id: TaffyNodeId) -> Layout {
    let Some(node) = self.get_layout_node_ref(node_id) else {
      return Layout::new();
    };

    node.unrounded_layout
  }

  fn set_final_layout(&mut self, node_id: TaffyNodeId, layout: &Layout) {
    let Some(node) = self.get_layout_node_mut_ref(node_id) else {
      return;
    };

    node.final_layout = *layout;
  }

  fn is_out_of_flow(&self, node_id: TaffyNodeId) -> bool {
    self.get_layout_node_ref(node_id).is_some_and(|node| {
      node.style.position.is_out_of_flow() && node.style.display != TaffyDisplay::None
    })
  }

  fn hoisted_child_count(&self, node_id: TaffyNodeId) -> usize {
    self
      .get_layout_node_ref(node_id)
      .map_or(0, |node| node.hoisted_children.len())
  }

  fn get_hoisted_child_id(&self, node_id: TaffyNodeId, index: usize) -> TaffyNodeId {
    self
      .get_layout_node_ref(node_id)
      .and_then(|node| node.hoisted_children.get(index).copied())
      // Taffy asks only below `hoisted_child_count`.
      .unwrap_or(node_id)
  }
}

impl RenderNode {
  /// The taffy style this node lays out with: its own override when it has one, otherwise
  /// its computed style.
  fn layout_style(&self, sizing: &SizingContext) -> taffy::Style {
    match self.layout_style_override.as_deref() {
      Some(style) => style.clone(),
      None => taffy::Style {
        item_is_replaced: self.node.as_ref().is_some_and(Node::is_replaced_element),
        ..self.context.style.to_taffy_style(sizing)
      },
    }
  }

  /// A box with no layout override, marker, or table role.
  pub(super) fn new(
    context: RenderContext,
    origin: NodeOrigin,
    node: Option<Node>,
    children: Option<Box<[RenderNode]>>,
  ) -> Self {
    Self {
      context,
      node,
      origin,
      children,
      layout_style_override: None,
      anonymous_text_content: None,
      marker: None,
      force_inline_layout: false,
      table_header_lines: None,
      table_footer_lines: None,
      table_part: None,
      table_columns: None,
      fragment_items: FragmentItemsCache::default(),
    }
  }

  /// Lays its inline content out to the fragment items it keeps, in the content box of `layout`,
  /// unless it keeps ones for that content box already.
  pub(crate) fn keep_fragment_items(&self, layout: ComputedLayout) {
    let font_style = SizedFontStyle::from_style(&self.context.style, &self.context);

    OwnContent::of(self).fragment_items(&font_style, layout);
  }

  /// The fragment items its inline content laid out to in the content box of `layout`, if it
  /// laid out there last.
  pub(crate) fn fragment_items_in(&self, layout: ComputedLayout) -> Option<Rc<FragmentItems>> {
    self
      .fragment_items
      .0
      .borrow()
      .as_ref()
      .filter(|items| items.content_box == ContentBox::of(layout))
      .map(Rc::clone)
  }

  /// Forgets the fragment items it and the boxes under it laid out to, which no longer hold once
  /// the tree may have changed since.
  pub(crate) fn clear_fragment_items(&self) {
    let mut stack = vec![self];

    while let Some(node) = stack.pop() {
      node.fragment_items.0.take();
      stack.extend(node.children.iter().flatten());
      stack.extend(node.marker.as_deref());
    }
  }

  /// An anonymous box laid out with `layout_style` instead of its computed style.
  fn anonymous(
    context: RenderContext,
    node: Option<Node>,
    children: Option<Box<[RenderNode]>>,
    layout_style: Rc<Style>,
  ) -> Self {
    let mut anonymous = Self::new(context, NodeOrigin::Anonymous, node, children);

    anonymous.layout_style_override = Some(layout_style);
    anonymous
  }

  pub(super) fn anonymous_text_item(parent_context: &RenderContext, text: String) -> Self {
    Self::text_item(RenderContext::for_anonymous(parent_context), text)
  }

  fn text_item(context: RenderContext, text: String) -> Self {
    let mut item = Self::anonymous(context, None, None, block_style());

    item.anonymous_text_content = Some(text);
    item.force_inline_layout = true;
    item
  }

  pub(super) fn anonymous_block_container(
    parent_context: &RenderContext,
    children: Vec<RenderNode>,
  ) -> Self {
    Self::anonymous(
      RenderContext::for_anonymous(parent_context),
      None,
      Some(children.into_boxed_slice()),
      block_style(),
    )
  }

  pub(super) fn anonymous_image_item(
    parent_context: &RenderContext,
    image: BackgroundImage,
  ) -> Self {
    // Cap image content to the parent pseudo's box so explicit `width` / `height`
    // on the pseudo wins over intrinsic / default sizing.
    let max_size = TaffySize {
      width: taffy::LengthPercentageAuto::percent(1.0),
      height: taffy::LengthPercentageAuto::percent(1.0),
    };

    match image {
      BackgroundImage::Url(url) => Self::anonymous(
        RenderContext::for_anonymous(parent_context),
        Some(Node::image(url)),
        None,
        Rc::new(Style {
          max_size,
          ..Style::default()
        }),
      ),
      gradient => {
        let mut context = RenderContext::for_anonymous(parent_context);

        context.style_mut().background_data_mut().background_image =
          Some(BackgroundImages::from([gradient]));
        Self::anonymous(
          context,
          Some(Node::container([])),
          None,
          // css-images-3 §5.1 default object size when the parent is auto.
          Rc::new(Style {
            size: TaffySize {
              width: taffy::Dimension::length(300.0),
              height: taffy::Dimension::length(150.0),
            },
            max_size,
            ..Style::default()
          }),
        )
      }
    }
  }

  /// An element's own text, moved into a child so generated content can precede it or its
  /// content can align.
  pub(super) fn generated_sibling_text(parent_context: &RenderContext, text: String) -> Self {
    let (style, sizing, current_color) =
      parent_context.resolve_pseudo_style(&MatchedDeclarationsView::default());
    let context = RenderContext::from_parent(parent_context, style, sizing, current_color);

    Self::text_item(context, text)
  }

  fn pseudo_content_child(
    originating_node: &Node,
    pseudo_context: &RenderContext,
    item: ContentItem,
  ) -> Option<Self> {
    let text = match item {
      ContentItem::Text(text) => text.as_ref().to_owned(),
      ContentItem::Attr(attr) => originating_node
        .attribute(&attr.name)
        .map(str::to_owned)
        .unwrap_or_else(|| attr.fallback.as_ref().to_owned()),
      ContentItem::Image(image) => {
        return Some(Self::anonymous_image_item(pseudo_context, *image));
      }
    };

    (!text.is_empty()).then(|| Self::anonymous_text_item(pseudo_context, text))
  }

  fn from_pseudo_match(
    parent_context: &RenderContext,
    originating_node: &Node,
    pseudo_matched: &MatchedDeclarationsView<'_>,
  ) -> Option<Self> {
    let (mut style, sizing, current_color) = parent_context.resolve_pseudo_style(pseudo_matched);

    if matches!(style.base_data.display, Display::None) {
      return None;
    }

    // flex/grid add no semantics over a flat content list; downgrade per spec §8.
    if style.base_data.display.should_blockify_children() {
      style.base_data_mut().display = Display::Block;
    }

    let items = match take(&mut style.misc1_data_mut().content) {
      ContentValue::Items(items) => items,
      _ => return None,
    };

    let pseudo_context = RenderContext::from_parent(parent_context, style, sizing, current_color);

    let children: Box<[Self]> = items
      .into_vec()
      .into_iter()
      .filter_map(|item| Self::pseudo_content_child(originating_node, &pseudo_context, item))
      .collect();

    if children.is_empty() {
      return None;
    }

    Some(Self::new(
      pseudo_context,
      NodeOrigin::Pseudo,
      Some(Node::container([])),
      Some(children),
    ))
  }

  /// Blink positions an outside marker against the item's first line box, so the
  /// marker goes on the box that establishes that line, however deep it sits. An
  /// inside marker is the item's own content and stays on the item.
  fn attach_marker(&mut self, marker: RenderNode) {
    if self.should_create_inline_layout() {
      self.marker = Some(Box::new(marker));
      return;
    }

    if marker.context.style.inherited_data.list_style_position == ListStylePosition::Outside
      && let Some(block) = self.marker_host_child()
    {
      block.attach_marker(marker);
      return;
    }

    let has_block_content = self.children.as_deref().is_some_and(|children| {
      children
        .iter()
        .any(|child| !child.participates_in_inline_formatting_context())
    });

    if !has_block_content {
      // Text of its own, or nothing at all: the marker shares that line.
      self.force_inline_layout = true;
      self.marker = Some(Box::new(marker));
      return;
    }

    // Block-level content the marker may not join, so it gets a line of its own.
    let mut line = RenderNode::anonymous_block_container(&self.context, Vec::new());
    line.force_inline_layout = true;
    line.marker = Some(Box::new(marker));

    let mut children = Vec::from(self.children.take().unwrap_or_default());
    children.insert(0, line);
    self.children = Some(children.into_boxed_slice());
  }

  /// The block box the marker travels into when this box has no line of its own.
  fn marker_host_child(&mut self) -> Option<&mut RenderNode> {
    let child = self
      .children
      .as_deref_mut()?
      .iter_mut()
      .find(|child| !child.is_out_of_flow())?;

    child.hosts_marker_line().then_some(child)
  }

  fn hosts_marker_line(&self) -> bool {
    self.context.style.base_data.display == Display::Block
      && self.context.style.base_data.float == Float::None
      && self.leads_to_a_line()
  }

  /// Whether this box, or the block chain below it, ends in a line the marker can share.
  fn leads_to_a_line(&self) -> bool {
    self.should_create_inline_layout()
      || self.children.is_none()
      || self
        .children
        .as_deref()
        .and_then(|children| children.iter().find(|child| !child.is_out_of_flow()))
        .is_some_and(RenderNode::hosts_marker_line)
  }

  fn is_anonymous_text_item(&self) -> bool {
    self.anonymous_text_content.is_some() && self.node.is_none()
  }

  fn is_whitespace_only_text_node(&self) -> bool {
    self
      .node
      .as_ref()
      .is_some_and(Node::is_whitespace_only_text)
  }

  // Only fully-collapsible whitespace may be dropped. preserve / preserve-spaces
  // keep their spaces, and preserve-breaks may hold a forced break, so a
  // whitespace-only node in any of those still renders.
  pub(super) fn is_collapsible_whitespace_only_text_node(&self) -> bool {
    self.context.style.inherited_data.white_space_collapse == WhiteSpaceCollapse::Collapse
      && self.is_whitespace_only_text_node()
  }

  /// Whether the node paints its own shadows, background, and border: not when
  /// `visibility: hidden`, nor for an anonymous box, which carries its parent's background only
  /// for the text it wraps to show through `background-clip: text`.
  pub fn paints_own_box(&self) -> bool {
    self.node.is_some() && self.context.style.is_visible()
  }

  /// True if any direct child is an anonymous text item.
  pub fn has_anonymous_text_item_child(&self) -> bool {
    self
      .children
      .as_ref()
      .is_some_and(|children| children.iter().any(RenderNode::is_anonymous_text_item))
  }

  /// The authored node's document-order position, absent for generated boxes.
  pub fn source_order(&self) -> Option<usize> {
    match self.origin {
      NodeOrigin::Authored { source_order } => Some(source_order),
      _ => None,
    }
  }

  /// Resolves the descendant at `path` (child indices from this node).
  pub fn node_at_path(&self, path: &[usize]) -> Option<&RenderNode> {
    let mut current = self;
    for &index in path {
      current = current.children.as_deref()?.get(index)?;
    }
    Some(current)
  }

  /// Mutable [`node_at_path`](Self::node_at_path).
  pub fn node_at_path_mut(&mut self, path: &[usize]) -> Option<&mut RenderNode> {
    let mut current = self;
    for &index in path {
      current = current.children.as_deref_mut()?.get_mut(index)?;
    }
    Some(current)
  }

  pub(crate) fn is_inline_level(&self) -> bool {
    self.context.style.base_data.display.is_inline_level()
  }

  pub(crate) fn is_inline_atomic_container(&self) -> bool {
    matches!(
      self.context.style.base_data.display,
      Display::InlineBlock | Display::InlineFlex | Display::InlineGrid | Display::WebkitInlineBox
    )
  }

  /// True if this node is laid out as an inline-level box (atomic inline or float).
  pub(crate) fn participates_as_inline_box(&self) -> bool {
    self.is_inline_atomic_container() || self.context.style.base_data.float != Float::None
  }

  fn participates_in_inflow_inline_formatting_context(&self) -> bool {
    self.is_inline_level()
      || self.is_inline_atomic_container()
      || self.anonymous_text_content.is_some()
  }

  pub(crate) fn participates_in_inline_formatting_context(&self) -> bool {
    self.participates_in_inflow_inline_formatting_context()
      || self.is_out_of_flow()
      || self.context.style.base_data.float != Float::None
  }

  pub(crate) fn is_out_of_flow(&self) -> bool {
    self.context.style.base_data.position.is_out_of_flow()
  }

  /// True if this node's children form an inline formatting context.
  pub fn should_create_inline_layout(&self) -> bool {
    self.force_inline_layout
      || (matches!(
        self.context.style.base_data.display,
        Display::Block
          | Display::FlowRoot
          | Display::InlineBlock
          | Display::ListItem
          | Display::TableCell
      ) && self.children.as_ref().is_some_and(|children| {
        children
          .iter()
          .any(RenderNode::participates_in_inflow_inline_formatting_context)
          && children
            .iter()
            .all(RenderNode::participates_in_inline_formatting_context)
      }))
  }

  /// Builds a render tree from a node under the given parent context.
  pub fn from_node(parent_context: &RenderContext, node: Node) -> Self {
    Self::from_node_within(parent_context, node, MAX_GRID_ROWS)
  }

  /// [`Self::from_node`], with every table lowered onto grids of at most `max_rows` rows.
  pub(super) fn from_node_within(
    parent_context: &RenderContext,
    node: Node,
    max_rows: usize,
  ) -> Self {
    let matched_styles = match_stylesheets_view(
      &node,
      parent_context.stylesheet(),
      parent_context.sizing.viewport,
    );
    let mut tree = Self::from_node_iterative(parent_context, node, &matched_styles);

    tree.lower_tables(max_rows);

    if tree.is_inline_level() {
      tree.context.blockify();
    }

    tree
  }

  fn from_node_iterative(
    parent_context: &RenderContext,
    root: Node,
    matched_declarations: &[NodeMatchedDeclarations<'_>],
  ) -> Self {
    let mut source_cursor = 0;
    let mut root_counter = ListCounter::new(&root);
    let mut stack = vec![PendingRenderNode::build(
      parent_context,
      root,
      matched_declarations,
      &mut source_cursor,
      &mut root_counter,
      false,
    )];

    loop {
      let Some(current) = stack.last_mut() else {
        return RenderNode::empty(parent_context);
      };

      if let Some(child) = current.pending_children.next() {
        let child_pending = PendingRenderNode::build(
          &current.context,
          child,
          matched_declarations,
          &mut source_cursor,
          &mut current.list_counter,
          current.inside_list,
        );
        stack.push(child_pending);
        continue;
      }

      let Some(finished) = stack.pop() else {
        return RenderNode::empty(parent_context);
      };

      if !finished.owns_list_counter
        && let Some(parent) = stack.last_mut()
      {
        parent.list_counter = finished.list_counter;
      }

      let render_node = finished.finish();

      if let Some(parent) = stack.last_mut() {
        parent.rendered_children.push(render_node);
      } else {
        return render_node;
      }
    }
  }

  fn empty(parent_context: &RenderContext) -> Self {
    Self::new(
      parent_context.clone(),
      NodeOrigin::Anonymous,
      Some(Node::container([])),
      None,
    )
  }

  /// Blockifies, trims, and wraps `children` the way their parent's display
  /// requires, blockifying the parent itself when it holds a block.
  fn normalize_children(
    context: &mut RenderContext,
    children: Box<[RenderNode]>,
  ) -> Box<[RenderNode]> {
    if context.style.base_data.display.should_blockify_children() {
      // CSS Flexbox L1 §4 / Grid L1 §6: collapsible whitespace-only text
      // between items is not rendered; every remaining child blockifies.
      let mut children = Vec::from(children);
      children.retain(|child| !child.is_collapsible_whitespace_only_text_node());
      for child in &mut children {
        child.context.blockify();
      }

      return children.into_boxed_slice();
    }

    let children = RenderNode::wrap_orphan_table_parts(context, children);
    // Blink's Text::TextLayoutObjectIsNeeded: collapsible
    // whitespace-only text renders only after an in-flow inline-level
    // sibling, and leading whitespace only inside an inline parent
    // (#711, #992).
    let children = drop_collapsible_boundary_whitespace(
      Vec::from(children),
      context.style.base_data.display.is_inline(),
    )
    .into_boxed_slice();

    // https://github.com/kane50613/takumi/issues/738: out-of-flow boxes
    // must not be swept into an anonymous block box.
    let has_inline = children
      .iter()
      .any(|child| child.participates_in_inline_formatting_context() && !child.is_out_of_flow());
    let has_block = children
      .iter()
      .any(|child| !child.participates_in_inline_formatting_context());
    let has_out_of_flow = has_inline
      && children
        .iter()
        .any(|child| child.is_out_of_flow() || child.holds_inline_out_of_flow());
    let parent_is_inline = context.style.base_data.display.is_inline();

    if parent_is_inline && has_block {
      context.blockify();
    }

    // A block parent whose inline content holds out-of-flow boxes wraps that
    // content, placeholders included, so the boxes lay out against a block.
    // An inline parent keeps its inline formatting context untouched — an
    // anonymous block there would be dropped by the surrounding line box.
    if !(has_inline && (has_block || (!parent_is_inline && has_out_of_flow))) {
      return children;
    }

    let mut final_children = Vec::new();
    let mut inline_group = Vec::new();

    // An out-of-flow box stays in the run of inline content around it, as a placeholder
    // marking its static position, rather than breaking the run into two lines.
    for item in children {
      if item.participates_in_inline_formatting_context() {
        inline_group.push(item);
        continue;
      }

      flush_inline_group(&mut inline_group, &mut final_children, context);
      final_children.push(item);
    }

    flush_inline_group(&mut inline_group, &mut final_children, context);
    final_children.into_boxed_slice()
  }

  /// Padding in pixels, with a percentage resolving to zero.
  pub(super) fn padding_px(&self) -> Rect<f32> {
    self.padding_px_against(0.0)
  }

  /// The padding, its percentages of `containing_block_width`.
  pub(super) fn padding_px_against(&self, containing_block_width: f32) -> Rect<f32> {
    let style = &self.context.style;

    Rect {
      top: style.box_data.padding_top,
      right: style.box_data.padding_right,
      bottom: style.box_data.padding_bottom,
      left: style.box_data.padding_left,
    }
    .map(|length| length.to_px(&self.context.sizing, containing_block_width))
  }

  /// The child-index path to the first node in preorder, `self` included, that `matches` accepts.
  pub fn path_where(&self, matches: impl Fn(&RenderNode) -> bool) -> Option<Vec<usize>> {
    let mut path = Vec::new();
    // Each open node with the index of the next child to visit, so a deep tree never recurses.
    let mut open = vec![(self, 0)];

    if matches(self) {
      return Some(path);
    }

    while let Some((node, next)) = open.last_mut() {
      let Some(child) = node
        .children
        .as_deref()
        .and_then(|children| children.get(*next))
      else {
        open.pop();
        path.pop();
        continue;
      };

      path.push(*next);
      *next += 1;

      if matches(child) {
        return Some(path);
      }
      open.push((child, 0));
    }

    None
  }

  /// Used border widths in pixels.
  pub(super) fn border_px(&self) -> Rect<f32> {
    let style = &self.context.style;

    Rect {
      top: style.box_data.border_top_width,
      right: style.box_data.border_right_width,
      bottom: style.box_data.border_bottom_width,
      left: style.box_data.border_left_width,
    }
    .map(|width| width.to_used_px(&self.context.sizing))
  }

  /// Margins in pixels, with a percentage resolving to zero.
  pub(super) fn margin_px(&self) -> Rect<f32> {
    let style = &self.context.style;

    Rect {
      top: style.box_data.margin_top,
      right: style.box_data.margin_right,
      bottom: style.box_data.margin_bottom,
      left: style.box_data.margin_left,
    }
    .map(|length| length.to_px(&self.context.sizing, 0.0))
  }

  fn inline_content_baseline_offset(
    &self,
    available_space: Size<AvailableSpace>,
    size: Size<f32>,
    use_last_line: bool,
  ) -> Option<f32> {
    let baseline =
      self.inline_content_border_box_baseline(available_space, size, use_last_line, None)?;

    Some(self.margin_px().top + baseline)
  }

  /// Baseline of the first or last line box, measured from the border box top.
  /// Reads the baseline from the fragment items it kept for `kept`, when it kept them, instead of
  /// laying its content out again.
  fn inline_content_border_box_baseline(
    &self,
    available_space: Size<AvailableSpace>,
    size: Size<f32>,
    use_last_line: bool,
    kept: Option<ComputedLayout>,
  ) -> Option<f32> {
    if matches!(
      self.node.as_ref().and_then(Node::inline_content),
      Some(InlineContentKind::Box)
    ) {
      return None;
    }

    if let Some(baseline) = kept
      .and_then(|layout| self.fragment_items_in(layout))
      .and_then(|items| items.baseline(use_last_line))
    {
      return Some(self.content_top() + baseline);
    }

    let baseline = self.inline_content_baseline(available_space, size.width, use_last_line)?;

    Some(self.content_top() + baseline)
  }

  /// The top border and padding the baselines of its inline content sit below.
  fn content_top(&self) -> f32 {
    let sizing = &self.context.sizing;
    let border_top = Length::from(self.context.style.box_data.border_top_width).to_px(sizing, 0.0);
    let padding_top = self.context.style.box_data.padding_top.to_px(sizing, 0.0);

    border_top + padding_top
  }

  fn layout_first_baseline_offset(&self, results: &LayoutResults) -> Option<f32> {
    let baseline = results.first_baseline_y(NodeId::ROOT).ok().flatten()?;

    Some(self.margin_px().top + baseline)
  }

  fn valid_baseline_offset(candidate: Option<f32>, box_height: f32) -> Option<f32> {
    candidate
      .filter(|baseline| baseline.is_finite() && *baseline >= 0.0 && *baseline <= box_height + 0.5)
  }

  /// Where an atomic inline box takes its baseline from, in order; an empty list, or no source
  /// that resolves, falls back to the bottom margin edge.
  fn inline_baseline_sources(&self) -> &'static [InlineBaselineSource] {
    match self.context.style.base_data.display {
      Display::InlineBlock if self.context.style.clips_overflow() => &[],
      Display::InlineBlock => &[
        InlineBaselineSource::InlineContentLastLine,
        InlineBaselineSource::LayoutFirstBaseline,
      ],
      Display::InlineFlex | Display::InlineGrid | Display::WebkitInlineBox => {
        &[InlineBaselineSource::LayoutFirstBaseline]
      }
      _ => &[],
    }
  }

  fn resolve_inline_baseline_source(
    &self,
    available_space: Size<AvailableSpace>,
    size: Size<f32>,
    source: InlineBaselineSource,
    results: &LayoutResults,
  ) -> Option<f32> {
    match source {
      InlineBaselineSource::InlineContentLastLine => {
        self.inline_content_baseline_offset(available_space, size, true)
      }
      InlineBaselineSource::LayoutFirstBaseline => self.layout_first_baseline_offset(results),
    }
  }

  fn resolve_inline_baseline_offset(
    &self,
    available_space: Size<AvailableSpace>,
    size: Size<f32>,
    results: &LayoutResults,
  ) -> Option<f32> {
    let margin = self.margin_px();
    let margin_box_height = size.height + margin.top + margin.bottom;

    self.inline_baseline_sources().iter().find_map(|&source| {
      let candidate = self.resolve_inline_baseline_source(available_space, size, source, results);

      Self::valid_baseline_offset(candidate, margin_box_height)
    })
  }

  pub(crate) fn measure_inline_box(
    &self,
    available_space: Size<AvailableSpace>,
  ) -> AtomicInlineMetrics {
    if self.participates_as_inline_box() {
      return self.measure_atomic_subtree(available_space);
    }

    // Only an atomic box has a baseline of its own; a replaced one sits on its bottom margin edge.
    AtomicInlineMetrics {
      size: self.node.as_ref().map_or(Size::ZERO, |node| {
        node.measure(
          &self.context,
          available_space,
          Size::NONE,
          &self.layout_style(&self.context.sizing),
        )
      }),
      baseline_offset: None,
    }
  }

  /// Blink's `LayoutAtomicInline` sizing: an auto width shrinks to fit, and any other resolves
  /// against the containing block, which `available_space` stands in for.
  pub(crate) fn layout_atomic_inline(
    &self,
    available_space: Size<AvailableSpace>,
  ) -> AtomicInlineMetrics {
    if !self.participates_as_inline_box()
      || self.layout_style(&self.context.sizing).size.width.is_auto()
    {
      return self.measure_inline_box(available_space);
    }

    let results = LayoutResults::compute(self, available_space);
    let size = results.root_size();

    AtomicInlineMetrics {
      size,
      baseline_offset: self.resolve_inline_baseline_offset(available_space, size, &results),
    }
  }

  /// An atomic inline box's shrink-to-fit size and baseline.
  fn measure_atomic_subtree(&self, available_space: Size<AvailableSpace>) -> AtomicInlineMetrics {
    let at_width = |width| Size {
      width,
      height: available_space.height,
    };

    // CSS shrink-to-fit for inline-level atomic boxes:
    // width = min(max-content, max(min-content, available)).
    // Reference: https://www.w3.org/TR/CSS22/visudet.html#float-width
    let (min_content, max_content) = LayoutTree::from_render_node(self).root_min_max_widths();
    let used_width = match available_space.width {
      AvailableSpace::Definite(available) => max_content.min(min_content.max(available)),
      AvailableSpace::MinContent => min_content,
      AvailableSpace::MaxContent => max_content,
    };
    let results = LayoutResults::compute(self, at_width(AvailableSpace::Definite(used_width)));
    let size = results.root_size();

    AtomicInlineMetrics {
      size,
      baseline_offset: self.resolve_inline_baseline_offset(available_space, size, &results),
    }
  }

  /// The size of this inline formatting context, and whether it sized inline boxes, the only
  /// part of it the available height reaches.
  pub(crate) fn measure_inline_content(
    &self,
    available_space: Size<AvailableSpace>,
    known_dimensions: Size<Option<f32>>,
  ) -> (Size<f32>, bool) {
    (
      InlineNode::new(self)
        .measure(available_space, known_dimensions)
        .size,
      true,
    )
  }

  pub(crate) fn measure(
    &self,
    available_space: Size<AvailableSpace>,
    known_dimensions: Size<Option<f32>>,
    style: &Style,
  ) -> Size<f32> {
    let Some(node) = &self.node else {
      return Size::ZERO;
    };

    // An inline box that tree building left among block-level siblings, such as in a table row
    // no table wraps, lays out as the anonymous block CSS 2.2 §9.2.1.1 would wrap it in.
    if matches!(node.kind, NodeKind::Text(_))
      || self.context.style.base_data.display == Display::Inline
    {
      return InlineNode::new(self)
        .measure(available_space, known_dimensions)
        .size;
    }

    node.measure(&self.context, available_space, known_dimensions, style)
  }
}

fn flush_inline_group(
  inline_group: &mut Vec<RenderNode>,
  final_children: &mut Vec<RenderNode>,
  parent_render_context: &RenderContext,
) {
  if inline_group.is_empty() {
    return;
  }

  final_children.push(RenderNode::anonymous_block_container(
    parent_render_context,
    take(inline_group),
  ));
}

// Mirrors Blink's Text::TextLayoutObjectIsNeeded, minus the ends-with-space
// refinement (inline collapsing already merges adjacent spaces).
fn drop_collapsible_boundary_whitespace(
  input: Vec<RenderNode>,
  parent_is_inline: bool,
) -> Vec<RenderNode> {
  let mut out = Vec::with_capacity(input.len());
  let mut after_in_flow_inline = parent_is_inline;

  for child in input {
    if child.is_collapsible_whitespace_only_text_node() && !after_in_flow_inline {
      continue;
    }

    if !child.is_out_of_flow() && child.context.style.base_data.float == Float::None {
      after_in_flow_inline = child.participates_in_inflow_inline_formatting_context();
    }

    out.push(child);
  }

  out
}

/// A node whose context is resolved while its children are still being built.
struct PendingRenderNode {
  context: RenderContext,
  node: Node,
  source_order: usize,
  children_is_some: bool,
  pending_children: IntoIter<Node>,
  rendered_children: Vec<RenderNode>,
  pseudo_after: Option<RenderNode>,
  list_counter: ListCounter,
  marker_ordinal: Option<i32>,
  inside_list: bool,
  owns_list_counter: bool,
}

impl PendingRenderNode {
  fn build(
    parent_context: &RenderContext,
    mut node: Node,
    matched_declarations: &[NodeMatchedDeclarations<'_>],
    source_cursor: &mut usize,
    counter: &mut ListCounter,
    inside_list: bool,
  ) -> Self {
    let source_order = *source_cursor;
    *source_cursor += 1;
    let (style, sizing, current_color) =
      parent_context.resolve_child_style(&mut node, source_order, matched_declarations);
    let children = node.take_children();
    let children_is_some = children.is_some();
    let children = children.map_or_else(Vec::new, <[Node]>::into_vec);
    let context = RenderContext::from_parent(parent_context, style, sizing, current_color);

    let element_matched = matched_declarations.get(source_order);
    let marker_ordinal =
      (context.style.base_data.display == Display::ListItem).then(|| counter.take(&node));
    let pseudo_before = element_matched
      .and_then(|m| m.before())
      .and_then(|m| RenderNode::from_pseudo_match(&context, &node, m));
    let pseudo_after = element_matched
      .and_then(|m| m.after())
      .and_then(|m| RenderNode::from_pseudo_match(&context, &node, m));
    let pseudo_before_present = pseudo_before.is_some();

    let has_generated_children =
      marker_ordinal.is_some() || pseudo_before.is_some() || pseudo_after.is_some();
    let mut rendered_children = Vec::with_capacity(children.len() + 3);
    rendered_children.extend(pseudo_before);

    // The inline collector emits an element's own text before any child, so
    // an element that folded its text has to hand it back as a child for the
    // generated content to come first.
    if pseudo_before_present && let Some(text) = node.take_text() {
      rendered_children.push(RenderNode::generated_sibling_text(&context, text));
    }

    let owns_list_counter = owns_list_counter(&node, inside_list);

    Self {
      source_order,
      children_is_some: children_is_some || has_generated_children,
      list_counter: if owns_list_counter {
        ListCounter::new(&node)
      } else {
        *counter
      },
      inside_list: inside_list || is_list_element(&node),
      owns_list_counter,
      context,
      node,
      rendered_children,
      pending_children: children.into_iter(),
      pseudo_after,
      marker_ordinal,
    }
  }

  /// Closes the node once every child is rendered.
  fn finish(mut self) -> RenderNode {
    if let Some(after) = self.pseudo_after.take() {
      self.rendered_children.push(after);
    }

    let mut context = self.context;
    let children = if self.children_is_some {
      Some(RenderNode::normalize_children(
        &mut context,
        self.rendered_children.into_boxed_slice(),
      ))
    } else {
      Self::anonymous_text_child(&context, &self.node).map(|child| Box::from([child]))
    };
    let mut render_node = RenderNode::new(
      context,
      NodeOrigin::Authored {
        source_order: self.source_order,
      },
      Some(self.node),
      children,
    );

    if let Some(ordinal) = self.marker_ordinal
      && let Some(marker) = list_marker(&render_node.context, ordinal)
    {
      render_node.attach_marker(marker);
    }

    render_node
  }

  /// The text a childless flex or grid item carries, wrapped as its own child.
  fn anonymous_text_child(context: &RenderContext, node: &Node) -> Option<RenderNode> {
    if !context.style.base_data.display.should_blockify_children() {
      return None;
    }

    let text = node.inline_content().and_then(|content| match content {
      InlineContentKind::Text(text) => Some(text.into_owned()),
      InlineContentKind::Box => None,
    })?;

    Some(RenderNode::anonymous_text_item(context, text))
  }
}

impl RenderContext {
  /// The used `line-height: normal` for `style` at `font_size`, or zero for any other
  /// `line-height`.
  pub(crate) fn resolve_normal_line_height(&self, style: &ComputedStyle, font_size: f32) -> f32 {
    if !matches!(style.inherited_data.line_height, LineHeight::Normal) {
      return 0.0;
    }

    self
      .primary_font_metrics(style, font_size)
      .map_or(font_size, PrimaryFontMetrics::line_spacing)
  }

  /// The metrics of `style`'s primary font at `font_size`.
  pub(crate) fn primary_font_metrics(
    &self,
    style: &ComputedStyle,
    font_size: f32,
  ) -> Option<PrimaryFontMetrics> {
    let description = style.font_selection_description(&self.sizing);
    let request = description.font_selection_request;
    let font_family = self.expand_font_family(&style.inherited_data.font_family);
    let mut hasher = Xxh3::new();

    font_family.hash_tokens(&mut hasher);

    for value in [request.weight, request.width, request.slope] {
      hasher.write_u32(value.to_f32().to_bits());
    }

    hasher.write_u8(u8::from(description.is_italic_keyword));
    hasher.write_u32(font_size.to_bits());

    self.cached_primary_font_metrics(hasher.finish(), || {
      self.first_font_metrics(font_family.tokens(), &description, font_size)
    })
  }

  /// A child's cascaded style and its element-owned important declarations.
  fn cascade(
    &self,
    node_layers: NodeStyleLayers,
    matched_declarations: &MatchedDeclarationsView<'_>,
  ) -> (NodeStyle, ElementImportant) {
    let mut style = NodeStyle::default();

    // `tw` is the last declared layer, below unlayered author rules, so its
    // important half goes last: the cascade reverses layer order for important
    // declarations.
    let tw = node_layers.author_tw.map(|author_tw| {
      author_tw.declaration_blocks(
        self.sizing.viewport,
        &self.stylesheet().breakpoints,
        self.tw_cache(),
      )
    });

    if let Some(preset) = node_layers.preset {
      style.append_block(preset.declarations);
    }

    if let Some(dir) = node_layers.dir {
      style.push(StyleDeclaration::direction(dir), false);
    }

    for &declarations in matched_declarations.layered_normal() {
      style.merge_matched_block(declarations);
    }

    // `tw` is the last declared layer, as Tailwind orders utilities: above every
    // named `@layer`, below unlayered author rules.
    if let Some(tw) = &tw {
      style.append_block_cloned(&tw.normal);
    }

    for &declarations in matched_declarations.unlayered_normal() {
      style.merge_matched_block(declarations);
    }

    // An element's own declarations outrank selector-based ones at the same
    // importance.
    let (inline_normal, inline_important) = node_layers
      .inline
      .map(|inline| StyleDeclarationBlock::from(inline).split_importance())
      .unzip();

    if let Some(inline_normal) = inline_normal {
      style.append_block(inline_normal);
    }

    // Important declarations reverse layer order, so `tw`, the last declared
    // layer, sits above unlayered rules and below every named `@layer`.
    for &declarations in matched_declarations.unlayered_important() {
      style.merge_matched_block(declarations);
    }

    if let Some(tw) = &tw {
      style.append_block_cloned(&tw.important);
    }

    for &declarations in matched_declarations.layered_important() {
      style.merge_matched_block(declarations);
    }

    if let Some(inline_important) = &inline_important {
      style.append_block(inline_important.clone());
    }

    (
      style,
      ElementImportant {
        tw,
        inline: inline_important,
      },
    )
  }

  /// This style as a child inherits it, with the stylesheet's registered custom properties.
  fn inherited_style(&self) -> Cow<'_, ComputedStyle> {
    registered_custom_property_parent_style(
      &self.style,
      slice::from_ref(self.stylesheet().as_ref()),
      self.sizing.viewport,
    )
  }

  /// Resolves a generated box's style and sizing from its matched declarations.
  pub(super) fn resolve_pseudo_style(
    &self,
    pseudo_matched: &MatchedDeclarationsView<'_>,
  ) -> (ComputedStyle, SizingContext, Color) {
    let (style_layers, _) = self.cascade(NodeStyleLayers::default(), pseudo_matched);
    let mut style = style_layers.inherit(&self.inherited_style());
    let sizing = self.child_sizing(&style, &self.sizing, false);
    let current_color = style.inherited_data.color.resolve(self.current_color);

    style.make_computed(&sizing);
    (style, sizing, current_color)
  }

  /// Resolves a child's style and sizing from its matched declarations.
  fn resolve_child_style(
    &self,
    node: &mut Node,
    source_order: usize,
    matched_declarations: &[NodeMatchedDeclarations<'_>],
  ) -> (ComputedStyle, SizingContext, Color) {
    let default_matched = MatchedDeclarationsView::default();
    let matched = matched_declarations
      .get(source_order)
      .map(NodeMatchedDeclarations::element)
      .unwrap_or(&default_matched);
    let layers = node.take_style_layers();
    let lang = layers.lang;

    let (style_layers, element_important) = self.cascade(layers, matched);
    let inherited_parent = self.inherited_style();

    let mut style = style_layers.inherit_with_lang(&inherited_parent, lang);

    // A tree built in code is content, not a document: `rem` resolves against
    // the viewport, so a `font-size` on the outermost node styles text without
    // rescaling every `rem` length below it. A parsed document is the
    // exception, since its outermost node is the `<html>` element that CSS
    // does make the `rem` basis.
    let is_document_root = source_order == 0
      && node
        .tag_name()
        .is_some_and(|tag| tag.eq_ignore_ascii_case("html"));

    let mut child_sizing_for_final: Option<SizingContext> = None;
    if !style.misc1_data.animation_name.is_empty() {
      let (animated, child_sizing) = self.animated_style(style);

      style = animated;
      child_sizing_for_final = Some(child_sizing);
    }

    // Important declarations outrank an animation, and `inherit` among them
    // needs the parent the first pass resolved against.
    let important = matched
      .unlayered_important()
      .iter()
      .map(|declarations| declarations.iter())
      .chain(
        element_important
          .tw
          .iter()
          .map(|blocks| blocks.important.iter()),
      )
      .chain(
        matched
          .layered_important()
          .iter()
          .map(|declarations| declarations.iter()),
      )
      .chain(
        element_important
          .inline
          .iter()
          .map(|declarations| declarations.iter()),
      );

    for declarations in important {
      for declaration in declarations {
        declaration
          .clone()
          .apply_with_parent(&mut style, &inherited_parent);
      }
    }

    // Blink's `LayoutBR` breaks the line whatever `white-space` says.
    if node
      .tag_name()
      .is_some_and(|tag| tag.eq_ignore_ascii_case("br"))
    {
      style.inherited_data_mut().white_space_collapse = WhiteSpaceCollapse::PreserveBreaks;
    }

    let sizing = self.child_sizing(
      &style,
      child_sizing_for_final.as_ref().unwrap_or(&self.sizing),
      is_document_root,
    );
    let current_color = style.inherited_data.color.resolve(self.current_color);
    style.make_computed(&sizing);
    (style, sizing, current_color)
  }

  /// The sizing a child with `style` resolves against. Its font size resolves in
  /// `font_size_basis`, and a document root sets the root font metrics itself.
  fn child_sizing(
    &self,
    style: &ComputedStyle,
    font_size_basis: &SizingContext,
    is_document_root: bool,
  ) -> SizingContext {
    let font_size = style
      .inherited_data
      .font_size
      .to_px(font_size_basis, font_size_basis.font_size);
    let normal_basis = self.resolve_normal_line_height(style, font_size);
    let line_height = style
      .inherited_data
      .line_height
      .to_px(&self.sizing, normal_basis);

    self.sizing.with_font_metrics(
      font_size,
      self
        .sizing
        .root_font_size
        .or_else(|| is_document_root.then_some(font_size)),
      line_height,
      self
        .sizing
        .root_line_height
        .or_else(|| is_document_root.then_some(line_height)),
    )
  }

  /// Samples the stylesheet animations on `style` at this render's time,
  /// against the sizing the pre-animation style produces.
  fn animated_style(&self, style: ComputedStyle) -> (ComputedStyle, SizingContext) {
    let child_sizing = self.child_sizing(&style, &self.sizing, false);
    let child_current_color = style.inherited_data.color.resolve(self.current_color);
    let child_context = RenderContext::from_parent(
      self,
      style.clone(),
      child_sizing.clone(),
      child_current_color,
    );
    let animated = apply_stylesheet_animations(
      style,
      child_context.stylesheet(),
      child_context.time_ms(),
      &child_context.sizing,
      child_context.current_color,
    );

    (animated, child_sizing)
  }
}

#[cfg(test)]
mod tests {
  use std::str::FromStr;

  use taffy::NodeId as TaffyNodeId;

  use super::{
    NodeOrigin, RenderNode, registered_custom_property_parent_style, sort_children_by_order,
  };
  use crate::{
    context::RenderContext,
    resources::font::Fonts,
    style::{
      ComputedStyle, Length, PropertyRule, SizingContext, Style, StyleDeclaration,
      StyleDeclarationBlock, StyleSheet,
    },
    viewport::Viewport,
  };

  fn parse_stylesheet(css: &str) -> StyleSheet {
    let result = StyleSheet::parse(css);
    assert!(result.is_ok(), "expected stylesheet to parse: {result:?}");
    result.unwrap_or_default()
  }

  #[test]
  fn render_node_drop_is_iterative() {
    let context = RenderContext::builder()
      .fonts(Fonts::default().snapshot())
      .sizing(
        SizingContext::builder()
          .viewport(Viewport::default())
          .build(),
      )
      .build();
    let leaf = |children: Option<Box<[RenderNode]>>| {
      RenderNode::new(context.clone(), NodeOrigin::Anonymous, None, children)
    };

    let mut root = leaf(None);
    for _ in 0..500_000 {
      root = leaf(Some(Box::new([root])));
    }

    drop(root);
  }

  #[test]
  fn anonymous_box_has_no_used_border_width() {
    let sizing = SizingContext::builder()
      .viewport(Viewport::default())
      .build();
    let parent = RenderContext::builder()
      .fonts(Fonts::default().snapshot())
      .sizing(sizing)
      .build();

    let anonymous = RenderContext::for_anonymous(&parent);
    let style = &anonymous.style;
    let sizing = &anonymous.sizing;

    // `border-style` is `none`, so the initial `medium` width has a used value
    // of zero: an anonymous box reports no border it cannot render.
    assert_eq!(style.box_data.border_top_width.to_used_px(sizing), 0.0);
    assert_eq!(style.box_data.border_right_width.to_used_px(sizing), 0.0);
    assert_eq!(style.box_data.border_bottom_width.to_used_px(sizing), 0.0);
    assert_eq!(style.box_data.border_left_width.to_used_px(sizing), 0.0);
  }

  #[test]
  fn sort_children_by_order_keeps_source_order_for_equal_values() {
    let mut children = vec![
      TaffyNodeId::from(3usize),
      TaffyNodeId::from(1usize),
      TaffyNodeId::from(2usize),
    ];
    sort_children_by_order(&mut children, |child_id| match usize::from(child_id) {
      1 => -1,
      _ => 0,
    });
    assert_eq!(
      children,
      vec![
        TaffyNodeId::from(1usize),
        TaffyNodeId::from(3usize),
        TaffyNodeId::from(2usize)
      ]
    );
  }

  #[test]
  fn registered_custom_property_can_disable_inheritance() {
    let mut parent = ComputedStyle::default();
    parent
      .custom_properties
      .set("--box-size".to_owned(), "50px".to_owned());

    let stylesheets = [StyleSheet::from(vec![PropertyRule {
      name: "--box-size".to_owned(),
      syntax: "*".to_owned(),
      inherits: false,
      initial_value: Some("10px".to_owned()),
      media_queries: Vec::new(),
    }])];

    let adjusted_parent =
      registered_custom_property_parent_style(&parent, &stylesheets, Viewport::default());
    assert_eq!(
      adjusted_parent.custom_properties.get("--box-size"),
      Some("10px")
    );
  }

  /// An `@property` registration is what decides whether a name inherits, even
  /// for the `--tw-*` names the utility engine also writes.
  #[test]
  fn a_registered_tw_property_reaches_the_child() {
    let parent = ComputedStyle::default();
    let stylesheets = [StyleSheet::from(vec![PropertyRule {
      name: "--tw-gradient-from-position".to_owned(),
      syntax: "<length-percentage>".to_owned(),
      inherits: false,
      initial_value: Some("0%".to_owned()),
      media_queries: Vec::new(),
    }])];

    let adjusted_parent =
      registered_custom_property_parent_style(&parent, &stylesheets, Viewport::default());
    let child = ComputedStyle::from_parent(&adjusted_parent);

    assert_eq!(
      child.custom_properties.get("--tw-gradient-from-position"),
      Some("0%")
    );
  }

  #[test]
  fn registered_custom_property_preserves_parent_value_when_inheriting() {
    let mut parent = ComputedStyle::default();
    parent
      .custom_properties
      .set("--box-size".to_owned(), "50px".to_owned());

    let stylesheets = [StyleSheet::from(vec![PropertyRule {
      name: "--box-size".to_owned(),
      syntax: "*".to_owned(),
      inherits: true,
      initial_value: Some("10px".to_owned()),
      media_queries: Vec::new(),
    }])];

    let adjusted_parent =
      registered_custom_property_parent_style(&parent, &stylesheets, Viewport::default());
    assert_eq!(
      adjusted_parent.custom_properties.get("--box-size"),
      Some("50px")
    );
  }

  #[test]
  fn registered_custom_property_uses_initial_value_when_missing_and_inheriting() {
    let parent = ComputedStyle::default();

    let stylesheets = [StyleSheet::from(vec![PropertyRule {
      name: "--box-size".to_owned(),
      syntax: "*".to_owned(),
      inherits: true,
      initial_value: Some("10px".to_owned()),
      media_queries: Vec::new(),
    }])];

    let adjusted_parent =
      registered_custom_property_parent_style(&parent, &stylesheets, Viewport::default());
    assert_eq!(
      adjusted_parent.custom_properties.get("--box-size"),
      Some("10px")
    );
  }

  #[test]
  fn registered_custom_property_uses_last_inherited_initial_value_when_parent_is_missing() {
    let parent = ComputedStyle::default();

    let stylesheets = [StyleSheet::from(vec![
      PropertyRule {
        name: "--box-size".to_owned(),
        syntax: "*".to_owned(),
        inherits: true,
        initial_value: Some("10px".to_owned()),
        media_queries: Vec::new(),
      },
      PropertyRule {
        name: "--box-size".to_owned(),
        syntax: "*".to_owned(),
        inherits: true,
        initial_value: Some("20px".to_owned()),
        media_queries: Vec::new(),
      },
    ])];

    let adjusted_parent =
      registered_custom_property_parent_style(&parent, &stylesheets, Viewport::default());
    assert_eq!(
      adjusted_parent.custom_properties.get("--box-size"),
      Some("20px")
    );
  }

  #[test]
  fn registered_custom_property_later_inheriting_rule_restores_parent_value() {
    let mut parent = ComputedStyle::default();
    parent
      .custom_properties
      .set("--box-size".to_owned(), "50px".to_owned());

    let stylesheets = [StyleSheet::from(vec![
      PropertyRule {
        name: "--box-size".to_owned(),
        syntax: "*".to_owned(),
        inherits: false,
        initial_value: Some("10px".to_owned()),
        media_queries: Vec::new(),
      },
      PropertyRule {
        name: "--box-size".to_owned(),
        syntax: "*".to_owned(),
        inherits: true,
        initial_value: Some("20px".to_owned()),
        media_queries: Vec::new(),
      },
    ])];

    let adjusted_parent =
      registered_custom_property_parent_style(&parent, &stylesheets, Viewport::default());
    assert_eq!(
      adjusted_parent.custom_properties.get("--box-size"),
      Some("50px")
    );
  }

  #[test]
  fn registered_custom_property_later_inheriting_rule_clears_prior_synthesized_value_without_initial_value()
   {
    let parent = ComputedStyle::default();

    let stylesheets = [StyleSheet::from(vec![
      PropertyRule {
        name: "--box-size".to_owned(),
        syntax: "*".to_owned(),
        inherits: false,
        initial_value: Some("10px".to_owned()),
        media_queries: Vec::new(),
      },
      PropertyRule {
        name: "--box-size".to_owned(),
        syntax: "*".to_owned(),
        inherits: true,
        initial_value: None,
        media_queries: Vec::new(),
      },
    ])];

    let adjusted_parent =
      registered_custom_property_parent_style(&parent, &stylesheets, Viewport::default());
    assert_eq!(adjusted_parent.custom_properties.get("--box-size"), None);
  }

  #[test]
  fn registered_custom_property_accepts_assignment_without_syntax_validation() {
    let parent = ComputedStyle::default();
    let stylesheet = parse_stylesheet(
      r#"
        @property --box-size {
          syntax: "<length>";
          inherits: false;
          initial-value: 10px;
        }
      "#,
    );
    let adjusted_parent =
      registered_custom_property_parent_style(&parent, &[stylesheet], Viewport::default());
    let style = Style::default().with(StyleDeclaration::CustomProperty(
      "--box-size".to_owned(),
      "red".to_owned(),
    ));

    let resolved = style.inherit(&adjusted_parent);
    assert_eq!(resolved.custom_properties.get("--box-size"), Some("red"));
  }

  #[test]
  fn registered_custom_property_accepts_valid_length_assignment() {
    let parent = ComputedStyle::default();
    let stylesheet = parse_stylesheet(
      r#"
        @property --box-size {
          syntax: "<length>";
          inherits: false;
          initial-value: 10px;
        }
      "#,
    );
    let adjusted_parent =
      registered_custom_property_parent_style(&parent, &[stylesheet], Viewport::default());
    let style = Style::default().with(StyleDeclaration::CustomProperty(
      "--box-size".to_owned(),
      "24px".to_owned(),
    ));

    let resolved = style.inherit(&adjusted_parent);
    assert_eq!(resolved.custom_properties.get("--box-size"), Some("24px"));
  }

  #[test]
  fn registered_custom_property_keeps_var_assignment_without_validation() {
    let parent = ComputedStyle::default();
    let stylesheet = parse_stylesheet(
      r#"
        @property --box-size {
          syntax: "<length>";
          inherits: false;
          initial-value: 10px;
        }
      "#,
    );
    let adjusted_parent =
      registered_custom_property_parent_style(&parent, &[stylesheet], Viewport::default());
    let style = Style::default()
      .with(StyleDeclaration::CustomProperty(
        "--source".to_owned(),
        "18px".to_owned(),
      ))
      .with(StyleDeclaration::CustomProperty(
        "--box-size".to_owned(),
        "var(--source)".to_owned(),
      ));

    let resolved = style.inherit(&adjusted_parent);
    assert_eq!(
      resolved.custom_properties.get("--box-size"),
      Some("var(--source)")
    );
  }

  #[test]
  fn registered_custom_property_still_accepts_keyword_assignment() {
    let parent = ComputedStyle::default();
    let stylesheet = parse_stylesheet(
      r#"
        @property --display-state {
          syntax: "none | auto";
          inherits: false;
          initial-value: none;
        }
      "#,
    );
    let adjusted_parent =
      registered_custom_property_parent_style(&parent, &[stylesheet], Viewport::default());
    let style = Style::default().with(StyleDeclaration::CustomProperty(
      "--display-state".to_owned(),
      "auto".to_owned(),
    ));

    let resolved = style.inherit(&adjusted_parent);
    assert_eq!(
      resolved.custom_properties.get("--display-state"),
      Some("auto")
    );
  }

  #[test]
  fn registered_custom_property_still_accepts_alternative_assignment() {
    let parent = ComputedStyle::default();
    let stylesheet = parse_stylesheet(
      r#"
        @property --accent {
          syntax: "<length> | <color>";
          inherits: false;
          initial-value: red;
        }
      "#,
    );
    let adjusted_parent =
      registered_custom_property_parent_style(&parent, &[stylesheet], Viewport::default());
    let style = Style::default().with(StyleDeclaration::CustomProperty(
      "--accent".to_owned(),
      "12px".to_owned(),
    ));

    let resolved = style.inherit(&adjusted_parent);
    assert_eq!(resolved.custom_properties.get("--accent"), Some("12px"));
  }

  #[test]
  fn registered_custom_property_still_accepts_supported_assignments() {
    let parent = ComputedStyle::default();
    let stylesheet = parse_stylesheet(
      r#"
        @property --fade-duration {
          syntax: "<time>";
          inherits: false;
          initial-value: 150ms;
        }
        @property --move {
          syntax: "<transform-function>";
          inherits: false;
          initial-value: translate(10px, 20px);
        }
        @property --bg {
          syntax: "<image>";
          inherits: false;
          initial-value: linear-gradient(red, blue);
        }
      "#,
    );
    let adjusted_parent =
      registered_custom_property_parent_style(&parent, &[stylesheet], Viewport::default());
    let style = Style::default()
      .with(StyleDeclaration::CustomProperty(
        "--fade-duration".to_owned(),
        "2s".to_owned(),
      ))
      .with(StyleDeclaration::CustomProperty(
        "--move".to_owned(),
        "rotate(45deg)".to_owned(),
      ))
      .with(StyleDeclaration::CustomProperty(
        "--bg".to_owned(),
        "url(hero.png)".to_owned(),
      ));

    let resolved = style.inherit(&adjusted_parent);
    assert_eq!(
      resolved.custom_properties.get("--fade-duration"),
      Some("2s")
    );
    assert_eq!(
      resolved.custom_properties.get("--move"),
      Some("rotate(45deg)")
    );
    assert_eq!(
      resolved.custom_properties.get("--bg"),
      Some("url(hero.png)")
    );
  }

  #[test]
  fn registered_custom_property_initial_value_applies_through_var_resolution() {
    let parent = ComputedStyle::default();
    let stylesheet = parse_stylesheet(
      r#"
        @property --box-size {
          syntax: "<length>";
          inherits: false;
          initial-value: 10px;
        }
      "#,
    );
    let adjusted_parent =
      registered_custom_property_parent_style(&parent, &[stylesheet], Viewport::default());
    let declarations = StyleDeclarationBlock::from_str("width: var(--box-size)");
    assert!(
      declarations.is_ok(),
      "width declaration using registered custom property should parse: {declarations:?}"
    );
    let Ok(declarations) = declarations else {
      return;
    };

    let mut style = Style::default();
    style.append_block(declarations);

    let resolved = style.inherit(&adjusted_parent);
    assert_eq!(resolved.box_data.width, Length::Px(10.0).into());
  }

  #[test]
  fn registered_custom_property_accepts_invalid_transform_assignment_without_validation() {
    let parent = ComputedStyle::default();
    let stylesheet = parse_stylesheet(
      r#"
        @property --move {
          syntax: "<transform-function>";
          inherits: false;
          initial-value: translate(10px, 20px);
        }
      "#,
    );
    let adjusted_parent =
      registered_custom_property_parent_style(&parent, &[stylesheet], Viewport::default());
    let style = Style::default().with(StyleDeclaration::CustomProperty(
      "--move".to_owned(),
      "red".to_owned(),
    ));

    let resolved = style.inherit(&adjusted_parent);
    assert_eq!(resolved.custom_properties.get("--move"), Some("red"));
  }

  #[test]
  fn lang_pseudo_class_matches_the_nearest_ancestor_or_self_lang_attribute() {
    use std::sync::Arc;

    use crate::{
      context::RenderContext,
      layout::{node::Node, tree::RenderNode},
      resources::font::Fonts,
      style::{Lang, SizingContext},
    };

    let stylesheet = StyleSheet::parse(
      r#"
        :lang(zh-Hant) { width: 10px; }
        :lang(ja) { width: 20px; }
      "#,
    )
    .expect("stylesheet parses");

    let fonts = Fonts::default();
    let context = RenderContext::builder()
      .fonts(fonts.snapshot())
      .sizing(
        SizingContext::builder()
          .viewport(Viewport::default())
          .build(),
      )
      .stylesheet(Arc::new(stylesheet))
      .build();

    let tree = RenderNode::from_node(
      &context,
      Node::container([
        // No `lang` of its own — `:lang(zh-Hant)` must walk up to the root to match.
        Node::container([Node::text("inherits")]),
        Node::container([Node::text("overrides")]).with_lang(Lang::parse("ja").unwrap()),
      ])
      .with_lang(Lang::parse("zh-Hant").unwrap()),
    );

    assert_eq!(tree.context.style.box_data.width, Length::Px(10.0).into());

    let children = tree.children.as_deref().expect("block children");
    assert_eq!(
      children[0].context.style.box_data.width,
      Length::Px(10.0).into()
    );
    assert_eq!(
      children[1].context.style.box_data.width,
      Length::Px(20.0).into()
    );
  }

  #[test]
  fn rem_follows_the_document_root_only_when_the_tree_is_a_document() {
    use std::sync::Arc;

    use crate::{
      context::RenderContext,
      layout::{node::Node, tree::RenderNode},
      resources::font::Fonts,
      style::{SizingContext, StyleSheet},
      viewport::Viewport,
    };

    fn child_width(root: Node) -> f32 {
      let stylesheet = StyleSheet::parse("#root { font-size: 32px } #child { width: 1rem }")
        .expect("stylesheet parses");
      let fonts = Fonts::default();
      let context = RenderContext::builder()
        .fonts(fonts.snapshot())
        .sizing(
          SizingContext::builder()
            .viewport(Viewport::default())
            .build(),
        )
        .stylesheet(Arc::new(stylesheet))
        .build();

      let tree = RenderNode::from_node(&context, root);
      let children = tree.children.as_deref().expect("children");

      let child = &children[0];

      child
        .context
        .style
        .box_data
        .width
        .as_length()
        .expect("length width")
        .to_px(&child.context.sizing, 0.0)
    }

    let content = Node::container([Node::container([]).with_id("child")]).with_id("root");
    assert_eq!(child_width(content), 16.0);

    let document = Node::container([Node::container([]).with_id("child")])
      .with_id("root")
      .with_tag_name("html");
    assert_eq!(child_width(document), 32.0);
  }
}
