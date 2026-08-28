mod test_utils;

use takumi::prelude::{Length::*, *};
use test_utils::{CONTEXT, TEST_IMAGES};

fn create_measure_viewport() -> Viewport {
  Viewport::new((1200, 630))
}

fn create_measure_viewport_with_dpr(device_pixel_ratio: f32) -> Viewport {
  Viewport::new((
    (1200.0 * device_pixel_ratio) as u32,
    (630.0 * device_pixel_ratio) as u32,
  ))
  .with_device_pixel_ratio(device_pixel_ratio)
}

fn measure(node: Node, viewport: Viewport) -> MeasuredNode {
  takumi::measure(
    RenderOptions::builder()
      .viewport(viewport)
      .node(node)
      .fonts(&CONTEXT)
      .images(TEST_IMAGES.clone())
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

fn assert_within(actual: f32, expected: f32, tolerance: f32) {
  assert!(
    (actual - expected).abs() <= tolerance,
    "expected {expected} +/- {tolerance}, got {actual}"
  );
}

fn measured_text_runs(result: &MeasuredNode) -> &[MeasuredTextRun] {
  if !result.runs.is_empty() {
    return &result.runs;
  }

  assert!(
    !result.children.is_empty(),
    "no measured text runs found in {result:#?}"
  );
  assert_eq!(result.children.len(), 1);
  &result.children[0].runs
}

fn assert_text_runs_same(actual: &[MeasuredTextRun], expected: &[MeasuredTextRun]) {
  assert_eq!(actual.len(), expected.len());

  for (actual, expected) in actual.iter().zip(expected) {
    assert_eq!(actual.text, expected.text);
    assert_within(actual.x, expected.x, 0.05);
    assert_within(actual.y, expected.y, 0.05);
    assert_within(actual.width, expected.width, 0.05);
    assert_within(actual.height, expected.height, 0.05);
  }
}

fn assert_measured_node_same(actual: &MeasuredNode, expected: &MeasuredNode) {
  assert_within(actual.width, expected.width, 0.05);
  assert_within(actual.height, expected.height, 0.05);

  for (actual, expected) in actual.transform.iter().zip(expected.transform.iter()) {
    assert_within(*actual, *expected, 0.05);
  }

  assert_text_runs_same(&actual.runs, &expected.runs);
  assert_eq!(actual.children.len(), expected.children.len());
  for (actual, expected) in actual.children.iter().zip(expected.children.iter()) {
    assert_measured_node_same(actual, expected);
  }
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

  let result = takumi::measure(
    RenderOptions::builder()
      .viewport(create_measure_viewport())
      .node(node)
      .fonts(&CONTEXT)
      .images(TEST_IMAGES.clone())
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

  let result = takumi::measure(
    RenderOptions::builder()
      .viewport(create_measure_viewport())
      .node(node)
      .fonts(&CONTEXT)
      .images(TEST_IMAGES.clone())
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

  let result = takumi::measure(
    RenderOptions::builder()
      .viewport(create_measure_viewport())
      .node(node)
      .fonts(&CONTEXT)
      .images(TEST_IMAGES.clone())
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

  let result = takumi::measure(
    RenderOptions::builder()
      .viewport(create_measure_viewport())
      .node(node)
      .fonts(&CONTEXT)
      .images(TEST_IMAGES.clone())
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

  let result = takumi::measure(
    RenderOptions::builder()
      .viewport(create_measure_viewport())
      .node(node)
      .fonts(&CONTEXT)
      .images(TEST_IMAGES.clone())
      .build(),
  )
  .unwrap();

  assert_eq!(result.width, 400.0);
  assert_eq!(result.height, 300.0);
  assert_eq!(result.transform, Affine::IDENTITY.to_cols_array());

  assert_eq!(result.children.len(), 1);
  let inline_image = &result.children[0];
  assert_eq!(inline_image.width, 128.0);
  assert_eq!(inline_image.height, 128.0);
  assert_within(inline_image.transform[4], 105.47, 0.1);
  assert_within(inline_image.transform[5], 0.0, 1.5);

  assert_eq!(result.runs.len(), 3);

  let first = &result.runs[0];
  assert_eq!(first.text, "Hello World");
  assert_within(first.x, 0.0, 0.1);
  assert_within(first.y, 108.0, 1.5);
  assert_within(first.width, 105.47, 0.1);
  assert_within(first.height, 26.0, 0.1);

  let second = &result.runs[1];
  assert_eq!(second.text, "This is Takumi ");
  assert_within(second.x, 233.47, 0.1);
  assert_within(second.y, 108.0, 1.5);
  assert_within(second.height, 26.0, 0.1);

  let third = &result.runs[2];
  assert_eq!(third.text, "Speaking");
  assert_within(third.x, 0.0, 0.1);
  assert_within(third.y, 134.0, 1.5);
  assert_within(third.width, 85.73, 0.1);
  assert_within(third.height, 26.0, 0.1);
}

#[test]
fn test_measure_text_fit_per_line_grow_scales_run_geometry() {
  let base_style = Style::default()
    .with(StyleDeclaration::display(Display::Flex))
    .with(StyleDeclaration::width(Px(320.0)))
    .with(StyleDeclaration::font_size(Px(34.0).into()))
    .with(StyleDeclaration::line_height(LineHeight::Unitless(1.0)))
    .with(StyleDeclaration::text_wrap_mode(TextWrapMode::NoWrap))
    .with(StyleDeclaration::white_space_collapse(
      WhiteSpaceCollapse::PreserveBreaks,
    ));
  let text = "Short\nA much longer line".to_string();

  let no_fit = measure(
    Node::text(text.clone()).with_style(base_style.clone()),
    create_measure_viewport(),
  );
  let fit = measure(
    Node::text(text).with_style(
      base_style.clone().with(StyleDeclaration::text_fit(
        TextFit::builder()
          .mode(TextFitMode::Grow)
          .target(TextFitTarget::PerLineAll)
          .limit(Some(1.8))
          .build(),
      )),
    ),
    create_measure_viewport(),
  );

  let no_fit_runs = measured_text_runs(&no_fit);
  let fit_runs = measured_text_runs(&fit);
  assert_eq!(no_fit_runs.len(), 2);
  assert_eq!(fit_runs.len(), 2);
  assert!(fit_runs[0].width > no_fit_runs[0].width);
  assert!(fit_runs[0].height > no_fit_runs[0].height);
  assert!(fit.children[0].height > no_fit.children[0].height);
}

/// taffy subtracts the content-box inset without a floor, so the available
/// width reaching the text can be negative. parley asserts on that.
#[test]
fn test_measure_padding_wider_than_the_box_does_not_panic() {
  let node = Node::from_html(
    r#"<div style="width:40px"><div style="padding:0 60px; font-size:24px">word up</div></div>"#,
    FromHtmlOptions::default(),
  )
  .expect("parse");
  let out = measure(node, create_measure_viewport());

  assert_close(out.width, 40.0);
}

/// taffy hands the measure function a border-box width whatever `box-sizing`
/// says, so a content-box box used to wrap against its border-box width.
#[test]
fn test_measure_content_box_wraps_inside_its_padding() {
  let text = "aa bb cc dd ee ff gg hh";
  let content_box = measure(
    Node::from_html(
      &format!(
        r#"<div style="display:flex"><div style="box-sizing:content-box; width:100px; padding:0 40px; font-size:24px">{text}</div></div>"#
      ),
      FromHtmlOptions::default(),
    )
    .expect("parse"),
    create_measure_viewport(),
  );
  let border_box = measure(
    Node::from_html(
      &format!(
        r#"<div style="display:flex"><div style="box-sizing:border-box; width:180px; padding:0 40px; font-size:24px">{text}</div></div>"#
      ),
      FromHtmlOptions::default(),
    )
    .expect("parse"),
    create_measure_viewport(),
  );

  assert_close(content_box.height, border_box.height);
}

/// A text node carries its own inline content but has no children, so the
/// measure traversal used to walk past it without emitting a run.
#[test]
fn test_measure_reports_runs_for_a_bare_text_node() {
  let html = |body: &str| {
    Node::from_html(
      &format!(r#"<div style="width:320px; font-size:32px;">{body}</div>"#),
      FromHtmlOptions::default(),
    )
    .expect("parse")
  };
  let bare = measure(html("word"), create_measure_viewport());
  let wrapped = measure(html("<span>word</span>"), create_measure_viewport());

  let bare_runs = measured_text_runs(&bare);
  let wrapped_runs = measured_text_runs(&wrapped);
  assert_eq!(bare_runs.len(), wrapped_runs.len());
  assert_close(bare_runs[0].width, wrapped_runs[0].width);
  assert_close(bare_runs[0].height, wrapped_runs[0].height);
}

#[test]
fn test_measure_text_fit_per_line_shrink_scales_run_geometry() {
  let base_style = Style::default()
    .with(StyleDeclaration::display(Display::Flex))
    .with(StyleDeclaration::width(Px(320.0)))
    .with(StyleDeclaration::font_size(Px(34.0).into()))
    .with(StyleDeclaration::line_height(LineHeight::Unitless(1.0)))
    .with(StyleDeclaration::text_wrap_mode(TextWrapMode::NoWrap))
    .with(StyleDeclaration::white_space_collapse(
      WhiteSpaceCollapse::PreserveBreaks,
    ));
  let text =
    "This first line is intentionally wide\nThis second line also needs shrinking".to_string();

  let no_fit = measure(
    Node::text(text.clone()).with_style(base_style.clone()),
    create_measure_viewport(),
  );
  let fit = measure(
    Node::text(text).with_style(
      base_style.with(StyleDeclaration::text_fit(
        TextFit::builder()
          .mode(TextFitMode::Shrink)
          .target(TextFitTarget::PerLineAll)
          .build(),
      )),
    ),
    create_measure_viewport(),
  );

  let no_fit_runs = measured_text_runs(&no_fit);
  let fit_runs = measured_text_runs(&fit);
  assert_eq!(no_fit_runs.len(), 2);
  assert_eq!(fit_runs.len(), 2);
  assert!(fit_runs[0].width < no_fit_runs[0].width);
  assert!(fit_runs[0].height < no_fit_runs[0].height);
}

#[test]
fn test_measure_text_fit_per_line_skips_forced_break_lines() {
  let base_style = Style::default()
    .with(StyleDeclaration::display(Display::Flex))
    .with(StyleDeclaration::width(Px(320.0)))
    .with(StyleDeclaration::font_size(Px(34.0).into()))
    .with(StyleDeclaration::line_height(LineHeight::Unitless(1.0)))
    .with(StyleDeclaration::text_wrap_mode(TextWrapMode::NoWrap))
    .with(StyleDeclaration::white_space_collapse(
      WhiteSpaceCollapse::PreserveBreaks,
    ));
  let text = "Short\nA much longer line".to_string();

  let no_fit = measure(
    Node::text(text.clone()).with_style(base_style.clone()),
    create_measure_viewport(),
  );
  let per_line = measure(
    Node::text(text.clone()).with_style(
      base_style.clone().with(StyleDeclaration::text_fit(
        TextFit::builder()
          .mode(TextFitMode::Grow)
          .target(TextFitTarget::PerLine)
          .limit(Some(1.8))
          .build(),
      )),
    ),
    create_measure_viewport(),
  );
  let per_line_all = measure(
    Node::text(text).with_style(
      base_style.with(StyleDeclaration::text_fit(
        TextFit::builder()
          .mode(TextFitMode::Grow)
          .target(TextFitTarget::PerLineAll)
          .limit(Some(1.8))
          .build(),
      )),
    ),
    create_measure_viewport(),
  );

  let no_fit_runs = measured_text_runs(&no_fit);
  let per_line_runs = measured_text_runs(&per_line);
  let per_line_all_runs = measured_text_runs(&per_line_all);

  assert_text_runs_same(per_line_runs, no_fit_runs);
  assert!(per_line_all_runs[0].width > no_fit_runs[0].width);
  assert!(per_line_all_runs[0].height > no_fit_runs[0].height);
}

fn grow_per_line_all_text_fit() -> TextFit {
  TextFit::builder()
    .mode(TextFitMode::Grow)
    .target(TextFitTarget::PerLineAll)
    .limit(Some(1.8))
    .build()
}

fn measure_text_fit_line_height(line_height: LineHeight) -> (MeasuredNode, MeasuredNode) {
  let base_style = Style::default()
    .with(StyleDeclaration::display(Display::Flex))
    .with(StyleDeclaration::width(Px(320.0)))
    .with(StyleDeclaration::font_size(Px(34.0).into()))
    .with(StyleDeclaration::line_height(line_height))
    .with(StyleDeclaration::text_wrap_mode(TextWrapMode::NoWrap))
    .with(StyleDeclaration::white_space_collapse(
      WhiteSpaceCollapse::PreserveBreaks,
    ));
  let text = "Short\nA much longer line".to_string();

  let no_fit = measure(
    Node::text(text.clone()).with_style(base_style.clone()),
    create_measure_viewport(),
  );
  let fit = measure(
    Node::text(text)
      .with_style(base_style.with(StyleDeclaration::text_fit(grow_per_line_all_text_fit()))),
    create_measure_viewport(),
  );

  (no_fit, fit)
}

#[test]
fn test_measure_text_fit_grow_scales_unitless_line_height() {
  let (no_fit, fit) = measure_text_fit_line_height(LineHeight::Unitless(4.0));

  let no_fit_runs = measured_text_runs(&no_fit);
  let fit_runs = measured_text_runs(&fit);
  assert_eq!(no_fit_runs.len(), 2);
  assert_eq!(fit_runs.len(), 2);
  assert!(fit_runs[0].width > no_fit_runs[0].width);
  assert!(fit_runs[0].height > no_fit_runs[0].height);
  assert!(fit.children[0].height > no_fit.children[0].height);
}

#[test]
fn test_measure_text_fit_grow_preserves_absolute_line_height() {
  let (no_fit, fit) = measure_text_fit_line_height(LineHeight::Length(Px(40.0)));

  let no_fit_runs = measured_text_runs(&no_fit);
  let fit_runs = measured_text_runs(&fit);
  assert_eq!(no_fit_runs.len(), 2);
  assert_eq!(fit_runs.len(), 2);
  assert!(fit_runs[0].width > no_fit_runs[0].width);
  assert!(fit_runs[0].height > no_fit_runs[0].height);
  assert_within(fit.children[0].height, no_fit.children[0].height, 0.05);
}

#[test]
fn test_measure_text_fit_grow_preserves_percentage_line_height() {
  let (no_fit, fit) = measure_text_fit_line_height(LineHeight::Length(Percentage(150.0)));

  let no_fit_runs = measured_text_runs(&no_fit);
  let fit_runs = measured_text_runs(&fit);
  assert_eq!(no_fit_runs.len(), 2);
  assert_eq!(fit_runs.len(), 2);
  assert!(fit_runs[0].width > no_fit_runs[0].width);
  assert!(fit_runs[0].height > no_fit_runs[0].height);
  assert_within(fit.children[0].height, no_fit.children[0].height, 0.05);
}

#[test]
fn test_measure_text_fit_center_alignment_keeps_scaled_text_centered() {
  let viewport = create_measure_viewport();
  let text = "Takumi 1.2 now support the latest.".to_string();
  let base = Style::default()
    .with(StyleDeclaration::display(Display::Block))
    .with(StyleDeclaration::width(Percentage(100.0)))
    .with(StyleDeclaration::font_size(Px(48.0).into()))
    .with(StyleDeclaration::font_weight(FontWeight::from(700.0)))
    .with(StyleDeclaration::text_align(TextAlign::Center));
  let no_fit = measure(
    Node::container([Node::text(text.clone())]).with_style(base.clone()),
    viewport,
  );
  let measured = measure(
    Node::container([Node::text(text)]).with_style(
      base.with(StyleDeclaration::text_fit(
        TextFit::builder()
          .mode(TextFitMode::Grow)
          .target(TextFitTarget::PerLineAll)
          .build(),
      )),
    ),
    viewport,
  );
  let no_fit_runs = measured_text_runs(&no_fit);
  let runs = measured_text_runs(&measured);
  assert_eq!(no_fit_runs.len(), 1);
  assert_eq!(runs.len(), 1);
  assert!(runs[0].width > no_fit_runs[0].width);

  let run = &runs[0];
  assert_within(run.x, (1200.0 - run.width) * 0.5, 0.1);
}

#[test]
fn test_measure_text_fit_is_disabled_by_floats() {
  let base_style = Style::default()
    .with(StyleDeclaration::display(Display::Block))
    .with(StyleDeclaration::width(Px(240.0)))
    .with(StyleDeclaration::font_size(Px(20.0).into()))
    .with(StyleDeclaration::line_height(LineHeight::Unitless(1.2)));
  let fit_style = base_style.clone().with(StyleDeclaration::text_fit(
    TextFit::builder()
      .mode(TextFitMode::Shrink)
      .target(TextFitTarget::Consistent)
      .build(),
  ));
  let node = |style: Style| {
    Node::container([
      Node::image("assets/images/yeecord.png").with_style(
        Style::default()
          .with(StyleDeclaration::display(Display::Inline))
          .with(StyleDeclaration::float(Float::Left))
          .with(StyleDeclaration::width(Px(72.0)))
          .with(StyleDeclaration::height(Px(72.0))),
      ),
      Node::text(
        "Takumi should wrap this sentence around the floated image for the first few lines before returning to the full measure width once the float ends.".to_string(),
      )
      .with_style(Style::default().with(StyleDeclaration::display(Display::Inline))),
    ])
    .with_style(style)
  };

  let no_fit = measure(node(base_style), create_measure_viewport());
  let fit = measure(node(fit_style), create_measure_viewport());

  assert_measured_node_same(&fit, &no_fit);
}

#[test]
fn test_measure_text_fit_scales_text_around_inline_atomic_content() {
  let base_style = Style::default()
    .with(StyleDeclaration::display(Display::Block))
    .with(StyleDeclaration::width(Px(320.0)))
    .with(StyleDeclaration::font_size(Px(34.0).into()))
    .with(StyleDeclaration::line_height(LineHeight::Unitless(1.0)))
    .with(StyleDeclaration::text_wrap_mode(TextWrapMode::NoWrap));
  let fit_style = base_style.clone().with(StyleDeclaration::text_fit(
    TextFit::builder()
      .mode(TextFitMode::Grow)
      .target(TextFitTarget::Consistent)
      .limit(Some(1.8))
      .build(),
  ));
  let node = |style: Style| {
    Node::container([
      Node::text("Ship ".to_string())
        .with_style(Style::default().with(StyleDeclaration::display(Display::Inline))),
      Node::image(("assets/images/yeecord.png", 64.0, 64.0)).with_style(
        Style::default()
          .with(StyleDeclaration::display(Display::InlineBlock))
          .with(StyleDeclaration::width(Em(1.0)))
          .with(StyleDeclaration::height(Em(1.0))),
      ),
      Node::text(" now".to_string())
        .with_style(Style::default().with(StyleDeclaration::display(Display::Inline))),
    ])
    .with_style(style)
  };

  let no_fit = measure(node(base_style), create_measure_viewport());
  let fit = measure(node(fit_style), create_measure_viewport());

  let no_fit_runs = measured_text_runs(&no_fit);
  let fit_runs = measured_text_runs(&fit);

  assert_eq!(no_fit_runs.len(), 2);
  assert_eq!(fit_runs.len(), 2);
  assert!(fit_runs[0].width > no_fit_runs[0].width);
  assert!(fit_runs[1].width > no_fit_runs[1].width);
  assert!(fit.height > no_fit.height);

  assert_eq!(no_fit.children.len(), 1);
  assert_eq!(fit.children.len(), 1);
  assert_within(fit.children[0].width, no_fit.children[0].width, 0.05);
  assert_within(fit.children[0].height, no_fit.children[0].height, 0.05);
  assert_within(no_fit.children[0].transform[4], no_fit_runs[0].width, 0.1);
  assert_within(fit.children[0].transform[4], fit_runs[0].width, 0.1);
  assert_within(
    fit_runs[1].x,
    fit.children[0].transform[4] + fit.children[0].width,
    0.1,
  );
}

#[test]
fn test_measure_text_fit_is_disabled_by_spacing_adjustments() {
  let base_style = Style::default()
    .with(StyleDeclaration::display(Display::Flex))
    .with(StyleDeclaration::width(Px(320.0)))
    .with(StyleDeclaration::font_size(Px(34.0).into()))
    .with(StyleDeclaration::line_height(LineHeight::Unitless(1.0)))
    .with(StyleDeclaration::text_wrap_mode(TextWrapMode::NoWrap));
  let cases = [
    base_style
      .clone()
      .with(StyleDeclaration::letter_spacing(Px(2.0))),
    base_style.with(StyleDeclaration::word_spacing(Px(10.0))),
  ];

  for style in cases {
    let no_fit = measure(
      Node::text("Space words".to_string()).with_style(style.clone()),
      create_measure_viewport(),
    );
    let fit = measure(
      Node::text("Space words".to_string()).with_style(
        style.with(StyleDeclaration::text_fit(
          TextFit::builder()
            .mode(TextFitMode::Grow)
            .target(TextFitTarget::Consistent)
            .limit(Some(1.8))
            .build(),
        )),
      ),
      create_measure_viewport(),
    );

    assert_measured_node_same(&fit, &no_fit);
  }
}

#[test]
fn test_measure_left_float_offsets_text_runs_until_float_bottom() {
  let node = Node::container([
    Node::image("assets/images/yeecord.png").with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Inline))
        .with(StyleDeclaration::float(Float::Left))
        .with(StyleDeclaration::width(Px(72.0)))
        .with(StyleDeclaration::height(Px(72.0))),
    ),
    Node::text(
      "Takumi should wrap this sentence around the floated image for the first few lines before returning to the full measure width once the float ends.".to_string(),
    )
    .with_style(Style::default().with(StyleDeclaration::display(Display::Inline))),
  ])
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Block))
      .with(StyleDeclaration::width(Px(240.0)))
      .with(StyleDeclaration::font_size(Px(20.0).into()))
      .with(StyleDeclaration::line_height(LineHeight::Unitless(1.2))),
  );

  let result = measure(node, create_measure_viewport());
  let float_box = result
    .children
    .iter()
    .find(|child| (child.width - 72.0).abs() <= 0.01 && (child.height - 72.0).abs() <= 0.01)
    .expect("expected floated inline box to be measured");

  assert_close(float_box.transform[4], 0.0);
  assert_close(float_box.transform[5], 0.0);

  let mut saw_wrapped_line = false;
  let mut saw_full_width_line_below_float = false;
  for run in &result.runs {
    if run.x >= 70.0 {
      saw_wrapped_line = true;
    }

    if run.y >= 72.0 && run.x <= 1.0 {
      saw_full_width_line_below_float = true;
    }
  }

  assert!(
    saw_wrapped_line,
    "expected at least one line to start after the float"
  );
  assert!(
    saw_full_width_line_below_float,
    "expected text to return to the full line width below the float"
  );
}

#[test]
fn test_measure_floated_inline_block_container_is_not_dropped() {
  let node = Node::container([
    Node::container([Node::text("Card".to_string()).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Block))
        .with(StyleDeclaration::font_size(Px(18.0).into())),
    )])
    .with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::InlineBlock))
        .with(StyleDeclaration::float(Float::Left))
        .with(StyleDeclaration::width(Px(96.0)))
        .with(StyleDeclaration::height(Px(56.0))),
    ),
    Node::text(
      "Floated inline-block containers should remain in the inline formatting context instead of disappearing after blockification.".to_string(),
    )
    .with_style(Style::default().with(StyleDeclaration::display(Display::Inline))),
  ])
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Block))
      .with(StyleDeclaration::width(Px(260.0)))
      .with(StyleDeclaration::font_size(Px(20.0).into()))
      .with(StyleDeclaration::line_height(LineHeight::Unitless(1.2))),
  );

  let result = measure(node, create_measure_viewport());
  let float_box = result
    .children
    .iter()
    .find(|child| (child.width - 96.0).abs() <= 0.01 && (child.height - 56.0).abs() <= 0.01)
    .expect("expected floated inline-block container to be measured");

  assert_close(float_box.transform[4], 0.0);
  assert_close(float_box.transform[5], 0.0);
  assert!(
    result.runs.iter().any(|run| run.x >= 95.0),
    "expected text runs to wrap around the floated inline-block container"
  );
}

