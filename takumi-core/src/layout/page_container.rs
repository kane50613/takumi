//! Blink's [`PageContainerLayoutAlgorithm`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/page_container_layout_algorithm.cc)
//! and `ResolvePageBoxGeometry`: a page's box model from its `@page` style, its page border box,
//! and its [page-margin boxes](https://drafts.csswg.org/css-page-3/#margin-boxes). In a horizontal
//! writing mode, without fitting a page to a sheet of another size or the safe printable inset.
//! The page area stays in the paged layout of the document, so the boxes come as trees to paint
//! around it.

use crate::{
  context::RenderContext,
  geometry::{AvailableSpace, NodeId, Point, Rect, Size},
  layout::{
    counters::{CountersAttachmentContext, ListItemCounter, PageObject},
    floats_utils::Float,
    layout_style::{LayoutLength, LayoutStyle},
    page_sizes::PageBox,
    tree::{LayoutResults, LayoutTree, RenderNode},
  },
  layout_unit::LayoutUnit,
  style::{
    BoxSizing, ContentItem, ContentValue, Display, JustifyContent, Position, StyleDeclarationBlock,
    VerticalAlign, VerticalAlignKeyword,
    page_rule::{PAGES_COUNTER, PageMarginKind, PageStyles},
  },
};

/// What no rule gives a page-margin box: the properties outside the
/// [ones that apply to it](https://drafts.csswg.org/css-page-3/#margin-property-list).
const MARGIN_BOX_RESET: &str = "display:block;position:static;float:none;transform:none;\
  rotate:none;scale:none;translate:none;";

/// One box of a page container: a tree to lay out in a viewport of `size` and paint at `offset`
/// from the page's top left corner.
pub struct PageFragment {
  /// Where the box's border box starts on the page.
  pub offset: Point<f32>,
  /// The box's border-box size.
  pub size: Size<f32>,
  /// The box, which lays out at `size` whatever space it is given.
  pub root: RenderNode,
}

/// One page's boxes around its page area.
pub struct PageContainer {
  /// The page's box model: its size, and the used margins, borders, and padding around its area.
  pub page_box: PageBox,
  /// The page border box, which paints under the page's content.
  pub border_box: Option<PageFragment>,
  /// The page-margin boxes, which paint over the page's content, in paint order.
  pub margin_boxes: Vec<PageFragment>,
  /// Whether a box reads `counter(pages)` and no page count was given, so the container is to
  /// be laid out again once the document's pages are counted.
  pub needs_page_count: bool,
}

/// Lays out the page containers of a document's pages in order, carrying the counters from one
/// to the next as Blink's `PaginatedRootLayoutAlgorithm` does.
pub struct PageContainers {
  /// The root element's context, which the page context inherits from.
  root: RenderContext,
  counters: CountersAttachmentContext,
}

/// Which side of the page an edge's three boxes run along.
#[derive(Clone, Copy, PartialEq)]
enum Edge {
  Top,
  Right,
  Bottom,
  Left,
}

impl Edge {
  fn is_horizontal(self) -> bool {
    matches!(self, Self::Top | Self::Bottom)
  }

  /// The edge's boxes in Blink's start, center, end order along the page's own axes.
  fn kinds(self) -> [PageMarginKind; 3] {
    match self {
      Self::Top => [
        PageMarginKind::TopLeft,
        PageMarginKind::TopCenter,
        PageMarginKind::TopRight,
      ],
      Self::Right => [
        PageMarginKind::RightTop,
        PageMarginKind::RightMiddle,
        PageMarginKind::RightBottom,
      ],
      Self::Bottom => [
        PageMarginKind::BottomLeft,
        PageMarginKind::BottomCenter,
        PageMarginKind::BottomRight,
      ],
      Self::Left => [
        PageMarginKind::LeftTop,
        PageMarginKind::LeftMiddle,
        PageMarginKind::LeftBottom,
      ],
    }
  }
}

/// The area a box, or an edge's three boxes, may take.
#[derive(Clone, Copy)]
struct Area {
  origin: Point<f32>,
  size: Size<f32>,
}

