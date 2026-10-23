#[cfg(feature = "fragmentation")]
use std::ops::Range;
use std::{
  borrow::Cow, cell::RefCell, collections::VecDeque, mem::take, rc::Rc, slice, sync::Arc,
  vec::IntoIter,
};

use rustc_hash::{FxHashMap, FxHashSet};
use smallvec::SmallVec;

#[cfg(feature = "fragmentation")]
use crate::layout::{
  layout_result::LayoutResult, layout_view::LiveLayout, paged_layout::PagedLayout,
};

use crate::{
  context::{BoxId, RenderContext},
  font_style::SizedFontStyle,
  fonts::{font_description::FontFallbackKey, font_fallback_list::FontFallbackList},
  geometry::{AvailableSpace, ComputedLayout, NodeId, Point, Rect, Size},
  layout::{
    block_node::LayoutCache,
    constraint_space::CacheSlot,
    grid::grid_layout_algorithm::GridGeometry,
    inline::{
      ContentBox, FragmentItems, InlineContent, InlineContentKind, InlineOutOfFlow,
      inline_node::{InlineNode, ItemShapes},
      inline_node_data::InlineNodeData,
    },
    layout_result::{OutOfFlowCandidate, StaticPosition},
    layout_style::{LayoutLength, LayoutStyle},
    layout_view::{HoistedBy, LayoutView},
    length_utils::margins_for,
    list_marker::{ListCounter, is_list_element, list_marker, owns_list_counter},
    node::{Node, NodeKind, NodeStyleLayers},
    out_of_flow::{ContainingBlock, ContainingBlockClaims, OutOfFlowLayoutPart},
    table::{ColumnConstraints, HiddenParts, TableFragment, TableStructure},
  },
  layout_unit::{BoxStrut, LayoutUnit},
  matching::{MatchedDeclarationsView, NodeMatchedDeclarations, match_stylesheets_view},
  painter::OwnContent,
  resources::font::PrimaryFontMetrics,
  sort_key::sort_by_key,
  style::{
    Affine, BackgroundImage, BackgroundImages, Color, ComputedStyle, ContentItem, ContentValue,
    Direction, Display, Float, LineHeight, MatchedStyleKey, Position, Registrations, SizingContext,
    Style as NodeStyle, StyleDeclaration, StyleDeclarationBlock, StyleSheet, WhiteSpaceCollapse,
    apply_stylesheet_animations,
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
  placements: FxHashMap<NodeId, P>,
  content_boxes: FxHashMap<NodeId, Size<Option<f32>>>,
}

impl<P> Default for ContainingBlocks<P> {
  fn default() -> Self {
    Self {
      placements: FxHashMap::default(),
      content_boxes: FxHashMap::default(),
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

  /// The placement and container size a child resolves against: its containing block's,
  /// `hoisted_cb`, when hoisted, otherwise the parent's.
  pub fn base_for(
    &self,
    hoisted_cb: Option<NodeId>,
    parent_placement: P,
    parent_content_box: Size<Option<f32>>,
  ) -> (P, Size<Option<f32>>) {
    match hoisted_cb {
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

  /// The placement a child resolves against, or `None` when it was hoisted to the containing block
  /// `hoisted_cb`, which none of the recorded nodes is.
  pub(crate) fn recorded_placement_for(
    &self,
    hoisted_cb: Option<NodeId>,
    parent_placement: P,
  ) -> Option<P> {
    match hoisted_cb {
      Some(cb) => self.placements.get(&cb).copied(),
      None => Some(parent_placement),
    }
  }
}

/// Immutable per-node layout output after computing a tree.
pub struct LayoutResults {
  pub(in crate::layout) nodes: Vec<LayoutResultNode>,
  /// Each laid-out table's paint data, by node.
  pub(in crate::layout) tables: FxHashMap<usize, Vec<TableFragment>>,
  /// The cells and rows layout hid, which paint nothing.
  pub(in crate::layout) hidden: FxHashSet<usize>,
  /// The boxes each box positioned as their containing block.
  #[cfg(feature = "fragmentation")]
  pub(in crate::layout) hoisted_children: FxHashMap<NodeId, Vec<NodeId>>,
}

pub(in crate::layout) struct LayoutResultNode {
  pub(in crate::layout) fragments: Fragments,
  pub(in crate::layout) box_children: Box<[OrderedChild]>,
}

impl LayoutResults {
  /// Lays out the tree under `root`, a box its parent placed, in `available_space`. The root's
  /// margins shrink the space but leave its border box at the origin, where the parent expects it.
  pub fn compute(root: &RenderNode, available_space: Size<AvailableSpace>) -> Self {
    let mut results = Self::lay_out(root, available_space, false);

    if let Some(root) = results.nodes.first_mut() {
      root.fragments[0].layout.location = Point::new(0.0, 0.0);
    }
    results
  }

  /// [`Self::compute`] for the root of a document, whose initial containing block fits the root
  /// when `available_space` leaves its width to the content.
  pub fn compute_document(root: &RenderNode, available_space: Size<AvailableSpace>) -> Self {
    let mut results = Self::lay_out(root, available_space, true);

    if let Some(root) = results.nodes.first_mut() {
      root.fragments[0].layout.location = Point::new(0.0, 0.0);
    }
    results
  }

  /// [`Self::compute_document`] for paint, the root at its margin-offset location.
  pub(crate) fn compute_for_paint(
    root: &RenderNode,
    available_space: Size<AvailableSpace>,
  ) -> Self {
    Self::lay_out(root, available_space, true)
  }

  fn lay_out(root: &RenderNode, available_space: Size<AvailableSpace>, is_document: bool) -> Self {
    let mut tree = LayoutTree::from_render_node(root);
    let available_space = if is_document {
      tree.initial_containing_block(available_space)
    } else {
      available_space
    };

    tree.compute_layout(available_space);
    tree.into_results()
  }

  /// The layout as a [`LayoutView`].
  pub fn view(&self) -> LayoutView<'_> {
    LayoutView::Final(self)
  }

  /// Computed layout of a node.
  pub fn layout(&self, node_id: NodeId) -> crate::Result<ComputedLayout> {
    self.view().layout(node_id)
  }

  /// Computed layout of a node's fragment `fragment_index`.
  pub fn fragment_layout(
    &self,
    node_id: NodeId,
    fragment_index: usize,
  ) -> crate::Result<ComputedLayout> {
    self.view().fragment_layout(node_id, fragment_index)
  }

  /// Whether the node's fragment `fragment_index` repeats an earlier one, as a table header does on each page.
  pub fn is_repeated_fragment(&self, node_id: NodeId, fragment_index: usize) -> bool {
    self.view().is_repeated_fragment(node_id, fragment_index)
  }

  /// Paint-ordered children of a node.
  pub fn box_children(&self, node_id: NodeId) -> crate::Result<&[OrderedChild]> {
    self.view().box_children(node_id)
  }

  /// The paint data of the table `node_id`'s fragment `fragment_index`, when it is one.
  pub fn table_fragment(&self, node_id: NodeId, fragment_index: usize) -> Option<&TableFragment> {
    self.view().table_fragment(node_id, fragment_index)
  }

  /// Whether layout hid the box `node_id`, as Blink's `IsHiddenForPaint`: a cell in collapsed
  /// columns or empty under `empty-cells: hide`, or a collapsed row.
  pub fn is_hidden_for_paint(&self, node_id: NodeId) -> bool {
    self.view().is_hidden_for_paint(node_id)
  }

  /// Blink's `IsLineClampClippedFloat`: whether `node_id` is a float its line-clamp container
  /// clips to its content box's block end.
  pub fn is_line_clamp_clipped_float(&self, node_id: NodeId) -> bool {
    self.view().is_line_clamp_clipped_float(node_id)
  }

  /// [`Self::compute_document`] laid out across pages `page_size`, with the page count.
  #[cfg(feature = "fragmentation")]
  pub(crate) fn compute_pages(
    root: &RenderNode,
    page_size: Size<f32>,
    max_pages: usize,
  ) -> crate::Result<(Self, usize)> {
    let mut layout = PagedLayout::new(root, page_size, max_pages);

    while layout.next_page()? {}

    let page_count = layout.page_count();

    Ok((layout.into_results(), page_count))
  }

  /// How many boxes were laid out.
  #[cfg(all(test, feature = "fragmentation"))]
  pub(crate) fn node_count(&self) -> usize {
    self.nodes.len()
  }

  /// Each fragment of `node_id`, by fragment index.
  #[cfg(all(test, feature = "fragmentation"))]
  pub(crate) fn fragments(&self, node_id: NodeId) -> crate::Result<&[BoxFragment]> {
    self.view().fragments(node_id)
  }
}

/// The boxes of a render tree as layout lays them out.
pub struct LayoutTree<'r> {
  nodes: Vec<LayoutNodeState>,
  render_nodes: Vec<&'r RenderNode>,
  /// The inline box that is the containing block of each out-of-flow box an inline formatting
  /// context holds, by node, when one is.
  inline_containers: FxHashMap<usize, &'r RenderNode>,
  /// Each inline formatting context's prepared items, which every pass of one layout shares, as
  /// Blink keeps them until `NeedsCollectInlines`.
  inline_node_data: RefCell<FxHashMap<usize, Rc<InlineNodeData<'r>>>>,
  /// Each table's parts, gathered when the tree is built.
  tables: FxHashMap<usize, Rc<TableStructure>>,
  /// Each table's column constraints, measured on its first layout.
  table_columns: FxHashMap<usize, Rc<ColumnConstraints>>,
  /// Each table's paint data from its last full layout, one per fragment.
  table_fragments: FxHashMap<usize, Vec<TableFragment>>,
  /// The cells and rows each table's layout hid, which every fragment of it shares.
  table_hidden: FxHashMap<NodeId, HiddenParts>,
  /// Every cell and row some table's layout hid.
  hidden: FxHashSet<usize>,
  /// Which boxes positioned each out-of-flow box as their containing block.
  pub(in crate::layout) hoisted_by: HoistedBy,
  /// How many algorithm runs have started.
  #[cfg(feature = "fragmentation")]
  runs: u32,
  /// The algorithm runs under way, the innermost last.
  #[cfg(feature = "fragmentation")]
  running: Vec<PlacedBy>,
  /// The layout styles the latest table parts took, which an equal part's shares.
  table_part_styles: RecentStyles<LayoutStyle>,
  /// The node each atomic inline, float, list marker, and block-in-inline of an inline formatting
  /// context lays out as, made the first time its line lays it out.
  atomic_inlines: FxHashMap<BoxId, NodeId>,
  /// Each inline formatting context's lines from its block layout, until it finishes.
  pub(crate) inline_contents: FxHashMap<NodeId, InlineContent<'r>>,
  /// The first layout of each repeated table header and footer, which every repetition reuses,
  /// as Blink reads a repeated root's first layout result.
  #[cfg(feature = "fragmentation")]
  pub(crate) repeated_roots: FxHashMap<NodeId, Rc<LayoutResult>>,
}

/// What a box's part in a table changes in the styles it is laid out with.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct TableBoxStyle {
  /// The border it lays out with in place of its own.
  pub(crate) border: Option<BoxStrut>,
  /// Whether its padding is dropped, as a row, row group, or collapsing table drops it.
  pub(crate) clears_padding: bool,
  /// Whether its `width`, `height`, and margins are dropped, as a cell takes the size its table
  /// gives it.
  pub(crate) sized_by_table: bool,
  /// Whether it lays out at all: a column and column group do not.
  pub(crate) hidden: bool,
  /// Whether it is a table, which a block container shrinks to fit rather than stretching.
  pub(crate) is_table: bool,
}

impl TableBoxStyle {
  pub(crate) const TABLE: Self = Self {
    border: None,
    clears_padding: false,
    sized_by_table: false,
    hidden: false,
    is_table: true,
  };
  pub(crate) const PART: Self = Self {
    border: Some(BoxStrut::ZERO),
    clears_padding: true,
    is_table: false,
    ..Self::TABLE
  };
  pub(crate) const CELL: Self = Self {
    sized_by_table: true,
    is_table: false,
    ..Self::TABLE
  };
  pub(crate) const HIDDEN: Self = Self {
    hidden: true,
    is_table: false,
    ..Self::TABLE
  };

  /// `layout_style` as this part of a table lays out with it.
  fn apply(&self, layout_style: &mut LayoutStyle) {
    if *self == Self::default() {
      return;
    }
    if self.hidden {
      layout_style.display = Display::None;
      return;
    }
    if let Some(border) = self.border {
      layout_style.border = Rect {
        top: border.top.to_float(),
        right: border.right.to_float(),
        bottom: border.bottom.to_float(),
        left: border.left.to_float(),
      };
    }
    if self.clears_padding {
      layout_style.padding = Rect::all(LayoutLength::Fixed(0.0));
    }
    if self.sized_by_table {
      layout_style.size = Size::all(LayoutLength::Auto);
      layout_style.min_size = Size::all(LayoutLength::Auto);
      layout_style.max_size = Size::all(LayoutLength::Auto);
      layout_style.margin = Rect::all(LayoutLength::Fixed(0.0));
    }
  }
}

/// A box as layout lays it out: its styles, what it keeps of its layouts, and where it went.
pub(crate) struct LayoutNodeState {
  /// The style it lays out with, shared with recent nodes whose style came out equal.
  pub(crate) layout_style: Rc<LayoutStyle>,
  /// Whether the styles are the same whatever query container they resolve against, so repeated
  /// layouts can reuse them.
  container_independent: bool,
  pub(crate) cache: LayoutCache,
  /// Blink's `LayoutBox::layout_results_`: where each of its fragments went, by fragment index.
  pub(crate) fragments: Fragments,
  pub(crate) is_inline_children: bool,
  /// Whether its inline formatting context lays it out as an atomic inline, float, list marker,
  /// or block-in-inline.
  pub(crate) is_atomic_inline: bool,
  /// What its part in a table changes in its styles.
  pub(crate) table_box: TableBoxStyle,
  /// Its children in layout order, or for an inline formatting context the out-of-flow boxes in
  /// its inline content.
  pub(crate) children: Rc<[NodeId]>,
  /// Its children in paint order, the boxes of its inline content among them.
  pub(crate) box_children: Box<[OrderedChild]>,
  /// The out-of-flow boxes this node positioned as their containing block, from its last layout.
  pub(crate) hoisted_children: Vec<NodeId>,
  /// The grid this node last laid out, which places its out-of-flow children in their areas.
  pub(crate) grid_geometry: Option<Rc<GridGeometry>>,
  /// The out-of-flow boxes this node is the containing block for.
  pub(crate) claims: ContainingBlockClaims,
}

/// A box's fragments, one unless it fragmented.
pub(crate) type Fragments = SmallVec<[BoxFragment; 1]>;

/// Blink's `PhysicalBoxFragment` as takumi keeps it: where its parent placed one fragment of a box.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct BoxFragment {
  pub(crate) layout: ComputedLayout,
  /// Blink's `IsLineClampClippedFloat` of the layout that placed it.
  pub(crate) is_line_clamp_clipped_float: bool,
  /// Which of the parent's fragments holds it, as Blink's `PhysicalFragmentLink` in that
  /// fragment's children says.
  #[cfg(feature = "fragmentation")]
  pub(crate) container_fragment_index: usize,
  /// Whether layout placed or hid the box, as a box the layout has not reached is not.
  #[cfg(feature = "fragmentation")]
  pub(crate) is_laid_out: bool,
  /// The run of the box's algorithm that made the layout this is the fragment of.
  #[cfg(feature = "fragmentation")]
  pub(crate) run: u32,
  /// The run of the container's algorithm that placed it. A later run of the container that
  /// places the fragment `container_fragment_index` again replaces this one, so one left from an
  /// earlier run, such as a measuring run, names a fragment that does not hold the box.
  #[cfg(feature = "fragmentation")]
  pub(crate) placed_by: PlacedBy,
  /// Whether the fragment repeats an earlier one, as a table header does on each page.
  pub(crate) is_repeated: bool,
}

/// A run of a box's layout algorithm: the box and which of its runs, counting from one. Zero is no
/// run, as when a page places the root.
#[cfg(feature = "fragmentation")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct PlacedBy {
  pub(crate) node: u32,
  pub(crate) run: u32,
}

