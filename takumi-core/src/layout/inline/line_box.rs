//! The inline boxes open on one line, aligned by `vertical-align` as Blink's
//! `InlineLayoutStateStack::ApplyBaselineShift` aligns them, and the line box they make.
//!

// The alignment rules follow Blink, under the notice in LICENSE-CHROMIUM.

use std::{collections::HashMap, mem::take, rc::Rc};

use smallvec::{SmallVec, smallvec};

use super::{
  items::DecorationLink,
  text_fit::{LineFit, TextScale},
};
use crate::{
  context::RenderContext,
  layout_unit::LayoutUnit,
  resources::font::PrimaryFontMetrics,
  style::{ResolvedVerticalAlign, VerticalAlignKeyword},
};

/// How far a box reaches above and below its baseline, as Blink's `FontHeight`; positive
/// `descent` is below.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct FontHeight {
  pub(super) ascent: LayoutUnit,
  pub(super) descent: LayoutUnit,
}

impl FontHeight {
  /// Nothing yet, which any height unites to.
  pub(super) const EMPTY: Self = Self {
    ascent: LayoutUnit::MIN,
    descent: LayoutUnit::MIN,
  };

  /// A box with no height, what Blink's `FontHeight()` gives an empty box it must align.
  const ZERO: Self = Self {
    ascent: LayoutUnit::ZERO,
    descent: LayoutUnit::ZERO,
  };

  /// `ascent` and `descent` px in the nearest layout units.
  pub(super) fn nearest(ascent: f32, descent: f32) -> Self {
    Self {
      ascent: LayoutUnit::from_f32_round(ascent),
      descent: LayoutUnit::from_f32_round(descent),
    }
  }

  /// A font of `ascent` and `descent` px with its text sized by `scale`, as Blink's
  /// `InlineBoxState::ComputeTextMetrics` measures it: the rounded metrics of the font at its size,
  /// or the nearest layout units scaled when the text scales as it paints.
  pub(crate) fn text(ascent: f32, descent: f32, scale: TextScale) -> Self {
    match scale {
      TextScale::Paint(scale) if scale != 1.0 => Self {
        ascent: LayoutUnit::from_f32(LayoutUnit::from_f32_round(ascent).to_f32() * scale),
        descent: LayoutUnit::from_f32(LayoutUnit::from_f32_round(descent).to_f32() * scale),
      },
      scale => Self {
        ascent: LayoutUnit::from_f32((ascent * scale.font()).round()),
        descent: LayoutUnit::from_f32((descent * scale.font()).round()),
      },
    }
  }

  pub(super) fn is_empty(self) -> bool {
    self == Self::EMPTY
  }

  pub(super) fn unite(&mut self, other: Self) {
    self.ascent = self.ascent.max(other.ascent);
    self.descent = self.descent.max(other.descent);
  }

  /// Blink's `CalculateLeadingSpace` and `AddLeading`: the box grown to `line_height`, the half
  /// above floored to a whole pixel.
  pub(super) fn with_leading(self, line_height: LayoutUnit) -> Self {
    let leading = line_height - (self.ascent + self.descent);
    let above = LayoutUnit::from_int((leading / 2).floor());

    Self {
      ascent: self.ascent + above,
      descent: self.descent + (leading - above),
    }
  }

  /// Moves the box down by `delta`.
  fn moved(self, delta: LayoutUnit) -> Self {
    if self.is_empty() {
      return self;
    }

    Self {
      ascent: self.ascent - delta,
      descent: self.descent + delta,
    }
  }
}

/// The font a box aligns its children against.
#[derive(Clone, Copy, Debug)]
pub(crate) struct BoxFont {
  /// The computed `font-size`.
  pub(crate) size: f32,
  /// The primary font's metrics.
  pub(crate) metrics: Option<PrimaryFontMetrics>,
}

impl BoxFont {
  /// The font of the box `context` styles.
  pub(crate) fn of(context: &RenderContext) -> Self {
    Self {
      size: context.sizing.font_size,
      metrics: context.primary_font_metrics(&context.style, context.sizing.font_size),
    }
  }