impl PageBox {
  /// Blink's `LayoutAllMarginBoxes` rects: the area of the corner or edge `kind` lies in.
  fn margin_area(&self, kind: PageMarginKind) -> Area {
    let margin = self.margin.map(|side| side.max(0.0));
    let right = self.size.width - self.margin.right;
    let bottom = self.size.height - self.margin.bottom;
    let across = self.size.width - self.margin.left - self.margin.right;
    let down = self.size.height - self.margin.top - self.margin.bottom;
    let (x, y, width, height) = match kind {
      PageMarginKind::TopLeftCorner => (0.0, 0.0, margin.left, margin.top),
      PageMarginKind::TopRightCorner => (right, 0.0, margin.right, margin.top),
      PageMarginKind::BottomRightCorner => (right, bottom, margin.right, margin.bottom),
      PageMarginKind::BottomLeftCorner => (0.0, bottom, margin.left, margin.bottom),
      PageMarginKind::TopLeft | PageMarginKind::TopCenter | PageMarginKind::TopRight => {
        (self.margin.left, 0.0, across, margin.top)
      }
      PageMarginKind::BottomLeft | PageMarginKind::BottomCenter | PageMarginKind::BottomRight => {
        (self.margin.left, bottom, across, margin.bottom)
      }
      PageMarginKind::RightTop | PageMarginKind::RightMiddle | PageMarginKind::RightBottom => {
        (right, self.margin.top, margin.right, down)
      }
      PageMarginKind::LeftTop | PageMarginKind::LeftMiddle | PageMarginKind::LeftBottom => {
        (0.0, self.margin.top, margin.left, down)
      }
    };

    Area {
      origin: Point::new(x, y),
      size: Size::new(width, height),
    }
  }
}

impl PageMarginKind {
  /// The [alignment](https://drafts.csswg.org/css-page-3/#margin-text-alignment) Blink's UA sheet
  /// gives the box.
  fn default_css(self) -> &'static str {
    match self {
      Self::TopLeftCorner | Self::BottomLeftCorner | Self::TopRight | Self::BottomRight => {
        "text-align:right;vertical-align:middle;"
      }
      Self::TopRightCorner | Self::BottomRightCorner | Self::TopLeft | Self::BottomLeft => {
        "text-align:left;vertical-align:middle;"
      }
      Self::TopCenter | Self::BottomCenter | Self::RightMiddle | Self::LeftMiddle => {
        "text-align:center;vertical-align:middle;"
      }
      Self::RightTop | Self::LeftTop => "text-align:center;vertical-align:top;",
      Self::RightBottom | Self::LeftBottom => "text-align:center;vertical-align:bottom;",
    }
  }

  /// Whether the box touches the page's left edge and its top edge, which decide the margin an
  /// over-constrained box gives up.
  fn at_start_edges(self) -> (bool, bool) {
    let left = matches!(
      self,
      Self::TopLeftCorner
        | Self::BottomLeftCorner
        | Self::LeftTop
        | Self::LeftMiddle
        | Self::LeftBottom
    );
    let top = matches!(
      self,
      Self::TopLeftCorner | Self::TopRightCorner | Self::TopLeft | Self::TopCenter | Self::TopRight
    );

    (left, top)
  }

  fn is_corner(self) -> bool {
    matches!(
      self,
      Self::TopLeftCorner | Self::TopRightCorner | Self::BottomRightCorner | Self::BottomLeftCorner
    )
  }
}

/// The length `length` computes to against `basis`, zero for `auto`.
fn px(length: LayoutLength, basis: f32) -> f32 {
  length
    .resolve_or_zero(Some(LayoutUnit::from_float(basis)))
    .to_float()
}

/// A box's border, padding, and margins against `available`, each side's percentage taking the
/// size on its own axis as Blink's `kContainingBlockSize` has them, with `auto` margins zero.
struct Decorations {
  border_padding: Rect<f32>,
  padding: Rect<f32>,
  margin: Rect<f32>,
}

