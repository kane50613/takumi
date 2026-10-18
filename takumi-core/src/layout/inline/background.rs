//! Per-line fragments of inline spans, which paint their backgrounds, borders and outlines.

use crate::{
  geometry::{ComputedLayout, PathCommand, Point, Rect, Size},
  layout::{border::BorderProperties, corner_shape::KAPPA, tree::RenderNode},
  style::{BackgroundClip, BoxDecorationBreak, Color, Direction, Sides, SpacePair},
};

use std::rc::Rc;

use super::{
  items::{DecorationLink, InlineDecoration},
  outline::InlineOutlineRect,
};

/// A resolved inline background fragment: one rounded rect a decorated span
/// fills on one line, in border-box space, in paint order (outer spans first).
#[derive(Clone, Copy)]
#[non_exhaustive]
pub struct InlineBackgroundFragment<'c> {
  /// Left edge.
  pub x: f32,
  /// Top edge.
  pub y: f32,
  /// Fragment width.
  pub width: f32,
  /// Fragment height.
  pub height: f32,
  /// The border, without the sides a line wraps at, its radii clamped to the fragment.
  pub border: BorderProperties,
  /// Fill color.
  pub color: Color,
  /// The span's `opacity`.
  pub opacity: f32,
  /// Baseline of the owning line in border-box space.
  pub baseline: f32,
  /// The span's background, when it paints one.
  pub background: Option<FragmentBackground<'c>>,
  /// The span.
  pub owner: &'c RenderNode,
  /// The span's id.
  pub(crate) span: usize,
}

/// The background of a span on one line: its color, and its `background-image` layers laid over
/// the strip its fragments would make on one line, as Blink's
/// `InlineBoxFragmentPainterBase::PaintRectForImageStrip` lays them.
#[derive(Clone, Copy)]
#[non_exhaustive]
pub struct FragmentBackground<'c> {
  /// The span.
  pub node: &'c RenderNode,
  /// The strip's top-left, in border-box space.
  pub strip_origin: Point<f32>,
  /// The strip, which the layers lay out over.
  pub strip: ComputedLayout,
  /// The fragment, placed at `x` and `y`, which `background-clip` clips the layers to.
  pub fragment: ComputedLayout,
}

