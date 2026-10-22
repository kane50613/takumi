//! An inline formatting context's content once shaped and broken into lines, after Blink's
//! `FragmentItems`: glyph runs, inline boxes and span fragments, with nothing left to shape or
//! break. A run's glyphs sit in one flat array, eight bytes each for plain text, like the glyph
//! data of Blink's `ShapeResult`.

mod builder;

pub(super) use builder::line_span_fragments;

use std::{mem::take, ops::Range, rc::Rc};

use rustc_hash::FxHashMap;

use skrifa::FontRef;

use super::{
  background::{FragmentBackground, InlineBackgroundFragment},
  decorations::DecorationPlacement,
  items::ProcessedInlineSpan,
  metrics::VisualInlineBox,
  outline::InlineOutlineRect,
  runs::{
    InlineRunLayout, MeasuredInlineBox, MeasuredInlineRun, PositionedGlyph, PositionedInlineRun,
    ShapedRun, slice_text_at_char_boundaries,
  },
  text_fit::LineScaleState,
};
use crate::{
  context::RenderContext,
  geometry::{ComputedLayout, Point, Size},
  layout::{border::BorderProperties, tree::RenderNode},
  resources::{font::FontError, glyph::ResolvedGlyphs},
  style::Color,
};

/// The laid-out content of an inline formatting context.
pub struct FragmentItems {
  /// The content box it laid out in.
  pub(crate) content_box: ContentBox,
  /// The text layout shaped, then the ellipses and hyphens it generated.
  pub(crate) text: String,
  /// Where the generated text starts in `text`.
  generated_text_start: usize,
  /// Whether `text-overflow: ellipsis` cut the spans short, which collecting them again does not.
  pub(crate) ellipsized: bool,
  /// Each line box's top and bottom, below the content box's top.
  line_boxes: Vec<(f32, f32)>,
  /// The first and last shown line's baselines, below the content box's top; `None` when it has
  /// no lines.
  baselines: Option<(f32, f32)>,
  /// What its ink covers.
  pub(crate) ink: Ink,
  /// How far its spans' decorations, shadows and strokes reach past the glyphs.
  pub(crate) span_ink_reach: f32,
  text_items: Vec<TextItem>,
  /// The runs' fonts, metrics and brushes, which every run one span shapes in one face shares:
  /// each a run holding no glyphs at no place.
  faces: Vec<Rc<ShapedRun>>,
  /// The outlines of the glyphs each of [`Self::faces`] draws, resolved once for ink and paint.
  face_glyphs: Vec<ResolvedGlyphs>,
  glyphs: GlyphStore,
  /// In-flow and out-of-flow inline boxes, sorted by id.
  inline_boxes: Vec<VisualInlineBox>,
  outline_rects: Vec<InlineOutlineRect>,
  backgrounds: Vec<BackgroundItem>,
}

/// An [`InlineBackgroundFragment`] naming its span by decoration id instead of holding its node.
struct BackgroundItem {
  x: f32,
  y: f32,
  width: f32,
  height: f32,
  border: BorderProperties,
  color: Color,
  opacity: f32,
  baseline: f32,
  /// The strip's origin, the strip, and the fragment of the span's background.
  background: Option<(Point<f32>, ComputedLayout, ComputedLayout)>,
  span: usize,
}

impl BackgroundItem {
  fn of(fragment: InlineBackgroundFragment<'_>) -> Self {
    Self {
      x: fragment.x,
      y: fragment.y,
      width: fragment.width,
      height: fragment.height,
      border: fragment.border,
      color: fragment.color,
      opacity: fragment.opacity,
      baseline: fragment.baseline,
      background: fragment.background.map(|background| {
        (
          background.strip_origin,
          background.strip,
          background.fragment,
        )
      }),
      span: fragment.span,
    }
  }