impl Decorations {
  fn of(style: &LayoutStyle, available: Size<f32>) -> Self {
    let sides = |lengths: Rect<LayoutLength>| Rect {
      top: px(lengths.top, available.height),
      right: px(lengths.right, available.width),
      bottom: px(lengths.bottom, available.height),
      left: px(lengths.left, available.width),
    };
    let padding = sides(style.padding);

    Self {
      border_padding: Rect {
        top: padding.top + style.border.top,
        right: padding.right + style.border.right,
        bottom: padding.bottom + style.border.bottom,
        left: padding.left + style.border.left,
      },
      padding,
      margin: sides(style.margin),
    }
  }
}

/// A box's border-box size on one axis in a space that stretches it: its specified size, or the
/// `available` size its margins leave.
fn border_box_axis(
  style: &LayoutStyle,
  horizontal: bool,
  available: f32,
  margin_sum: f32,
  border_padding_sum: f32,
) -> f32 {
  let (size, min, max) = if horizontal {
    (style.size.width, style.min_size.width, style.max_size.width)
  } else {
    (
      style.size.height,
      style.min_size.height,
      style.max_size.height,
    )
  };
  let basis = Some(LayoutUnit::from_float(available));
  let border_box = |length: LayoutLength| {
    length.resolve(basis).map(|resolved| {
      let resolved = resolved.to_float();

      match style.box_sizing {
        BoxSizing::BorderBox => resolved.max(border_padding_sum),
        BoxSizing::ContentBox => resolved + border_padding_sum,
      }
    })
  };
  let size = border_box(size).unwrap_or(available - margin_sum);
  let size = border_box(max).map_or(size, |max| size.min(max));

  border_box(min)
    .map_or(size, |min| size.max(min))
    .max(border_padding_sum)
}

/// Blink's `ResolveAutoMargins` on one axis: the `auto` ones of `margins` share `additional`.
fn resolve_auto_margins(
  lengths: (LayoutLength, LayoutLength),
  additional: f32,
  margins: (&mut f32, &mut f32),
) {
  match (lengths.0.is_auto(), lengths.1.is_auto()) {
    (true, true) => {
      *margins.0 = additional / 2.0;
      *margins.1 = additional - additional / 2.0;
    }
    (true, false) => *margins.0 = additional,
    (false, true) => *margins.1 = additional,
    (false, false) => {}
  }
}

/// Blink's `ResolveMarginsForPageMarginBox` on one axis: `auto` margins share what the box leaves
/// of `available`, and a box that still does not fill it gives up the margin facing the page's
/// edge.
fn resolve_margin_box_margins(
  lengths: (LayoutLength, LayoutLength),
  border_box: f32,
  available: f32,
  at_start_edge: bool,
  margins: (&mut f32, &mut f32),
) {
  let additional = available - border_box - *margins.0 - *margins.1;
  let (start, end) = margins;

  resolve_auto_margins(lengths, additional.max(0.0), (&mut *start, &mut *end));

  let inequality = available - (border_box + *start + *end);

  if at_start_edge {
    *start += inequality;
  } else {
    *end += inequality;
  }
}

/// Blink's `PreferredSizeInfo`: a box's border-box size range along its edge and its margins there.
#[derive(Clone, Copy, Default)]
struct PreferredSize {
  min: f32,
  max: f32,
  margin_sum: f32,
  is_auto: bool,
}

impl PreferredSize {
  fn min_length(self) -> f32 {
    self.min + self.margin_sum
  }

  fn max_length(self) -> f32 {
    self.max + self.margin_sum
  }

  fn doubled(self) -> Self {
    Self {
      min: self.min * 2.0,
      max: self.max * 2.0,
      margin_sum: self.margin_sum * 2.0,
      is_auto: self.is_auto,
    }
  }
}

