//! How far above its page a box can paint, bounded from the render tree before any page is laid out.

use crate::{
  context::{BoxId, RenderContext},
  font_style::SizedFontStyle,
  geometry::Size,
  layout::{
    layout_style::{LayoutLength, LayoutStyle},
    out_of_flow::{ContainingBlockClaims, depends_on_containing_block_block_size},
    tree::RenderNode,
  },
  scene::{box_ink_reach, filter_reach, has_inline_paint_content, text_ink_reach},
  style::{Display, Position},
};

/// How many font sizes above its line a glyph's ink is taken to reach, which a font would have to
/// exceed to paint a line past it.
const GLYPH_REACH_EMS: f32 = 4.0;

/// The container size a probe resolves container-relative lengths against, which differs between
/// the two probes so a length that depends on it shows.
const PROBE_SIZES: [f32; 2] = [0.0, 1024.0];

/// What the styles of a render tree say, before any page is laid out, about how far its boxes
/// paint from the page layout puts them on, and which boxes they leave to the layout.
pub(crate) struct PaintReach {
  /// An upper bound on how many px above the page layout puts it on a box not in
  /// [`Self::unbounded`] can paint, or none when a negative margin that depends on the layout, or a
  /// box that repeats on every page, leaves no bound.
  ///
  /// A fragment on page `p` paints no higher than `p` pages down less this bound. A box sits at or
  /// below its page's top but for the negative margins before it and the offsets of the boxes around
  /// it, and what it paints reaches past it by its shadow, outline, glyph overshoot and the filters
  /// that spread it. The bound adds every negative margin of the tree, which pull the boxes after
  /// them up, to the largest sum of offsets and filter reaches down a chain of boxes and the ink of
  /// the box at its end, plus a px for bounds rounded outwards.
  ///
  /// Glyph ink is taken to reach at most [`GLYPH_REACH_EMS`] font sizes above its line, which is an
  /// assumption about fonts and not a measurement.
  pub(crate) back: Option<f32>,
  /// The boxes, each by the block that lays out its inline content, whose paint no bound covers: a
  /// transform, a percentage offset, or a length that depends on the size of its container. The
  /// layout bounds them once it has laid them out.
  pub(crate) unbounded: Vec<BoxId>,
  /// The absolute boxes, and the fixed boxes a transformed or filtered box contains.
  pub(crate) positioned: Vec<PositionedBox>,
}

/// An out-of-flow box and the box that positions it.
pub(crate) struct PositionedBox {
  pub(crate) id: BoxId,
  /// The block that lays out the containing block, none for the initial containing block.
  pub(crate) containing_block: Option<BoxId>,
}

/// A box on the way down the tree, with what its ancestors decide for it.
struct Frame<'n> {
  node: &'n RenderNode,
  /// The sum of offsets and filter reaches down to the parent.
  offset: f32,
  /// The block that lays out the inline content the parent belongs to.
  block: BoxId,
  /// The block of the nearest ancestor that contains absolute boxes.
  absolute_container: Option<BoxId>,
  /// The block of the nearest ancestor that contains fixed boxes.
  fixed_container: Option<BoxId>,
  /// Whether a fixed box or a repeated table group holds it, so a page it is on repeats it.
  repeats: bool,
}

impl PaintReach {
  /// Scans the tree under `root`.
  pub(crate) fn of(root: &RenderNode) -> Self {
    let mut margins = 0.0_f32;
    let mut chain = 0.0_f32;
    let mut is_bounded = true;
    let mut unbounded = Vec::new();
    let mut positioned = Vec::new();
    let mut stack = vec![Frame {
      node: root,
      offset: 0.0,
      block: root.context.box_id(),
      absolute_container: None,
      fixed_container: None,
      repeats: false,
    }];

    while let Some(frame) = stack.pop() {
      let node = frame.node;
      let style = &node.context.style;
      let position = style.base_data.position;
      let display = style.base_data.display;

      if display == Display::None {
        continue;
      }

      let probes = Probes::of(node);
      let styles = probes.styles(node);
      let block = if node.is_inline_span() {
        frame.block
      } else {
        node.context.box_id()
      };
      let repeats = frame.repeats
        || position == Position::Fixed
        || matches!(
          display,
          Display::TableHeaderGroup | Display::TableFooterGroup
        );

      match negative_margins(&styles) {
        Some(pulled) => margins += pulled,
        None => is_bounded = false,
      }

      let displacement = displacement(&styles);
      let filter = probes.filter();
      let ink = probes.ink(node, &styles);
      let is_placed_by_layout =
        style.has_transform_related_property() || displacement.is_none() || filter.is_none();
      let offset = match (displacement, filter) {
        (Some(displacement), Some(filter)) if !is_placed_by_layout => {
          frame.offset + displacement + filter
        }
        _ => 0.0,
      };

      if is_placed_by_layout || ink.is_none() {
        // A repeat is laid out again on every page, so laying out one says nothing of the rest.
        if repeats {
          is_bounded = false;
        } else {
          unbounded.push(block);
        }
      }
      if !is_placed_by_layout && let Some(ink) = ink {
        chain = chain.max(offset + ink);
      }

      let claims = ContainingBlockClaims::of(style);

      match position {
        Position::Absolute => positioned.push(PositionedBox {
          id: node.context.box_id(),
          containing_block: frame.absolute_container,
        }),
        Position::Fixed => {
          if let Some(containing_block) = frame.fixed_container {
            positioned.push(PositionedBox {
              id: node.context.box_id(),
              containing_block: Some(containing_block),
            });
          }
        }
        Position::Static | Position::Relative => {}
      }

      let absolute_container = if claims.absolute {
        Some(block)
      } else {
        frame.absolute_container
      };
      let fixed_container = if claims.fixed {
        Some(block)
      } else {
        frame.fixed_container
      };

      stack.extend(node.children.iter().flatten().map(|child| Frame {
        node: child,
        offset,
        block,
        absolute_container,
        fixed_container,
        repeats,
      }));
    }

    Self {
      back: is_bounded.then_some(margins + chain + 1.0),
      unbounded,
      positioned,
    }
  }
}

