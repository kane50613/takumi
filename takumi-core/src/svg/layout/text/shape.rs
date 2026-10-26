//! Shaping a chunk's text into glyph clusters, after Blink's `InlineNode::ShapeText`: spans shape
//! with their own font and spacing, in the visual order the bidi algorithm gives.

use std::{rc::Rc, sync::Arc};

use skrifa::raw::types::F2Dot14;

use super::{TextChunk, TextSpan};
use crate::{
  fonts::{
    font::Font,
    font_baseline::FontBaseline,
    font_custom_platform_data::FontCustomPlatformData,
    shaping::{
      harfbuzz_shaper::HarfBuzzShaper, shape_result_spacing::ShapeResultSpacing,
      shape_result_view::ShapeResultView,
    },
    simple_font_data::SimpleFontData,
  },
  layout::inline::decorations::{DecoratingBox, DecorationFont},
  resources::font::{PrimaryFontMetrics, RunFace, RunSynthesis},
  style::{
    Affine, AlignmentBaseline, Direction, DominantBaseline, SizedTextDecorationThickness,
    TextDecorationLines, TextUnderlinePosition,
  },
  text::bidi_paragraph::{BidiParagraph, Run, direction_from_level},
};

/// A glyph that has already been positioned correctly.
///
/// The transform takes the font size into account, so applying it to the outline of the glyph is
/// all that is needed to display it.
#[derive(Clone)]
pub(super) struct PositionedGlyph {
  /// The transform of the glyph within its cluster, which places the marks of zalgo text.
  pub(super) glyph_transform: Affine,
  /// The transform of the cluster the glyph is part of.
  pub(super) cluster_transform: Affine,
  /// The transform of the span the glyph is part of.
  pub(super) span_transform: Affine,
  pub(super) id: u32,
  /// The font the glyph was shaped with.
  pub(super) font: Arc<ResolvedFont>,
}

impl PositionedGlyph {
  /// The transform of the outline of the glyph, which is in pixels at the font size.
  pub(super) fn transform(&self) -> Affine {
    self.span_transform * self.cluster_transform * self.glyph_transform
  }
}

/// The glyphs of one character cluster, which move together.
#[derive(Clone)]
pub(super) struct GlyphCluster {
  pub(super) byte_index: usize,
  pub(super) width: f32,
  pub(super) advance: f32,
  pub(super) ascent: f32,
  pub(super) descent: f32,
  pub(super) has_relative_shift: bool,
  pub(super) glyphs: Vec<PositionedGlyph>,
  pub(super) transform: Affine,
  pub(super) visible: bool,
}

impl GlyphCluster {
  pub(super) fn height(&self) -> f32 {
    self.ascent - self.descent
  }
}

/// A span's font: the shaping font, and the metrics of its primary face.
#[derive(Clone)]
pub(super) struct TextFont {
  pub(super) font: Rc<Font>,
  pub(super) primary: Arc<ResolvedFont>,
}

/// A face at the size and variation coordinates a glyph was shaped with, and its metrics.
#[derive(Debug)]
pub(super) struct ResolvedFont {
  pub(super) typeface: Arc<FontCustomPlatformData>,
  /// The normalized variation coordinates the face was shaped at.
  normalized_coords: Vec<F2Dot14>,
  size: f32,
  synthesis: RunSynthesis,
  metrics: PrimaryFontMetrics,
}

impl ResolvedFont {
  /// `font_data`'s face and metrics; none for a face without metrics.
  fn new(font_data: &SimpleFontData) -> Option<Self> {
    Some(Self {
      typeface: Arc::clone(&font_data.face.typeface),
      normalized_coords: font_data.normalized_coords(),
      size: font_data.size,
      synthesis: font_data.synthesis(),
      metrics: font_data.font_metrics()?,
    })
  }

  /// The face its glyphs resolve against.
  pub(super) fn face(&self) -> RunFace<'_> {
    RunFace {
      font_id: self.typeface.id,
      font_index: self.typeface.index,
      font_size: self.size,
      normalized_coords: &self.normalized_coords,
      synthesis: self.synthesis,
    }
  }

  pub(super) fn ascent(&self) -> f32 {
    self.metrics.ascent
  }

  /// Negative, below the baseline.
  pub(super) fn descent(&self) -> f32 {
    -self.metrics.descent
  }

  pub(super) fn height(&self) -> f32 {
    self.metrics.float_height()
  }

  /// How far the text moves down to put `baseline` where its alphabetic baseline was, as Blink
  /// aligns SVG text.
  pub(super) fn baseline_shift(&self, baseline: FontBaseline) -> f32 {
    self.metrics.ascent - self.metrics.float_ascent(baseline)
  }

  /// The center of a `line` decoration below the baseline, and its thickness, as Blink's
  /// `TextDecorationInfo` places it.
  pub(super) fn decoration_line(&self, line: TextDecorationLines) -> (f32, f32) {
    let font = DecorationFont::new(self.metrics, self.size);
    let thickness = font.thickness(SizedTextDecorationThickness::Auto);
    let top = DecoratingBox::new(font, 0.0).line_top(
      line,
      (TextUnderlinePosition::Auto, None),
      -self.ascent(),
      thickness,
    );

    (top + thickness / 2.0, thickness)
  }
}

