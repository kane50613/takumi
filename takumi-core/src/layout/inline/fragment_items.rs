//! An inline formatting context's content once shaped and broken into lines, after Blink's
//! `FragmentItems`: glyph runs, inline boxes and span fragments, with nothing left to shape or
//! break. A run's glyphs sit in one flat array, eight bytes each for plain text, like the glyph
//! data of Blink's `ShapeResult`.

use std::{collections::HashMap, convert::Infallible, mem::take, ops::Range};

use skrifa::FontRef;

use super::{
  BuiltInlineLayout, PlacedItem,
  background::InlineBackgroundFragment,
  decorations::DecorationPlacement,
  metrics::VisualInlineBox,
  outline::InlineOutlineRect,
  runs::{
    InlineRunLayout, PositionedGlyph, PositionedInlineRun, ShapedRun, glyph_clusters,
    glyph_skips_ink,
  },
  text_fit::LineScaleState,
};
use crate::{context::RenderContext, geometry::ComputedLayout, resources::font::FontError};

/// The laid-out content of an inline formatting context.
pub(crate) struct FragmentItems<'c> {
  text_items: Vec<TextItem>,
  glyphs: GlyphStore,
  /// In-flow and out-of-flow inline boxes, sorted by id.
  inline_boxes: Vec<VisualInlineBox>,
  outline_rects: Vec<InlineOutlineRect>,
  background_fragments: Vec<InlineBackgroundFragment<'c>>,
}

/// A glyph run on its line: Blink's `FragmentItem` of type `kText`.
struct TextItem {
  /// The run, its glyphs and cluster ranges moved into the [`GlyphStore`].
  run: ShapedRun,
  glyphs: RunGlyphs,
  line_scale: LineScaleState,
  static_inline_prefix: f32,
  baseline_shift: f32,
  decoration_placement: DecorationPlacement,
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

impl<'c> BuiltInlineLayout<'c> {
  /// The laid-out content as fragment items.
  pub(crate) fn fragment_items(&self, layout: ComputedLayout) -> FragmentItems<'c> {
    let mut items = FragmentItems {
      text_items: Vec::new(),
      glyphs: GlyphStore::default(),
      inline_boxes: Vec::new(),
      outline_rects: Vec::new(),
      background_fragments: Vec::new(),
    };
    let mut decoration_coverage = self.decoration_coverage();
    let mut inline_boxes = HashMap::new();
    let content = layout.content_box_offset();

    let Ok(()) = self.walk_items::<Infallible>(layout, |line, item| {
      let setup = &line.setup;

      self.cover(&mut decoration_coverage, content, line, &item);

      match item {
        PlacedItem::Run {
          glyph_run,
          static_inline_prefix,
          hanging,
          stretch,
        } => {
          // A run carrying only the direction mark paints nothing; a run the
          // mark's cluster merged into (emoji sequences) paints as the first
          // real span.
          let mut brush = glyph_run.style().brush.clone();

          if brush.is_direction_mark {
            if glyph_run.advance() == 0.0 {
              return Ok(());
            }
            brush.is_direction_mark = false;
          }

          let mut glyphs: Vec<PositionedGlyph> = glyph_run
            .positioned_glyphs()
            .map(|g| PositionedGlyph {
              id: g.id,
              x: g.x,
              y: g.y,
              skips_ink: true,
            })
            .collect();
          let clusters = glyph_clusters(&glyph_run, &glyphs);

          for (glyph, cluster) in glyphs.iter_mut().zip(&clusters) {
            glyph.skips_ink = !cluster.emoji && glyph_skips_ink(&self.text, cluster.range.clone());
          }
          let cluster_ranges = clusters.into_iter().map(|cluster| cluster.range).collect();
          let shaped = ShapedRun::of(&glyph_run, glyphs, hanging, &stretch, brush, cluster_ranges);
          let decoration_placement = self.decoration_placement(
            line,
            &shaped,
            glyph_run.style().brush.source_span_id,
            static_inline_prefix,
            layout,
          );
          let baseline_shift = self.run_baseline_shift(line, &glyph_run);

          let (run, glyphs) = items.glyphs.push(shaped);

          items.text_items.push(TextItem {
            run,
            glyphs,
            line_scale: setup.run_scale(baseline_shift),
            static_inline_prefix,
            baseline_shift,
            decoration_placement,
          });
        }
        PlacedItem::Box(inline_box) => {
          inline_boxes.insert(inline_box.id, inline_box);
        }
        PlacedItem::Placeholder(_) => {}
      }
      Ok(())
    });

    items.inline_boxes = self.with_floats(inline_boxes);
    (items.background_fragments, items.outline_rects) = decoration_coverage.into_fragments();
    items
  }
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

impl<'c> FragmentItems<'c> {
  /// How many glyphs it stores as simple and as full glyphs.
  #[cfg(test)]
  pub(super) fn glyph_counts(&self) -> (usize, usize) {
    (self.glyphs.simple.len(), self.glyphs.full.len())
  }

  /// The runs resolved for painting at `layout` in `context`: their glyphs' outlines and the
  /// paint offset of the box they paint in.
  pub(crate) fn resolve_runs(
    self,
    context: &RenderContext,
    layout: ComputedLayout,
  ) -> Result<InlineRunLayout<'c>, FontError> {
    let Self {
      text_items,
      glyphs,
      inline_boxes,
      outline_rects,
      background_fragments,
    } = self;
    let paint_offset = context.box_paint_offset(layout);
    let mut runs = Vec::with_capacity(text_items.len());

    for item in text_items {
      let (positioned, cluster_ranges) = glyphs.get(&item.glyphs);
      let glyph_run = ShapedRun {
        glyphs: positioned,
        cluster_ranges,
        ..item.run
      };
      let font = FontRef::from_index(glyph_run.font_data(), glyph_run.font_index)
        .map_err(|_| FontError::InvalidFontIndex)?;
      let resolved_glyphs = context.fonts().with_context(|fonts| {
        fonts.resolve_glyphs(
          glyph_run.face(),
          font,
          glyph_run.glyphs.iter().map(|glyph| glyph.id),
        )
      });

      runs.push(PositionedInlineRun {
        glyph_run,
        resolved_glyphs,
        line_scale: item.line_scale,
        static_inline_prefix: item.static_inline_prefix,
        baseline_shift: item.baseline_shift,
        decoration_placement: item.decoration_placement,
        paint_offset,
        #[cfg(feature = "paint-tree")]
        index: runs.len(),
      });
    }

    Ok(InlineRunLayout {
      runs,
      inline_boxes,
      outline_rects,
      background_fragments,
    })
  }
}