#[test]
fn test_measure_clear_left_moves_following_float_below_previous_left_float() {
  let node = Node::container([
    Node::container([]).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::InlineBlock))
        .with(StyleDeclaration::float(Float::Left))
        .with(StyleDeclaration::width(Px(72.0)))
        .with(StyleDeclaration::height(Px(72.0))),
    ),
    Node::container([]).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::InlineBlock))
        .with(StyleDeclaration::float(Float::Left))
        .with(StyleDeclaration::clear(Clear::Left))
        .with(StyleDeclaration::width(Px(48.0)))
        .with(StyleDeclaration::height(Px(48.0))),
    ),
    Node::text(
      "A cleared float should begin below the previous left float instead of sitting beside it."
        .to_string(),
    )
    .with_style(Style::default().with(StyleDeclaration::display(Display::Inline))),
  ])
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Block))
      .with(StyleDeclaration::width(Px(240.0)))
      .with(StyleDeclaration::font_size(Px(20.0).into()))
      .with(StyleDeclaration::line_height(LineHeight::Unitless(1.2))),
  );

  let result = measure(node, create_measure_viewport());
  let first_float = result
    .children
    .iter()
    .find(|child| (child.width - 72.0).abs() <= 0.01 && (child.height - 72.0).abs() <= 0.01)
    .expect("expected first floated box to be measured");
  let cleared_float = result
    .children
    .iter()
    .find(|child| (child.width - 48.0).abs() <= 0.01 && (child.height - 48.0).abs() <= 0.01)
    .expect("expected cleared floated box to be measured");

  assert_close(first_float.transform[4], 0.0);
  assert_close(first_float.transform[5], 0.0);
  assert_close(cleared_float.transform[4], 0.0);
  assert!(
    cleared_float.transform[5] >= 72.0,
    "expected cleared float to start below the first left float"
  );
}

