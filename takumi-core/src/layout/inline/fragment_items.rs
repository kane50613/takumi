//! An inline formatting context's content once shaped and broken into lines, after Blink's
//! `FragmentItems`: glyph runs, inline boxes and span fragments, with nothing left to shape or
//! break. A run's glyphs sit in one flat array, eight bytes each for plain text, like the glyph
//! data of Blink's `ShapeResult`.

use std::{collections::HashMap, convert::Infallible, mem::take, ops::Range, rc::Rc};

use skrifa::FontRef;

use super::{
  BuiltInlineLayout, LineSetup, PlacedItem,
  background::{FragmentBackground, InlineBackgroundFragment},
  decorations::DecorationPlacement,
  glyph_run_rect,
  items::{DecorationLink, ProcessedInlineSpan},
  metrics::VisualInlineBox,
  outline::InlineOutlineRect,
  runs::{
    HangingWhitespace, InlineRunLayout, PositionedGlyph, PositionedInlineRun, ShapedRun,
    glyph_clusters, glyph_skips_ink,
  },
  text_fit::LineScaleState,
};
use crate::{
  context::RenderContext,
  geometry::{ComputedLayout, Point, Size},
  layout::{border::BorderProperties, tree::RenderNode},
  resources::font::FontError,
  scene::text_ink_reach,
  style::Affine,
  style::Color,
};

/// The laid-out content of an inline formatting context.
pub struct FragmentItems {
  /// The content box it laid out in.
  pub(crate) content_box: ContentBox,
  /// The text layout shaped.
  pub(crate) text: String,
  /// Whether `text-overflow: ellipsis` cut the spans short, which collecting them again does not.
  pub(crate) ellipsized: bool,
  /// Each line box's top and bottom, below the content box's top.
  line_boxes: Vec<(f32, f32)>,
  /// What its ink covers.
  pub(crate) ink: Ink,
  /// How far its spans' decorations, shadows and strokes reach past the glyphs.
  pub(crate) span_ink_reach: f32,
  text_items: Vec<TextItem>,
  /// The runs' fonts, metrics and brushes, which every run one span shapes in one face shares:
  /// each a run holding no glyphs at no place.
  faces: Vec<Rc<ShapedRun>>,
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
  hanging: HangingWhitespace,
  text_range: Range<usize>,
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

impl BuiltInlineLayout<'_> {
  /// The laid-out content as fragment items.
  pub(crate) fn fragment_items(
    &self,
    layout: ComputedLayout,
    context: &RenderContext,
  ) -> FragmentItems {
    let mut ink = None;
    let mut decoration_ink = None;
    let mut items = FragmentItems {
      content_box: ContentBox::of(layout),
      text: self.text.clone(),
      ellipsized: self.ellipsized,
      line_boxes: self.line_boxes().collect(),
      ink: Ink::default(),
      span_ink_reach: self.span_ink_reach(),
      text_items: Vec::new(),
      faces: Vec::new(),
      glyphs: GlyphStore::default(),
      inline_boxes: Vec::new(),
      outline_rects: Vec::new(),
      backgrounds: Vec::new(),
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
          let baseline_shift = self.run_baseline_shift(line, &glyph_run);
          let (origin, size) = glyph_run_rect(&glyph_run, hanging, &stretch, baseline_shift);
          let (origin, size) = setup.scale_rect(origin, size, static_inline_prefix, baseline_shift);

          InkBox::include(&mut ink, origin, size);

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
          include_glyph_ink(
            &mut ink,
            &shaped,
            context,
            setup,
            static_inline_prefix,
            baseline_shift,
          );
          if !shaped.brush.decorations.is_empty() {
            let undecorated = ShapedRun {
              glyphs: Vec::new(),
              cluster_ranges: Vec::new(),
              ..shaped.clone()
            };

            // Approximate: the lines snap to local pixels, where paint snaps them to the device's.
            for line in undecorated.decoration_lines(
              &decoration_placement,
              Affine::IDENTITY,
              Affine::IDENTITY,
              Point::ZERO,
            ) {
              let area = line.bounds();

              InkBox::include(
                &mut decoration_ink,
                Point {
                  x: area.left,
                  y: area.top,
                },
                Size {
                  width: area.right - area.left,
                  height: area.bottom - area.top,
                },
              );
            }
          }

          items.push_run(
            context,
            shaped,
            setup.run_scale(baseline_shift),
            static_inline_prefix,
            baseline_shift,
            decoration_placement,
          );
        }
        PlacedItem::Box(inline_box) => {
          InkBox::include(
            &mut ink,
            Point::new(inline_box.x, inline_box.y),
            Size::new(inline_box.width, inline_box.height),
          );
          inline_boxes.insert(inline_box.id, inline_box);
        }
        PlacedItem::Placeholder(_) => {}
      }
      Ok(())
    });

    for inline_box in &self.positioned_floats {
      InkBox::include(
        &mut ink,
        Point::new(inline_box.x, inline_box.y),
        Size::new(inline_box.width, inline_box.height),
      );
    }
    items.ink = Ink {
      content: ink,
      decorations: decoration_ink,
    };
    items.inline_boxes = self.with_floats(inline_boxes);
    let (backgrounds, outline_rects) = decoration_coverage.into_fragments();

    items.backgrounds = backgrounds.into_iter().map(BackgroundItem::of).collect();
    items.outline_rects = outline_rects;
    items.shrink_to_fit();
    items
  }
}

