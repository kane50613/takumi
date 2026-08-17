//! Collection of link, heading and anchor targets, and emission of link
//! annotations and the outline.

use std::{
  cell::RefCell,
  collections::{HashMap, HashSet},
};

use takumi_core::{
  font_style::SizedFontStyle,
  geometry::{ComputedLayout as Layout, Point as CorePoint, Size, transformed_rect_extents},
  layout::{
    inline::{
      InlineItem, InlineLayoutMode, InlineLayoutRequest, collect_inline_items, create_inline_layout,
    },
    tree::RenderNode,
  },
  scene::{NodePaint, PaintItemKind},
  style::{Affine, Position},
};

use crate::{
  krilla::{
    action::{Action, LinkAction},
    annotation::{Annotation, LinkAnnotation, Target},
    destination::{Destination, XyzDestination},
    geom::Rect as KrillaRect,
    outline::{Outline, OutlineNode},
    page::Page,
  },
  options::PT_PER_PX,
  tags::{TagCollector, text_content},
  tree::PreparedTree,
};

/// A hyperlink box in content coordinates.
pub(crate) struct LinkTarget {
  uri: String,
  rect: KrillaRect,
  /// Source-node path, so the annotation can join that node's `Link` element.
  path: Vec<usize>,
}

/// Link, heading and anchor targets collected from one tree.
pub(crate) struct Interactive {
  pub(crate) links: Vec<LinkTarget>,
  pub(crate) headings: Vec<HeadingTarget>,
  /// Element ids to the box they name, for `href="#id"`.
  pub(crate) anchors: HashMap<Box<str>, AnchorTarget>,
  /// Where every box layout gave a position sits, by the source order of its
  /// node. A node laid out inline has no box of its own, so it has no entry
  /// here and takes its page from the boxes around it.
  pub(crate) extents: HashMap<usize, BoxExtent>,
}

/// A laid-out box's vertical extent in content coordinates.
pub(crate) struct BoxExtent {
  pub(crate) top: f32,
  /// The bottom the flow continues from, absent for an out-of-flow box, whose
  /// position says nothing about what comes after it.
  pub(crate) flow_bottom: Option<f32>,
}

/// An element carrying an `id`, in content coordinates.
#[derive(Clone)]
pub(crate) struct AnchorTarget {
  pub(crate) top: f32,
  /// Path of the element, so a destination can name its structure element.
  pub(crate) path: Vec<usize>,
}

/// A heading in content coordinates, for the outline.
pub(crate) struct HeadingTarget {
  level: u8,
  text: String,
  pub(crate) top: f32,
  /// Path of the heading element. A heading whose text sits in child elements
  /// paints once per child, and the outline wants one entry.
  pub(crate) path: Vec<usize>,
}

/// The axis-aligned bounding box of a node-local rect under the node's
/// absolute transform, in content coordinates.
fn transformed_rect(transform: Affine, origin: (f32, f32), size: Size<f32>) -> Option<KrillaRect> {
  let (left, top, right, bottom) = transformed_rect_extents(
    CorePoint {
      x: origin.0,
      y: origin.1,
    },
    size,
    transform,
  )?;

  KrillaRect::from_ltrb(left, top, right, bottom)
}

fn heading_level(tag: &str) -> Option<u8> {
  let mut bytes = tag.bytes();

  if !bytes.next()?.eq_ignore_ascii_case(&b'h') {
    return None;
  }
  let level = bytes.next()?;

  if bytes.next().is_none() && (b'1'..=b'6').contains(&level) {
    Some(level - b'0')
  } else {
    None
  }
}

/// The element paths a destination can point at: every anchor and every
/// heading the outline lists. Their structure elements need an id so a
/// structure destination can name them.
pub(crate) fn destination_targets(interactive: &Interactive) -> HashSet<Vec<usize>> {
  interactive
    .anchors
    .values()
    .map(|anchor| anchor.path.clone())
    .chain(
      interactive
        .headings
        .iter()
        .map(|heading| heading.path.clone()),
    )
    .collect()
}

/// Collects hyperlinks and headings from the prepared scene, in paint order.
pub(crate) fn collect_interactive(tree: &PreparedTree) -> Interactive {
  let mut collected = Interactive {
    links: Vec::new(),
    headings: Vec::new(),
    anchors: HashMap::new(),
    extents: HashMap::new(),
  };

  collect_interactive_context(tree, 0, &mut collected);
  collected.headings.sort_by(|a, b| a.top.total_cmp(&b.top));
  collected
}