#[cfg(feature = "fragmentation")]
impl BoxFragment {
  /// Where in `fragments`, a box's fragments in layout order, are the ones its parent's fragment
  /// `container` holds. Layout places a box's fragments in its parent's fragments in order.
  pub(crate) fn held_by(fragments: &[Self], container: usize) -> Range<usize> {
    let start = fragments.partition_point(|fragment| fragment.container_fragment_index < container);
    let len =
      fragments[start..].partition_point(|fragment| fragment.container_fragment_index == container);

    start..start + len
  }
}

/// The styles the latest nodes laid out with, which an equal style shares instead of a copy, as
/// the inline layout cache shares faces.
struct RecentStyles<T>(VecDeque<Rc<T>>);

impl<T> Default for RecentStyles<T> {
  fn default() -> Self {
    Self(VecDeque::new())
  }
}

impl<T: PartialEq> RecentStyles<T> {
  const LEN: usize = 8;

  /// `style`, or an equal one a recent node laid out with.
  fn share(&mut self, style: T) -> Rc<T> {
    if let Some(shared) = self.0.iter().find(|shared| ***shared == style) {
      return Rc::clone(shared);
    }

    let style = Rc::new(style);

    if self.0.len() == Self::LEN {
      self.0.pop_front();
    }
    self.0.push_back(Rc::clone(&style));
    style
  }
}