/// Blink's `ResolveTwoEdgeMarginBoxLengths`: the margin-box sizes of `sizes[0]` and `sizes[2]`
/// in `available`, around the fixed `sizes[1]`.
fn resolve_two_lengths(sizes: [PreferredSize; 3], available: f32) -> (f32, f32) {
  let mut available_for_flex = available;
  let mut total_auto_min = 0.0;
  let mut total_auto_max = 0.0;

  for size in sizes {
    if size.is_auto {
      total_auto_min += size.min_length();
      total_auto_max += size.max_length();
    } else {
      available_for_flex -= size.min_length();
    }
  }

  let (flex_space, unflexed, factors) = if available_for_flex > total_auto_max {
    let unflexed = sizes.map(PreferredSize::max_length);

    (available_for_flex - total_auto_max, unflexed, unflexed)
  } else {
    let flex_space = available_for_flex - total_auto_min;
    let factors = if flex_space > 0.0 {
      sizes.map(|size| size.max_length() - size.min_length())
    } else {
      sizes.map(PreferredSize::min_length)
    };

    (flex_space, sizes.map(PreferredSize::min_length), factors)
  };
  let mut first = unflexed[0];
  let mut second = unflexed[2];

  if sizes[0].is_auto {
    if sizes[2].is_auto {
      let total = factors[0] + factors[2];

      if total > 0.0 {
        first += flex_space * factors[0] / total;
      }
    } else {
      first = available - second;
    }
  }

  if sizes[2].is_auto {
    second = available - first;
  }

  (first, second)
}

/// Blink's `CalculateEdgeMarginBoxSizes`: the border-box sizes of an edge's boxes along it.
fn edge_sizes(mut sizes: [Option<PreferredSize>; 3], available: f32) -> [f32; 3] {
  let autos = || sizes.into_iter().flatten().filter(|size| size.is_auto);

  if autos().next().is_some() && autos().map(PreferredSize::max_length).sum::<f32>() == 0.0 {
    for size in sizes.iter_mut().flatten().filter(|size| size.is_auto) {
      *size = PreferredSize {
        min: 1.0,
        max: 1.0,
        margin_sum: 0.0,
        is_auto: true,
      };
    }
  }

  let preferred = sizes.map(Option::unwrap_or_default);
  let mut lengths = preferred.map(PreferredSize::max_length);

  if sizes[1].is_some() {
    if preferred[1].is_auto {
      let center_for = |side: PreferredSize| {
        resolve_two_lengths(
          [preferred[1], PreferredSize::default(), side.doubled()],
          available,
        )
        .0
      };

      lengths[1] = center_for(preferred[0]).min(center_for(preferred[2]));
    }

    let side_space = available - lengths[1];

    if preferred[0].is_auto {
      lengths[0] = side_space / 2.0;
    }

    if preferred[2].is_auto {
      lengths[2] = side_space - side_space / 2.0;
    }
  } else {
    (lengths[0], lengths[2]) = resolve_two_lengths(preferred, available);
  }

  [0, 1, 2].map(|index| (lengths[index] - preferred[index].margin_sum).max(0.0))
}

/// A page-margin box that generates content, before it is sized.
struct MarginBox {
  kind: PageMarginKind,
  context: RenderContext,
  /// How its style lays it out, as a block that aligns its content as a table cell does.
  style: LayoutStyle,
  area: Area,
  decorations: Decorations,
}

impl MarginBox {
  /// The box laid out as `style`, with the content its `content` generates.
  fn node(&self, style: LayoutStyle, counters: &CountersAttachmentContext) -> RenderNode {
    let children =
      RenderNode::page_margin_content(&self.context, &mut counters.clone()).unwrap_or_default();

    RenderNode::page_box(self.context.clone(), children, style)
  }

  /// `self.style` with its box model resolved: a border box of `size`, no margins, and its
  /// padding in px.
  fn sized(&self, size: Size<LayoutLength>) -> LayoutStyle {
    LayoutStyle {
      size,
      min_size: Size::new(LayoutLength::Auto, LayoutLength::Auto),
      max_size: Size::new(LayoutLength::Auto, LayoutLength::Auto),
      box_sizing: BoxSizing::BorderBox,
      margin: Rect::default(),
      padding: self.decorations.padding.map(LayoutLength::Fixed),
      ..self.style.clone()
    }
  }