/// The background and outline fragments of each span's line fragments, which come grouped by span
/// in line order.
pub(super) fn resolve_span_fragments<'c>(
  fragments: &[(Rc<DecorationLink<'c>>, SpanFragment)],
) -> (Vec<InlineBackgroundFragment<'c>>, Vec<InlineOutlineRect>) {
  let mut backgrounds = Vec::new();
  let mut outlines = Vec::new();

  for span in fragments.chunk_by(|(_, a), (_, b)| a.id == b.id) {
    let strip_width: f32 = span.iter().map(|(_, fragment)| fragment.width).sum();
    let mut before = 0.0;

    for (link, fragment) in span {
      let strip_offset = before;

      before += fragment.width;
      resolve_fragment(
        &link.decoration,
        *fragment,
        strip_offset,
        strip_width,
        &mut backgrounds,
        &mut outlines,
      );
    }
  }

  (backgrounds, outlines)
}

/// Resolves `fragment`, `strip_offset` along its span's strip of `strip_width`, into its
/// background and outline fragments.
fn resolve_fragment<'c>(
  decoration: &InlineDecoration<'c>,
  fragment: SpanFragment,
  strip_offset: f32,
  strip_width: f32,
  backgrounds: &mut Vec<InlineBackgroundFragment<'c>>,
  outlines: &mut Vec<InlineOutlineRect>,
) {
  let SpanFragment {
    id,
    line_index,
    x,
    y,
    width,
    height,
    baseline,
    has_start,
    has_end,
  } = fragment;

  if !decoration.paints {
    return;
  }

  let mut border = decoration.border;
  // The start edge sits on the first line, the end edge on the last;
  // wrap-edge corners stay square, like `box-decoration-break: slice`.
  let (has_left, has_right) = decoration.direction.inline_sides(has_start, has_end);
  let radii = decoration.radius.0.map(|radius| {
    let radius = radius.to_px(&decoration.sizing, width, height);

    (radius.x, radius.y)
  });
  let [top_left, top_right, bottom_right, bottom_left] = radii;
  let sliced = [
    if has_left { top_left } else { (0.0, 0.0) },
    if has_right { top_right } else { (0.0, 0.0) },
    if has_right { bottom_right } else { (0.0, 0.0) },
    if has_left { bottom_left } else { (0.0, 0.0) },
  ];

  border.radius = fitted_radii(sliced, width, height);

  if !has_left {
    border.width.left = 0.0;
  }

  if !has_right {
    border.width.right = 0.0;
  }

  if width <= 0.0 || height <= 0.0 {
    return;
  }

  if let Some(outline) = decoration.outline {
    outlines.push(InlineOutlineRect {
      owner: id,
      line_index,
      x,
      y,
      width,
      height,
      radius: fitted_radii(radii, width, height),
      outline,
      opacity: decoration.opacity,
    });
  }

  let style = &decoration.owner.context.style;
  let has_images = style
    .background_data
    .background_image
    .as_deref()
    .is_some_and(|images| !images.is_empty());
  let background = (decoration.color.0[3] != 0
    || has_images
    || style.background_data.background_clip == BackgroundClip::Text)
    .then(|| {
      let side = |has: bool, width: f32| if has { width } else { 0.0 };
      let padding = Rect {
        left: side(has_left, decoration.padding.left),
        right: side(has_right, decoration.padding.right),
        ..decoration.padding
      };
      let fragment = ComputedLayout::new(
        Point { x, y },
        Size { width, height },
        border.width,
        padding,
      );
      let (strip_x, strip) = match style.box_data.box_decoration_break {
        BoxDecorationBreak::Clone => (x, fragment),
        BoxDecorationBreak::Slice => (
          match decoration.direction {
            Direction::Rtl => x + width + strip_offset - strip_width,
            _ => x - strip_offset,
          },
          ComputedLayout::new(
            Point::ZERO,
            Size {
              width: strip_width,
              height,
            },
            decoration.border.width,
            decoration.padding,
          ),
        ),
      };

      FragmentBackground {
        node: decoration.owner,
        strip_origin: Point { x: strip_x, y },
        strip,
        fragment,
      }
    });

  if border.has_visible_sides() || background.is_some() {
    backgrounds.push(InlineBackgroundFragment {
      x,
      y,
      width,
      height,
      border,
      color: decoration.color,
      opacity: decoration.opacity,
      baseline,
      background,
      owner: decoration.owner,
      span: decoration.id,
    });
  }
}

/// One span's border box on one line.
#[derive(Clone, Copy)]
pub(super) struct SpanFragment {
  pub(super) id: usize,
  pub(super) line_index: usize,
  pub(super) x: f32,
  pub(super) y: f32,
  pub(super) width: f32,
  pub(super) height: f32,
  pub(super) baseline: f32,
  /// Whether the span's start edge sits on this line, its first.
  pub(super) has_start: bool,
  /// Whether the span's end edge sits on this line, its last.
  pub(super) has_end: bool,
}

/// An inline span that contains out-of-flow boxes.
#[derive(Clone, Copy)]
pub(crate) struct InlineContainingBlock<'c> {
  /// The span.
  pub(crate) owner: &'c RenderNode,
  /// The box the span bounds the boxes it contains with.
  pub(crate) padding_box: PaddingBox,
}

/// A padding box, placed in its inline formatting context's border box.
#[derive(Clone, Copy)]
pub(crate) struct PaddingBox {
  pub(crate) origin: Point<f32>,
  pub(crate) size: Size<f32>,
}

impl PaddingBox {
  /// The padding box of the inline formatting context `layout` lays out.
  pub(crate) fn of(layout: ComputedLayout) -> Self {
    Self {
      origin: Point {
        x: layout.border.left,
        y: layout.border.top,
      },
      size: Size {
        width: layout.padding_box_width(),
        height: layout.padding_box_height(),
      },
    }
  }
}

/// `radii` shrunk by one uniform factor so adjacent corners never cross, as css-backgrounds-3's
/// corner overlap rule asks.
fn fitted_radii(radii: [(f32, f32); 4], width: f32, height: f32) -> Sides<SpacePair<f32>> {
  let [tl, tr, br, bl] = radii;
  let factor = [
    width / (tl.0 + tr.0),
    width / (bl.0 + br.0),
    height / (tl.1 + bl.1),
    height / (tr.1 + br.1),
  ]
  .into_iter()
  .filter(|f| f.is_finite())
  .fold(1.0_f32, f32::min)
  .max(0.0);

  Sides(radii.map(|(rx, ry)| SpacePair::from_pair(rx * factor, ry * factor)))
}