impl LayoutNodeState {
  fn new(
    layout_style: Rc<LayoutStyle>,
    container_independent: bool,
    is_inline_children: bool,
    claims: ContainingBlockClaims,
  ) -> Self {
    Self {
      layout_style,
      container_independent,
      cache: LayoutCache::default(),
      fragments: SmallVec::from_buf([BoxFragment::default()]),
      is_inline_children,
      is_atomic_inline: false,
      table_box: TableBoxStyle::default(),
      children: Rc::new([]),
      box_children: Box::new([]),
      hoisted_children: Vec::new(),
      grid_geometry: None,
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
  pub(crate) layout_style_override: Option<Rc<LayoutStyle>>,
  /// Text for an anonymous inline-text wrapper.
  pub anonymous_text_content: Option<String>,
  pub(crate) force_inline_layout: bool,
  /// Blink's `IsBlockInInline`: the anonymous block holding block-level children of an inline box.
  pub(crate) is_block_in_inline: bool,
  /// Whether it is the root element, whose box always establishes an independent formatting
  /// context, as Blink's `CreatesNewFormattingContext` reads `IsDocumentElement`.
  pub(crate) is_root_element: bool,
  /// The fragment items its inline content last laid out to.
  pub(crate) fragment_items: FragmentItemsCache,
  /// The shapes its inline items took when its inline content was last prepared.
  pub(crate) item_shapes: ItemShapes,
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
    };

    collect(self, &mut stack);
    while let Some(mut node) = stack.pop() {
      collect(&mut node, &mut stack);
    }
  }
}

/// The fragment items a node's inline content last laid out to, by fragment index. They belong to
/// that node, so a clone, which may change its style or content, starts without them.
#[derive(Default)]
pub(crate) struct FragmentItemsCache(RefCell<SmallVec<[Option<Rc<FragmentItems>>; 1]>>);

impl FragmentItemsCache {
  /// Keeps `items` as fragment `fragment_index`'s.
  pub(crate) fn keep(&self, fragment_index: usize, items: Rc<FragmentItems>) {
    let mut fragments = self.0.borrow_mut();

    if fragments.len() <= fragment_index {
      fragments.resize(fragment_index + 1, None);
    }
    fragments[fragment_index] = Some(items);
  }
}

impl FragmentItemsCache {
  /// Keeps fragment `source`'s items as fragment `target`'s too.
  #[cfg(feature = "fragmentation")]
  pub(crate) fn repeat(&self, source: usize, target: usize) {
    let items = self.0.borrow().get(source).cloned().flatten();

    if let Some(items) = items {
      self.keep(target, items);
    }
  }
}

impl Clone for FragmentItemsCache {
  fn clone(&self) -> Self {
    Self::default()
  }
}

thread_local! {
  /// The layout an anonymous block box takes in place of a computed style, shared by all of them
  /// in each direction.
  static BLOCK_STYLES: [Rc<LayoutStyle>; 2] = [Direction::Ltr, Direction::Rtl]
    .map(|direction| Rc::new(LayoutStyle { direction, ..LayoutStyle::default() }));
}

/// The layout of an anonymous block box in `context`, whose direction it inherits.
fn block_style(context: &RenderContext) -> Rc<LayoutStyle> {
  let index = usize::from(context.style.inherited_data.direction == Direction::Rtl);

  BLOCK_STYLES.with(|styles| Rc::clone(&styles[index]))
}

/// An element's own important declarations.
struct ElementImportant {
  inline: Option<StyleDeclarationBlock>,
}

fn registered_custom_property_parent_style<'a>(
  parent_style: &'a ComputedStyle,
  registrations: &Registrations,
) -> Cow<'a, ComputedStyle> {
  // Every element below the first to take the registrations carries them already, and
  // `inherited` hands a child each one's value from there.
  if registrations.is_empty() || parent_style.custom_properties.registers(registrations) {
    return Cow::Borrowed(parent_style);
  }

  let mut adjusted_parent = parent_style.clone();

  adjusted_parent
    .custom_properties
    .register_in_scope(registrations, &parent_style.custom_properties);
  Cow::Owned(adjusted_parent)
}

/// The `@property` rules in force at `viewport`, a later rule for a name replacing an earlier one.
fn active_registrations(stylesheets: &[StyleSheet], viewport: Viewport) -> Registrations {
  Arc::new(
    stylesheets
      .iter()
      .flat_map(StyleSheet::property_rules)
      .filter(|rule| {
        rule
          .media_queries
          .iter()
          .all(|media_query| media_query.matches(viewport))
      })
      .map(|rule| (rule.name.clone(), rule.clone()))
      .collect(),
  )
}

impl<'r> LayoutTree<'r> {
  /// Builds a layout tree from a render-node root.
  pub fn from_render_node(render_root: &'r RenderNode) -> Self {
    let capacity = render_root.layout_node_estimate();
    let mut tree = Self {
      nodes: Vec::with_capacity(capacity),
      render_nodes: Vec::with_capacity(capacity),
      inline_containers: FxHashMap::default(),
      inline_node_data: RefCell::default(),
      tables: FxHashMap::default(),
      table_columns: FxHashMap::default(),
      table_fragments: FxHashMap::default(),
      table_hidden: FxHashMap::default(),
      hidden: FxHashSet::default(),
      hoisted_by: HoistedBy::default(),
      #[cfg(feature = "fragmentation")]
      runs: 0,
      #[cfg(feature = "fragmentation")]
      running: Vec::new(),
      table_part_styles: RecentStyles::default(),
      atomic_inlines: FxHashMap::default(),
      inline_contents: FxHashMap::default(),
      #[cfg(feature = "fragmentation")]
      repeated_roots: FxHashMap::default(),
    };

    tree.push_subtree(render_root);
    tree.nodes.shrink_to_fit();
    tree.render_nodes.shrink_to_fit();
    tree.prepare_tables(0);

    tree
  }

  /// Adds the atomic inline `child` its inline content placed to the boxes `parent` paints, at
  /// `path` among `parent`'s render children, keeping them in tree order.
  pub(crate) fn add_inline_box_children(
    &mut self,
    parent: NodeId,
    children: Vec<(NodeId, SmallVec<[usize; 2]>)>,
  ) {
    let state = &mut self.nodes[usize::from(parent)];
    let mut known: FxHashSet<NodeId> = state
      .box_children
      .iter()
      .map(|known| known.node_id)
      .collect();
    let mut box_children = state.box_children.to_vec();

    for (child, path) in children {
      let Some((&render_index, inline_path)) = path.split_first() else {
        continue;
      };

      if known.insert(child) {
        box_children.push(OrderedChild {
          render_index,
          inline_path: (!inline_path.is_empty()).then(|| inline_path.into()),
          node_id: child,
          hoisted_cb: None,
        });
      }
    }
    if box_children.len() == state.box_children.len() {
      return;
    }
    box_children.sort_by(|a, b| {
      (a.render_index, a.inline_path.as_deref()).cmp(&(b.render_index, b.inline_path.as_deref()))
    });
    state.box_children = box_children.into_boxed_slice();
  }

  /// The node the box `render_node` of an inline formatting context lays out as, its subtree
  /// appended to the tree the first time.
  pub(crate) fn atomic_inline_node(&mut self, render_node: &'r RenderNode) -> NodeId {
    if let Some(&node) = self.atomic_inlines.get(&render_node.context.box_id()) {
      return node;
    }

    let start = self.nodes.len();
    let node = NodeId::new(start);

    self.push_subtree(render_node);
    self.nodes[start].is_atomic_inline = true;
    self.prepare_tables(start);
    self
      .atomic_inlines
      .insert(render_node.context.box_id(), node);
    node
  }