/// A box's context resolved twice, against different container sizes.
struct Probes([RenderContext; 2]);

impl Probes {
  fn of(node: &RenderNode) -> Self {
    Self(PROBE_SIZES.map(|size| {
      let mut context = node.context.clone();

      context.sizing.set_container_size(Some(size), Some(size));
      context
    }))
  }

  fn styles(&self, node: &RenderNode) -> [LayoutStyle; 2] {
    [0, 1].map(|probe| node.layout_box_style(&self.0[probe].sizing))
  }

  /// How far the box's filters spread what it and the boxes inside it paint.
  fn filter(&self) -> Option<f32> {
    same(filter_reach(&self.0[0]), filter_reach(&self.0[1]))
  }

  /// How far the box's own ink reaches past its border box, none when it depends on the container.
  fn ink(&self, node: &RenderNode, styles: &[LayoutStyle; 2]) -> Option<f32> {
    let [first, second] = &self.0;
    let sizes = PROBE_SIZES.map(|size| Size {
      width: size,
      height: size,
    });
    let mut ink = same(
      box_ink_reach(first, sizes[0]),
      box_ink_reach(second, sizes[1]),
    )?;

    if styles[0].display == Display::Inline {
      let edge = |style: &LayoutStyle| {
        let padding = &style.padding;

        [padding.top, padding.right, padding.bottom, padding.left]
          .into_iter()
          .map(|length| match length {
            LayoutLength::Auto => Some(0.0),
            LayoutLength::Fixed(px) => Some(px.max(0.0)),
            _ => None,
          })
          .chain(
            [
              style.border.top,
              style.border.right,
              style.border.bottom,
              style.border.left,
            ]
            .map(Some),
          )
          .try_fold(0.0_f32, |edge, side| Some(edge.max(side?)))
      };

      ink += same(edge(&styles[0])?, edge(&styles[1])?)?;
    }
    if has_inline_paint_content(node) {
      let text = |context: &RenderContext| {
        let font_style = SizedFontStyle::from_style(&context.style, context);

        GLYPH_REACH_EMS * font_style.sizing.font_size + text_ink_reach(&font_style)
      };

      ink += same(text(first), text(second))?;
    }
    Some(ink)
  }
}

/// `value` when both probes resolve it alike.
fn same(first: f32, second: f32) -> Option<f32> {
  (first == second).then_some(first)
}

/// `length` in px when both probes resolve it to the same fixed length or `auto`, as zero.
fn fixed(styles: &[LayoutStyle; 2], length: impl Fn(&LayoutStyle) -> LayoutLength) -> Option<f32> {
  match (length(&styles[0]), length(&styles[1])) {
    (LayoutLength::Auto, LayoutLength::Auto) => Some(0.0),
    (LayoutLength::Fixed(first), LayoutLength::Fixed(second)) => same(first, second),
    _ => None,
  }
}

/// How far the box's margins that are negative pull the boxes after them up.
fn negative_margins(styles: &[LayoutStyle; 2]) -> Option<f32> {
  let sides = [
    |style: &LayoutStyle| style.margin.top,
    |style: &LayoutStyle| style.margin.right,
    |style: &LayoutStyle| style.margin.bottom,
    |style: &LayoutStyle| style.margin.left,
  ];
  let mut pulled = 0.0;

  for side in sides {
    // A percentage of a positive width is never negative.
    pulled += match (side(&styles[0]), side(&styles[1])) {
      (LayoutLength::Percent(first), LayoutLength::Percent(second))
        if first >= 0.0 && second >= 0.0 =>
      {
        0.0
      }
      _ => (-fixed(styles, side)?).max(0.0),
    };
  }
  Some(pulled)
}

