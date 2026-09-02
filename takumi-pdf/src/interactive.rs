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
    node::{Node, NodeKind},
    tree::RenderNode,
  },
  scene::NodePaint,
  style::{Affine, Color, Position, TextAlign},
};

use crate::{
  emitter::DocumentState,
  krilla::{
    action::{Action, LinkAction},
    annotation::{Annotation, FormField, LinkAnnotation, Target, WidgetAnnotation, WidgetStyle},
    destination::{Destination, XyzDestination},
    geom::Rect as KrillaRect,
    outline::{Outline, OutlineNode},
    page::Page,
  },
  options::PT_PER_PX,
  tags::{TagCollector, text_content},
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

/// A form field box in content coordinates.
pub(crate) struct FieldTarget {
  name: String,
  rect: KrillaRect,
  field: FieldKind,
  style: FieldStyle,
  /// The accessible name, absent when the element names itself only through a
  /// `<label for>` that has not been seen yet.
  described: Option<String>,
  /// The `id` a `<label for>` would point at.
  id: Option<String>,
  /// Source-node path, so the widget can join that node's `Form` element.
  path: Vec<usize>,
  /// This button's place among the radio buttons sharing its name.
  group_index: usize,
}

/// What kind of control an element asks for, before it is placed on a page.
enum FieldKind {
  Text {
    value: String,
    multiline: bool,
    password: bool,
    max_len: Option<i32>,
  },
  CheckBox {
    on: bool,
    export: String,
  },
  Radio {
    /// The submitted value, which becomes a group index once every button of
    /// the group is known.
    export: String,
    on: bool,
  },
  Choice {
    value: String,
    options: Vec<(String, Option<String>)>,
    /// Whether more than one option can be picked, which also makes the
    /// control a list box rather than a drop-down.
    multi: bool,
  },
}

/// The CSS a widget is painted with, kept so the field can carry it too.
struct FieldStyle {
  color: [f32; 3],
  font_size: f32,
  background: Option<[f32; 3]>,
  /// The border color and the width it is stroked at.
  border: Option<([f32; 3], f32)>,
  /// `/Q`: 0 left, 1 center, 2 right.
  align: i32,
  read_only: bool,
  required: bool,
}

impl FieldStyle {
  fn to_widget(&self) -> WidgetStyle {
    WidgetStyle {
      color: self.color,
      font_size: self.font_size,
      background: self.background,
      border: self.border,
      align: self.align,
      read_only: self.read_only,
      required: self.required,
    }
  }
}

/// The color as PDF's three components, absent when fully transparent.
fn pdf_color(color: Color) -> Option<[f32; 3]> {
  let [red, green, blue, alpha] = color.0;

  (alpha > 0).then(|| [red, green, blue].map(|channel| channel as f32 / 255.0))
}

impl Interactive {
  /// Numbers each control among the ones sharing its name, which is what
  /// names a radio button's appearance state, and records the names that more
  /// than one non-radio control claims.
  fn resolve_field_names(&mut self) {
    let mut counts: HashMap<String, usize> = HashMap::new();

    for field in &mut self.fields {
      let index = counts.entry(field.name.clone()).or_default();

      field.group_index = *index;
      *index += 1;

      if *index == 2 && !matches!(field.field, FieldKind::Radio { .. }) {
        self.duplicate_field_names.push(field.name.clone());
      }
    }
  }
}

/// Whether the tag names a form control, whose widget annotation replaces the
/// content its box would otherwise contribute.
pub(crate) fn is_form_control(tag: &str) -> bool {
  ["input", "textarea", "select"]
    .iter()
    .any(|control| tag.eq_ignore_ascii_case(control))
}

/// Reads the control an element asks for, absent when it is not one.
fn field_kind(source: &Node, node: &RenderNode) -> Option<FieldKind> {
  let tag = source.tag_name()?;
  let max_len = source
    .attribute("maxlength")
    .and_then(|value| value.parse().ok());

  if tag.eq_ignore_ascii_case("textarea") {
    return Some(FieldKind::Text {
      value: source.attribute("value").unwrap_or_default().to_string(),
      multiline: true,
      password: false,
      max_len,
    });
  }

  if tag.eq_ignore_ascii_case("select") {
    let options = option_labels(node);

    return Some(FieldKind::Choice {
      value: source
        .attribute("value")
        .map(str::to_string)
        .or_else(|| options.first().map(|(display, _)| display.clone()))
        .unwrap_or_default(),
      options,
      multi: source.attribute("multiple").is_some(),
    });
  }

  if !tag.eq_ignore_ascii_case("input") {
    return None;
  }

  let value = source.attribute("value").unwrap_or_default().to_string();
  let checked = source.attribute("checked").is_some();

  let kind = source.attribute("type").unwrap_or("text");
  // An unnamed value submits as `on`, which is what a browser sends.
  let export = match value.is_empty() {
    true => "on".to_string(),
    false => value.clone(),
  };

  Some(match kind {
    kind if kind.eq_ignore_ascii_case("checkbox") => FieldKind::CheckBox {
      on: checked,
      export,
    },
    kind if kind.eq_ignore_ascii_case("radio") => FieldKind::Radio {
      export,
      on: checked,
    },
    _ => FieldKind::Text {
      value,
      multiline: false,
      password: kind.eq_ignore_ascii_case("password"),
      max_len,
    },
  })
}

/// Every `<option>` under a `<select>` as its label and the value it submits.
/// Layout moves the source children into the render tree, so both are read
/// from there.
fn option_labels(node: &RenderNode) -> Vec<(String, Option<String>)> {
  let mut labels = Vec::new();

  collect_option_labels(node, &mut labels);

  labels
}

fn collect_option_labels(node: &RenderNode, labels: &mut Vec<(String, Option<String>)>) {
  for child in node.children.as_deref().unwrap_or_default() {
    let is_option = child
      .node
      .as_ref()
      .and_then(Node::tag_name)
      .is_some_and(|tag| tag.eq_ignore_ascii_case("option"));

    match is_option {
      true => labels.push((
        option_text(child),
        child
          .node
          .as_ref()
          .and_then(|node| node.attribute("value"))
          .map(str::to_string),
      )),
      false => collect_option_labels(child, labels),
    }
  }
}

/// Every piece of text under one `<option>`, joined.
fn option_text(node: &RenderNode) -> String {
  let mut text = match node.node.as_ref().map(|node| &node.kind) {
    Some(NodeKind::Text(data)) => data.text.clone(),
    _ => String::new(),
  };

  if let Some(content) = &node.anonymous_text_content {
    text.push_str(content);
  }
  for child in node.children.as_deref().unwrap_or_default() {
    text.push_str(&option_text(child));
  }

  text
}

/// Link, heading and anchor targets collected from one tree.
pub(crate) struct Interactive {
  pub(crate) links: Vec<LinkTarget>,
  pub(crate) fields: Vec<FieldTarget>,
  /// `<label for>` text by the id it names, for field tooltips.
  pub(crate) labels: HashMap<String, String>,
  /// Names more than one non-radio control claims.
  pub(crate) duplicate_field_names: Vec<String>,
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
    let mut collected = Self {
      links: Vec::new(),
      fields: Vec::new(),
      labels: HashMap::new(),
      duplicate_field_names: Vec::new(),
      headings: Vec::new(),
      anchors: HashMap::new(),
      extents: HashMap::new(),
    };

    tree.for_each_paint(|paint| collect_interactive_paint(tree, paint, &mut collected));
    collected.resolve_field_names();
    collected.headings.sort_by(|a, b| a.top.total_cmp(&b.top));
    collected
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

  if source
    .tag_name()
    .is_some_and(|tag| tag.eq_ignore_ascii_case("label"))
    && let Some(target) = source.attribute("for")
  {
    let text = text_content(node);

    if !text.trim().is_empty() {
      collected
        .labels
        .entry(target.to_string())
        .or_insert_with(|| text.trim().to_string());
    }
  }

  if let Some(field) = field_kind(source, node)
    && let Some(name) = source
      .attribute("name")
      .or_else(|| source.id())
      .filter(|name| !name.is_empty())
  {
    let style = &node.context.style;
    let color = style.color.resolve(Color([0, 0, 0, 255]));

    collected.fields.push(FieldTarget {
      name: name.to_string(),
      rect,
      field,
      described: ["aria-label", "title", "placeholder"]
        .into_iter()
        .find_map(|attribute| source.attribute(attribute))
        .map(str::to_string),
      id: source.id().map(str::to_string),
      path: paint.path.clone(),
      group_index: 0,
      style: FieldStyle {
        color: pdf_color(color).unwrap_or([0.0, 0.0, 0.0]),
        font_size: node.context.sizing.font_size * PT_PER_PX,
        background: pdf_color(style.background_color.resolve(color)),
        border: pdf_color(style.border_top_color.resolve(color))
          .filter(|_| layout.border.top > 0.0)
          .map(|border| (border, layout.border.top * PT_PER_PX)),
        align: match style.text_align {
          TextAlign::Center => 1,
          TextAlign::Right | TextAlign::End => 2,
          _ => 0,
        },
        read_only: source.attribute("readonly").is_some() || source.attribute("disabled").is_some(),
        required: source.attribute("required").is_some(),
      },
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

/// Adds this page's slice of every form field as widget annotations.
pub(crate) fn add_field_annotations(
  page: &mut Page,
  interactive: &Interactive,
  window: Window,
  offset: (f32, f32),
  state: &DocumentState,
) {
  if state.form.is_none() {
    return;
  }
  let (y0, y1) = window.y.unwrap_or((f32::NEG_INFINITY, f32::INFINITY));

  for field in &interactive.fields {
    // A control split down the middle is not a control. The whole box has to
    // land on this page or it belongs to another one.
    if field.rect.top() < y0 || field.rect.bottom() > y1 {
      continue;
    }
    let Some(rect) = KrillaRect::from_ltrb(
      (field.rect.left() + offset.0) * PT_PER_PX,
      (field.rect.top() - y0 + offset.1) * PT_PER_PX,
      (field.rect.right() + offset.0) * PT_PER_PX,
      (field.rect.bottom() - y0 + offset.1) * PT_PER_PX,
    ) else {
      continue;
    };
    let described = field.described.clone().or_else(|| {
      field
        .id
        .as_ref()
        .and_then(|id| interactive.labels.get(id))
        .cloned()
    });
    let annotation = Annotation::new_widget(
      WidgetAnnotation::new(
        rect,
        field.name.clone(),
        field.field.to_form_field(field.group_index),
        field.style.to_widget(),
      )
      .with_description(described.clone())
      .with_lang(state.lang.map(str::to_string)),
      // PDF/UA reads a field's name through `/TU`, and krilla asks every
      // annotation for alt text; the field's own label answers both.
      Some(described.unwrap_or_else(|| field.name.clone())),
    );

    match state.tags.as_ref() {
      Some(tags) => {
        let identifier = page.add_tagged_annotation(annotation);

        tags.borrow_mut().record_annotation(&field.path, identifier);
      }
      None => page.add_annotation(annotation),
    }
  }
}

impl FieldKind {
  fn to_form_field(&self, index: usize) -> FormField {
    match self {
      FieldKind::CheckBox { on, export } => FormField::CheckBox {
        on: *on,
        export: export.clone(),
      },
      FieldKind::Radio { on, export } => FormField::Radio {
        index,
        export: export.clone(),
        on: *on,
      },
      FieldKind::Text {
        value,
        multiline,
        password,
        max_len,
      } => FormField::Text {
        value: value.clone(),
        multiline: *multiline,
        password: *password,
        max_len: *max_len,
      },
      FieldKind::Choice {
        value,
        options,
        multi,
      } => FormField::Choice {
        value: value.clone(),
        options: options.clone(),
        multi: *multi,
      },
    }
  }
}
