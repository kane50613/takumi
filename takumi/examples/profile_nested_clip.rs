use std::hint::black_box;
use takumi::base::{
  FontContext,
  layout::{
    Viewport,
    node::Node,
    style::{
      AlignItems, BorderRadius, Color, ColorInput, Display, FromCss, JustifyContent,
      Length::Percentage, Overflow, SpacePair, Style, StyleDeclaration,
    },
  },
};
use takumi::raster::{RenderOptions, render};

const ITERS: usize = 100;

fn nested_clip_masks_fixture() -> Node {
  let leaf = |radius: &str| {
    Node::container([]).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::height(Percentage(100.0)))
        .with_border_radius(BorderRadius::from_str(radius).unwrap())
        .with_overflow(SpacePair::from_single(Overflow::Clip))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([200, 30, 30, 255]),
        ))),
    )
  };

  let mut current = leaf("12px");
  for radius in ["24px", "32px", "40px", "48px", "56px"] {
    current = Node::container([current]).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::width(Percentage(95.0)))
        .with(StyleDeclaration::height(Percentage(95.0)))
        .with_border_radius(BorderRadius::from_str(radius).unwrap())
        .with_overflow(SpacePair::from_single(Overflow::Clip))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color([30, 30, 200, 255]),
        ))),
    );
  }

  Node::container([current]).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::align_items(AlignItems::Center))
      .with(StyleDeclaration::justify_content(JustifyContent::Center))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color::white(),
      ))),
  )
}

fn main() {
  let font_context = FontContext::default();
  for _ in 0..ITERS {
    let options = RenderOptions::builder()
      .viewport(Viewport::new((800, 800)))
      .node(nested_clip_masks_fixture())
      .font_context(&font_context)
      .build();
    let image = render(options).unwrap();
    black_box(image);
  }
}
