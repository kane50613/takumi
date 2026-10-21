//! The utilities, keyed by root as Tailwind's [`Utilities`](https://github.com/tailwindlabs/tailwindcss/blob/main/packages/tailwindcss/src/utilities.ts)
//! registers them: a root alone, as `border` is, or a root and a value, as `border-2` is.

use std::sync::Arc;

use phf::phf_map;

use super::{
  Namespace, TW_VAR_SPACING, TailwindPropertyParser, builder::TailwindDeclarationBuilder,
  parse_opacity_modifier, parser::*, with_opacity,
};
use crate::style::*;

/// The stop list Tailwind compiles gradients to, with each variable's
/// `@property` initial value inlined as its fallback.
const GRADIENT_STOPS: &str = "var(--tw-gradient-via-stops, var(--tw-gradient-from, transparent) var(--tw-gradient-from-position, 0%), var(--tw-gradient-to, transparent) var(--tw-gradient-to-position, 100%))";
const GRADIENT_VIA_STOPS: &str = "var(--tw-gradient-from, transparent) var(--tw-gradient-from-position, 0%), var(--tw-gradient-via) var(--tw-gradient-via-position, 50%), var(--tw-gradient-to, transparent) var(--tw-gradient-to-position, 100%)";

const TRANSLATE_PAIR: &str = "var(--tw-translate-x, 0px) var(--tw-translate-y, 0px)";
const SCALE_PAIR: &str = "var(--tw-scale-x, 100%) var(--tw-scale-y, 100%)";

const HAIRLINE: LineWidth = LineWidth::Length(Length::Px(1.0));

/// A root's utilities.
pub(super) struct Utility {
  /// What the root alone writes, as `border` does.
  bare: Option<fn(&mut TailwindDeclarationBuilder)>,
  /// The value forms the root reads, tried in order.
  values: &'static [Value],
  /// What a theme token none of `values` know writes, as the namespaces it reads paired with the
  /// longhands each one fills. `bg-brand-500` has no built-in value to expand, so this is the
  /// only place its target is written down; a root reading two namespaces emits one variable per
  /// group, and the undefined ones leave their longhands unset.
  theme: &'static [(Namespace, &'static [LonghandId])],
}

/// One value form of a root.
pub(super) struct Value {
  /// The theme namespaces the value reads, tried in order.
  namespaces: &'static [Namespace],
  /// Emits the suffix, negated when the token starts with `-`.
  emit: fn(&str, bool, &mut TailwindDeclarationBuilder) -> ValueMatch,
}

/// What a value form made of a suffix.
enum ValueMatch {
  Miss,
  /// The suffix is this form's, but the token negates a value that has no negative.
  NoNegative,
  Emitted,
}

/// A value a `-` prefix negates, as `-mt-4` does.
trait Negate: Sized {
  fn negate(self) -> Option<Self>;
}

impl Negate for Length {
  fn negate(self) -> Option<Self> {
    (!matches!(self, Length::Auto)).then(|| self.negative())
  }
}

impl Negate for GridPlacement {
  fn negate(self) -> Option<Self> {
    match self {
      GridPlacement::Line(line) => line.checked_neg().map(GridPlacement::Line),
      _ => None,
    }
  }
}

impl Negate for PercentageNumber {
  fn negate(self) -> Option<Self> {
    Some(-self)
  }
}

impl Negate for Angle {
  fn negate(self) -> Option<Self> {
    Some(-self)
  }
}

impl Negate for TwLetterSpacing {
  fn negate(self) -> Option<Self> {
    Some(-self)
  }
}

impl Utility {
  const fn fixed(emit: fn(&mut TailwindDeclarationBuilder)) -> Self {
    Self {
      bare: Some(emit),
      values: &[],
      theme: &[],
    }
  }

  const fn functional(values: &'static [Value]) -> Self {
    Self {
      bare: None,
      values,
      theme: &[],
    }
  }

  const fn bare(mut self, emit: fn(&mut TailwindDeclarationBuilder)) -> Self {
    self.bare = Some(emit);
    self
  }

  const fn theme(mut self, theme: &'static [(Namespace, &'static [LonghandId])]) -> Self {
    self.theme = theme;
    self
  }

  /// Emits `suffix` through the first value form that reads it, or else the variables the
  /// theme groups read.
  fn emit_value(
    &self,
    suffix: &str,
    negative: bool,
    builder: &mut TailwindDeclarationBuilder,
  ) -> ValueMatch {
    for value in self.values {
      let mut builtin = TailwindDeclarationBuilder::default();

      match (value.emit)(suffix, negative, &mut builtin) {
        ValueMatch::Miss => continue,
        ValueMatch::NoNegative => return ValueMatch::NoNegative,
        ValueMatch::Emitted => {}
      }

      let declarations = builtin.finish().declarations;

      // A value composing through custom properties has no single longhand to defer, so it
      // keeps its built-in value.
      match var_expression(value.namespaces, suffix, negative) {
        Some((name, expression))
          if !declarations.iter().any(|declaration| {
            matches!(
              declaration,
              StyleDeclaration::CustomProperty(..)
                | StyleDeclaration::Deferred(..)
                | StyleDeclaration::VarRef(..)
            )
          }) =>
        {
          builder.push_all(declarations.into_iter().map(|declaration| {
            var_ref(
              &name,
              &expression,
              declaration.longhand_id(),
              Some(declaration),
            )
          }));
        }
        _ => builder.push_all(declarations),
      }

      return ValueMatch::Emitted;
    }

    let mut matched = ValueMatch::Miss;

    for (namespace, longhands) in self.theme {
      let Some((name, expression)) = var_expression(&[*namespace], suffix, negative) else {
        continue;
      };

      for &longhand in *longhands {
        builder.push(var_ref(&name, &expression, longhand, None));
        matched = ValueMatch::Emitted;
      }
    }

    matched
  }
}

/// Emits the declarations a utility token compiles to, or `None` when no utility reads it.
pub(super) fn emit(token: &str, builder: &mut TailwindDeclarationBuilder) -> Option<()> {
  if let Some(emit) = UTILITIES.get(token).and_then(|utility| utility.bare) {
    emit(builder);
    return Some(());
  }

  let (token, negative) = match token.strip_prefix('-') {
    Some(stripped) => (stripped, true),
    None => (token, false),
  };

  for (dash, _) in token.match_indices('-').rev() {
    let Some(utility) = UTILITIES.get(&token[..dash]) else {
      continue;
    };

    match utility.emit_value(&token[dash + 1..], negative, builder) {
      ValueMatch::Miss => continue,
      ValueMatch::NoNegative => return None,
      ValueMatch::Emitted => return Some(()),
    }
  }

  None
}

fn var_ref(
  name: &Arc<str>,
  expression: &Arc<str>,
  longhand: LonghandId,
  fallback: Option<StyleDeclaration>,
) -> StyleDeclaration {
  let (name, expression) = match companion_variable(name, longhand) {
    Some(companion) => (companion.clone(), format!("var({companion})")),
    None => (name.clone(), expression.to_string()),
  };

  StyleDeclaration::VarRef(TwVarRef {
    name,
    deferred: DeferredDeclaration {
      property: PropertyId::Longhand(longhand),
      specified_value: expression,
    },
    fallback: fallback.map(Box::new),
  })
}

/// The variable a longhand reads when the token spells it separately from its
/// primary value, as `--text-lg--line-height` does for `--text-lg`.
fn companion_variable(name: &str, longhand: LonghandId) -> Option<Arc<str>> {
  (longhand == LonghandId::LineHeight && name.starts_with(Namespace::Text.prefix()))
    .then(|| format!("{name}--line-height").into())
}

