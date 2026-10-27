use takumi::prelude::*;
use takumi::unstable::base::layout::node::EmojiProvider;

use crate::test_utils::{CONTEXT, TEST_IMAGES, create_test_viewport, write_goldens};

/// Each emoji becomes an inline image a little wider than the text's em box, with the provider's
/// URL as its source. A test image stands in for the CDN file.
#[test]
fn emoji_images() {
  let mut node = Node::from_html(
    r#"<p style="font-size: 48px; margin: 0">Hi 😀 and 👍🏽!</p>"#,
    FromHtmlOptions::default(),
  )
  .unwrap();

  node.extract_emojis(EmojiProvider::Twemoji);

  let mut images = TEST_IMAGES.clone();
  let stand_in = images["assets/images/yeecord.png"].clone();

  for emoji in ["😀", "👍🏽"] {
    images.insert(EmojiProvider::Twemoji.url(emoji).into(), stand_in.clone());
  }

  write_goldens(
    RenderOptions::builder()
      .viewport(create_test_viewport())
      .node(node)
      .fonts(&CONTEXT)
      .images(images)
      .build(),
    "emoji_images",
  )
  .unwrap();
}
