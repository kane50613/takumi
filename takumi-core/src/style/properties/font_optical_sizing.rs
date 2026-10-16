use crate::style::impl_css_enum;

/// `font-optical-sizing`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum FontOpticalSizing {
  /// Sets the `opsz` axis to the font size.
  #[default]
  Auto,
  /// Leaves the `opsz` axis at its default.
  None,
}

impl_css_enum!(
  ident FontOpticalSizing,
  "auto" => FontOpticalSizing::Auto,
  "none" => FontOpticalSizing::None,
);
