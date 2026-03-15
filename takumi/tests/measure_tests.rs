mod test_utils;

use takumi::{
  layout::{
    Viewport,
    node::Node,
    style::{
      Affine, AlignItems, BorderStyle, Color, ColorInput, Display, FlexDirection, JustifyContent,
      Length::*, Position, Sides, Style, StyleDeclaration,
    },
  },
  rendering::{MeasuredNode, MeasuredTextRun, RenderOptions, measure_layout},
};
use test_utils::CONTEXT;

fn create_measure_viewport() -> Viewport {
  (1200, 630).into()
}

fn create_measure_viewport_with_dpr(device_pixel_ratio: f32) -> Viewport {
  Viewport::new(
    Some((1200.0 * device_pixel_ratio) as u32),
    Some((630.0 * device_pixel_ratio) as u32),
  )
  .with_device_pixel_ratio(device_pixel_ratio)
}

fn measure(node: Node, viewport: Viewport) -> MeasuredNode {
  measure_layout(
    RenderOptions::builder()
      .viewport(viewport)
      .node(node)
      .global(&CONTEXT)
      .build(),
  )
  .unwrap()
}

fn assert_close(actual: f32, expected: f32) {
  assert!(
    (actual - expected).abs() <= 0.01,
    "expected {expected}, got {actual}"
  );
}

#[test]
fn test_measure_simple_container() {
  let node: Node = Node::container([]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Px(100.0)))
      .with(StyleDeclaration::height(Px(100.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([255, 0, 0, 255]),
      ))),
  );

  let result = measure_layout(
    RenderOptions::builder()
      .viewport(create_measure_viewport())
      .node(node)
      .global(&CONTEXT)
      .build(),
  )
  .unwrap();

  assert_eq!(
    result,
    MeasuredNode {
      width: 100.0,
      height: 100.0,
      transform: Affine::IDENTITY.to_cols_array(),
      children: Vec::new(),
      runs: Vec::new(),
    }
  );
}

#[test]
fn test_measure_text_node() {
  let node: Node = Node::text("Hello World".to_string()).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Px(300.0)))
      .with(StyleDeclaration::font_size(Px(20.0).into())),
  );

  let result = measure_layout(
    RenderOptions::builder()
      .viewport(create_measure_viewport())
      .node(node)
      .global(&CONTEXT)
      .build(),
  )
  .unwrap();

  assert_eq!(
    result,
    MeasuredNode {
      width: 300.0,
      height: 26.0,
      transform: Affine::IDENTITY.to_cols_array(),
      children: vec![MeasuredNode {
        width: 106.0,
        height: 26.0,
        transform: Affine::IDENTITY.to_cols_array(),
        children: Vec::new(),
        runs: vec![MeasuredTextRun {
          text: "Hello World".to_string(),
          x: 0.0,
          y: -0.10000038,
          width: 105.46001,
          height: 26.0,
        }],
      }],
      runs: Vec::new(),
    }
  )
}

#[test]
fn test_measure_flex_text_node_centers_inner_text() {
  let node: Node = Node::text("Hello World".to_string()).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Px(300.0)))
      .with(StyleDeclaration::height(Px(120.0)))
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::justify_content(JustifyContent::Center))
      .with(StyleDeclaration::align_items(AlignItems::Center))
      .with(StyleDeclaration::font_size(Px(20.0).into())),
  );

  let result = measure_layout(
    RenderOptions::builder()
      .viewport(create_measure_viewport())
      .node(node)
      .global(&CONTEXT)
      .build(),
  )
  .unwrap();

  assert_eq!(result.width, 300.0);
  assert_eq!(result.height, 120.0);
  assert_eq!(result.children.len(), 1);
  assert_eq!(result.runs.len(), 0);

  let anonymous_item = &result.children[0];
  assert_eq!(anonymous_item.runs.len(), 1);
  let run = &anonymous_item.runs[0];
  let expected_x = (result.width - run.width) / 2.0;
  let expected_y = (result.height - run.height) / 2.0;
  let global_run_x = anonymous_item.transform[4] + run.x;
  let global_run_y = anonymous_item.transform[5] + run.y;
  assert!(
    (global_run_x - expected_x).abs() <= 1.0,
    "run.x = {}",
    global_run_x
  );
  assert!(
    (global_run_y - expected_y).abs() <= 1.0,
    "run.y = {}",
    global_run_y
  );
}

