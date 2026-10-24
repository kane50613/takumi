//! References that lead back to themselves are cut, as Blink's `SVGResourcesCycleSolver` does by
//! dropping the reference that closes the cycle.

use std::sync::Arc;

use super::{Document, NodeId, NodeKind};
use crate::svg::{
  Attr, Paint, Tag,
  style::{FilterValue, SvgStyle},
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Link {
  Fill,
  Stroke,
  ClipPath,
  Mask,
  Filter,
}

impl Document {
  pub(super) fn cut_cycles(&mut self) {
    for paint in [Link::Fill, Link::Stroke] {
      while let Some(id) = self.recursive_pattern_paint(paint) {
        self.cut(id, paint);
      }
    }
    for (tag, link) in [
      (Tag::ClipPath, Link::ClipPath),
      (Tag::Mask, Link::Mask),
      (Tag::Filter, Link::Filter),
    ] {
      while let Some(id) = self.recursive_link(tag, link) {
        self.cut(id, link);
      }
    }
    self.cut_fe_image_cycles();
  }

  fn style_mut(&mut self, id: NodeId) -> Option<&mut SvgStyle> {
    match &mut self.nodes[id.index()].kind {
      NodeKind::Element { style, .. } => Some(Arc::make_mut(style)),
      _ => None,
    }
  }

  fn cut(&mut self, id: NodeId, link: Link) {
    let Some(style) = self.style_mut(id) else {
      return;
    };

    match link {
      Link::Fill => style.fill = Paint::None,
      Link::Stroke => style.stroke = Paint::None,
      Link::ClipPath => style.clip_path = None,
      Link::Mask => style.mask = None,
      Link::Filter => style.filter.clear(),
    }
  }

  fn nodes_with(&self, tag: Tag) -> Vec<NodeId> {
    self
      .nodes
      .iter()
      .enumerate()
      .filter(
        |(_, data)| matches!(&data.kind, NodeKind::Element { tag: found, .. } if *found == tag),
      )
      .map(|(index, _)| NodeId::new(index))
      .collect()
  }

  /// The id a paint of `node` names, when it is a `url()`.
  fn paint_target(&self, node: NodeId, paint: Link) -> Option<&str> {
    let style = self.node(node).style()?;
    let paint = if paint == Link::Fill {
      &style.fill
    } else {
      &style.stroke
    };

    match paint {
      Paint::Url(id, _) => Some(id),
      _ => None,
    }
  }

  fn recursive_pattern_paint(&self, paint: Link) -> Option<NodeId> {
    for pattern in self.nodes_with(Tag::Pattern) {
      let pattern_id = self.node(pattern).element_id();

      for node in self.node(pattern).descendants() {
        let Some(target) = self.paint_target(node.id(), paint) else {
          continue;
        };

        if target == pattern_id {
          return Some(node.id());
        }
        if let Some(linked) = self.element_by_id(target) {
          for inner in linked.descendants() {
            if self.paint_target(inner.id(), paint) == Some(pattern_id) {
              return Some(inner.id());
            }
          }
        }
      }
    }
    None
  }

  /// The elements a `clip-path`, `mask`, or `filter` of `node` names.
  fn targets(&self, node: NodeId, link: Link) -> Vec<NodeId> {
    let Some(style) = self.node(node).style() else {
      return Vec::new();
    };
    let ids: Vec<&str> = match link {
      Link::ClipPath => style.clip_path.as_deref().into_iter().collect(),
      Link::Mask => style.mask.as_deref().into_iter().collect(),
      Link::Filter => style
        .filter
        .iter()
        .filter_map(|filter| match filter {
          FilterValue::Url(id) => Some(&**id),
          FilterValue::Function(_) => None,
        })
        .collect(),
      Link::Fill | Link::Stroke => Vec::new(),
    };

    ids
      .into_iter()
      .filter_map(|id| self.element_by_id(id).map(|element| element.id()))
      .collect()
  }

  fn recursive_link(&self, tag: Tag, link: Link) -> Option<NodeId> {
    for element in self.nodes_with(tag) {
      for child in self.node(element).descendants() {
        for target in self.targets(child.id(), link) {
          if target == element {
            return Some(child.id());
          }
          for inner in self.node(target).descendants() {
            if self.targets(inner.id(), link).contains(&element) {
              return Some(inner.id());
            }
          }
        }
      }
    }
    None
  }

  /// An `feImage` that draws an element filtered by the filter that holds the `feImage`.
  fn cut_fe_image_cycles(&mut self) {
    let mut filtered = Vec::new();

    for fe_image in self.nodes_with(Tag::FeImage) {
      let node = self.node(fe_image);
      let (Some(link), Some(filter)) = (node.linked(Attr::Href), node.parent()) else {
        continue;
      };
      let filter_id = filter.element_id();

      if link.style().is_some_and(|style| {
        style
          .filter
          .contains(&FilterValue::Url(Arc::from(filter_id)))
      }) {
        filtered.push(link.id());
      }
    }
    for id in filtered {
      self.cut(id, Link::Filter);
    }
  }
}
