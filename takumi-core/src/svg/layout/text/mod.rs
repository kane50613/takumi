//! SVG text after Blink's `LayoutSVGText` and `SvgTextLayoutAlgorithm`: the characters of a `text`
//! element are shaped with takumi's fonts, positioned by the `x`, `y`, `dx`, `dy` and `rotate`
//! attributes, anchored, and painted as glyph outlines. Where Blink's text-on-path and
//! `textLength` details differ it follows usvg, and it draws no SVG-in-font glyphs.

mod attributes;
mod chunk;
mod paint;
mod position;
mod shape;

use std::rc::Rc;

use kurbo::CubicBez;

pub(super) use self::attributes::PositionLists;
use self::{
  chunk::TextTree,
  shape::{FontFaces, TextFont},
};
use super::{Container, Fill, Scope, Stroke, Styled, SvgLayout};
use crate::{
  geometry::PathCommand,
  style::{AlignmentBaseline, BaselineShift, DominantBaseline, TextAnchor},
};

/// The characters of a `text` element, ready to lay out.
struct Text {
  /// The `dx` of each character.
  dx: Vec<f32>,
  /// The `dy` of each character.
  dy: Vec<f32>,
  /// The rotation of each character in degrees.
  rotate: Vec<f32>,
  chunks: Vec<TextChunk>,
}

/// A run of text that is anchored together: it starts at an absolute position, at a `textPath`, or
/// at the start of the `text`.
struct TextChunk {
  /// The index among the characters of the `text` of each character of the chunk.
  characters: Vec<usize>,
  x: Option<f32>,
  y: Option<f32>,
  anchor: TextAnchor,
  spans: Vec<TextSpan>,
  flow: TextFlow,
  text: String,
}

/// Where a chunk's text runs.
#[derive(Clone)]
enum TextFlow {
  Linear,
  Path(TextPath),
}

/// The path a `textPath` sets text along and where on it the text starts.
#[derive(Clone)]
struct TextPath {
  start_offset: f32,
  path: Rc<TextPathShape>,
}

/// The outline of the shape a `textPath` names, with the transform of the shape applied.
pub(super) struct TextPathShape {
  commands: Vec<PathCommand>,
  /// The curves of the commands.
  curves: Vec<CubicBez>,
  /// Where each curve ends along the path.
  ends: Vec<f64>,
}

/// A part of a chunk that shares one style, as the bytes `start..end` of the chunk's text.
#[derive(Clone)]
struct TextSpan {
  start: usize,
  end: usize,
  style: Rc<SpanStyle>,
}

/// What the characters of one element are drawn with, shared by every span the element makes.
struct SpanStyle {
  fill: Option<Fill>,
  stroke: Option<Stroke>,
  stroke_first: bool,
  font: Option<TextFont>,
  decorations: Decorations,
  visible: bool,
  dominant_baseline: DominantBaseline,
  alignment_baseline: AlignmentBaseline,
  baseline_offset: BaselineOffset,
  /// The length the span is stretched to, from `textLength`.
  text_length: Option<f32>,
  length_adjust: LengthAdjust,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum LengthAdjust {
  Spacing,
  SpacingAndGlyphs,
}

/// The `baseline-shift` of an element and of the elements around it, which stack.
#[derive(Clone, Copy, Default)]
struct BaselineOffset {
  /// The lengths, in pixels.
  length: f32,
  /// The superscripts less the subscripts, each of which is half the height of the font.
  superscripts: i32,
}

impl BaselineOffset {
  /// The `baseline-shift` of an element stacked on that of the elements around it.
  fn new(styled: &Styled, around: Self) -> Self {
    let font_size = styled.sizing().font_size;

    match styled.style().svg_misc_data.baseline_shift {
      BaselineShift::Sub => Self {
        superscripts: around.superscripts - 1,
        ..around
      },
      BaselineShift::Super => Self {
        superscripts: around.superscripts + 1,
        ..around
      },
      BaselineShift::Length(length) => Self {
        length: around.length + length.to_px(styled.sizing(), font_size),
        ..around
      },
    }
  }
}

/// The fill and stroke a decoration line is painted with.
struct DecorationStyle {
  fill: Option<Fill>,
  stroke: Option<Stroke>,
}

/// The decoration lines an element is drawn with, shared with the elements they come from.
#[derive(Clone, Default)]
struct Decorations {
  underline: Option<Rc<DecorationStyle>>,
  overline: Option<Rc<DecorationStyle>>,
  line_through: Option<Rc<DecorationStyle>>,
}

impl SvgLayout<'_> {
  /// Lays out the `text` element into `parent`.
  pub(super) fn convert_text(&mut self, text: &Styled, scope: &Scope, parent: &mut Container) {
    let tree = TextTree::new(self, scope, text);
    let positions = tree.character_positions(self, scope);
    let mut faces = FontFaces::default();
    let chunks = tree.chunks(self, scope, &positions, &mut faces);

    if chunks.is_empty() {
      return;
    }

    let text = Text {
      dx: positions
        .iter()
        .map(|position| position.dx.unwrap_or(0.0))
        .collect(),
      dy: positions
        .iter()
        .map(|position| position.dy.unwrap_or(0.0))
        .collect(),
      rotate: tree.rotations(self),
      chunks,
    };

    if let Some(laid_out) = text.lay_out(&mut faces) {
      self.paint_text(&laid_out, parent);
    }
  }
}