#[test]
fn test_measure_flex_text_node_anonymous_item_uses_intrinsic_size() {
  let node: Node = Node::text("Hello World".to_string()).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Px(300.0)))
      .with(StyleDeclaration::height(Px(120.0)))
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::justify_content(JustifyContent::Center))
      .with(StyleDeclaration::align_items(AlignItems::Center))
      .with(StyleDeclaration::font_size(Px(20.0).into())),
  );

  let result = measure_layout(
    RenderOptions::builder()
      .viewport(create_measure_viewport())
      .node(node)
      .global(&CONTEXT)
      .build(),
  )
  .unwrap();

  assert_eq!(result.children.len(), 1);
  let anonymous_item = &result.children[0];

  assert!(
    anonymous_item.width < result.width,
    "anonymous item width should be intrinsic, got child={} parent={}",
    anonymous_item.width,
    result.width
  );
  assert!(
    anonymous_item.height <= result.height,
    "anonymous item height should fit parent, got child={} parent={}",
    anonymous_item.height,
    result.height
  );
}

#[test]
fn test_measure_inline_layout() {
  let children: Vec<Node> = vec![
    Node::text("Hello World".to_string())
      .with_style(Style::default().with(StyleDeclaration::display(Display::Inline))),
    Node::image("assets/images/yeecord.png").with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Inline))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([255, 0, 0, 255]),
        ))),
    ),
    Node::text("This is Takumi Speaking".to_string())
      .with_style(Style::default().with(StyleDeclaration::display(Display::Inline))),
  ];

  let node: Node = Node::container(children).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Px(400.0)))
      .with(StyleDeclaration::height(Px(300.0)))
      .with(StyleDeclaration::font_size(Px(20.0).into()))
      .with(StyleDeclaration::display(Display::Block)),
  );

  let result = measure_layout(
    RenderOptions::builder()
      .viewport(create_measure_viewport())
      .node(node)
      .global(&CONTEXT)
      .build(),
  )
  .unwrap();

  assert_eq!(
    result,
    MeasuredNode {
      width: 400.0,
      height: 300.0,
      transform: Affine::IDENTITY.to_cols_array(),
      runs: vec![
        MeasuredTextRun {
          text: "Hello World".to_string(),
          x: 0.0,
          y: 104.9, // we have the image 128px height on the same line, so the text is centered vertically
          width: 105.46001,
          height: 26.0,
        },
        MeasuredTextRun {
          text: "This is Takumi ".to_string(),
          x: 233.46,
          y: 104.9,
          width: 132.79999,
          height: 26.0,
        },
        MeasuredTextRun {
          text: "Speaking".to_string(),
          x: 0.0,
          y: 127.9,
          width: 85.71999,
          height: 26.0,
        },
      ],
      children: vec![MeasuredNode {
        width: 128.0,
        height: 128.0,
        transform: [1.0, 0.0, 0.0, 1.0, 105.46001, -3.0],
        children: Vec::new(),
        runs: Vec::new(),
      }],
    }
  )
}

#[test]
fn test_measure_inline_layout_preserves_text_span_boundaries() {
  let node: Node = Node::container([
    Node::text("STEAM ".to_string()).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::display(Display::Inline)),
    ),
    Node::text("education can".to_string()).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::display(Display::Inline))
        .with(StyleDeclaration::outline_width(Px(2.0)))
        .with(StyleDeclaration::outline_style(BorderStyle::Solid))
        .with(StyleDeclaration::outline_color(ColorInput::Value(Color([
          255, 0, 0, 255,
        ])))),
    ),
    Node::text(" for everyone.".to_string()).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::display(Display::Inline)),
    ),
  ])
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Px(600.0)))
      .with(StyleDeclaration::height(Px(120.0)))
      .with(StyleDeclaration::font_size(Px(20.0).into()))
      .with(StyleDeclaration::display(Display::Block)),
  );

  let result = measure(node, create_measure_viewport());

  assert_eq!(
    result
      .runs
      .iter()
      .map(|run| run.text.as_str())
      .collect::<Vec<_>>(),
    vec!["STEAM", "education can", "for everyone."]
  );
}