impl BuiltInlineLayout<'_> {
  /// How far its spans' decorations, shadows and strokes reach past the glyphs.
  fn span_ink_reach(&self) -> f32 {
    let decoration_reach = self
      .spans
      .iter()
      .filter_map(ProcessedInlineSpan::text_chain)
      .flat_map(|chain| chain.ancestors())
      .map(|link| link.decoration.reach())
      .fold(0.0_f32, f32::max);

    self
      .spans
      .iter()
      .filter_map(|span| match span {
        ProcessedInlineSpan::Text { style, .. } => Some(text_ink_reach(style)),
        _ => None,
      })
      .fold(decoration_reach, f32::max)
  }
}

/// `ink` grown to take the ink of `run`'s glyphs on the line `setup` sets up: what the run's
/// metrics box misses, such as synthetic-italic skew, faux-bold outset, negative bearings, and
/// glyphs taller than the font's metrics.
fn include_glyph_ink(
  ink: &mut Option<InkBox>,
  run: &ShapedRun,
  context: &RenderContext,
  setup: &LineSetup,
  static_inline_prefix: f32,
  baseline_shift: f32,
) {
  let Ok(font) = FontRef::from_index(run.font_data(), run.font_index) else {
    return;
  };
  let resolved_glyphs = context.fonts().with_context(|fonts| {
    fonts.resolve_glyphs(run.face(), font, run.glyphs.iter().map(|glyph| glyph.id))
  });

  for glyph in &run.glyphs {
    let Some((min_x, min_y, max_x, max_y)) = resolved_glyphs
      .get(&glyph.id)
      .and_then(|resolved| resolved.ink_extents())
    else {
      continue;
    };
    let (origin, size) = setup.scale_rect(
      Point {
        x: glyph.x + min_x,
        y: glyph.y + baseline_shift + min_y,
      },
      Size {
        width: max_x - min_x,
        height: max_y - min_y,
      },
      static_inline_prefix,
      baseline_shift,
    );

    InkBox::include(ink, origin, size);
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

impl FragmentItems {
  /// Frees the room its lists grew past their items, since a node keeps them for the render.
  fn shrink_to_fit(&mut self) {
    self.text.shrink_to_fit();
    self.line_boxes.shrink_to_fit();
    self.text_items.shrink_to_fit();
    self.faces.shrink_to_fit();
    self.glyphs.simple.shrink_to_fit();
    self.glyphs.full.shrink_to_fit();
    self.inline_boxes.shrink_to_fit();
    self.outline_rects.shrink_to_fit();
    self.backgrounds.shrink_to_fit();
  }

  fn push_run(
    &mut self,
    context: &RenderContext,
    run: ShapedRun,
    line_scale: LineScaleState,
    static_inline_prefix: f32,
    baseline_shift: f32,
    decoration_placement: DecorationPlacement,
  ) {
    let (mut face, glyphs) = self.glyphs.push(run);
    let offset = take(&mut face.offset);
    let baseline = take(&mut face.baseline);
    let advance = take(&mut face.advance);
    let hanging = take(&mut face.hanging);
    let text_range = take(&mut face.text_range);
    let face = self.face_index(face, context);

    self.text_items.push(TextItem {
      face,
      offset,
      baseline,
      advance,
      hanging,
      text_range,
      glyphs,
      line_scale,
      static_inline_prefix,
      baseline_shift,
      decoration_placement,
    });
  }

  /// The index of `face` among [`Self::faces`], added, shared with a box laid out lately when
  /// one drew in it, when no run before draws in it.
  fn face_index(&mut self, face: ShapedRun, context: &RenderContext) -> u32 {
    let index = self
      .faces
      .iter()
      .rposition(|existing| existing.same_face(&face))
      .unwrap_or_else(|| {
        self.faces.push(context.inline_cache().share_face(face));
        self.faces.len() - 1
      });

    index as u32
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

  /// How many glyphs it stores as simple and as full glyphs.
  #[cfg(test)]
  pub(super) fn glyph_counts(&self) -> (usize, usize) {
    (self.glyphs.simple.len(), self.glyphs.full.len())
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
        hanging: item.hanging,
        text_range: item.text_range.clone(),
        ..ShapedRun::clone(&self.faces[item.face as usize])
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
        decoration_placement: item.decoration_placement.clone(),
        paint_offset,
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
fn span_owners<'c>(spans: &[ProcessedInlineSpan<'c>]) -> HashMap<usize, &'c RenderNode> {
  spans
    .iter()
    .filter_map(|span| match span {
      ProcessedInlineSpan::Text { decorations, .. }
      | ProcessedInlineSpan::Spacer { decorations, .. } => decorations.as_deref(),
      ProcessedInlineSpan::Box(item) => item.decorations.as_deref(),
      ProcessedInlineSpan::DirectionMark { .. } => None,
    })
    .flat_map(DecorationLink::ancestors)
    .map(|link| (link.decoration.id, link.decoration.owner))
    .collect()
}
