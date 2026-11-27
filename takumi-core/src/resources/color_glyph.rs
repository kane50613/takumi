//! A COLR glyph recorded from skrifa's painter callbacks into instructions each backend replays,
//! in font units with the y-axis pointing up, as the COLR table defines them. Colors stay palette
//! indices until a backend draws them, since the text color resolves the foreground index.

use crate::path::{Path, PathBuilder, Point, Rect};
use crate::style::Affine;
use skrifa::{
  FontRef, GlyphId, MetadataProvider,
  color::{Brush, ColorPainter, ColorStop as SkrifaColorStop, CompositeMode, Extend},
  instance::{LocationRef, Size},
  outline::{DrawSettings, OutlinePen},
  raw::types::BoundingBox,
};

use crate::resources::glyph::draw_outline;

/// A COLR glyph's drawing, and the box its clips bound.
pub struct ColorGlyph {
  /// What the glyph draws, in paint order.
  pub instructions: Vec<ColorInstruction>,
  /// The union of the glyph's clips, which a fill without a clip of its own covers, or `None`
  /// when it has no clip and so draws nothing.
  pub bounds: Option<Rect>,
}

/// One step of a COLR glyph's drawing.
pub enum ColorInstruction {
  /// Instructions drawn in a group of their own, merged down with the mode.
  Layer(CompositeMode, Vec<ColorInstruction>),
  /// `source` drawn over `backdrop` with the mode, as a `SrcOver` layer around one layer reads.
  Composite {
    /// What is drawn.
    source: Vec<ColorInstruction>,
    /// How it merges with `backdrop`.
    mode: CompositeMode,
    /// What is drawn first.
    backdrop: Vec<ColorInstruction>,
  },
  /// `paint` filling the last clip, with the clips before it applied.
  Filled(ColorPaint, Vec<Path>),
}

/// A palette color, or the text color for the index `0xFFFF`, at an alpha.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColorRef {
  /// The index into the font's first palette.
  pub palette_index: u16,
  /// The alpha the color is faded by, from 0 to 1.
  pub alpha: f32,
}

impl ColorRef {
  /// Whether it is the text color, the COLR palette index `0xFFFF`.
  pub fn is_foreground(self) -> bool {
    self.palette_index == u16::MAX
  }
}

/// A gradient stop.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColorStop {
  /// Where the stop sits along the gradient, from 0 to 1.
  pub offset: f32,
  /// The stop's color.
  pub color: ColorRef,
}

/// What a COLR fill paints, its gradient geometry in the glyph space `transform` maps.
pub enum ColorPaint {
  /// One color.
  Solid(ColorRef),
  /// A gradient along the line from `p0` to `p1`.
  Linear {
    /// The start point.
    p0: Point,
    /// The end point.
    p1: Point,
    /// The color stops.
    stops: Vec<ColorStop>,
    /// What the gradient paints past its ends.
    extend: Extend,
    /// The transform the gradient is defined under.
    transform: Affine,
  },
  /// A gradient between two circles.
  Radial {
    /// The start circle's center.
    c0: Point,
    /// The start circle's radius.
    r0: f32,
    /// The end circle's center.
    c1: Point,
    /// The end circle's radius.
    r1: f32,
    /// The color stops.
    stops: Vec<ColorStop>,
    /// What the gradient paints past its ends.
    extend: Extend,
    /// The transform the gradient is defined under.
    transform: Affine,
  },
  /// A gradient around `center`, in degrees clockwise from the positive x-axis. skrifa swaps
  /// COLR's counterclockwise angles, and may reverse the stops to do it.
  Sweep {
    /// The center.
    center: Point,
    /// The angle the gradient starts at.
    start_angle: f32,
    /// The angle the gradient ends at.
    end_angle: f32,
    /// The color stops.
    stops: Vec<ColorStop>,
    /// What the gradient paints past its ends.
    extend: Extend,
    /// The transform the gradient is defined under.
    transform: Affine,
  },
}

impl ColorGlyph {
  /// The COLR drawing of `glyph`, or `None` when the font has none for it, or it is malformed or
  /// unbounded, as a fill with no clip is.
  pub fn record(font: &FontRef<'_>, location: LocationRef<'_>, glyph: GlyphId) -> Option<Self> {
    let color_glyph = font.color_glyphs().get(glyph)?;
    let mut recorder = Recorder::new(font, location);

    color_glyph.paint(location, &mut recorder).ok()?;
    recorder.finish()
  }
}

/// Builds a [`ColorGlyph`] from skrifa's painter callbacks.
struct Recorder<'a> {
  font: &'a FontRef<'a>,
  location: LocationRef<'a>,
  /// The number of colors in the font's first palette.
  palette_len: usize,
  clips: Vec<Vec<Path>>,
  /// Each open layer's instructions, and whether every fill in it is bounded.
  stack: Vec<(Vec<ColorInstruction>, bool)>,
  bounds: Option<Rect>,
  layers: Vec<CompositeMode>,
  transforms: Vec<Affine>,
  error: bool,
}