/// The `var()` expression a utility suffix reads, or `None` when the value is
/// spelled in the class itself. Numeric spacing multiplies the `--spacing` step
/// the way Tailwind's own `calc(var(--spacing) * 4)` does.
fn var_expression(
  namespaces: &[Namespace],
  suffix: &str,
  negative: bool,
) -> Option<(Arc<str>, Arc<str>)> {
  if suffix.starts_with('[') {
    return None;
  }

  // `bg-brand-500/50` mixes the variable with transparent, the way Tailwind's
  // own opacity modifier compiles.
  if let Some((token, opacity)) = suffix.split_once('/') {
    let &namespace = namespaces.first()?;
    let percentage = parse_opacity_modifier(opacity)?;

    if namespace != Namespace::Color {
      return None;
    }

    let name = format!("{}{token}", namespace.prefix());
    let expression = with_opacity(&format!("var({name})"), percentage);

    return Some((name.into(), expression.into()));
  }

  // `max-w-4` multiplies `--spacing` where `max-w-prose` reads `--container-prose`.
  if suffix.parse::<f32>().is_ok() && namespaces.contains(&Namespace::Spacing) {
    let sign = if negative { "-" } else { "" };

    return Some((
      "--spacing".into(),
      format!("calc(var(--spacing, {TW_VAR_SPACING}rem) * {sign}{suffix})").into(),
    ));
  }

  let &namespace = namespaces.first()?;

  let name = format!("{}{suffix}", namespace.prefix());
  let expression = match negative {
    true => format!("calc(var({name}) * -1)"),
    false => format!("var({name})"),
  };

  Some((name.into(), expression.into()))
}

/// The theme namespaces a value form reads: the value type's own list, or the override an entry
/// spells in brackets.
macro_rules! namespaces {
  ($ty:ty) => {
    <$ty as TailwindPropertyParser>::NAMESPACES
  };
  ($ty:ty, $($namespace:expr),+) => {
    &[$($namespace),+]
  };
}

/// A [`Value`] reading `$ty`. `Type: constructor, …` pushes `StyleDeclaration::constructor(value)`
/// for each constructor, `Type, emit` calls `emit`. A leading `-` lets the token negate the value,
/// and `[namespace, …]` after the type overrides the theme namespaces it reads.
macro_rules! value {
  (@negate true, $value:ident) => {
    match Negate::negate($value) {
      Some(negated) => negated,
      None => return ValueMatch::NoNegative,
    }
  };
  (@negate false, $value:ident) => {
    return ValueMatch::NoNegative
  };
  (@form $negatable:tt; $ty:ty $([$($namespace:expr),+])? : $($constructor:ident),+) => {
    value!(@form $negatable; $ty $([$($namespace),+])?, |value, builder| {
      $(builder.push(StyleDeclaration::$constructor(value));)+
    })
  };
  (@form $negatable:tt; $ty:ty $([$($namespace:expr),+])?, $emit:expr) => {
    Value {
      namespaces: namespaces!($ty $(, $($namespace),+)?),
      emit: |suffix, negative, builder| {
        let Some(value) = <$ty as TailwindPropertyParser>::parse_tw_with_arbitrary(suffix) else {
          return ValueMatch::Miss;
        };
        let value: $ty = match negative {
          false => value,
          true => value!(@negate $negatable, value),
        };
        let emit: fn($ty, &mut TailwindDeclarationBuilder) = $emit;

        emit(value, builder);
        ValueMatch::Emitted
      },
    }
  };
  (- $($form:tt)+) => {
    value!(@form true; $($form)+)
  };
  ($($form:tt)+) => {
    value!(@form false; $($form)+)
  };
}

/// A [`Value`] reading a corner radius into each corner.
macro_rules! rounded {
  ($($corner:ident),+) => {
    value!(TwRounded, |rounded, builder| {
      let radius = SpacePair::from_single(rounded.0);

      $(builder.push(StyleDeclaration::$corner(radius));)+
    })
  };
}

/// A root alone pushing `StyleDeclaration::constructor(value)` for each pair.
macro_rules! fixed {
  ($($constructor:ident($value:expr)),+ $(,)?) => {
    Utility::fixed(|builder| {
      $(builder.push(StyleDeclaration::$constructor($value));)+
    })
  };
}

const FLEX_BASIS: &[Value] = &[value!(FlexBasis, |basis, builder| builder
  .push(StyleDeclaration::flex_basis(Some(basis))))];