  /// The prepared items of the inline formatting context the node at `index` establishes.
  pub(crate) fn inline_node_data(&self, index: usize) -> Rc<InlineNodeData<'r>> {
    Rc::clone(
      self
        .inline_node_data
        .borrow_mut()
        .entry(index)
        .or_insert_with(|| Rc::new(InlineNode::new(self.render_nodes[index]).prepare_layout())),
    )
  }

  /// Appends the layout nodes of the subtree under `render_root`.
  fn push_subtree(&mut self, render_root: &'r RenderNode) {
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
      node_id: NodeId,
      /// The path from its parent, as its parent's [`OrderedChild`] records it.
      render_path: SmallVec<[usize; 2]>,
      /// The inline box containing it, when it is out of flow.
      inline_container: Option<&'r RenderNode>,
      next_child_index: usize,
      children: PendingChildren<'r>,
      child_ids: Vec<NodeId>,
      box_children: Vec<OrderedChild>,
    }

    fn push_node_state<'r>(
      nodes: &mut Vec<LayoutNodeState>,
      render_nodes: &mut Vec<&'r RenderNode>,
      recent_styles: &mut RecentStyles<LayoutStyle>,
      InlineOutOfFlow {
        path: render_path,
        node: render_node,
        container: inline_container,
      }: InlineOutOfFlow<'r>,
    ) -> PendingNode<'r> {
      let node_id = NodeId::new(nodes.len());
      let is_inline_children = render_node.should_create_inline_layout();
      let children = if is_inline_children {
        PendingChildren::OutOfFlow(render_node.inline_out_of_flow())
      } else {
        PendingChildren::Render(render_node.children.as_deref().unwrap_or_default())
      };
      render_nodes.push(render_node);

      let (layout_style, container_independent) = {
        let sizing = &render_node.context.sizing;

        // Resolution reports whether it read the query container, which is
        // exact. Comparing two resolved sizes is not: `min(10px, 100cqw)`
        // agrees across two large containers and disagrees with a small one.
        sizing.container_read.set(false);
        let layout_style = render_node.layout_box_style(sizing);
        let independent = !sizing.container_read.get();

        (layout_style, independent)
      };

      nodes.push(LayoutNodeState::new(
        recent_styles.share(layout_style),
        container_independent,
        is_inline_children,
        ContainingBlockClaims::of(&render_node.context.style),
      ));

      let capacity = children.len();
      PendingNode {
        node_id,
        render_path,
        inline_container,
        next_child_index: 0,
        children,
        child_ids: Vec::with_capacity(capacity),
        box_children: Vec::with_capacity(capacity),
      }
    }

    let Self {
      nodes,
      render_nodes,
      inline_containers,
      ..
    } = self;
    let mut recent_styles = RecentStyles::default();
    let root = push_node_state(
      nodes,
      render_nodes,
      &mut recent_styles,
      InlineOutOfFlow {
        path: SmallVec::new(),
        node: render_root,
        container: None,
      },
    );
    let mut stack = vec![root];

    while let Some(current) = stack.last_mut() {
      if let Some(child) = current.children.get(current.next_child_index) {
        current.next_child_index += 1;
        let pending = push_node_state(nodes, render_nodes, &mut recent_styles, child);

        stack.push(pending);
        continue;
      }

      let Some(finished) = stack.pop() else {
        break;
      };
      let index = usize::from(finished.node_id);
      let mut child_ids = finished.child_ids;

      let display = nodes[index].layout_style.display;

      if display.is_flex() || display.is_grid() {
        sort_children_by_order(&mut child_ids, |child_id| {
          render_nodes
            .get(usize::from(child_id))
            .map_or(0, |child| child.context.style.misc3_data.order.0)
        });
      }
      nodes[index].children = child_ids.into();
      nodes[index].box_children = finished.box_children.into_boxed_slice();

      if let Some(parent) = stack.last_mut() {
        if let Some(container) = finished.inline_container {
          inline_containers.insert(index, container);
        }
        parent.child_ids.push(finished.node_id);

        let (&render_index, inline_path) = finished.render_path.split_first().unwrap_or((&0, &[]));

        parent.box_children.push(OrderedChild {
          render_index,
          inline_path: (!inline_path.is_empty()).then(|| inline_path.into()),
          node_id: finished.node_id,
          hoisted_cb: None,
        });
      }
    }
  }

  /// Computes the layout for the whole tree: the root at its margin edge's offset in
  /// `available_space`, which is the initial containing block of the out-of-flow boxes no other
  /// box contains.
  pub fn compute_layout(&mut self, available_space: Size<AvailableSpace>) {
    let space = self.root_space(available_space);
    let definite = |available: AvailableSpace| match available {
      AvailableSpace::Definite(size) => LayoutUnit::from_float(size),
      AvailableSpace::MinContent | AvailableSpace::MaxContent => LayoutUnit::ZERO,
    };
    let (location, result) = if self.layout_style(NodeId::ROOT).is_out_of_flow() {
      // An out-of-flow root is positioned in the initial containing block.
      let initial_containing_block = ContainingBlock {
        origin: Point::new(LayoutUnit::ZERO, LayoutUnit::ZERO),
        size: available_space.map(definite),
      };

      self.lay_out_out_of_flow_box(
        &OutOfFlowCandidate::new(NodeId::ROOT, StaticPosition::default()),
        initial_containing_block,
        &space,
      )
    } else {
      let margins = margins_for(
        self.layout_style(NodeId::ROOT),
        space.percentage_resolution_inline_size,
        Direction::Ltr,
      );

      (
        Point::new(margins.inline_start, margins.block_start),
        self.layout(NodeId::ROOT, &space),
      )
    };

    self.place_child(NodeId::ROOT, 0, 0, location, &result);

    if result.out_of_flow.is_empty() {
      return;
    }

    // An indefinite axis of the initial containing block falls back to the root's padding box.
    let root_padding_box = ContainingBlock::padding_box(
      Size {
        width: result.size.inline_size,
        height: result.size.block_size,
      },
      result.border,
    );
    let viewport = |available: AvailableSpace,
                    origin: LayoutUnit,
                    fallback: (LayoutUnit, LayoutUnit)| match available {
      AvailableSpace::Definite(size) => (LayoutUnit::ZERO - origin, LayoutUnit::from_float(size)),
      AvailableSpace::MinContent | AvailableSpace::MaxContent => fallback,
    };
    let (x, width) = viewport(
      available_space.width,
      location.x,
      (root_padding_box.origin.x, root_padding_box.size.width),
    );
    let (y, height) = viewport(
      available_space.height,
      location.y,
      (root_padding_box.origin.y, root_padding_box.size.height),
    );

    self.lay_out_out_of_flow(
      OutOfFlowLayoutPart::new(
        (NodeId::ROOT, 0),
        ContainingBlockClaims::ALL,
        ContainingBlock {
          origin: Point::new(x, y),
          size: Size { width, height },
        },
        &space,
      ),
      result.out_of_flow.clone(),
    );
  }

  /// The initial containing block of a document laid out in `available_space`: a min-content or
  /// max-content width becomes the root's contribution and margins, which the root then fills.
  fn initial_containing_block(
    &mut self,
    available_space: Size<AvailableSpace>,
  ) -> Size<AvailableSpace> {
    if let AvailableSpace::Definite(_) = available_space.width {
      return available_space;
    }

    let (min_width, max_width) = self.root_min_max_widths(available_space.height);
    let margins = margins_for(self.layout_style(NodeId::ROOT), None, Direction::Ltr);
    let width = match available_space.width {
      AvailableSpace::MinContent => min_width,
      _ => max_width,
    };

    Size {
      width: AvailableSpace::Definite(width + margins.inline_sum().to_float()),
      ..available_space
    }
  }

  /// The root's border-box min-content and max-content widths in a containing block
  /// `available_height` tall, sized without laying out its height, as Blink's
  /// `ComputeMinMaxSizes` sizes a box's inline axis alone.
  pub(crate) fn root_min_max_widths(&mut self, available_height: AvailableSpace) -> (f32, f32) {
    let mut space = self.root_space(Size {
      width: AvailableSpace::MaxContent,
      height: available_height,
    });

    space.available_inline_size = None;
    space.cache_slot = CacheSlot::Measure;

    let sizes = self
      .min_and_max_content_contribution(NodeId::ROOT, &space)
      .sizes;

    (sizes.min_size.to_float(), sizes.max_size.to_float())
  }

  /// Consumes the tree into immutable per-node layout results.
  pub(crate) fn into_results(mut self) -> LayoutResults {
    // A box another node than its box-tree parent positioned resolves against that containing
    // block.
    for (index, node) in self.nodes.iter_mut().enumerate() {
      for child in node.box_children.iter_mut() {
        child.hoisted_cb = self
          .hoisted_by
          .containing_block(child.node_id)
          .filter(|&container| usize::from(container) != index);
      }
    }

    #[cfg(feature = "fragmentation")]
    let hoisted_children = self
      .nodes
      .iter()
      .enumerate()
      .filter(|(_, node)| !node.hoisted_children.is_empty())
      .map(|(index, node)| (NodeId::new(index), node.hoisted_children.clone()))
      .collect();

    // A fresh allocation: collecting in place keeps the node states' much larger one alive.
    let mut nodes = Vec::with_capacity(self.nodes.len());

    nodes.extend(self.nodes.into_iter().map(|node| LayoutResultNode {
      fragments: node.fragments,
      box_children: node.box_children,
    }));
    LayoutResults {
      nodes,
      tables: self.table_fragments,
      hidden: self.hidden,
      #[cfg(feature = "fragmentation")]
      hoisted_children,
    }
  }

  /// The cells and rows the layout of the table `table` hid.
  pub(in crate::layout) fn hidden_parts(&self, table: NodeId) -> Option<&HiddenParts> {
    self.table_hidden.get(&table)
  }

  /// Keeps `parts` as the cells and rows the layout of the table `table` hid, in place of the ones
  /// it hid before.
  pub(in crate::layout) fn set_hidden_parts(&mut self, table: NodeId, parts: HiddenParts) {
    let before = self.table_hidden.get(&table).map(|before| &before.nodes);

    for &node in before.into_iter().flatten() {
      self.hidden.remove(&usize::from(node));
    }
    self
      .hidden
      .extend(parts.nodes.iter().map(|&node| usize::from(node)));
    self.table_hidden.insert(table, parts);
  }

  /// The layout so far, for reading while it lays out more pages.
  #[cfg(feature = "fragmentation")]
  pub fn view(&self) -> LayoutView<'_> {
    LayoutView::Live(LiveLayout {
      nodes: &self.nodes,
      tables: &self.table_fragments,
      hidden: &self.hidden,
      hoisted_by: &self.hoisted_by,
    })
  }

  /// Starts a run of `node`'s algorithm, and returns its number.
  #[cfg(feature = "fragmentation")]
  pub(in crate::layout) fn begin_run(&mut self, node: NodeId) -> u32 {
    self.runs += 1;
    self.running.push(PlacedBy {
      node: usize::from(node) as u32,
      run: self.runs,
    });
    self.runs
  }

  /// Ends the innermost run.
  #[cfg(feature = "fragmentation")]
  pub(in crate::layout) fn end_run(&mut self) {
    self.running.pop();
  }

  /// The run placing a fragment now: the innermost one, none when a page places the root.
  #[cfg(feature = "fragmentation")]
  pub(in crate::layout) fn running(&self) -> PlacedBy {
    self.running.last().copied().unwrap_or_default()
  }

  pub(crate) fn node_state(&self, node: NodeId) -> &LayoutNodeState {
    &self.nodes[usize::from(node)]
  }

  pub(crate) fn node_state_mut(&mut self, node: NodeId) -> &mut LayoutNodeState {
    &mut self.nodes[usize::from(node)]
  }

  /// Resolves `node`'s styles again against a query container of `container_size`, unless they
  /// read no query container.
  pub(crate) fn restyle_for_container(
    &mut self,
    node: NodeId,
    container_size: Size<Option<LayoutUnit>>,
  ) {
    let index = usize::from(node);
    let render_node = self.render_nodes[index];
    let state = &mut self.nodes[index];

    if state.container_independent {
      return;
    }

    let mut sizing = render_node.context.sizing.clone();

    sizing.container_size = container_size.map(|size| size.map(LayoutUnit::to_float));

    let mut layout_style = render_node.layout_box_style(&sizing);

    state.table_box.apply(&mut layout_style);
    state.layout_style = Rc::new(layout_style);
  }

  /// Blink's `LayoutBox::SetLayoutResult`: keeps `fragment` as `node`'s fragment
  /// `fragment_index`.
  pub(crate) fn set_fragment(
    &mut self,
    node: NodeId,
    fragment_index: usize,
    fragment: BoxFragment,
  ) {
    let fragments = &mut self.nodes[usize::from(node)].fragments;

    if fragment_index < fragments.len() {
      fragments[fragment_index] = fragment;
    } else {
      debug_assert_eq!(
        fragment_index,
        fragments.len(),
        "a fragment follows the one before it"
      );
      fragments.push(fragment);
    }
  }

  /// The number of nodes in the tree.
  pub(in crate::layout) fn node_count(&self) -> usize {
    self.nodes.len()
  }

  /// The render node `node` lays out.
  pub(in crate::layout) fn render_node(&self, node: impl Into<usize>) -> &'r RenderNode {
    self.render_nodes[node.into()]
  }

  /// The children `node` lays out, in layout order.
  pub(crate) fn child_nodes(&self, node: NodeId) -> Rc<[NodeId]> {
    Rc::clone(&self.node_state(node).children)
  }

  /// The style `node` lays out with.
  pub(crate) fn layout_style(&self, node: NodeId) -> &LayoutStyle {
    &self.node_state(node).layout_style
  }

  pub(crate) fn layout_style_rc(&self, node: NodeId) -> &Rc<LayoutStyle> {
    &self.node_state(node).layout_style
  }

  /// The inline box that is the containing block of the out-of-flow `node` in an inline
  /// formatting context, if one is.
  pub(crate) fn inline_container(&self, node: NodeId) -> Option<&'r RenderNode> {
    self.inline_containers.get(&usize::from(node)).copied()
  }

  /// The inline spans containing any of `boxes`, each once, in the order `boxes` first meets them.
  pub(crate) fn inline_containers_of(&self, boxes: &[NodeId]) -> Vec<&'r RenderNode> {
    let mut containers: Vec<&'r RenderNode> = Vec::new();

    for &child in boxes {
      if let Some(container) = self.inline_container(child)
        && !containers
          .iter()
          .any(|known| known.context.box_id() == container.context.box_id())
      {
        containers.push(container);
      }
    }

    containers
  }

  pub(in crate::layout) fn table_structure(&self, table: NodeId) -> Option<Rc<TableStructure>> {
    self.tables.get(&usize::from(table)).cloned()
  }

  pub(in crate::layout) fn set_table_structure(
    &mut self,
    table: NodeId,
    structure: TableStructure,
  ) {
    self.tables.insert(usize::from(table), Rc::new(structure));
  }

  pub(in crate::layout) fn table_column_constraints(
    &self,
    table: NodeId,
  ) -> Option<Rc<ColumnConstraints>> {
    self.table_columns.get(&usize::from(table)).cloned()
  }

  pub(in crate::layout) fn set_table_column_constraints(
    &mut self,
    table: NodeId,
    columns: Rc<ColumnConstraints>,
  ) {
    self.table_columns.insert(usize::from(table), columns);
  }

  /// Copies the table `table`'s paint data for fragment `source` to fragment `target`.
  #[cfg(feature = "fragmentation")]
  pub(in crate::layout) fn repeat_table_fragment(
    &mut self,
    table: NodeId,
    source: usize,
    target: usize,
  ) {
    let Some(fragments) = self.table_fragments.get_mut(&usize::from(table)) else {
      return;
    };
    let Some(fragment) = fragments.get(source).cloned() else {
      return;
    };

    if fragments.len() <= target {
      fragments.resize_with(target + 1, TableFragment::default);
    }
    fragments[target] = fragment;
  }

  pub(in crate::layout) fn set_table_fragment(
    &mut self,
    table: NodeId,
    fragment_index: usize,
    fragment: TableFragment,
  ) {
    let fragments = self.table_fragments.entry(usize::from(table)).or_default();

    fragments.truncate(fragment_index);
    push_tight(fragments, fragment);
  }

  /// Changes what `node`'s part in a table changes in its styles.
  pub(in crate::layout) fn set_table_box_style(&mut self, node: NodeId, table_box: TableBoxStyle) {
    let index = usize::from(node);
    let render_node = self.render_nodes[index];
    let sizing = &render_node.context.sizing;
    let mut layout_style = render_node.layout_box_style(sizing);
    let state = &mut self.nodes[index];

    table_box.apply(&mut layout_style);
    state.table_box = table_box;
    state.layout_style = self.table_part_styles.share(layout_style);
    state.cache.clear();
  }

  /// `node`'s layout so far.
  pub(in crate::layout) fn computed_layout(
    &self,
    node: NodeId,
    fragment_index: usize,
  ) -> ComputedLayout {
    self.nodes[usize::from(node)].fragments[fragment_index].layout
  }
}