  /// The fragment, its span being `owner`.
  fn resolve<'c>(&self, owner: &'c RenderNode) -> InlineBackgroundFragment<'c> {
    InlineBackgroundFragment {
      x: self.x,
      y: self.y,
      width: self.width,
      height: self.height,
      border: self.border,
      color: self.color,
      opacity: self.opacity,
      baseline: self.baseline,
      background: self
        .background
        .map(|(strip_origin, strip, fragment)| FragmentBackground {
          node: owner,
          strip_origin,
          strip,
          fragment,
        }),
      owner,
      span: self.span,
    }
  }
}

/// What an inline formatting context's ink covers.
#[derive(Clone, Copy, Default)]
pub(crate) struct Ink {
  /// Its runs, glyph ink and inline boxes, in content-box space.
  pub(crate) content: Option<InkBox>,
  /// Its decoration lines, in border-box space.
  pub(crate) decorations: Option<InkBox>,
}

/// A local box ink falls in, grown one placed rectangle at a time.
#[derive(Clone, Copy)]
pub(crate) struct InkBox {
  min: Point<f32>,
  max: Point<f32>,
}

impl InkBox {
  /// `ink` grown to take the rectangle at `origin` of `size`.
  fn include(ink: &mut Option<Self>, origin: Point<f32>, size: Size<f32>) {
    let max = Point {
      x: origin.x + size.width,
      y: origin.y + size.height,
    };

    *ink = Some(ink.map_or(Self { min: origin, max }, |ink| Self {
      min: Point {
        x: ink.min.x.min(origin.x),
        y: ink.min.y.min(origin.y),
      },
      max: Point {
        x: ink.max.x.max(max.x),
        y: ink.max.y.max(max.y),
      },
    }));
  }

  /// Its top-left and size.
  pub(crate) fn rect(self) -> (Point<f32>, Size<f32>) {
    (
      self.min,
      Size {
        width: self.max.x - self.min.x,
        height: self.max.y - self.min.y,
      },
    )
  }
}

/// What fragment items read of the layout they lay out in: where its content box sits in its
/// border box, and the content box's size.
#[derive(Clone, Copy, PartialEq)]
pub(crate) struct ContentBox {
  offset: Point<f32>,
  size: Size<f32>,
}

impl ContentBox {
  pub(crate) fn of(layout: ComputedLayout) -> Self {
    Self {
      offset: layout.content_box_offset(),
      size: layout.content_box_size(),
    }
  }
}

/// A glyph run on its line: Blink's `FragmentItem` of type `kText`.
struct TextItem {
  /// The face it draws in, among [`FragmentItems::faces`].
  face: u32,
  offset: f32,
  baseline: f32,
  advance: f32,
  text_range: Range<usize>,
  glyphs: RunGlyphs,
  placement: RunPlacement,
}

/// Where a run paints on its line, past where it was shaped.
struct RunPlacement {
  line_scale: LineScaleState,
  decoration_placement: DecorationPlacement,
  /// How far `position: relative` on its spans moves it.
  relative_offset: Point<f32>,
}

/// The glyphs of every run of a [`FragmentItems`], in two encodings.
#[derive(Default)]
struct GlyphStore {
  simple: Vec<SimpleGlyph>,
  full: Vec<FullGlyph>,
}

/// Where a run's glyphs sit in its [`GlyphStore`].
enum RunGlyphs {
  /// One glyph per cluster on one baseline, its clusters adjoining in logical order, the first
  /// starting at `cluster_start` and the last ending at `cluster_end`.
  Simple {
    glyphs: Range<u32>,
    y: f32,
    cluster_start: usize,
    cluster_end: usize,
  },
  /// Any other run. `clusters` is false when its glyphs could not be matched to clusters.
  Full { glyphs: Range<u32>, clusters: bool },
}

/// A glyph of a [`RunGlyphs::Simple`] run.
#[derive(Clone, Copy)]
struct SimpleGlyph {
  x: f32,
  id: u16,
  /// Where its cluster starts past the run's `cluster_start`, below [`SimpleGlyph::SKIPS_INK`].
  cluster: u16,
}

impl SimpleGlyph {
  /// The bit of `cluster` holding [`PositionedGlyph::skips_ink`].
  const SKIPS_INK: u16 = 1 << 15;

