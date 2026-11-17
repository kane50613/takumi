//! Configuring PDF version and export mode.

pub mod validate;
mod version;

pub use validate::{Accessibility, Archival, ValidationError, Validators};
pub use version::PdfVersion;

use crate::krilla::configure::validate::ValidatorsBuilder;

/// A configuration of validator and PDF version.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Configuration {
  validators: Validators,
  version: PdfVersion,
}

impl Configuration {
  /// Return the validators of the configuration.
  pub fn validators(self) -> Validators {
    self.validators
  }

  /// Return the PDF version of the configuration.
  pub fn version(self) -> PdfVersion {
    self.version
  }
}

/// A configuration of validator and PDF version.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct ConfigurationBuilder {
  validators: ValidatorsBuilder,
}

impl ConfigurationBuilder {
  /// Create a new `ConfigurationBuilder` with default settings.
  pub fn new() -> Self {
    Self::default()
  }

  /// Set the PDF/A validator, overwriting the current one if already set.
  pub fn with_archival_validator(mut self, archival: Archival) -> Self {
    self.validators = self.validators.with_archival_validator(archival);
    self
  }

  /// Set the PDF/UA accessibility validator, overwriting the current one if already set.
  pub fn with_accessibility_validator(mut self, ua: Accessibility) -> Self {
    self.validators = self.validators.with_accessibility_validator(ua);
    self
  }

  /// Build the [`Configuration`] at the newest version every validator allows, or fail when
  /// their version ranges do not overlap.
  pub fn finish(self) -> Result<Configuration, ConfigurationError> {
    let validators = self
      .validators
      .finish()
      .map_err(ConfigurationError::NoOverlappingValidatorsRange)?;
    let version = if validators.is_empty() {
      PdfVersion::default()
    } else {
      validators.max()
    };

    Ok(Configuration {
      validators,
      version,
    })
  }
}

/// An error that occurred while building a [`Configuration`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConfigurationError {
  /// The selected validators have no overlapping valid PDF version range.
  NoOverlappingValidatorsRange(Validators),
}

#[cfg(test)]
mod tests {
  use crate::krilla::configure::{
    Accessibility, Archival, Configuration, ConfigurationBuilder, ConfigurationError, PdfVersion,
  };

  #[test]
  fn invalid_combination_2() {
    // A4 requires PDF 2.0; UA1 max is PDF 1.7 → no overlapping range.
    assert!(matches!(
      ConfigurationBuilder::new()
        .with_archival_validator(Archival::A4)
        .with_accessibility_validator(Accessibility::UA1)
        .finish(),
      Err(ConfigurationError::NoOverlappingValidatorsRange(_))
    ));
  }

  #[test]
  fn multi_validator_pdf_a3b_pdf_ua1() {
    let config = ConfigurationBuilder::new()
      .with_archival_validator(Archival::A3_B)
      .with_accessibility_validator(Accessibility::UA1)
      .finish()
      .unwrap();
    assert_eq!(config.validators().archival(), Some(Archival::A3_B));
    assert_eq!(
      config.validators().accessibility(),
      Some(Accessibility::UA1)
    );
    assert_eq!(config.version(), PdfVersion::Pdf17);
  }

  #[test]
  fn multi_validator_pdfa2a_pdfua1() {
    assert!(
      ConfigurationBuilder::new()
        .with_archival_validator(Archival::A2_A)
        .with_accessibility_validator(Accessibility::UA1)
        .finish()
        .is_ok()
    );
  }

  #[test]
  fn empty_validators() {
    let config = ConfigurationBuilder::new().finish().unwrap();
    assert!(config.validators().is_empty());
    assert_eq!(config.version(), PdfVersion::Pdf17);
  }

  #[test]
  fn default_config() {
    assert_eq!(
      ConfigurationBuilder::new().finish().unwrap(),
      Configuration::default()
    );
  }
}