impl<'a> Recorder<'a> {
  fn new(font: &'a FontRef<'a>, location: LocationRef<'a>) -> Self {
    let palette_len = font
      .color_palettes()
      .get(0)
      .map_or(0, |palette| palette.colors().len());

    Self {
      font,
      location,
      palette_len,
      clips: vec![vec![]],
      stack: vec![(vec![], true)],
      bounds: None,
      layers: vec![],
      transforms: vec![Affine::IDENTITY],
      error: false,
    }
  }

  fn finish(mut self) -> Option<ColorGlyph> {
    if self.error {
      return None;
    }

    let (instructions, bounded) = self.stack.pop()?;

    bounded.then_some(ColorGlyph {
      instructions,
      bounds: self.bounds,
    })
  }

  fn push_clip(&mut self, path: Path) {
    let Some(mut clips) = self.clips.last().cloned() else {
      self.error = true;
      return;
    };

    self.bounds = match self.bounds {
      Some(bounds) => bounds.join(&path.bounds()),
      None => Some(path.bounds()),
    };

    if self.bounds.is_none() {
      self.error = true;
      return;
    }

    clips.push(path);
    self.clips.push(clips);
  }

  /// `path` under the current transform.
  fn transformed(&self, path: Option<Path>) -> Option<Path> {
    path?.transform(*self.transforms.last()?)
  }

  /// The color, or `None` for a palette index the font's first palette lacks.
  fn color(&self, palette_index: u16, alpha: f32) -> Option<ColorRef> {
    let color = ColorRef {
      palette_index,
      alpha: alpha.clamp(0.0, 1.0),
    };

    (color.is_foreground() || usize::from(palette_index) < self.palette_len).then_some(color)
  }

  fn stops(&self, stops: &[SkrifaColorStop]) -> Option<Vec<ColorStop>> {
    stops
      .iter()
      .map(|stop| {
        Some(ColorStop {
          offset: Some(stop.offset).filter(|offset| (0.0..=1.0).contains(offset))?,
          color: self.color(stop.palette_index, stop.alpha)?,
        })
      })
      .collect()
  }

  fn paint(&self, brush: Brush<'_>) -> Option<ColorPaint> {
    let transform = *self.transforms.last()?;

    Some(match brush {
      Brush::Solid {
        palette_index,
        alpha,
      } => ColorPaint::Solid(self.color(palette_index, alpha)?),
      Brush::LinearGradient {
        p0,
        p1,
        color_stops,
        extend,
      } => ColorPaint::Linear {
        p0: Point::from_xy(p0.x, p0.y),
        p1: Point::from_xy(p1.x, p1.y),
        stops: self.stops(color_stops)?,
        extend,
        transform,
      },
      Brush::RadialGradient {
        c0,
        r0,
        c1,
        r1,
        color_stops,
        extend,
      } => ColorPaint::Radial {
        c0: Point::from_xy(c0.x, c0.y),
        r0,
        c1: Point::from_xy(c1.x, c1.y),
        r1,
        stops: self.stops(color_stops)?,
        extend,
        transform,
      },
      Brush::SweepGradient {
        c0,
        start_angle,
        end_angle,
        color_stops,
        extend,
      } => ColorPaint::Sweep {
        center: Point::from_xy(c0.x, c0.y),
        start_angle,
        end_angle,
        stops: self.stops(color_stops)?,
        extend,
        transform,
      },
    })
  }
}

