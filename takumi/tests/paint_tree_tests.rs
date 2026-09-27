mod test_utils;

use std::fs::{create_dir_all, write};

use takumi::prelude::*;
use takumi_core::paint_tree::{
  Drawable, NodeKind, Paint, PaintDocument, PaintFilter, PaintNode, PaintStep, PaintTreeOptions,
  Role, Shape, TextRun, paint_tree,
};
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
  .collapsed { font-size: 0; }
  .sized { display: inline; font-size: 18px; }
  .fade { opacity: 0.5; }
  .padded-pic { width: 120px; height: 80px; padding: 10px; border: 2px solid black; }
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

fn build(node: Node) -> PaintDocument {
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
  .document
}

fn find<'d>(document: &'d PaintDocument, id: &str) -> &'d PaintNode {
  document
    .nodes
    .iter()
    .find(|node| {
      matches!(node.kind, NodeKind::Box { .. })
        && node
          .element
          .as_ref()
          .and_then(|element| element.id.as_deref())
          == Some(id)
    })
    .unwrap_or_else(|| panic!("no box with id {id}"))
}

/// The runs under `node`, in document order.
fn runs<'d>(document: &'d PaintDocument, node: &'d PaintNode) -> Vec<&'d TextRun> {
  let own = match &node.kind {
    NodeKind::Text { runs, .. } => runs.iter().collect(),
    _ => Vec::new(),
  };

  own
    .into_iter()
    .chain(
      node
        .children
        .iter()
        .flat_map(|&child| runs(document, &document.nodes[child])),
    )
    .collect()
}

fn roles(node: &PaintNode) -> Vec<Role> {
  node
    .drawables
    .iter()
    .map(|drawable| match drawable {
      Drawable::Fill { role, .. }
      | Drawable::Stroke { role, .. }
      | Drawable::Shadow { role, .. }
      | Drawable::Glyphs { role, .. }
      | Drawable::Image { role, .. } => *role,
    })
    .collect()
}

#[test]
fn paint_tree_records_used_values() {
  let document = build(card());
  let json = serde_json::to_string_pretty(&document).unwrap();
  create_dir_all("tests/fixtures-generated").ok();
  let path = "tests/fixtures-generated/paint_tree_card.json";
  write(path, json).unwrap();
  format_generated(path);

  assert_eq!((document.width, document.height), (600.0, 400.0));

  let card = find(&document, "card");
  assert_eq!(card.width, 400.0);
  assert_eq!(
    roles(card),
    [
      Role::BoxShadow,
      Role::Background,
      Role::Background,
      Role::BoxShadow,
      Role::Border
    ]
  );
  assert!(matches!(
    &card.drawables[0],
    Drawable::Shadow { blur, offset, .. } if *blur == 4.0 && offset.y == 4.0
  ));
  assert!(matches!(
    &card.drawables[1],
    Drawable::Fill {
      paint: Paint::Color {
        color: [247, 243, 236, 255]
      },
      shape: Shape::RoundedRect { .. },
      ..
    }
  ));
  assert!(matches!(
    &card.drawables[2],
    Drawable::Fill { paint: Paint::Pattern { tile, .. }, .. }
      if matches!(tile.as_ref(), Paint::LinearGradient { stops, .. } if stops[0].color == [255, 0, 0, 255])
  ));

  let NodeKind::Box {
    outline,
    overflow_clip,
    ..
  } = &card.kind
  else {
    unreachable!()
  };
  assert!(outline.iter().all(|drawable| matches!(
    drawable,
    Drawable::Stroke { role: Role::Outline, stroke, .. } if stroke.dash.is_some()
  )));
  assert!(!outline.is_empty());
  assert!(matches!(
    overflow_clip,
    Some(Shape::RoundedRect { rect, .. }) if (rect.x, rect.y) == (4.0, 4.0)
  ));

  let runs = runs(&document, &document.nodes[0]);
  let texts: Vec<&str> = runs.iter().map(|run| run.text.as_str()).collect();
  assert_eq!(texts, ["4,2 %", "hello ", "world", " and ", "marked"]);
  assert_eq!(runs[0].font_size, 40.0);
  let world = runs[2];
  assert_eq!(document.fonts[world.font].weight, 700.0);
  assert_eq!(document.fonts[world.font].family.as_deref(), Some("Geist"));
  assert_eq!(
    world
      .element
      .as_ref()
      .and_then(|element| element.tag_name.as_deref()),
    Some("b")
  );
  assert_eq!(document.fonts[runs[1].font].weight, 400.0);
  assert!(!world.outline.is_empty());

  let paragraph = find(&document, "paragraph");
  let text = paragraph
    .children
    .iter()
    .map(|&child| &document.nodes[child])
    .find(|node| matches!(node.kind, NodeKind::Text { .. }))
    .expect("paragraph text");
  assert!(text.drawables.iter().any(|drawable| matches!(
    drawable,
    Drawable::Fill {
      role: Role::InlineBackground,
      paint: Paint::Color {
        color: [255, 255, 0, 128]
      },
      ..
    }
  )));
  assert!(text.drawables.iter().any(|drawable| matches!(
    drawable,
    Drawable::Glyphs {
      role: Role::Text,
      paint: Paint::Color {
        color: [0, 0, 255, 255]
      },
      ..
    }
  )));

  let image = document
    .nodes
    .iter()
    .find_map(|node| match &node.kind {
      NodeKind::Image { image } => Some((node, image)),
      _ => None,
    })
    .expect("image node");
  assert_eq!((image.0.width, image.0.height), (120.0, 80.0));
  assert_eq!(image.1.src, "assets/images/yeecord.png");
  assert!(matches!(
    &image.0.drawables[0],
    Drawable::Image { rect, .. } if rect.width >= 120.0 && rect.height >= 80.0
  ));

  let glass = document
    .nodes
    .iter()
    .find_map(|node| match &node.kind {
      NodeKind::Box {
        effects: Some(effects),
        ..
      } => Some(effects),
      _ => None,
    })
    .expect("glass effects");
  assert_eq!(glass.filters, [PaintFilter::Blur { radius: 2.0 }]);
  assert!(matches!(glass.clip, Some(Shape::Path { .. })));
  assert_eq!(glass.blend_mode, "multiply");

  let depth = document.steps.iter().try_fold(0_i32, |depth, step| {
    let depth = match step {
      PaintStep::BeginGroup { .. } | PaintStep::BeginClip { .. } => depth + 1,
      PaintStep::EndGroup { .. } | PaintStep::EndClip { .. } => depth - 1,
      PaintStep::Draw { .. } => depth,
    };
    (depth >= 0).then_some(depth)
  });
  assert_eq!(depth, Some(0));
}

