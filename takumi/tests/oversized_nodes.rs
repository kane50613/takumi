mod test_utils;

use takumi::{prelude::*, render};
use test_utils::CONTEXT;

/// Renders into a 200x200 viewport, so the declared style is the only thing
/// pushing buffer sizes past `u32`.
fn opaque_pixels(image: &Bitmap) -> usize {
  image
    .as_raw()
    .as_chunks::<4>()
    .0
    .iter()
    .filter(|p| p[3] != 0)
    .count()
}

fn render_with_css(css: &str) -> Result<Bitmap> {
  render(
    RenderOptions::builder()
      .viewport(Viewport::new((200, 200)))
      .node(Node::container([]).with_class_name("box"))
      .stylesheet(StyleSheet::parse_loosy(css).into())
      .fonts(&CONTEXT)
      .build(),
  )
}

#[test]
fn an_inset_shadow_on_an_oversized_node_renders() {
  assert!(
    render_with_css(".box { width: 100000px; height: 100000px; box-shadow: inset 0 0 4px black; }")
      .is_ok()
  );
}

/// An inset far outside the box clips nothing, so the paint has to match the
/// same node without a `clip-path`.
#[test]
fn an_oversized_clip_path_clips_nothing() {
  let clipped = render_with_css(
    ".box { width: 50px; height: 50px; background: red; clip-path: inset(-200000px); }",
  )
  .unwrap();
  let plain = render_with_css(".box { width: 50px; height: 50px; background: red; }").unwrap();

  assert_eq!(opaque_pixels(&clipped), opaque_pixels(&plain));
  assert_eq!(clipped.as_raw(), plain.as_raw());
}

/// A mask too large to rasterize has to hide the node, not paint it unmasked.
#[test]
fn an_oversized_mask_hides_the_node() {
  let image = render_with_css(
    ".box { width: 16385px; height: 16384px; background: red; mask-image: linear-gradient(transparent, transparent); }",
  )
  .unwrap();

  assert_eq!(opaque_pixels(&image), 0);
}