fn collect_interactive_context(tree: &PreparedTree, id: usize, collected: &mut Interactive) {
  let Some(context) = tree.contexts.get(id) else {
    return;
  };

  if let Some(paint) = context.root() {
    collect_interactive_paint(tree, paint, collected);
  }
  for bucket in context.in_paint_order() {
    for item in bucket {
      match &item.kind {
        PaintItemKind::Node(paint) => collect_interactive_paint(tree, paint, collected),
        PaintItemKind::Context(child) => {
          collect_interactive_context(tree, *child, collected);
        }
      }
    }
  }
}

fn collect_interactive_paint(tree: &PreparedTree, paint: &NodePaint, collected: &mut Interactive) {
  let Some(node) = tree.root.node_at_path(&paint.path) else {
    return;
  };
  let Ok(layout) = tree.results.layout(paint.node_id) else {
    return;
  };
  let Some(source) = node.node.as_ref() else {
    return;
  };
  let Some(rect) = transformed_rect(paint.transform, (0.0, 0.0), layout.size) else {
    return;
  };

  if let Some(index) = node.source_order() {
    // `transform` moves where a box paints without moving the flow it left
    // behind, so the flow edge is measured with the box's own transform undone.
    let in_flow = !matches!(
      node.context.style.position,
      Position::Absolute | Position::Fixed
    );
    let flow_bottom = in_flow
      .then(|| {
        node
          .context
          .style
          .local_transform(layout.size.width, layout.size.height, &node.context.sizing)
          .invert()
      })
      .flatten()
      .and_then(|undo| transformed_rect(paint.transform * undo, (0.0, 0.0), layout.size))
      .map(|flow| flow.bottom());

    collected.extents.entry(index).or_insert(BoxExtent {
      top: rect.top(),
      flow_bottom,
    });
  }

  if let Some(id) = source.id() {
    // The first box wins: a duplicated id is invalid HTML, and the earlier one
    // is what a browser would scroll to.
    collected.anchors.entry(id.into()).or_insert(AnchorTarget {
      top: rect.top(),
      path: paint.path.clone(),
    });
  }

  match source.href().filter(|uri| allowed_link_uri(uri)) {
    // The whole box is one link; per-run collection would double-annotate it.
    Some(uri) => collected.links.push(LinkTarget {
      uri: uri.to_string(),
      rect,
      path: paint.path.clone(),
    }),
    None if node.should_create_inline_layout() => {
      collect_inline_links(
        node,
        layout,
        paint.transform,
        &paint.path,
        &mut collected.links,
      );
    }
    None => {}
  }
  // The heading itself paints only when it holds the text directly; markup
  // like `<h1>Plain <strong>bold</strong></h1>` paints the children instead.
  let Some((path, heading, level)) = heading_ancestor(tree, &paint.path) else {
    return;
  };

  if collected
    .headings
    .iter()
    .any(|collected| collected.path == path)
  {
    return;
  }
  let text = text_content(heading);

  if !text.is_empty() {
    collected.headings.push(HeadingTarget {
      level,
      text,
      top: rect.top(),
      path,
    });
  }
}

/// The nearest heading at or above `path`, with its own path and level.
fn heading_ancestor<'t>(
  tree: &'t PreparedTree,
  path: &[usize],
) -> Option<(Vec<usize>, &'t RenderNode, u8)> {
  for length in (0..=path.len()).rev() {
    let ancestor = tree.root.node_at_path(&path[..length])?;
    let Some(source) = ancestor.node.as_ref() else {
      continue;
    };

    if let Some(level) = source.tag_name().and_then(heading_level) {
      return Some((path[..length].to_vec(), ancestor, level));
    }
  }
  None
}

/// Decodes the percent escapes an `id` fragment carries in a URL, so
/// `#section%201` finds the element with `id="section 1"`.
pub(crate) fn percent_decode(fragment: &str) -> String {
  let mut decoded = Vec::with_capacity(fragment.len());
  let bytes = fragment.as_bytes();
  let mut index = 0;

  while index < bytes.len() {
    let escape = (bytes[index] == b'%' && index + 2 < bytes.len())
      .then(|| std::str::from_utf8(&bytes[index + 1..index + 3]).ok())
      .flatten()
      .and_then(|hex| u8::from_str_radix(hex, 16).ok());

    match escape {
      Some(byte) => {
        decoded.push(byte);
        index += 3;
      }
      None => {
        decoded.push(bytes[index]);
        index += 1;
      }
    }
  }

  String::from_utf8(decoded).unwrap_or_else(|_| fragment.to_string())
}

