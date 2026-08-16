//! Trees drawn once per page: header and footer bands, and repeated `fixed`
//! boxes.

use std::cell::RefCell;

use takumi_core::{context::RenderContext, layout::node::Node, style::Affine, viewport::Viewport};

use crate::{
  counters::{has_page_counters, substitute_page_counters, substitute_target_counters},
  emitter::{FontMap, RenderIssues},
  interactive::{LinkTarget, collect_interactive},
  krilla::{
    geom::{Rect as KrillaRect, Transform},
    paint::FillRule,
    surface::Surface,
    tagging::{Artifact, ArtifactType, ContentTag},
  },
  options::{BAND_EDGE_PADDING, PdfError},
  page::PageFrame,
  paint::rect_path,
  tree::{PreparedTree, RepeatedBox, RepeatedTemplate, TreeInputs, prepare_repeated, prepare_tree},
};

/// Where a repeatable draws on the page.
pub(crate) enum RepeatBounds {
  /// The strip above the content, inside the top margin.
  Header,
  /// The strip below the content, inside the bottom margin.
  Footer,
  /// The content area, under the content when `below` is set.
  Content {
    /// A negative `z-index` on the box puts it under the content.
    below: bool,
  },
}

impl RepeatBounds {
  /// The clip rect a repeatable draws inside. A band clips to its measured
  /// height, so a wrap caused by a wider real counter cannot move the band box
  /// between pages.
  fn rect(&self, frame: &PageFrame, height: f32) -> (f32, f32, f32, f32) {
    match self {
      Self::Header => (0.0, BAND_EDGE_PADDING, frame.size.0, height),
      Self::Footer => (
        0.0,
        frame.size.1 - BAND_EDGE_PADDING - height,
        frame.size.0,
        height,
      ),
      Self::Content { .. } => (
        frame.margin.left,
        frame.margin.top,
        frame.content_width,
        frame.window_height,
      ),
    }
  }
}

/// A tree drawn once per page. One holding a page counter lays out again for
/// every page with that page's numbers; the rest reuse the measured layout.
pub(crate) struct Repeatable {
  prepared: PreparedTree,
  template: Option<RepeatTemplate>,
  bounds: RepeatBounds,
  /// Links collected once from the measured layout. Bands never annotate
  /// links, and a renumbered page collects its own.
  links: Vec<LinkTarget>,
}

/// What a per-page layout starts from.
enum RepeatTemplate {
  /// A band re-lays out its option node with the page's counters.
  Band(Node),
  /// A repeated box re-lays out the subtree it was taken from.
  Fixed(RepeatedTemplate),
}

impl Repeatable {
  /// Measures a band with three-digit counters, recording the template when it
  /// must re-prepare per page.
  pub(crate) fn band(
    inputs: &TreeInputs<'_>,
    template: &Node,
    viewport: Viewport,
    bounds: RepeatBounds,
  ) -> Result<Self, PdfError> {
    Ok(Self {
      prepared: prepare_band(inputs, template, 999, 999, viewport)?,
      template: has_page_counters(template).then(|| RepeatTemplate::Band(template.clone())),
      bounds,
      links: Vec::new(),
    })
  }

  /// Wraps a repeated `fixed` box, caching its links when it never re-lays out.
  pub(crate) fn fixed(repeat: RepeatedBox) -> Self {
    let links = match repeat.template {
      Some(_) => Vec::new(),
      None => collect_interactive(&repeat.prepared).links,
    };
    let below = repeat.prepared.paints_below();

    Self {
      prepared: repeat.prepared,
      template: repeat.template.map(RepeatTemplate::Fixed),
      bounds: RepeatBounds::Content { below },
      links,
    }
  }

  /// The height the measured layout came out at.
  pub(crate) fn height(&self) -> f32 {
    self.prepared.height
  }

  /// Resolves the tree this page draws: a fresh layout with the page's
  /// counters, or the measured one.
  pub(crate) fn for_page(
    &self,
    inputs: &TreeInputs<'_>,
    page_context: &RenderContext,
    frame: &PageFrame,
    page: usize,
    pages: usize,
  ) -> Result<RepeatablePage<'_>, PdfError> {
    let fresh = match &self.template {
      None => None,
      Some(RepeatTemplate::Band(node)) => Some(prepare_band(
        inputs,
        node,
        page,
        pages,
        frame.band_viewport,
      )?),
      Some(RepeatTemplate::Fixed(template)) => Some(prepare_repeated(
        page_context,
        template,
        page,
        pages,
        frame.page_area,
      )?),
    };
    let fresh_links = match (&fresh, &self.bounds) {
      (Some(tree), RepeatBounds::Content { .. }) => collect_interactive(tree).links,
      _ => Vec::new(),
    };

    Ok(RepeatablePage {
      repeatable: self,
      fresh,
      fresh_links,
    })
  }
}

/// One repeatable resolved for one page.
pub(crate) struct RepeatablePage<'r> {
  repeatable: &'r Repeatable,
  fresh: Option<PreparedTree>,
  fresh_links: Vec<LinkTarget>,
}

impl RepeatablePage<'_> {
  fn tree(&self) -> &PreparedTree {
    self.fresh.as_ref().unwrap_or(&self.repeatable.prepared)
  }

  /// Whether this draws under the content: the header band, or a repeated box
  /// with a negative `z-index`.
  pub(crate) fn draws_before_content(&self) -> bool {
    matches!(
      self.repeatable.bounds,
      RepeatBounds::Header | RepeatBounds::Content { below: true }
    )
  }

  /// The links this page annotates.
  pub(crate) fn links(&self) -> impl Iterator<Item = &LinkTarget> {
    self.repeatable.links.iter().chain(&self.fresh_links)
  }

  /// Emits the tree clipped to its bounds on the page.
  pub(crate) fn emit(
    &self,
    frame: &PageFrame,
    fonts: &mut FontMap,
    artifact: bool,
    issues: &RefCell<RenderIssues>,
    document_lang: Option<&str>,
    surface: &mut Surface,
  ) -> Result<(), PdfError> {
    let (x, y, width, height) = self.repeatable.bounds.rect(frame, self.repeatable.height());
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
    let mut emitter = self
      .tree()
      .emitter(fonts, None, None, issues, document_lang);

    emitter.emit_context(0, Affine::IDENTITY, surface)?;
    surface.pop();
    surface.pop();
    if artifact {
      surface.end_tagged();
    }
    Ok(())
  }
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