#[test]
fn test_measure_line_box_reflows_below_float_that_intersects_tall_line() {
  let node = Node::container([
    Node::container([])
      .with_style(
        Style::default()
          .with(StyleDeclaration::display(Display::InlineBlock))
          .with(StyleDeclaration::float(Float::Left))
          .with(StyleDeclaration::width(Px(80.0)))
          .with(StyleDeclaration::height(Px(40.0))),
      ),
    Node::container([])
      .with_style(
        Style::default()
          .with(StyleDeclaration::display(Display::InlineBlock))
          .with(StyleDeclaration::width(Px(100.0)))
          .with(StyleDeclaration::height(Px(20.0))),
      ),
    Node::container([])
      .with_style(
        Style::default()
          .with(StyleDeclaration::display(Display::InlineBlock))
          .with(StyleDeclaration::float(Float::Left))
          .with(StyleDeclaration::width(Px(120.0)))
          .with(StyleDeclaration::height(Px(40.0))),
      ),
    Node::text(
      "Text after the second float should move below it when the current line box height intersects that float."
        .to_string(),
    )
    .with_style(Style::default().with(StyleDeclaration::display(Display::Inline))),
  ])
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Block))
      .with(StyleDeclaration::width(Px(180.0)))
      .with(StyleDeclaration::font_size(Px(20.0).into()))
      .with(StyleDeclaration::line_height(LineHeight::Unitless(3.0))),
  );

  let result = measure(node, create_measure_viewport());
  let second_float = result
    .children
    .iter()
    .find(|child| (child.width - 120.0).abs() <= 0.01 && (child.height - 40.0).abs() <= 0.01)
    .expect("expected second floated box to be measured");
  let first_run = result
    .runs
    .first()
    .expect("expected text after the floats to be measured");

  assert_close(second_float.transform[4], 0.0);
  assert_close(second_float.transform[5], 40.0);
  assert!(
    first_run.y >= 80.0,
    "expected text to reflow below the intersecting float instead of overlapping it"
  );
}

