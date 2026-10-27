//! What a render tree needs from outside before it renders, read after the cascade so stylesheet
//! rules, `display: none`, generated content, and `text-transform` count as they will render.

use std::collections::BTreeSet;

use crate::{
  layout::{
    node::{NodeKind, is_remote_url},
    tree::RenderNode,
  },
  style::{BackgroundImage, ComputedStyle, Display, TextTransform},
};

/// The remote images a render tree draws and the characters its text shapes.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RenderNeeds {
  /// The `http:` and `https:` URLs of image elements, of computed `background-image` and
  /// `mask-image` values, and of list items' `list-style-image`.
  pub images: BTreeSet<String>,
  /// Every character the tree's text, generated content and list markers included, shapes.
  ///
  /// Approximate: `capitalize` contributes both the text and the text with its first letters
  /// capitalized as if each run started a word, since the character before a run is only known
  /// during inline layout.
  pub characters: BTreeSet<char>,
}

impl RenderNode {
  /// What this render tree needs before it renders.
  pub fn needs(&self) -> RenderNeeds {
    let mut needs = RenderNeeds::default();

    // The tree builder drops hidden children but keeps a hidden root.
    if self.context.style.base_data.display == Display::None {
      return needs;
    }

    let mut stack = vec![self];

    while let Some(node) = stack.pop() {
      needs.add_style(&node.context.style);

      if let Some(NodeKind::Image(image)) = node.node.as_ref().map(|node| &node.kind)
        && let Some(url) = image.url()
      {
        needs.images.insert(url.to_owned());
      }

      if let Some(text) = node.own_text() {
        needs.add_text(&node.context.style, text);
      }

      stack.extend(node.children.iter().flat_map(|children| children.iter()));
    }

    needs
  }
}

impl RenderNeeds {
  fn add_style(&mut self, style: &ComputedStyle) {
    let images = [
      style.background_data.background_image.as_ref(),
      style.misc2_data.mask_image.as_ref(),
    ]
    .into_iter()
    .flatten()
    .flat_map(|images| images.iter())
    .chain(
      (style.base_data.display == Display::ListItem)
        .then(|| style.misc_inherited1_data.list_style_image.image())
        .flatten(),
    );

    for image in images {
      if let BackgroundImage::Url(url) = image
        && is_remote_url(url)
      {
        self.images.insert(url.to_string());
      }
    }
  }

  fn add_text(&mut self, style: &ComputedStyle, text: &str) {
    self.characters.extend(text.chars());

    if style.inherited_data.text_transform != TextTransform::default() {
      self
        .characters
        .extend(style.apply_text_transform(text, ' ').chars());
    }
  }
}

#[cfg(test)]
mod tests {
  use super::RenderNeeds;
  use crate::layout::{inline::testing::render_tree, node::Node};

  const CSS: &str = "
    .hidden { display: none; background-image: url(https://example.com/hidden.png) }
    .bg { background-image: url(https://example.com/bg.png), url(data:image/png;base64,AA) }
    .bg::before { content: 'x' url(https://example.com/before.png) }
    .mask { mask-image: url('https://example.com/mask.png') }
    .upper { text-transform: uppercase }
    .item { display: list-item; list-style: decimal inside }
  ";

  #[test]
  fn reads_nothing_under_a_hidden_root() {
    let needs = render_tree([Node::text("a")], ".container { display: none }").needs();

    assert_eq!(needs, RenderNeeds::default());
  }

  #[test]
  fn reads_a_marker_image_only_for_a_list_item() {
    let needs = render_tree(
      [Node::container([Node::text("a")]).with_class_name("plain")],
      ".container { list-style-image: url(https://example.com/marker.png) }",
    )
    .needs();

    assert!(needs.images.is_empty());
  }

  #[test]
  fn reads_full_width_transforms() {
    let needs = render_tree(
      [Node::text("A").with_class_name("wide")],
      ".wide { text-transform: full-width }",
    )
    .needs();

    assert!(needs.characters.contains(&'Ａ'));
  }

  #[test]
  fn reads_what_the_cascade_leaves() {
    let needs = render_tree(
      [
        Node::text("ab").with_class_name("upper"),
        Node::container([Node::text("z")]).with_class_name("hidden"),
        Node::container([]).with_class_name("bg"),
        Node::container([]).with_class_name("mask"),
        Node::container([Node::text("c")]).with_class_name("item"),
        Node::image("https://example.com/img.png"),
        Node::image("data:image/png;base64,AA"),
        Node::image("HTTPS://example.com/upper.png"),
      ],
      CSS,
    )
    .needs();

    assert_eq!(
      needs.images.into_iter().collect::<Vec<_>>(),
      [
        "HTTPS://example.com/upper.png",
        "https://example.com/before.png",
        "https://example.com/bg.png",
        "https://example.com/img.png",
        "https://example.com/mask.png",
      ]
    );
    assert_eq!(
      needs.characters.into_iter().collect::<String>(),
      " .1ABabcx"
    );
  }
}
