//! What a render tree needs from outside before it renders, read after the cascade so stylesheet
//! rules, `display: none`, generated content, and `text-transform` count as they will render.

use std::collections::BTreeSet;

use crate::{
  context::RenderContext,
  layout::{
    inline::{inline_item::InlineItemType, inline_node::InlineNode},
    node::{NodeKind, is_remote_url},
    tree::RenderNode,
  },
  style::{BackgroundImage, ComputedStyle, Display, Float, find_keyframes},
};

/// The remote images a render tree draws and the characters its text shapes.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RenderNeeds {
  /// The `http:` and `https:` URLs of image elements, of computed `background-image` and
  /// `mask-image` values, and of list items' `list-style-image`.
  pub images: BTreeSet<String>,
  /// Every character the tree's text, generated content and list markers included, shapes, after
  /// `text-transform` as inline layout applies it.
  pub characters: BTreeSet<char>,
}

impl RenderNode {
  /// Whether an inline walk passes this box without reading its content, which lays out on its own.
  fn lays_out_apart(&self) -> bool {
    self.is_out_of_flow()
      || self.context.style.base_data.float != Float::None
      || self.is_atomic_inline()
      || self.is_block_in_inline
  }

  /// What this render tree needs before it renders.
  pub fn needs(&self) -> RenderNeeds {
    let mut needs = RenderNeeds::default();

    // The tree builder drops hidden children but keeps a hidden root.
    if self.context.style.base_data.display == Display::None {
      return needs;
    }

    let mut stack = vec![(self, false)];

    // Whether an ancestor's inline walk reads the node's text, which then reads its children's too,
    // except a box it passes because that box lays out on its own.
    while let Some((node, walked)) = stack.pop() {
      needs.add_style(&node.context.style);
      needs.add_keyframes(&node.context);

      if let Some(NodeKind::Image(image)) = node.node.as_ref().map(|node| &node.kind)
        && let Some(url) = image.url()
      {
        needs.images.insert(url.to_owned());
      }

      let creates_inline_context = node.should_create_inline_layout();

      // A text laid out on its own, such as a flex item or a float, collects its own content.
      if !walked && (creates_inline_context || node.own_text().is_some()) {
        needs.add_inline_text(node);
      }

      let walks_children = walked || creates_inline_context;

      stack.extend(
        node
          .children
          .iter()
          .flat_map(|children| children.iter())
          .map(|child| (child, walks_children && !child.lays_out_apart())),
      );
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

  /// Adds the images the `@keyframes` rules the box animates with switch to, which only show at
  /// later frames.
  fn add_keyframes(&mut self, context: &RenderContext) {
    for name in context.style.misc1_data.animation_name.iter().flatten() {
      let Some(rule) = find_keyframes(context.stylesheet(), name, context.sizing.viewport) else {
        continue;
      };

      for keyframe in &rule.keyframes {
        self.images.extend(
          keyframe
            .declarations
            .image_urls()
            .filter(|url| is_remote_url(url))
            .map(str::to_owned),
        );
      }
    }
  }

  /// Adds the text inline layout shapes in the inline formatting context `root` establishes,
  /// collected as layout collects it, `text-transform` applied with the character before each run.
  fn add_inline_text(&mut self, root: &RenderNode) {
    let data = InlineNode::new(root).collect_inlines();

    for item in &data.items {
      if item.item_type == InlineItemType::Text {
        self
          .characters
          .extend(data.text_content[item.start_offset()..item.end_offset()].chars());
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use super::RenderNeeds;
  use crate::layout::{
    inline::testing::{render_tree, span},
    node::Node,
  };

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
  fn reads_images_an_animation_switches_to() {
    let needs = render_tree(
      [Node::container([]).with_class_name("spin")],
      "@keyframes swap { to { background-image: url(https://example.com/later.png) } }
       @keyframes unused { to { background-image: url(https://example.com/unused.png) } }
       .spin { animation: swap 1s }",
    )
    .needs();

    assert_eq!(
      needs.images.into_iter().collect::<Vec<_>>(),
      ["https://example.com/later.png"]
    );
  }

  #[test]
  fn capitalizes_only_where_a_word_starts() {
    let needs = render_tree(
      [span("ab"), span("cd"), span(" ef")],
      ".span { text-transform: capitalize }",
    )
    .needs();

    assert_eq!(needs.characters.into_iter().collect::<String>(), " AEbcdf");
  }

  #[test]
  fn reads_a_nested_inline_with_the_context_around_it() {
    let needs = render_tree(
      [
        span("ab"),
        Node::container([Node::text("cd")]).with_class_name("span"),
      ],
      ".span { text-transform: capitalize }",
    )
    .needs();

    assert_eq!(needs.characters.into_iter().collect::<String>(), "Abcd");
  }

  #[test]
  fn reads_a_floated_text_in_an_inline_context() {
    let needs = render_tree(
      [span("a"), Node::text("q").with_class_name("float")],
      ".float { float: left }",
    )
    .needs();

    assert_eq!(needs.characters.into_iter().collect::<String>(), "aq");
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
    assert_eq!(needs.characters.into_iter().collect::<String>(), " .1ABcx");
  }
}