/// The faces shaping picked, each resolved once.
#[derive(Default)]
pub(super) struct FontFaces {
  faces: Vec<(Rc<SimpleFontData>, Arc<ResolvedFont>)>,
}

impl FontFaces {
  pub(super) fn resolved(&mut self, font_data: &Rc<SimpleFontData>) -> Option<Arc<ResolvedFont>> {
    if let Some((_, resolved)) = self
      .faces
      .iter()
      .find(|(cached, _)| Rc::ptr_eq(cached, font_data))
    {
      return Some(Arc::clone(resolved));
    }

    let resolved = Arc::new(ResolvedFont::new(font_data)?);

    self
      .faces
      .push((Rc::clone(font_data), Arc::clone(&resolved)));
    Some(resolved)
  }
}

impl DominantBaseline {
  /// Blink's `ComputedStyle::GetFontBaseline`.
  pub(super) fn font_baseline(self) -> FontBaseline {
    match self {
      Self::Auto | Self::UseScript | Self::NoChange | Self::ResetSize | Self::Alphabetic => {
        FontBaseline::Alphabetic
      }
      Self::Ideographic => FontBaseline::IdeographicUnder,
      Self::Hanging => FontBaseline::Hanging,
      Self::Mathematical => FontBaseline::Math,
      Self::Central => FontBaseline::Central,
      Self::Middle => FontBaseline::XMiddle,
      Self::TextAfterEdge => FontBaseline::TextUnder,
      Self::TextBeforeEdge => FontBaseline::TextOver,
    }
  }
}

impl AlignmentBaseline {
  /// Blink's `InlineBoxState::ResetStyle` for SVG text; none where it takes the dominant baseline.
  pub(super) fn font_baseline(self) -> Option<FontBaseline> {
    match self {
      Self::Auto | Self::Baseline => None,
      Self::BeforeEdge | Self::TextBeforeEdge => Some(FontBaseline::TextOver),
      Self::Middle => Some(FontBaseline::XMiddle),
      Self::Central => Some(FontBaseline::Central),
      Self::AfterEdge | Self::TextAfterEdge => Some(FontBaseline::TextUnder),
      Self::Ideographic => Some(FontBaseline::IdeographicUnder),
      Self::Alphabetic => Some(FontBaseline::Alphabetic),
      Self::Hanging => Some(FontBaseline::Hanging),
      Self::Mathematical => Some(FontBaseline::Math),
    }
  }
}

/// A glyph and its metrics.
struct Glyph {
  id: u32,
  /// The position in bytes in the text of the chunk, which matches the glyph to its span.
  byte_index: usize,
  /// The glyph offset in pixels.
  dx: f32,
  /// The glyph offset in pixels, downward.
  dy: f32,
  /// The glyph advance in pixels.
  width: f32,
  font: Arc<ResolvedFont>,
}

impl TextChunk {
  /// The span that holds the byte, if any.
  pub(super) fn span_at(&self, byte_index: usize) -> Option<&TextSpan> {
    self
      .span_index_at(byte_index)
      .map(|index| &self.spans[index])
  }

  /// The index of the span that holds the byte, which the spans cover in order.
  pub(super) fn span_index_at(&self, byte_index: usize) -> Option<usize> {
    let index = self.spans.partition_point(|span| span.end <= byte_index);

    self.spans[index..]
      .first()
      .filter(|span| span.contains(byte_index))
      .map(|_| index)
  }

  /// Shapes the text at the 0x0 position into clusters, in visual order.
  pub(super) fn clusters(&self, faces: &mut FontFaces) -> Vec<GlyphCluster> {
    let glyphs = self.shape(faces);
    let mut clusters = Vec::new();
    let mut start = 0;

    while start < glyphs.len() {
      let byte_index = glyphs[start].byte_index;
      let end = glyphs[start..]
        .iter()
        .position(|glyph| glyph.byte_index != byte_index)
        .map_or(glyphs.len(), |offset| start + offset);

      if self.span_at(byte_index).is_some() {
        clusters.push(self.cluster(&glyphs[start..end], byte_index));
      }

      start = end;
    }

    clusters
  }