/// Whether an `href` is written to the PDF: a `#fragment` pointing inside the
/// document, or an `http`, `https`, `mailto` or `tel` URI. Other schemes (and
/// scheme-less values, which have no meaning inside a standalone document) are
/// dropped.
fn allowed_link_uri(uri: &str) -> bool {
  if let Some(id) = uri.strip_prefix('#') {
    return !id.is_empty();
  }
  let Some((scheme, _)) = uri.split_once(':') else {
    return false;
  };

  ["http", "https", "mailto", "tel"]
    .iter()
    .any(|allowed| scheme.eq_ignore_ascii_case(allowed))
}

/// Measures a box's inline layout and records one link box per glyph run that
/// sits inside an anchor.
fn collect_inline_links(
  node: &RenderNode,
  layout: Layout,
  transform: Affine,
  path: &[usize],
  links: &mut Vec<LinkTarget>,
) {
  let context = &node.context;
  let font_style = SizedFontStyle::from_style(&context.style, context);
  let content = layout.content_box_size();

  if font_style.sizing.font_size == 0.0 || content.width <= 0.0 || content.height <= 0.0 {
    return;
  }
  let items = collect_inline_items(node);

  if !items
    .iter()
    .any(|item| matches!(item, InlineItem::Text { link: Some(_), .. }))
  {
    return;
  }
  let built = create_inline_layout(InlineLayoutRequest::in_content_box(
    items,
    content,
    &font_style,
    context,
    InlineLayoutMode::Measure,
  ));
  let (runs, _) = built.measure_runs(layout);

  for run in runs {
    let Some(uri) = run.link.filter(|uri| allowed_link_uri(uri)) else {
      continue;
    };
    let Some(rect) = transformed_rect(
      transform,
      (run.x, run.y),
      Size {
        width: run.width,
        height: run.height,
      },
    ) else {
      continue;
    };

    links.push(LinkTarget {
      uri: uri.to_string(),
      rect,
      path: path.to_vec(),
    });
  }
}

/// Adds this page's slice of every link as annotations. `window` is the page's
/// content window in content coordinates; `offset` maps content to page
/// coordinates.
pub(crate) fn add_link_annotations<'l>(
  page: &mut Page,
  links: impl IntoIterator<Item = &'l LinkTarget>,
  window: (f32, f32),
  offset: (f32, f32),
  tags: Option<&RefCell<TagCollector>>,
  anchor: impl Fn(&str) -> Option<XyzDestination>,
) {
  for link in links {
    let top = link.rect.top().max(window.0);
    let bottom = link.rect.bottom().min(window.1);

    if bottom <= top {
      continue;
    }
    let Some(rect) = KrillaRect::from_ltrb(
      (link.rect.left() + offset.0) * PT_PER_PX,
      (top - window.0 + offset.1) * PT_PER_PX,
      (link.rect.right() + offset.0) * PT_PER_PX,
      (bottom - window.0 + offset.1) * PT_PER_PX,
    ) else {
      continue;
    };

    // A fragment that matches no element is dropped: the annotation would be a
    // clickable box that goes nowhere.
    let target = match link.uri.strip_prefix('#') {
      Some(id) => match anchor(&percent_decode(id)) {
        Some(destination) => Target::Destination(Destination::Xyz(destination)),
        None => continue,
      },
      None => Target::Action(Action::Link(LinkAction::new(link.uri.clone()))),
    };
    let annotation = Annotation::new_link(
      LinkAnnotation::new(rect, target),
      // Tagged output requires alt text on link annotations; the target URI
      // is the honest description available.
      tags.is_some().then(|| link.uri.clone()),
    );

    match tags {
      Some(tags) => {
        let identifier = page.add_tagged_annotation(annotation);

        tags.borrow_mut().record_annotation(&link.path, identifier);
      }
      None => page.add_annotation(annotation),
    }
  }
}

/// Nests flat headings into an outline tree: a heading adopts the following
/// deeper headings as children, like an HTML document outline.
pub(crate) fn build_outline(
  headings: &[HeadingTarget],
  destination: impl Fn(&HeadingTarget) -> XyzDestination,
) -> Outline {
  fn take(
    headings: &[HeadingTarget],
    index: &mut usize,
    level: u8,
    destination: &impl Fn(&HeadingTarget) -> XyzDestination,
  ) -> Vec<OutlineNode> {
    let mut nodes = Vec::new();

    while let Some(heading) = headings.get(*index) {
      if heading.level < level {
        break;
      }
      *index += 1;
      let mut node = OutlineNode::new(heading.text.clone(), destination(heading));

      for child in take(headings, index, heading.level + 1, destination) {
        node.push_child(child);
      }
      nodes.push(node);
    }
    nodes
  }

  let mut outline = Outline::new();
  let mut index = 0;

  for node in take(headings, &mut index, 1, &destination) {
    outline.push_child(node);
  }
  outline
}