impl InlineBackgroundFragment<'_> {
  /// The rounded-rect contour the fragment fills, with quarter-ellipse corners.
  pub fn path(&self) -> Vec<PathCommand> {
    let InlineBackgroundFragment {
      x,
      y,
      width,
      height,
      border,
      ..
    } = *self;
    let point = |x, y| Point { x, y };
    let [tl, tr, br, bl] = border.radius.0.map(|SpacePair { x: rx, y: ry }| {
      if rx > 0.0 && ry > 0.0 {
        (rx, ry)
      } else {
        (0.0, 0.0)
      }
    });
    if [tl, tr, br, bl] == [(0.0, 0.0); 4] {
      return vec![
        PathCommand::MoveTo(point(x, y)),
        PathCommand::LineTo(point(x + width, y)),
        PathCommand::LineTo(point(x + width, y + height)),
        PathCommand::LineTo(point(x, y + height)),
        PathCommand::Close,
      ];
    }
    let mut path = Vec::with_capacity(9);

    path.push(PathCommand::MoveTo(point(x + tl.0, y)));
    path.push(PathCommand::LineTo(point(x + width - tr.0, y)));
    if tr.0 > 0.0 {
      path.push(PathCommand::CubicTo(
        point(x + width - tr.0 + tr.0 * KAPPA, y),
        point(x + width, y + tr.1 - tr.1 * KAPPA),
        point(x + width, y + tr.1),
      ));
    }
    path.push(PathCommand::LineTo(point(x + width, y + height - br.1)));
    if br.0 > 0.0 {
      path.push(PathCommand::CubicTo(
        point(x + width, y + height - br.1 + br.1 * KAPPA),
        point(x + width - br.0 + br.0 * KAPPA, y + height),
        point(x + width - br.0, y + height),
      ));
    }
    path.push(PathCommand::LineTo(point(x + bl.0, y + height)));
    if bl.0 > 0.0 {
      path.push(PathCommand::CubicTo(
        point(x + bl.0 - bl.0 * KAPPA, y + height),
        point(x, y + height - bl.1 + bl.1 * KAPPA),
        point(x, y + height - bl.1),
      ));
    }
    path.push(PathCommand::LineTo(point(x, y + tl.1)));
    if tl.0 > 0.0 {
      path.push(PathCommand::CubicTo(
        point(x, y + tl.1 - tl.1 * KAPPA),
        point(x + tl.0 - tl.0 * KAPPA, y),
        point(x + tl.0, y),
      ));
    }
    path.push(PathCommand::Close);
    path
  }
}

/// The padding box each span bounds its out-of-flow boxes with, from its line fragments grouped
/// by span in line order, after Blink's `OutOfFlowLayoutPart::AddInlineContainingBlockInfo`, in a
/// `direction` formatting context.
pub(super) fn containing_blocks<'c>(
  fragments: &[(Rc<DecorationLink<'c>>, SpanFragment)],
  direction: Direction,
) -> Vec<InlineContainingBlock<'c>> {
  let rtl = direction == Direction::Rtl;
  // The inline offset of a rect's logical start, measured against the formatting context's
  // direction from an origin that cancels out when converting back.
  let inline_start = |x: f32, width: f32| if rtl { -(x + width) } else { x };

  fragments
    .chunk_by(|(_, a), (_, b)| a.id == b.id)
    .filter_map(|span| {
      let ((link, first), (_, last)) = (span.first()?, span.last()?);
      let decoration = &link.decoration;
      let widths = decoration.border.width;
      let same_direction = decoration.direction == direction;
      let (border_start, border_end) = if rtl {
        (widths.right, widths.left)
      } else {
        (widths.left, widths.right)
      };
      let mut start = Point {
        x: inline_start(first.x, first.width),
        y: first.y + widths.top,
      };
      let mut end = Point {
        x: inline_start(last.x, last.width) + last.width,
        y: last.y + last.height - widths.bottom,
      };

      if same_direction {
        start.x += border_start;
        end.x -= border_end;
      }
      end.x = end.x.max(start.x);
      end.y = end.y.max(start.y);

      let size = Size {
        width: end.x - start.x,
        height: end.y - start.y,
      };

      Some(InlineContainingBlock {
        owner: decoration.owner,
        padding_box: PaddingBox {
          origin: Point {
            x: if rtl {
              -(start.x + size.width)
            } else {
              start.x
            },
            y: start.y,
          },
          size,
        },
      })
    })
    .collect()
}
