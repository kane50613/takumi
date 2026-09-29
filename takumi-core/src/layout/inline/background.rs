//! Per-line fragments of inline spans, which paint their backgrounds, borders and outlines.

use crate::{
  geometry::{ComputedLayout, PathCommand, Point, Rect, Size},
  layout::{border::BorderProperties, corner_shape::KAPPA, tree::RenderNode},
  style::{BackgroundClip, BoxDecorationBreak, Color, Direction, Sides, SpacePair},
};
use std::{collections::HashMap, rc::Rc};

use super::{
  items::{DecorationLink, InlineDecoration},
  line_box::{BoxKey, LineBoxOffsets},
  outline::InlineOutlineRect,
};

/// A resolved inline background fragment: one rounded rect a decorated span
/// fills on one line, in border-box space, in paint order (outer spans first).
/// The naive drifts from Blink are listed on `DecorationAccumulator`'s doc in
/// this module's source.
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

/// Where a line sits, in border-box space.
#[derive(Clone, Copy)]
pub(super) struct LinePosition {
  pub(super) top: f32,
  pub(super) bottom: f32,
  pub(super) baseline: f32,
  /// The line's `text-fit` scale.
  pub(super) scale: f32,
}

/// The line a covering item sits on.
pub(super) struct CoverLine<'a> {
  pub(super) index: usize,
  pub(super) position: LinePosition,
  /// Where each box on the line sits below its baseline.
  pub(super) offsets: &'a LineBoxOffsets,
}

/// What covers a span on a line.
#[derive(Clone, Copy)]
pub(super) enum Covering {
  /// A glyph run, with its font's content area.
  Run { top: f32, bottom: f32 },
  /// An atomic box or an out-of-flow placeholder.
  Box,
  /// A span's padding.
  Padding,
}

/// Per-line bounds of one decorated span, unioned over the items it covers.
struct FragmentBounds {
  x0: f32,
  x1: f32,
  line: LinePosition,
  /// The content area of the runs on the line.
  runs: Option<(f32, f32)>,
  /// Whether anything but the span's padding sits on the line.
  has_content: bool,
}

/// Accumulates decorated-span coverage per line and resolves it into
/// [`InlineBackgroundFragment`]s, mirroring Blink's per-line inline box
/// fragments (`InlineBoxFragmentPainterBase::PaintBackgroundBorderShadow`).
///
/// Naive next to Blink; where it drifts:
/// - `background-clip: text` on nested spans shows only the innermost span's background
/// - `box-decoration-break: clone` lays the images over each fragment, but the fragments keep
///   `slice`'s padding and borders
/// - a line taller than a page paints its background only on the page owning
///   the line, while Blink spills monolithic overflow onto the next page
#[derive(Default)]
pub(super) struct DecorationAccumulator<'c> {
  /// Each span's position among `decorations`, by the span's id.
  ids: HashMap<usize, usize>,
  decorations: Vec<InlineDecoration<'c>>,
  fragments: HashMap<(usize, usize), FragmentBounds>,
}

impl<'c> DecorationAccumulator<'c> {
  /// The id for `link`, assigning parents first so outer spans paint first.
  fn ensure(&mut self, link: &Rc<DecorationLink<'c>>) -> usize {
    if let Some(id) = self.ids.get(&link.decoration.id) {
      return *id;
    }
    if let Some(parent) = &link.parent {
      self.ensure(parent);
    }
    let id = self.decorations.len();

    self.ids.insert(link.decoration.id, id);
    self.decorations.push(link.decoration.clone());
    id
  }

  pub(super) fn cover(
    &mut self,
    chain: Option<&Rc<DecorationLink<'c>>>,
    line: &CoverLine<'_>,
    x0: f32,
    x1: f32,
    covering: Covering,
  ) {
    let mut next = chain;

    while let Some(link) = next {
      next = link.parent.as_ref();

      if !link.decoration.has_fragments {
        continue;
      }

      let id = self.ensure(link);
      let position = line.position;
      let baseline =
        position.baseline + line.offsets.of(BoxKey::Span(link.decoration.id)) * position.scale;
      let bounds = self
        .fragments
        .entry((id, line.index))
        .or_insert(FragmentBounds {
          x0,
          x1,
          line: LinePosition {
            baseline,
            ..position
          },
          runs: None,
          has_content: false,
        });

      bounds.x0 = bounds.x0.min(x0);
      bounds.x1 = bounds.x1.max(x1);
      bounds.has_content |= !matches!(covering, Covering::Padding);
      if let Covering::Run { top, bottom } = covering {
        bounds.runs = Some(
          bounds
            .runs
            .map_or((top, bottom), |(a, b)| (a.min(top), b.max(bottom))),
        );
      }
    }
  }

