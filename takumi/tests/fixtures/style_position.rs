use takumi::prelude::{Length::*, *};

use crate::test_utils::run_fixture_test;

#[test]
fn test_style_position() {
  let container = Node::container([Node::container([]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Px(100.0)))
      .with(StyleDeclaration::height(Px(100.0)))
      .with(StyleDeclaration::position(Position::Absolute))
      .with_inset(Sides([Px(20.0); 4]))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([255, 0, 0, 255]),
      ))),
  )])
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([0, 0, 255, 255]),
      ))),
  );

  run_fixture_test(container, "style_position");
}

#[test]
fn test_style_stacking_context_z_index_siblings() {
  let negative = Node::container([]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::position(Position::Absolute))
      .with(StyleDeclaration::z_index(ZIndex::Integer(-1)))
      .with(StyleDeclaration::width(Px(360.0)))
      .with(StyleDeclaration::height(Px(360.0)))
      .with(StyleDeclaration::top(Px(120.0)))
      .with(StyleDeclaration::left(Px(120.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([255, 0, 0, 255]),
      ))),
  );

  let positive = Node::container([]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::position(Position::Absolute))
      .with(StyleDeclaration::z_index(ZIndex::Integer(2)))
      .with(StyleDeclaration::width(Px(360.0)))
      .with(StyleDeclaration::height(Px(360.0)))
      .with(StyleDeclaration::top(Px(180.0)))
      .with(StyleDeclaration::left(Px(180.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([0, 255, 0, 255]),
      ))),
  );

  let auto = Node::container([]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::position(Position::Absolute))
      .with(StyleDeclaration::width(Px(360.0)))
      .with(StyleDeclaration::height(Px(360.0)))
      .with(StyleDeclaration::top(Px(240.0)))
      .with(StyleDeclaration::left(Px(240.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([0, 0, 255, 255]),
      ))),
  );

  let container = Node::container([negative, positive, auto]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([245, 245, 245, 255]),
      ))),
  );

  run_fixture_test(container, "style_stacking_context_z_index_siblings");
}

#[test]
fn test_style_stacking_context_nested_context_atomicity() {
  let nested_high = Node::container([]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::position(Position::Absolute))
      .with(StyleDeclaration::z_index(ZIndex::Integer(9999)))
      .with(StyleDeclaration::width(Px(220.0)))
      .with(StyleDeclaration::height(Px(220.0)))
      .with(StyleDeclaration::top(Px(30.0)))
      .with(StyleDeclaration::left(Px(260.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([255, 255, 0, 255]),
      ))),
  );

  let context_low = Node::container([nested_high]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::position(Position::Absolute))
      .with(StyleDeclaration::z_index(ZIndex::Integer(1)))
      .with(StyleDeclaration::width(Px(460.0)))
      .with(StyleDeclaration::height(Px(300.0)))
      .with(StyleDeclaration::top(Px(130.0)))
      .with(StyleDeclaration::left(Px(160.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([255, 128, 128, 255]),
      ))),
  );

  let context_high = Node::container([]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::position(Position::Absolute))
      .with(StyleDeclaration::z_index(ZIndex::Integer(2)))
      .with(StyleDeclaration::width(Px(340.0)))
      .with(StyleDeclaration::height(Px(340.0)))
      .with(StyleDeclaration::top(Px(200.0)))
      .with(StyleDeclaration::left(Px(200.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([128, 128, 255, 255]),
      ))),
  );

  let container = Node::container([context_low, context_high]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([255, 255, 255, 255]),
      ))),
  );

  run_fixture_test(container, "style_stacking_context_nested_context_atomicity");
}

#[test]
fn test_style_stacking_context_flex_item_z_index() {
  let flex_a = Node::container([]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::z_index(ZIndex::Integer(2)))
      .with(StyleDeclaration::width(Px(360.0)))
      .with(StyleDeclaration::height(Px(260.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([255, 0, 0, 200]),
      ))),
  );

  let flex_b = Node::container([]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::z_index(ZIndex::Integer(1)))
      .with(StyleDeclaration::width(Px(360.0)))
      .with(StyleDeclaration::height(Px(260.0)))
      .with(StyleDeclaration::margin_left(Px(-120.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([0, 0, 255, 200]),
      ))),
  );

  let container = Node::container([flex_a, flex_b]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([245, 245, 245, 255]),
      ))),
  );

  run_fixture_test(container, "style_stacking_context_flex_item_z_index");
}

