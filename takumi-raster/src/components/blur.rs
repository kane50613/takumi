use takumi_core::blur::{blur_alpha, blur_rgba};

use crate::{Error, Result, checked_area};

pub(crate) use takumi_core::style::BlurType;

/// Blurs a `width` by `height` alpha mask in place by a CSS blur `radius` of `blur_type`.
pub(crate) fn apply_blur_alpha_bytes(
  data: &mut [u8],
  width: u32,
  height: u32,
  radius: f32,
  blur_type: BlurType,
) -> Result<()> {
  let Some(expected) = checked_area(width, height, 1) else {
    return Ok(());
  };

  if data.len() != expected {
    return Err(Error::InvalidAlphaBufferLength {
      actual: data.len(),
      expected,
    });
  }
  blur_alpha(
    data,
    width as usize,
    height as usize,
    blur_type.to_sigma(radius),
  );
  Ok(())
}

/// Blurs a `width` by `height` premultiplied RGBA image in place by a CSS blur `radius` of
/// `blur_type`.
pub(crate) fn apply_blur_rgba_bytes(
  data: &mut [u8],
  width: u32,
  height: u32,
  radius: f32,
  blur_type: BlurType,
) -> Result<()> {
  let Some(expected) = checked_area(width, height, 4) else {
    return Ok(());
  };

  if data.len() != expected {
    return Err(Error::InvalidRgbaBufferLength {
      actual: data.len(),
      expected,
    });
  }
  blur_rgba(
    data.as_chunks_mut::<4>().0,
    width as usize,
    height as usize,
    blur_type.to_sigma(radius),
  );
  Ok(())
}

#[cfg(test)]
mod tests {
  use std::assert_matches;

  use super::{BlurType, apply_blur_rgba_bytes};
  use crate::Error;

  #[test]
  fn apply_blur_rgba_bytes_returns_error_for_invalid_buffer_length() {
    let mut data = vec![0u8; 3];

    assert_matches!(
      apply_blur_rgba_bytes(&mut data, 1, 1, 4.0, BlurType::Filter),
      Err(Error::InvalidRgbaBufferLength {
        actual: 3,
        expected: 4
      })
    );
  }
}