  /// The box's border-box size across `horizontal`, in a space that stretches it.
  fn stretched(&self, horizontal: bool) -> f32 {
    let Decorations {
      border_padding,
      margin,
      ..
    } = &self.decorations;

    if horizontal {
      border_box_axis(
        &self.style,
        true,
        self.area.size.width,
        margin.left + margin.right,
        border_padding.left + border_padding.right,
      )
    } else {
      border_box_axis(
        &self.style,
        false,
        self.area.size.height,
        margin.top + margin.bottom,
        border_padding.top + border_padding.bottom,
      )
    }
  }

  /// Blink's `EdgeMarginNodePreferredSize` along `edge`.
  fn preferred_size(&self, edge: Edge, counters: &CountersAttachmentContext) -> PreferredSize {
    let margin = &self.decorations.margin;

    if edge.is_horizontal() {
      let is_auto = self.style.size.width.is_auto();
      let (min, max) = if is_auto {
        let node = self.node(
          self.sized(Size::new(LayoutLength::Auto, self.style.size.height)),
          counters,
        );

        LayoutTree::from_render_node(&node)
          .root_min_max_widths(AvailableSpace::Definite(self.area.size.height))
      } else {
        let width = self.stretched(true);

        (width, width)
      };

      return PreferredSize {
        min,
        max,
        margin_sum: margin.left + margin.right,
        is_auto,
      };
    }

    let is_auto = self.style.size.height.is_auto();
    let height = if is_auto {
      let node = self.node(
        self.sized(Size::new(
          LayoutLength::Fixed(self.stretched(true)),
          LayoutLength::Auto,
        )),
        counters,
      );
      let results = LayoutResults::compute_for_paint(
        &node,
        Size::new(
          AvailableSpace::Definite(self.area.size.width),
          AvailableSpace::Definite(self.area.size.height),
        ),
      );

      results
        .layout(NodeId::ROOT)
        .map_or(0.0, |layout| layout.size.height)
    } else {
      self.stretched(false)
    };

    PreferredSize {
      min: height,
      max: height,
      margin_sum: margin.top + margin.bottom,
      is_auto,
    }
  }

  /// Blink's `ResolveMargins` on an axis where the box lies along the page's edge: the box's
  /// border-box size there and how far its margins put it into its area.
  fn across(&self, horizontal: bool) -> (f32, f32) {
    let size = self.stretched(horizontal);
    let margin = &self.decorations.margin;
    let (available, lengths, mut start, mut end, at_start) = if horizontal {
      (
        self.area.size.width,
        (self.style.margin.left, self.style.margin.right),
        margin.left,
        margin.right,
        self.kind.at_start_edges().0,
      )
    } else {
      (
        self.area.size.height,
        (self.style.margin.top, self.style.margin.bottom),
        margin.top,
        margin.bottom,
        self.kind.at_start_edges().1,
      )
    };

    resolve_margin_box_margins(lengths, size, available, at_start, (&mut start, &mut end));
    (size, start)
  }

  /// The box at `offset` in its area with a border box of `size`.
  fn fragment(
    &self,
    offset: Point<f32>,
    size: Size<f32>,
    counters: &CountersAttachmentContext,
  ) -> PageFragment {
    PageFragment {
      offset: self.area.origin + offset,
      size,
      root: self.node(self.sized(size.map(LayoutLength::Fixed)), counters),
    }
  }
}

impl PageContainers {
  /// The containers of a document whose root element's context is `root`.
  pub fn new(root: RenderContext) -> Self {
    let mut counters = CountersAttachmentContext::default();

    // Page boxes inherit from the document root element.
    counters.enter_object(&root.style, ListItemCounter::None);

    Self { root, counters }
  }