/// Maps a utility root to its utilities.
pub(super) static UTILITIES: phf::Map<&str, Utility> = phf_map! {
  "list" => Utility::functional(&[
    value!(ListStyleType: list_style_type),
    value!(ListStylePosition: list_style_position),
  ]),
  "list-image" => Utility::functional(&[value!(ListStyleImage: list_style_image)]),
  "object" => Utility::functional(&[
    value!(ObjectFit: object_fit),
    value!(PositionValue: object_position),
  ]),
  "bg" => Utility::functional(&[
    value!(ColorInput: background_color),
    value!(BackgroundImage, background_image),
    value!(PositionValue, background_position),
    value!(BackgroundSize, background_size),
  ])
  .theme(&[(Namespace::Color, &[LonghandId::BackgroundColor])]),
  "bg-clip" => Utility::functional(&[value!(BackgroundClip: background_clip)]),
  "mask" => Utility::functional(&[value!(BackgroundImage, |image, builder| builder
    .push(StyleDeclaration::mask_image(Some([image].into()))))]),
  "bg-linear" => Utility::functional(&[value!(Angle, |angle, builder| builder.push_deferred(
    LonghandId::BackgroundImage,
    format!("linear-gradient({}deg in oklab, var(--tw-gradient-stops))", *angle),
  ))]),
  "bg-radial" => Utility::fixed(|builder| builder.push_deferred(
    LonghandId::BackgroundImage,
    "radial-gradient(in oklab, var(--tw-gradient-stops))".to_owned(),
  )),
  "bg-conic" => Utility::functional(&[value!(Angle, conic_gradient)])
    .bare(|builder| conic_gradient(Angle::zero(), builder)),
  "from" => Utility::functional(&[
    value!(TwGradientPosition, |position, builder| builder
      .push_custom("--tw-gradient-from-position", &position.0.to_css_string())),
    value!(TwVarColor, |color, builder| {
      builder.push_custom("--tw-gradient-from", &color.0);
      builder.push_custom("--tw-gradient-stops", GRADIENT_STOPS);
    }),
  ]),
  "to" => Utility::functional(&[
    value!(TwGradientPosition, |position, builder| builder
      .push_custom("--tw-gradient-to-position", &position.0.to_css_string())),
    value!(TwVarColor, |color, builder| {
      builder.push_custom("--tw-gradient-to", &color.0);
      builder.push_custom("--tw-gradient-stops", GRADIENT_STOPS);
    }),
  ]),
  "via" => Utility::functional(&[
    value!(TwGradientPosition, |position, builder| builder
      .push_custom("--tw-gradient-via-position", &position.0.to_css_string())),
    value!(TwVarColor, |color, builder| {
      builder.push_custom("--tw-gradient-via", &color.0);
      builder.push_custom("--tw-gradient-via-stops", GRADIENT_VIA_STOPS);
      builder.push_custom("--tw-gradient-stops", "var(--tw-gradient-via-stops)");
    }),
  ]),
  "bg-size" => Utility::functional(&[value!(BackgroundSize, background_size)]),
  "bg-position" => Utility::functional(&[value!(PositionValue, background_position)]),
  "bg-repeat" => fixed!(background_repeat([BackgroundRepeat::repeat()].into())),
  "bg-no-repeat" => fixed!(background_repeat([BackgroundRepeat::no_repeat()].into())),
  "bg-space" | "bg-repeat-space" => fixed!(background_repeat([BackgroundRepeat::space()].into())),
  "bg-round" | "bg-repeat-round" => fixed!(background_repeat([BackgroundRepeat::round()].into())),
  "bg-repeat-x" => fixed!(background_repeat(
    [BackgroundRepeat(BackgroundRepeatStyle::Repeat, BackgroundRepeatStyle::NoRepeat)].into()
  )),
  "bg-repeat-y" => fixed!(background_repeat(
    [BackgroundRepeat(BackgroundRepeatStyle::NoRepeat, BackgroundRepeatStyle::Repeat)].into()
  )),
  "w" => Utility::functional(&[value!(Size: width)])
    .theme(&[(Namespace::Spacing, &[LonghandId::Width])]),
  "h" => Utility::functional(&[value!(Size: height)])
    .theme(&[(Namespace::Spacing, &[LonghandId::Height])]),
  "min-w" => Utility::functional(&[value!(Length: min_width)])
    .theme(&[(Namespace::Spacing, &[LonghandId::MinWidth])]),
  "min-h" => Utility::functional(&[value!(Length: min_height)])
    .theme(&[(Namespace::Spacing, &[LonghandId::MinHeight])]),
  "max-w" => Utility::functional(&[value!(
    Length [Namespace::Container, Namespace::Spacing],
    |length, builder| builder.push(StyleDeclaration::max_width(length.into()))
  )])
  // Groups apply in order, so when two namespaces fill the same longhand the one Tailwind
  // prefers goes last.
  .theme(&[
    (Namespace::Spacing, &[LonghandId::MaxWidth]),
    (Namespace::Container, &[LonghandId::MaxWidth]),
  ]),
  "max-h" => Utility::functional(&[value!(Length, |length, builder| builder
    .push(StyleDeclaration::max_height(length.into())))])
  .theme(&[(Namespace::Spacing, &[LonghandId::MaxHeight])]),
  "w-screen" => fixed!(width(Size::Length(Length::Vw(100.0)))),
  "h-screen" => fixed!(height(Size::Length(Length::Vh(100.0)))),
  "min-w-screen" => fixed!(min_width(Length::Vw(100.0))),
  "min-h-screen" => fixed!(min_height(Length::Vh(100.0))),
  "max-w-screen" => fixed!(max_width(Length::Vw(100.0).into())),
  "max-h-screen" => fixed!(max_height(Length::Vh(100.0).into())),
  "size" => Utility::functional(&[value!(Size: width, height)])
    .theme(&[(Namespace::Spacing, &[LonghandId::Width, LonghandId::Height])]),
  "font" => Utility::functional(&[
    value!(FontWeight: font_weight),
    value!(FontFamily: font_family),
  ])
  .theme(&[
    (Namespace::Font, &[LonghandId::FontFamily]),
    (Namespace::FontWeight, &[LonghandId::FontWeight]),
  ]),
  "font-stretch" => Utility::functional(&[value!(FontStretch: font_stretch)]),
  "gap" => Utility::functional(&[value!(Length, |gap, builder| {
    builder.push(StyleDeclaration::row_gap(gap.into()));
    builder.push(StyleDeclaration::column_gap(gap.into()));
  })])
  .theme(&[(Namespace::Spacing, &[LonghandId::ColumnGap, LonghandId::RowGap])]),
  "gap-x" => Utility::functional(&[value!(Length, |gap, builder| builder
    .push(StyleDeclaration::column_gap(gap.into())))])
  .theme(&[(Namespace::Spacing, &[LonghandId::ColumnGap])]),
  "gap-y" => Utility::functional(&[value!(Length, |gap, builder| builder
    .push(StyleDeclaration::row_gap(gap.into())))])
  .theme(&[(Namespace::Spacing, &[LonghandId::RowGap])]),
  "justify" => Utility::functional(&[value!(JustifyContent: justify_content)]),
  "content" => Utility::functional(&[
    value!(ContentValue: content),
    value!(JustifyContent: align_content),
  ]),
  "items" => Utility::functional(&[value!(AlignItems: align_items)]),
  "self" => Utility::functional(&[value!(AlignItems: align_self)]),
  "justify-self" => Utility::functional(&[value!(AlignItems: justify_self)]),
  "justify-items" => Utility::functional(&[value!(AlignItems: justify_items)]),
  "overflow" => Utility::functional(&[value!(Overflow: overflow_x, overflow_y)]),
  "overflow-x" => Utility::functional(&[value!(Overflow: overflow_x)]),
  "overflow-y" => Utility::functional(&[value!(Overflow: overflow_y)]),
  "border" => Utility::functional(&[
    value!(ColorInput: border_top_color, border_right_color, border_bottom_color, border_left_color),
    value!(BorderStyle, border_style),
    value!(LineWidth: border_top_width, border_right_width, border_bottom_width, border_left_width),
  ])
  .bare(|builder| {
    builder.push(StyleDeclaration::border_top_width(HAIRLINE));
    builder.push(StyleDeclaration::border_right_width(HAIRLINE));
    builder.push(StyleDeclaration::border_bottom_width(HAIRLINE));
    builder.push(StyleDeclaration::border_left_width(HAIRLINE));
  })
  .theme(&[(
    Namespace::Color,
    &[
      LonghandId::BorderTopColor,
      LonghandId::BorderRightColor,
      LonghandId::BorderBottomColor,
      LonghandId::BorderLeftColor,
    ],
  )]),
  "border-t" => Utility::functional(&[
    value!(ColorInput: border_top_color),
    value!(LineWidth: border_top_width),
  ])
  .bare(|builder| builder.push(StyleDeclaration::border_top_width(HAIRLINE)))
  .theme(&[(Namespace::Color, &[LonghandId::BorderTopColor])]),
  "border-r" => Utility::functional(&[
    value!(ColorInput: border_right_color),
    value!(LineWidth: border_right_width),
  ])
  .bare(|builder| builder.push(StyleDeclaration::border_right_width(HAIRLINE)))
  .theme(&[(Namespace::Color, &[LonghandId::BorderRightColor])]),
  "border-b" => Utility::functional(&[
    value!(ColorInput: border_bottom_color),
    value!(LineWidth: border_bottom_width),
  ])
  .bare(|builder| builder.push(StyleDeclaration::border_bottom_width(HAIRLINE)))
  .theme(&[(Namespace::Color, &[LonghandId::BorderBottomColor])]),
  "border-l" => Utility::functional(&[
    value!(ColorInput: border_left_color),
    value!(LineWidth: border_left_width),
  ])
  .bare(|builder| builder.push(StyleDeclaration::border_left_width(HAIRLINE)))
  .theme(&[(Namespace::Color, &[LonghandId::BorderLeftColor])]),
  "border-x" => Utility::functional(&[
    value!(ColorInput: border_left_color, border_right_color),
    value!(LineWidth: border_left_width, border_right_width),
  ])
  .bare(|builder| {
    builder.push(StyleDeclaration::border_left_width(HAIRLINE));
    builder.push(StyleDeclaration::border_right_width(HAIRLINE));
  })
  .theme(&[(Namespace::Color, &[LonghandId::BorderLeftColor, LonghandId::BorderRightColor])]),
  "border-y" => Utility::functional(&[
    value!(ColorInput: border_top_color, border_bottom_color),
    value!(LineWidth: border_top_width, border_bottom_width),
  ])
  .bare(|builder| {
    builder.push(StyleDeclaration::border_top_width(HAIRLINE));
    builder.push(StyleDeclaration::border_bottom_width(HAIRLINE));
  })
  .theme(&[(Namespace::Color, &[LonghandId::BorderTopColor, LonghandId::BorderBottomColor])]),
  "outline" => Utility::functional(&[
    value!(ColorInput: outline_color),
    value!(BorderStyle: outline_style),
    value!(LineWidth: outline_width),
  ])
  .bare(|builder| {
    builder.push(StyleDeclaration::outline_width(HAIRLINE));
    builder.push(StyleDeclaration::outline_style(BorderStyle::Solid));
  })
  .theme(&[(Namespace::Color, &[LonghandId::OutlineColor])]),
  "outline-offset" => Utility::functional(&[value!(LineWidth, |offset, builder| builder
    .push(StyleDeclaration::outline_offset(Length::from(offset))))]),
  "shadow" => Utility::functional(&[
    value!(TwVarColor, |color, builder| builder.push_custom("--tw-shadow-color", &color.0)),
    value!(BoxShadow, |shadow, builder| builder
      .push_deferred(LonghandId::BoxShadow, box_shadow_css(&shadow))),
  ])
  .bare(|builder| box_shadow_preset(builder, "--shadow", &SHADOW_SM)),
  "shadow-2xs" => Utility::fixed(|builder| box_shadow_preset(builder, "--shadow-2xs", &SHADOW_2XS)),
  "shadow-xs" => Utility::fixed(|builder| box_shadow_preset(builder, "--shadow-xs", &SHADOW_XS)),
  "shadow-sm" => Utility::fixed(|builder| box_shadow_preset(builder, "--shadow-sm", &SHADOW_SM)),
  "shadow-md" => Utility::fixed(|builder| box_shadow_preset(builder, "--shadow-md", &SHADOW_MD)),
  "shadow-lg" => Utility::fixed(|builder| box_shadow_preset(builder, "--shadow-lg", &SHADOW_LG)),
  "shadow-xl" => Utility::fixed(|builder| box_shadow_preset(builder, "--shadow-xl", &SHADOW_XL)),
  "shadow-2xl" => Utility::fixed(|builder| box_shadow_preset(builder, "--shadow-2xl", &SHADOW_2XL)),
  "shadow-none" | "inset-shadow-none" => Utility::fixed(|builder| builder
    .push_deferred(LonghandId::BoxShadow, "none".to_owned())),
  "inset-shadow-2xs" => Utility::fixed(|builder| box_shadow_preset(builder, "--inset-shadow-2xs", &INSET_SHADOW_2XS)),
  "inset-shadow-xs" => Utility::fixed(|builder| box_shadow_preset(builder, "--inset-shadow-xs", &INSET_SHADOW_XS)),
  "inset-shadow-sm" => Utility::fixed(|builder| box_shadow_preset(builder, "--inset-shadow-sm", &INSET_SHADOW_SM)),
  "grow" | "flex-grow" => Utility::functional(&[value!(FlexGrow, |grow, builder| builder.push(StyleDeclaration::flex_grow(Some(grow))))])
    .bare(|builder| builder.push(StyleDeclaration::flex_grow(Some(FlexGrow(1.0))))),
  "shrink" | "flex-shrink" => Utility::functional(&[value!(FlexGrow, |shrink, builder| builder.push(StyleDeclaration::flex_shrink(Some(shrink))))])
    .bare(|builder| builder.push(StyleDeclaration::flex_shrink(Some(FlexGrow(1.0))))),
  "basis" => Utility::functional(FLEX_BASIS)
    .theme(&[(Namespace::Spacing, &[LonghandId::FlexBasis])]),
  "flex-basis" => Utility::functional(FLEX_BASIS),
  "aspect" => Utility::functional(&[value!(AspectRatio: aspect_ratio)])
    .theme(&[(Namespace::Aspect, &[LonghandId::AspectRatio])]),
  "aspect-auto" => fixed!(aspect_ratio(AspectRatio::Auto)),
  "text" => Utility::functional(&[
    value!(TwFontSize, |size, builder| {
      builder.push(StyleDeclaration::font_size(size.font_size));

      if let Some(line_height) = size.line_height {
        builder.push(StyleDeclaration::line_height(line_height));
      }
    }),
    value!(ColorInput: color),
    value!(TextAlign: text_align),
    value!(TextWrap, |wrap, builder| builder.push_all(StyleDeclaration::expand_text_wrap(wrap))),
  ])
  .theme(&[
    (Namespace::Text, &[LonghandId::FontSize, LonghandId::LineHeight]),
    (Namespace::Color, &[LonghandId::Color]),
  ]),
  "text-ellipsis" => fixed!(text_overflow(TextOverflow::Ellipsis)),
  "text-clip" => fixed!(text_overflow(TextOverflow::Clip)),
  "truncate" => fixed!(
    text_overflow(TextOverflow::Ellipsis),
    text_wrap_mode(TextWrapMode::NoWrap),
    white_space_collapse(WhiteSpaceCollapse::Collapse),
    overflow_x(Overflow::Hidden),
    overflow_y(Overflow::Hidden),
  ),
  "decoration" => Utility::functional(&[
    value!(ColorInput: text_decoration_color),
    value!(TextDecorationThickness: text_decoration_thickness),
  ])
  .theme(&[(Namespace::Color, &[LonghandId::TextDecorationColor])]),
  "decoration-solid" => fixed!(text_decoration_style(TextDecorationStyle::Solid)),
  "decoration-double" => fixed!(text_decoration_style(TextDecorationStyle::Double)),
  "decoration-dotted" => fixed!(text_decoration_style(TextDecorationStyle::Dotted)),
  "decoration-dashed" => fixed!(text_decoration_style(TextDecorationStyle::Dashed)),
  "decoration-wavy" => fixed!(text_decoration_style(TextDecorationStyle::Wavy)),
  "underline" => fixed!(text_decoration_line(Some(TextDecorationLines::UNDERLINE))),
  "overline" => fixed!(text_decoration_line(Some(TextDecorationLines::OVERLINE))),
  "line-through" => fixed!(text_decoration_line(Some(TextDecorationLines::LINE_THROUGH))),
  "no-underline" => fixed!(text_decoration_line(Some(TextDecorationLines::empty()))),
  "uppercase" => fixed!(text_transform(TextTransformCase::Uppercase.into())),
  "lowercase" => fixed!(text_transform(TextTransformCase::Lowercase.into())),
  "capitalize" => fixed!(text_transform(TextTransformCase::Capitalize.into())),
  "normal-case" => fixed!(text_transform(TextTransformCase::None.into())),
  "italic" => fixed!(font_style(FontStyle::italic())),
  "not-italic" => fixed!(font_style(FontStyle::normal())),
  "leading" => Utility::functional(&[value!(LineHeight: line_height)])
    .theme(&[(Namespace::Leading, &[LonghandId::LineHeight])]),
  "tracking" => Utility::functional(&[value!(-TwLetterSpacing, |spacing, builder| builder
    .push(StyleDeclaration::letter_spacing(spacing.0)))])
  .theme(&[(Namespace::Tracking, &[LonghandId::LetterSpacing])]),
  "opacity" => Utility::functional(&[value!(PercentageNumber: opacity)]),
  "line-clamp" => Utility::functional(&[value!(LineClamp, |clamp, builder| builder
    .push_all(StyleDeclaration::expand_line_clamp(clamp)))]),
  "whitespace" => Utility::functional(&[value!(WhiteSpace, |white_space, builder| builder
    .push_all(StyleDeclaration::expand_white_space(white_space)))]),
  "wrap" => Utility::functional(&[value!(OverflowWrap: overflow_wrap)]),
  "break-normal" => fixed!(word_break(WordBreak::Normal)),
  "break-all" => fixed!(word_break(WordBreak::BreakAll)),
  "break-keep" => fixed!(word_break(WordBreak::KeepAll)),
  "flex" => Utility::functional(&[value!(Flex, |flex, builder| builder
    .push_all(StyleDeclaration::expand_flex(flex)))])
  .bare(|builder| builder.push(StyleDeclaration::display(Display::Flex))),
  "flex-auto" => Utility::fixed(|builder| builder.push_all(StyleDeclaration::expand_flex(Flex::auto()))),
  "flex-initial" => Utility::fixed(|builder| builder.push_all(StyleDeclaration::expand_flex(Flex::initial()))),
  "flex-none" => Utility::fixed(|builder| builder.push_all(StyleDeclaration::expand_flex(Flex::none()))),
  "flex-row" => fixed!(flex_direction(FlexDirection::Row)),
  "flex-row-reverse" => fixed!(flex_direction(FlexDirection::RowReverse)),
  "flex-col" => fixed!(flex_direction(FlexDirection::Column)),
  "flex-col-reverse" => fixed!(flex_direction(FlexDirection::ColumnReverse)),
  "flex-wrap" => fixed!(flex_wrap(FlexWrap::Wrap)),
  "flex-wrap-reverse" => fixed!(flex_wrap(FlexWrap::WrapReverse)),
  "flex-nowrap" => fixed!(flex_wrap(FlexWrap::NoWrap)),
  "origin" => Utility::functional(&[value!(PositionValue: transform_origin)]),
  "translate" => Utility::functional(&[value!(-Length, |length, builder| builder
    .push_axes(&["--tw-translate-x", "--tw-translate-y"], &length.to_css_string(), LonghandId::Translate, TRANSLATE_PAIR))]),
  "translate-x" => Utility::functional(&[value!(-Length, |length, builder| builder
    .push_axes(&["--tw-translate-x"], &length.to_css_string(), LonghandId::Translate, TRANSLATE_PAIR))]),
  "translate-y" => Utility::functional(&[value!(-Length, |length, builder| builder
    .push_axes(&["--tw-translate-y"], &length.to_css_string(), LonghandId::Translate, TRANSLATE_PAIR))]),
  "rotate" => Utility::functional(&[value!(-Angle, |angle, builder| builder
    .push(StyleDeclaration::rotate(Some(angle))))]),
  "scale" => Utility::functional(&[value!(-PercentageNumber, |scale, builder| builder
    .push_axes(&["--tw-scale-x", "--tw-scale-y"], &scale.to_css_string(), LonghandId::Scale, SCALE_PAIR))]),
  "scale-x" => Utility::functional(&[value!(-PercentageNumber, |scale, builder| builder
    .push_axes(&["--tw-scale-x"], &scale.to_css_string(), LonghandId::Scale, SCALE_PAIR))]),
  "scale-y" => Utility::functional(&[value!(-PercentageNumber, |scale, builder| builder
    .push_axes(&["--tw-scale-y"], &scale.to_css_string(), LonghandId::Scale, SCALE_PAIR))]),
  "m" => Utility::functional(&[value!(-Length: margin_top, margin_right, margin_bottom, margin_left)])
    .theme(&[(
      Namespace::Spacing,
      &[
        LonghandId::MarginTop,
        LonghandId::MarginRight,
        LonghandId::MarginBottom,
        LonghandId::MarginLeft,
      ],
    )]),
  "mx" => Utility::functional(&[value!(-Length: margin_left, margin_right)])
    .theme(&[(Namespace::Spacing, &[LonghandId::MarginLeft, LonghandId::MarginRight])]),
  "my" => Utility::functional(&[value!(-Length: margin_top, margin_bottom)])
    .theme(&[(Namespace::Spacing, &[LonghandId::MarginTop, LonghandId::MarginBottom])]),
  "mt" => Utility::functional(&[value!(-Length: margin_top)])
    .theme(&[(Namespace::Spacing, &[LonghandId::MarginTop])]),
  "mr" => Utility::functional(&[value!(-Length: margin_right)])
    .theme(&[(Namespace::Spacing, &[LonghandId::MarginRight])]),
  "mb" => Utility::functional(&[value!(-Length: margin_bottom)])
    .theme(&[(Namespace::Spacing, &[LonghandId::MarginBottom])]),
  "ml" => Utility::functional(&[value!(-Length: margin_left)])
    .theme(&[(Namespace::Spacing, &[LonghandId::MarginLeft])]),
  "ms" => Utility::functional(&[value!(-Length: margin_inline_start)])
    .theme(&[(Namespace::Spacing, &[LonghandId::MarginInlineStart])]),
  "me" => Utility::functional(&[value!(-Length: margin_inline_end)])
    .theme(&[(Namespace::Spacing, &[LonghandId::MarginInlineEnd])]),
  "p" => Utility::functional(&[value!(Length: padding_top, padding_right, padding_bottom, padding_left)])
    .theme(&[(
      Namespace::Spacing,
      &[
        LonghandId::PaddingTop,
        LonghandId::PaddingRight,
        LonghandId::PaddingBottom,
        LonghandId::PaddingLeft,
      ],
    )]),
  "px" => Utility::functional(&[value!(Length: padding_left, padding_right)])
    .theme(&[(Namespace::Spacing, &[LonghandId::PaddingLeft, LonghandId::PaddingRight])]),
  "py" => Utility::functional(&[value!(Length: padding_top, padding_bottom)])
    .theme(&[(Namespace::Spacing, &[LonghandId::PaddingTop, LonghandId::PaddingBottom])]),
  "pt" => Utility::functional(&[value!(Length: padding_top)])
    .theme(&[(Namespace::Spacing, &[LonghandId::PaddingTop])]),
  "pr" => Utility::functional(&[value!(Length: padding_right)])
    .theme(&[(Namespace::Spacing, &[LonghandId::PaddingRight])]),
  "pb" => Utility::functional(&[value!(Length: padding_bottom)])
    .theme(&[(Namespace::Spacing, &[LonghandId::PaddingBottom])]),
  "pl" => Utility::functional(&[value!(Length: padding_left)])
    .theme(&[(Namespace::Spacing, &[LonghandId::PaddingLeft])]),
  "ps" => Utility::functional(&[value!(Length: padding_inline_start)])
    .theme(&[(Namespace::Spacing, &[LonghandId::PaddingInlineStart])]),
  "pe" => Utility::functional(&[value!(Length: padding_inline_end)])
    .theme(&[(Namespace::Spacing, &[LonghandId::PaddingInlineEnd])]),
  "inset" => Utility::functional(&[value!(-Length: top, right, bottom, left)])
    .theme(&[(
      Namespace::Spacing,
      &[
        LonghandId::Top,
        LonghandId::Right,
        LonghandId::Bottom,
        LonghandId::Left,
      ],
    )]),
  "inset-x" => Utility::functional(&[value!(-Length: left, right)])
    .theme(&[(Namespace::Spacing, &[LonghandId::Left, LonghandId::Right])]),
  "inset-y" => Utility::functional(&[value!(-Length: top, bottom)])
    .theme(&[(Namespace::Spacing, &[LonghandId::Top, LonghandId::Bottom])]),
  "top" => Utility::functional(&[value!(-Length: top)])
    .theme(&[(Namespace::Spacing, &[LonghandId::Top])]),
  "right" => Utility::functional(&[value!(-Length: right)])
    .theme(&[(Namespace::Spacing, &[LonghandId::Right])]),
  "bottom" => Utility::functional(&[value!(-Length: bottom)])
    .theme(&[(Namespace::Spacing, &[LonghandId::Bottom])]),
  "left" => Utility::functional(&[value!(-Length: left)])
    .theme(&[(Namespace::Spacing, &[LonghandId::Left])]),
  "rounded" => Utility::functional(&[rounded!(
    border_top_left_radius,
    border_top_right_radius,
    border_bottom_right_radius,
    border_bottom_left_radius
  )])
  .bare(|builder| {
    let radius = SpacePair::from_single(Length::Rem(0.25));

    builder.push(StyleDeclaration::border_top_left_radius(radius));
    builder.push(StyleDeclaration::border_top_right_radius(radius));
    builder.push(StyleDeclaration::border_bottom_right_radius(radius));
    builder.push(StyleDeclaration::border_bottom_left_radius(radius));
  })
  .theme(&[(
    Namespace::Radius,
    &[
      LonghandId::BorderTopLeftRadius,
      LonghandId::BorderTopRightRadius,
      LonghandId::BorderBottomRightRadius,
      LonghandId::BorderBottomLeftRadius,
    ],
  )]),
  "rounded-t" => Utility::functional(&[rounded!(border_top_left_radius, border_top_right_radius)])
    .theme(&[(Namespace::Radius, &[LonghandId::BorderTopLeftRadius, LonghandId::BorderTopRightRadius])]),
  "rounded-r" => Utility::functional(&[rounded!(border_top_right_radius, border_bottom_right_radius)])
    .theme(&[(Namespace::Radius, &[LonghandId::BorderTopRightRadius, LonghandId::BorderBottomRightRadius])]),
  "rounded-b" => Utility::functional(&[rounded!(border_bottom_left_radius, border_bottom_right_radius)])
    .theme(&[(Namespace::Radius, &[LonghandId::BorderBottomRightRadius, LonghandId::BorderBottomLeftRadius])]),
  "rounded-l" => Utility::functional(&[rounded!(border_top_left_radius, border_bottom_left_radius)])
    .theme(&[(Namespace::Radius, &[LonghandId::BorderTopLeftRadius, LonghandId::BorderBottomLeftRadius])]),
  "rounded-tl" => Utility::functional(&[rounded!(border_top_left_radius)])
    .theme(&[(Namespace::Radius, &[LonghandId::BorderTopLeftRadius])]),
  "rounded-tr" => Utility::functional(&[rounded!(border_top_right_radius)])
    .theme(&[(Namespace::Radius, &[LonghandId::BorderTopRightRadius])]),
  "rounded-br" => Utility::functional(&[rounded!(border_bottom_right_radius)])
    .theme(&[(Namespace::Radius, &[LonghandId::BorderBottomRightRadius])]),
  "rounded-bl" => Utility::functional(&[rounded!(border_bottom_left_radius)])
    .theme(&[(Namespace::Radius, &[LonghandId::BorderBottomLeftRadius])]),
  "grid-cols" => Utility::functional(&[value!(TwGridTemplate, |template, builder| builder
    .push(StyleDeclaration::grid_template_columns(Some(template.0))))]),
  "grid-rows" => Utility::functional(&[value!(TwGridTemplate, |template, builder| builder
    .push(StyleDeclaration::grid_template_rows(Some(template.0))))]),
  "auto-cols" => Utility::functional(&[value!(GridTrackSize, |size, builder| builder
    .push(StyleDeclaration::grid_auto_columns(Some([size].into()))))]),
  "auto-rows" => Utility::functional(&[value!(GridTrackSize, |size, builder| builder
    .push(StyleDeclaration::grid_auto_rows(Some([size].into()))))]),
  "grid-flow-row" => fixed!(grid_auto_flow(GridAutoFlow::row())),
  "grid-flow-col" => fixed!(grid_auto_flow(GridAutoFlow::column())),
  "grid-flow-row-dense" | "grid-flow-dense" => fixed!(grid_auto_flow(GridAutoFlow::row().dense())),
  "grid-flow-col-dense" => fixed!(grid_auto_flow(GridAutoFlow::column().dense())),
  "col" => Utility::functional(&[value!(GridLine, |line, builder| builder
    .push_all(StyleDeclaration::expand_grid_column(line)))]),
  "row" => Utility::functional(&[value!(GridLine, |line, builder| builder
    .push_all(StyleDeclaration::expand_grid_row(line)))]),
  "col-span" => Utility::functional(&[value!(GridPlacementSpan, |span, builder| builder
    .push_all(StyleDeclaration::expand_grid_column(GridLine::span(span))))]),
  "row-span" => Utility::functional(&[value!(GridPlacementSpan, |span, builder| builder
    .push_all(StyleDeclaration::expand_grid_row(GridLine::span(span))))]),
  "col-span-full" => Utility::fixed(|builder| builder
    .push_all(StyleDeclaration::expand_grid_column(GridLine::full()))),
  "row-span-full" => Utility::fixed(|builder| builder
    .push_all(StyleDeclaration::expand_grid_row(GridLine::full()))),
  "col-start" => Utility::functional(&[value!(-GridPlacement: grid_column_start)]),
  "col-end" => Utility::functional(&[value!(-GridPlacement: grid_column_end)]),
  "row-start" => Utility::functional(&[value!(-GridPlacement: grid_row_start)]),
  "row-end" => Utility::functional(&[value!(-GridPlacement: grid_row_end)]),
  "col-start-auto" => fixed!(grid_column_start(GridPlacement::Auto)),
  "col-end-auto" => fixed!(grid_column_end(GridPlacement::Auto)),
  "row-start-auto" => fixed!(grid_row_start(GridPlacement::Auto)),
  "row-end-auto" => fixed!(grid_row_end(GridPlacement::Auto)),
  "blur" => Utility::functional(&[value!(TwBlur, |blur, builder| builder
    .push_filter(false, "blur", &blur_css(&blur)))]),
  "brightness" => Utility::functional(&[value!(PercentageNumber, |amount, builder| builder
    .push_filter(false, "brightness", &Filter::Brightness(amount).to_css_string()))]),
  "contrast" => Utility::functional(&[value!(PercentageNumber, |amount, builder| builder
    .push_filter(false, "contrast", &Filter::Contrast(amount).to_css_string()))]),
  "grayscale" => Utility::functional(&[value!(PercentageNumber, |amount, builder| builder
    .push_filter(false, "grayscale", &Filter::Grayscale(amount).to_css_string()))])
  .bare(|builder| builder
    .push_filter(false, "grayscale", &Filter::Grayscale(PercentageNumber(1.0)).to_css_string())),
  "hue-rotate" => Utility::functional(&[value!(-Angle, |angle, builder| builder
    .push_filter(false, "hue-rotate", &Filter::HueRotate(angle).to_css_string()))]),
  "invert" => Utility::functional(&[value!(PercentageNumber, |amount, builder| builder
    .push_filter(false, "invert", &Filter::Invert(amount).to_css_string()))])
  .bare(|builder| builder
    .push_filter(false, "invert", &Filter::Invert(PercentageNumber(1.0)).to_css_string())),
  "saturate" => Utility::functional(&[value!(PercentageNumber, |amount, builder| builder
    .push_filter(false, "saturate", &Filter::Saturate(amount).to_css_string()))]),
  "sepia" => Utility::functional(&[value!(PercentageNumber, |amount, builder| builder
    .push_filter(false, "sepia", &Filter::Sepia(amount).to_css_string()))])
  .bare(|builder| builder
    .push_filter(false, "sepia", &Filter::Sepia(PercentageNumber(1.0)).to_css_string())),
  "filter" => Utility::functional(&[value!(Filters, |filters, builder| {
    if filters.is_empty() {
      builder.push_deferred(LonghandId::Filter, "none".to_owned());
    } else {
      builder.push(StyleDeclaration::filter(filters));
    }
  })]),
  "backdrop-blur" => Utility::functional(&[value!(TwBlur, |blur, builder| builder
    .push_filter(true, "blur", &blur_css(&blur)))]),
  "backdrop-brightness" => Utility::functional(&[value!(PercentageNumber, |amount, builder| builder
    .push_filter(true, "brightness", &Filter::Brightness(amount).to_css_string()))]),
  "backdrop-contrast" => Utility::functional(&[value!(PercentageNumber, |amount, builder| builder
    .push_filter(true, "contrast", &Filter::Contrast(amount).to_css_string()))]),
  "backdrop-grayscale" => Utility::functional(&[value!(PercentageNumber, |amount, builder| builder
    .push_filter(true, "grayscale", &Filter::Grayscale(amount).to_css_string()))])
  .bare(|builder| builder
    .push_filter(true, "grayscale", &Filter::Grayscale(PercentageNumber(1.0)).to_css_string())),
  "backdrop-hue-rotate" => Utility::functional(&[value!(-Angle, |angle, builder| builder
    .push_filter(true, "hue-rotate", &Filter::HueRotate(angle).to_css_string()))]),
  "backdrop-invert" => Utility::functional(&[value!(PercentageNumber, |amount, builder| builder
    .push_filter(true, "invert", &Filter::Invert(amount).to_css_string()))])
  .bare(|builder| builder
    .push_filter(true, "invert", &Filter::Invert(PercentageNumber(1.0)).to_css_string())),
  "backdrop-opacity" => Utility::functional(&[value!(PercentageNumber, |amount, builder| builder
    .push_filter(true, "opacity", &Filter::Opacity(amount).to_css_string()))]),
  "backdrop-saturate" => Utility::functional(&[value!(PercentageNumber, |amount, builder| builder
    .push_filter(true, "saturate", &Filter::Saturate(amount).to_css_string()))]),
  "backdrop-sepia" => Utility::functional(&[value!(PercentageNumber, |amount, builder| builder
    .push_filter(true, "sepia", &Filter::Sepia(amount).to_css_string()))])
  .bare(|builder| builder
    .push_filter(true, "sepia", &Filter::Sepia(PercentageNumber(1.0)).to_css_string())),
  "backdrop-filter" => Utility::functional(&[value!(Filters, |filters, builder| {
    if filters.is_empty() {
      builder.push_deferred(LonghandId::BackdropFilter, "none".to_owned());
    } else {
      builder.push(StyleDeclaration::backdrop_filter(filters));
    }
  })]),
  "drop-shadow" => Utility::functional(&[value!(TwDropShadow, drop_shadow)])
    .bare(|builder| drop_shadow(dsp("", ts(1.0, 2.0, 26)), builder)),
  "drop-shadow-xs" => Utility::fixed(|builder| drop_shadow(dsp("xs", ts(1.0, 1.0, 13)), builder)),
  "drop-shadow-sm" => Utility::fixed(|builder| drop_shadow(dsp("sm", ts(1.0, 2.0, 38)), builder)),
  "drop-shadow-md" => Utility::fixed(|builder| drop_shadow(dsp("md", ts(3.0, 3.0, 31)), builder)),
  "drop-shadow-lg" => Utility::fixed(|builder| drop_shadow(dsp("lg", ts(4.0, 4.0, 38)), builder)),
  "drop-shadow-xl" => Utility::fixed(|builder| drop_shadow(dsp("xl", ts(9.0, 7.0, 26)), builder)),
  "drop-shadow-2xl" => Utility::fixed(|builder| drop_shadow(dsp("2xl", ts(25.0, 25.0, 38)), builder)),
  "drop-shadow-none" => Utility::fixed(|builder| drop_shadow(
    TwDropShadow { shadow: ts(0.0, 0.0, 0), token: None },
    builder,
  )),
  "text-shadow" => Utility::functional(&[
    value!(TwVarColor, |color, builder| builder.push_custom("--tw-text-shadow-color", &color.0)),
    value!(TextShadow, |shadow, builder| builder
      .push_deferred(LonghandId::TextShadow, text_shadow_css(&shadow))),
  ]),
  "text-shadow-2xs" => Utility::fixed(|builder| text_shadow_preset(builder, "--text-shadow-2xs", &TEXT_SHADOW_2XS)),
  "text-shadow-xs" => Utility::fixed(|builder| text_shadow_preset(builder, "--text-shadow-xs", &TEXT_SHADOW_XS)),
  "text-shadow-sm" => Utility::fixed(|builder| text_shadow_preset(builder, "--text-shadow-sm", &TEXT_SHADOW_SM)),
  "text-shadow-md" => Utility::fixed(|builder| text_shadow_preset(builder, "--text-shadow-md", &TEXT_SHADOW_MD)),
  "text-shadow-lg" => Utility::fixed(|builder| text_shadow_preset(builder, "--text-shadow-lg", &TEXT_SHADOW_LG)),
  "text-shadow-none" => Utility::fixed(|builder| builder
    .push_deferred(LonghandId::TextShadow, "none".to_owned())),
  "mix-blend" => Utility::functional(&[value!(BlendMode: mix_blend_mode)]),
  "bg-blend" => Utility::functional(&[value!(BlendMode, |mode, builder| builder
    .push(StyleDeclaration::background_blend_mode([mode].into())))]),
  "align" => Utility::functional(&[value!(VerticalAlign: vertical_align)]),
  "animate" => Utility::functional(&[value!(TwAnimation, animation)]),
  "box-border" => fixed!(box_sizing(BoxSizing::BorderBox)),
  "box-content" => fixed!(box_sizing(BoxSizing::ContentBox)),
  "inline" => fixed!(display(Display::Inline)),
  "inline-block" => fixed!(display(Display::InlineBlock)),
  "inline-flex" => fixed!(display(Display::InlineFlex)),
  "inline-grid" => fixed!(display(Display::InlineGrid)),
  "block" => fixed!(display(Display::Block)),
  "flow-root" => fixed!(display(Display::FlowRoot)),
  "grid" => fixed!(display(Display::Grid)),
  "hidden" => fixed!(display(Display::None)),
  "list-item" => fixed!(display(Display::ListItem)),
  "absolute" => fixed!(position(Position::Absolute)),
  "relative" => fixed!(position(Position::Relative)),
  "static" => fixed!(position(Position::Static)),
  "fixed" => fixed!(position(Position::Fixed)),
  "contain-none" => fixed!(contain(Contain::NONE)),
  "contain-content" => fixed!(contain(Contain::CONTENT)),
  "contain-layout" => fixed!(contain(Contain::LAYOUT)),
  "contain-style" => fixed!(contain(Contain::STYLE)),
  "contain-paint" => fixed!(contain(Contain::PAINT)),
  "isolate" => fixed!(isolation(Isolation::Isolate)),
  "isolation-auto" => fixed!(isolation(Isolation::Auto)),
  "visible" => fixed!(visibility(Visibility::Visible)),
  "invisible" => fixed!(visibility(Visibility::Hidden)),
};

