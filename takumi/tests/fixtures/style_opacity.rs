use takumi::{prelude::*, render};

use crate::test_utils::CONTEXT;

fn darkest_pixel_in_range(image: &Bitmap, x_range: std::ops::Range<u32>) -> u8 {
  let width = image.width();

  image
    .as_raw()
    .as_chunks::<4>()
    .0
    .iter()
    .enumerate()
    .filter_map(|(index, pixel)| {
      let x = index as u32 % width;
      (x_range.contains(&x) && pixel[3] > 0).then(|| pixel[0].min(pixel[1]).min(pixel[2]))
    })
    .min()
    .unwrap_or(255)
}

#[test]
fn test_inline_text_span_opacity() {
  let root = Node::container([
    Node::text("H".to_string()).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Inline))
        .with(StyleDeclaration::opacity(PercentageNumber(0.5))),
    ),
    Node::container([]).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::InlineBlock))
        .with(StyleDeclaration::width(Length::Px(80.0)))
        .with(StyleDeclaration::height(Length::Px(1.0))),
    ),
    Node::text("H".to_string())
      .with_style(Style::default().with(StyleDeclaration::display(Display::Inline))),
  ])
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Block))
      .with(StyleDeclaration::width(Length::Px(320.0)))
      .with(StyleDeclaration::height(Length::Px(120.0)))
      .with(StyleDeclaration::font_size(Length::Px(96.0).into()))
      .with(StyleDeclaration::font_weight(FontWeight::from(700.0)))
      .with(StyleDeclaration::color(ColorInput::Value(Color::black())))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color::white(),
      ))),
  );

  let image = render(
    RenderOptions::builder()
      .viewport(Viewport::new((320, 120)))
      .node(root)
      .fonts(&CONTEXT)
      .build(),
  )
  .unwrap();

  let translucent_text = darkest_pixel_in_range(&image, 0..80);
  let opaque_text = darkest_pixel_in_range(&image, 140..240);

  assert!(translucent_text > opaque_text.saturating_add(48));
}
