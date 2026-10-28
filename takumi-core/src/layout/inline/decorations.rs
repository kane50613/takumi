//! Text decoration lines and where skip-ink cuts them.

use crate::{
  context::BoxId,
  context::RenderContext,
  geometry::{ComputedLayout, Point},
  layout::intercept::{Spans, skip_ink_ranges},
  layout_unit::LayoutUnit,
  resources::{
    font::PrimaryFontMetrics,
    glyph::{ResolvedGlyph, ResolvedGlyphs, ResolvedOutlineGlyph},
  },
  style::{
    Affine, AppliedTextDecoration, AppliedTextDecorations, BoxDecorationBreak, Color,
    SizedTextDecorationInset, SizedTextDecorationThickness, TextDecorationLines,
    TextDecorationSkipInk, TextDecorationStyle, TextUnderlinePosition,
  },
};
use std::iter::repeat_n;

use super::runs::ShapedRun;

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
}

/// A text decoration line as Blink's `DecorationGeometry` holds it, unsnapped.
pub struct DecorationLine {
  /// Left end and top.
  pub origin: Point<f32>,
  /// Length.
  pub width: f32,
  /// Thickness.
  pub thickness: f32,
  /// Decoration color, already resolved against `current-color`.
  pub color: Color,
  /// Maps the border box into the device's drawing space.
  pub transform: Affine,
  /// Maps the border box onto the output's pixels.
  pub output: Affine,
  /// Where the border box sits in the space paint snaps to pixels in.
  pub paint_offset: Point<f32>,
  /// Whether the line paints above glyphs (line-through) vs below (under/overline).
  pub over: bool,
  /// Which decoration this is, so a backend can single one out.
  pub line: TextDecorationLines,
  /// How the line is drawn.
  pub style: TextDecorationStyle,
  /// The x-ranges `skip-ink` cuts out, sorted.
  pub skips: Spans,
}

/// How far a run's line of one decoration pulls in at its line-left and line-right ends, as
/// Blink's `TextDecorationInfo::ResolveDecorationInsets` resolves `text-decoration-inset`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) enum DecorationInset {
  /// Neither end pulls in.
  #[default]
  None,
  /// `auto` at the ends the run holds of its fragment, which the line's thickness sizes.
  Auto { left: bool, right: bool },
  /// Lengths in pixels.
  Edges { left: f32, right: f32 },
}

impl DecorationInset {
  /// The insets of a line `thickness` thick. Overflow keeps only the ends that push out, as Blink's
  /// conservative bounds do, since a positive end applies to some fragments only.
  fn edges(self, thickness: f32, for_overflow: bool) -> (f32, f32) {
    match self {
      Self::None => (0.0, 0.0),
      Self::Auto { left, right } => {
        let inset = SizedTextDecorationInset::auto_inset(thickness);

        (
          if left { inset } else { 0.0 },
          if right { inset } else { 0.0 },
        )
      }
      Self::Edges { left, right } if for_overflow => (left.min(0.0), right.min(0.0)),
      Self::Edges { left, right } => (left, right),
    }
  }

  /// The inset of a run that holds the line-left end of its fragment when `left` and the
  /// line-right end when `right`.
  pub(crate) fn for_run(self, left: bool, right: bool) -> Self {
    match self {
      Self::None => Self::None,
      Self::Auto {
        left: fragment_left,
        right: fragment_right,
      } => Self::Auto {
        left: fragment_left && left,
        right: fragment_right && right,
      },
      Self::Edges {
        left: left_inset,
        right: right_inset,
      } => Self::Edges {
        left: if left { left_inset } else { 0.0 },
        right: if right { right_inset } else { 0.0 },
      },
    }
  }
}

/// A leaf of an inline formatting context, in the order Blink's `InlineCursor` visits leaves.
pub(crate) struct DecoratedLeaf {
  /// The line it sits on.
  pub(crate) line: usize,
  /// Whether it is text; an atomic inline is not.
  pub(crate) text: bool,
  /// Whether it is a forced line break.
  pub(crate) line_break: bool,
  pub(crate) width: f32,
  pub(crate) decorations: AppliedTextDecorations,
  /// The box whose text it holds, which its other fragments share.
  pub(crate) object: Option<BoxId>,
  /// Whether its text runs right to left, which swaps its start and end.
  pub(crate) rtl: bool,
}

