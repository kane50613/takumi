/// A variable namespace, spelled as the CSS custom-property prefix it reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Namespace {
  /// `--color-*`
  Color,
  /// `--spacing-*`
  Spacing,
  /// `--container-*`
  Container,
  /// `--text-*`
  Text,
  /// `--font-*`
  Font,
  /// `--font-weight-*`
  FontWeight,
  /// `--tracking-*`
  Tracking,
  /// `--leading-*`
  Leading,
  /// `--radius-*`
  Radius,
  /// `--aspect-*`
  Aspect,
}

impl Namespace {
  pub(crate) const fn prefix(self) -> &'static str {
    match self {
      Namespace::Color => "--color-",
      Namespace::Spacing => "--spacing-",
      Namespace::Container => "--container-",
      Namespace::Text => "--text-",
      Namespace::Font => "--font-",
      Namespace::FontWeight => "--font-weight-",
      Namespace::Tracking => "--tracking-",
      Namespace::Leading => "--leading-",
      Namespace::Radius => "--radius-",
      Namespace::Aspect => "--aspect-",
    }
  }
}