#[test]
fn test_measure_text_indent_first_line_only() {
  let node = Node::text("alpha\nbeta".to_string()).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Px(300.0)))
      .with(StyleDeclaration::font_size(Px(20.0).into()))
      .with(StyleDeclaration::white_space_collapse(
        WhiteSpaceCollapse::PreserveBreaks,
      ))
      .with(StyleDeclaration::text_indent(TextIndent::new(Px(24.0)))),
  );

  let result = measure(node, create_measure_viewport());
  let runs = measured_text_runs(&result);

  assert_eq!(runs.len(), 2);
  assert_close(runs[0].x, 24.0);
  assert_close(runs[1].x, 0.0);
}

#[test]
fn test_measure_text_indent_each_line() {
  let node = Node::text("alpha\nbeta\ngamma".to_string()).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Px(300.0)))
      .with(StyleDeclaration::font_size(Px(20.0).into()))
      .with(StyleDeclaration::white_space_collapse(
        WhiteSpaceCollapse::PreserveBreaks,
      ))
      .with(StyleDeclaration::text_indent(
        TextIndent::new(Px(24.0)).with_each_line(true),
      )),
  );

  let result = measure(node, create_measure_viewport());
  let runs = measured_text_runs(&result);

  assert_eq!(runs.len(), 3);
  assert_close(runs[0].x, 24.0);
  assert_close(runs[1].x, 24.0);
  assert_close(runs[2].x, 24.0);
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
        .with(StyleDeclaration::outline_width(Px(2.0).into()))
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
    vec!["STEAM ", "education can", " for everyone."]
  );
}