/// The leaves of an inline formatting context, which `text-decoration-inset` measures runs across.
pub(crate) struct DecoratedLeaves(pub(crate) Vec<DecoratedLeaf>);

impl DecoratedLeaves {
  /// The insets of each decoration of each leaf.
  pub(crate) fn insets(&self) -> Vec<Vec<DecorationInset>> {
    self
      .0
      .iter()
      .enumerate()
      .map(|(index, leaf)| {
        if !leaf.text {
          return Vec::new();
        }

        (0..leaf.decorations.as_slice().len())
          .map(|decoration| self.inset(index, decoration))
          .collect()
      })
      .collect()
  }

  /// Blink's `ResolveDecorationInsets` for the decoration at `decoration` of the leaf at `index`.
  fn inset(&self, index: usize, decoration: usize) -> DecorationInset {
    let leaf = &self.0[index];
    let applied = leaf.decorations.as_slice()[decoration];
    let (start, end) = match applied.inset {
      SizedTextDecorationInset::Auto => {
        return DecorationInset::Auto {
          left: true,
          right: true,
        };
      }
      inset if inset.is_zero() => return DecorationInset::None,
      SizedTextDecorationInset::Edges(start, end) => (start, end),
    };
    let (start, end) = if applied.box_decoration_break == BoxDecorationBreak::Slice {
      let (before, after) = self.run_metrics(index, decoration, &applied);
      let total = before + leaf.width + after;

      (
        inset_for_fragment(start.resolve(total), before),
        inset_for_fragment(end.resolve(total), after),
      )
    } else {
      let has_previous = self.same_on_line(index.checked_sub(1), leaf.line, decoration, &applied);
      let has_next = self.same_on_line(Some(index + 1), leaf.line, decoration, &applied);

      if has_previous && has_next {
        return DecorationInset::None;
      }

      (
        if has_previous {
          0.0
        } else {
          start.resolve(leaf.width)
        },
        if has_next {
          0.0
        } else {
          end.resolve(leaf.width)
        },
      )
    };
    let (left, right) = if leaf.rtl { (end, start) } else { (start, end) };

    DecorationInset::Edges { left, right }
  }

  /// Blink's `ComputeDecoratedRunMetrics`: the widths of the decorated run before and after the
  /// leaf at `index`, and at least of its box's other fragments before and after it.
  fn run_metrics(
    &self,
    index: usize,
    decoration: usize,
    applied: &AppliedTextDecoration,
  ) -> (f32, f32) {
    let continues = |leaf: &&DecoratedLeaf| leaf.continues(decoration, applied);
    let before: f32 = self.0[..index]
      .iter()
      .rev()
      .take_while(continues)
      .map(|leaf| leaf.width)
      .sum();
    let after: f32 = self.0[index + 1..]
      .iter()
      .take_while(continues)
      .map(|leaf| leaf.width)
      .sum();
    let object = self.0[index].object;
    let in_node = |leaves: &[DecoratedLeaf]| -> f32 {
      leaves
        .iter()
        .filter(|leaf| object.is_some() && leaf.object == object)
        .map(|leaf| leaf.width)
        .sum()
    };

    (
      before.max(in_node(&self.0[..index])),
      after.max(in_node(&self.0[index + 1..])),
    )
  }

  /// Blink's `HasSameDecorationAtIndexOnLine` for the text leaf at `index`, when it is on `line`.
  fn same_on_line(
    &self,
    index: Option<usize>,
    line: usize,
    decoration: usize,
    applied: &AppliedTextDecoration,
  ) -> bool {
    index
      .and_then(|index| self.0.get(index))
      .filter(|leaf| leaf.line == line && leaf.text && !leaf.line_break)
      .is_some_and(|leaf| leaf.continues(decoration, applied) && leaf.width > 0.0)
  }
}