fn background_image(image: BackgroundImage, builder: &mut TailwindDeclarationBuilder) {
  builder.push(StyleDeclaration::background_image(Some([image].into())));
}

fn background_position(position: PositionValue, builder: &mut TailwindDeclarationBuilder) {
  builder.push(StyleDeclaration::background_position([position].into()));
}

fn background_size(size: BackgroundSize, builder: &mut TailwindDeclarationBuilder) {
  builder.push(StyleDeclaration::background_size([size].into()));
}

fn conic_gradient(angle: Angle, builder: &mut TailwindDeclarationBuilder) {
  let image = if *angle == 0.0 {
    "conic-gradient(in oklab, var(--tw-gradient-stops))".to_owned()
  } else {
    format!(
      "conic-gradient(from {}deg in oklab, var(--tw-gradient-stops))",
      *angle
    )
  };

  builder.push_deferred(LonghandId::BackgroundImage, image);
}

/// Sets `--tw-border-style` with the sides, so a later width utility draws in this style.
fn border_style(style: BorderStyle, builder: &mut TailwindDeclarationBuilder) {
  builder.push_custom("--tw-border-style", &style.to_css_string());
  builder.push(StyleDeclaration::border_top_style(style));
  builder.push(StyleDeclaration::border_right_style(style));
  builder.push(StyleDeclaration::border_bottom_style(style));
  builder.push(StyleDeclaration::border_left_style(style));
}

