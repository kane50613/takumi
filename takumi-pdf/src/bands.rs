//! Measurement, per-page preparation and emission of the repeated header/footer bands.

use takumi_core::{layout::node::Node, style::Affine, viewport::Viewport};

use std::cell::RefCell;

use crate::{
  counters::{has_page_counters, substitute_page_counters, substitute_target_counters},
  emitter::{FontMap, RenderIssues},
  krilla::{
    geom::{Rect as KrillaRect, Transform},
    paint::FillRule,
    surface::Surface,
    tagging::{Artifact, ArtifactType, ContentTag},
  },
  options::PdfError,
  paint::rect_path,
  tree::{PreparedTree, TreeInputs, prepare_tree},
};

/// A band measured before pagination: the laid-out tree plus whether it must
/// re-prepare per page (it contains counter hooks).
pub(crate) struct MeasuredBand {
  pub(crate) measured: PreparedTree,
  pub(crate) dynamic: bool,
}

/// Measures a band with three-digit counters and records whether it must
/// re-prepare per page.
pub(crate) fn measure_band(
  inputs: &TreeInputs<'_>,
  template: &Node,
  viewport: Viewport,
) -> Result<MeasuredBand, PdfError> {
  Ok(MeasuredBand {
    measured: prepare_band(inputs, template, 999, 999, viewport)?,
    dynamic: has_page_counters(template),
  })
}

/// Lays out a band template with the given counter values.
pub(crate) fn prepare_band(
  inputs: &TreeInputs<'_>,
  template: &Node,
  page: usize,
  pages: usize,
  viewport: Viewport,
) -> Result<PreparedTree, PdfError> {
  let mut node = template.clone();

  substitute_page_counters(&mut node, page, pages);
  // A band lays out per page, after the pass that resolves target counters, so
  // its hooks name no page. They empty like any other unresolved target instead
  // of leaving the placeholder the template put there.
  substitute_target_counters(&mut node, None, &|_: &str| None, &mut Vec::new());
  prepare_tree(inputs, node, viewport)
}

/// Emits a band clipped to `(x, y, width, height)` on the page.
pub(crate) fn emit_band(
  band: &PreparedTree,
  fonts: &mut FontMap,
  bounds: (f32, f32, f32, f32),
  artifact: bool,
  issues: &RefCell<RenderIssues>,
  document_lang: Option<&str>,
  surface: &mut Surface,
) -> Result<(), PdfError> {
  let (x, y, width, height) = bounds;
  let Some(path) = KrillaRect::from_xywh(x, y, width, height).and_then(rect_path) else {
    return Ok(());
  };

  if artifact {
    // `Other` stays valid below PDF 2.0, where the header/footer artifact
    // subtypes do not exist yet.
    surface.start_tagged(ContentTag::Artifact(Artifact::new(
      ArtifactType::Other,
      None,
    )));
  }
  surface.push_clip_path(&path, &FillRule::NonZero);
  surface.push_transform(&Transform::from_translate(x, y));
  let mut emitter = band.emitter(fonts, None, None, issues, document_lang);

  emitter.emit_context(0, Affine::IDENTITY, surface)?;
  surface.pop();
  surface.pop();
  if artifact {
    surface.end_tagged();
  }
  Ok(())
}