  /// How far below its box's baseline text in this font paints on a line `fit` fits, the root
  /// inline box's text when `root`. Blink's `TextFragmentPainter` puts the text origin at the
  /// fragment's top, its box's text ascent above the baseline, plus the font's whole-pixel ascent
  /// as the text scales.
  pub(crate) fn text_origin_shift(self, fit: LineFit, root: bool) -> f32 {
    let Some(metrics) = self.metrics else {
      return 0.0;
    };

    if fit.scale == 1.0 {
      return 0.0;
    }

    let exact = metrics.exact;
    let painted = if fit.reshaped {
      LayoutUnit::from_f32((exact.ascent * fit.scale).round())
    } else {
      LayoutUnit::from_f32(metrics.ascent * fit.scale)
    };
    let text = FontHeight::text(exact.ascent, exact.descent, fit.text_scale(root));

    (painted - text.ascent).to_f32()
  }
}

/// What a box is, as the line box tree keys it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum BoxKey {
  /// An inline span, by its id.
  Span(usize),
  /// An atomic inline, by its inline box id.
  Atomic(u64),
}

/// One box open on the line.
struct OpenBox {
  key: Option<BoxKey>,
  parent: usize,
  /// Whether the box has closed, which puts its fragment in reach of `top` and `bottom`.
  closed: bool,
  /// Its own strut, then everything aligned inside it.
  metrics: FontHeight,
  align: ResolvedVerticalAlign,
  font: Option<BoxFont>,
  /// Children whose alignment waits for this box's metrics.
  pending: Vec<usize>,
  /// How far the box sits below its parent's baseline.
  shift: LayoutUnit,
  /// Its border box grown by its line height's leading, which `top` and `bottom` align to, or
  /// empty when it makes no box fragment of its own.
  box_metrics: FontHeight,
}

/// The boxes open on one line, the root inline box first.
pub(super) struct LineBoxTree {
  boxes: SmallVec<[OpenBox; 1]>,
  /// How `text-fit` fits the line.
  fit: LineFit,
  /// Each open box's position among `boxes`, by its key.
  indices: HashMap<BoxKey, usize>,
}

/// Where each box on a line sits once aligned.
#[derive(Clone, Debug, Default)]
pub(crate) struct LineBoxOffsets {
  offsets: HashMap<BoxKey, f32>,
}

impl LineBoxOffsets {
  /// How far the box `key` sits below the line's baseline.
  pub(super) fn of(&self, key: BoxKey) -> f32 {
    self.offsets.get(&key).copied().unwrap_or(0.0)
  }
}

impl LineBoxTree {
  /// A line `fit` fits holding only the root inline box, which starts with `strut`.
  pub(super) fn new(strut: FontHeight, font: BoxFont, fit: LineFit) -> Self {
    Self {
      fit,
      boxes: smallvec![OpenBox {
        key: None,
        parent: 0,
        closed: false,
        metrics: strut,
        align: ResolvedVerticalAlign::Keyword(VerticalAlignKeyword::Baseline),
        font: Some(font),
        pending: Vec::new(),
        shift: LayoutUnit::ZERO,
        box_metrics: FontHeight::EMPTY,
      }],
      indices: HashMap::new(),
    }
  }

  /// The box `key`, opening it inside `parent` with `strut` if it is not open yet.
  pub(super) fn open(
    &mut self,
    key: BoxKey,
    parent: usize,
    strut: FontHeight,
    align: ResolvedVerticalAlign,
    font: Option<BoxFont>,
  ) -> usize {
    if let Some(&index) = self.indices.get(&key) {
      return index;
    }

    self.boxes.push(OpenBox {
      key: Some(key),
      parent,
      closed: false,
      metrics: strut,
      align,
      font,
      pending: Vec::new(),
      shift: LayoutUnit::ZERO,
      box_metrics: FontHeight::EMPTY,
    });

    let index = self.boxes.len() - 1;

    self.indices.insert(key, index);
    index
  }

