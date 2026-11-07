//! The page window a prepared tree emits through: what it paints, what it
//! owns, and the clip and translation that put it on the page.

use takumi_core::{geometry::Point, painter::LineItem, scene::SceneBounds, style::Affine};

use crate::{
  emitter::Emitter,
  krilla::{
    geom::{Rect as KrillaRect, Transform},
    paint::FillRule,
    surface::Surface,
  },
  options::PdfError,
  paint::rect_path,
  tags::ARTIFACT,
};

/// The content windows one emit walk filters by, in content coordinates.
#[derive(Clone, Copy, Default)]
pub(crate) struct Window {
  /// The vertical paint window `[top, bottom)`, outside which nothing paints.
  pub(crate) y: Option<(f32, f32)>,
  /// The text-line ownership window, so a line's text lands on one page.
  pub(crate) lines: Option<(f32, f32)>,
  /// The extent the boxes clipping their overflow leave, which [`Self::narrow`] sets.
  pub(crate) clip: Option<(f32, f32)>,
  /// How the box painting maps its lines into these windows when it paints under a transform:
  /// the transform and the horizontal span its lines take.
  pub(crate) frame: Option<LineFrame>,
}

/// The transform a box's lines paint under, and the horizontal span they take in its space.
#[derive(Clone, Copy)]
pub(crate) struct LineFrame {
  pub(crate) transform: Affine,
  pub(crate) left: f32,
  pub(crate) right: f32,
}

impl LineFrame {
  /// `item` in the space the transform maps into: its extent and line box across the span, and
  /// its baseline at the span's middle.
  fn map(self, item: LineItem) -> LineItem {
    let y = |x: f32, y: f32| self.transform.transform_point(x, y).1;
    let extent = |top: f32, bottom: f32| {
      let corners = [
        y(self.left, top),
        y(self.right, top),
        y(self.left, bottom),
        y(self.right, bottom),
      ];

      (
        corners.into_iter().fold(f32::INFINITY, f32::min),
        corners.into_iter().fold(f32::NEG_INFINITY, f32::max),
      )
    };
    let (top, bottom) = extent(item.top, item.bottom);

    LineItem {
      baseline: y((self.left + self.right) / 2.0, item.baseline),
      top,
      bottom,
      line: extent(item.line.0, item.line.1),
    }
  }
}

impl Window {
  pub(crate) fn excludes(&self, top: f32, bottom: f32) -> bool {
    self.y.is_some_and(|(y0, y1)| bottom <= y0 || top >= y1)
  }

  pub(crate) fn excludes_bounds(&self, bounds: Option<SceneBounds>) -> bool {
    bounds.is_some_and(|b| self.excludes(b.top as f32, b.bottom as f32))
  }

  /// Whether this page draws a line's item: any part of it the clips leave, on the page that owns
  /// its line, or on every page its monolithic overflow spreads over, as Blink spreads it.
  pub(crate) fn shows_line_item(&self, item: LineItem) -> bool {
    let item = self.in_window(item);
    let overflows = self
      .y
      .is_some_and(|(y0, y1)| item.bottom - item.top > y1 - y0);

    !self.excludes(item.top, item.bottom) && (!self.disowns(item) || overflows)
  }

  /// Whether the line holding `item` belongs to another page.
  pub(crate) fn disowns_line(&self, item: LineItem) -> bool {
    self.disowns(self.in_window(item))
  }

  /// A line belongs to the page holding its baseline, or the middle of the part a clip leaves
  /// when the clip cuts its baseline away. Approximate: Blink picks no owner from geometry. It
  /// paints each line in the page whose fragment holds its line box.
  fn disowns(&self, item: LineItem) -> bool {
    let height = match self.clip {
      Some((top, bottom)) if item.baseline < top || item.baseline >= bottom => {
        (item.top.max(top) + item.bottom.min(bottom)) / 2.0
      }
      _ => item.baseline,
    };

    self
      .lines
      .is_some_and(|(y0, y1)| height < y0 || height >= y1)
  }

  fn in_window(&self, item: LineItem) -> LineItem {
    self.frame.map_or(item, |frame| frame.map(item))
  }

  /// Narrows the paint window and the clip to a box clipping its overflow.
  pub(crate) fn narrow(&mut self, top: f32, bottom: f32) {
    let narrowed = |(from, to): (f32, f32)| (from.max(top), to.min(bottom));

    self.y = self.y.map(narrowed);
    self.clip = Some(self.clip.map_or((top, bottom), narrowed));
  }
}

/// One windowed emit: its clip, translation, window, and whether it repeats.
pub(crate) struct ContentWindow {
  /// Clip rect on the page, as `(x, y, width, height)`.
  pub(crate) clip: (f32, f32, f32, f32),
  /// Translation from content to page coordinates.
  pub(crate) translate: Point<f32>,
  pub(crate) window: Window,
  /// A repeated occurrence is an artifact: the first one carried the tags.
  pub(crate) artifact: bool,
}

impl ContentWindow {
  pub(crate) fn emit(
    &self,
    mut emitter: Emitter<'_>,
    surface: &mut Surface,
  ) -> Result<(), PdfError> {
    let (x, y, width, height) = self.clip;
    let Some(path) = KrillaRect::from_xywh(x, y, width, height).and_then(rect_path) else {
      return Ok(());
    };

    if self.artifact {
      surface.start_tagged(ARTIFACT);
    }
    surface.push_clip_path(&path, &FillRule::NonZero);
    surface.push_transform(&Transform::from_translate(
      self.translate.x,
      self.translate.y,
    ));
    emitter.window = self.window;
    emitter.emit(surface)?;
    surface.pop();
    surface.pop();
    if self.artifact {
      surface.end_tagged();
    }
    Ok(())
  }
}
