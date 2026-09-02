//! Per-line background fragments of inline spans.

use crate::{
  geometry::{PathCommand, Point},
  style::Color,
};
use std::{collections::HashMap, rc::Rc};

use super::items::{DecorationLink, InlineDecoration};

/// A resolved inline background fragment: one rounded rect a decorated span
/// fills on one line, in border-box space, in paint order (outer spans first).
/// The naive drifts from Blink are listed on `DecorationAccumulator`'s doc in
/// this module's source.
#[derive(Clone, Copy)]
#[non_exhaustive]
pub struct InlineBackgroundFragment {
  /// Left edge.
  pub x: f32,
  /// Top edge.
  pub y: f32,
  /// Fragment width.
  pub width: f32,
  /// Fragment height.
  pub height: f32,
  /// Corner radii as `(x, y)` pairs (top-left, top-right, bottom-right, bottom-left), already
  /// clamped to the fragment.
  pub radii: [(f32, f32); 4],
  /// Fill color.
  pub color: Color,
  /// The span's `opacity`.
  pub opacity: f32,
  /// Baseline of the owning line in border-box space.
  pub baseline: f32,
}

/// One tier of a fragment's vertical bounds, unioned over covering items.
#[derive(Clone, Copy)]
struct VerticalExtent {
  top: f32,
  bottom: f32,
  baseline: f32,
  set: bool,
}

impl VerticalExtent {
  const EMPTY: Self = Self {
    top: f32::INFINITY,
    bottom: f32::NEG_INFINITY,
    baseline: 0.0,
    set: false,
  };

  fn merge(&mut self, top: f32, bottom: f32, baseline: f32) {
    self.top = self.top.min(top);
    self.bottom = self.bottom.max(bottom);
    if !self.set {
      self.baseline = baseline;
    }
    self.set = true;
  }

  fn get(&self) -> Option<(f32, f32, f32)> {
    self.set.then_some((self.top, self.bottom, self.baseline))
  }
}

/// What a covering item contributes vertically to a span's fragment.
pub(super) enum CoverExtent {
  /// A glyph run's leaded box; sizes the fragment when the run's font size matches the span's own.
  Run {
    font_size: f32,
    top: f32,
    bottom: f32,
    baseline: f32,
  },
  /// The owning line's extent; the last resort for padding-only coverage.
  Line {
    top: f32,
    bottom: f32,
    baseline: f32,
  },
}

/// Per-line bounds of one decorated span, unioned over the items it covers.
struct FragmentBounds {
  x0: f32,
  x1: f32,
  /// The span's own runs, like Blink sizing an inline box fragment from the
  /// box's own text metrics.
  own: VerticalExtent,
  /// Descendant runs, used when the span has no text of its own.
  descendant: VerticalExtent,
  /// The owning line, used when nothing on the fragment carries text.
  line: VerticalExtent,
}

/// Accumulates decorated-span coverage per line and resolves it into
/// [`InlineBackgroundFragment`]s, mirroring Blink's per-line inline box
/// fragments (`InlineBoxFragmentPainterBase::PaintBackgroundBorderShadow`).
///
/// Naive next to Blink; where it drifts:
/// - only `background-color` fills; gradients, images, and `border` on a span
///   paint nothing
/// - the span's own runs are recognized by font size, not by element, so a
///   same-size fallback font can grow the height where Blink keeps the
///   primary font's
/// - a line taller than a page paints its background only on the page owning
///   the line, while Blink spills monolithic overflow onto the next page
#[derive(Default)]
pub(super) struct DecorationAccumulator {
  ids: HashMap<*const DecorationLink, usize>,
  decorations: Vec<InlineDecoration>,
  fragments: HashMap<(usize, usize), FragmentBounds>,
}

impl DecorationAccumulator {
  /// The id for `link`, assigning parents first so outer spans paint first.
  fn ensure(&mut self, link: &Rc<DecorationLink>) -> usize {
    if let Some(id) = self.ids.get(&Rc::as_ptr(link)) {
      return *id;
    }
    if let Some(parent) = &link.parent {
      self.ensure(parent);
    }
    let id = self.decorations.len();

    self.ids.insert(Rc::as_ptr(link), id);
    self.decorations.push(link.decoration);
    id
  }