  /// Blink's `PageContainerLayoutAlgorithm::Layout` for the next page: its boxes as `styles`
  /// style them over `base`, the page's size and the margins no rule sets. `page_count` is what
  /// `counter(pages)` reads.
  pub fn lay_out_next(
    &mut self,
    styles: &PageStyles,
    base: PageBox,
    page_count: Option<usize>,
  ) -> PageContainer {
    let context = Self::page_context(&self.root, styles, &base);
    let style = LayoutStyle::from_computed(&context.style, &context.sizing, false);
    let (page_box, border_box_size) = Self::resolve_page_box_geometry(&style, base);

    self.counters.page_count = page_count.map(|count| count as i32);
    self
      .counters
      .enter_page_object(&context.style, PageObject::PageBox);

    let border_box = (!styles.page.declarations.is_empty()).then(|| PageFragment {
      offset: Point::new(page_box.margin.left, page_box.margin.top),
      size: border_box_size,
      root: RenderNode::page_box(
        context.clone(),
        Box::default(),
        LayoutStyle {
          display: Display::FlowRoot,
          position: Position::Static,
          float: Float::None,
          size: border_box_size.map(LayoutLength::Fixed),
          min_size: Size::new(LayoutLength::Auto, LayoutLength::Auto),
          max_size: Size::new(LayoutLength::Auto, LayoutLength::Auto),
          box_sizing: BoxSizing::BorderBox,
          margin: Rect::default(),
          padding: page_box.padding.map(LayoutLength::Fixed),
          ..style
        },
      ),
    });
    let (margin_boxes, needs_page_count) =
      self.lay_out_all_margin_boxes(&context, styles, &page_box, page_count);

    self
      .counters
      .leave_page_object(&context.style, PageObject::PageBox);

    PageContainer {
      page_box,
      border_box,
      margin_boxes,
      needs_page_count,
    }
  }

  /// Blink's `StyleForPage`: the page context `styles` give a page under `root`, over the margins
  /// of `base`, which stand in for the ones Blink takes from the print settings.
  fn page_context(root: &RenderContext, styles: &PageStyles, base: &PageBox) -> RenderContext {
    let mut declarations = StyleDeclarationBlock::parse_loosy(&format!(
      "margin:{}px {}px {}px {}px",
      base.margin.top, base.margin.right, base.margin.bottom, base.margin.left
    ));

    for declaration in styles.page.iter() {
      declarations.push(declaration.clone(), false);
    }

    root.for_page_box(declarations)
  }

  /// The box of a page `styles` style under `root` over `base`, its size and the margins no rule
  /// sets.
  pub fn page_box(root: &RenderContext, styles: &PageStyles, base: PageBox) -> PageBox {
    let context = Self::page_context(root, styles, &base);
    let style = LayoutStyle::from_computed(&context.style, &context.sizing, false);

    Self::resolve_page_box_geometry(&style, base).0
  }

  /// Blink's `ResolvePageBoxGeometry`: the page's box model as `style` gives it in `base`'s size,
  /// and its border-box size. A style that leaves less than a pixel of page area on either axis is
  /// ignored, as Blink's `LayoutPageContainer` ignores one that leaves none.
  fn resolve_page_box_geometry(style: &LayoutStyle, base: PageBox) -> (PageBox, Size<f32>) {
    let decorations = Decorations::of(style, base.size);
    let mut margin = decorations.margin;
    let border_padding = decorations.border_padding;
    let border_box = Size::new(
      border_box_axis(
        style,
        true,
        base.size.width,
        margin.left + margin.right,
        border_padding.left + border_padding.right,
      ),
      border_box_axis(
        style,
        false,
        base.size.height,
        margin.top + margin.bottom,
        border_padding.top + border_padding.bottom,
      ),
    );

    // This may result in negative margins, if the specified width or height is larger than the
    // page. <https://github.com/w3c/csswg-drafts/issues/8508>
    resolve_auto_margins(
      (style.margin.left, style.margin.right),
      base.size.width - (border_box.width + margin.left + margin.right),
      (&mut margin.left, &mut margin.right),
    );
    resolve_auto_margins(
      (style.margin.top, style.margin.bottom),
      base.size.height - (border_box.height + margin.top + margin.bottom),
      (&mut margin.top, &mut margin.bottom),
    );

    // The page is as large as its margin box: Blink sizes a PDF's pages from the `@page`
    // properties alone, so a border box and margins that do not add up to `size` change it.
    let page_box = PageBox {
      size: Size::new(
        border_box.width + margin.left + margin.right,
        border_box.height + margin.top + margin.bottom,
      ),
      margin,
      border: style.border,
      padding: decorations.padding,
    };
    let area = page_box.area();

    if area.width >= 1.0 && area.height >= 1.0 {
      (page_box, border_box)
    } else {
      (base, base.size.inset(base.margin))
    }
  }

