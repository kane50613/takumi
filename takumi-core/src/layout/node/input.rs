use std::{collections::BTreeMap, fmt, sync::Arc};

use serde::{
  Deserialize, Deserializer,
  de::{DeserializeSeed, Error, IgnoredAny, MapAccess, SeqAccess, Visitor},
};

use crate::{
  layout::node::{
    ImageData, ImageSourceInput, MAXIMUM_DOM_TREE_DEPTH, Names, Node, NodeKind, NodeMetadata,
    TextData,
  },
  style::{Lang, ParsedValues, Style},
};

impl<'de> Deserialize<'de> for Node {
  fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
    let names = Names::default();

    ParsedValues::scope(|| {
      NodeSeed {
        depth: 0,
        names: &names,
      }
      .deserialize(deserializer)
    })
  }
}

impl<'de> Deserialize<'de> for NodeKind {
  fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
    deserializer
      .deserialize_map(NodeVisitor {
        reads_metadata: false,
        depth: 0,
        names: &Names::default(),
      })
      .map(|(_, kind)| kind)
  }
}

/// A node `depth` levels below the root, refused past [`MAXIMUM_DOM_TREE_DEPTH`].
struct NodeSeed<'n> {
  depth: usize,
  names: &'n Names,
}

impl<'de> DeserializeSeed<'de> for NodeSeed<'_> {
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
      names: self.names,
    })?;

    Ok(Node { metadata, kind })
  }
}

/// A container's children, one level below it: absent, `null`, or a list.
struct Children<'n> {
  depth: usize,
  names: &'n Names,
}

impl<'de> DeserializeSeed<'de> for Children<'_> {
  type Value = Option<Vec<Node>>;

  fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
    deserializer.deserialize_option(self)
  }
}

impl<'de> Visitor<'de> for Children<'_> {
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

    while let Some(child) = seq.next_element_seed(NodeSeed {
      depth: self.depth,
      names: self.names,
    })? {
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

struct NodeVisitor<'n> {
  /// Whether metadata keys are read, or skipped unread as a bare [`NodeKind`] ignores them.
  reads_metadata: bool,
  /// How many levels below the root the node sits.
  depth: usize,
  /// The tree's element and attribute names read so far.
  names: &'n Names,
}

impl<'de> Visitor<'de> for NodeVisitor<'_> {
  type Value = (NodeMetadata, NodeKind);

  fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
    formatter.write_str("a node")
  }

  fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
    let mut fields = NodeFields::default();

    while let Some(field) = map.next_key::<NodeField>()? {
      // Once `type` is read, a key another node type owns is skipped unread.
      let owns = |kind: NodeType| fields.node_type.is_none_or(|known| known == kind);

      match field {
        _ if field.is_metadata() && !self.reads_metadata => {
          map.next_value::<IgnoredAny>()?;
        }
        NodeField::Type => fields.node_type = Some(map.next_value()?),
        NodeField::Children if owns(NodeType::Container) => {
          fields.children = map.next_value_seed(Children {
            depth: self.depth + 1,
            names: self.names,
          })?;
        }
        NodeField::Text if owns(NodeType::Text) => fields.text = Some(map.next_value()?),
        NodeField::Src if owns(NodeType::Image) => fields.src = Some(map.next_value()?),
        NodeField::Width if owns(NodeType::Image) => fields.width = map.next_value()?,
        NodeField::Height if owns(NodeType::Image) => fields.height = map.next_value()?,
        NodeField::TagName => {
          fields.metadata.tag_name = map
            .next_value::<Option<Box<str>>>()?
            .map(|name| self.names.get(&name));
        }
        NodeField::ClassName => fields.metadata.class_name = map.next_value()?,
        NodeField::Id => fields.metadata.id = map.next_value()?,
        NodeField::Attributes => {
          fields.metadata.attributes = map
            .next_value::<Option<BTreeMap<Box<str>, Box<str>>>>()?
            .map(|attributes| {
              attributes
                .into_iter()
                .map(|(name, value)| (self.names.get(&name), value))
                .collect()
            });
        }
        NodeField::Preset => {
          fields.metadata.preset = map.next_value::<Option<Style>>()?.map(Arc::new);
        }
        NodeField::Style => fields.metadata.style = map.next_value()?,
        NodeField::Tw => fields.metadata.tw = map.next_value()?,
        NodeField::Dir => fields.metadata.dir = map.next_value()?,
        NodeField::Lang => fields.lang = map.next_value::<Option<Box<str>>>()?,
        _ => {
          map.next_value::<IgnoredAny>()?;
        }
      }
    }

    fields.into_node(self.names).map_err(Error::missing_field)
  }
}