/// How far the box's own offset moves it up from where the flow puts it. An absolute box that
/// depends on the block size of its containing block sits where [`held_displacement`] says, and a
/// fixed one repeats on every page, so none bounds it.
fn displacement(styles: &[LayoutStyle; 2]) -> Option<f32> {
  let style = &styles[0];

  match style.position {
    Position::Static => Some(0.0),
    Position::Relative => Some(
      fixed(styles, |style| style.inset.top)?.abs()
        + fixed(styles, |style| style.inset.bottom)?.abs(),
    ),
    Position::Absolute | Position::Fixed => {
      if depends_on_containing_block_block_size(style) {
        return (style.position == Position::Absolute).then(|| held_displacement(styles));
      }

      Some((-fixed(styles, |style| style.inset.top)?).max(0.0))
    }
  }
}

/// How far above the start of its containing block an absolute box that depends on the block size
/// of its containing block can start, which the block's first page then holds back. A box anchored
/// at its top starts at its offset. One anchored at its bottom starts its offset, size, margins,
/// padding and border above the block's end, so no higher than that above the block's start.
///
/// A size that is auto or a percentage counts as zero, which takes the box to fit its containing
/// block and so is approximate for a box taller than the block.
fn held_displacement(styles: &[LayoutStyle; 2]) -> f32 {
  let style = &styles[0];
  let px = |length: &dyn Fn(&LayoutStyle) -> LayoutLength| {
    styles
      .iter()
      .map(|style| match length(style) {
        LayoutLength::Fixed(px) | LayoutLength::Calc { px, .. } => px,
        _ => 0.0,
      })
      .fold(0.0_f32, |most, px| most.max(px.abs()))
  };

  if !style.inset.top.is_auto() {
    return styles
      .iter()
      .map(|style| match style.inset.top {
        LayoutLength::Fixed(px) | LayoutLength::Calc { px, .. } => (-px).max(0.0),
        _ => 0.0,
      })
      .fold(0.0, f32::max);
  }
  if style.inset.bottom.is_auto() {
    return 0.0;
  }

  let border = |style: &LayoutStyle| style.border.top + style.border.bottom;

  px(&|style| style.inset.bottom)
    + px(&|style| style.size.height).max(px(&|style| style.min_size.height))
    + px(&|style| style.margin.top)
    + px(&|style| style.margin.bottom)
    + px(&|style| style.padding.top)
    + px(&|style| style.padding.bottom)
    + styles.iter().map(border).fold(0.0, f32::max)
}

#[cfg(test)]
mod tests {
  use std::sync::Arc;

  use super::PaintReach;
  use crate::{
    context::RenderContext,
    layout::{node::Node, tree::RenderNode},
    resources::font::Fonts,
    style::{SizingContext, StyleSheet},
    viewport::Viewport,
  };

  /// The reach of a block with a child styled by `css`, among blocks without any.
  fn reach_of(css: &str) -> PaintReach {
    let stylesheet = StyleSheet::parse(&format!(
      ".block {{ display: block; height: 50px }} .child {{ {css} }}"
    ))
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
    let root = Node::container([
      Node::container(Vec::<Node>::new()).with_class_name("block"),
      Node::container(Vec::<Node>::new()).with_class_name("block child"),
    ])
    .with_class_name("block");

    PaintReach::of(&RenderNode::from_node(&context, root))
  }

  #[test]
  fn plain_boxes_reach_back_by_the_rounding_of_their_bounds() {
    assert_eq!(reach_of("").back, Some(1.0));
  }

  #[test]
  fn shadows_offsets_and_negative_margins_add_their_reach() {
    assert_eq!(reach_of("box-shadow: 0 0 0 30px red").back, Some(31.0));
    assert_eq!(reach_of("position: relative; top: -40px").back, Some(41.0));
    assert_eq!(reach_of("position: absolute; top: -25px").back, Some(26.0));
    assert_eq!(reach_of("margin-top: -60px").back, Some(61.0));
    assert_eq!(reach_of("margin-top: 60px").back, Some(1.0));
  }

  #[test]
  fn boxes_the_layout_places_anywhere_are_listed_and_leave_the_bound_to_the_rest() {
    for css in [
      "transform: translateY(-300px)",
      "position: relative; top: 10%",
      "box-shadow: 0 10% 0 0 red",
    ] {
      let reach = reach_of(css);

      assert_eq!(reach.back, Some(1.0), "{css}");
      assert_eq!(reach.unbounded.len(), 1, "{css}");
    }
  }

  #[test]
  fn an_absolute_box_that_depends_on_its_containing_block_is_pending_in_it() {
    for (css, back) in [
      ("position: absolute; top: 10%", 1.0),
      ("position: absolute; top: -20px; height: 50%", 21.0),
      // The box ends at the block's end, and starts its height above that.
      (
        "position: absolute; bottom: 5px; height: 40px; margin-top: 3px",
        49.0,
      ),
    ] {
      let reach = reach_of(css);

      assert_eq!(reach.back, Some(back), "{css}");
      assert!(reach.unbounded.is_empty(), "{css}");
      assert_eq!(reach.positioned.len(), 1, "{css}");
      assert!(reach.positioned[0].containing_block.is_none(), "{css}");
    }
  }

  #[test]
  fn what_depends_on_every_page_or_on_layout_leaves_no_bound() {
    for css in ["margin-top: -10%", "position: fixed; bottom: 0"] {
      assert_eq!(reach_of(css).back, None, "{css}");
    }
  }
}