#[test]
fn test_style_absolute_in_block_relative_with_sibling() {
  let absolute = Node::container([]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::position(Position::Absolute))
      .with(StyleDeclaration::width(Px(100.0)))
      .with(StyleDeclaration::height(Px(20.0)))
      .with(StyleDeclaration::bottom(Px(0.0)))
      .with(StyleDeclaration::left(Px(0.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([128, 128, 255, 255]),
      ))),
  );

  let sibling = Node::container([Node::text("ERR".to_string())]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::color(ColorInput::Value(Color::white())))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color::black(),
      ))),
  );

  let container = Node::container([absolute, sibling]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Block))
      .with(StyleDeclaration::position(Position::Relative))
      .with(StyleDeclaration::width(Px(300.0)))
      .with(StyleDeclaration::height(Px(100.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([0, 0, 245, 255]),
      ))),
  );

  let root = Node::container([container]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::align_items(AlignItems::Center))
      .with(StyleDeclaration::justify_content(JustifyContent::Center))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color::white(),
      ))),
  );

  run_fixture_test(root, "style_absolute_in_block_relative_with_sibling");
}

// An absolute child resolves against the nearest *positioned* ancestor,
// skipping a `position: static` ancestor in between. The green box should sit
// at the bottom-right of the blue 300x300 relative container, not of the
// yellow 120x120 static middle.
#[test]
fn test_style_absolute_skips_static_ancestor() {
  let absolute = Node::container([]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::position(Position::Absolute))
      .with(StyleDeclaration::width(Px(80.0)))
      .with(StyleDeclaration::height(Px(80.0)))
      .with(StyleDeclaration::bottom(Px(0.0)))
      .with(StyleDeclaration::right(Px(0.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([0, 200, 0, 255]),
      ))),
  );
  let middle = Node::container([absolute]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Block))
      .with(StyleDeclaration::position(Position::Static))
      .with(StyleDeclaration::width(Px(120.0)))
      .with(StyleDeclaration::height(Px(120.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([240, 220, 0, 255]),
      ))),
  );
  let relative = Node::container([middle]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Block))
      .with(StyleDeclaration::position(Position::Relative))
      .with(StyleDeclaration::width(Px(300.0)))
      .with(StyleDeclaration::height(Px(300.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([0, 0, 200, 255]),
      ))),
  );
  let root = Node::container([relative]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::align_items(AlignItems::Center))
      .with(StyleDeclaration::justify_content(JustifyContent::Center))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color::white(),
      ))),
  );
  run_fixture_test(root, "style_absolute_skips_static_ancestor");
}

// A fixed box resolves against the viewport (root), ignoring positioned
// ancestors. The purple box should pin to the top-left of the canvas at
// (10, 10) regardless of the offset relative ancestor it lives inside.
#[test]
fn test_style_fixed_anchors_to_viewport() {
  let fixed = Node::container([]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::position(Position::Fixed))
      .with(StyleDeclaration::width(Px(100.0)))
      .with(StyleDeclaration::height(Px(100.0)))
      .with(StyleDeclaration::top(Px(10.0)))
      .with(StyleDeclaration::left(Px(10.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([160, 0, 200, 255]),
      ))),
  );
  let inner = Node::container([fixed]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Block))
      .with(StyleDeclaration::position(Position::Relative))
      .with(StyleDeclaration::width(Px(200.0)))
      .with(StyleDeclaration::height(Px(200.0)))
      .with(StyleDeclaration::margin_top(Px(160.0)))
      .with(StyleDeclaration::margin_left(Px(160.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([220, 220, 220, 255]),
      ))),
  );
  let root = Node::container([inner]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Block))
      .with(StyleDeclaration::position(Position::Relative))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color::white(),
      ))),
  );
  run_fixture_test(root, "style_fixed_anchors_to_viewport");
}