impl DecoratedLeaf {
  /// Blink's `ContinuesDecoratedRun`: whether it is text decorated by `applied` at `decoration`.
  fn continues(&self, decoration: usize, applied: &AppliedTextDecoration) -> bool {
    self.text && self.decorations.as_slice().get(decoration) == Some(applied)
  }
}

/// Blink's `ResolveInsetForFragment`: what of an inset is left past `preceding` of the run.
fn inset_for_fragment(inset: f32, preceding: f32) -> f32 {
  if inset <= 0.0 {
    return if preceding == 0.0 { inset } else { 0.0 };
  }

  (inset - preceding).max(0.0)
}

/// Where a run's decoration lines paint: the border box's transform into the device's drawing
/// space, the device's transform onto its pixels, and where the border box sits in paint space.
#[derive(Clone, Copy, Debug)]
pub(crate) struct DecorationSpace {
  pub(crate) transform: Affine,
  pub(crate) device: Affine,
  pub(crate) paint_offset: Point<f32>,
}

/// The font a decorating box measures its decorations with, as Blink's `UsedFont`: its primary
/// font at its size, scaled by `text-fit`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct DecorationFont {
  metrics: PrimaryFontMetrics,
  size: f32,
  scale: f32,
}

impl DecorationFont {
  /// The primary font with `metrics` at `size`, unscaled.
  #[cfg(feature = "svg")]
  pub(crate) fn new(metrics: PrimaryFontMetrics, size: f32) -> Self {
    Self {
      metrics,
      size,
      scale: 1.0,
    }
  }

  /// The font of a box styled with `font`, scaled by `scale`, or of `run` when the box has no
  /// primary font.
  fn of(font: BoxFont, scale: f32, run: &ShapedRun) -> Self {
    Self {
      metrics: font.metrics.unwrap_or_else(|| run.primary_metrics()),
      size: font.size,
      scale,
    }
  }

  /// Blink's `UsedFont::FloatAscent`.
  fn ascent(self) -> f32 {
    self.metrics.ascent * self.scale
  }

  /// Blink's `TextDecorationThickness::Resolve`, at least a pixel.
  pub(crate) fn thickness(self, thickness: SizedTextDecorationThickness) -> f32 {
    let auto = self.size * self.scale / 10.0;

    match thickness {
      SizedTextDecorationThickness::Auto => auto,
      SizedTextDecorationThickness::FromFont => self
        .metrics
        .underline
        .map_or(auto, |underline| underline.thickness * self.scale),
      SizedTextDecorationThickness::Value(value) => value.round(),
    }
    .max(1.0)
  }

  /// The underline's top below the box's top, after Blink's
  /// `TextDecorationOffset::ComputeUnderlineOffset`.
  fn underline_offset(
    self,
    position: TextUnderlinePosition,
    offset: Option<f32>,
    thickness: f32,
  ) -> f32 {
    let style_offset = offset.unwrap_or(0.0);
    let auto = || {
      let gap = match offset {
        Some(_) => 0.0,
        None => (thickness / 2.0).ceil().max(1.0),
      };

      (self.ascent() + gap + style_offset.round()).trunc()
    };

    match position {
      TextUnderlinePosition::Auto => auto(),
      TextUnderlinePosition::FromFont => self.metrics.underline.map_or_else(auto, |underline| {
        ((self.metrics.ascent + underline.position) * self.scale + style_offset).round()
      }),
      TextUnderlinePosition::Under => {
        let offset = LayoutUnit::from_float_round(
          (self.metrics.ascent + self.metrics.em_descent) * self.scale,
        ) + LayoutUnit::from_float_round(style_offset);

        (offset.floor() + 1) as f32
      }
    }
  }
}

/// A box whose decoration a run paints, as Blink's `DecoratingBox`: the font it measures the
/// decoration with and where its baseline sits.
#[derive(Clone, Copy, Debug)]
pub(crate) struct DecoratingBox {
  font: DecorationFont,
  baseline: f32,
}

impl DecoratingBox {
  /// The box `font` measures, with its baseline at `baseline`.
  #[cfg(feature = "svg")]
  pub(crate) fn new(font: DecorationFont, baseline: f32) -> Self {
    Self { font, baseline }
  }