  /// Each span's border box on each line it covers, sorted by span then line.
  fn span_fragments(&self) -> Vec<SpanFragment> {
    let mut has_content = vec![false; self.decorations.len()];

    for ((id, _), bounds) in &self.fragments {
      has_content[*id] |= bounds.has_content;
    }
    // Parley can leave a span's start padding at the end of a line when the text after it wraps,
    // where Blink's line breaker carries the open tag along with the text. A span with content
    // elsewhere drops such a padding-only fragment.
    let kept = |(id, line_index): &(usize, usize)| {
      !has_content[*id] || self.fragments[&(*id, *line_index)].has_content
    };
    let mut keys: Vec<(usize, usize)> = self.fragments.keys().copied().filter(kept).collect();
    let mut line_range = vec![(usize::MAX, 0); self.decorations.len()];

    for (id, line_index) in &keys {
      let range = &mut line_range[*id];

      range.0 = range.0.min(*line_index);
      range.1 = range.1.max(*line_index);
    }

    keys.sort_unstable();

    keys
      .into_iter()
      .map(|(id, line_index)| {
        let FragmentBounds {
          x0, x1, line, runs, ..
        } = self.fragments[&(id, line_index)];
        let decoration = &self.decorations[id];
        // The content area of Blink's `InlineBoxState::ComputeTextMetrics`: the span's own
        // primary font around the baseline, whatever its content. Without a primary font, the
        // fonts its runs fell back to stand in, then the line.
        let (top, bottom) = decoration
          .font
          .metrics
          .map(|metrics| {
            (
              line.baseline - metrics.ascent * line.scale,
              line.baseline + metrics.descent * line.scale,
            )
          })
          .or(runs)
          .unwrap_or((line.top, line.bottom));
        let (min_line, max_line) = line_range[id];

        SpanFragment {
          id,
          line_index,
          x: x0,
          y: top - decoration.padding.top - decoration.border.width.top,
          width: x1 - x0,
          height: bottom - top + decoration.padding.vertical() + decoration.border.width.vertical(),
          baseline: line.baseline,
          has_start: line_index == min_line,
          has_end: line_index == max_line,
        }
      })
      .collect()
  }

  /// The spans' background fragments and their outlines' line fragments, both sorted by span
  /// then line.
  pub(super) fn into_fragments(
    self,
  ) -> (Vec<InlineBackgroundFragment<'c>>, Vec<InlineOutlineRect>) {
    let mut backgrounds = Vec::new();
    let mut outlines = Vec::new();
    let fragments = self.span_fragments();

    for span in fragments.chunk_by(|a, b| a.id == b.id) {
      let strip_width: f32 = span.iter().map(|fragment| fragment.width).sum();
      let mut before = 0.0;

      for &fragment in span {
        let strip_offset = before;

        before += fragment.width;
        self.resolve_fragment(
          fragment,
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
  fn resolve_fragment(
    &self,
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
    let decoration = &self.decorations[id];

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
      .background_image
      .as_deref()
      .is_some_and(|images| !images.is_empty());
    let background =
      (decoration.color.0[3] != 0 || has_images || style.background_clip == BackgroundClip::Text)
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
          let (strip_x, strip) = match style.box_decoration_break {
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

  /// The padding box each span bounds its out-of-flow boxes with, after Blink's
  /// `OutOfFlowLayoutPart::AddInlineContainingBlockInfo`, in a `direction` formatting context.
  pub(super) fn containing_blocks(&self, direction: Direction) -> Vec<InlineContainingBlock<'c>> {
    let fragments = self.span_fragments();
    let rtl = direction == Direction::Rtl;
    // The inline offset of a rect's logical start, measured against the formatting context's
    // direction from an origin that cancels out when converting back.
    let inline_start = |x: f32, width: f32| if rtl { -(x + width) } else { x };

    fragments
      .chunk_by(|a, b| a.id == b.id)
      .filter_map(|span| {
        let (first, last) = (span.first()?, span.last()?);
        let decoration = &self.decorations[first.id];
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
}

/// One span's border box on one line.
#[derive(Clone, Copy)]
struct SpanFragment {
  id: usize,
  line_index: usize,
  x: f32,
  y: f32,
  width: f32,
  height: f32,
  baseline: f32,
  /// Whether the span's start edge sits on this line, its first.
  has_start: bool,
  /// Whether the span's end edge sits on this line, its last.
  has_end: bool,
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
