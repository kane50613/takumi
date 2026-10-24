//! The elements and characters of a `text`, after usvg's text tree: white space collapsed as the
//! SVG white space rules and Chrome have it, the characters grouped into anchored chunks, and each
//! run of characters given the style of the element it sits in.

use std::{collections::HashMap, rc::Rc};

use super::{
  BaselineOffset, DecorationStyle, Decorations, LengthAdjust, Scope, SpanStyle, Styled, SvgLayout,
  TextChunk, TextFlow, TextPath, TextPathShape, TextSpan,
  attributes::CharacterPosition,
  shape::{FontFaces, TextFont},
};
use crate::{
  font_style::SizedFontStyle,
  fonts::font_description::FontDescription,
  geometry::{PathCommand, Point},
  layout::node::NodeKind,
  style::{Affine, BaselineShift, DominantBaseline, Length, TextDecorationLines, Visibility},
  svg::{element::SvgElementKind, length::LengthAxis},
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum XmlSpace {
  Default,
  Preserve,
}

impl XmlSpace {
  fn parse(value: &str) -> Self {
    if value == "preserve" {
      Self::Preserve
    } else {
      Self::Default
    }
  }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ElementKind {
  Text,
  /// A `tspan`, a `tref`, or an `a`.
  Span,
  TextPath,
}

struct TextElement {
  kind: ElementKind,
  styled: Styled,
  space: Option<XmlSpace>,
  decorations: Decorations,
  baseline_offset: BaselineOffset,
}

enum Content {
  Element(Box<TextElement>),
  Text(String),
}

struct TextNode {
  parent: Option<usize>,
  children: Vec<usize>,
  content: Content,
  /// The characters of the node and everything below it.
  characters: usize,
}

/// The elements and characters of a `text`, with the text element as node zero.
pub(super) struct TextTree {
  nodes: Vec<TextNode>,
  /// The decoration lines declared by the elements above the `text`.
  outer_decorations: TextDecorationLines,
}

/// Where a walk over the characters of the tree stands.
struct ChunkState {
  /// Set once the budget is spent, which ends the walk.
  exhausted: bool,
  /// The style of the characters of each element, made once.
  styles: HashMap<usize, Option<Rc<SpanStyle>>>,
  chars: usize,
  chunk_bytes: usize,
  split_chunk: bool,
  flow: TextFlow,
  chunks: Vec<TextChunk>,
}

impl TextTree {
  pub(super) fn new(layout: &mut SvgLayout, scope: &Scope, text: &Styled) -> Self {
    let mut tree = Self {
      nodes: Vec::new(),
      outer_decorations: layout.outer_decorations(text.index),
    };
    let text_space = layout.inherited_space(text.index);
    let root = TextElement {
      kind: ElementKind::Text,
      styled: text.clone(),
      space: layout.own_space(text.index),
      decorations: tree.decorations(layout, scope, text, None),
      baseline_offset: BaselineOffset::default(),
    };
    let root = tree.push(None, Content::Element(Box::new(root)));

    tree.add_children(layout, scope, root, text_space);
    tree.collapse_spaces(text_space);
    tree.count_characters();
    tree
  }

  /// The count of the characters in the subtree of `node`.
  pub(super) fn count_chars(&self, node: usize) -> usize {
    self.nodes[node].characters
  }

  /// Counts the characters of every subtree in one pass, children before their parents.
  fn count_characters(&mut self) {
    for id in (0..self.nodes.len()).rev() {
      let own = self.text(id).map_or(0, |text| text.chars().count());
      let below: usize = self.nodes[id]
        .children
        .iter()
        .map(|&child| self.nodes[child].characters)
        .sum();

      self.nodes[id].characters = own + below;
    }
  }

  /// The nodes of the subtree of `node` in document order, `node` first.
  pub(super) fn descendants(&self, node: usize) -> Vec<usize> {
    let mut order = Vec::new();
    let mut stack = vec![node];

    while let Some(id) = stack.pop() {
      order.push(id);
      stack.extend(self.nodes[id].children.iter().rev());
    }

    order
  }

  /// The element at `node` and its `styled`, none for a text node.
  pub(super) fn element(&self, node: usize) -> Option<&Styled> {
    match &self.nodes[node].content {
      Content::Element(element) => Some(&element.styled),
      Content::Text(_) => None,
    }
  }

  /// Whether the node is a `textPath` element.
  pub(super) fn is_text_path(&self, node: usize) -> bool {
    self.kind(node) == Some(ElementKind::TextPath)
  }

  fn text(&self, node: usize) -> Option<&str> {
    match &self.nodes[node].content {
      Content::Text(text) => Some(text),
      Content::Element(_) => None,
    }
  }

  fn text_mut(&mut self, node: usize) -> Option<&mut String> {
    match &mut self.nodes[node].content {
      Content::Text(text) => Some(text),
      Content::Element(_) => None,
    }
  }

  fn kind(&self, node: usize) -> Option<ElementKind> {
    match &self.nodes[node].content {
      Content::Element(element) => Some(element.kind),
      Content::Text(_) => None,
    }
  }

  fn push(&mut self, parent: Option<usize>, content: Content) -> usize {
    let id = self.nodes.len();

    self.nodes.push(TextNode {
      parent,
      children: Vec::new(),
      content,
      characters: 0,
    });

    if let Some(parent) = parent {
      self.nodes[parent].children.push(id);
    }

    id
  }

  /// The elements and text below `node`, each element costing an instance of the budget. False
  /// once the budget is spent, which ends the walk.
  fn add_children(
    &mut self,
    layout: &mut SvgLayout,
    scope: &Scope,
    node: usize,
    space: XmlSpace,
  ) -> bool {
    let Some(Content::Element(parent)) = self.nodes.get(node).map(|node| &node.content) else {
      return true;
    };
    let (parent_index, parent_context) = (parent.styled.index, parent.styled.context.clone());
    let (parent_decorations, parent_offset) = (parent.decorations.clone(), parent.baseline_offset);
    let document = layout.document;

    for &child in &document.elements[parent_index].children {
      let kind = document.elements[child].kind;

      if kind == SvgElementKind::CharacterData {
        let raw = layout.character_data(child);

        if layout.budget.spend_text_bytes(raw.len()) {
          self.push(Some(node), Content::Text(collapse_text(raw, space)));
        }

        continue;
      }

      let element_kind = match kind {
        SvgElementKind::Tspan | SvgElementKind::Tref => ElementKind::Span,
        SvgElementKind::G if layout.styles.node(child).tag_name() == Some("a") => ElementKind::Span,
        // A `textPath` is only a direct child of a `text`.
        SvgElementKind::TextPath if self.kind(node) == Some(ElementKind::Text) => {
          ElementKind::TextPath
        }
        _ => continue,
      };

      if !layout.budget.spend_instance() {
        return false;
      }

      let own_space = layout.own_space(child);
      let child_space = own_space.unwrap_or(space);
      let styled = layout.styled(child, &parent_context);
      let element = TextElement {
        kind: element_kind,
        decorations: self.decorations(layout, scope, &styled, Some(&parent_decorations)),
        baseline_offset: baseline_offset(&styled, parent_offset),
        styled,
        space: own_space,
      };
      let id = self.push(Some(node), Content::Element(Box::new(element)));

      if kind == SvgElementKind::Tref {
        if let Some(text) = layout.referenced_text(child) {
          self.push(Some(id), Content::Text(collapse_text(&text, child_space)));
        }
      } else {
        if !layout.budget.enter() {
          return false;
        }

        let completed = self.add_children(layout, scope, id, child_space);

        layout.budget.leave();

        if !completed {
          return false;
        }
      }
    }

    true
  }

  /// The decorations of an element: each line is drawn in the fill and stroke of the nearest
  /// element that declares it, up to the `text`, which stands for the elements above it.
  fn decorations(
    &self,
    layout: &mut SvgLayout,
    scope: &Scope,
    styled: &Styled,
    parent: Option<&Decorations>,
  ) -> Decorations {
    let declared = styled
      .style()
      .visual_data
      .text_decoration_line
      .unwrap_or_default();
    let declared = match parent {
      Some(_) => declared,
      None => declared | self.outer_decorations,
    };
    let mut own = None;
    let mut line = |layout: &mut SvgLayout,
                    line: TextDecorationLines,
                    inherited: Option<&Rc<DecorationStyle>>| {
      if declared.contains(line) {
        Some(Rc::clone(own.get_or_insert_with(|| {
          Rc::new(DecorationStyle {
            fill: layout.resolve_fill(styled, true, scope),
            stroke: layout.resolve_stroke(styled, true, scope),
          })
        })))
      } else {
        inherited.map(Rc::clone)
      }
    };

    Decorations {
      underline: line(
        layout,
        TextDecorationLines::UNDERLINE,
        parent.and_then(|parent| parent.underline.as_ref()),
      ),
      overline: line(
        layout,
        TextDecorationLines::OVERLINE,
        parent.and_then(|parent| parent.overline.as_ref()),
      ),
      line_through: line(
        layout,
        TextDecorationLines::LINE_THROUGH,
        parent.and_then(|parent| parent.line_through.as_ref()),
      ),
    }
  }

  /// The `xml:space` that applies to the element a text node sits in.
  fn space_of_parent(&self, node: usize, text_space: XmlSpace) -> XmlSpace {
    self.nodes[node]
      .parent
      .and_then(|parent| match &self.nodes[parent].content {
        Content::Element(element) => element.space,
        Content::Text(_) => None,
      })
      .unwrap_or(text_space)
  }

  /// The text nodes of the tree with their depth, in document order.
  fn text_nodes(&self) -> Vec<(usize, usize)> {
    let mut nodes = Vec::new();
    let mut stack = vec![(0, 0)];

    while let Some((node, depth)) = stack.pop() {
      if self.text(node).is_some() {
        nodes.push((node, depth));
        continue;
      }

      for &child in self.nodes[node].children.iter().rev() {
        stack.push((child, depth + 1));
      }
    }

    nodes
  }

  fn remove_first_space(&mut self, node: usize) {
    if let Some(text) = self.text_mut(node)
      && text.starts_with(' ')
    {
      text.remove(0);
    }
  }

  fn remove_last_space(&mut self, node: usize) {
    if let Some(text) = self.text_mut(node)
      && text.ends_with(' ')
    {
      text.pop();
    }
  }

  fn is_empty_text(&self, node: usize) -> bool {
    self.text(node).is_none_or(str::is_empty)
  }

  /// Removes the spaces that the SVG white space rules and Chrome drop across text nodes, after
  /// usvg's `trim_text_nodes`.
  /// https://www.w3.org/TR/SVG11/text.html#WhiteSpace
  fn collapse_spaces(&mut self, text_space: XmlSpace) {
    let nodes = self.text_nodes();

    // Each text node has already collapsed its own runs of spaces, so at most one is left at each
    // end.
    if nodes.len() == 1 {
      if text_space == XmlSpace::Default {
        let node = nodes[0].0;

        if self.text(node) == Some(" ") {
          if let Some(text) = self.text_mut(node) {
            text.clear();
          }
        } else if self.text(node).is_some_and(|text| text.len() >= 2) {
          self.remove_first_space(node);
          self.remove_last_space(node);
        }
      }

      return;
    }

    let last = nodes.len().saturating_sub(1);
    let mut last_non_empty = None;

    for index in 0..last {
      let (mut node1, depth1) = nodes[index];
      let (node2, depth2) = nodes[index + 1];

      if self.is_empty_text(node1)
        && let Some(previous) = last_non_empty
      {
        node1 = previous;
      }

      let space1 = self.space_of_parent(node1, text_space);
      let space2 = self.space_of_parent(node2, text_space);
      let edge = |text: Option<&str>| {
        text.map(|text| {
          (
            text.as_bytes().first().copied(),
            text.as_bytes().last().copied(),
          )
        })
      };
      let (first1, last1) = edge(self.text(node1)).unwrap_or_default();
      let (first2, last2) = edge(self.text(node2)).unwrap_or_default();

      if depth1 < depth2 {
        if first2 == Some(b' ') && space2 == XmlSpace::Default {
          self.remove_first_space(node2);
        }
      } else if last1 == Some(b' ') && last1 == first2 {
        if space1 == XmlSpace::Default && space2 == XmlSpace::Default {
          self.remove_last_space(node1);
        } else if space1 == XmlSpace::Preserve && space2 == XmlSpace::Default {
          self.remove_first_space(node2);
        }
      }

      let is_first = index == 0;
      let is_last = index == last - 1;

      if is_first
        && first1 == Some(b' ')
        && space1 == XmlSpace::Default
        && !self.is_empty_text(node1)
      {
        self.remove_first_space(node1);
      } else if is_last
        && last2 == Some(b' ')
        && !self.is_empty_text(node2)
        && space2 == XmlSpace::Default
      {
        self.remove_last_space(node2);
      }

      if is_last && last1 == Some(b' ') && !self.is_empty_text(node1) && self.is_empty_text(node2) {
        self.remove_last_space(node1);
      }

      if self.text(node1).is_some_and(|text| !text.trim().is_empty()) {
        last_non_empty = Some(node1);
      }
    }
  }

  /// The chunks of the characters, with the style of the element each one sits in. None of the
  /// spans of a chunk has a font when no font matches.
  pub(super) fn chunks(
    &self,
    layout: &mut SvgLayout,
    scope: &Scope,
    positions: &[CharacterPosition],
    faces: &mut FontFaces,
  ) -> Vec<TextChunk> {
    let mut state = ChunkState {
      exhausted: false,
      styles: HashMap::new(),
      chars: 0,
      chunk_bytes: 0,
      split_chunk: false,
      flow: TextFlow::Linear,
      chunks: Vec::new(),
    };

    self.collect_chunks(layout, scope, positions, faces, 0, &mut state);
    state.chunks
  }

  fn collect_chunks(
    &self,
    layout: &mut SvgLayout,
    scope: &Scope,
    positions: &[CharacterPosition],
    faces: &mut FontFaces,
    parent: usize,
    state: &mut ChunkState,
  ) {
    for &child in &self.nodes[parent].children {
      if state.exhausted {
        return;
      }

      if let Some(kind) = self.kind(child) {
        if kind == ElementKind::TextPath {
          let Some(styled) = self.element(child) else {
            continue;
          };

          // A `textPath` without characters sets nothing along its path.
          if self.count_chars(child) == 0 {
            state.split_chunk = true;
            continue;
          }

          match layout.text_flow(styled, scope) {
            Some(flow) => state.flow = flow,
            None => {
              // The characters of an invalid `textPath` still hold their place in the positions.
              state.chars += self.count_chars(child);
              continue;
            }
          }

          state.split_chunk = true;
        }

        if !layout.budget.enter() {
          state.exhausted = true;
          return;
        }

        self.collect_chunks(layout, scope, positions, faces, child, state);
        layout.budget.leave();

        if kind == ElementKind::TextPath {
          state.flow = TextFlow::Linear;
          state.split_chunk = true;
        }

        continue;
      }

      let text = self.text(child).unwrap_or_default();
      let style = match state.styles.get(&parent) {
        Some(style) => style.clone(),
        None => {
          let style = self.span_style(layout, scope, parent, faces);

          state.styles.insert(parent, style.clone());
          style
        }
      };

      let Some(style) = style else {
        state.chars += text.chars().count();
        continue;
      };

      self.push_text_run(layout, text, &style, positions, state, parent);
    }
  }

  /// Adds the characters of one text node to the chunks as spans of `style`. A new chunk costs an
  /// instance of the budget.
  fn push_text_run(
    &self,
    layout: &mut SvgLayout,
    text: &str,
    style: &Rc<SpanStyle>,
    positions: &[CharacterPosition],
    state: &mut ChunkState,
    parent: usize,
  ) {
    let anchor = self
      .element(parent)
      .map(|styled| styled.style().svg_inherited_data.text_anchor)
      .unwrap_or_default();
    let mut is_new_span = true;

    for character in text.chars() {
      let length = character.len_utf8();
      let position = positions.get(state.chars).copied().unwrap_or_default();
      // A chunk starts at the first span, at an absolute position, and around a `textPath`.
      let is_new_chunk = position.x.is_some()
        || position.y.is_some()
        || state.split_chunk
        || state.chunks.is_empty();

      state.split_chunk = false;

      if is_new_chunk {
        if !layout.budget.spend_instance() {
          state.exhausted = true;
          return;
        }

        state.chunk_bytes = 0;
        state.chunks.push(TextChunk {
          characters: vec![state.chars],
          x: position.x,
          y: position.y,
          anchor,
          spans: vec![TextSpan {
            start: 0,
            end: length,
            style: Rc::clone(style),
          }],
          flow: state.flow.clone(),
          text: character.to_string(),
        });
      } else if let Some(chunk) = state.chunks.last_mut() {
        chunk.text.push(character);
        chunk.characters.push(state.chars);

        if is_new_span {
          chunk.spans.push(TextSpan {
            start: state.chunk_bytes,
            end: state.chunk_bytes + length,
            style: Rc::clone(style),
          });
        } else if let Some(last) = chunk.spans.last_mut() {
          last.end += length;
        }
      }

      is_new_span = false;
      state.chars += 1;
      state.chunk_bytes += length;
    }
  }

  /// The style the characters of the element at `node` are drawn with, none when they are not
  /// drawn: the element is hidden or has no font size.
  fn span_style(
    &self,
    layout: &mut SvgLayout,
    scope: &Scope,
    node: usize,
    faces: &mut FontFaces,
  ) -> Option<Rc<SpanStyle>> {
    let Some(Content::Element(element)) = self.nodes.get(node).map(|node| &node.content) else {
      return None;
    };
    let styled = &element.styled;

    if !layout.is_visible_element(styled, scope) || styled.sizing().font_size <= 0.0 {
      return None;
    }

    let style = styled.style();
    let sized = SizedFontStyle::from_style(style, &styled.context);
    let font = Rc::new(styled.context.font(FontDescription::new(&sized)));
    let font = font.primary_font().and_then(|primary| {
      faces.resolved(&primary).map(|primary| TextFont {
        font: Rc::clone(&font),
        primary,
      })
    });
    let dominant_baseline = match style.svg_inherited_data.dominant_baseline {
      // `no-change` takes the value of the parent.
      DominantBaseline::NoChange => self.nodes[node]
        .parent
        .and_then(|parent| self.element(parent))
        .map(|parent| parent.style().svg_inherited_data.dominant_baseline)
        .unwrap_or_default(),
      baseline => baseline,
    };
    let text_length = layout
      .length_attribute(styled.index, "textLength")
      .map(|length| layout.px(styled, scope, length, LengthAxis::X))
      .filter(|length| *length >= 0.0);

    Some(Rc::new(SpanStyle {
      fill: layout.resolve_fill(styled, true, scope),
      stroke: layout.resolve_stroke(styled, true, scope),
      stroke_first: style.svg_inherited_data.paint_order.strokes_first(),
      font,
      decorations: element.decorations.clone(),
      visible: style.inherited_data.visibility == Visibility::Visible,
      dominant_baseline,
      alignment_baseline: style.svg_data.alignment_baseline,
      baseline_offset: element.baseline_offset,
      text_length,
      length_adjust: if layout.attribute(styled.index, "lengthAdjust") == Some("spacingAndGlyphs") {
        LengthAdjust::SpacingAndGlyphs
      } else {
        LengthAdjust::Spacing
      },
    }))
  }
}

/// The `baseline-shift` of an element stacked on that of the elements around it.
fn baseline_offset(styled: &Styled, around: BaselineOffset) -> BaselineOffset {
  let font_size = styled.sizing().font_size;

  match styled.style().svg_misc_data.baseline_shift {
    BaselineShift::Sub => BaselineOffset {
      superscripts: around.superscripts - 1,
      ..around
    },
    BaselineShift::Super => BaselineOffset {
      superscripts: around.superscripts + 1,
      ..around
    },
    BaselineShift::Length(length) => BaselineOffset {
      length: around.length + length.to_px(styled.sizing(), font_size),
      ..around
    },
  }
}

impl<'a> SvgLayout<'a> {
  /// The decoration lines the elements above the one at `index` declare.
  fn outer_decorations(&self, index: usize) -> TextDecorationLines {
    let mut lines = TextDecorationLines::empty();
    let mut outer = self.document.elements[index].parent;

    while let Some(ancestor) = outer {
      if let Some(declared) = self
        .styled_in_place(ancestor)
        .style()
        .visual_data
        .text_decoration_line
      {
        lines |= declared;
      }

      outer = self.document.elements[ancestor].parent;
    }

    lines
  }

  /// The `xml:space` an element sets, if it sets one.
  fn own_space(&self, index: usize) -> Option<XmlSpace> {
    self.attribute(index, "xml:space").map(XmlSpace::parse)
  }

  /// The `xml:space` that applies to an element: its own, or that of the nearest ancestor.
  fn inherited_space(&self, index: usize) -> XmlSpace {
    let mut current = Some(index);

    while let Some(id) = current {
      if let Some(space) = self.own_space(id) {
        return space;
      }

      current = self.document.elements[id].parent;
    }

    XmlSpace::Default
  }

  fn character_data(&self, index: usize) -> &'a str {
    match &self.styles.node(index).kind {
      NodeKind::Text(data) => &data.text,
      _ => "",
    }
  }

  /// The characters of the element a `tref` names, all of those below it included, none when it
  /// names no element or has no characters. The characters are charged to the text budget as the
  /// walk reads them, so it stops at the budget and a large text costs each `tref` what it holds.
  fn referenced_text(&mut self, index: usize) -> Option<String> {
    let id = self.attribute(index, "href")?.trim().strip_prefix('#')?;
    let target = *self.document.first_ids.get(id)?;

    if self.document.elements[target].kind == SvgElementKind::Unknown {
      return None;
    }

    let mut text = String::new();
    let mut stack = vec![target];

    while let Some(current) = stack.pop() {
      // Each element visited costs a byte, so a tree without characters ends too.
      let characters = match self.document.elements[current].kind {
        SvgElementKind::CharacterData => self.character_data(current),
        _ => "",
      };

      if !self.budget.spend_text_bytes(characters.len().max(1)) {
        return None;
      }

      text.push_str(characters);
      stack.extend(self.document.elements[current].children.iter().rev());
    }

    (!text.is_empty()).then_some(text)
  }

  /// The path a `textPath` runs along, none when it names no shape, the shape is invalid, or the
  /// path outgrows the budget. The path of a shape is resolved once, and each use is charged its
  /// commands.
  fn text_flow(&mut self, text_path: &Styled, scope: &Scope) -> Option<TextFlow> {
    let linked = self.href_target(text_path.index)?;
    let path = match self.text_paths.get(&linked) {
      Some(path) => Rc::clone(path),
      None => {
        let path = self.resolve_text_path(linked, scope)?;

        self.text_paths.insert(linked, Rc::clone(&path));
        path
      }
    };

    if !self.budget.spend_painted(path.commands.len()) {
      return None;
    }

    let start_offset = match self.length_attribute(text_path.index, "startOffset") {
      // A percentage is a distance along the whole path.
      Some(Length::Percentage(percent)) => (path.length() * f64::from(percent) / 100.0) as f32,
      Some(length) => self.px(text_path, scope, length, LengthAxis::X),
      None => 0.0,
    };

    Some(TextFlow::Path(TextPath { start_offset, path }))
  }

  /// The outline of the shape with its transform applied, and its length.
  fn resolve_text_path(&mut self, linked: usize, scope: &Scope) -> Option<Rc<TextPathShape>> {
    let linked_styled = self.styled_in_place(linked);

    if !linked_styled.kind.is_shape() {
      return None;
    }

    let commands = self.shape_commands(&linked_styled, scope)?;
    let transform = self.resolve_transform(&linked_styled, scope);
    let commands: Vec<PathCommand> = if transform.is_identity() {
      commands
    } else {
      commands
        .into_iter()
        .map(|command| command.map_points(|point| transformed(transform, point)))
        .collect()
    };

    Some(Rc::new(TextPathShape::new(commands)))
  }
}

fn transformed(transform: Affine, point: Point<f32>) -> Point<f32> {
  let (x, y) = transform.transform_point(point.x, point.y);

  Point::new(x, y)
}

/// `text` with its line breaks and tabs as spaces and, unless `space` preserves them, each run of
/// spaces as one.
fn collapse_text(text: &str, space: XmlSpace) -> String {
  let mut collapsed = String::with_capacity(text.len());
  let mut previous = None;

  for character in text.chars() {
    let character = match character {
      '\r' | '\n' | '\t' => ' ',
      other => other,
    };

    if space == XmlSpace::Default && character == ' ' && previous == Some(' ') {
      continue;
    }

    previous = Some(character);
    collapsed.push(character);
  }

  collapsed
}