  /// `glyph`, its cluster starting `offset` past its run's, when it fits on the run's baseline `y`.
  fn of(glyph: &PositionedGlyph, offset: usize, y: f32) -> Option<Self> {
    let offset = u16::try_from(offset)
      .ok()
      .filter(|offset| offset & Self::SKIPS_INK == 0)?;
    let skips_ink = if glyph.skips_ink { Self::SKIPS_INK } else { 0 };

    (glyph.y.to_bits() == y.to_bits()).then_some(Self {
      x: glyph.x,
      id: u16::try_from(glyph.id).ok()?,
      cluster: offset | skips_ink,
    })
  }

  fn glyph(self, y: f32) -> PositionedGlyph {
    PositionedGlyph {
      id: u32::from(self.id),
      x: self.x,
      y,
      skips_ink: self.cluster & Self::SKIPS_INK != 0,
    }
  }

  fn cluster_offset(self) -> usize {
    usize::from(self.cluster & !Self::SKIPS_INK)
  }
}

/// A glyph of a [`RunGlyphs::Full`] run.
#[derive(Clone, Copy)]
struct FullGlyph {
  glyph: PositionedGlyph,
  cluster: (u32, u32),
}

impl GlyphStore {
  /// Moves `run`'s glyphs and cluster ranges into the store.
  fn push(&mut self, mut run: ShapedRun) -> (ShapedRun, RunGlyphs) {
    let glyphs = take(&mut run.glyphs);
    let clusters = take(&mut run.cluster_ranges);
    let stored = self
      .push_simple(&glyphs, &clusters)
      .unwrap_or_else(|| self.push_full(glyphs, &clusters));

    (run, stored)
  }

  /// Stores glyphs as [`RunGlyphs::Simple`], or nothing when they do not fit it.
  fn push_simple(
    &mut self,
    glyphs: &[PositionedGlyph],
    clusters: &[Range<usize>],
  ) -> Option<RunGlyphs> {
    let y = glyphs.first()?.y;
    let cluster_start = clusters.first()?.start;
    let cluster_end = clusters.last()?.end;
    let adjoining = clusters
      .windows(2)
      .all(|pair| pair[0].end == pair[1].start && pair[0].start < pair[1].start);

    if clusters.len() != glyphs.len() || !adjoining {
      return None;
    }
    let encoded = glyphs
      .iter()
      .zip(clusters)
      .map(|(glyph, cluster)| SimpleGlyph::of(glyph, cluster.start - cluster_start, y))
      .collect::<Option<Vec<_>>>()?;
    let start = self.simple.len() as u32;

    self.simple.extend(encoded);
    Some(RunGlyphs::Simple {
      glyphs: start..self.simple.len() as u32,
      y,
      cluster_start,
      cluster_end,
    })
  }

  fn push_full(&mut self, glyphs: Vec<PositionedGlyph>, clusters: &[Range<usize>]) -> RunGlyphs {
    let start = self.full.len() as u32;

    self
      .full
      .extend(glyphs.into_iter().enumerate().map(|(index, glyph)| {
        let cluster = clusters
          .get(index)
          .map_or((0, 0), |cluster| (cluster.start as u32, cluster.end as u32));

        FullGlyph { glyph, cluster }
      }));
    RunGlyphs::Full {
      glyphs: start..self.full.len() as u32,
      clusters: !clusters.is_empty(),
    }
  }

