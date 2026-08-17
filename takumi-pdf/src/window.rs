//! A clipped, translated window through which a prepared tree emits onto a page.

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
use takumi_core::style::Affine;

/// One windowed emit: the page-space clip, the content translation, the
/// content windows the emitter filters by, and whether the content is a
/// repeated artifact.
pub(crate) struct ContentWindow {
  /// Clip rect on the page, as `(x, y, width, height)`.
  pub(crate) clip: (f32, f32, f32, f32),
  /// Translation from content to page coordinates.
  pub(crate) translate: (f32, f32),
  pub(crate) window: Option<(f32, f32)>,
  pub(crate) x_window: Option<(f32, f32)>,
  pub(crate) line_window: Option<(f32, f32)>,
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
    emitter.x_window = self.x_window;
    emitter.line_window = self.line_window;
    emitter.emit_context(0, Affine::IDENTITY, surface)?;
    surface.pop();
    surface.pop();
    if self.artifact {
      surface.end_tagged();
    }
    Ok(())
  }
}