#[test]
fn padded_image_clips_to_its_whole_content_box() {
  let document = build(Node::container([
    Node::image("assets/images/yeecord.png").with_class_name("padded-pic")
  ]));
  let image = document
    .nodes
    .iter()
    .find(|node| matches!(node.kind, NodeKind::Image { .. }))
    .expect("image node");
  let Some(Drawable::Image {
    clip: Shape::Rect { rect },
    ..
  }) = image.drawables.first()
  else {
    panic!("the image clips to a rect: {:?}", image.drawables.first());
  };

  assert_eq!(
    (rect.x, rect.y, rect.width, rect.height),
    (0.0, 0.0, image.width, image.height)
  );
}

#[test]
fn stacking_context_root_keeps_inline_content() {
  let document = build(
    Node::container([Node::container([
      Node::text("hello "),
      Node::image("assets/images/yeecord.png").with_class_name("pic"),
    ])
    .with_class_name("p fade")
    .with_id("paragraph")])
    .with_class_name("card")
    .with_id("card"),
  );

  let paragraph = find(&document, "paragraph");
  assert!(matches!(
    &paragraph.kind,
    NodeKind::Box { effects: Some(effects), .. } if effects.opacity == 0.5
  ));
  let texts: Vec<&str> = runs(&document, paragraph)
    .iter()
    .map(|run| run.text.as_str())
    .collect();
  assert_eq!(texts, ["hello "]);
  assert!(
    document
      .nodes
      .iter()
      .any(|node| matches!(node.kind, NodeKind::Image { .. })),
    "inline image lost from the stacking-context root"
  );
}

#[test]
fn zero_font_size_container_keeps_sized_child_runs() {
  let document = build(
    Node::container([Node::container([
      Node::container([Node::text("visible")]).with_class_name("sized")
    ])
    .with_class_name("p collapsed")
    .with_id("paragraph")])
    .with_class_name("card")
    .with_id("card"),
  );

  let paragraph = find(&document, "paragraph");
  let runs = runs(&document, paragraph);
  assert_eq!(
    runs.iter().map(|run| run.text.as_str()).collect::<Vec<_>>(),
    ["visible"]
  );
  assert_eq!(runs[0].font_size, 18.0);
}

#[test]
fn unpainted_root_still_leaves_a_root_box() {
  for class_name in ["card fade-out", "card flatten"] {
    let document = paint_tree(
      PaintTreeOptions::builder()
        .viewport(Viewport::new((200, 100)))
        .node(Node::container([Node::text("hidden")]).with_class_name(class_name))
        .fonts(&CONTEXT)
        .stylesheet(
          StyleSheet::parse_loosy(
            ".fade-out { opacity: 0 } .flatten { transform: scale(0) } .card { width: 100px }",
          )
          .into(),
        )
        .build(),
    )
    .unwrap()
    .document;

    let root = &document.nodes[0];
    assert!(matches!(root.kind, NodeKind::Box { .. }), "{class_name}");
    assert_eq!(root.parent, None, "{class_name}");
    assert!(
      document
        .nodes
        .iter()
        .skip(1)
        .all(|node| node.parent.is_some())
    );
  }
}

#[test]
fn a_hairline_repeating_gradient_paints_its_average() {
  let document = paint_tree(
    PaintTreeOptions::builder()
      .viewport(Viewport::new((1000, 100)))
      .node(Node::container([]).with_class_name("stripes"))
      .fonts(&CONTEXT)
      .stylesheet(
        StyleSheet::parse_loosy(
          ".stripes { width: 1000px; height: 100px; background-image: repeating-linear-gradient(90deg, rgb(255, 0, 0) 0, rgb(0, 0, 255) 0.002px) }",
        )
        .into(),
      )
      .build(),
  )
  .unwrap()
  .document;

  let stops = document
    .nodes
    .iter()
    .flat_map(|node| &node.drawables)
    .find_map(|drawable| match drawable {
      Drawable::Fill {
        paint: Paint::Pattern { tile, .. },
        ..
      } => match tile.as_ref() {
        Paint::LinearGradient { stops, .. } => Some(stops.clone()),
        _ => None,
      },
      _ => None,
    })
    .expect("the gradient layer");

  let [red, green, blue, alpha] = stops[0].color;

  assert_eq!(stops.len(), 2);
  assert_eq!(stops[0].color, stops[1].color);
  assert!(
    red.abs_diff(blue) <= 2 && green < 8 && alpha == 255,
    "{:?}",
    stops[0].color
  );
}
