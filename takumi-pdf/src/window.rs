//! The page window a prepared tree emits through: what it paints, what it
//! owns, and the clip and translation that put it on the page.

use takumi_core::{scene::SceneBounds, style::Affine};

use crate::{
  emitter::Emitter,
  krilla::{
    geom::{Rect as KrillaRect, Transform},
    paint::FillRule,
    surface::Surface,
    tagging::{Artifact, ArtifactType, ContentTag},
  },
  options::PdfError,
  paint::rect_path,
};

/// The content windows one emit walk filters by, in content coordinates.
#[derive(Clone, Copy, Default)]
pub(crate) struct Window {
  /// Vertical paint window `[top, bottom)`. Paint wholly outside it is
  /// skipped, so clipped-away content never reaches the content stream (or
  /// text extraction).
  pub(crate) y: Option<(f32, f32)>,
  /// Horizontal paint window `[left, right)`: a repeated table header replays
  /// only its own table's column of the scene.
  pub(crate) x: Option<(f32, f32)>,
  /// Text-line ownership window `[this page's cut, next page's cut)`. Wider
  /// than `y` at the edges (first page reaches up to −∞, last to +∞) and
  /// narrower at the bottom when a cut lands above the page's full height, so
  /// every line is emitted on exactly one page.
  pub(crate) lines: Option<(f32, f32)>,
}

impl Window {
  pub(crate) fn excludes(&self, top: f32, bottom: f32) -> bool {
    self.y.is_some_and(|(y0, y1)| bottom <= y0 || top >= y1)
  }

  pub(crate) fn excludes_bounds(&self, bounds: Option<SceneBounds>) -> bool {
    bounds.is_some_and(|b| {
      self.excludes(b.top as f32, b.bottom as f32)
        || self
          .x
          .is_some_and(|(x0, x1)| b.right as f32 <= x0 || b.left as f32 >= x1)
    })
  }

  /// Whether a text line at `baseline` belongs to another page. Ownership is
  /// keyed on the baseline (always inside the line's own box, unlike the font
  /// ascent band, which can poke above the container a forced break cut at) and
  /// half-open, so each line is emitted exactly once.
  pub(crate) fn disowns_line(&self, baseline: f32) -> bool {
    self
      .lines
      .is_some_and(|(y0, y1)| baseline < y0 || baseline >= y1)
  }

  /// Narrows both vertical windows to a box that clips its overflow. A clip
  /// keeps content off the page but not out of the text layer, so what it
  /// cuts away must never be emitted.
  pub(crate) fn narrow(&mut self, top: f32, bottom: f32) {
    for window in [&mut self.y, &mut self.lines] {
      if let Some((from, to)) = window {
        *window = Some((from.max(top), to.min(bottom)));
      }
    }
  }
}

/// One windowed emit: the page-space clip, the content translation, the
/// window the emitter filters by, and whether the content is a repeated
/// artifact.
pub(crate) struct ContentWindow {
  /// Clip rect on the page, as `(x, y, width, height)`.
  pub(crate) clip: (f32, f32, f32, f32),
  /// Translation from content to page coordinates.
  pub(crate) translate: (f32, f32),
  pub(crate) window: Window,
  /// A repeated occurrence is an artifact: the first one carried the tags.
  /// `Other` stays valid below PDF 2.0, where the header/footer artifact
  /// subtypes do not exist yet.
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
      surface.start_tagged(ContentTag::Artifact(Artifact::new(
        ArtifactType::Other,
        None,
      )));
    }
    surface.push_clip_path(&path, &FillRule::NonZero);
    surface.push_transform(&Transform::from_translate(
      self.translate.0,
      self.translate.1,
    ));
    emitter.window = self.window;
    emitter.emit_context(0, Affine::IDENTITY, surface)?;
    surface.pop();
    surface.pop();
    if self.artifact {
      surface.end_tagged();
    }
    Ok(())
  }
}