#[test]
fn test_measure_inline_layout_preserves_space_only_spans() {
  let node: Node = Node::container([
    Node::text("A".to_string()).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::display(Display::Inline)),
    ),
    Node::text(" ".to_string()).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::display(Display::Inline)),
    ),
    Node::text("B".to_string()).with_style(
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
    vec!["A", " ", "B"]
  );
}

#[test]
fn test_measure_inline_atomic_containers_fixture() {
  let atomic = |display, bg_color, border_color, label: &str| -> Node {
    Node::container([Node::text(label.to_string())]).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::display(display))
        .with_padding(Sides([Px(8.0); 4]))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          bg_color,
        )))
        .with_border_width(Sides([Px(5.0).into(); 4]))
        .with_border_style(Sides([BorderStyle::Solid; 4]))
        .with_border_color(Sides([ColorInput::Value(border_color); 4])),
    )
  };

  let node = Node::container([Node::container([
    Node::text("before ".to_string())
      .with_style(Style::default().with(StyleDeclaration::display(Display::Inline))),
    atomic(
      Display::InlineBlock,
      Color([255, 0, 0, 100]),
      Color([180, 20, 20, 255]),
      "inline-block",
    ),
    Node::text(" mid ".to_string())
      .with_style(Style::default().with(StyleDeclaration::display(Display::Inline))),
    atomic(
      Display::InlineFlex,
      Color([0, 255, 0, 100]),
      Color([20, 140, 20, 255]),
      "inline-flex",
    ),
    Node::text(" end ".to_string())
      .with_style(Style::default().with(StyleDeclaration::display(Display::Inline))),
    atomic(
      Display::InlineGrid,
      Color([0, 0, 255, 100]),
      Color([20, 20, 180, 255]),
      "inline-grid",
    ),
  ])
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::display(Display::Block))
      .with(StyleDeclaration::font_size(Px(24.0).into()))
      .with_border_width(Sides([Px(6.0).into(); 4]))
      .with_border_style(Sides([BorderStyle::Solid; 4]))
      .with_border_color(Sides([ColorInput::Value(Color([40, 40, 40, 255])); 4])),
  )])
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::align_items(AlignItems::Center))
      .with(StyleDeclaration::justify_content(JustifyContent::Center))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color::white(),
      )))
      .with_white_space(WhiteSpace::pre()),
  );

  let result = measure(node, create_measure_viewport());
  assert_eq!(result.children.len(), 1);

  let inline_container = &result.children[0];
  assert_eq!(inline_container.height, 70.0);
  assert_eq!(inline_container.children.len(), 3);

  for child in &inline_container.children {
    assert_eq!(child.transform[5], inline_container.transform[5]);
    assert_eq!(child.height, 58.0);
  }

  let runs = &inline_container.runs;
  assert_eq!(runs.len(), 3);
  assert_close(runs[0].y, 12.88);
  assert_close(runs[1].y, 12.88);
  assert_close(runs[2].y, 12.88);
}