  /// A run's glyphs and their cluster ranges, as [`ShapedRun`] holds them.
  fn get(&self, glyphs: &RunGlyphs) -> (Vec<PositionedGlyph>, Vec<Range<usize>>) {
    match glyphs {
      RunGlyphs::Simple {
        glyphs,
        y,
        cluster_start,
        cluster_end,
      } => {
        let simple = &self.simple[glyphs.start as usize..glyphs.end as usize];
        let starts = simple
          .iter()
          .map(|glyph| cluster_start + glyph.cluster_offset());
        let ends = starts.clone().skip(1).chain([*cluster_end]);
        let positioned = simple.iter().map(|glyph| glyph.glyph(*y)).collect();

        (
          positioned,
          starts.zip(ends).map(|(start, end)| start..end).collect(),
        )
      }
      RunGlyphs::Full { glyphs, clusters } => {
        let full = &self.full[glyphs.start as usize..glyphs.end as usize];
        let positioned = full.iter().map(|glyph| glyph.glyph).collect();
        let clusters = if *clusters {
          full
            .iter()
            .map(|glyph| glyph.cluster.0 as usize..glyph.cluster.1 as usize)
            .collect()
        } else {
          Vec::new()
        };

        (positioned, clusters)
      }
    }
  }
}

impl FragmentItems {
  /// The items placed in the content box of `layout`, the box they were laid out in once its
  /// block size is known.
  pub(crate) fn in_content_box_of(mut self, layout: ComputedLayout) -> Self {
    self.content_box = ContentBox::of(layout);
    self
  }

  /// Frees the room its lists grew past their items, since a node keeps them for the render.
  fn shrink_to_fit(&mut self) {
    self.text.shrink_to_fit();
    self.line_boxes.shrink_to_fit();
    self.text_items.shrink_to_fit();
    self.faces.shrink_to_fit();
    self.face_glyphs.shrink_to_fit();
    self.glyphs.simple.shrink_to_fit();
    self.glyphs.full.shrink_to_fit();
    self.inline_boxes.shrink_to_fit();
    self.outline_rects.shrink_to_fit();
    self.backgrounds.shrink_to_fit();
  }

  /// Adds `run` as a text item, first passing it to `ink` with its face's glyphs resolved.
  fn push_run(
    &mut self,
    context: &RenderContext,
    run: ShapedRun,
    placement: RunPlacement,
    ink: impl FnOnce(&ShapedRun, &ResolvedGlyphs),
  ) {
    let existing = self
      .faces
      .iter()
      .rposition(|existing| existing.same_face(&run));
    let face = existing.unwrap_or_else(|| {
      self.face_glyphs.push(ResolvedGlyphs::default());
      self.face_glyphs.len() - 1
    });
    let resolved = &mut self.face_glyphs[face];

    if let Ok(font) = FontRef::from_index(run.font_data(), run.font_index) {
      context.fonts().resolve_glyphs_into(
        resolved,
        run.face(),
        font,
        run.glyphs.iter().map(|glyph| glyph.id),
      );
    }

    ink(&run, resolved);

    let (mut shaped_face, glyphs) = self.glyphs.push(run);
    let offset = take(&mut shaped_face.offset);
    let baseline = take(&mut shaped_face.baseline);
    let advance = take(&mut shaped_face.advance);
    let text_range = take(&mut shaped_face.text_range);

    if existing.is_none() {
      self
        .faces
        .push(context.inline_cache().share_face(shaped_face));
    }

    self.text_items.push(TextItem {
      face: face as u32,
      offset,
      baseline,
      advance,
      text_range,
      glyphs,
      placement,
    });
  }

  /// The text layout shaped.
  pub fn text(&self) -> &str {
    &self.text
  }

  /// Each line box's top and bottom, below the content box's top.
  pub fn line_boxes(&self) -> &[(f32, f32)] {
    &self.line_boxes
  }

  /// The in-flow and out-of-flow inline boxes, sorted by id.
  pub fn inline_boxes(&self) -> &[VisualInlineBox] {
    &self.inline_boxes
  }

