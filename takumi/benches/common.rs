#![allow(
  dead_code,
  reason = "each bench binary compiles this module and uses a subset"
)]

use std::{collections::HashMap, sync::Arc, time::Duration};

use criterion::Criterion;
use takumi::prelude::{FontOverride, FontResource, Fonts, GenericFamily, ImageSource};

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