  /// The innermost span in `chain`, opening it and its ancestors with their struts, or the root
  /// when `chain` is empty.
  pub(super) fn open_chain(&mut self, chain: Option<&Rc<DecorationLink<'_>>>) -> usize {
    let Some(link) = chain else {
      return 0;
    };
    let decoration = &link.decoration;
    let key = BoxKey::Span(decoration.id);

    if let Some(&index) = self.indices.get(&key) {
      return index;
    }

    let parent = self.open_chain(link.parent.as_ref());
    let scale = self.fit.text_scale(false);
    let strut = decoration
      .strut
      .as_ref()
      .map_or(FontHeight::EMPTY, |strut| strut.height(scale));
    let index = self.open(
      key,
      parent,
      strut,
      decoration.vertical_align,
      Some(decoration.font),
    );
    let border = decoration.border.width;

    if (border.top > 0.0 || border.bottom > 0.0)
      && !strut.is_empty()
      && let Some(text) = decoration.font.metrics
    {
      // Blink's `MetricsForTopAndBottomAlign`: the box fragment's height less its padding, which
      // is the font's text height with the borders, grown by the leading its line height leaves.
      let text = FontHeight::text(text.exact.ascent, text.exact.descent, scale);
      let content = FontHeight {
        ascent: text.ascent + LayoutUnit::from_f32(border.top),
        descent: text.descent + LayoutUnit::from_f32(border.bottom),
      };

      self.boxes[index].box_metrics = content.with_leading(strut.ascent + strut.descent);
    }

    index
  }

  /// Grows the box `index` by content sitting on its baseline.
  pub(super) fn add(&mut self, index: usize, height: FontHeight) {
    self.boxes[index].metrics.unite(height);
  }

  /// Aligns every box, deepest first as each closes, and returns the root's height and where
  /// each box sits.
  pub(super) fn resolve(mut self) -> (FontHeight, LineBoxOffsets) {
    if self.boxes.len() == 1 {
      return (self.boxes[0].metrics, LineBoxOffsets::default());
    }

    let mut children = vec![Vec::new(); self.boxes.len()];
    let mut order = Vec::with_capacity(self.boxes.len() - 1);

    for index in 1..self.boxes.len() {
      children[self.boxes[index].parent].push(index);
    }
    close_order(&children, 0, &mut order);
    // Blink's `EndBoxState`: each box closes after its children, adds its fragment, then aligns.
    for index in order {
      self.boxes[index].closed = true;
      self.apply_pending(index);
      self.apply_baseline_shift(index);
    }
    self.apply_pending(0);

    let mut offsets = vec![LayoutUnit::ZERO; self.boxes.len()];

    for index in 1..self.boxes.len() {
      // A parent always precedes its children.
      offsets[index] = offsets[self.boxes[index].parent] + self.boxes[index].shift;
    }

    (
      self.boxes[0].metrics,
      LineBoxOffsets {
        offsets: self
          .boxes
          .iter()
          .zip(offsets)
          .filter_map(|(open, offset)| open.key.map(|key| (key, offset.to_f32())))
          .collect(),
      },
    )
  }

