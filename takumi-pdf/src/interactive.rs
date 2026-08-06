//! Collection of link and heading targets, and emission of link annotations and the outline.

use std::cell::RefCell;

use crate::krilla::{
  action::{Action, LinkAction},
  annotation::{Annotation, LinkAnnotation, Target},
  destination::XyzDestination,
  geom::Rect as KrillaRect,
  outline::{Outline, OutlineNode},
};
use takumi_core::{
  font_style::SizedFontStyle,
  geometry::{AvailableSpace, ComputedLayout as Layout, Size},
  layout::{
    inline::{
      InlineItem, InlineLayoutMode, InlineLayoutRequest, collect_inline_items,
      create_inline_layout, resolve_inline_max_height,
    },
    node::{Node, NodeKind},
    tree::RenderNode,
  },
  scene::{NodePaint, PaintItemKind},
  style::Affine,
};

use crate::krilla::page::Page;
use crate::options::PT_PER_PX;
use crate::tags::TagCollector;
use crate::tree::PreparedTree;

/// A hyperlink box in content coordinates.
pub(crate) struct LinkTarget {
  uri: String,
  rect: KrillaRect,
  /// Source-node path, so the annotation can join that node's `Link` element.
  path: Vec<usize>,
}

/// A heading in content coordinates, for the outline.
pub(crate) struct HeadingTarget {
  level: u8,
  text: String,
  pub(crate) top: f32,
}

/// The axis-aligned bounding box of a node-local rect under the node's
/// absolute transform, in content coordinates.
fn transformed_rect(transform: Affine, origin: (f32, f32), size: Size<f32>) -> Option<KrillaRect> {
  let cols = transform.to_cols_array();
  let corners = [
    (origin.0, origin.1),
    (origin.0 + size.width, origin.1),
    (origin.0, origin.1 + size.height),
    (origin.0 + size.width, origin.1 + size.height),
  ];
  let mut left = f32::INFINITY;
  let mut top = f32::INFINITY;
  let mut right = f32::NEG_INFINITY;
  let mut bottom = f32::NEG_INFINITY;

  for (x, y) in corners {
    let px = cols[0] * x + cols[2] * y + cols[4];
    let py = cols[1] * x + cols[3] * y + cols[5];

    left = left.min(px);
    top = top.min(py);
    right = right.max(px);
    bottom = bottom.max(py);
  }
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

fn node_text(node: &Node, out: &mut String) {
  match &node.kind {
    NodeKind::Text(text) => out.push_str(&text.text),
    NodeKind::Container { children } => {
      for child in children {
        node_text(child, out);
      }
    }
    _ => {}
  }
}

/// Collects hyperlinks and headings from the prepared scene, in paint order.
pub(crate) fn collect_interactive(tree: &PreparedTree) -> (Vec<LinkTarget>, Vec<HeadingTarget>) {
  let mut links = Vec::new();
  let mut headings = Vec::new();

  collect_interactive_context(tree, 0, &mut links, &mut headings);
  headings.sort_by(|a, b| a.top.total_cmp(&b.top));
  (links, headings)
}

fn collect_interactive_context(
  tree: &PreparedTree,
  id: usize,
  links: &mut Vec<LinkTarget>,
  headings: &mut Vec<HeadingTarget>,
) {
  let Some(context) = tree.contexts.get(id) else {
    return;
  };

  if let Some(paint) = context.root() {
    collect_interactive_paint(tree, paint, links, headings);
  }
  for bucket in context.in_paint_order() {
    for item in bucket {
      match &item.kind {
        PaintItemKind::Node(paint) => collect_interactive_paint(tree, paint, links, headings),
        PaintItemKind::Context(child) => {
          collect_interactive_context(tree, *child, links, headings);
        }
      }
    }
  }
}

fn collect_interactive_paint(
  tree: &PreparedTree,
  paint: &NodePaint,
  links: &mut Vec<LinkTarget>,
  headings: &mut Vec<HeadingTarget>,
) {
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

  match source.href().filter(|uri| allowed_link_uri(uri)) {
    // The whole box is one link; per-run collection would double-annotate it.
    Some(uri) => links.push(LinkTarget {
      uri: uri.to_string(),
      rect,
      path: paint.path.clone(),
    }),
    None if node.should_create_inline_layout() => {
      collect_inline_links(node, layout, paint.transform, &paint.path, links);
    }
    None => {}
  }
  if let Some(level) = source.tag_name().and_then(heading_level) {
    let mut text = String::new();

    node_text(source, &mut text);
    let text = text.trim();

    if !text.is_empty() {
      headings.push(HeadingTarget {
        level,
        text: text.to_string(),
        top: rect.top(),
      });
    }
  }
}

/// Whether an `href` is written to the PDF: `http`, `https`, `mailto`, or
/// `tel`. Other schemes (and scheme-less values, which have no meaning inside
/// a standalone document) are dropped.
fn allowed_link_uri(uri: &str) -> bool {
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
  let built = create_inline_layout(InlineLayoutRequest {
    items,
    available_space: Size {
      width: AvailableSpace::Definite(content.width),
      height: AvailableSpace::Definite(content.height),
    },
    max_width: content.width,
    max_height: resolve_inline_max_height(&font_style, content.height),
    style: &font_style,
    context,
    mode: InlineLayoutMode::Measure,
    shape_cacheable: true,
  });
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
pub(crate) fn add_link_annotations(
  page: &mut Page,
  links: &[LinkTarget],
  window: (f32, f32),
  offset: (f32, f32),
  tags: Option<&RefCell<TagCollector>>,
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

    let annotation = Annotation::new_link(
      LinkAnnotation::new(
        rect,
        Target::Action(Action::Link(LinkAction::new(link.uri.clone()))),
      ),
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
