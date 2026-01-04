use cssparser::{Parser, Token, match_ignore_ascii_case};
use image::{GenericImageView, Rgba};

use crate::layout::style::{CssToken, FromCss, ParseResult};

const DEFAULT_OPACITY: f32 = 0.15;
const DEFAULT_SEED: i32 = 0;

#[inline]
fn hash_2d(x: u32, y: u32, seed: u32) -> u8 {
  let mut h = seed.wrapping_add(x.wrapping_mul(374761393));
  h ^= h >> 13;
  h = h.wrapping_mul(1274126177);
  h ^= h >> 16;
  h = h.wrapping_add(y.wrapping_mul(668265263));
  h ^= h >> 13;
  h = h.wrapping_mul(1274126177);
  h ^= h >> 16;
  (h & 0xFF) as u8
}

/// Procedural noise gradient that generates organic, natural-looking patterns using fractal Brownian motion.
/// This creates dynamic textures that can be used as backgrounds or overlays with customizable parameters
/// for controlling the noise characteristics and visual appearance.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct NoiseV1 {
  /// Random seed value that determines the unique noise pattern generated
  pub seed: Option<i32>,
  /// Controls the opacity of the noise pattern. 0.0 is fully transparent, 1.0 is fully opaque
  pub opacity: Option<f32>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct NoiseV1Tile {
  pub width: u32,
  pub height: u32,
  pub seed: i32,
  pub opacity: f32,
}

impl NoiseV1Tile {
  pub fn new(noise: NoiseV1, width: u32, height: u32) -> Self {
    Self {
      width,
      height,
      seed: noise.seed.unwrap_or(DEFAULT_SEED),
      opacity: noise.opacity.unwrap_or(DEFAULT_OPACITY),
    }
  }
}

impl GenericImageView for NoiseV1Tile {
  type Pixel = Rgba<u8>;

  fn dimensions(&self) -> (u32, u32) {
    (self.width, self.height)
  }

  fn get_pixel(&self, x: u32, y: u32) -> Self::Pixel {
    let color = hash_2d(x, y, self.seed as u32);
    let alpha = (color as f32 * self.opacity).clamp(0.0, 255.0) as u8;

    Rgba([color, color, color, alpha])
  }
}

impl<'i> FromCss<'i> for NoiseV1 {
  /// Example: noise-v1(seed(42) opacity(0.5))
  /// Syntax: noise-v1([<seed>] | [<opacity>])
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, NoiseV1> {
    input.expect_function_matching("noise-v1")?;

    input.parse_nested_block(|input| {
      let mut instance = NoiseV1::default();

      while !input.is_exhausted() {
        let location = input.current_source_location();
        let token = input.next()?;

        let Token::Function(key) = token else {
          return Err(
            location
              .new_basic_unexpected_token_error(token.clone())
              .into(),
          );
        };

        match_ignore_ascii_case! {key,
          "seed" => instance.seed = Some(input.parse_nested_block(|input| Ok(input.expect_integer()?))?),
          "opacity" => instance.opacity = Some(input.parse_nested_block(|input| Ok(input.expect_number()?))?),
          _ => return Err(Self::unexpected_token_error(location, token)),
        }
      }

      Ok(instance)
    })
  }

  fn valid_tokens() -> &'static [CssToken] {
    &[CssToken::Token("seed()"), CssToken::Token("opacity()")]
  }
}
