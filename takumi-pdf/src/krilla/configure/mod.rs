//! Configuring PDF version and export mode.

pub mod validate;
mod version;

pub use validate::{Accessibility, Archival, ValidationError, Validator, Validators};
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
  version: Option<PdfVersion>,
}

impl ConfigurationBuilder {
  /// Create a new `ConfigurationBuilder` with default settings.
  pub fn new() -> Self {
    Self::default()
  }

  /// Set the PDF version, overwriting the current one if already set.
  pub fn with_version(mut self, version: PdfVersion) -> Self {
    self.version = Some(version);
    self
  }

  /// Set a validator, overwriting the current one if the same standard family is already set.
  pub fn set_validator(mut self, validator: Validator) -> Self {
    self.validators = self.validators.set_validator(validator);
    self
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

  /// Build the [`Configuration`], returning an error if the validators and version are incompatible.
  pub fn finish(self) -> Result<Configuration, ConfigurationError> {
    let validators = self
      .validators
      .finish()
      .map_err(ConfigurationError::NoOverlappingValidatorsRange)?;

    let validator_range = validators.min().unwrap_or(PdfVersion::MIN)..=validators.max();
    match self.version {
      Some(version) if validator_range.contains(&version) => Ok(Configuration {
        validators,
        version,
      }),
      Some(version) => Err(ConfigurationError::VersionDoesNotMatchValidatorsRange(
        version, validators,
      )),
      None if !validators.is_empty() => Ok(Configuration {
        validators,
        version: *validator_range.end(),
      }),
      None => {
        let version = PdfVersion::default();
        debug_assert!(validator_range.contains(&version));
        Ok(Configuration {
          validators,
          version,
        })
      }
    }
  }
}

/// An error that occurred while building a [`Configuration`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConfigurationError {
  /// The selected validators have no overlapping valid PDF version range.
  NoOverlappingValidatorsRange(Validators),
  /// The explicitly set PDF version falls outside the range allowed by the validators.
  VersionDoesNotMatchValidatorsRange(PdfVersion, Validators),
}

#[cfg(test)]
mod tests {
  use crate::krilla::configure::{
    Accessibility, Archival, Configuration, ConfigurationBuilder, ConfigurationError, PdfVersion,
  };

  #[test]
  fn invalid_combination_1() {
    // A1_B max is PDF 1.4; explicit PDF 1.7 is out of range.
    assert!(matches!(
      ConfigurationBuilder::new()
        .with_version(PdfVersion::Pdf17)
        .with_archival_validator(Archival::A1_B)
        .finish(),
      Err(ConfigurationError::VersionDoesNotMatchValidatorsRange(
        PdfVersion::Pdf17,
        _
      ))
    ));
  }

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
  fn invalid_combination_3() {
    // A1_B max is PDF 1.4; UA1 max is PDF 1.7 → intersection is PDF14..=PDF14.
    // Explicitly setting PDF 1.7 is out of range.
    assert!(matches!(
      ConfigurationBuilder::new()
        .with_archival_validator(Archival::A1_B)
        .with_accessibility_validator(Accessibility::UA1)
        .with_version(PdfVersion::Pdf17)
        .finish(),
      Err(ConfigurationError::VersionDoesNotMatchValidatorsRange(
        PdfVersion::Pdf17,
        _
      ))
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