/// What the keys of a node object set.
#[derive(Default)]
struct NodeFields {
  node_type: Option<NodeType>,
  metadata: NodeMetadata,
  children: Option<Vec<Node>>,
  text: Option<String>,
  src: Option<ImageSourceInput>,
  width: Option<f32>,
  height: Option<f32>,
  lang: Option<Box<str>>,
}

impl NodeFields {
  /// The node they make, or the key it lacks.
  fn into_node(mut self, names: &Names) -> Result<(NodeMetadata, NodeKind), &'static str> {
    if let Some(text) = self.lang {
      let (lang, keeps_text) = Lang::from_attribute(&text);
      let has_lang_attribute = self
        .metadata
        .attributes
        .iter()
        .flatten()
        .any(|(name, _)| name.as_ref() == "lang");

      self.metadata.lang = lang;
      if keeps_text && !has_lang_attribute {
        self.metadata.attributes = Some(
          self
            .metadata
            .attributes
            .take()
            .into_iter()
            .flatten()
            .chain([(names.get("lang"), text)])
            .collect(),
        );
      }
    }

    let kind = match self.node_type.ok_or("type")? {
      NodeType::Container => NodeKind::Container {
        children: self.children.unwrap_or_default(),
      },
      NodeType::Image => NodeKind::Image(ImageData {
        src: self.src.ok_or("src")?,
        width: self.width,
        height: self.height,
      }),
      NodeType::Text => NodeKind::Text(TextData {
        text: self.text.ok_or("text")?,
      }),
    };

    Ok((self.metadata, kind))
  }
}

#[cfg(test)]
mod tests {
  use std::thread;

  use serde_json::{from_str, from_value, json};

  use crate::{
    layout::node::{MAXIMUM_DOM_TREE_DEPTH, Node, NodeKind},
    style::{Lang, Style},
  };

  fn error(value: serde_json::Value) -> String {
    from_value::<Node>(value).unwrap_err().to_string()
  }

  #[test]
  fn a_repeated_style_value_reads_as_it_does_alone() {
    let styles = [
      json!({ "fontSize": 13, "margin": "1px 2px", "color": "red" }),
      json!({ "fontSize": 13.0, "padding": "1px 2px", "color": "red" }),
      json!({ "fontSize": 13, "margin": "1px 2px", "padding": "1px 2px" }),
    ];
    let node: Node = from_value(json!({
      "type": "container",
      "children": styles
        .iter()
        .map(|style| json!({ "type": "text", "text": "a", "style": style }))
        .collect::<Vec<_>>(),
    }))
    .unwrap();
    let NodeKind::Container { children } = &node.kind else {
      panic!("expected a container");
    };

    for (child, style) in children.iter().zip(styles) {
      assert_eq!(
        child.metadata.style,
        Some(Box::new(from_value::<Style>(style).unwrap()))
      );
    }
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

  #[test]
  fn keeps_the_text_of_a_lang_it_does_not_spell() {
    for (text, lang) in [("EN_us", Some("en-US")), ("", None), ("i-klingon", None)] {
      let node: Node = from_value(json!({ "type": "text", "text": "a", "lang": text })).unwrap();

      assert_eq!(node.metadata.lang.as_ref().map(Lang::as_str), lang);
      assert_eq!(node.attribute("lang"), Some(text));
    }

    let node: Node = from_value(json!({ "type": "text", "text": "a", "lang": "en-US" })).unwrap();

    assert_eq!(node.metadata.attributes, None);
  }
}
