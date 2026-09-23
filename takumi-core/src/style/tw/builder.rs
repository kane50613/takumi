use crate::style::*;

/// The filter chains Tailwind compiles, in its fixed order. An unset variable
/// collapses to nothing through the empty fallback.
const FILTER_CHAIN: &str = "var(--tw-blur,) var(--tw-brightness,) var(--tw-contrast,) var(--tw-grayscale,) var(--tw-hue-rotate,) var(--tw-invert,) var(--tw-saturate,) var(--tw-sepia,) var(--tw-drop-shadow,)";
const BACKDROP_FILTER_CHAIN: &str = "var(--tw-backdrop-blur,) var(--tw-backdrop-brightness,) var(--tw-backdrop-contrast,) var(--tw-backdrop-grayscale,) var(--tw-backdrop-hue-rotate,) var(--tw-backdrop-invert,) var(--tw-backdrop-opacity,) var(--tw-backdrop-saturate,) var(--tw-backdrop-sepia,)";

#[derive(Debug, Default)]
pub(super) struct TailwindDeclarationBuilder {
  pub(super) declarations: StyleDeclarationBlock,
}

impl TailwindDeclarationBuilder {
  /// A builder sized for a class list that expands to about `utilities`
  /// declarations, so the block does not grow one push at a time.
  pub(super) fn with_capacity(utilities: usize) -> Self {
    let mut declarations = StyleDeclarationBlock::default();

    declarations.reserve(utilities);

    Self { declarations }
  }

  pub(super) fn push(&mut self, declaration: StyleDeclaration, important: bool) {
    if let StyleDeclaration::CustomProperty(name, _) = &declaration {
      self.declarations.push_element_state(name);
    }

    self.declarations.push(declaration, important);
  }

  pub(super) fn push_all(
    &mut self,
    declarations: impl IntoIterator<Item = StyleDeclaration>,
    important: bool,
  ) {
    for declaration in declarations {
      self.push(declaration, important);
    }
  }

  pub(super) fn push_custom(&mut self, name: &str, value: &str, important: bool) {
    self.push(
      StyleDeclaration::CustomProperty(name.to_owned(), value.to_owned()),
      important,
    );
  }

  pub(super) fn push_deferred(
    &mut self,
    longhand: LonghandId,
    specified_value: String,
    important: bool,
  ) {
    self.push(
      StyleDeclaration::Deferred(DeferredDeclaration {
        property: PropertyId::Longhand(longhand),
        specified_value,
      }),
      important,
    );
  }

  /// Sets one `--tw-*` filter variable and the chain that reads them all.
  pub(super) fn push_filter(&mut self, backdrop: bool, name: &str, value: &str, important: bool) {
    let (prefix, longhand, chain) = match backdrop {
      false => ("--tw-", LonghandId::Filter, FILTER_CHAIN),
      true => (
        "--tw-backdrop-",
        LonghandId::BackdropFilter,
        BACKDROP_FILTER_CHAIN,
      ),
    };

    self.push_custom(&format!("{prefix}{name}"), value, important);
    self.push_deferred(longhand, chain.to_owned(), important);
  }

  pub(super) fn finish(mut self) -> StyleDeclarationBlock {
    type BorderSide = (LonghandId, LonghandId, fn(BorderStyle) -> StyleDeclaration);
    let sides: [BorderSide; 4] = [
      (
        LonghandId::BorderTopWidth,
        LonghandId::BorderTopStyle,
        StyleDeclaration::border_top_style,
      ),
      (
        LonghandId::BorderRightWidth,
        LonghandId::BorderRightStyle,
        StyleDeclaration::border_right_style,
      ),
      (
        LonghandId::BorderBottomWidth,
        LonghandId::BorderBottomStyle,
        StyleDeclaration::border_bottom_style,
      ),
      (
        LonghandId::BorderLeftWidth,
        LonghandId::BorderLeftStyle,
        StyleDeclaration::border_left_style,
      ),
    ];
    for (width_id, style_id, style_decl) in sides {
      let sets = |longhand| {
        self
          .declarations
          .iter()
          .any(|declaration| declaration.affected_longhands().contains(&longhand))
      };

      if sets(width_id) && !sets(style_id) {
        self
          .declarations
          .push(style_decl(BorderStyle::Solid), false);
      }
    }

    self.declarations
  }
}