  /// Aligns the children of `index` that wait for its metrics, after Blink's
  /// `ApplyBaselineShift` resolving `pending_descendants`.
  fn apply_pending(&mut self, index: usize) {
    let pending = take(&mut self.boxes[index].pending);
    let text = self.boxes[index].font.and_then(|font| font.metrics);
    let scale = self.fit.text_scale(index == 0);
    let mut has_top_or_bottom = false;

    for &child in &pending {
      let metrics = &mut self.boxes[child].metrics;

      if metrics.is_empty() {
        *metrics = FontHeight::ZERO;
      }
    }

    for &child in &pending {
      let metrics = self.boxes[child].metrics;
      let shift = match self.boxes[child].align {
        // Blink's `TextTop`: the box's text ascent.
        ResolvedVerticalAlign::Keyword(VerticalAlignKeyword::TextTop) => {
          metrics.ascent
            - text.map_or(LayoutUnit::ZERO, |text| {
              FontHeight::text(text.exact.ascent, text.exact.descent, scale).ascent
            })
        }
        // Blink's `FixedDescent`.
        ResolvedVerticalAlign::Keyword(VerticalAlignKeyword::TextBottom) => {
          text.map_or(LayoutUnit::ZERO, |text| {
            LayoutUnit::from_f32_round(text.exact.descent)
          }) - metrics.descent
        }
        _ => {
          has_top_or_bottom = true;
          continue;
        }
      };

      self.place(child, index, metrics, shift);
    }

    if !has_top_or_bottom {
      return;
    }

    // `top` and `bottom` align to the subtree the other values already aligned, with every box
    // fragment closed so far where it sits, grown to a taller `top` or `bottom` box by its other
    // edge, as Blink's `MetricsForTopAndBottomAlign`.
    let mut aligned = self.boxes[index].metrics;

    for other in 1..self.boxes.len() {
      let open = &self.boxes[other];

      if !open.closed || open.box_metrics.is_empty() || Self::aligns_to_line_edge(open.align) {
        continue;
      }
      aligned.unite(open.box_metrics.moved(self.line_offset(other)));
    }

    let aligned = if aligned.is_empty() {
      FontHeight::ZERO
    } else {
      aligned
    };
    let mut max = aligned;

    for &child in &pending {
      let child = &self.boxes[child];
      let height = child.metrics.ascent + child.metrics.descent;

      if height <= max.ascent + max.descent {
        continue;
      }
      match child.align {
        ResolvedVerticalAlign::Keyword(VerticalAlignKeyword::Top) => {
          max = FontHeight {
            ascent: aligned.ascent,
            descent: height - aligned.ascent,
          };
        }
        ResolvedVerticalAlign::Keyword(VerticalAlignKeyword::Bottom) => {
          max = FontHeight {
            ascent: height - aligned.descent,
            descent: aligned.descent,
          };
        }
        _ => {}
      }
    }

    for &child in &pending {
      let metrics = self.boxes[child].metrics;
      let shift = match self.boxes[child].align {
        ResolvedVerticalAlign::Keyword(VerticalAlignKeyword::Top) => metrics.ascent - max.ascent,
        ResolvedVerticalAlign::Keyword(VerticalAlignKeyword::Bottom) => {
          max.descent - metrics.descent
        }
        _ => continue,
      };

      self.place(child, index, metrics, shift);
    }
  }

  /// Aligns the box `index` against its parent once its own content is aligned, or queues it on
  /// the box whose metrics it needs.
  fn apply_baseline_shift(&mut self, index: usize) {
    let parent = self.boxes[index].parent;
    let parent_font = self.boxes[parent].font;
    let metrics = self.boxes[index].metrics;
    let one = LayoutUnit::from_int(1);
    // Blink's `ComputedFontSizeAsFixed`, scaled with the line's text.
    let font_size = parent_font.map(|font| {
      let size = LayoutUnit::from_f32_round(font.size);

      if self.fit.scale == 1.0 {
        size
      } else {
        LayoutUnit::from_f32(size.to_f32() * self.fit.scale)
      }
    });
    let shift = match self.boxes[index].align {
      ResolvedVerticalAlign::Shift(px) => -LayoutUnit::from_f32(px),
      ResolvedVerticalAlign::Keyword(keyword) => match keyword {
        VerticalAlignKeyword::Baseline => LayoutUnit::ZERO,
        VerticalAlignKeyword::Sub => font_size.map_or(LayoutUnit::ZERO, |size| size / 5 + one),
        VerticalAlignKeyword::Super => font_size.map_or(LayoutUnit::ZERO, |size| -(size / 3 + one)),
        VerticalAlignKeyword::Middle => {
          let x_height = parent_font
            .and_then(|font| font.metrics)
            .and_then(|metrics| metrics.x_height)
            .map_or(LayoutUnit::ZERO, |x_height| {
              LayoutUnit::from_f32_round(x_height / 2.0)
            });

          (metrics.ascent - metrics.descent) / 2 - x_height
        }
        VerticalAlignKeyword::TextTop | VerticalAlignKeyword::TextBottom => {
          self.boxes[parent].pending.push(index);
          return;
        }
        VerticalAlignKeyword::Top | VerticalAlignKeyword::Bottom => {
          let mut ancestor = parent;

          while ancestor > 0
            && !matches!(
              self.boxes[ancestor].align,
              ResolvedVerticalAlign::Keyword(
                VerticalAlignKeyword::Top | VerticalAlignKeyword::Bottom
              )
            )
          {
            ancestor = self.boxes[ancestor].parent;
          }
          self.boxes[ancestor].pending.push(index);
          return;
        }
      },
    };

    self.place(index, parent, metrics, shift);
  }