  /// Blink's `DecoratingBox::ContentOffsetInContainer`, the top of its content area.
  fn top(self) -> f32 {
    self.baseline - self.font.ascent()
  }

  /// The top of its `line`, `thickness` thick, over text whose top is `text_top`, as Blink's
  /// `TextDecorationInfo` places it.
  pub(crate) fn line_top(
    self,
    line: TextDecorationLines,
    (underline_position, underline_offset): (TextUnderlinePosition, Option<f32>),
    text_top: f32,
    thickness: f32,
  ) -> f32 {
    match line {
      TextDecorationLines::UNDERLINE => {
        self.top()
          + self
            .font
            .underline_offset(underline_position, underline_offset, thickness)
      }
      TextDecorationLines::OVERLINE => text_top - thickness.floor(),
      _ => text_top + 2.0 * self.font.ascent() / 3.0 - thickness / 2.0,
    }
  }
}

/// Where a run's decorations go, after Blink's `TextDecorationInfo`: in the border box, which
/// `text-fit` leaves unscaled, against the run's text and the boxes that decorate it.
#[derive(Clone, Debug, Default)]
pub(crate) struct DecorationPlacement {
  /// Maps the run's glyphs into the border box.
  glyphs: Affine,
  /// Where the run's text baseline lands.
  baseline: f32,
  /// Top of the run's text.
  text_top: f32,
  /// The decorated span's left end.
  left: f32,
  /// The decorated span's width.
  width: f32,
  /// The box each of the run's decorations sits against, outermost first.
  boxes: Vec<DecoratingBox>,
  /// How far each of the run's decorations pulls in at its ends, outermost first.
  insets: Vec<DecorationInset>,
}

impl DecorationPlacement {
  /// Blink's `TextDecorationInfo` for `run`, whose glyphs `glyphs` maps into the content box at
  /// `content`, its text in `text_font` scaled by `scale`: each of its decorations against the
  /// root box, or the spans of `spans`, outermost first, that decorate it. Each box comes with
  /// its baseline.
  pub(super) fn for_run(
    run: &ShapedRun,
    glyphs: Affine,
    content: Point<f32>,
    (text_font, scale): (BoxFont, f32),
    (root_font, line_baseline): (BoxFont, f32),
    spans: &[(BoxFont, f32)],
    insets: Vec<DecorationInset>,
  ) -> Self {
    let decorations = run.brush.decorations.as_slice();

    if decorations.is_empty() {
      return Self::default();
    }

    let root = DecoratingBox {
      font: DecorationFont::of(root_font, scale, run),
      baseline: content.y + line_baseline,
    };
    let spans = spans.iter().map(|&(font, baseline)| DecoratingBox {
      font: DecorationFont::of(font, 1.0, run),
      baseline: content.y + baseline,
    });
    let text = DecorationFont::of(text_font, scale, run);
    let baseline = content.y + run.baseline;
    let left = content.x + run.offset;
    let (left, _) = glyphs.transform_point(left, 0.0);
    let (right, _) = glyphs.transform_point(content.x + run.offset + run.advance, 0.0);

    Self {
      glyphs,
      baseline,
      text_top: baseline - text.ascent(),
      left,
      width: right - left,
      boxes: repeat_n(root, decorations.len().saturating_sub(spans.len()))
        .chain(spans)
        .collect(),
      insets,
    }
  }
}