  /// Blink's `LayoutAllMarginBoxes`: the page-margin boxes that generate content, in paint order,
  /// and whether one reads a page count that was not given.
  fn lay_out_all_margin_boxes(
    &mut self,
    page_context: &RenderContext,
    styles: &PageStyles,
    page_box: &PageBox,
    page_count: Option<usize>,
  ) -> (Vec<PageFragment>, bool) {
    let mut needs_page_count = false;
    let boxes: Vec<MarginBox> = styles
      .margins
      .iter()
      .filter_map(|(kind, rule)| {
        let mut declarations = StyleDeclarationBlock::parse_loosy(kind.default_css());

        for declaration in rule
          .iter()
          .chain(StyleDeclarationBlock::parse_loosy(MARGIN_BOX_RESET).iter())
        {
          declarations.push(declaration.clone(), false);
        }

        let context = page_context.for_page_box(declarations);
        let ContentValue::Items(items) = &context.style.misc1_data.content else {
          return None;
        };

        needs_page_count |= page_count.is_none()
          && items.iter().any(
            |item| matches!(item, ContentItem::Counter(counter) if &*counter.name == PAGES_COUNTER),
          );

        let area = page_box.margin_area(*kind);
        let style = LayoutStyle::from_computed(&context.style, &context.sizing, false);
        let align_content = match context.style.base_data.vertical_align {
          VerticalAlign::Keyword(VerticalAlignKeyword::Middle) => JustifyContent::Center,
          VerticalAlign::Keyword(VerticalAlignKeyword::Bottom) => JustifyContent::End,
          _ => JustifyContent::Start,
        };

        Some(MarginBox {
          kind: *kind,
          decorations: Decorations::of(&style, area.size),
          style: LayoutStyle {
            display: Display::FlowRoot,
            position: Position::Static,
            float: Float::None,
            align_content,
            ..style
          },
          context,
          area,
        })
      })
      .collect();
    let counters = &self.counters;
    let mut fragments: Vec<(PageMarginKind, PageFragment)> = Vec::with_capacity(boxes.len());

    for margin_box in boxes
      .iter()
      .filter(|margin_box| margin_box.kind.is_corner())
    {
      let (width, left) = margin_box.across(true);
      let (height, top) = margin_box.across(false);

      fragments.push((
        margin_box.kind,
        margin_box.fragment(Point::new(left, top), Size::new(width, height), counters),
      ));
    }

    for edge in [Edge::Top, Edge::Right, Edge::Bottom, Edge::Left] {
      let members = edge
        .kinds()
        .map(|kind| boxes.iter().find(|margin_box| margin_box.kind == kind));
      let Some(area) = members.iter().flatten().next().map(|member| member.area) else {
        continue;
      };
      let available = if edge.is_horizontal() {
        area.size.width
      } else {
        area.size.height
      };
      let preferred =
        members.map(|member| member.map(|member| member.preferred_size(edge, counters)));
      let sizes = edge_sizes(preferred, available);

      for (index, margin_box) in members.into_iter().enumerate() {
        let (Some(margin_box), Some(preferred)) = (margin_box, preferred[index]) else {
          continue;
        };
        let margin = &margin_box.decorations.margin;
        let outer = sizes[index] + preferred.margin_sum;
        let along = match index {
          0 => 0.0,
          1 => (available - outer) / 2.0,
          _ => available - outer,
        };
        let (cross_size, cross_offset) = margin_box.across(!edge.is_horizontal());
        let (offset, size) = if edge.is_horizontal() {
          (
            Point::new(along + margin.left, cross_offset),
            Size::new(sizes[index], cross_size),
          )
        } else {
          (
            Point::new(cross_offset, along + margin.top),
            Size::new(cross_size, sizes[index]),
          )
        };

        fragments.push((margin_box.kind, margin_box.fragment(offset, size, counters)));
      }
    }

    // Blink's paint order: clockwise from the top left corner.
    fragments.sort_by_key(|&(kind, _)| PageMarginKind::ALL.iter().position(|&other| other == kind));

    (
      fragments
        .into_iter()
        .map(|(_, fragment)| fragment)
        .collect(),
      needs_page_count,
    )
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn auto(min: f32, max: f32, margin_sum: f32) -> Option<PreferredSize> {
    Some(PreferredSize {
      min,
      max,
      margin_sum,
      is_auto: true,
    })
  }

  fn fixed(size: f32) -> Option<PreferredSize> {
    Some(PreferredSize {
      min: size,
      max: size,
      margin_sum: 0.0,
      is_auto: false,
    })
  }

  /// WPT `css/css-page/margin-boxes/dimensions-003-print.html`: the space two `auto` boxes leave
  /// goes to each by its max-content size, margins included.
  #[test]
  fn two_auto_boxes_share_the_edge_by_their_max_content_sizes() {
    assert_eq!(
      edge_sizes(
        [auto(32.0, 32.0, 16.0), None, auto(16.0, 16.0, 16.0)],
        320.0
      ),
      [176.0, 0.0, 112.0]
    );
  }

  #[test]
  fn an_auto_center_box_stays_centered_between_unequal_sides() {
    let sizes = edge_sizes(
      [
        auto(100.0, 100.0, 0.0),
        auto(50.0, 50.0, 0.0),
        auto(20.0, 20.0, 0.0),
      ],
      400.0,
    );

    assert_eq!(sizes[0], sizes[2]);
    assert_eq!(sizes.iter().sum::<f32>(), 400.0);
    assert_eq!(sizes[1], 80.0);
  }

  #[test]
  fn a_lone_auto_box_takes_the_edge_and_empty_ones_split_it() {
    assert_eq!(
      edge_sizes([auto(0.0, 0.0, 0.0), None, None], 200.0),
      [200.0, 0.0, 0.0]
    );
    assert_eq!(
      edge_sizes([auto(0.0, 0.0, 0.0), None, auto(0.0, 0.0, 0.0)], 200.0),
      [100.0, 0.0, 100.0]
    );
    assert_eq!(
      edge_sizes([fixed(60.0), None, auto(10.0, 30.0, 0.0)], 200.0),
      [60.0, 0.0, 140.0]
    );
  }

  #[test]
  fn each_box_lies_in_the_margin_its_name_says() {
    let page_box = PageBox {
      size: Size::new(400.0, 300.0),
      margin: Rect {
        top: 10.0,
        right: 20.0,
        bottom: 30.0,
        left: 40.0,
      },
      ..PageBox::default()
    };
    let area = |kind| {
      let Area { origin, size } = page_box.margin_area(kind);

      (origin.x, origin.y, size.width, size.height)
    };

    assert_eq!(area(PageMarginKind::TopLeftCorner), (0.0, 0.0, 40.0, 10.0));
    assert_eq!(area(PageMarginKind::TopCenter), (40.0, 0.0, 340.0, 10.0));
    assert_eq!(
      area(PageMarginKind::RightMiddle),
      (380.0, 10.0, 20.0, 260.0)
    );
    assert_eq!(
      area(PageMarginKind::BottomRightCorner),
      (380.0, 270.0, 20.0, 30.0)
    );
    assert_eq!(area(PageMarginKind::LeftBottom), (0.0, 10.0, 40.0, 260.0));
  }

  #[test]
  fn auto_margins_take_what_a_box_leaves_and_an_over_constrained_box_leaves_the_edge() {
    let auto = LayoutLength::Auto;
    let zero = LayoutLength::Fixed(0.0);
    let resolve = |lengths, at_start_edge| {
      let (mut start, mut end) = (0.0, 0.0);

      resolve_margin_box_margins(lengths, 20.0, 100.0, at_start_edge, (&mut start, &mut end));
      (start, end)
    };

    assert_eq!(resolve((auto, auto), true), (40.0, 40.0));
    assert_eq!(resolve((auto, zero), true), (80.0, 0.0));
    assert_eq!(resolve((zero, zero), true), (80.0, 0.0));
    assert_eq!(resolve((zero, zero), false), (0.0, 80.0));
  }
}