/// Pushes `item`, allocating room for only it when `entries` is empty: most boxes keep one entry
/// per kind, where a first push would make room for four.
pub(crate) fn push_tight<T>(entries: &mut Vec<T>, item: T) {
  if entries.capacity() == 0 {
    entries.reserve_exact(1);
  }
  entries.push(item);
}

/// Stable-sorts flex/grid children by `order`, keeping source order for ties.
fn sort_children_by_order(children: &mut [NodeId], mut child_order: impl FnMut(NodeId) -> i32) {
  if children.iter().all(|&child_id| child_order(child_id) == 0) {
    return;
  }
  sort_by_key(children, |&child_id| child_order(child_id));
}

impl RenderNode {
  /// Blink's `HasDescendantThatDependsOnPercentageBlockSize` of the box's layout: an in-flow box
  /// inside it whose block size resolves a percentage, through anonymous and inline boxes, or a
  /// relatively positioned one whose block insets do.
  pub(crate) fn has_descendant_that_depends_on_percentage_block_size(&self) -> bool {
    self.children.iter().flatten().any(|child| {
      let style = child.layout_box_style(&child.context.sizing);

      !style.is_out_of_flow()
        && (child.depends_on_percentage_block_size(&style)
          || (style.position == Position::Relative
            && (style.inset.top.has_percent() || style.inset.bottom.has_percent())))
    })
  }

  /// Blink's `DependsOnPercentageBlockSize` of the box's fragment laid out with `style`: its block
  /// size resolves a percentage, or the content of an inline or anonymous box does.
  fn depends_on_percentage_block_size(&self, style: &LayoutStyle) -> bool {
    if self.is_inline_span() {
      return self.has_descendant_that_depends_on_percentage_block_size();
    }

    style.size.height.may_have_percent_dependence()
      || style.min_size.height.may_have_percent_dependence()
      || style.max_size.height.may_have_percent_dependence()
      || (self.origin == NodeOrigin::Anonymous
        && self.has_descendant_that_depends_on_percentage_block_size())
  }
}

impl RenderNode {
  /// The style the native layout lays this node out with: its own override when it has one,
  /// otherwise its computed style.
  pub(crate) fn layout_box_style(&self, sizing: &SizingContext) -> LayoutStyle {
    match self.layout_style_override.as_deref() {
      Some(style) => LayoutStyle {
        is_replaced: self.is_replaced(),
        ..style.clone()
      },
      None => LayoutStyle::from_computed(&self.context.style, sizing, self.is_replaced()),
    }
  }