impl ShapedRun {
  /// The outlines `skip-ink` gives way to, positioned from `origin`.
  fn ink_outlines<'g>(
    &self,
    resolved_glyphs: &'g ResolvedGlyphs,
    origin: Point<f32>,
  ) -> Vec<(Point<f32>, &'g ResolvedOutlineGlyph)> {
    self
      .glyphs
      .iter()
      .filter(|glyph| glyph.skips_ink)
      .filter_map(|glyph| {
        let ResolvedGlyph::Outline(outline) = resolved_glyphs.get(&glyph.id)?.as_ref() else {
          return None;
        };

        Some((
          Point {
            x: origin.x + glyph.x,
            y: origin.y + glyph.y,
          },
          outline,
        ))
      })
      .collect()
  }

  /// The lines the run's `text-decoration` paints at `placement`, cut by `skip-ink`, as Blink's
  /// `TextPainter::ClipDecorationsStripe` cuts them from the unscaled glyphs.
  pub(crate) fn decorations(
    &self,
    resolved_glyphs: &ResolvedGlyphs,
    layout: ComputedLayout,
    placement: &DecorationPlacement,
    space: DecorationSpace,
  ) -> Vec<DecorationLine> {
    let DecorationSpace {
      transform,
      device,
      paint_offset,
    } = space;
    let content = layout.content_box_offset();
    let mut lines = self.decoration_lines(
      placement,
      transform,
      device * transform,
      paint_offset,
      false,
    );

    if self.brush.decoration_skip_ink == TextDecorationSkipInk::None {
      return lines;
    }

    let glyph_baseline = content.y + self.baseline;
    let mut outlines = None;

    for decoration in lines.iter_mut().filter(|decoration| !decoration.over) {
      let band = decoration.bounds();
      let outlines = outlines.get_or_insert_with(|| self.ink_outlines(resolved_glyphs, content));

      decoration.skips = skip_ink_ranges(
        outlines.iter().copied(),
        glyph_baseline + band.top - placement.baseline,
        glyph_baseline + band.bottom - placement.baseline,
        decoration.thickness,
      )
      .into_iter()
      .map(|(start, end)| {
        (
          placement.glyphs.transform_point(start, 0.0).0,
          placement.glyphs.transform_point(end, 0.0).0,
        )
      })
      .collect();
    }
    lines
  }

  /// The run's decoration lines at `placement`, uncut, or the reach of their ink for overflow.
  pub(crate) fn decoration_lines(
    &self,
    placement: &DecorationPlacement,
    transform: Affine,
    output: Affine,
    paint_offset: Point<f32>,
    for_overflow: bool,
  ) -> Vec<DecorationLine> {
    if self.advance <= 0.0 {
      return Vec::new();
    }

    self
      .brush
      .decorations
      .as_slice()
      .iter()
      .zip(&placement.boxes)
      .enumerate()
      .flat_map(move |(index, (decoration, decorating))| {
        let thickness = decorating.font.thickness(decoration.thickness);
        let (left_inset, right_inset) = placement
          .insets
          .get(index)
          .copied()
          .unwrap_or_default()
          .edges(thickness, for_overflow);

        [
          (TextDecorationLines::UNDERLINE, false),
          (TextDecorationLines::OVERLINE, false),
          (TextDecorationLines::LINE_THROUGH, true),
        ]
        .into_iter()
        .filter(|(line, _)| decoration.line.contains(*line))
        .map(move |(line, over)| {
          let top = decorating.line_top(
            line,
            (decoration.underline_position, decoration.underline_offset),
            placement.text_top,
            thickness,
          );

          DecorationLine {
            origin: Point {
              x: placement.left + left_inset,
              y: top,
            },
            width: (placement.width - left_inset - right_inset).max(0.0),
            thickness,
            color: decoration.color,
            transform,
            output,
            paint_offset,
            over,
            line,
            style: decoration.style,
            skips: Spans::new(),
          }
        })
      })
      .collect()
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::{
    resources::font::{ExactFontMetrics, FontUnderline},
    style::{Length, SizingContext, TextDecorationInset},
    viewport::Viewport,
  };

  fn font(scale: f32) -> DecorationFont {
    DecorationFont {
      metrics: PrimaryFontMetrics {
        ascent: 40.0,
        descent: 10.0,
        line_gap: 0.0,
        x_height: None,
        exact: ExactFontMetrics {
          ascent: 40.0,
          descent: 10.0,
        },
        underline: Some(FontUnderline {
          position: 5.0,
          thickness: 2.0,
        }),
        em_descent: 20.0,
        zero_width: 25.0,
        has_zero_width: true,
        avg_char_width: 0.0,
        max_char_width: 0.0,
      },
      size: 100.0,
      scale,
    }
  }

  fn leaf(line: usize, width: f32, decoration: Option<AppliedTextDecoration>) -> DecoratedLeaf {
    DecoratedLeaf {
      line,
      text: true,
      line_break: false,
      width,
      decorations: decoration.into_iter().collect(),
      object: None,
      rtl: false,
    }
  }

  fn underline(
    inset: TextDecorationInset,
    box_decoration_break: BoxDecorationBreak,
  ) -> AppliedTextDecoration {
    AppliedTextDecoration {
      line: TextDecorationLines::UNDERLINE,
      style: TextDecorationStyle::default(),
      color: Color::black(),
      thickness: SizedTextDecorationThickness::Auto,
      underline_offset: None,
      underline_position: TextUnderlinePosition::Auto,
      inset: inset.sized(
        &SizingContext::builder()
          .viewport(Viewport::new((100, 100)))
          .build(),
      ),
      box_decoration_break,
    }
  }

  /// A sliced run resolves a percentage against its whole length and trims only its outer ends,
  /// as Blink's `ResolveDecorationInsets` does across fragments.
  #[test]
  fn a_sliced_run_insets_its_outer_ends_against_its_whole_length() {
    let decoration = underline(
      TextDecorationInset::Edges(Length::Percentage(25.0), Length::Percentage(25.0)),
      BoxDecorationBreak::Slice,
    );
    let leaves = DecoratedLeaves(vec![
      leaf(0, 40.0, Some(decoration)),
      leaf(0, 40.0, Some(decoration)),
      leaf(1, 80.0, Some(decoration)),
    ]);

    assert_eq!(
      leaves.insets(),
      [
        vec![DecorationInset::Edges {
          left: 40.0,
          right: 0.0
        }],
        vec![DecorationInset::Edges {
          left: 0.0,
          right: 0.0
        }],
        vec![DecorationInset::Edges {
          left: 0.0,
          right: 40.0
        }],
      ]
    );
  }

  /// A cloned run insets each line's fragment against its own length.
  #[test]
  fn a_cloned_run_insets_each_line_on_its_own() {
    let decoration = underline(
      TextDecorationInset::Edges(Length::Px(5.0), Length::Px(5.0)),
      BoxDecorationBreak::Clone,
    );
    let leaves = DecoratedLeaves(vec![
      leaf(0, 40.0, Some(decoration)),
      leaf(1, 40.0, Some(decoration)),
    ]);

    assert_eq!(
      leaves.insets(),
      [
        vec![DecorationInset::Edges {
          left: 5.0,
          right: 5.0
        }],
        vec![DecorationInset::Edges {
          left: 5.0,
          right: 5.0
        }],
      ]
    );
  }

  #[test]
  fn an_underline_sits_by_its_position_below_the_top() {
    // `auto` leaves a gap of half the thickness, at least a pixel, under the baseline.
    assert_eq!(
      font(1.0).underline_offset(TextUnderlinePosition::Auto, None, 1.0),
      41.0
    );
    assert_eq!(
      font(1.0).underline_offset(TextUnderlinePosition::Auto, None, 5.0),
      43.0
    );
    assert_eq!(
      font(1.0).underline_offset(TextUnderlinePosition::FromFont, None, 2.0),
      45.0
    );
    // A pixel past the bottom of the em box.
    assert_eq!(
      font(1.0).underline_offset(TextUnderlinePosition::Under, None, 2.0),
      61.0
    );
  }

  #[test]
  fn a_set_underline_offset_drops_the_auto_gap() {
    assert_eq!(
      font(1.0).underline_offset(TextUnderlinePosition::Auto, Some(3.0), 4.0),
      43.0
    );
    assert_eq!(
      font(1.0).underline_offset(TextUnderlinePosition::Under, Some(-4.0), 2.0),
      57.0
    );
  }

  #[test]
  fn text_fit_measures_decorations_with_the_scaled_font() {
    let font = font(1.5);
    let thickness = font.thickness(SizedTextDecorationThickness::Auto);

    assert_eq!(thickness, 15.0);
    assert_eq!(
      font.underline_offset(TextUnderlinePosition::Auto, None, thickness),
      68.0
    );
    assert_eq!(
      font.thickness(SizedTextDecorationThickness::Value(2.4)),
      2.0
    );
  }
}
