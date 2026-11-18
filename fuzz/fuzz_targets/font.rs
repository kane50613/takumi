//! Registers arbitrary bytes as a font and draws text with it.
#![no_main]

use libfuzzer_sys::fuzz_target;
use takumi::prelude::*;

fuzz_target!(|data: &[u8]| {
  let mut fonts = Fonts::default();
  let Ok(families) = fonts.register(FontResource::new(data.to_vec())) else {
    return;
  };
  let Some(family) = families.first() else {
    return;
  };
  let node = Node::text(format!("Fuzz 字 é ﬁ {}", family.name)).with_style(
    Style::default().with(StyleDeclaration::font_family(FontFamily::from_names([
      family.name.to_string(),
    ]))),
  );
  let options = RenderOptions::builder()
    .viewport(Viewport::new((96, 64)))
    .node(node)
    .fonts(&fonts)
    .build();

  let _ = takumi::render(options);
});