  /// Each run's text, link, and box, and each inline box's border box, all in the content box of
  /// `node`, whose inline content these are.
  pub fn measure_runs(
    &self,
    node: &RenderNode,
  ) -> (Vec<MeasuredInlineRun<'_>>, Vec<MeasuredInlineBox>) {
    let spans = node.inline_spans(self);
    let mut runs = Vec::with_capacity(self.text_items.len());

    for item in &self.text_items {
      let face = &self.faces[item.face as usize];
      let span = face
        .brush
        .source_span_id
        .and_then(|id| spans.get(id as usize));
      let text_range = match span {
        Some(ProcessedInlineSpan::Text { byte_range, .. })
          if item.text_range.start < self.generated_text_start =>
        {
          item.text_range.start.max(byte_range.start)..item.text_range.end.min(byte_range.end)
        }
        _ => item.text_range.clone(),
      };
      let text = slice_text_at_char_boundaries(&self.text, text_range);

      if text.is_empty() {
        continue;
      }

      let line_scale = item.placement.line_scale;
      let scale = line_scale.scale;
      let origin = line_scale.layout_origin - self.content_box.offset;
      let top = item.baseline - face.metrics.ascent;
      let link = match span {
        Some(ProcessedInlineSpan::Text { link, .. }) => link.clone(),
        _ => None,
      };

      runs.push(MeasuredInlineRun {
        text,
        x: origin.x + (item.offset - origin.x) * scale + item.placement.relative_offset.x,
        y: origin.y + (top - origin.y) * scale + item.placement.relative_offset.y,
        width: item.advance * scale,
        height: (face.metrics.ascent + face.metrics.descent) * scale,
        link,
      });
    }

    // An atomic inline container measures as a box of the layout tree.
    let inline_boxes = self
      .inline_boxes
      .iter()
      .filter(|inline_box| {
        !matches!(
          spans.get(inline_box.id as usize),
          Some(ProcessedInlineSpan::Box(item)) if item.render_node.is_inline_content_box()
        )
      })
      .map(|inline_box| MeasuredInlineBox {
        x: inline_box.x,
        y: inline_box.y,
        width: inline_box.width,
        height: inline_box.height,
      })
      .collect();

    (runs, inline_boxes)
  }

  /// The runs resolved for painting at `layout` in `context`, among `spans`: their glyphs'
  /// outlines, the paint offset of the box they paint in, and the nodes of their spans.
  pub(crate) fn resolve_runs<'c>(
    &self,
    spans: &[ProcessedInlineSpan<'c>],
    context: &RenderContext,
    layout: ComputedLayout,
  ) -> Result<InlineRunLayout<'c>, FontError> {
    let paint_offset = context.box_paint_offset(layout);
    let mut runs = Vec::with_capacity(self.text_items.len());

    for item in &self.text_items {
      let (glyphs, cluster_ranges) = self.glyphs.get(&item.glyphs);
      let glyph_run = ShapedRun {
        glyphs,
        cluster_ranges,
        offset: item.offset,
        baseline: item.baseline,
        advance: item.advance,
        text_range: item.text_range.clone(),
        ..ShapedRun::clone(&self.faces[item.face as usize])
      };
      FontRef::from_index(glyph_run.font_data(), glyph_run.font_index)
        .map_err(|_| FontError::InvalidFontIndex)?;

      runs.push(PositionedInlineRun {
        glyph_run,
        resolved_glyphs: self.face_glyphs[item.face as usize].clone(),
        line_scale: item.placement.line_scale,
        decoration_placement: item.placement.decoration_placement.clone(),
        paint_offset,
        relative_offset: item.placement.relative_offset,
        #[cfg(feature = "paint-tree")]
        index: runs.len(),
      });
    }

    let background_fragments = if self.backgrounds.is_empty() {
      Vec::new()
    } else {
      let owners = span_owners(spans);

      self
        .backgrounds
        .iter()
        .filter_map(|item| Some(item.resolve(*owners.get(&item.span)?)))
        .collect()
    };

    Ok(InlineRunLayout {
      runs,
      inline_boxes: self.inline_boxes.clone(),
      outline_rects: self.outline_rects.clone(),
      background_fragments,
    })
  }
}

/// The node of every span `spans` open, by decoration id.
fn span_owners<'c>(spans: &[ProcessedInlineSpan<'c>]) -> FxHashMap<usize, &'c RenderNode> {
  spans
    .iter()
    .filter_map(ProcessedInlineSpan::chain)
    .flat_map(|link| link.ancestors())
    .map(|link| (link.decoration.id, link.decoration.owner))
    .collect()
}