#[test]
fn test_measure_text_node_centers_glyphs_with_explicit_line_height() {
  let node = Node::text("Line height 40px".to_string()).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::font_size(Px(24.0).into()))
      .with(StyleDeclaration::line_height(LineHeight::Length(Px(40.0)))),
  );

  let result = measure(node, create_measure_viewport());
  assert_eq!(result.children.len(), 1);

  let anonymous_item = &result.children[0];
  assert_eq!(anonymous_item.height, 40.0);
  assert_eq!(anonymous_item.runs.len(), 1);

  let run = &anonymous_item.runs[0];
  let leading_top = run.y;
  let leading_bottom = anonymous_item.height - (run.y + run.height);
  assert!(leading_bottom >= leading_top);
  assert!(
    (leading_bottom - leading_top).abs() <= 1.25,
    "expected browser-style half-leading split, top={leading_top}, bottom={leading_bottom}"
  );
}

#[test]
fn test_measure_text_node_respects_compact_line_height() {
  let node = Node::text("Compact line height".to_string()).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::font_size(Px(24.0).into()))
      .with(StyleDeclaration::line_height(LineHeight::Unitless(0.5))),
  );

  let result = measure(node, create_measure_viewport());
  assert_eq!(result.children.len(), 1);

  let anonymous_item = &result.children[0];
  assert_eq!(anonymous_item.height, 12.0);
  assert_eq!(anonymous_item.runs.len(), 1);

  let run = &anonymous_item.runs[0];
  assert!(
    run.y < 0.0,
    "expected glyphs to overflow above the compact line box"
  );
}

#[test]
fn test_measure_inline_layout_keeps_compact_text_line_height_with_small_inline_box() {
  let children: Vec<Node> = vec![
    Node::text("Compact".to_string())
      .with_style(Style::default().with(StyleDeclaration::display(Display::Inline))),
    Node::image("assets/images/yeecord.png").with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Inline))
        .with(StyleDeclaration::width(Px(8.0)))
        .with(StyleDeclaration::height(Px(8.0))),
    ),
    Node::text("line".to_string())
      .with_style(Style::default().with(StyleDeclaration::display(Display::Inline))),
  ];

  let node: Node = Node::container(children).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Block))
      .with(StyleDeclaration::width(Px(400.0)))
      .with(StyleDeclaration::font_size(Px(24.0).into()))
      .with(StyleDeclaration::line_height(LineHeight::Unitless(0.5))),
  );

  let result = measure(node, create_measure_viewport());
  assert_eq!(result.height, 14.0);
  assert_eq!(result.children.len(), 1);

  let inline_box = &result.children[0];
  assert_eq!(inline_box.width, 8.0);
  assert_eq!(inline_box.height, 8.0);
}

#[test]
fn test_measure_inline_image_uses_replaced_baseline_fallback() {
  let node: Node = Node::container([
    Node::text("Hello ".to_string())
      .with_style(Style::default().with(StyleDeclaration::display(Display::Inline))),
    Node::image("assets/images/yeecord.png").with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Inline))
        .with(StyleDeclaration::box_sizing(BoxSizing::ContentBox))
        .with(StyleDeclaration::width(Px(20.0)))
        .with(StyleDeclaration::height(Px(20.0)))
        .with_padding(Sides([Px(4.0); 4]))
        .with_border_width(Sides([Px(2.0).into(); 4]))
        .with_border_style(Sides([BorderStyle::Solid; 4])),
    ),
    Node::text("world".to_string())
      .with_style(Style::default().with(StyleDeclaration::display(Display::Inline))),
  ])
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Block))
      .with(StyleDeclaration::font_size(Px(20.0).into()))
      .with(StyleDeclaration::line_height(LineHeight::Unitless(1.0))),
  );

  let result = measure(node, create_measure_viewport());
  assert_eq!(result.children.len(), 1);

  let inline_image = &result.children[0];
  assert_eq!(inline_image.width, 32.0);
  assert_eq!(inline_image.height, 32.0);
  assert_within(inline_image.transform[5], 0.0, 0.1);

  assert_eq!(result.runs.len(), 2);
  assert_within(result.runs[0].y, 11.9, 1.0);
  assert_within(result.runs[1].y, 11.9, 1.0);
}

#[test]
fn test_measure_inline_image_respects_box_sizing_with_border() {
  let image_style = |box_sizing| {
    Style::default()
      .with(StyleDeclaration::display(Display::Inline))
      .with(StyleDeclaration::box_sizing(box_sizing))
      .with(StyleDeclaration::width(Px(20.0)))
      .with(StyleDeclaration::height(Px(20.0)))
      .with_border_width(Sides([Px(2.0).into(); 4]))
      .with_border_style(Sides([BorderStyle::Solid; 4]))
  };

  let build = |box_sizing| {
    Node::container([Node::image("assets/images/yeecord.png").with_style(image_style(box_sizing))])
      .with_style(Style::default().with(StyleDeclaration::display(Display::Block)))
  };

  let content_box = measure(build(BoxSizing::ContentBox), create_measure_viewport());
  let border_box = measure(build(BoxSizing::BorderBox), create_measure_viewport());

  assert_eq!(content_box.children.len(), 1);
  assert_eq!(border_box.children.len(), 1);

  let content_inline_image = &content_box.children[0];
  let border_inline_image = &border_box.children[0];

  assert_eq!(content_inline_image.width, 24.0);
  assert_eq!(content_inline_image.height, 24.0);
  assert_eq!(border_inline_image.width, 20.0);
  assert_eq!(border_inline_image.height, 20.0);
}

