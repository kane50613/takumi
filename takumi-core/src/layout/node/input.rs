use std::fmt;

use serde::{
  Deserialize, Deserializer,
  de::{DeserializeSeed, Error, IgnoredAny, MapAccess, SeqAccess, Visitor},
};

use crate::layout::node::{
  ImageData, MAXIMUM_DOM_TREE_DEPTH, Node, NodeKind, NodeMetadata, TextData,
};

impl<'de> Deserialize<'de> for Node {
  fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
    NodeSeed { depth: 0 }.deserialize(deserializer)
  }
}

impl<'de> Deserialize<'de> for NodeKind {
  fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
    deserializer
      .deserialize_map(NodeVisitor {
        reads_metadata: false,
        depth: 0,
      })
      .map(|(_, kind)| kind)
  }
}

/// A node `depth` levels below the root, refused past [`MAXIMUM_DOM_TREE_DEPTH`].
struct NodeSeed {
  depth: usize,
}

impl<'de> DeserializeSeed<'de> for NodeSeed {
  type Value = Node;

  fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Node, D::Error> {
    if self.depth >= MAXIMUM_DOM_TREE_DEPTH {
      return Err(Error::custom(format_args!(
        "nodes nest deeper than {MAXIMUM_DOM_TREE_DEPTH} levels"
      )));
    }

    let (metadata, kind) = deserializer.deserialize_map(NodeVisitor {
      reads_metadata: true,
      depth: self.depth,
    })?;

    Ok(Node { metadata, kind })
  }
}

/// A container's children, one level below it: absent, `null`, or a list.
struct Children {
  depth: usize,
}

impl<'de> DeserializeSeed<'de> for Children {
  type Value = Option<Vec<Node>>;

  fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
    deserializer.deserialize_option(self)
  }
}

impl<'de> Visitor<'de> for Children {
  type Value = Option<Vec<Node>>;

  fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
    formatter.write_str("a sequence")
  }

  fn visit_none<E: Error>(self) -> Result<Self::Value, E> {
    Ok(None)
  }

  fn visit_unit<E: Error>(self) -> Result<Self::Value, E> {
    Ok(None)
  }

  fn visit_some<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
    deserializer.deserialize_seq(self)
  }

  fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
    let mut children = Vec::new();

    while let Some(child) = seq.next_element_seed(NodeSeed { depth: self.depth })? {
      children.push(child);
    }

    Ok(Some(children))
  }
}

#[derive(Clone, Copy, Deserialize)]
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

impl NodeField {
  /// Whether the key belongs to [`NodeMetadata`] rather than to a node type.
  fn is_metadata(self) -> bool {
    matches!(
      self,
      Self::TagName
        | Self::ClassName
        | Self::Id
        | Self::Attributes
        | Self::Preset
        | Self::Style
        | Self::Tw
        | Self::Dir
        | Self::Lang
    )
  }
}

#[derive(Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
enum NodeType {
  Container,
  Image,
  Text,
}

struct NodeVisitor {
  /// Whether metadata keys are read, or skipped unread as a bare [`NodeKind`] ignores them.
  reads_metadata: bool,
  /// How many levels below the root the node sits.
  depth: usize,
}

impl<'de> Visitor<'de> for NodeVisitor {
  type Value = (NodeMetadata, NodeKind);

  fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
    formatter.write_str("a node")
  }

  fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
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
        _ if field.is_metadata() && !self.reads_metadata => {
          map.next_value::<IgnoredAny>()?;
        }
        NodeField::Type => node_type = Some(map.next_value()?),
        NodeField::Children if owns(NodeType::Container) => {
          children = map.next_value_seed(Children {
            depth: self.depth + 1,
          })?;
        }
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

    Ok((metadata, kind))
  }
}

#[cfg(test)]
mod tests {
  use std::thread;

  use serde_json::{from_str, from_value, json};

  use crate::layout::node::{MAXIMUM_DOM_TREE_DEPTH, Node, NodeKind};

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
  fn reads_a_kind_on_its_own_past_metadata() {
    let input = json!({ "className": 1, "type": "text", "text": "a" });
    let kind: NodeKind = from_value(input.clone()).unwrap();

    assert!(matches!(&kind, NodeKind::Text(data) if data.text == "a"));
    assert!(error(input).contains("invalid type"));
  }

  #[test]
  fn refuses_nodes_nested_past_the_limit() {
    let nested = |levels: usize| {
      (1..levels).fold(
        json!({ "type": "text", "text": "a" }),
        |child, _| json!({ "type": "container", "children": [child] }),
      )
    };

    // A debug build's test thread is too shallow for `serde_json`'s own recursion that deep.
    thread::Builder::new()
      .stack_size(64 << 20)
      .spawn(move || {
        assert!(from_value::<Node>(nested(MAXIMUM_DOM_TREE_DEPTH)).is_ok());
        assert!(error(nested(MAXIMUM_DOM_TREE_DEPTH + 1)).contains("nest deeper than"));
      })
      .unwrap()
      .join()
      .unwrap();
  }

  #[test]
  fn names_a_missing_or_unknown_type() {
    assert!(error(json!({ "text": "a" })).contains("missing field `type`"));
    assert!(error(json!({ "type": "video" })).contains("unknown variant `video`"));
    assert!(error(json!({ "type": "text" })).contains("missing field `text`"));
    assert!(error(json!({ "type": "image" })).contains("missing field `src`"));
  }
}
