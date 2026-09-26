#![allow(
  dead_code,
  reason = "each bench binary compiles this module and uses a subset"
)]

use std::{collections::HashMap, sync::Arc, time::Duration};

use criterion::Criterion;
use takumi::prelude::{
  AlignItems, BackgroundClip, BackgroundImages, BackgroundRepeats, BackgroundSizes, Color,
  ColorInput, Display, FontOverride, FontResource, FontWeight, Fonts, FromCssStr, GenericFamily,
  ImageSource, JustifyContent,
  Length::{Percentage, Px},
  Node, PositionValues, Style, StyleDeclaration,
};

/// The `src` the fixtures use for their bitmap; `images()` resolves it.
pub const IMAGE_SRC: &str = "yeecord.png";

pub fn criterion() -> Criterion {
  Criterion::default()
    .warm_up_time(Duration::from_millis(500))
    .measurement_time(Duration::from_secs(2))
    .sample_size(20)
}

/// Geist for text and Twemoji for emoji, since `Fonts::default()` carries no fonts.
pub fn fonts() -> Fonts {
  let mut fonts = Fonts::default();
  let regular: &[u8] = include_bytes!("../../assets/fonts/geist/Geist[wght].woff2");
  fonts
    .register(
      FontResource::new(regular.to_vec())
        .override_info(FontOverride {
          family_name: Some("Geist".into()),
          ..Default::default()
        })
        .generic_family(GenericFamily::SANS_SERIF),
    )
    .unwrap();
  let emoji: &[u8] = include_bytes!("../../assets/fonts/twemoji/TwemojiMozilla-colr.woff2");
  fonts
    .register(
      FontResource::new(emoji.to_vec())
        .override_info(FontOverride {
          family_name: Some("Twemoji Mozilla".into()),
          ..Default::default()
        })
        .generic_family(GenericFamily::EMOJI),
    )
    .unwrap();
  fonts
}

/// The decoded bitmap behind [`IMAGE_SRC`]; a bare path is not resolved at render time.
pub fn images() -> HashMap<Arc<str>, ImageSource> {
  let bytes: &[u8] = include_bytes!("../../assets/images/yeecord.png");
  HashMap::from([(
    Arc::from(IMAGE_SRC),
    ImageSource::from_bytes(bytes).unwrap(),
  )])
}

pub fn gradient_clip_text_fixture() -> Node {
  let gradient = BackgroundImages::from_css_str(
    "linear-gradient(90deg, #ff3b30, #ffcc00, #34c759, #007aff, #5856d6)",
  )
  .unwrap();

  Node::container([
    Node::text("Gradient Text Benchmark".to_string()).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::background_image(Some(gradient)))
        .with(StyleDeclaration::background_size(
          BackgroundSizes::from_css_str("100% 100%").unwrap(),
        ))
        .with(StyleDeclaration::background_position(
          PositionValues::from_css_str("0 0").unwrap(),
        ))
        .with(StyleDeclaration::background_repeat(
          BackgroundRepeats::from_css_str("no-repeat").unwrap(),
        ))
        .with(StyleDeclaration::background_clip(BackgroundClip::Text))
        .with(StyleDeclaration::color(ColorInput::Value(
          Color::transparent(),
        ))),
    ),
  ])
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([242, 242, 242, 255]),
      )))
      .with(StyleDeclaration::font_size(Px(72.0).into()))
      .with(StyleDeclaration::font_weight(FontWeight::from(800.0)))
      .with(StyleDeclaration::align_items(AlignItems::Center))
      .with(StyleDeclaration::justify_content(JustifyContent::Center)),
  )
}