#[test]
fn test_measure_inline_image_border_box_single_axis_preserves_aspect_ratio() {
  let node: Node = Node::container([Node::image("assets/images/yeecord.png").with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Inline))
      .with(StyleDeclaration::box_sizing(BoxSizing::BorderBox))
      .with(StyleDeclaration::width(Px(48.0)))
      .with_padding_inline(SpacePair::from_single(Px(4.0))),
  )])
  .with_style(Style::default().with(StyleDeclaration::display(Display::Block)));

  let result = measure(node, create_measure_viewport());
  assert_eq!(result.children.len(), 1);

  let inline_image = &result.children[0];
  assert_eq!(inline_image.width, 48.0);
  assert_eq!(inline_image.height, 40.0);
}

#[test]
fn test_measure_text_node_keeps_first_line_when_height_is_smaller_than_line_height() {
  let node = Node::text("Visible text".to_string()).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Px(200.0)))
      .with(StyleDeclaration::height(Px(10.0)))
      .with(StyleDeclaration::font_size(Px(16.0).into()))
      .with(StyleDeclaration::line_height(Px(30.0).into())),
  );

  let result = measure(node, create_measure_viewport());
  let runs = measured_text_runs(&result);

  assert_eq!(runs.len(), 1);
  assert_eq!(runs[0].text, "Visible text");
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
fn test_measure_lh_resolves_against_explicit_line_height() {
  let viewport = create_measure_viewport();

  let result = measure(
    Node::container([Node::container([]).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::width(Px(40.0)))
        .with(StyleDeclaration::height(Lh(1.0))),
    )])
    .with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::line_height(LineHeight::Length(Px(60.0)))),
    ),
    viewport,
  );

  assert_eq!(result.children.len(), 1);
  assert_close(result.children[0].height, 60.0);
}

#[test]
fn test_measure_rlh_resolves_against_the_document_root_line_height() {
  fn measure_inner_height(root: Node) -> f32 {
    let result = measure(root, create_measure_viewport());

    assert_eq!(result.children.len(), 1);
    result.children[0].height
  }

  fn tree() -> Node {
    Node::container([Node::container([]).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::width(Px(40.0)))
        .with(StyleDeclaration::height(Rlh(1.0)))
        .with(StyleDeclaration::line_height(LineHeight::Length(Px(20.0)))),
    )])
    .with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::line_height(LineHeight::Length(Px(48.0)))),
    )
  }

  assert_close(measure_inner_height(tree()), 16.0);
  assert_close(measure_inner_height(tree().with_tag_name("html")), 48.0);
}

#[test]
fn test_measure_rem_resolves_against_the_document_root_font_size() {
  // CSS Values 4 §6.1: `rem` is the computed `font-size` of the root element.
  // A tree built in code is content rather than a document, so `rem` follows the
  // viewport; a tree parsed from a document is rooted at a real `<html>`.
  fn measure_inner_width(root: Node) -> f32 {
    let result = measure(root, create_measure_viewport());

    assert_eq!(result.children.len(), 1);
    result.children[0].width
  }

  fn tree() -> Node {
    Node::container([Node::container([]).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::width(Rem(1.0)))
        .with(StyleDeclaration::height(Rem(1.0))),
    )])
    .with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::font_size(Px(22.0).into())),
    )
  }

  assert_close(measure_inner_width(tree()), 16.0);
  assert_close(measure_inner_width(tree().with_tag_name("html")), 22.0);
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

  let result = takumi::measure(
    RenderOptions::builder()
      .viewport(create_measure_viewport())
      .node(node)
      .fonts(&CONTEXT)
      .images(TEST_IMAGES.clone())
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

  let result = takumi::measure(
    RenderOptions::builder()
      .viewport(create_measure_viewport())
      .node(node)
      .fonts(&CONTEXT)
      .images(TEST_IMAGES.clone())
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

  let result = takumi::measure(
    RenderOptions::builder()
      .viewport(create_measure_viewport())
      .node(node)
      .fonts(&CONTEXT)
      .images(TEST_IMAGES.clone())
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

    let result = takumi::measure(
      RenderOptions::builder()
        .viewport(create_measure_viewport())
        .node(node)
        .fonts(&CONTEXT)
        .images(TEST_IMAGES.clone())
        .build(),
    )
    .unwrap();

    assert_eq!(result.children.len(), 1);
    let image = &result.children[0];
    assert_eq!(image.width, expected_width, "case {} width", case_index);
    assert_eq!(image.height, expected_height, "case {} height", case_index);
  }
}

// https://github.com/kane50613/takumi/issues/695
#[test]
fn test_grid_container_drops_whitespace_only_text_children() {
  let row = || {
    Node::container([Node::text("row".to_string())
      .with_style(Style::default().with(StyleDeclaration::display(Display::Inline)))])
  };
  let grid_style = || {
    Style::default()
      .with(StyleDeclaration::display(Display::Grid))
      .with(StyleDeclaration::width(Px(200.0)))
  };

  let with_whitespace =
    Node::container([row(), Node::text("\n  \t".to_string()), row()]).with_style(grid_style());

  let without_whitespace = Node::container([row(), row()]).with_style(grid_style());

  let with_result = measure(with_whitespace, create_measure_viewport());
  let without_result = measure(without_whitespace, create_measure_viewport());

  assert_eq!(with_result.children.len(), 2);
  assert_eq!(without_result.children.len(), 2);
  assert_close(with_result.height, without_result.height);
}

// https://github.com/kane50613/takumi/issues/695
#[test]
fn test_flex_container_drops_whitespace_only_text_children() {
  let row = || {
    Node::container([Node::text("row".to_string())
      .with_style(Style::default().with(StyleDeclaration::display(Display::Inline)))])
  };
  let flex_style = || {
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Px(200.0)))
  };

  let with_whitespace =
    Node::container([row(), Node::text("\n  \t".to_string()), row()]).with_style(flex_style());

  let without_whitespace = Node::container([row(), row()]).with_style(flex_style());

  let with_result = measure(with_whitespace, create_measure_viewport());
  let without_result = measure(without_whitespace, create_measure_viewport());

  assert_eq!(with_result.children.len(), 2);
  assert_eq!(without_result.children.len(), 2);
  assert_close(with_result.height, without_result.height);
}

