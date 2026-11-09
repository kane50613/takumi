//! A page's [page-margin boxes](https://drafts.csswg.org/css-page-3/#margin-boxes), sized and
//! placed as Blink's [`PageContainerLayoutAlgorithm`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/page_container_layout_algorithm.cc)
//! places them, and given as one tree to lay out over the page. In a horizontal writing mode. A
//! `calc()` margin on a box's variable axis counts as zero, a counter other than `page` and `pages`
//! reads zero, and the page context paints no background or border of its own.

use std::fmt::Write;

use crate::{
  context::RenderContext,
  geometry::{AvailableSpace, NodeId, Size},
  layout::{
    layout_style::{LayoutLength, LayoutStyle},
    node::Node,
    page_sizes::{PageBox, PageRules},
    tree::{LayoutResults, RenderNode},
  },
  style::{
    BackgroundImage, ContentItem, ContentValue, CounterChange, Direction, PageName, Style,
    StyleDeclarationBlock, VerticalAlign, VerticalAlignKeyword,
    page_rule::{PageKind, PageMarginKind, PageStyles},
  },
};

/// The counter CSS Paged Media §7.1 increments on every page.
const PAGE_COUNTER: &str = "page";
/// The counter Blink sets to the document's page count.
const PAGES_COUNTER: &str = "pages";

/// What the page context must not paint or lay out over the page it only passes inheritance on.
const PAGE_CONTEXT_RESET: &str = "margin:0;padding:0;border:0;background:none;outline:none;\
  box-shadow:none;opacity:1;transform:none;filter:none;overflow:visible;display:block;\
  position:relative;";

/// What a page-margin box takes from no rule: the properties outside the
/// [ones that apply to it](https://drafts.csswg.org/css-page-3/#margin-property-list).
const MARGIN_BOX_RESET: &str = "display:flow-root;float:none;clear:none;transform:none;\
  rotate:none;scale:none;translate:none;";

/// The values of the counters a page's margin boxes read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageCounters {
  /// `counter(page)`, which [`PageStyles::advance_page_counter`] moves from page to page.
  pub page: i32,
  /// `counter(pages)`: the document's page count, once known.
  pub pages: Option<usize>,
}

