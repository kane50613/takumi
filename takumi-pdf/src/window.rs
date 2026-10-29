//! The page window a prepared tree emits through: what it paints, what it
//! owns, and the clip and translation that put it on the page.

use takumi_core::{geometry::Point, painter::LineItem, scene::SceneBounds};

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
}

impl Window {
  pub(crate) fn excludes(&self, top: f32, bottom: f32) -> bool {
    self.y.is_some_and(|(y0, y1)| bottom <= y0 || top >= y1)
  }

  pub(crate) fn excludes_bounds(&self, bounds: Option<SceneBounds>) -> bool {
    bounds.is_some_and(|b| self.excludes(b.top as f32, b.bottom as f32))
  }

  /// Whether this page draws a line's item, as Blink spreads monolithic overflow.
  pub(crate) fn shows_line_item(&self, item: LineItem) -> bool {
    let overflows = self
      .y
      .is_some_and(|(y0, y1)| item.bottom - item.top > y1 - y0);

    !self.disowns_line(item.baseline) || (overflows && !self.excludes(item.top, item.bottom))
  }

  /// Whether the line at `baseline` belongs to another page.
  pub(crate) fn disowns_line(&self, baseline: f32) -> bool {
    self
      .lines
      .is_some_and(|(y0, y1)| baseline < y0 || baseline >= y1)
  }

  /// Narrows both windows to a box clipping its overflow.
  pub(crate) fn narrow(&mut self, top: f32, bottom: f32) {
    for window in [&mut self.y, &mut self.lines] {
      if let Some((from, to)) = window {
        *window = Some((from.max(top), to.min(bottom)));
      }
    }
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