// https://github.com/kane50613/takumi/issues/711
#[test]
fn test_block_container_drops_whitespace_between_absolute_and_in_flow_sibling() {
  let absolute_child = || {
    Node::container([]).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Block))
        .with(StyleDeclaration::position(Position::Absolute))
        .with(StyleDeclaration::width(Px(40.0)))
        .with(StyleDeclaration::height(Px(40.0))),
    )
  };
  let in_flow_child = || {
    Node::container([]).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Block))
        .with(StyleDeclaration::width(Px(100.0)))
        .with(StyleDeclaration::height(Px(100.0))),
    )
  };
  let block_style = || {
    Style::default()
      .with(StyleDeclaration::display(Display::Block))
      .with(StyleDeclaration::position(Position::Relative))
      .with(StyleDeclaration::width(Px(200.0)))
  };

  let with_whitespace = Node::container([
    absolute_child(),
    Node::text("\n  \t".to_string()),
    in_flow_child(),
  ])
  .with_style(block_style());

  let without_whitespace =
    Node::container([absolute_child(), in_flow_child()]).with_style(block_style());

  let with_result = measure(with_whitespace, create_measure_viewport());
  let without_result = measure(without_whitespace, create_measure_viewport());

  assert_eq!(with_result, without_result);
}

// https://github.com/kane50613/takumi/issues/992
#[test]
fn test_block_container_drops_whitespace_between_absolute_only_siblings() {
  let absolute_child = || {
    Node::container([]).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Block))
        .with(StyleDeclaration::position(Position::Absolute))
        .with(StyleDeclaration::width(Px(40.0)))
        .with(StyleDeclaration::height(Px(40.0))),
    )
  };
  let block_style = || {
    Style::default()
      .with(StyleDeclaration::display(Display::Block))
      .with(StyleDeclaration::position(Position::Relative))
      .with(StyleDeclaration::width(Px(200.0)))
      .with(StyleDeclaration::height(Px(200.0)))
  };

  let with_whitespace = Node::container([
    Node::text("\n  ".to_string()),
    absolute_child(),
    Node::text("\n  ".to_string()),
    absolute_child(),
    Node::text("\n".to_string()),
  ])
  .with_style(block_style());

  let without_whitespace =
    Node::container([absolute_child(), absolute_child()]).with_style(block_style());

  let with_result = measure(with_whitespace, create_measure_viewport());
  let without_result = measure(without_whitespace, create_measure_viewport());

  assert_eq!(with_result.children.len(), 2);
  assert!(with_result.runs.is_empty());
  for child in &with_result.children {
    assert_close(child.width, 40.0);
    assert_close(child.height, 40.0);
  }
  assert_eq!(with_result, without_result);
}

#[test]
fn test_block_container_preserves_pre_whitespace_next_to_absolute_sibling() {
  let abs = || {
    Node::container([]).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Block))
        .with(StyleDeclaration::position(Position::Absolute))
        .with(StyleDeclaration::width(Px(40.0)))
        .with(StyleDeclaration::height(Px(40.0))),
    )
  };
  let parent = Node::container([Node::text("\n\n".to_string()), abs()]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Block))
      .with(StyleDeclaration::white_space_collapse(
        WhiteSpaceCollapse::Preserve,
      ))
      .with(StyleDeclaration::width(Px(200.0))),
  );

  let result = measure(parent, create_measure_viewport());

  assert!(
    result
      .children
      .iter()
      .any(|child| child.width == 40.0 && child.height == 40.0),
    "absolute child must stay in the layout"
  );
  assert!(
    result.height > 0.0,
    "preserved line breaks should keep their line boxes"
  );
}

#[test]
fn test_block_container_drops_whitespace_only_child() {
  let parent = Node::container([Node::text("\n  ".to_string())]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Block))
      .with(StyleDeclaration::width(Px(200.0))),
  );

  let result = measure(parent, create_measure_viewport());

  assert!(result.runs.is_empty());
  assert_close(result.height, 0.0);
}

// https://github.com/kane50613/takumi/issues/711
#[test]
fn test_block_container_preserves_whitespace_between_inline_siblings() {
  let inline_span = |label: &'static str| {
    Node::text(label.to_string())
      .with_style(Style::default().with(StyleDeclaration::display(Display::Inline)))
  };
  let block_child = || {
    Node::container([]).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Block))
        .with(StyleDeclaration::width(Px(50.0)))
        .with(StyleDeclaration::height(Px(50.0))),
    )
  };
  let block_style = || {
    Style::default()
      .with(StyleDeclaration::display(Display::Block))
      .with(StyleDeclaration::width(Px(400.0)))
  };

  let with_space = Node::container([
    inline_span("a"),
    Node::text(" ".to_string()),
    inline_span("b"),
    block_child(),
  ])
  .with_style(block_style());

  let without_space =
    Node::container([inline_span("a"), inline_span("b"), block_child()]).with_style(block_style());

  let with_result = measure(with_space, create_measure_viewport());
  let without_result = measure(without_space, create_measure_viewport());

  assert!(
    with_result.height > without_result.height
      || with_result.children[0] != without_result.children[0],
    "inline-interior whitespace should change layout (preserved space between siblings)"
  );
}

// https://github.com/kane50613/takumi/issues/992
#[test]
fn test_block_container_keeps_absolute_child_next_to_text() {
  let abs = || {
    Node::container([]).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Block))
        .with(StyleDeclaration::position(Position::Absolute))
        .with(StyleDeclaration::width(Px(40.0)))
        .with(StyleDeclaration::height(Px(40.0))),
    )
  };
  let parent = Node::container([Node::text("hi".to_string()), abs()]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Block))
      .with(StyleDeclaration::width(Px(200.0))),
  );

  let result = measure(parent, create_measure_viewport());

  assert!(
    result
      .children
      .iter()
      .any(|child| child.width == 40.0 && child.height == 40.0),
    "absolute child must stay in the layout"
  );
  assert!(
    result.height > 0.0,
    "text sibling must still produce a line box"
  );
}