/// A page's page-margin boxes as one tree, to lay out in a viewport of the page's size.
pub struct PageMarginBoxes {
  /// The tree: a box of the page's size that holds each page-margin box at its place.
  pub node: Node,
  /// Whether a box reads `counter(pages)` and no page count was given, so the tree is to be built
  /// again once the document's pages are counted.
  pub needs_page_count: bool,
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

/// A span along one axis of the page.
#[derive(Clone, Copy)]
struct Span {
  start: f32,
  size: f32,
}

/// The area a box, or an edge's three boxes, may take.
#[derive(Clone, Copy)]
struct Area {
  x: Span,
  y: Span,
}

impl PageBox {
  /// Blink's `LayoutAllMarginBoxes` rects: the area of the corner or edge `kind` lies in.
  fn margin_area(&self, kind: PageMarginKind) -> Area {
    let margin = self.margin.map(|side| side.max(0.0));
    let left = Span {
      start: 0.0,
      size: margin.left,
    };
    let right = Span {
      start: self.size.width - self.margin.right,
      size: margin.right,
    };
    let top = Span {
      start: 0.0,
      size: margin.top,
    };
    let bottom = Span {
      start: self.size.height - self.margin.bottom,
      size: margin.bottom,
    };
    let across = Span {
      start: self.margin.left,
      size: self.size.width - self.margin.left - self.margin.right,
    };
    let down = Span {
      start: self.margin.top,
      size: self.size.height - self.margin.top - self.margin.bottom,
    };
    let (x, y) = match kind {
      PageMarginKind::TopLeftCorner => (left, top),
      PageMarginKind::TopRightCorner => (right, top),
      PageMarginKind::BottomRightCorner => (right, bottom),
      PageMarginKind::BottomLeftCorner => (left, bottom),
      PageMarginKind::TopLeft | PageMarginKind::TopCenter | PageMarginKind::TopRight => {
        (across, top)
      }
      PageMarginKind::BottomLeft | PageMarginKind::BottomCenter | PageMarginKind::BottomRight => {
        (across, bottom)
      }
      PageMarginKind::RightTop | PageMarginKind::RightMiddle | PageMarginKind::RightBottom => {
        (right, down)
      }
      PageMarginKind::LeftTop | PageMarginKind::LeftMiddle | PageMarginKind::LeftBottom => {
        (left, down)
      }
    };

    Area { x, y }
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

  /// Whether the box touches the page's left edge, and its top edge, which decide the margin an
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
}

/// A page-margin box that generates content.
struct MarginBox {
  kind: PageMarginKind,
  css: String,
  children: Vec<Node>,
  style: LayoutStyle,
  /// The `align-content` its `vertical-align` stands for, as a table cell's does.
  alignment: &'static str,
}

impl MarginBox {
  fn node(&self, extra_css: &str) -> Node {
    Node::container(self.children.clone()).with_style(style(&[
      self.kind.default_css(),
      &self.css,
      MARGIN_BOX_RESET,
      extra_css,
    ]))
  }
}

fn style(css: &[&str]) -> Style {
  Style::from(StyleDeclarationBlock::parse_loosy(&css.concat()))
}

/// The length `length` computes to against `basis`, `None` for `auto`.
fn resolve(length: &LayoutLength, basis: f32) -> Option<f32> {
  match length {
    LayoutLength::Auto => None,
    LayoutLength::Fixed(px) => Some(*px),
    LayoutLength::Percent(fraction) => Some(fraction * basis),
    _ => Some(0.0),
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

impl PageStyles {
  /// The `page` counter on the page these style, given its value on the page before, or 0 before
  /// the first: the page context's `counter-reset`, then its `counter-increment`, which is 1
  /// where the context sets none, as CSS Paged Media §7.1 has it.
  pub fn advance_page_counter(&self, context: &RenderContext, counter: i32) -> i32 {
    if !self.page.contains("counter-") {
      return counter.saturating_add(1);
    }

    let probe = RenderNode::from_node(
      context,
      Node::container([]).with_style(style(&[&self.page, PAGE_CONTEXT_RESET])),
    );
    let changes = &probe.context.style.misc1_data;
    let of_page = |change: &&CounterChange| &*change.name == PAGE_COUNTER;
    let increments: Vec<i32> = changes
      .counter_increment
      .iter()
      .filter(of_page)
      .map(|change| change.value)
      .collect();
    let increment = if increments.is_empty() {
      1
    } else {
      increments.into_iter().sum()
    };

    changes
      .counter_reset
      .iter()
      .filter(of_page)
      .last()
      .map_or(counter, |change| change.value)
      .saturating_add(increment)
  }
}

impl PageRules {
  /// The declarations that style page `page`'s boxes as a page `name`.
  pub fn page_styles(&self, page: usize, name: &PageName) -> PageStyles {
    self.stylesheet.page_styles(
      self.viewport,
      PageKind::of(page, self.context.direction()),
      name,
      &self.context,
    )
  }
}

impl PageMarginBoxes {
  /// The page-margin boxes `styles` give a page of `page_box` whose root's text runs in
  /// `direction`, `None` when no rule gives one content.
  pub fn lay_out(
    context: &RenderContext,
    styles: &PageStyles,
    page_box: PageBox,
    direction: Direction,
    counters: PageCounters,
  ) -> Option<Self> {
    if styles.margins.is_empty() {
      return None;
    }

    let page_css = [styles.page.as_str(), PAGE_CONTEXT_RESET].concat();
    let probe = RenderNode::from_node(
      context,
      Node::container(
        styles
          .margins
          .iter()
          .map(|(kind, css)| {
            Node::container([]).with_style(style(&[kind.default_css(), css, MARGIN_BOX_RESET]))
          })
          .collect::<Vec<_>>(),
      )
      .with_style(style(&[&page_css])),
    );
    let mut needs_page_count = false;
    let boxes: Vec<MarginBox> = styles
      .margins
      .iter()
      .zip(probe.children.iter().flatten())
      .filter_map(|((kind, css), probed)| {
        let ContentValue::Items(items) = &probed.context.style.misc1_data.content else {
          return None;
        };
        let mut children = Vec::new();
        let mut text = String::new();

        for item in items {
          match item {
            ContentItem::Text(string) => text.push_str(string),
            ContentItem::Counter(counter) => {
              let value = match &*counter.name {
                PAGE_COUNTER => counters.page,
                PAGES_COUNTER => {
                  needs_page_count |= counters.pages.is_none();
                  counters.pages.unwrap_or_default() as i32
                }
                _ => 0,
              };

              text.push_str(&counter.text(&[value], direction));
            }
            ContentItem::Image(image) => {
              if let BackgroundImage::Url(url) = &**image {
                if !text.is_empty() {
                  children.push(Node::text(std::mem::take(&mut text)));
                }

                children.push(Node::image(&**url));
              }
            }
            ContentItem::Attr(_) => {}
          }
        }

        if !text.is_empty() {
          children.push(Node::text(text));
        }

        let alignment = match probed.context.style.base_data.vertical_align {
          VerticalAlign::Keyword(VerticalAlignKeyword::Middle) => "center",
          VerticalAlign::Keyword(VerticalAlignKeyword::Bottom) => "end",
          _ => "start",
        };

        Some(MarginBox {
          kind: *kind,
          css: css.clone(),
          children,
          style: probed.layout_box_style(&probed.context.sizing),
          alignment,
        })
      })
      .collect();

    if boxes.is_empty() {
      return None;
    }

    let mut placed: Vec<(PageMarginKind, Node)> = Vec::with_capacity(boxes.len());

    for edge in [Edge::Top, Edge::Right, Edge::Bottom, Edge::Left] {
      let kinds = edge.kinds();
      let members = kinds.map(|kind| boxes.iter().find(|margin_box| margin_box.kind == kind));

      if members.iter().all(Option::is_none) {
        continue;
      }

      let area = page_box.margin_area(kinds[0]);
      let main = if edge.is_horizontal() { area.x } else { area.y };
      let preferred = members.map(|member| {
        member.map(|margin_box| {
          margin_box.preferred_size(
            context,
            &page_css,
            edge,
            Size::new(area.x.size, area.y.size),
          )
        })
      });
      let sizes = edge_sizes(preferred, main.size);

      for (index, margin_box) in members.into_iter().enumerate() {
        let (Some(margin_box), Some(preferred)) = (margin_box, preferred[index]) else {
          continue;
        };
        let margins = if edge.is_horizontal() {
          (
            &margin_box.style.margin.left,
            &margin_box.style.margin.right,
          )
        } else {
          (
            &margin_box.style.margin.top,
            &margin_box.style.margin.bottom,
          )
        };
        let margin_start = resolve(margins.0, area.x.size).unwrap_or_default();
        let outer = sizes[index] + preferred.margin_sum;
        let offset = match index {
          0 => 0.0,
          1 => (main.size - outer) / 2.0,
          _ => main.size - outer,
        };
        let start = offset + margin_start;
        let mut css = String::new();
        let (near, far, size, margin_near, margin_far) = if edge.is_horizontal() {
          ("left", "right", "width", "margin-left", "margin-right")
        } else {
          ("top", "bottom", "height", "margin-top", "margin-bottom")
        };
        let _ = write!(
          css,
          "{near}:{start}px;{far}:{}px;{size}:auto;{margin_near}:0;{margin_far}:0;",
          main.size - start - sizes[index],
        );

        margin_box.write_cross_axis(&mut css, !edge.is_horizontal());
        placed.push((margin_box.kind, margin_box.placed(area, &css)));
      }
    }

    for margin_box in &boxes {
      if !matches!(
        margin_box.kind,
        PageMarginKind::TopLeftCorner
          | PageMarginKind::TopRightCorner
          | PageMarginKind::BottomRightCorner
          | PageMarginKind::BottomLeftCorner
      ) {
        continue;
      }

      let area = page_box.margin_area(margin_box.kind);
      let mut css = String::new();

      margin_box.write_cross_axis(&mut css, true);
      margin_box.write_cross_axis(&mut css, false);
      placed.push((margin_box.kind, margin_box.placed(area, &css)));
    }

    // Blink's paint order: clockwise from the top left corner.
    placed.sort_by_key(|&(kind, _)| PageMarginKind::ALL.iter().position(|&other| other == kind));

    let node = Node::container(placed.into_iter().map(|(_, node)| node).collect::<Vec<_>>())
      .with_style(style(&[
        &page_css,
        &format!(
          "width:{}px;height:{}px;",
          page_box.size.width, page_box.size.height
        ),
      ]));

    Some(Self {
      node,
      needs_page_count,
    })
  }
}

impl MarginBox {
  /// The box placed by `geometry` in `area`, which its percentages resolve against.
  fn placed(&self, area: Area, geometry: &str) -> Node {
    let holder = format!(
      "position:absolute;left:{}px;top:{}px;width:{}px;height:{}px;",
      area.x.start,
      area.y.start,
      area.x.size.max(0.0),
      area.y.size.max(0.0)
    );
    let css = format!(
      "position:absolute;align-content:{};{geometry}",
      self.alignment
    );

    Node::container([self.node(&css)]).with_style(style(&[&holder]))
  }

  /// Blink's `ResolveMargins` along an axis the box does not share: it fills its area, or holds
  /// its own size there with its `auto` margins taking the rest. A box with neither gives up the
  /// margin on the page's edge.
  fn write_cross_axis(&self, css: &mut String, horizontal: bool) {
    let (near, far, size, margins, at_start) = if horizontal {
      (
        "left",
        "right",
        &self.style.size.width,
        (&self.style.margin.left, &self.style.margin.right),
        self.kind.at_start_edges().0,
      )
    } else {
      (
        "top",
        "bottom",
        &self.style.size.height,
        (&self.style.margin.top, &self.style.margin.bottom),
        self.kind.at_start_edges().1,
      )
    };
    let over_constrained = !size.is_auto() && !margins.0.is_auto() && !margins.1.is_auto();
    let near_inset = if over_constrained && at_start {
      "auto"
    } else {
      "0"
    };
    let _ = write!(css, "{near}:{near_inset};{far}:0;");
  }

  /// Blink's `EdgeMarginNodePreferredSize` in the `area` of `edge`.
  fn preferred_size(
    &self,
    context: &RenderContext,
    page_css: &str,
    edge: Edge,
    area: Size<f32>,
  ) -> PreferredSize {
    let (size, margins) = if edge.is_horizontal() {
      (
        &self.style.size.width,
        (&self.style.margin.left, &self.style.margin.right),
      )
    } else {
      (
        &self.style.size.height,
        (&self.style.margin.top, &self.style.margin.bottom),
      )
    };
    let margin_sum = resolve(margins.0, area.width).unwrap_or_default()
      + resolve(margins.1, area.width).unwrap_or_default();
    let is_auto = size.is_auto();
    let measure = |css: &str| {
      let holder = Node::container([self.node(css)]).with_style(style(&[&format!(
        "position:absolute;left:0;top:0;width:{}px;height:{}px;display:flow-root;",
        area.width, area.height
      )]));
      let root = RenderNode::from_node(
        context,
        Node::container([holder]).with_style(style(&[page_css])),
      );
      let results = LayoutResults::compute_for_paint(
        &root,
        Size::new(
          AvailableSpace::Definite(area.width),
          AvailableSpace::Definite(area.height),
        ),
      );
      let measured = results
        .box_children(NodeId::ROOT)
        .ok()
        .and_then(|children| children.first())
        .and_then(|holder| results.box_children(holder.node_id).ok()?.first())
        .and_then(|measured| results.layout(measured.node_id).ok())
        .map(|layout| layout.size)
        .unwrap_or_default();

      if edge.is_horizontal() {
        measured.width
      } else {
        measured.height
      }
    };
    let (min, max) = if !edge.is_horizontal() {
      let block_size = measure("margin:0;");

      (block_size, block_size)
    } else if is_auto {
      (
        measure("position:absolute;width:min-content;"),
        measure("position:absolute;width:max-content;"),
      )
    } else {
      let inline_size = measure("position:absolute;");

      (inline_size, inline_size)
    };

    PreferredSize {
      min,
      max,
      margin_sum,
      is_auto,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::geometry::Rect;

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
    };
    let area = |kind| {
      let Area { x, y } = page_box.margin_area(kind);

      (x.start, y.start, x.size, y.size)
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
}
