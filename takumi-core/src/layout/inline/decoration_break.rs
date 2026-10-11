//! `box-decoration-break: clone` on inline spans: the edges a span repeats on every line it wraps
//! onto, after Blink's `LineBreaker::RecalcClonedBoxDecorations`.

use std::ops::Range;

use parley::BreakLines;

use super::{
  InlineBrush, InlineLayout,
  items::{DecorationLink, ProcessedInlineSpan},
};
use crate::style::BoxDecorationBreak;

/// The spans of an inline layout that clone their edges onto every line.
pub(crate) struct ClonedSpans {
  spans: Vec<ClonedSpan>,
}

/// A span that clones its edges, and where its content sits in the layout's text.
struct ClonedSpan {
  id: usize,
  /// The spans around it, innermost first, itself excluded.
  ancestors: Vec<usize>,
  /// The bytes its content covers.
  range: Range<usize>,
  /// Its margin, border and padding on the inline-start side.
  start: f32,
  /// Its margin, border and padding on the inline-end side.
  end: f32,
  /// Its border and padding alone on each side, which its own fragments cover.
  inner_start: f32,
  inner_end: f32,
}

/// The cloned edges of one line: the spans whose start edge opens it and whose end edge closes it.
#[derive(Clone, Default, PartialEq)]
pub(crate) struct LineEdges {
  /// The ids of the spans open at the line's start.
  pub(crate) start: Vec<usize>,
  /// The ids of the spans still open at the line's end.
  pub(crate) end: Vec<usize>,
  /// How far the start edges push the line's content in.
  pub(crate) start_width: f32,
  /// How far the end edges reach past the line's content.
  pub(crate) end_width: f32,
}

impl LineEdges {
  /// Narrows line `line` of `edges`, which `breaker` breaks next, by its start edges, and moves the
  /// line in past the edges on the paragraph's start side so it aligns between them.
  pub(crate) fn reserve_on(
    edges: &[Self],
    line: u32,
    breaker: &mut BreakLines<'_, InlineBrush>,
    rtl: bool,
  ) {
    let Some(edges) = edges.get(line as usize) else {
      return;
    };
    let state = breaker.state_mut();
    let shift = if rtl {
      edges.end_width
    } else {
      edges.start_width
    };

    state.set_line_x(state.line_x() + shift);
    state.set_line_max_advance((state.line_max_advance() - edges.start_width).max(0.0));
  }
}

impl ClonedSpans {
  /// The cloning spans of `spans`, or `None` when no span clones its edges.
  pub(crate) fn of(spans: &[ProcessedInlineSpan<'_>]) -> Option<Self> {
    let mut cloned: Vec<ClonedSpan> = Vec::new();

    for span in spans {
      let (chain, range) = match span {
        ProcessedInlineSpan::Text {
          byte_range,
          decorations,
          ..
        } => (decorations.as_deref(), byte_range.clone()),
        ProcessedInlineSpan::Box(item) => {
          let index = item.inline_box.index;

          (item.decorations.as_deref(), index..index)
        }
        ProcessedInlineSpan::Spacer {
          inline_box,
          decorations,
        } => (decorations.as_deref(), inline_box.index..inline_box.index),
        ProcessedInlineSpan::DirectionMark { .. } => continue,
      };

      for link in chain.into_iter().flat_map(DecorationLink::ancestors) {
        let decoration = &link.decoration;

        if decoration.owner.context.style.box_data.box_decoration_break != BoxDecorationBreak::Clone
        {
          continue;
        }
        match cloned.iter_mut().find(|span| span.id == decoration.id) {
          Some(span) => {
            span.range.start = span.range.start.min(range.start);
            span.range.end = span.range.end.max(range.end);
          }
          None => cloned.push(ClonedSpan::of(link, range.clone())),
        }
      }
    }

    (!cloned.is_empty()).then_some(Self { spans: cloned })
  }

  /// The cloned edges of the line holding the bytes `line`.
  pub(crate) fn line_edges(&self, line: Range<usize>) -> LineEdges {
    let mut edges = LineEdges::default();

    for span in &self.spans {
      let range = &span.range;

      if range.start < line.start && range.end > line.start {
        edges.start.push(span.id);
        edges.start_width += span.start;
      }
      if range.start < line.end && range.end > line.end {
        edges.end.push(span.id);
        edges.end_width += span.end;
      }
    }

    edges
  }

  /// How far `id`'s fragment on a line with `edges` reaches past the line's content at its start
  /// and end: its own border and padding, and the whole edges of the cloning spans inside it.
  fn fragment_reach(&self, id: usize, edges: &LineEdges) -> (Option<f32>, Option<f32>) {
    let Some(span) = self.spans.iter().find(|span| span.id == id) else {
      return (None, None);
    };
    let inside = |side: &[usize], edge: fn(&ClonedSpan) -> f32| {
      side
        .iter()
        .filter_map(|id| self.spans.iter().find(|span| span.id == *id))
        .filter(|inner| inner.ancestors.contains(&span.id))
        .map(edge)
        .sum::<f32>()
    };
    let start = edges
      .start
      .contains(&id)
      .then(|| span.inner_start + inside(&edges.start, |span| span.start));
    let end = edges
      .end
      .contains(&id)
      .then(|| span.inner_end + inside(&edges.end, |span| span.end));

    (start, end)
  }

  /// Whether `id` clones its edges.
  fn clones(&self, id: usize) -> bool {
    self.spans.iter().any(|span| span.id == id)
  }
}

/// The cloning spans of a paragraph broken into lines, and the edges each line repeats.
pub(crate) struct ClonedLines {
  spans: ClonedSpans,
  lines: Vec<LineEdges>,
}

impl ClonedLines {
  /// The cloned edges of `layout`'s lines, or `None` when none of `spans` clones its edges.
  pub(crate) fn of(spans: &[ProcessedInlineSpan<'_>], layout: &InlineLayout) -> Option<Self> {
    let spans = ClonedSpans::of(spans)?;
    let lines = layout
      .lines()
      .map(|line| spans.line_edges(line.text_range()))
      .collect();

    Some(Self { spans, lines })
  }

  /// Whether span `id` clones its edges.
  pub(crate) fn clones(&self, id: usize) -> bool {
    self.spans.clones(id)
  }

  /// How far span `id`'s fragment on line `line` reaches past the line's content at its start and
  /// end, where it repeats an edge.
  pub(crate) fn reach(&self, id: usize, line: usize) -> (Option<f32>, Option<f32>) {
    self
      .lines
      .get(line)
      .map_or((None, None), |edges| self.spans.fragment_reach(id, edges))
  }
}

impl ClonedSpan {
  fn of(link: &DecorationLink<'_>, range: Range<usize>) -> Self {
    let decoration = &link.decoration;
    let node = decoration.owner;
    let margin = node.margin_px();
    let border = node.border_px();
    let padding = node.padding_px();
    let direction = decoration.direction;
    let (margin_start, margin_end) = direction.inline_sides(margin.left, margin.right);
    let (inner_start, inner_end) =
      direction.inline_sides(border.left + padding.left, border.right + padding.right);

    Self {
      id: decoration.id,
      ancestors: link
        .ancestors()
        .skip(1)
        .map(|link| link.decoration.id)
        .collect(),
      range,
      start: margin_start + inner_start,
      end: margin_end + inner_end,
      inner_start,
      inner_end,
    }
  }
}