fn animation(animation: TwAnimation, builder: &mut TailwindDeclarationBuilder) {
  let TwAnimation { animations, token } = animation;

  let Some(token) = token else {
    builder.push_all(StyleDeclaration::expand_animation(animations));
    return;
  };

  let fallback = animation_shorthand_css(&animations);
  let specified_value = if fallback.is_empty() {
    format!("var(--animate-{token})")
  } else {
    format!("var(--animate-{token}, {fallback})")
  };

  builder.push(StyleDeclaration::Deferred(DeferredDeclaration {
    property: PropertyId::Shorthand(ShorthandId::Animation),
    specified_value,
  }));
}

fn drop_shadow(drop_shadow: TwDropShadow, builder: &mut TailwindDeclarationBuilder) {
  builder.push_filter(false, "drop-shadow", &drop_shadow_css(&drop_shadow));
}

fn box_shadow_preset(
  builder: &mut TailwindDeclarationBuilder,
  variable: &str,
  layers: &[BoxShadow],
) {
  builder.push_deferred(
    LonghandId::BoxShadow,
    preset_shadow_css(variable, layers.iter().map(box_shadow_css)),
  );
}

fn text_shadow_preset(
  builder: &mut TailwindDeclarationBuilder,
  variable: &str,
  layers: &[TextShadow],
) {
  builder.push_deferred(
    LonghandId::TextShadow,
    preset_shadow_css(variable, layers.iter().map(text_shadow_css)),
  );
}