  /// Whether `align` is `top` or `bottom`, which aligns to the line box's edges.
  fn aligns_to_line_edge(align: ResolvedVerticalAlign) -> bool {
    matches!(
      align,
      ResolvedVerticalAlign::Keyword(VerticalAlignKeyword::Top | VerticalAlignKeyword::Bottom)
    )
  }

  /// How far the box `index` sits below the line's baseline so far.
  fn line_offset(&self, index: usize) -> LayoutUnit {
    let mut offset = LayoutUnit::ZERO;
    let mut current = index;

    while current > 0 {
      offset += self.boxes[current].shift;
      current = self.boxes[current].parent;
    }

    offset
  }

  /// Moves the box `index` by `shift` and grows `into` by it.
  fn place(&mut self, index: usize, into: usize, metrics: FontHeight, shift: LayoutUnit) {
    let moved = metrics.moved(shift);

    self.boxes[index].shift = shift;
    self.boxes[into].metrics.unite(moved);
  }
}

/// Appends the boxes under `index` in the order they close, each after its children.
fn close_order(children: &[Vec<usize>], index: usize, order: &mut Vec<usize>) {
  for &child in &children[index] {
    close_order(children, child, order);
    order.push(child);
  }
}

#[cfg(test)]
mod tests {
  use super::{BoxFont, BoxKey, FontHeight, LineBoxTree, LineFit};
  use crate::style::{ResolvedVerticalAlign, VerticalAlignKeyword};

  const FONT: BoxFont = BoxFont {
    size: 20.0,
    metrics: None,
  };

  fn height(ascent: f32, descent: f32) -> FontHeight {
    FontHeight::nearest(ascent, descent)
  }

  fn keyword(keyword: VerticalAlignKeyword) -> ResolvedVerticalAlign {
    ResolvedVerticalAlign::Keyword(keyword)
  }

  #[test]
  fn sub_and_super_shift_by_the_parent_font_size() {
    let mut tree = LineBoxTree::new(height(16.0, 4.0), FONT, LineFit::NONE);

    tree.open(
      BoxKey::Span(0),
      0,
      height(8.0, 2.0),
      keyword(VerticalAlignKeyword::Sub),
      None,
    );
    tree.open(
      BoxKey::Span(1),
      0,
      height(8.0, 2.0),
      keyword(VerticalAlignKeyword::Super),
      None,
    );

    let (_, offsets) = tree.resolve();

    assert_eq!(offsets.of(BoxKey::Span(0)), 5.0);
    // Blink divides the layout units, so 20 / 3 truncates to 6.65625.
    assert_eq!(offsets.of(BoxKey::Span(1)), -7.65625);
  }

  #[test]
  fn top_and_bottom_boxes_taller_than_the_line_grow_its_other_edge() {
    let mut tree = LineBoxTree::new(height(10.0, 5.0), FONT, LineFit::NONE);

    tree.open(
      BoxKey::Atomic(0),
      0,
      height(30.0, 0.0),
      keyword(VerticalAlignKeyword::Top),
      None,
    );
    tree.open(
      BoxKey::Atomic(1),
      0,
      height(40.0, 0.0),
      keyword(VerticalAlignKeyword::Bottom),
      None,
    );

    let (line, offsets) = tree.resolve();

    assert_eq!(line, height(35.0, 5.0));
    assert_eq!(offsets.of(BoxKey::Atomic(0)), -5.0);
    assert_eq!(offsets.of(BoxKey::Atomic(1)), 5.0);
  }

  #[test]
  fn an_empty_box_aligns_as_a_zero_height_one() {
    let mut tree = LineBoxTree::new(height(10.0, 5.0), FONT, LineFit::NONE);

    tree.open(
      BoxKey::Span(0),
      0,
      FontHeight::EMPTY,
      keyword(VerticalAlignKeyword::Top),
      None,
    );

    let (line, offsets) = tree.resolve();

    assert_eq!(line, height(10.0, 5.0));
    assert_eq!(offsets.of(BoxKey::Span(0)), -10.0);
  }
}