#[test]
fn test_measure_text_node_rem_font_size_matches_px_when_dpr_is_below_one() {
  let viewport = create_measure_viewport_with_dpr(0.75);
  let text = "Rem font size still applies".to_string();

  let rem_result = measure(
    Node::text(text.clone()).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::width(Px(400.0)))
        .with(StyleDeclaration::font_size(Rem(1.0).into())),
    ),
    viewport,
  );

  let px_result = measure(
    Node::text(text).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::width(Px(400.0)))
        .with(StyleDeclaration::font_size(Px(16.0).into())),
    ),
    viewport,
  );

  assert_eq!(rem_result.children.len(), 1);
  assert_eq!(px_result.children.len(), 1);

  let rem_text = &rem_result.children[0];
  let px_text = &px_result.children[0];

  assert_close(rem_result.height, px_result.height);
  assert_close(rem_text.width, px_text.width);
  assert_close(rem_text.height, px_text.height);
  assert_close(rem_text.runs[0].width, px_text.runs[0].width);
  assert_close(rem_text.runs[0].height, px_text.runs[0].height);
}

#[test]
fn test_measure_nested_em_font_size_inherits_correctly_from_rem_when_dpr_is_below_one() {
  let viewport = create_measure_viewport_with_dpr(0.75);

  let rem_parent_result = measure(
    Node::container([Node::text("Nested em".to_string()).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::font_size(Em(2.0).into())),
    )])
    .with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::width(Px(400.0)))
        .with(StyleDeclaration::font_size(Rem(1.0).into())),
    ),
    viewport,
  );

  let px_parent_result = measure(
    Node::container([Node::text("Nested em".to_string()).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::font_size(Em(2.0).into())),
    )])
    .with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::width(Px(400.0)))
        .with(StyleDeclaration::font_size(Px(16.0).into())),
    ),
    viewport,
  );

  assert_eq!(rem_parent_result.children.len(), 1);
  assert_eq!(px_parent_result.children.len(), 1);

  let rem_text = &rem_parent_result.children[0].children[0];
  let px_text = &px_parent_result.children[0].children[0];

  assert_close(
    rem_parent_result.children[0].height,
    px_parent_result.children[0].height,
  );
  assert_close(rem_text.width, px_text.width);
  assert_close(rem_text.height, px_text.height);
  assert_close(rem_text.runs[0].width, px_text.runs[0].width);
  assert_close(rem_text.runs[0].height, px_text.runs[0].height);
}

#[test]
fn test_measure_svg_attr_size_in_absolute_flex_container() {
  let svg = r##"<svg width="100" height="100" viewBox="0 0 40 40" fill="none" xmlns="http://www.w3.org/2000/svg"><path d="M20 0L24.4903 15.5097L40 20L24.4903 24.4903L20 40L15.5097 24.4903L0 20L15.5097 15.5097L20 0Z" fill="#E0FF25"/></svg>"##;

  let node: Node = Node::container([Node::container([Node::image(svg).with_tag_name("svg")])
    .with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::position(Position::Absolute))
        .with_inset(Sides([Auto, Px(40.0), Px(40.0), Auto]))
        .with(StyleDeclaration::display(Display::Flex)),
    )])
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0))),
  );

  let result = measure_layout(
    RenderOptions::builder()
      .viewport(create_measure_viewport())
      .node(node)
      .global(&CONTEXT)
      .build(),
  )
  .unwrap();

  assert_eq!(result.children.len(), 1);

  let absolute_container = &result.children[0];
  assert_eq!(absolute_container.width, 100.0);
  assert_eq!(absolute_container.height, 100.0);
  assert_eq!(
    absolute_container.transform,
    [1.0, 0.0, 0.0, 1.0, 1060.0, 490.0]
  );
  assert_eq!(absolute_container.children.len(), 1);

  let svg_child = &absolute_container.children[0];
  assert_eq!(svg_child.width, 100.0);
  assert_eq!(svg_child.height, 100.0);
  assert_eq!(svg_child.transform, [1.0, 0.0, 0.0, 1.0, 1060.0, 490.0]);
}

