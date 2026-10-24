//! SVG lengths and their conversion to user units, after Blink's `SVGLength` and `SVGLengthContext`.

/// The unit of an [`Length`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LengthUnit {
  /// A bare number, in user units.
  None,
  /// `px`
  Px,
  /// `em`
  Em,
  /// `ex`
  Ex,
  /// `in`
  In,
  /// `cm`
  Cm,
  /// `mm`
  Mm,
  /// `pt`
  Pt,
  /// `pc`
  Pc,
  /// `%`
  Percent,
}

/// A number with a unit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Length {
  /// The number as written.
  pub number: f32,
  /// The unit as written.
  pub unit: LengthUnit,
}

/// The axis a percentage resolves against, Blink's `SVGLengthMode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LengthMode {
  /// The viewport width.
  Width,
  /// The viewport height.
  Height,
  /// The normalized diagonal of the viewport.
  Other,
}

/// What a length resolves against, Blink's `SVGLengthContext`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LengthContext {
  /// The font size in user units, which `em` and `ex` scale.
  pub font_size: f32,
  /// The viewport width in user units.
  pub width: f32,
  /// The viewport height in user units.
  pub height: f32,
  /// The pixels per inch that absolute units convert with.
  pub dpi: f32,
}

impl Length {
  /// A bare number.
  pub const fn number(number: f32) -> Self {
    Self {
      number,
      unit: LengthUnit::None,
    }
  }

  /// A percentage.
  pub const fn percent(number: f32) -> Self {
    Self {
      number,
      unit: LengthUnit::Percent,
    }
  }

  /// Blink's `SVGLength::ConvertValueFromUnitsToUserUnits`, with a percentage taken against the
  /// viewport on the axis of `mode`.
  pub fn to_user_units(self, mode: LengthMode, context: &LengthContext) -> f32 {
    match self.unit {
      LengthUnit::Percent => {
        let base = match mode {
          LengthMode::Width => context.width,
          LengthMode::Height => context.height,
          LengthMode::Other => ((context.width.powi(2) + context.height.powi(2)) / 2.0).sqrt(),
        };

        base * self.number / 100.0
      }
      _ => self.to_absolute(context),
    }
  }

  /// The length in user units, with a percentage as the fraction it names.
  pub fn to_fraction_or_user_units(self, context: &LengthContext) -> f32 {
    match self.unit {
      LengthUnit::Percent => self.number / 100.0,
      _ => self.to_absolute(context),
    }
  }

  fn to_absolute(self, context: &LengthContext) -> f32 {
    let number = self.number;

    match self.unit {
      LengthUnit::None | LengthUnit::Px => number,
      LengthUnit::Em => number * context.font_size,
      LengthUnit::Ex => number * context.font_size / 2.0,
      LengthUnit::In => number * context.dpi,
      LengthUnit::Cm => number * context.dpi / 2.54,
      LengthUnit::Mm => number * context.dpi / 25.4,
      LengthUnit::Pt => number * context.dpi / 72.0,
      LengthUnit::Pc => number * context.dpi / 6.0,
      LengthUnit::Percent => number,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  const CONTEXT: LengthContext = LengthContext {
    font_size: 10.0,
    width: 200.0,
    height: 100.0,
    dpi: 96.0,
  };

  #[test]
  fn absolute_units_use_the_dpi_and_font_size() {
    let inch = Length {
      number: 1.0,
      unit: LengthUnit::In,
    };
    let ex = Length {
      number: 2.0,
      unit: LengthUnit::Ex,
    };

    assert_eq!(inch.to_user_units(LengthMode::Width, &CONTEXT), 96.0);
    assert_eq!(ex.to_user_units(LengthMode::Width, &CONTEXT), 10.0);
  }

  #[test]
  fn percentages_follow_the_axis() {
    let half = Length::percent(50.0);

    assert_eq!(half.to_user_units(LengthMode::Width, &CONTEXT), 100.0);
    assert_eq!(half.to_user_units(LengthMode::Height, &CONTEXT), 50.0);
    assert_eq!(
      half.to_user_units(LengthMode::Other, &CONTEXT),
      ((200.0f32.powi(2) + 100.0f32.powi(2)) / 2.0).sqrt() * 50.0 / 100.0
    );
    assert_eq!(half.to_fraction_or_user_units(&CONTEXT), 0.5);
  }
}
