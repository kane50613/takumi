use takumi::{prelude::*, render};

#[test]
fn transparent_drop_shadow_does_not_panic() {
  let node = Node::container([]).with_style(
    Style::default()
      .with(StyleDeclaration::width(Length::Px(16.0)))
      .with(StyleDeclaration::height(Length::Px(16.0)))
      .with(StyleDeclaration::filter(
        Filters::from_css_str("drop-shadow(1px 1px 1px black)").unwrap(),
      )),
  );
  let fonts = Fonts::default();
  let options = RenderOptions::builder()
    .viewport(Viewport::new((16, 16)))
    .node(node)
    .fonts(&fonts)
    .build();

  render(options).unwrap();
}