  pub(super) fn cover(
    &mut self,
    chain: Option<&Rc<DecorationLink>>,
    line_index: usize,
    x0: f32,
    x1: f32,
    extent: &CoverExtent,
  ) {
    let mut next = chain;

    while let Some(link) = next {
      let id = self.ensure(link);
      let bounds = self
        .fragments
        .entry((id, line_index))
        .or_insert(FragmentBounds {
          x0,
          x1,
          own: VerticalExtent::EMPTY,
          descendant: VerticalExtent::EMPTY,
          line: VerticalExtent::EMPTY,
        });

      bounds.x0 = bounds.x0.min(x0);
      bounds.x1 = bounds.x1.max(x1);
      match *extent {
        CoverExtent::Run {
          font_size,
          top,
          bottom,
          baseline,
        } => {
          let tier = if (link.decoration.font_size - font_size).abs() < 0.01 {
            &mut bounds.own
          } else {
            &mut bounds.descendant
          };

          tier.merge(top, bottom, baseline);
        }
        CoverExtent::Line {
          top,
          bottom,
          baseline,
        } => bounds.line.merge(top, bottom, baseline),
      }
      next = link.parent.as_ref();
    }
  }

  pub(super) fn into_fragments(self) -> Vec<InlineBackgroundFragment> {
    // A span with any text sizes every fragment from runs; the line-extent
    // tier only carries a span with no text at all (padding-only), so a
    // spacer the line breaker strands on its own line stays invisible.
    let mut has_text = vec![false; self.decorations.len()];

    for ((id, _), bounds) in &self.fragments {
      has_text[*id] |= bounds.own.set || bounds.descendant.set;
    }
    let vertical = |id: usize, bounds: &FragmentBounds| {
      bounds
        .own
        .get()
        .or_else(|| bounds.descendant.get())
        .or_else(|| (!has_text[id]).then(|| bounds.line.get()).flatten())
    };
    let mut line_range = vec![(usize::MAX, 0); self.decorations.len()];

    for ((id, line_index), bounds) in &self.fragments {
      if vertical(*id, bounds).is_some() {
        let range = &mut line_range[*id];

        range.0 = range.0.min(*line_index);
        range.1 = range.1.max(*line_index);
      }
    }
    let mut keys: Vec<(usize, usize)> = self.fragments.keys().copied().collect();

    keys.sort_unstable();
    keys
      .into_iter()
      .filter_map(|key| {
        let (id, line_index) = key;
        let bounds = &self.fragments[&key];

        let (top, bottom, baseline) = vertical(id, bounds)?;
        let decoration = &self.decorations[id];
        let x = bounds.x0;
        let y = top - decoration.padding.top;
        let width = bounds.x1 - bounds.x0;
        let height = bottom - top + decoration.padding.vertical();
        let (min_line, max_line) = line_range[id];
        // The start edge sits on the first line, the end edge on the last;
        // wrap-edge corners stay square, like `box-decoration-break: slice`.
        let (has_start, has_end) = (line_index == min_line, line_index == max_line);
        let (has_left, has_right) = if decoration.rtl {
          (has_end, has_start)
        } else {
          (has_start, has_end)
        };
        // css-backgrounds-3 corner overlap: one uniform factor shrinks every
        // radius so adjacent corners never cross.
        let raw = [
          if has_left {
            decoration.radii[0]
          } else {
            (0.0, 0.0)
          },
          if has_right {
            decoration.radii[1]
          } else {
            (0.0, 0.0)
          },
          if has_right {
            decoration.radii[2]
          } else {
            (0.0, 0.0)
          },
          if has_left {
            decoration.radii[3]
          } else {
            (0.0, 0.0)
          },
        ];
        let [tl, tr, br, bl] = raw;
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
        let radii = raw.map(|(rx, ry)| (rx * factor, ry * factor));

        (width > 0.0 && height > 0.0).then_some(InlineBackgroundFragment {
          x,
          y,
          width,
          height,
          radii,
          color: decoration.color,
          opacity: decoration.opacity,
          baseline,
        })
      })
      .collect()
  }
}

impl InlineBackgroundFragment {
  /// The rounded-rect contour the fragment fills, with quarter-ellipse corners.
  pub fn path(&self) -> Vec<PathCommand> {
    const KAPPA: f32 = 4.0 / 3.0 * (std::f32::consts::SQRT_2 - 1.0);

    let InlineBackgroundFragment {
      x,
      y,
      width,
      height,
      radii,
      ..
    } = *self;
    let point = |x, y| Point { x, y };
    let [tl, tr, br, bl] = radii.map(|(rx, ry)| {
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
