//! The character data of `text` elements, after the white space rules of Blink's
//! `LayoutSVGInlineText` and its `xml:space` handling.

use std::sync::Arc;

use roxmltree::Node as XmlNode;

use super::{Builder, NodeId, NodeKind, ParseError, svg_tag};
use crate::svg::{name::Tag, style::SvgStyle};

impl<'a, 'input: 'a> Builder<'a, 'input> {
  /// Appends the `tspan`, `textPath`, and `tref` children and the text of a `text` element.
  pub(super) fn text(
    &mut self,
    node: XmlNode<'a, 'input>,
    id: NodeId,
    style: &Arc<SvgStyle>,
  ) -> Result<(), ParseError> {
    self.text_children(node, id, style)?;
    self.trim_text_nodes(id);
    Ok(())
  }

  fn text_children(
    &mut self,
    parent: XmlNode<'a, 'input>,
    parent_id: NodeId,
    parent_style: &Arc<SvgStyle>,
  ) -> Result<(), ParseError> {
    for node in parent.children() {
      if node.is_text() {
        let text = collapse(node.text().unwrap_or(""), parent_style.preserve_white_space);

        self.append(parent_id, NodeKind::Text(text))?;
        continue;
      }

      let Some(mut tag) = svg_tag(node) else {
        continue;
      };

      if tag == Tag::A {
        tag = Tag::Tspan;
      }
      if !matches!(tag, Tag::Tspan | Tag::Tref | Tag::TextPath) {
        continue;
      }
      if tag == Tag::TextPath && parent.tag_name().name() != "text" {
        continue;
      }

      let is_tref = tag == Tag::Tref;

      if is_tref {
        tag = Tag::Tspan;
      }

      let id = self.element(node, parent_id, tag, parent_style, false)?;
      let style = self.style_of(id);

      if is_tref {
        if let Some(text) = self.referenced_text(node) {
          self.append(
            id,
            NodeKind::Text(collapse(&text, style.preserve_white_space)),
          )?;
        }
      } else {
        self.text_children(node, id, &style)?;
      }
    }
    Ok(())
  }

  /// All character data of the element a `tref` names.
  fn referenced_text(&self, node: XmlNode<'a, 'input>) -> Option<String> {
    let target = self.xml_link(node)?;

    svg_tag(target)?;

    let text: String = target
      .descendants()
      .filter(roxmltree::Node::is_text)
      .filter_map(|text| text.text())
      .collect();

    (!text.is_empty()).then_some(text)
  }

  fn preserves_white_space(&self, id: NodeId) -> bool {
    self.style_of(id).preserve_white_space
  }

  fn text_of(&self, id: NodeId) -> &str {
    match &self.document.nodes[id.index()].kind {
      NodeKind::Text(text) => text,
      _ => "",
    }
  }

  fn remove_first_space(&mut self, id: NodeId) {
    if let NodeKind::Text(text) = &mut self.document.nodes[id.index()].kind {
      text.remove(0);
    }
  }

  fn remove_last_space(&mut self, id: NodeId) {
    if let NodeKind::Text(text) = &mut self.document.nodes[id.index()].kind {
      text.pop();
    }
  }

  fn text_nodes(&self, parent: NodeId, depth: usize, nodes: &mut Vec<(NodeId, usize)>) {
    let mut child = self.document.nodes[parent.index()]
      .children
      .map(|(first, _)| first);

    while let Some(id) = child {
      match &self.document.nodes[id.index()].kind {
        NodeKind::Text(_) => nodes.push((id, depth)),
        NodeKind::Element { .. } => self.text_nodes(id, depth + 1, nodes),
        NodeKind::Root => {}
      }
      child = self.document.nodes[id.index()].next_sibling;
    }
  }

  /// Collapses the spaces that fall across adjacent text nodes, as Chrome does.
  fn trim_text_nodes(&mut self, text_element: NodeId) {
    let preserve = self.preserves_white_space(text_element);
    let mut nodes = Vec::new();

    self.text_nodes(text_element, 0, &mut nodes);

    if let [(only, _)] = nodes[..] {
      if !preserve {
        let text = self.text_of(only);

        match text.len() {
          0 => {}
          1 if text == " " => {
            if let NodeKind::Text(text) = &mut self.document.nodes[only.index()].kind {
              text.clear();
            }
          }
          _ => {
            let (first, last) = (text.starts_with(' '), text.ends_with(' '));

            if first {
              self.remove_first_space(only);
            }
            if last {
              self.remove_last_space(only);
            }
          }
        }
      }
      return;
    }
    if nodes.len() < 2 {
      return;
    }

    let last_pair = nodes.len() - 2;
    let mut last_non_empty: Option<NodeId> = None;

    for index in 0..nodes.len() - 1 {
      let (mut first_id, first_depth) = nodes[index];
      let (second_id, second_depth) = nodes[index + 1];

      if self.text_of(first_id).is_empty()
        && let Some(previous) = last_non_empty
      {
        first_id = previous;
      }

      let first_preserves = self.parent_preserves(first_id);
      let second_preserves = self.parent_preserves(second_id);
      let first_text = self.text_of(first_id).as_bytes();
      let second_text = self.text_of(second_id).as_bytes();
      let (c1, c2) = (first_text.first().copied(), first_text.last().copied());
      let (c3, c4) = (second_text.first().copied(), second_text.last().copied());

      if first_depth < second_depth {
        if c3 == Some(b' ') && !second_preserves {
          self.remove_first_space(second_id);
        }
      } else if c2 == Some(b' ') && c2 == c3 {
        if !first_preserves && !second_preserves {
          self.remove_last_space(first_id);
        } else if first_preserves && !second_preserves {
          self.remove_first_space(second_id);
        }
      }

      let is_first = index == 0;
      let is_last = index == last_pair;

      if is_first && c1 == Some(b' ') && !first_preserves && !self.text_of(first_id).is_empty() {
        self.remove_first_space(first_id);
      } else if is_last
        && c4 == Some(b' ')
        && !self.text_of(second_id).is_empty()
        && !second_preserves
      {
        self.remove_last_space(second_id);
      }

      if is_last
        && c2 == Some(b' ')
        && !self.text_of(first_id).is_empty()
        && self.text_of(second_id).is_empty()
        && self.text_of(first_id).ends_with(' ')
      {
        self.remove_last_space(first_id);
      }

      if !self.text_of(first_id).trim().is_empty() {
        last_non_empty = Some(first_id);
      }
    }
  }

  fn parent_preserves(&self, text: NodeId) -> bool {
    self.document.nodes[text.index()]
      .parent
      .is_some_and(|parent| self.preserves_white_space(parent))
  }
}

/// Turns tabs and line feeds into spaces, and collapses runs of spaces unless `preserve`.
fn collapse(text: &str, preserve: bool) -> String {
  let mut collapsed = String::with_capacity(text.len());
  let mut previous = '0';

  for character in text.chars() {
    let character = match character {
      '\r' | '\n' | '\t' => ' ',
      other => other,
    };

    if !preserve && character == ' ' && previous == ' ' {
      continue;
    }
    previous = character;
    collapsed.push(character);
  }
  collapsed
}