  fn is_replaced(&self) -> bool {
    self.node.as_ref().is_some_and(Node::is_replaced_element)
  }

  /// A box with no layout override.
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
      force_inline_layout: false,
      is_block_in_inline: false,
      is_root_element: false,
      fragment_items: FragmentItemsCache::default(),
      item_shapes: ItemShapes::default(),
    }
  }

  /// Whether it has inline content of its own to lay out.
  pub(crate) fn has_own_inline_content(&self) -> bool {
    let font_style = SizedFontStyle::from_style(&self.context.style, &self.context);

    OwnContent::of(self).has_inline_content(&font_style)
  }

  /// About how many layout nodes its subtree builds: one per box, including the out-of-flow
  /// boxes inside inline content, and one per positioned inline span, which may stand in as their
  /// containing block.
  fn layout_node_estimate(&self) -> usize {
    let mut count = 0;
    let mut boxes = vec![self];
    let mut inline = Vec::new();

    while let Some(node) = boxes.pop() {
      count += 1;

      let children = node.children.iter().flatten();

      if node.should_create_inline_layout() {
        inline.extend(children);
      } else {
        boxes.extend(children);
      }

      while let Some(item) = inline.pop() {
        if item.is_out_of_flow() {
          boxes.push(item);
        } else if item.is_inline_span() {
          count += usize::from(item.context.style.base_data.position.is_positioned());
          inline.extend(item.children.iter().flatten());
        }
      }
    }

    count
  }

  /// The fragment items the fragment its scene paints laid out to in the content box of `layout`,
  /// if that fragment laid out there last.
  pub(crate) fn fragment_items_in(&self, layout: ComputedLayout) -> Option<Rc<FragmentItems>> {
    self.fragment_items_of(self.context.fragment_index(), layout)
  }

  /// The fragment items fragment `fragment_index` laid out to in the content box of `layout`, if
  /// that fragment laid out there last.
  pub(crate) fn fragment_items_of(
    &self,
    fragment_index: usize,
    layout: ComputedLayout,
  ) -> Option<Rc<FragmentItems>> {
    self
      .fragment_items
      .0
      .borrow()
      .get(fragment_index)?
      .as_ref()
      .filter(|items| items.content_box == ContentBox::of(layout))
      .map(Rc::clone)
  }

  /// Forgets the fragment items and item shapes it and the boxes under it took, which no longer
  /// hold once the tree may have changed since.
  pub(crate) fn clear_layout_caches(&self) {
    let mut stack = vec![self];

    while let Some(node) = stack.pop() {
      node.fragment_items.0.borrow_mut().clear();
      node.item_shapes.clear();
      stack.extend(node.children.iter().flatten());
    }
  }

  /// An anonymous box laid out with `layout_style` instead of its computed style.
  fn anonymous(
    context: RenderContext,
    node: Option<Node>,
    children: Option<Box<[RenderNode]>>,
    layout_style: Rc<LayoutStyle>,
  ) -> Self {
    let mut anonymous = Self::new(context, NodeOrigin::Anonymous, node, children);

    anonymous.layout_style_override = Some(layout_style);
    anonymous
  }

  pub(super) fn anonymous_text_item(parent_context: &RenderContext, text: String) -> Self {
    Self::text_item(RenderContext::for_anonymous(parent_context), text)
  }

  fn text_item(context: RenderContext, text: String) -> Self {
    let layout_style = block_style(&context);
    let mut item = Self::anonymous(context, None, None, layout_style);

    item.anonymous_text_content = Some(text);
    item.force_inline_layout = true;
    item
  }

  pub(super) fn anonymous_block_container(
    parent_context: &RenderContext,
    children: Vec<RenderNode>,
  ) -> Self {
    let context = RenderContext::for_anonymous(parent_context);
    let layout_style = block_style(&context);

    Self::anonymous(
      context,
      None,
      Some(children.into_boxed_slice()),
      layout_style,
    )
  }

  /// Blink's `LayoutInline::CreateAnonymousContainerForBlockChildren`, which takes the inline box's
  /// direction for the blocks it holds.
  fn block_in_inline(parent_context: &RenderContext, children: Vec<RenderNode>) -> Self {
    let context = RenderContext::for_anonymous(parent_context);
    let layout_style = block_style(&context);
    let mut block = Self::anonymous(
      context,
      None,
      Some(children.into_boxed_slice()),
      layout_style,
    );

    block.is_block_in_inline = true;
    block
  }

  pub(super) fn anonymous_image_item(
    parent_context: &RenderContext,
    image: BackgroundImage,
  ) -> Self {
    // Cap image content to the parent pseudo's box so explicit `width` / `height`
    // on the pseudo wins over intrinsic / default sizing.
    let max_size = Size::all(LayoutLength::Percent(1.0));

    match image {
      BackgroundImage::Url(url) => Self::anonymous(
        RenderContext::for_anonymous(parent_context),
        Some(Node::image(url)),
        None,
        Rc::new(LayoutStyle {
          max_size,
          ..LayoutStyle::default()
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
          Rc::new(LayoutStyle {
            size: Size {
              width: LayoutLength::Fixed(300.0),
              height: LayoutLength::Fixed(150.0),
            },
            max_size,
            ..LayoutStyle::default()
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

  /// The children `items` generate: an image each, and the text between them as one anonymous
  /// text item, as Blink wraps adjacent generated text in one anonymous flex or grid item.
  fn pseudo_content_children(
    originating_node: &Node,
    pseudo_context: &RenderContext,
    items: Box<[ContentItem]>,
  ) -> Box<[Self]> {
    let mut children = Vec::with_capacity(items.len());
    let mut text = String::new();
    let flush = |text: &mut String, children: &mut Vec<Self>| {
      if !text.is_empty() {
        children.push(Self::anonymous_text_item(pseudo_context, take(text)));
      }
    };

    for item in items {
      match item {
        ContentItem::Text(content) => text.push_str(&content),
        ContentItem::Attr(attr) => text.push_str(
          originating_node
            .attribute(&attr.name)
            .unwrap_or(&attr.fallback),
        ),
        ContentItem::Image(image) => {
          flush(&mut text, &mut children);
          children.push(Self::anonymous_image_item(pseudo_context, *image));
        }
      }
    }

    flush(&mut text, &mut children);
    children.into_boxed_slice()
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

    let items = match take(&mut style.misc1_data_mut().content) {
      ContentValue::Items(items) => items,
      _ => return None,
    };

    let mut pseudo_context =
      RenderContext::from_parent(parent_context, style, sizing, current_color);

    let mut children = Self::pseudo_content_children(originating_node, &pseudo_context, items);

    // A flex or grid pseudo-element lays its content out as items, as a table one fixes up its
    // parts.
    if Self::holds_table_parts(&pseudo_context)
      || pseudo_context
        .style
        .base_data
        .display
        .should_blockify_children()
    {
      children = Self::normalize_children(&mut pseudo_context, children, false);
    }

    Some(Self::new(
      pseudo_context,
      NodeOrigin::Pseudo,
      Some(Node::container([])),
      Some(children),
    ))
  }

  /// The marker is the item's first child, as Blink's `::marker` is. An outside marker beside
  /// block content stays a child the item places against its first baseline, and an inside one
  /// takes a line of its own.
  fn attach_marker(&mut self, marker: RenderNode) {
    if self.should_create_inline_layout() {
      self.insert_first_child(marker);
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
      self.insert_first_child(marker);
      return;
    }

    if marker.is_outside_list_marker() {
      self.insert_first_child(marker);
      return;
    }

    // Block-level content an inside marker may not join, so it gets a line of its own.
    let mut line = RenderNode::anonymous_block_container(&self.context, vec![marker]);
    line.force_inline_layout = true;

    self.insert_first_child(line);
  }

  fn insert_first_child(&mut self, child: RenderNode) {
    let mut children = Vec::from(self.children.take().unwrap_or_default());

    children.insert(0, child);
    self.children = Some(children.into_boxed_slice());
  }

  /// The generated `::marker` box, its first child as Blink's `::marker` pseudo-element is.
  pub(crate) fn marker(&self) -> Option<&RenderNode> {
    self
      .children
      .as_deref()?
      .first()
      .filter(|child| child.origin == NodeOrigin::Marker)
  }

  /// Its children after the marker, which its own text comes between.
  pub(crate) fn children_after_marker(&self) -> &[RenderNode] {
    let children = self.children.as_deref().unwrap_or_default();

    &children[usize::from(self.marker().is_some())..]
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

  /// Whether it is a text node without a tag, whose text belongs to its parent element.
  pub(crate) fn is_untagged_text(&self) -> bool {
    self
      .node
      .as_ref()
      .is_some_and(|node| matches!(node.kind, NodeKind::Text(_)) && node.tag_name().is_none())
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
      Display::InlineBlock
        | Display::InlineFlex
        | Display::InlineGrid
        | Display::InlineTable
        | Display::WebkitInlineBox
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

  /// Blink's `IsOriginalDisplayInlineType`: whether the box was inline-level before positioning or
  /// floating blockified it.
  pub(crate) fn is_original_display_inline_type(&self) -> bool {
    self.context.style.original_display.is_inline_level()
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
          | Display::TableCaption
      ) && self.children.as_ref().is_some_and(|children| {
        // `all` first: it stops at a block child, so a block container answers at once.
        children
          .iter()
          .all(RenderNode::participates_in_inline_formatting_context)
          && children
            .iter()
            .any(RenderNode::participates_in_inflow_inline_formatting_context)
      }))
  }

  /// Builds a render tree from a node under the given parent context.
  pub fn from_node(parent_context: &RenderContext, node: Node) -> Self {
    Self::build(parent_context, node, false)
  }

  /// Builds a render tree from `initial_containing_block`, a box that stands in for the initial
  /// containing block: each node it holds is a root element.
  pub fn from_initial_containing_block(
    parent_context: &RenderContext,
    initial_containing_block: Node,
  ) -> Self {
    Self::build(parent_context, initial_containing_block, true)
  }

  fn build(parent_context: &RenderContext, node: Node, blockifies_children: bool) -> Self {
    let matched_styles = match_stylesheets_view(
      &node,
      parent_context.stylesheet(),
      parent_context.sizing.viewport,
    );
    let mut tree =
      Self::from_node_iterative(parent_context, node, &matched_styles, blockifies_children);

    tree.make_root_element();
    if blockifies_children {
      for child in tree.children.iter_mut().flatten() {
        child.make_root_element();
      }
    }
    tree
  }

  /// [CSS Display §2.7](https://drafts.csswg.org/css-display/#transformations): the root element's
  /// display type is always blockified, and its principal box always establishes an independent
  /// formatting context.
  fn make_root_element(&mut self) {
    if self.is_inline_level() {
      self.context.blockify();
    }
    self.is_root_element = true;
  }

  fn from_node_iterative(
    parent_context: &RenderContext,
    root: Node,
    matched_declarations: &[NodeMatchedDeclarations<'_>],
    root_blockifies_children: bool,
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
      true,
    )];

    loop {
      let at_root = stack.len() == 1;
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
          (root_blockifies_children && at_root)
            || current
              .context
              .style
              .base_data
              .display
              .should_blockify_children(),
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

      let Some(parent) = stack.last_mut() else {
        return render_node;
      };

      // Blink's `Element::LayoutObjectIsNeeded`: a `display: none` element gets no box.
      if render_node.context.style.base_data.display != Display::None {
        parent.rendered_children.push(render_node);
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
  /// requires. An inline parent holding a block wraps it as a block-in-inline, unless its own
  /// parent `blockifies` it, which it then does here.
  pub(super) fn normalize_children(
    context: &mut RenderContext,
    children: Box<[RenderNode]>,
    blockifies: bool,
  ) -> Box<[RenderNode]> {
    if RenderNode::holds_table_parts(context) {
      return RenderNode::fix_up_table_children(context, Vec::from(children))
        .unwrap_or_default()
        .into_boxed_slice();
    }

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
    // must not be swept into an anonymous block box, nor floats, which Blink's
    // `LayoutBlockFlow::AddChild` keeps among the block children.
    let has_inline = children
      .iter()
      .any(RenderNode::participates_in_inflow_inline_formatting_context);
    let has_block = children
      .iter()
      .any(|child| !child.participates_in_inline_formatting_context());
    let has_out_of_flow = has_inline
      && children
        .iter()
        .any(|child| child.is_out_of_flow() || child.holds_inline_out_of_flow());
    let parent_is_inline = context.style.base_data.display.is_inline();

    if parent_is_inline && has_block {
      if !blockifies {
        return wrap_runs(
          children,
          |child| !child.participates_in_inline_formatting_context(),
          |run| vec![RenderNode::block_in_inline(context, run)],
        );
      }

      context.blockify();
    }

    // A block parent whose inline content holds out-of-flow boxes wraps that
    // content, placeholders included, so the boxes lay out against a block.
    // An inline parent keeps its inline formatting context untouched — an
    // anonymous block there would be dropped by the surrounding line box.
    if !(has_inline && (has_block || (!parent_is_inline && has_out_of_flow))) {
      return children;
    }

    // Blink's `MakeChildrenNonInline`: an out-of-flow box or float stays in the run of inline
    // content around it, and a run of them alone stays among the block children.
    wrap_runs(
      children,
      RenderNode::participates_in_inline_formatting_context,
      |run| {
        if run
          .iter()
          .any(RenderNode::participates_in_inflow_inline_formatting_context)
        {
          vec![RenderNode::anonymous_block_container(context, run)]
        } else {
          run
        }
      },
    )
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

  /// The child-index path to `node` in this tree, `self` included.
  pub fn path_to(&self, node: &RenderNode) -> Option<Vec<usize>> {
    self.path_where(|candidate| candidate.context.box_id() == node.context.box_id())
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

  /// Whether it measures as the inline content it holds: a text node, or an inline box that tree
  /// building left among block-level siblings, such as in a table row no table wraps, which lays
  /// out as the anonymous block CSS 2.2 §9.2.1.1 would wrap it in.
  pub(crate) fn measures_as_inline_content(&self) -> bool {
    self.node.as_ref().is_some_and(|node| {
      matches!(node.kind, NodeKind::Text(_))
        || self.context.style.base_data.display == Display::Inline
    })
  }
}

/// `children` with each run of the ones `in_run` picks replaced by what `wrap` makes of it.
fn wrap_runs(
  children: Box<[RenderNode]>,
  in_run: impl Fn(&RenderNode) -> bool,
  wrap: impl Fn(Vec<RenderNode>) -> Vec<RenderNode>,
) -> Box<[RenderNode]> {
  let mut wrapped = Vec::new();
  let mut run = Vec::new();

  for child in children {
    if in_run(&child) {
      run.push(child);

      continue;
    }

    if !run.is_empty() {
      wrapped.extend(wrap(take(&mut run)));
    }

    wrapped.push(child);
  }

  if !run.is_empty() {
    wrapped.extend(wrap(run));
  }

  wrapped.into_boxed_slice()
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
  /// Whether its parent blockifies it, as a flex or grid item or the tree's root.
  blockifies: bool,
}

impl PendingRenderNode {
  fn build(
    parent_context: &RenderContext,
    mut node: Node,
    matched_declarations: &[NodeMatchedDeclarations<'_>],
    source_cursor: &mut usize,
    counter: &mut ListCounter,
    inside_list: bool,
    blockifies: bool,
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

    // As Blink's `CanHaveWhitespaceChildren` does, a flex or grid container drops white-space-only
    // text that no text precedes, whatever its `white-space`. The element's own text comes first.
    let drops_own_text =
      context.style.base_data.display.should_blockify_children() && node.is_whitespace_only_text();

    // An element that folded its text hands it back as a child beside its generated content:
    // the inline collector emits its own text before any child, so `::before` could not come
    // first, and a flex or grid container lays the text out as an item only as a child.
    if (pseudo_before_present || pseudo_after.is_some())
      && !drops_own_text
      && let Some(text) = node.take_text()
    {
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
      blockifies,
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

    // Text a table part holds itself goes into an anonymous child, as a run of text would. White
    // space alone stays, for a flex or grid parent to drop.
    if RenderNode::holds_table_parts(&self.context)
      && !self.node.is_whitespace_only_text()
      && let Some(text) = self.node.take_text()
    {
      self
        .rendered_children
        .insert(0, RenderNode::generated_sibling_text(&self.context, text));
      self.children_is_some = true;
    }

    let mut context = self.context;
    let children = if self.children_is_some {
      Some(RenderNode::normalize_children(
        &mut context,
        self.rendered_children.into_boxed_slice(),
        self.blockifies,
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
  /// The fallback list of `style`'s font at `font_size`, shared across the render.
  pub(crate) fn fallback_list(
    &self,
    style: &ComputedStyle,
    font_size: f32,
  ) -> Rc<FontFallbackList> {
    let sizing = SizingContext {
      font_size,
      ..self.sizing.clone()
    };

    self.font_fallback_list(&FontFallbackKey {
      family: self.expand_font_family(&style.inherited_data.font_family),
      selection: style.font_selection_description(&sizing),
      computed_size: font_size,
      specified_size: font_size,
    })
  }

  /// Blink's `ComputedStyle::GetFontHeight` source: the metrics of `style`'s primary font at
  /// `font_size`.
  pub(crate) fn primary_font_metrics(
    &self,
    style: &ComputedStyle,
    font_size: f32,
  ) -> Option<PrimaryFontMetrics> {
    self
      .fallback_list(style, font_size)
      .primary_simple_font_data()?
      .font_metrics()
  }

  /// A child's cascaded style and its element-owned important declarations.
  fn cascade(
    &self,
    node_layers: NodeStyleLayers,
    matched_declarations: &MatchedDeclarationsView<'_>,
  ) -> (NodeStyle, ElementImportant) {
    let mut style = NodeStyle::default();

    if let Some(preset) = node_layers.preset {
      style.append_block(preset.declarations);
    }

    if let Some(dir) = node_layers.dir {
      style.push(StyleDeclaration::direction(dir), false);
    }

    for declarations in matched_declarations.layered_normal() {
      style.merge_matched_block(declarations);
    }

    for declarations in matched_declarations.unlayered_normal() {
      style.merge_matched_block(declarations);
    }

    // An element's own declarations outrank selector-based ones at the same
    // importance.
    let (inline_normal, inline_important) = node_layers
      .inline
      .map(|inline| StyleDeclarationBlock::from(*inline).split_importance())
      .unzip();

    if let Some(inline_normal) = inline_normal {
      style.append_block(inline_normal);
    }

    // Important declarations reverse layer order.
    for declarations in matched_declarations.unlayered_important() {
      style.merge_matched_block(declarations);
    }

    for declarations in matched_declarations.layered_important() {
      style.merge_matched_block(declarations);
    }

    if let Some(inline_important) = &inline_important {
      style.append_block(inline_important.clone());
    }

    (
      style,
      ElementImportant {
        inline: inline_important,
      },
    )
  }

  /// This style as a child inherits it, with the stylesheet's registered custom properties.
  fn inherited_style(&self) -> Cow<'_, ComputedStyle> {
    // Registrations come only from this render's stylesheet, so a style that carries any carries
    // them all, and its descendants take them through `inherited`.
    if self.stylesheet().property_rules().is_empty()
      || self.style.custom_properties.has_registrations()
    {
      return Cow::Borrowed(&self.style);
    }

    self.registered_inherited_style()
  }

  /// [`Self::inherited_style`] for a style the stylesheet's `@property` rules have not reached yet,
  /// out of line so the common path stays small.
  #[inline(never)]
  fn registered_inherited_style(&self) -> Cow<'_, ComputedStyle> {
    let registrations = active_registrations(
      slice::from_ref(self.stylesheet().as_ref()),
      self.sizing.viewport,
    );

    registered_custom_property_parent_style(&self.style, &registrations)
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

  /// Resolves a child's style and sizing from its matched declarations, sharing the style a box
  /// with the same preset resolved to from the same declarations under an equal parent.
  fn resolve_child_style(
    &self,
    node: &mut Node,
    source_order: usize,
    matched_declarations: &[NodeMatchedDeclarations<'_>],
  ) -> (Rc<ComputedStyle>, SizingContext, Color) {
    let default_matched = MatchedDeclarationsView::default();
    let matched = matched_declarations
      .get(source_order)
      .map(NodeMatchedDeclarations::element)
      .unwrap_or(&default_matched);
    // A tree built in code is content, not a document: `rem` resolves against
    // the viewport, so a `font-size` on the outermost node styles text without
    // rescaling every `rem` length below it. A parsed document is the
    // exception, since its outermost node is the `<html>` element that CSS
    // does make the `rem` basis.
    let is_document_root = source_order == 0
      && node
        .tag_name()
        .is_some_and(|tag| tag.eq_ignore_ascii_case("html"));
    let layers = node.take_style_layers();
    let key = (layers.is_preset_only() && !is_document_root)
      .then(|| MatchedStyleKey::new(matched, &self.sizing, self.current_color));
    let cache = self.matched_styles();

    if let Some(key) = &key
      && let Some(resolved) = cache.get(key, &self.style, &self.sizing, layers.preset.as_deref())
    {
      return resolved;
    }

    let preset = key.as_ref().and(layers.preset.clone());
    let (style, sizing, current_color) =
      self.resolve_uncached_child_style(layers, matched, is_document_root);
    let style = Rc::new(style);

    // A style an animation or a query container decided holds for this box alone.
    if let Some(key) = key
      && style.misc1_data.animation_name.is_empty()
      && !sizing.container_read.get()
    {
      let inherits_explicitly = matched.inherits_explicitly()
        || preset
          .as_ref()
          .is_some_and(|preset| preset.declarations.inherits_explicitly());

      cache.insert(
        key,
        Rc::clone(&self.style),
        self.sizing.clone(),
        preset,
        inherits_explicitly,
        (Rc::clone(&style), sizing.clone(), current_color),
      );
    }

    (style, sizing, current_color)
  }

  /// Resolves a child's style and sizing from its style layers and matched declarations.
  fn resolve_uncached_child_style(
    &self,
    layers: NodeStyleLayers,
    matched: &MatchedDeclarationsView<'_>,
    is_document_root: bool,
  ) -> (ComputedStyle, SizingContext, Color) {
    let lang = layers.lang;

    let (style_layers, element_important) = self.cascade(layers, matched);
    let inherited_parent = self.inherited_style();

    let mut style = style_layers.inherit_with_lang(&inherited_parent, lang);

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
    let fallback_list = self.fallback_list(style, font_size);
    let metrics = fallback_list
      .primary_simple_font_data()
      .and_then(|font| font.font_metrics());
    // Blink's `ComputedStyleUtils::ZeroInlineSize`: `ch` reads `PrimaryFontWithDigitZero`.
    let ch = fallback_list
      .primary_simple_font_data_with_digit_zero()
      .and_then(|font| font.font_metrics())
      .map_or(0.0, |metrics| metrics.zero_width);
    let normal_basis = match style.inherited_data.line_height {
      LineHeight::Normal => metrics.map_or(font_size, PrimaryFontMetrics::line_spacing),
      _ => 0.0,
    };
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
      ch,
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

  use super::{
    NodeId, NodeOrigin, RenderNode, active_registrations, registered_custom_property_parent_style,
    sort_children_by_order,
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
    let mut children = vec![NodeId::new(3), NodeId::new(1), NodeId::new(2)];
    sort_children_by_order(&mut children, |child_id| match usize::from(child_id) {
      1 => -1,
      _ => 0,
    });
    assert_eq!(
      children,
      vec![NodeId::new(1), NodeId::new(3), NodeId::new(2)]
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

    let adjusted_parent = registered_custom_property_parent_style(
      &parent,
      &active_registrations(&stylesheets, Viewport::default()),
    );
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

    let adjusted_parent = registered_custom_property_parent_style(
      &parent,
      &active_registrations(&stylesheets, Viewport::default()),
    );
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

    let adjusted_parent = registered_custom_property_parent_style(
      &parent,
      &active_registrations(&stylesheets, Viewport::default()),
    );
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

    let adjusted_parent = registered_custom_property_parent_style(
      &parent,
      &active_registrations(&stylesheets, Viewport::default()),
    );
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

    let adjusted_parent = registered_custom_property_parent_style(
      &parent,
      &active_registrations(&stylesheets, Viewport::default()),
    );
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

    let adjusted_parent = registered_custom_property_parent_style(
      &parent,
      &active_registrations(&stylesheets, Viewport::default()),
    );
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

    let adjusted_parent = registered_custom_property_parent_style(
      &parent,
      &active_registrations(&stylesheets, Viewport::default()),
    );
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
    let adjusted_parent = registered_custom_property_parent_style(
      &parent,
      &active_registrations(&[stylesheet], Viewport::default()),
    );
    let style = Style::default().with(StyleDeclaration::CustomProperty(
      "--box-size".to_owned(),
      "red".to_owned(),
    ));

    let resolved = style.inherit(&adjusted_parent);
    assert_eq!(resolved.custom_properties.get("--box-size"), Some("red"));
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
    let adjusted_parent = registered_custom_property_parent_style(
      &parent,
      &active_registrations(&[stylesheet], Viewport::default()),
    );
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
    let adjusted_parent = registered_custom_property_parent_style(
      &parent,
      &active_registrations(&[stylesheet], Viewport::default()),
    );
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

  #[test]
  fn boxes_under_parents_with_equal_inherited_values_share_a_style() {
    use std::{rc::Rc, sync::Arc};

    use crate::layout::node::Node;

    let stylesheet = StyleSheet::parse(
      ".a { width: 10px } .b { width: 20px } .cell { height: 5px } .inherits { width: inherit }",
    )
    .expect("stylesheet parses");
    let context = RenderContext::builder()
      .fonts(Fonts::default().snapshot())
      .sizing(
        SizingContext::builder()
          .viewport(Viewport::default())
          .build(),
      )
      .stylesheet(Arc::new(stylesheet))
      .build();
    let row = |class: &str| {
      Node::container([
        Node::container([]).with_class_name("cell"),
        Node::container([]).with_class_name("inherits"),
      ])
      .with_class_name(class)
    };
    let tree = RenderNode::from_node(&context, Node::container([row("a"), row("b")]));
    let rows = tree.children.as_deref().expect("rows");
    let first = rows[0].children.as_deref().expect("cells");
    let second = rows[1].children.as_deref().expect("cells");

    assert!(Rc::ptr_eq(
      &first[0].context.style,
      &second[0].context.style
    ));
    assert_eq!(
      first[1].context.style.box_data.width,
      Length::Px(10.0).into()
    );
    assert_eq!(
      second[1].context.style.box_data.width,
      Length::Px(20.0).into()
    );
  }
}