/// `blur(var(--blur-md, 12px))` for a preset, `blur(2px)` for the rest, so a
/// `--blur-*` variable re-shapes the preset the way Tailwind's theme does.
fn blur_css(blur: &TwBlur) -> String {
  match blur.token {
    Some(token) => format!("blur(var(--blur-{token}, {}))", blur.radius.to_css_string()),
    None => Filter::Blur(blur.radius).to_css_string(),
  }
}

/// `var(--shadow-md, <built-in layers>)`: the variable overrides the whole
/// shape, and the fallback keeps its per-layer colour slots. A custom shape
/// carries its own colours, so `shadow-*` colour utilities only reach the
/// fallback.
fn preset_shadow_css(variable: &str, layers: impl Iterator<Item = String>) -> String {
  let fallback = layers.collect::<Vec<_>>().join(", ");

  format!("var({variable}, {fallback})")
}

/// The animation list as the shorthand text a `var()` fallback re-parses.
fn animation_shorthand_css(animations: &Animations) -> String {
  animations
    .iter()
    .filter_map(|animation| {
      let name = animation.name.as_deref()?;

      Some(format!(
        "{} {} {} {name}",
        animation.duration.to_css_string(),
        animation.timing_function.to_css_string(),
        animation.iteration_count.to_css_string(),
      ))
    })
    .collect::<Vec<_>>()
    .join(", ")
}