// A hoisted absolute participates correctly in its containing block's stacking
// order. The red abspos (z-index 1, hoisted out of the static wrapper) must
// paint *below* the green abspos (z-index 2) where they overlap.
#[test]
fn test_style_absolute_paint_order_under_z_sibling() {
  let red = Node::container([]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::position(Position::Absolute))
      .with(StyleDeclaration::z_index(ZIndex::Integer(1)))
      .with(StyleDeclaration::width(Px(160.0)))
      .with(StyleDeclaration::height(Px(160.0)))
      .with(StyleDeclaration::top(Px(40.0)))
      .with(StyleDeclaration::left(Px(40.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([220, 0, 0, 255]),
      ))),
  );
  let static_wrap = Node::container([red]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Block))
      .with(StyleDeclaration::position(Position::Static)),
  );
  let green = Node::container([]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::position(Position::Absolute))
      .with(StyleDeclaration::z_index(ZIndex::Integer(2)))
      .with(StyleDeclaration::width(Px(160.0)))
      .with(StyleDeclaration::height(Px(160.0)))
      .with(StyleDeclaration::top(Px(100.0)))
      .with(StyleDeclaration::left(Px(100.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([0, 180, 0, 255]),
      ))),
  );
  let container = Node::container([static_wrap, green]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Block))
      .with(StyleDeclaration::position(Position::Relative))
      .with(StyleDeclaration::width(Px(300.0)))
      .with(StyleDeclaration::height(Px(300.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([230, 230, 230, 255]),
      ))),
  );
  let root = Node::container([container]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::align_items(AlignItems::Center))
      .with(StyleDeclaration::justify_content(JustifyContent::Center))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color::white(),
      ))),
  );
  run_fixture_test(root, "style_absolute_paint_order_under_z_sibling");
}

// Percentage size and insets on an absolute child resolve against its
// containing block. Inside a 200x200 relative (via a static wrapper), the teal
// box should be 100x100 (50%) offset by 20px (10%) from the top-left.
#[test]
fn test_style_absolute_percentage_resolves_against_cb() {
  let absolute = Node::container([]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::position(Position::Absolute))
      .with(StyleDeclaration::width(Percentage(50.0)))
      .with(StyleDeclaration::height(Percentage(50.0)))
      .with(StyleDeclaration::top(Percentage(10.0)))
      .with(StyleDeclaration::left(Percentage(10.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([0, 170, 170, 255]),
      ))),
  );
  let static_wrap = Node::container([absolute]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Block))
      .with(StyleDeclaration::position(Position::Static))
      .with(StyleDeclaration::width(Px(80.0)))
      .with(StyleDeclaration::height(Px(80.0))),
  );
  let relative = Node::container([static_wrap]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Block))
      .with(StyleDeclaration::position(Position::Relative))
      .with(StyleDeclaration::width(Px(200.0)))
      .with(StyleDeclaration::height(Px(200.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([40, 40, 40, 255]),
      ))),
  );
  let root = Node::container([relative]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::align_items(AlignItems::Center))
      .with(StyleDeclaration::justify_content(JustifyContent::Center))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color::white(),
      ))),
  );
  run_fixture_test(root, "style_absolute_percentage_resolves_against_cb");
}

// z-index applies only to positioned elements. A `static` element's z-index is
// ignored, so the later in-flow positioned green box paints on top of the red
// static box despite red's z-index: 99.
#[test]
fn test_style_static_z_index_ignored() {
  let red_static = Node::container([]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::position(Position::Static))
      .with(StyleDeclaration::z_index(ZIndex::Integer(99)))
      .with(StyleDeclaration::width(Px(160.0)))
      .with(StyleDeclaration::height(Px(160.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([220, 0, 0, 255]),
      ))),
  );
  let green_relative = Node::container([]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::position(Position::Relative))
      .with(StyleDeclaration::width(Px(160.0)))
      .with(StyleDeclaration::height(Px(160.0)))
      .with(StyleDeclaration::margin_top(Px(-80.0)))
      .with(StyleDeclaration::margin_left(Px(80.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([0, 180, 0, 255]),
      ))),
  );
  let container = Node::container([red_static, green_relative]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Block))
      .with(StyleDeclaration::position(Position::Relative))
      .with(StyleDeclaration::width(Px(300.0)))
      .with(StyleDeclaration::height(Px(300.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([230, 230, 230, 255]),
      ))),
  );
  let root = Node::container([container]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::align_items(AlignItems::Center))
      .with(StyleDeclaration::justify_content(JustifyContent::Center))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color::white(),
      ))),
  );
  run_fixture_test(root, "style_static_z_index_ignored");
}

