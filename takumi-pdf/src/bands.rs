//! Trees drawn once per page: header and footer bands, and repeated `fixed`
//! boxes.

use std::ops::Range;

use takumi_core::{
  context::RenderContext,
  layout::{
    node::{Node, NodeKind},
    tree::RenderNode,
  },
  viewport::Viewport,
};

use crate::{
  counters::{has_page_counters, substitute_page_counters},
  emitter::DocumentState,
  interactive::{Interactive, LinkTarget},
  krilla::surface::Surface,
  options::{BAND_EDGE_PADDING, PdfError},
  page::PageFrame,
  tree::{PreparedTree, TreeInputs, page_root},
  window::{ContentWindow, Window},
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
  Fixed(FixedTemplate),
}

/// A repeated box's source subtree, with the context its styles resolved
/// under, so a re-layout inherits what its ancestors gave it.
pub(crate) struct FixedTemplate {
  pub(crate) node: Node,
  pub(crate) parent: RenderContext,
  pub(crate) source_order: usize,
}

impl FixedTemplate {
  /// The source orders the subtree occupies in the tree it was taken from.
  fn source_orders(&self) -> Range<usize> {
    self.source_order..self.source_order + node_count(&self.node)
  }

  /// Lays the box out again with the counters a page asks for.
  fn prepare(
    &self,
    page_context: &RenderContext,
    page: usize,
    pages: usize,
    page_area: Viewport,
  ) -> Result<PreparedTree, PdfError> {
    let mut node = self.node.clone();

    substitute_page_counters(&mut node, page, pages);
    let child = RenderNode::from_node(&self.parent, node);

    PreparedTree::lay_out(page_root(page_context, child), page_area)
  }
}

fn node_count(node: &Node) -> usize {
  match &node.kind {
    NodeKind::Container { children } => 1 + children.iter().map(node_count).sum::<usize>(),
    _ => 1,
  }
}

impl Repeatable {
  /// Measures a band with the last page's counters, the widest a decimal
  /// counter gets, recording the template when it must re-prepare per page.
  pub(crate) fn band(
    inputs: &TreeInputs<'_>,
    template: &Node,
    viewport: Viewport,
    bounds: RepeatBounds,
    pages: usize,
  ) -> Result<Self, PdfError> {
    Ok(Self {
      prepared: inputs.prepare_band(template, pages, pages, viewport)?,
      template: has_page_counters(template).then(|| RepeatTemplate::Band(template.clone())),
      bounds,
      links: Vec::new(),
    })
  }

  /// Wraps a repeated `fixed` box, caching its links when it never re-lays out.
  pub(crate) fn fixed(prepared: PreparedTree, template: Option<FixedTemplate>) -> Self {
    let links = match template {
      Some(_) => Vec::new(),
      None => Interactive::collect(&prepared).links,
    };
    let below = prepared.paints_below();

    Self {
      prepared,
      template: template.map(RepeatTemplate::Fixed),
      bounds: RepeatBounds::Content { below },
      links,
    }
  }

  /// Whether the band holds a counter and lays out again per page.
  pub(crate) fn dynamic(&self) -> bool {
    self.template.is_some()
  }

  /// The height the measured layout came out at.
  pub(crate) fn height(&self) -> f32 {
    self.prepared.height
  }

  /// The source orders a repeated box's counters live at, which the content
  /// pass leaves to the box itself.
  pub(crate) fn source_orders(&self) -> Option<Range<usize>> {
    match &self.template {
      Some(RepeatTemplate::Fixed(template)) => Some(template.source_orders()),
      _ => None,
    }
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
      Some(RepeatTemplate::Band(node)) => {
        Some(inputs.prepare_band(node, page, pages, frame.band_viewport)?)
      }
      Some(RepeatTemplate::Fixed(template)) => {
        Some(template.prepare(page_context, page, pages, frame.page_area)?)
      }
    };
    let fresh_links = match (&fresh, &self.bounds) {
      (Some(tree), RepeatBounds::Content { .. }) => Interactive::collect(tree).links,
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

  /// Emits the tree clipped to its bounds on the page, as an artifact when the
  /// document is tagged.
  pub(crate) fn emit(
    &self,
    frame: &PageFrame,
    state: &DocumentState<'_>,
    surface: &mut Surface,
  ) -> Result<(), PdfError> {
    let (x, y, width, height) = self.repeatable.bounds.rect(frame, self.repeatable.height());

    ContentWindow {
      clip: (x, y, width, height),
      translate: (x, y),
      window: Window::default(),
      artifact: state.tags.is_some(),
    }
    .emit(self.tree().emitter(state, None, false), surface)
  }
}