/// `drop-shadow(var(--drop-shadow-md, …))` for a preset; the bare `drop-shadow`
/// utility reads `--drop-shadow` itself.
fn drop_shadow_css(drop_shadow: &TwDropShadow) -> String {
  match drop_shadow.token {
    Some("") => format!(
      "drop-shadow(var(--drop-shadow, {}))",
      drop_shadow.shadow.to_css_string()
    ),
    Some(token) => format!(
      "drop-shadow(var(--drop-shadow-{token}, {}))",
      drop_shadow.shadow.to_css_string()
    ),
    None => Filter::DropShadow(drop_shadow.shadow).to_css_string(),
  }
}

/// One shadow layer with its colour behind `var()`, as Tailwind compiles it:
/// the colour utility overrides through the variable, the layer's own colour
/// stays as the fallback.
fn shadow_layer_css(
  prefix: &str,
  offsets: &[&Length],
  color: &ColorInput,
  variable: &str,
) -> String {
  let mut out = String::from(prefix);

  for length in offsets {
    out.push_str(&length.to_css_string());
    out.push(' ');
  }

  out.push_str(&format!("var({variable}, {})", color.to_css_string()));
  out
}

fn box_shadow_css(shadow: &BoxShadow) -> String {
  shadow_layer_css(
    if shadow.inset { "inset " } else { "" },
    &[
      &shadow.offset_x,
      &shadow.offset_y,
      &shadow.blur_radius,
      &shadow.spread_radius,
    ],
    &shadow.color,
    "--tw-shadow-color",
  )
}

