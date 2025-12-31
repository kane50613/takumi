use cssparser::{Parser, Token, match_ignore_ascii_case};

use crate::{
  layout::style::{Color, CssToken, FromCss, Gradient, ParseResult},
  rendering::RenderContext,
};

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
#[derive(Debug, Clone, PartialEq, Default)]
pub struct NoiseV1 {
  /// Random seed value that determines the unique noise pattern generated
  pub seed: Option<i32>,
  /// Controls the opacity of the noise pattern. 0.0 is fully transparent, 1.0 is fully opaque
  pub opacity: Option<f32>,
}

impl Gradient for NoiseV1 {
  type DrawContext = (i32, f32);

  fn at(&self, x: u32, y: u32, (seed, opacity): &Self::DrawContext) -> Color {
    let color = hash_2d(x, y, *seed as u32);
    let alpha = (color as f32 * opacity).clamp(0.0, 255.0) as u8;
    Color([color, color, color, alpha])
  }

  fn to_draw_context(
    &self,
    _width: f32,
    _height: f32,
    _context: &RenderContext,
  ) -> Self::DrawContext {
    (
      self.seed.unwrap_or(DEFAULT_SEED),
      self.opacity.unwrap_or(DEFAULT_OPACITY),
    )
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