// An absolute whose containing block is itself a hoisted `fixed` node. The
// purple fixed box pins to the viewport at (40, 40); the orange absolute,
// hoisted past the static wrapper, sits at the bottom-right of the purple box.
#[test]
fn test_style_nested_hoisting_fixed_cb() {
  let orange = Node::container([]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::position(Position::Absolute))
      .with(StyleDeclaration::width(Px(60.0)))
      .with(StyleDeclaration::height(Px(60.0)))
      .with(StyleDeclaration::bottom(Px(0.0)))
      .with(StyleDeclaration::right(Px(0.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([240, 140, 0, 255]),
      ))),
  );
  let static_wrap = Node::container([orange]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Block))
      .with(StyleDeclaration::position(Position::Static)),
  );
  let fixed = Node::container([static_wrap]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Block))
      .with(StyleDeclaration::position(Position::Fixed))
      .with(StyleDeclaration::width(Px(200.0)))
      .with(StyleDeclaration::height(Px(200.0)))
      .with(StyleDeclaration::top(Px(40.0)))
      .with(StyleDeclaration::left(Px(40.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([160, 0, 200, 255]),
      ))),
  );
  let root = Node::container([fixed]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Block))
      .with(StyleDeclaration::position(Position::Relative))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color::white(),
      ))),
  );
  run_fixture_test(root, "style_nested_hoisting_fixed_cb");
}

// A transform makes a box the containing block for its `fixed` descendants,
// even when the box is not positioned. The purple square resolves against the
// translated grey box, so it lands at (60, 60) rather than the canvas corner.
#[test]
fn test_style_fixed_captured_by_transform() {
  let fixed = Node::container([]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::position(Position::Fixed))
      .with(StyleDeclaration::width(Px(80.0)))
      .with(StyleDeclaration::height(Px(80.0)))
      .with(StyleDeclaration::top(Px(10.0)))
      .with(StyleDeclaration::left(Px(10.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([160, 0, 200, 255]),
      ))),
  );
  let transformed = Node::container([fixed]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Block))
      .with(StyleDeclaration::width(Px(200.0)))
      .with(StyleDeclaration::height(Px(200.0)))
      .with(StyleDeclaration::transform(Some(
        Transforms::from_css_str("translate(50px, 50px)").unwrap(),
      )))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([220, 220, 220, 255]),
      ))),
  );
  let root = Node::container([transformed]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Block))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color::white(),
      ))),
  );
  run_fixture_test(root, "style_fixed_captured_by_transform");
}

// The same rule reaches `absolute`: a filtered static ancestor is its
// containing block, so the red square resolves against the filtered box rather
// than the relative one wrapping it.
#[test]
fn test_style_absolute_captured_by_filter() {
  let absolute = Node::container([]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::position(Position::Absolute))
      .with(StyleDeclaration::width(Px(80.0)))
      .with(StyleDeclaration::height(Px(80.0)))
      .with(StyleDeclaration::top(Px(10.0)))
      .with(StyleDeclaration::left(Px(10.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([220, 0, 0, 255]),
      ))),
  );
  let filtered = Node::container([absolute]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Block))
      .with(StyleDeclaration::width(Px(200.0)))
      .with(StyleDeclaration::height(Px(200.0)))
      .with(StyleDeclaration::margin_top(Px(60.0)))
      .with(StyleDeclaration::margin_left(Px(60.0)))
      .with(StyleDeclaration::filter(
        Filters::from_css_str("grayscale(1)").unwrap(),
      ))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([220, 220, 220, 255]),
      ))),
  );
  let root = Node::container([filtered]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Block))
      .with(StyleDeclaration::position(Position::Relative))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color::white(),
      ))),
  );
  run_fixture_test(root, "style_absolute_captured_by_filter");
}