fn text_shadow_css(shadow: &TextShadow) -> String {
  shadow_layer_css(
    "",
    &[&shadow.offset_x, &shadow.offset_y, &shadow.blur_radius],
    &shadow.color,
    "--tw-text-shadow-color",
  )
}

// v4 `theme.css` composites. Alpha: `/ .075` ≈ 19, `/ .1` ≈ 26, `/ .25` ≈ 64.
const fn bs(inset: bool, oy: f32, blur: f32, spread: f32, alpha: u8) -> BoxShadow {
  BoxShadow {
    inset,
    offset_x: Length::Px(0.0),
    offset_y: Length::Px(oy),
    blur_radius: Length::Px(blur),
    spread_radius: Length::Px(spread),
    color: ColorInput::Value(Color([0, 0, 0, alpha])),
  }
}
const fn dsp(token: &'static str, shadow: TextShadow) -> TwDropShadow {
  TwDropShadow {
    shadow,
    token: Some(token),
  }
}
const fn ts(oy: f32, blur: f32, alpha: u8) -> TextShadow {
  TextShadow {
    offset_x: Length::Px(0.0),
    offset_y: Length::Px(oy),
    blur_radius: Length::Px(blur),
    color: ColorInput::Value(Color([0, 0, 0, alpha])),
  }
}

const SHADOW_2XS: [BoxShadow; 1] = [bs(false, 1.0, 0.0, 0.0, 13)];
const SHADOW_XS: [BoxShadow; 1] = [bs(false, 1.0, 2.0, 0.0, 13)];
const SHADOW_2XL: [BoxShadow; 1] = [bs(false, 25.0, 50.0, -12.0, 64)];
const INSET_SHADOW_2XS: [BoxShadow; 1] = [bs(true, 1.0, 0.0, 0.0, 13)];
const INSET_SHADOW_XS: [BoxShadow; 1] = [bs(true, 1.0, 1.0, 0.0, 13)];
const INSET_SHADOW_SM: [BoxShadow; 1] = [bs(true, 2.0, 4.0, 0.0, 13)];
const TEXT_SHADOW_2XS: [TextShadow; 1] = [ts(1.0, 0.0, 38)];
const TEXT_SHADOW_XS: [TextShadow; 1] = [ts(1.0, 1.0, 51)];
const SHADOW_SM: [BoxShadow; 2] = [bs(false, 1.0, 3.0, 0.0, 26), bs(false, 1.0, 2.0, -1.0, 26)];
const SHADOW_MD: [BoxShadow; 2] = [bs(false, 4.0, 6.0, -1.0, 26), bs(false, 2.0, 4.0, -2.0, 26)];
const SHADOW_LG: [BoxShadow; 2] = [
  bs(false, 10.0, 15.0, -3.0, 26),
  bs(false, 4.0, 6.0, -4.0, 26),
];
const SHADOW_XL: [BoxShadow; 2] = [
  bs(false, 20.0, 25.0, -5.0, 26),
  bs(false, 8.0, 10.0, -6.0, 26),
];

const TEXT_SHADOW_SM: [TextShadow; 3] = [ts(1.0, 0.0, 19), ts(1.0, 1.0, 19), ts(2.0, 2.0, 19)];
const TEXT_SHADOW_MD: [TextShadow; 3] = [ts(1.0, 1.0, 26), ts(1.0, 2.0, 26), ts(2.0, 4.0, 26)];
const TEXT_SHADOW_LG: [TextShadow; 3] = [ts(1.0, 2.0, 26), ts(3.0, 2.0, 26), ts(4.0, 8.0, 26)];