  /// The chunk's text in visual runs, ordered by the bidi algorithm with the controls of the
  /// elements around it, which the runs leave out.
  fn bidi_runs(&self) -> Vec<Run> {
    if self.controls.is_empty() {
      return BidiParagraph::new(&self.text, self.direction).get_visual_runs();
    }

    let mut text = String::with_capacity(self.text.len() + self.controls.len() * 3);
    // Where each control sits in `text`, which its length moves the characters after.
    let mut inserted = Vec::with_capacity(self.controls.len());
    let mut from = 0;

    for &(offset, control) in &self.controls {
      text.push_str(&self.text[from..offset]);
      inserted.push((text.len(), control.len_utf8()));
      text.push(control);
      from = offset;
    }

    text.push_str(&self.text[from..]);

    // The offset in the chunk's text of the offset `at` in `text`.
    let original = |at: usize| {
      at - inserted
        .iter()
        .take_while(|&&(position, _)| position < at)
        .map(|&(_, length)| length)
        .sum::<usize>()
    };

    BidiParagraph::new(&text, self.direction)
      .get_visual_runs()
      .into_iter()
      .map(|run| Run {
        start: original(run.start),
        end: original(run.end),
        level: run.level,
      })
      .filter(|run| run.start < run.end)
      .collect()
  }

  /// Shapes the chunk through `HarfBuzzShaper` and spaces it through `ShapeResultSpacing`, in
  /// visual order. Spans shape with their own font and spacing, so a bidi run is shaped piece by
  /// piece where the font changes.
  fn shape(&self, faces: &mut FontFaces) -> Vec<Glyph> {
    let shaper = HarfBuzzShaper::new(&self.text);
    let mut spacing = ShapeResultSpacing::new(&self.text, true);
    let mut glyphs = Vec::new();

    for run in self.bidi_runs() {
      let direction = direction_from_level(run.level);
      let first = self.spans.partition_point(|span| span.end <= run.start);
      let mut pieces: Vec<(&TextSpan, usize, usize)> = self.spans[first..]
        .iter()
        .take_while(|span| span.start < run.end)
        .filter_map(|span| {
          let start = span.start.max(run.start);
          let end = span.end.min(run.end);

          (start < end).then_some((span, start, end))
        })
        .collect();

      if direction == Direction::Rtl {
        pieces.reverse();
      }

      // Spans with one font shape as one run, so kerning joins the characters on either side of a
      // span boundary.
      let mut runs: Vec<(&Rc<Font>, usize, usize)> = Vec::new();

      for (span, start, end) in pieces {
        let Some(text_font) = &span.style.font else {
          continue;
        };
        let font = &text_font.font;

        match runs.last_mut() {
          Some((last, from, to))
            if last.description == font.description
              && if direction == Direction::Rtl {
                *from == end
              } else {
                *to == start
              } =>
          {
            if direction == Direction::Rtl {
              *from = start;
            } else {
              *to = end;
            }
          }
          _ => runs.push((font, start, end)),
        }
      }

      for (font, start, end) in runs {
        let mut result = shaper.shape(font, direction, start, end);

        if spacing.set_spacing_from(
          font.description.letter_spacing,
          font.description.word_spacing,
        ) {
          result.apply_spacing(&mut spacing, 0);
        }

        let mut shaped = Vec::new();
        let total_advance = ShapeResultView::create_from_result(&Rc::new(result)).for_each_glyph(
          0.0,
          |character_index, glyph, offset, advance, font_data| {
            shaped.push((character_index, glyph, offset, advance, font_data.cloned()));
          },
        );

        for (index, (character_index, glyph, offset, advance, font_data)) in
          shaped.iter().enumerate()
        {
          let next_advance = shaped
            .get(index + 1)
            .map_or(total_advance, |&(_, _, _, next_advance, _)| next_advance);
          let Some(resolved) = font_data
            .as_ref()
            .and_then(|font_data| faces.resolved(font_data))
          else {
            continue;
          };

          glyphs.push(Glyph {
            id: u32::from(*glyph),
            byte_index: *character_index as usize,
            dx: offset.x,
            dy: offset.y,
            width: next_advance - advance,
            font: resolved,
          });
        }
      }
    }

    glyphs
  }

  /// The cluster of `glyphs`, which all come from the character at `byte_index`.
  fn cluster(&self, glyphs: &[Glyph], byte_index: usize) -> GlyphCluster {
    let mut x = 0.0;
    let mut width = 0.0;
    let mut positioned = Vec::with_capacity(glyphs.len());

    for glyph in glyphs {
      // The first glyph of a cluster is offset from 0x0, the later ones from the current
      // position, so the advance is kept.
      positioned.push(PositionedGlyph {
        glyph_transform: Affine::translation(x + glyph.dx, glyph.dy),
        cluster_transform: Affine::IDENTITY,
        span_transform: Affine::IDENTITY,
        id: glyph.id,
        font: Arc::clone(&glyph.font),
      });

      x += glyph.width;

      if glyph.width > width {
        width = glyph.width;
      }
    }

    let font = &glyphs[0].font;

    GlyphCluster {
      byte_index,
      width,
      advance: x,
      ascent: font.ascent(),
      descent: font.descent(),
      has_relative_shift: false,
      transform: Affine::IDENTITY,
      glyphs: positioned,
      visible: true,
    }
  }
}

impl TextSpan {
  /// Whether the byte of the chunk's text belongs to the span.
  pub(super) fn contains(&self, byte_index: usize) -> bool {
    byte_index >= self.start && byte_index < self.end
  }
}
