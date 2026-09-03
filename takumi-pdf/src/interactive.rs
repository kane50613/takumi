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
      InlineItem, InlineLayoutMode, InlineLayoutRequest, ProcessedInlineSpan, collect_inline_items,
      create_inline_layout,
    },
    inline_box::{InlineBoxPaint, resolve_inline_box},
    tree::RenderNode,
  },
  scene::NodePaint,
  style::{Affine, Position},
};

use crate::{
  emitter::node_path,
  form::FieldTarget,
  krilla::{
    action::{Action, LinkAction},
    annotation::{Annotation, LinkAnnotation, Target},
    destination::{Destination, XyzDestination},
    geom::Rect as KrillaRect,
    outline::{Outline, OutlineNode},
    page::Page,
  },
  options::PT_PER_PX,
  tags::{TagCollector, raw_text, text_content},
  tree::PreparedTree,
  window::Window,
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
  pub(crate) fields: Vec<FieldTarget>,
  /// `<label for>` text by the id it names.
  pub(crate) labels: HashMap<String, String>,
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

impl Interactive {
  /// Collects hyperlinks and headings from the prepared scene, in paint order.
  pub(crate) fn collect(tree: &PreparedTree) -> Self {
    let mut collected = Self::walk(tree);

    collected.resolve_labels(tree);
    collected.headings.sort_by(|a, b| a.top.total_cmp(&b.top));
    collected
  }

  /// The targets of one tree, before the labels that need the whole tree are
  /// resolved.
  fn walk(tree: &PreparedTree) -> Self {
    let mut collected = Self {
      links: Vec::new(),
      fields: Vec::new(),
      labels: HashMap::new(),
      headings: Vec::new(),
      anchors: HashMap::new(),
      extents: HashMap::new(),
    };

    tree.for_each_paint(|paint| collect_interactive_paint(tree, paint, &mut collected));
    collected
  }

  /// Gives every field the text of the `<label>` wrapping it, and replaces
  /// each `aria-labelledby` id with the text of the element it names, which
  /// the walk may only have reached after the field.
  fn resolve_labels(&mut self, tree: &PreparedTree) {
    let texts = self
      .fields
      .iter()
      .map(|field| {
        let named = field
          .labelled_by()?
          .split_whitespace()
          .filter_map(|id| {
            let anchor = self.anchors.get(id)?;
            let text = text_content(tree.root.node_at_path(&anchor.path)?);

            (!text.is_empty()).then_some(text)
          })
          .collect::<Vec<_>>()
          .join(" ");

        (!named.is_empty()).then_some(named)
      })
      .collect::<Vec<_>>();

    for (field, text) in self.fields.iter_mut().zip(texts) {
      field.set_labelled_by(text);
      field.set_wrapping_label(wrapping_label(tree, &field.path));
    }
  }

  /// Moves the targets of an inline box's own tree onto the document: rects
  /// under `transform`, paths under `prefix`.
  fn adopt(&mut self, inner: Self, transform: Affine, prefix: &[usize]) {
    let placed = |rect: KrillaRect| {
      transformed_rect(
        transform,
        (rect.left(), rect.top()),
        Size {
          width: rect.width(),
          height: rect.height(),
        },
      )
    };
    let path = |inner: &[usize]| prefix.iter().chain(inner).copied().collect::<Vec<_>>();

    for mut link in inner.links {
      let Some(rect) = placed(link.rect) else {
        continue;
      };

      link.rect = rect;
      link.path = path(&link.path);
      self.links.push(link);
    }
    for mut field in inner.fields {
      let Some(rect) = placed(field.rect) else {
        continue;
      };

      field.rect = rect;
      field.path = path(&field.path);
      self.fields.push(field);
    }
    for (id, text) in inner.labels {
      self.labels.entry(id).or_insert(text);
    }
  }

  /// The element paths a destination can point at: every anchor and every
  /// heading the outline lists. Their structure elements need an id so a
  /// structure destination can name them.
  pub(crate) fn destination_targets(&self) -> HashSet<Vec<usize>> {
    self
      .anchors
      .values()
      .map(|anchor| anchor.path.clone())
      .chain(self.headings.iter().map(|heading| heading.path.clone()))
      .collect()
  }
}

fn collect_interactive_paint(tree: &PreparedTree, paint: &NodePaint, collected: &mut Interactive) {
  let Some(node) = tree.root.node_at_path(&paint.path) else {
    return;
  };
  let Ok(layout) = tree.results.layout(paint.node_id) else {
    return;
  };
  let Some(rect) = transformed_rect(paint.transform, (0.0, 0.0), layout.size) else {
    return;
  };
  let source = node.node.as_ref();
  let href = source
    .and_then(|source| source.href())
    .filter(|uri| allowed_link_uri(uri));

  match href {
    // The whole box is one link; per-run collection would double-annotate it.
    Some(uri) => collected.links.push(LinkTarget {
      uri: uri.to_string(),
      rect,
      path: paint.path.clone(),
    }),
    // An anonymous wrapper around inline content has no source of its own but
    // still lays out the links and boxes inside it.
    None if node.should_create_inline_layout() => {
      collect_inline_targets(tree, node, layout, paint.transform, &paint.path, collected);
    }
    None => {}
  }
  let Some(source) = source else {
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

  if source
    .tag_name()
    .is_some_and(|tag| tag.eq_ignore_ascii_case("label"))
    && let Some(target) = source.attribute("for")
  {
    let text = text_content(node);

    if !text.is_empty() {
      collected
        .labels
        .entry(target.to_string())
        .or_insert_with(|| text.clone());
    }
  }

  if let Some(field) = FieldTarget::of(node, source, layout, rect, &paint.path) {
    collected.fields.push(field);
  }

  if let Some(id) = source.id() {
    // The first box wins: a duplicated id is invalid HTML, and the earlier one
    // is what a browser would scroll to.
    collected.anchors.entry(id.into()).or_insert(AnchorTarget {
      top: rect.top(),
      path: paint.path.clone(),
    });
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

/// The text of the `<label>` wrapping `path`, without the control's own text,
/// which HTML leaves out of the name it computes.
fn wrapping_label(tree: &PreparedTree, path: &[usize]) -> Option<String> {
  for length in (0..path.len()).rev() {
    let Some(ancestor) = tree.root.node_at_path(&path[..length]) else {
      continue;
    };
    let labels = ancestor
      .node
      .as_ref()
      .and_then(|source| source.tag_name())
      .is_some_and(|tag| tag.eq_ignore_ascii_case("label"));

    if !labels {
      continue;
    }
    let mut text = String::new();

    text_around(ancestor, &path[length..], &mut text);

    return (!text.trim().is_empty()).then(|| text.trim().to_string());
  }
  None
}

/// Every piece of text under `node` except the subtree at `skip`.
fn text_around(node: &RenderNode, skip: &[usize], out: &mut String) {
  let Some((&next, rest)) = skip.split_first() else {
    return;
  };

  for (index, child) in node
    .children
    .as_deref()
    .unwrap_or_default()
    .iter()
    .enumerate()
  {
    match (index == next, rest.is_empty()) {
      (true, true) => {}
      (true, false) => text_around(child, rest, out),
      (false, _) => out.push_str(&raw_text(child)),
    }
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
/// sits inside an anchor, and whatever the inline boxes on its lines hold.
fn collect_inline_targets(
  tree: &PreparedTree,
  node: &RenderNode,
  layout: Layout,
  transform: Affine,
  path: &[usize],
  collected: &mut Interactive,
) {
  let context = &node.context;
  let font_style = SizedFontStyle::from_style(&context.style, context);
  let content = layout.content_box_size();

  if font_style.sizing.font_size == 0.0 || content.width <= 0.0 || content.height <= 0.0 {
    return;
  }
  let items = collect_inline_items(node);
  let has_link = items
    .iter()
    .any(|item| matches!(item, InlineItem::Text { link: Some(_), .. }));
  let has_box = items
    .iter()
    .any(|item| matches!(item, InlineItem::RenderNode { .. }));

  if !has_link && !has_box {
    return;
  }
  let built = create_inline_layout(InlineLayoutRequest::in_content_box(
    items,
    content,
    &font_style,
    context,
    InlineLayoutMode::Measure,
  ));

  if has_link {
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

      collected.links.push(LinkTarget {
        uri: uri.to_string(),
        rect,
        path: path.to_vec(),
      });
    }
  }
  if !has_box {
    return;
  }
  let Ok(runs) = built.resolve_runs(context, layout) else {
    return;
  };

  // An inline-level container is a scene of its own, laid out again at the
  // size its line gave it, so its targets are collected the way the
  // document's are and then moved under the box.
  for positioned in &runs.inline_boxes {
    let Some(ProcessedInlineSpan::Box(item)) = built.spans.get(positioned.id as usize) else {
      continue;
    };
    let Some((offset, InlineBoxPaint::Container(subtree))) =
      resolve_inline_box(positioned, item, layout)
    else {
      continue;
    };
    let mut box_path = Vec::new();

    if !node_path(&tree.root, item.render_node, &mut box_path) {
      continue;
    }
    let origin = Affine::translation(
      offset.x + subtree.margin_offset.x,
      offset.y + subtree.margin_offset.y,
    );
    let Ok(inner) = PreparedTree::of_inline_box(*subtree) else {
      continue;
    };

    collected.adopt(Interactive::walk(&inner), transform * origin, &box_path);
  }
}

/// Adds this page's slice of every link as annotations. `window` is the page's
/// content window in content coordinates, narrowed horizontally for a
/// replayed table header; `offset` maps content to page coordinates.
pub(crate) fn add_link_annotations<'l>(
  page: &mut Page,
  links: impl IntoIterator<Item = &'l LinkTarget>,
  window: Window,
  offset: (f32, f32),
  tags: Option<&RefCell<TagCollector>>,
  anchor: impl Fn(&str) -> Option<XyzDestination>,
) {
  let (y0, y1) = window.y.unwrap_or((f32::NEG_INFINITY, f32::INFINITY));
  let (x0, x1) = window.x.unwrap_or((f32::NEG_INFINITY, f32::INFINITY));

  for link in links {
    let top = link.rect.top().max(y0);
    let bottom = link.rect.bottom().min(y1);
    let left = link.rect.left().max(x0);
    let right = link.rect.right().min(x1);

    if bottom <= top || right <= left {
      continue;
    }
    let Some(rect) = KrillaRect::from_ltrb(
      (left + offset.0) * PT_PER_PX,
      (top - y0 + offset.1) * PT_PER_PX,
      (right + offset.0) * PT_PER_PX,
      (bottom - y0 + offset.1) * PT_PER_PX,
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
/// deeper headings as children, like an HTML document outline. A heading with
/// no destination (its page is dropped by page ranges) loses its entry and its
/// children take its place.
impl Interactive {
  pub(crate) fn outline(
    &self,
    destination: impl Fn(&HeadingTarget) -> Option<XyzDestination>,
  ) -> Outline {
    fn take(
      headings: &[HeadingTarget],
      index: &mut usize,
      level: u8,
      destination: &impl Fn(&HeadingTarget) -> Option<XyzDestination>,
    ) -> Vec<OutlineNode> {
      let mut nodes = Vec::new();

      while let Some(heading) = headings.get(*index) {
        if heading.level < level {
          break;
        }
        *index += 1;
        let children = take(headings, index, heading.level + 1, destination);

        match destination(heading) {
          Some(dest) => {
            let mut node = OutlineNode::new(heading.text.clone(), dest);

            for child in children {
              node.push_child(child);
            }
            nodes.push(node);
          }
          None => nodes.extend(children),
        }
      }
      nodes
    }

    let mut outline = Outline::new();
    let mut index = 0;

    for node in take(&self.headings, &mut index, 1, &destination) {
      outline.push_child(node);
    }
    outline
  }
}
