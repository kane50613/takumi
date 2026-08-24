/// A theme namespace, spelled as the CSS custom-property prefix it reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TwNamespace {
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
  /// `--shadow-*`
  Shadow,
  /// `--drop-shadow-*`
  DropShadow,
  /// `--text-shadow-*`
  TextShadow,
  /// `--blur-*`
  Blur,
  /// `--aspect-*`
  Aspect,
  /// `--animate-*`
  Animate,
}

impl TwNamespace {
  pub(crate) const fn prefix(self) -> &'static str {
    match self {
      TwNamespace::Color => "--color-",
      TwNamespace::Spacing => "--spacing-",
      TwNamespace::Container => "--container-",
      TwNamespace::Text => "--text-",
      TwNamespace::Font => "--font-",
      TwNamespace::FontWeight => "--font-weight-",
      TwNamespace::Tracking => "--tracking-",
      TwNamespace::Leading => "--leading-",
      TwNamespace::Radius => "--radius-",
      TwNamespace::Shadow => "--shadow-",
      TwNamespace::DropShadow => "--drop-shadow-",
      TwNamespace::TextShadow => "--text-shadow-",
      TwNamespace::Blur => "--blur-",
      TwNamespace::Aspect => "--aspect-",
      TwNamespace::Animate => "--animate-",
    }
  }
}
