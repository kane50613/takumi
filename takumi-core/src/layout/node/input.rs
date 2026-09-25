use std::fmt;

use serde::{
  Deserialize, Deserializer,
  de::{Error, IgnoredAny, MapAccess, Visitor},
};

use crate::layout::node::{ImageData, Node, NodeKind, NodeMetadata, TextData};

impl<'de> Deserialize<'de> for Node {
  fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
    deserializer.deserialize_map(NodeVisitor)
  }
}

#[derive(Deserialize)]
#[serde(field_identifier, rename_all = "camelCase")]
enum NodeField {
  Type,
  Children,
  Text,
  Src,
  Width,
  Height,
  TagName,
  ClassName,
  Id,
  Attributes,
  Preset,
  Style,
  Tw,
  Dir,
  Lang,
  #[serde(other)]
  Other,
}

#[derive(Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
enum NodeType {
  Container,
  Image,
  Text,
}

struct NodeVisitor;

impl<'de> Visitor<'de> for NodeVisitor {
  type Value = Node;

  fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
    formatter.write_str("a node")
  }

  fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Node, A::Error> {
    let mut node_type = None;
    let mut metadata = NodeMetadata::default();
    let mut children = None;
    let mut text = None;
    let mut src = None;
    let mut width = None;
    let mut height = None;

    while let Some(field) = map.next_key::<NodeField>()? {
      // Once `type` is read, a key another node type owns is skipped unread.
      let owns = |kind: NodeType| node_type.is_none_or(|known| known == kind);

      match field {
        NodeField::Type => node_type = Some(map.next_value()?),
        NodeField::Children if owns(NodeType::Container) => children = map.next_value()?,
        NodeField::Text if owns(NodeType::Text) => text = Some(map.next_value()?),
        NodeField::Src if owns(NodeType::Image) => src = Some(map.next_value()?),
        NodeField::Width if owns(NodeType::Image) => width = map.next_value()?,
        NodeField::Height if owns(NodeType::Image) => height = map.next_value()?,
        NodeField::TagName => metadata.tag_name = map.next_value()?,
        NodeField::ClassName => metadata.class_name = map.next_value()?,
        NodeField::Id => metadata.id = map.next_value()?,
        NodeField::Attributes => metadata.attributes = map.next_value()?,
        NodeField::Preset => metadata.preset = map.next_value()?,
        NodeField::Style => metadata.style = map.next_value()?,
        NodeField::Tw => metadata.tw = map.next_value()?,
        NodeField::Dir => metadata.dir = map.next_value()?,
        NodeField::Lang => metadata.lang = map.next_value()?,
        _ => {
          map.next_value::<IgnoredAny>()?;
        }
      }
    }

    let kind = match node_type.ok_or_else(|| Error::missing_field("type"))? {
      NodeType::Container => NodeKind::Container {
        children: children.unwrap_or_default(),
      },
      NodeType::Image => NodeKind::Image(ImageData {
        src: src.ok_or_else(|| Error::missing_field("src"))?,
        width,
        height,
      }),
      NodeType::Text => NodeKind::Text(TextData {
        text: text.ok_or_else(|| Error::missing_field("text"))?,
      }),
    };

    Ok(Node { metadata, kind })
  }
}

#[cfg(test)]
mod tests {
  use serde_json::{from_str, from_value, json};

  use crate::layout::node::{Node, NodeKind};

  fn error(value: serde_json::Value) -> String {
    from_value::<Node>(value).unwrap_err().to_string()
  }

  #[test]
  fn reads_the_type_after_other_keys() {
    let node: Node = from_value(json!({
      "children": [{ "text": "a", "type": "text" }],
      "className": "card",
      "type": "container",
    }))
    .unwrap();

    assert_eq!(node.metadata.class_name.as_deref(), Some("card"));
    assert!(matches!(&node.kind, NodeKind::Container { children } if children.len() == 1));
  }

  #[test]
  fn skips_keys_another_type_owns_after_the_type() {
    let node: Node = from_str(
      r#"{ "type": "text", "text": "a", "children": "not a list", "src": 1, "width": "wide" }"#,
    )
    .unwrap();

    assert!(matches!(&node.kind, NodeKind::Text(data) if data.text == "a"));
  }

  #[test]
  fn reads_keys_another_type_owns_before_the_type() {
    let error = from_str::<Node>(r#"{ "children": "not a list", "type": "text", "text": "a" }"#)
      .unwrap_err()
      .to_string();

    assert!(error.contains("expected a sequence"), "{error}");
  }

  #[test]
  fn defaults_missing_and_null_children_to_empty() {
    for node in [
      json!({ "type": "container" }),
      json!({ "type": "container", "children": null }),
    ] {
      let node: Node = from_value(node).unwrap();

      assert!(matches!(&node.kind, NodeKind::Container { children } if children.is_empty()));
    }
  }

  #[test]
  fn names_a_missing_or_unknown_type() {
    assert!(error(json!({ "text": "a" })).contains("missing field `type`"));
    assert!(error(json!({ "type": "video" })).contains("unknown variant `video`"));
    assert!(error(json!({ "type": "text" })).contains("missing field `text`"));
    assert!(error(json!({ "type": "image" })).contains("missing field `src`"));
  }
}
