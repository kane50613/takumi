mod test_utils;

use std::fs::{create_dir_all, write};

use takumi::prelude::*;
use takumi_core::paint_tree::{PaintFill, PaintNode, PaintTree, PaintTreeOptions, paint_tree};
use test_utils::{CONTEXT, TEST_IMAGES, format_generated};

const CSS: &str = r#"
  .card {
    display: flex;
    flex-direction: column;
    width: 400px;
    padding: 20px;
    background: #F7F3EC linear-gradient(to right, red, blue);
    border: 4px solid #B3261E;
    border-radius: 12px;
    box-shadow: 0 4px 8px rgba(0, 0, 0, 0.25), inset 0 1px 0 white;
    outline: 2px dashed green;
    outline-offset: 3px;
    overflow: hidden;
    color: #14110F;
    font-family: Geist;
    font-size: 26px;
  }
  .big { font-size: 40px; font-weight: 700; color: rgb(179, 38, 30); text-decoration: underline; }
  .mark { display: inline; background-color: yellow; opacity: 0.5; }
  b { display: inline; font-weight: 700; color: rgb(0, 0, 255); }
  .pic { width: 120px; height: 80px; object-fit: cover; border-radius: 8px; }
  .glass { filter: blur(2px); clip-path: circle(40%); mix-blend-mode: multiply; }
"#;

fn card() -> Node {
  Node::container([
    Node::text("4,2 %").with_class_name("big"),
    Node::container([
      Node::text("hello "),
      Node::container([Node::text("world")]).with_tag_name("b"),
      Node::text(" and "),
      Node::container([Node::text("marked")]).with_class_name("mark"),
    ])
    .with_class_name("p")
    .with_id("paragraph"),
    Node::image("assets/images/yeecord.png").with_class_name("pic"),
    Node::container([]).with_class_name("glass"),
  ])
  .with_class_name("card")
  .with_id("card")
}

fn build(node: Node) -> PaintTree {
  paint_tree(
    PaintTreeOptions::builder()
      .viewport(Viewport::new((600, 400)))
      .node(node)
      .fonts(&CONTEXT)
      .images(TEST_IMAGES.clone())
      .stylesheet(StyleSheet::parse_loosy(CSS).into())
      .build(),
  )
  .unwrap()
}

fn find<'t>(node: &'t PaintNode, id: &str) -> Option<&'t PaintNode> {
  if node.source.as_ref().and_then(|s| s.id.as_deref()) == Some(id) {
    return Some(node);
  }
  node.children.iter().find_map(|child| find(child, id))
}

fn all_runs(node: &PaintNode) -> Vec<&takumi_core::paint_tree::PaintTextRun> {
  let mut runs: Vec<_> = node.runs.iter().collect();
  for child in &node.children {
    runs.extend(all_runs(child));
  }
  runs
}

#[test]
fn paint_tree_records_used_values() {
  let tree = build(card());
  let json = serde_json::to_string_pretty(&tree).unwrap();
  create_dir_all("tests/fixtures-generated").ok();
  let path = "tests/fixtures-generated/paint_tree_card.json";
  write(path, json).unwrap();
  format_generated(path);

  assert_eq!((tree.width, tree.height), (600.0, 400.0));

  let card = find(&tree.root, "card").expect("card box");
  assert_eq!((card.width, card.height), (400.0, card.height));
  let decoration = card.box_decoration.as_ref().expect("card decorations");
  assert_eq!(decoration.background.color, Some([247, 243, 236, 255]));
  assert_eq!(decoration.border.widths, [4.0; 4]);
  assert_eq!(decoration.border.colors[0], [179, 38, 30, 255]);
  assert_eq!(decoration.border.styles[0], "solid");
  assert_eq!(decoration.border.radii[0], [12.0, 12.0]);
  assert_eq!(decoration.shadows.outer.len(), 1);
  assert_eq!(decoration.shadows.inset.len(), 1);
  assert_eq!(decoration.shadows.outer[0].blur, 8.0);
  let outline = decoration.outline.as_ref().expect("outline");
  assert_eq!(
    (outline.width, outline.offset, outline.style.as_str()),
    (2.0, 3.0, "dashed")
  );
  assert_eq!(decoration.background.layers.len(), 1);
  assert!(matches!(
    &decoration.background.layers[0].fill,
    PaintFill::Linear { stops, .. } if stops.len() == 2 && stops[0].color == [255, 0, 0, 255]
  ));
  let clip = card.clip.as_ref().expect("overflow clip");
  assert_eq!((clip.rect.x, clip.rect.y), (4.0, 4.0));
  assert!(clip.x && clip.y);

  let runs = all_runs(&tree.root);
  let texts: Vec<&str> = runs.iter().map(|run| run.text.as_str()).collect();
  assert_eq!(texts, ["4,2 %", "hello ", "world", " and ", "marked"]);
  let big = runs[0];
  assert_eq!(big.color, [179, 38, 30, 255]);
  assert_eq!(big.font_size, 40.0);
  assert!(!big.decorations.is_empty());
  assert!(big.decorations.iter().all(|d| d.line == "underline"));
  let world = runs[2];
  assert_eq!(world.color, [0, 0, 255, 255]);
  assert_eq!(tree.fonts[world.font_index].weight, 700.0);
  assert_eq!(
    tree.fonts[world.font_index].family.as_deref(),
    Some("Geist")
  );
  assert_eq!(tree.fonts[runs[1].font_index].weight, 400.0);

  let paragraph = find(&tree.root, "paragraph").expect("paragraph box");
  assert_eq!(paragraph.inline_backgrounds.len(), 1);
  assert_eq!(paragraph.inline_backgrounds[0].color, [255, 255, 0, 255]);
  assert_eq!(paragraph.inline_backgrounds[0].opacity, 0.5);

  let image = tree
    .root
    .children
    .iter()
    .chain(card.children.iter())
    .find_map(|node| node.image.as_ref())
    .expect("image content");
  assert_eq!(
    (image.content_box.width, image.content_box.height),
    (120.0, 80.0)
  );
  assert_eq!(image.src.as_deref(), Some("assets/images/yeecord.png"));
  assert!(image.placement.width >= 120.0 && image.placement.height >= 80.0);

  let glass = card
    .children
    .iter()
    .find(|node| node.unresolved_effects.is_some())
    .expect("glass box");
  let unresolved = glass.unresolved_effects.as_ref().unwrap();
  assert_eq!(unresolved.filter.as_deref(), Some("blur(2px)"));
  assert!(unresolved.clip_path.is_some());
  assert_eq!(glass.blend_mode.as_deref(), Some("multiply"));
}