impl ColorPainter for Recorder<'_> {
  fn push_transform(&mut self, transform: skrifa::color::Transform) {
    let Some(last) = self.transforms.last() else {
      self.error = true;
      return;
    };
    let next = *last
      * Affine {
        a: transform.xx,
        b: transform.yx,
        c: transform.xy,
        d: transform.yy,
        x: transform.dx,
        y: transform.dy,
      };

    self.transforms.push(next);
  }

  fn pop_transform(&mut self) {
    if self.transforms.pop().is_none() {
      self.error = true;
    }
  }

  fn push_clip_glyph(&mut self, glyph_id: GlyphId) {
    let outline_glyphs = self.font.outline_glyphs();
    let mut builder = PathRecorder(PathBuilder::new());
    let drawn = outline_glyphs.get(glyph_id).is_some_and(|outline| {
      draw_outline(
        &outline,
        DrawSettings::unhinted(Size::unscaled(), self.location),
        &mut builder,
      )
      .is_ok()
    });
    let path = drawn.then(|| builder.0.finish()).flatten();

    match self.transformed(path) {
      Some(path) => self.push_clip(path),
      None => self.error = true,
    }
  }

  fn push_clip_box(&mut self, clip_box: BoundingBox<f32>) {
    let mut builder = PathBuilder::new();

    builder.move_to(clip_box.x_min, clip_box.y_min);
    builder.line_to(clip_box.x_min, clip_box.y_max);
    builder.line_to(clip_box.x_max, clip_box.y_max);
    builder.line_to(clip_box.x_max, clip_box.y_min);
    builder.close();

    match self.transformed(builder.finish()) {
      Some(path) => self.push_clip(path),
      None => self.error = true,
    }
  }

  fn pop_clip(&mut self) {
    if self.clips.pop().is_none() {
      self.error = true;
    }
  }

  fn fill(&mut self, brush: Brush<'_>) {
    let Some(paint) = self.paint(brush) else {
      self.error = true;
      return;
    };
    let Some(clips) = self.clips.last().cloned() else {
      self.error = true;
      return;
    };
    let Some((stack, bounded)) = self.stack.last_mut() else {
      self.error = true;
      return;
    };

    *bounded &= !clips.is_empty();
    stack.push(ColorInstruction::Filled(paint, clips));
  }

  fn push_layer(&mut self, composite_mode: CompositeMode) {
    self.layers.push(composite_mode);
    self.stack.push((vec![], true));
  }

  fn pop_layer(&mut self) {
    let (Some(composite), Some((mut instructions, bounded))) =
      (self.layers.pop(), self.stack.pop())
    else {
      self.error = true;
      return;
    };
    let Some((stack, parent_bounded)) = self.stack.last_mut() else {
      self.error = true;
      return;
    };

    *parent_bounded = match composite {
      CompositeMode::Clear => true,
      CompositeMode::Src | CompositeMode::SrcOut => bounded,
      CompositeMode::Dest | CompositeMode::DestOut => *parent_bounded,
      CompositeMode::SrcIn | CompositeMode::DestIn => bounded || *parent_bounded,
      _ => bounded && *parent_bounded,
    };

    // skrifa wraps everything in a `SrcOver` base layer, so a `SrcOver` layer around one layer
    // composites that layer over what came before it.
    // https://github.com/googlefonts/fontations/blob/fd0178ce4ea48301a7f06b35a6fce9879c102292/skrifa/src/color/traversal.rs#L566-L593
    if composite == CompositeMode::SrcOver
      && let Some(ColorInstruction::Layer(mode, source)) =
        instructions.pop_if(|instruction| matches!(instruction, ColorInstruction::Layer(..)))
    {
      stack.push(ColorInstruction::Composite {
        source,
        mode,
        backdrop: instructions,
      });
      return;
    }

    stack.push(ColorInstruction::Layer(composite, instructions));
  }
}

/// Records a glyph outline into a tiny-skia path, in font units with the y-axis pointing up.
struct PathRecorder(PathBuilder);

impl OutlinePen for PathRecorder {
  fn move_to(&mut self, x: f32, y: f32) {
    self.0.move_to(x, y);
  }

  fn line_to(&mut self, x: f32, y: f32) {
    self.0.line_to(x, y);
  }

  fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
    self.0.quad_to(cx0, cy0, x, y);
  }

  fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
    self.0.cubic_to(cx0, cy0, cx1, cy1, x, y);
  }

  fn close(&mut self) {
    self.0.close();
  }
}

#[cfg(test)]
mod tests {
  use skrifa::{
    FontRef, GlyphId, MetadataProvider,
    color::{Brush, ColorPainter},
    instance::LocationRef,
    raw::{TableProvider, types::BoundingBox},
  };

  use super::{ColorGlyph, Recorder};

  const FONT: &[u8] =
    include_bytes!("../../../assets/fonts/noto-sans/NotoColorEmoji.COLR.subset.ttf");

  const CLIP: BoundingBox<f32> = BoundingBox {
    x_min: 0.0,
    y_min: 0.0,
    x_max: 10.0,
    y_max: 10.0,
  };

  #[test]
  fn a_palette_index_past_the_palette_fails_the_glyph() {
    let font = FontRef::new(FONT).unwrap();
    let mut recorder = Recorder::new(&font, LocationRef::default());

    recorder.push_clip_box(CLIP);
    recorder.fill(Brush::Solid {
      palette_index: 60_000,
      alpha: 1.0,
    });

    assert!(recorder.finish().is_none());
  }

  #[test]
  fn a_fill_without_a_clip_leaves_the_glyph_unbounded() {
    let font = FontRef::new(FONT).unwrap();
    let mut recorder = Recorder::new(&font, LocationRef::default());

    recorder.fill(Brush::Solid {
      palette_index: 0,
      alpha: 1.0,
    });

    assert!(recorder.finish().is_none());
  }

  #[test]
  fn a_color_glyph_records_bounded_instructions() {
    let font = FontRef::new(FONT).unwrap();
    let glyph = (0..font.maxp().unwrap().num_glyphs())
      .map(|id| GlyphId::new(u32::from(id)))
      .find(|&glyph| font.color_glyphs().get(glyph).is_some())
      .unwrap();
    let color_glyph = ColorGlyph::record(&font, LocationRef::default(), glyph).unwrap();

    assert!(color_glyph.bounds.is_some());
    assert!(!color_glyph.instructions.is_empty());
  }
}