#[test]
fn test_measure_svg_attr_size_in_absolute_flex_container_with_parent_padding() {
  let svg = r##"<svg width="150" height="46" viewBox="0 0 90 28" fill="none" xmlns="http://www.w3.org/2000/svg"><path d="M0 0L10 10" fill="#FFFFFF"/></svg>"##;

  let node: Node = Node::container([Node::container([
    Node::image((svg, 150.0, 46.0)).with_tag_name("svg")
  ])
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::position(Position::Absolute))
      .with_inset(Sides([Auto, Px(60.0), Px(60.0), Auto]))
      .with(StyleDeclaration::display(Display::Flex)),
  )])
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::position(Position::Relative))
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::flex_direction(FlexDirection::Column))
      .with(StyleDeclaration::justify_content(JustifyContent::Center))
      .with_padding(Sides([Px(60.0); 4])),
  );

  let result = measure_layout(
    RenderOptions::builder()
      .viewport(create_measure_viewport())
      .node(node)
      .global(&CONTEXT)
      .build(),
  )
  .unwrap();

  assert_eq!(result.children.len(), 1);

  let absolute_container = &result.children[0];
  assert_eq!(absolute_container.width, 150.0);
  assert_eq!(absolute_container.height, 46.0);
  assert_eq!(
    absolute_container.transform,
    [1.0, 0.0, 0.0, 1.0, 990.0, 524.0]
  );
  assert_eq!(absolute_container.children.len(), 1);

  let svg_child = &absolute_container.children[0];
  assert_eq!(svg_child.width, 150.0);
  assert_eq!(svg_child.height, 46.0);
  assert_eq!(svg_child.transform, [1.0, 0.0, 0.0, 1.0, 990.0, 524.0]);
}

#[test]
fn test_measure_svg_with_width_only_preserves_intrinsic_ratio() {
  let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 128 128"><circle cx="64" cy="64" r="64" fill="#ffffff"/></svg>"##;

  let node: Node = Node::container([Node::image(svg).with_tag_name("svg").with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Px(96.0))),
  )])
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::flex_direction(FlexDirection::Column)),
  );

  let result = measure_layout(
    RenderOptions::builder()
      .viewport(create_measure_viewport())
      .node(node)
      .global(&CONTEXT)
      .build(),
  )
  .unwrap();

  assert_eq!(result.children.len(), 1);
  let image = &result.children[0];
  assert_eq!(image.width, 96.0);
  assert_eq!(image.height, 96.0);
}

#[test]
fn test_measure_img_svg_attribute_sizing_cases() {
  let cases = [
    (
      r##"<svg xmlns="http://www.w3.org/2000/svg" width="240" height="180" viewBox="0 0 240 180"><rect width="240" height="180" fill="#000"/></svg>"##,
      Some(60.0),
      Some(60.0),
      60.0,
      60.0,
    ),
    (
      r##"<svg xmlns="http://www.w3.org/2000/svg" width="240" height="180" viewBox="0 0 240 180"><rect width="240" height="180" fill="#000"/></svg>"##,
      Some(60.0),
      None,
      60.0,
      45.0,
    ),
    (
      r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 240 180"><rect width="240" height="180" fill="#000"/></svg>"##,
      Some(60.0),
      None,
      60.0,
      45.0,
    ),
    (
      r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 240 180"><rect width="240" height="180" fill="#000"/></svg>"##,
      Some(60.0),
      Some(60.0),
      60.0,
      60.0,
    ),
  ];

  for (case_index, (svg, width, height, expected_width, expected_height)) in
    cases.into_iter().enumerate()
  {
    let image = Node::image((svg, width, height))
      .with_tag_name("img")
      .with_preset(
        Style::default()
          .with(StyleDeclaration::display(Display::Flex))
          .with(StyleDeclaration::display(Display::Inline)),
      );

    let node: Node = Node::container([image]).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::height(Percentage(100.0)))
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::flex_direction(FlexDirection::Column)),
    );

    let result = measure_layout(
      RenderOptions::builder()
        .viewport(create_measure_viewport())
        .node(node)
        .global(&CONTEXT)
        .build(),
    )
    .unwrap();

    assert_eq!(result.children.len(), 1);
    let image = &result.children[0];
    assert_eq!(image.width, expected_width, "case {} width", case_index);
    assert_eq!(image.height, expected_height, "case {} height", case_index);
  }
}
