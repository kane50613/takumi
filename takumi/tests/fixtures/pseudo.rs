use takumi::prelude::*;

use crate::test_utils::{
  CONTEXT, TEST_IMAGES, attrs, create_test_viewport, run_fixture_test_with_options,
};

fn cell(class: &str, label: &str, seed: &str) -> Node {
  Node::container([
    Node::text(label.to_string()).with_class_name("label"),
    Node::container([Node::text(seed.to_string())]).with_class_name(format!("demo {class}")),
  ])
  .with_class_name("cell")
}

const SHARED_CSS: &str = r#"
  .root {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
    padding: 16px;
    width: 100%;
    height: 100%;
    background: rgb(243, 244, 246);
    align-content: flex-start;
    font-family: "Geist";
  }
  .cell {
    display: flex;
    flex-direction: column;
    gap: 6px;
    width: 282px;
    height: 138px;
    background: white;
    border-radius: 8px;
    padding: 10px 12px;
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.05);
  }
  .label {
    font-size: 11px;
    color: rgb(100, 116, 139);
    font-family: "Geist Mono";
  }
  .demo {
    flex-grow: 1;
    font-size: 18px;
    color: rgb(17, 24, 39);
    display: flex;
    align-items: center;
  }
"#;

#[test]
fn test_pseudo_text_attr() {
  let cells = vec![
    cell("d1", r#"::before "★ ""#, "Mars"),
    cell("d2", r#"::after " ✓""#, "Done"),
    cell("d3", "::before + ::after", "Title"),
    cell("d4", r#"content: "[" "(" ")" "]""#, "Body"),
    Node::container([
      Node::text(r#"attr(data-tag)"#.to_string()).with_class_name("label"),
      Node::container([Node::text("X".to_string())])
        .with_class_name("demo d5")
        .with_attributes(attrs(&[("data-tag", "alpha")])),
    ])
    .with_class_name("cell"),
    cell("d6", r#"attr(missing, "FB")"#, "X"),
    Node::container([
      Node::text("attr(id) + attr(class)".to_string()).with_class_name("label"),
      Node::container([Node::text("X".to_string())])
        .with_class_name("demo d7")
        .with_id("hero"),
    ])
    .with_class_name("cell"),
    cell("d8", "inherits color → orange", "X"),
  ];

  let pseudo_css = r##"
    .d1::before { content: "★ "; color: rgb(234, 179, 8); }
    .d2::after  { content: " ✓"; color: rgb(34, 197, 94); }
    .d3::before { content: "["; color: rgb(59, 130, 246); }
    .d3::after  { content: "]"; color: rgb(59, 130, 246); }
    .d4::before { content: "[" "("; color: rgb(99, 102, 241); }
    .d4::after  { content: ")" "]"; color: rgb(99, 102, 241); }
    .d5::before { content: attr(data-tag) " — "; color: rgb(244, 63, 94); }
    .d6::before { content: attr(missing, "FB") " "; color: rgb(168, 85, 247); }
    .d7::before { content: "#" attr(id) " "; color: rgb(20, 184, 166); }
    .d8         { color: rgb(234, 88, 12); }
    .d8::before { content: "★ "; }
  "##;

  let root = Node::container(cells).with_class_name("root");

  let options = RenderOptions::builder()
    .viewport(create_test_viewport())
    .node(root)
    .fonts(&CONTEXT)
    .stylesheet(
      StyleSheet::parse(&format!("{SHARED_CSS}{pseudo_css}"))
        .unwrap()
        .into(),
    )
    .build();

  run_fixture_test_with_options(options, "pseudo_text_attr");
}

#[test]
fn test_pseudo_display_image() {
  let mut cells = vec![
    cell("g1", "default (inline)", "main"),
    cell("g2", "display: block", "main"),
    cell("g3", "display: inline-block", "main"),
    cell("g4", "display: flex → block", "main"),
    cell("g5", "bg + border + padding", "title"),
    cell("g6", "linear-gradient", ""),
    cell("g7", "radial-gradient", ""),
    cell("g8", "conic-gradient", ""),
    cell("g9", "repeating-linear", ""),
    cell("g10", "url() image", ""),
    cell("g11", "two gradients in list", ""),
  ];

  // Replaced-element exclusion: an image node with a pseudo rule should not
  // produce any pseudo content; the image renders normally.
  cells.push(
    Node::container([
      Node::text("img::before is skipped".to_string()).with_class_name("label"),
      Node::image("assets/images/yeecord.png".to_string()).with_class_name("demo g12"),
    ])
    .with_class_name("cell"),
  );

  let pseudo_css = r#"
    .g1::before { content: "[in]"; background: rgb(254, 226, 226); color: rgb(190, 18, 60); padding: 0 6px; margin-right: 6px; border-radius: 4px; }
    .g2::before { content: "BLOCK"; display: block; background: rgb(220, 252, 231); color: rgb(21, 128, 61); padding: 2px 8px; margin-bottom: 4px; border-radius: 4px; }
    .g3::before { content: "IB"; display: inline-block; background: rgb(219, 234, 254); color: rgb(29, 78, 216); padding: 2px 8px; margin-right: 6px; border-radius: 4px; }
    .g4::before { content: "FLEX"; display: flex; background: rgb(254, 240, 138); color: rgb(133, 77, 14); padding: 2px 8px; margin-bottom: 4px; border-radius: 4px; }
    .g5::before { content: "★"; display: inline-block; background: rgb(254, 226, 226); border: 2px solid rgb(220, 38, 38); border-radius: 50%; padding: 0 8px; color: rgb(127, 29, 29); margin-right: 8px; }
    .g5::after  { content: " ◆"; color: rgb(124, 58, 237); }
    .g6::before { content: linear-gradient(135deg, #ff3b30, #ffcc00); display: block; width: 240px; height: 60px; border-radius: 6px; }
    .g7::before { content: radial-gradient(circle, #00e5ff, #5856d6); display: block; width: 240px; height: 60px; border-radius: 6px; }
    .g8::before { content: conic-gradient(from 0deg, red, yellow, green, blue, red); display: block; width: 60px; height: 60px; border-radius: 50%; }
    .g9::before { content: repeating-linear-gradient(45deg, #f59e0b 0 10px, #1e293b 10px 20px); display: block; width: 240px; height: 60px; border-radius: 6px; }
    .g10::before { content: url("assets/images/yeecord.png"); display: block; width: 56px; height: 56px; }
    .g11::before { content: linear-gradient(red, blue) linear-gradient(green, yellow); display: block; width: 240px; height: 60px; }
    .g12 { width: 96px; height: 96px; object-fit: contain; }
    .g12::before { content: "X"; color: red; }
    .g12::after  { content: "Y"; color: red; }
  "#;

  let root = Node::container(cells).with_class_name("root");

  let options = RenderOptions::builder()
    .viewport(create_test_viewport())
    .node(root)
    .fonts(&CONTEXT)
    .images(TEST_IMAGES.clone())
    .stylesheet(
      StyleSheet::parse(&format!("{SHARED_CSS}{pseudo_css}"))
        .unwrap()
        .into(),
    )
    .build();

  run_fixture_test_with_options(options, "pseudo_display_image");
}
