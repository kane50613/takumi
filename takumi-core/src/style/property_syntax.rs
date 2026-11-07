//! The `syntax` an `@property` rule registers: the values its custom property takes, what they
//! compute to, and how they interpolate.
//! <https://drafts.css-houdini.org/css-properties-values-api-1/#syntax-strings>

use std::{
  fmt::{self, Display, Formatter},
  str::FromStr,
};

use cssparser::{Parser, Token, match_ignore_ascii_case, serialize_identifier, serialize_string};

use crate::{
  geometry::Size,
  layout::layout_style::LayoutLength,
  style::{
    Animatable, AnimationTime, BackgroundImage, Color, ColorInput, CssWideKeyword, FromCss,
    FromCssStr, Length, MakeComputed, SizingContext, ToCss, Transform, Transforms,
    contains_var_references, is_math_function, lerp, parse_angle, parse_calc_number_expression,
    parse_percentage_value,
  },
  viewport::Viewport,
};

/// The values a registered custom property takes.
#[derive(Debug, Clone, PartialEq)]
pub enum PropertySyntax {
  /// `*`, which takes any value, as an unregistered custom property does.
  Universal,
  /// The `|`-separated components, tried in order.
  Components(Box<[SyntaxComponent]>),
}

/// One component of a syntax: a data type or a keyword, alone or as a list.
#[derive(Debug, Clone, PartialEq)]
pub struct SyntaxComponent {
  name: ComponentName,
  multiplier: Option<Multiplier>,
}

#[derive(Debug, Clone, PartialEq)]
enum ComponentName {
  Type(DataType),
  Keyword(Box<str>),
}

/// The supported syntax component names.
/// <https://drafts.css-houdini.org/css-properties-values-api-1/#supported-names>
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DataType {
  Angle,
  Color,
  CustomIdent,
  Image,
  Integer,
  Length,
  LengthPercentage,
  Number,
  Percentage,
  Resolution,
  String,
  Time,
  TransformFunction,
  TransformList,
  Url,
}

const DATA_TYPES: [(&str, DataType); 15] = [
  ("angle", DataType::Angle),
  ("color", DataType::Color),
  ("custom-ident", DataType::CustomIdent),
  ("image", DataType::Image),
  ("integer", DataType::Integer),
  ("length", DataType::Length),
  ("length-percentage", DataType::LengthPercentage),
  ("number", DataType::Number),
  ("percentage", DataType::Percentage),
  ("resolution", DataType::Resolution),
  ("string", DataType::String),
  ("time", DataType::Time),
  ("transform-function", DataType::TransformFunction),
  ("transform-list", DataType::TransformList),
  ("url", DataType::Url),
];

/// `+` for a space-separated list, `#` for a comma-separated one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Multiplier {
  Space,
  Comma,
}

/// One computed item of a value: a number in its canonical unit, or text kept as specified.
#[derive(Debug, Clone)]
enum Item {
  /// CSS px.
  Length(f32),
  /// CSS px plus a percentage.
  LengthPercentage {
    px: f32,
    percent: f32,
  },
  Number(f32),
  Integer(f32),
  Percentage(f32),
  /// Degrees.
  Angle(f32),
  /// Seconds.
  Time(f32),
  /// Dots per CSS px.
  Resolution(f32),
  Color(ColorInput),
  Transforms(Transforms),
  Specified(String),
}

impl FromStr for PropertySyntax {
  type Err = ();

  /// Blink's `CSSSyntaxStringParser::Parse`.
  fn from_str(source: &str) -> Result<Self, ()> {
    let source = source.trim_matches(|c: char| c.is_ascii_whitespace());

    if source == "*" {
      return Ok(Self::Universal);
    }

    source
      .split('|')
      .map(SyntaxComponent::parse)
      .collect::<Result<_, _>>()
      .map(Self::Components)
  }
}

impl Display for PropertySyntax {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    let Self::Components(components) = self else {
      return f.write_str("*");
    };

    for (index, component) in components.iter().enumerate() {
      if index > 0 {
        f.write_str(" | ")?;
      }
      match &component.name {
        ComponentName::Type(data_type) => write!(f, "<{}>", data_type.name())?,
        ComponentName::Keyword(keyword) => f.write_str(keyword)?,
      }
      match component.multiplier {
        Some(Multiplier::Space) => f.write_str("+")?,
        Some(Multiplier::Comma) => f.write_str("#")?,
        None => {}
      }
    }
    Ok(())
  }
}

impl PropertySyntax {
  /// `value` computed against `sizing`, or normalized as specified without one. `None` when the
  /// syntax does not take it.
  pub(crate) fn compute(&self, value: &str, sizing: Option<&SizingContext>) -> Option<String> {
    let Self::Components(components) = self else {
      return Some(value.to_owned());
    };
    let (component, items) = components
      .iter()
      .find_map(|component| Some((component, component.parse_value(value, sizing)?)))?;

    Some(component.serialize(&items))
  }

  /// Whether `value` computes without the element: no `var()`, and nothing relative to its font,
  /// its viewport, or a query container.
  /// <https://drafts.css-houdini.org/css-properties-values-api-1/#computationally-independent>
  pub(crate) fn is_computationally_independent(&self, value: &str) -> bool {
    if *self == Self::Universal {
      return true;
    }
    if contains_var_references(value) {
      return false;
    }

    let [small, large] = [probe_sizing(100, 10.0), probe_sizing(300, 30.0)];
    let computed = self.compute(value, Some(&small));

    computed.is_some() && computed == self.compute(value, Some(&large))
  }

  /// `progress` of the way from `from` to `to`, item by item, when both take the same component
  /// and every item interpolates. `None` leaves the value to switch half way.
  pub(crate) fn interpolate(
    &self,
    from: &str,
    to: &str,
    progress: f32,
    sizing: &SizingContext,
    current_color: Color,
  ) -> Option<String> {
    let Self::Components(components) = self else {
      return None;
    };
    let matched = |value: &str| {
      components
        .iter()
        .enumerate()
        .find_map(|(index, component)| Some((index, component.parse_value(value, Some(sizing))?)))
    };
    let ((from_index, from_items), (to_index, to_items)) = (matched(from)?, matched(to)?);

    if from_index != to_index || from_items.len() != to_items.len() {
      return None;
    }

    let items = from_items
      .iter()
      .zip(&to_items)
      .map(|(from, to)| from.interpolate(to, progress, sizing, current_color))
      .collect::<Option<Vec<_>>>()?;

    Some(components[from_index].serialize(&items))
  }
}

/// A sizing whose font, viewport and container sizes all differ from another's, so a value that
/// reads any of them computes differently against each.
fn probe_sizing(size: u32, font_size: f32) -> SizingContext {
  let mut sizing = SizingContext::builder()
    .viewport(Viewport::new((size, size)))
    .build();

  sizing.font_size = font_size;
  sizing.root_font_size = Some(font_size);
  sizing.line_height = font_size;
  sizing.root_line_height = Some(font_size);
  sizing.ch = font_size;
  sizing.ex = font_size;
  sizing.container_size = Size {
    width: Some(size as f32),
    height: Some(size as f32),
  };
  sizing
}

/// `value` rounded to the nearest integer, a half up toward positive infinity, as CSS rounds an
/// `<integer>`.
/// <https://drafts.csswg.org/css-values-4/#combine-integers>
fn round_integer(value: f32) -> f32 {
  (value + 0.5).floor()
}

/// Whether `ident` is reserved, so neither a keyword of a syntax nor a `<custom-ident>`.
/// <https://drafts.csswg.org/css-values-4/#custom-idents>
fn is_reserved(ident: &str) -> bool {
  CssWideKeyword::from_css_str(ident).is_ok()
    || ["default", "revert", "revert-layer"]
      .iter()
      .any(|reserved| ident.eq_ignore_ascii_case(reserved))
}

impl SyntaxComponent {
  fn parse(source: &str) -> Result<Self, ()> {
    let source = source.trim_matches(|c: char| c.is_ascii_whitespace());
    let (body, multiplier) = match source.as_bytes().last() {
      Some(b'+') => (&source[..source.len() - 1], Some(Multiplier::Space)),
      Some(b'#') => (&source[..source.len() - 1], Some(Multiplier::Comma)),
      _ => (source, None),
    };
    let name = match body
      .strip_prefix('<')
      .and_then(|body| body.strip_suffix('>'))
    {
      Some(name) => ComponentName::Type(DataType::named(name).ok_or(())?),
      None if is_keyword(body) => ComponentName::Keyword(body.into()),
      None => return Err(()),
    };

    // A transform list is a list already.
    if name == ComponentName::Type(DataType::TransformList) && multiplier.is_some() {
      return Err(());
    }

    Ok(Self { name, multiplier })
  }

  /// The items `value` computes to under this component, or `None` when it does not match.
  fn parse_value(&self, value: &str, sizing: Option<&SizingContext>) -> Option<Vec<Item>> {
    let mut input = Parser::new(value);
    let mut items = Vec::new();

    match self.multiplier {
      None => items.push(self.parse_item(&mut input, sizing)?),
      Some(Multiplier::Space) => loop {
        items.push(self.parse_item(&mut input, sizing)?);
        if input.is_exhausted() {
          break;
        }
      },
      Some(Multiplier::Comma) => loop {
        items.push(self.parse_item(&mut input, sizing)?);
        if input.is_exhausted() {
          break;
        }
        input.expect_comma().ok()?;
      },
    }

    input.is_exhausted().then_some(items)
  }

  fn parse_item(&self, input: &mut Parser<'_>, sizing: Option<&SizingContext>) -> Option<Item> {
    match &self.name {
      ComponentName::Keyword(keyword) => {
        let ident = input.expect_ident().ok()?;

        (**ident == **keyword).then(|| Item::Specified(keyword.to_string()))
      }
      ComponentName::Type(data_type) => input
        .try_parse(|input| data_type.parse_item(input, sizing).ok_or(()))
        .ok(),
    }
  }

  fn serialize(&self, items: &[Item]) -> String {
    let separator = match self.multiplier {
      Some(Multiplier::Comma) => ", ",
      _ => " ",
    };

    items
      .iter()
      .map(ToString::to_string)
      .collect::<Vec<_>>()
      .join(separator)
  }
}

/// Whether `source` is an identifier a syntax may name as a keyword.
fn is_keyword(source: &str) -> bool {
  let mut input = Parser::new(source);

  input.expect_ident().is_ok() && input.is_exhausted() && !is_reserved(source)
}

impl DataType {
  fn named(name: &str) -> Option<Self> {
    DATA_TYPES
      .iter()
      .find(|(known, _)| *known == name)
      .map(|&(_, data_type)| data_type)
  }

  fn name(self) -> &'static str {
    DATA_TYPES
      .iter()
      .find(|&&(_, data_type)| data_type == self)
      .map_or("", |&(name, _)| name)
  }

  fn parse_item(self, input: &mut Parser<'_>, sizing: Option<&SizingContext>) -> Option<Item> {
    let start = input.position();
    let specified = |input: &Parser<'_>| Item::Specified(input.slice_from(start).trim().to_owned());

    match self {
      Self::Length | Self::LengthPercentage => {
        let percentage = self == Self::LengthPercentage;
        let length = Length::from_css(input).ok()?;
        // `Length` reads a unitless number as px, which only zero is.
        let unitless = input
          .slice_from(start)
          .trim()
          .parse::<f32>()
          .is_ok_and(|number| number != 0.0);

        if unitless || !is_length(&length, percentage) {
          return None;
        }

        Some(length_item(length, percentage, sizing).unwrap_or_else(|| specified(input)))
      }
      Self::Number => number(input).map(Item::Number),
      Self::Integer => match input.try_parse(Parser::expect_integer) {
        Ok(integer) => Some(Item::Integer(integer as f32)),
        Err(_) => parse_calc_number_expression(input)
          .ok()
          .map(|value| Item::Integer(round_integer(value))),
      },
      Self::Percentage => parse_percentage_value(input)
        .ok()
        .map(|fraction| Item::Percentage(fraction * 100.0)),
      Self::Angle => match input.next().ok()?.clone() {
        // Not `Angle`, which wraps to a turn and takes a unitless number.
        Token::Function(name) if is_math_function(&name) => {
          parse_angle(&name, input).ok().map(Item::Angle)
        }
        Token::Dimension { value, unit, .. } => {
          let degrees = match_ignore_ascii_case! { &unit,
            "deg" => value,
            "grad" => value * 0.9,
            "rad" => value.to_degrees(),
            "turn" => value * 360.0,
            _ => return None,
          };

          Some(Item::Angle(degrees))
        }
        _ => None,
      },
      Self::Time => AnimationTime::from_css(input)
        .ok()
        .map(|time| Item::Time(time.milliseconds / 1000.0)),
      Self::Resolution => match input.next().ok()? {
        Token::Dimension { value, unit, .. } if *value >= 0.0 => {
          let dppx = match_ignore_ascii_case! { unit,
            "dppx" | "x" => *value,
            "dpi" => value / 96.0,
            "dpcm" => value * 2.54 / 96.0,
            _ => return None,
          };

          Some(Item::Resolution(dppx))
        }
        _ => None,
      },
      Self::Color => ColorInput::from_css(input).ok().map(Item::Color),
      Self::Image => {
        if input
          .try_parse(|input| input.expect_ident_matching("none"))
          .is_ok()
        {
          return None;
        }

        let image = BackgroundImage::from_css(input).ok()?;
        let Some(sizing) = sizing else {
          return Some(specified(input));
        };
        let (mut computed, mut before, mut after) = (image.clone(), String::new(), String::new());

        computed.make_computed(sizing);
        image.to_css(&mut before).ok()?;
        computed.to_css(&mut after).ok()?;

        // Only a relative length changes in computing, and the rest stays as written, as Chromium
        // keeps it.
        Some(if before == after {
          specified(input)
        } else {
          Item::Specified(after)
        })
      }
      Self::Url => {
        let url = match input.next().ok()? {
          Token::UnquotedUrl(url) => url.clone(),
          Token::Function(name) if name.eq_ignore_ascii_case("url") => input
            .parse_nested_block(|input| {
              Ok::<_, cssparser::ParseError<()>>(input.expect_string()?.clone())
            })
            .ok()?,
          _ => return None,
        };

        Some(Item::Specified(format!("url({})", quoted(&url))))
      }
      Self::TransformFunction | Self::TransformList => {
        if input
          .try_parse(|input| input.expect_ident_matching("none"))
          .is_ok()
        {
          return None;
        }

        let mut transforms = if self == Self::TransformFunction {
          Transforms(Box::new([Transform::from_css(input).ok()?]))
        } else {
          Transforms::from_css(input).ok()?
        };

        if let Some(sizing) = sizing {
          transforms.make_computed(sizing);
        }
        Some(Item::Transforms(transforms))
      }
      Self::CustomIdent => {
        let ident = input.expect_ident().ok()?;
        let mut serialized = String::new();

        if is_reserved(ident) {
          return None;
        }
        serialize_identifier(ident, &mut serialized).ok()?;
        Some(Item::Specified(serialized))
      }
      Self::String => Some(Item::Specified(quoted(input.expect_string().ok()?))),
    }
  }
}

/// `text` as a CSS string, double-quoted as Blink serializes one.
fn quoted(text: &str) -> String {
  let mut quoted = String::new();
  let _ = serialize_string(text, &mut quoted);

  quoted
}

/// A `<number>`, or a `calc()` that reduces to one.
fn number(input: &mut Parser<'_>) -> Option<f32> {
  input
    .try_parse(Parser::expect_number)
    .ok()
    .or_else(|| parse_calc_number_expression(input).ok())
}

/// Whether `length` is a length, or a length or a percentage when `percentage` allows one.
fn is_length(length: &Length, percentage: bool) -> bool {
  match length {
    Length::Auto => false,
    Length::Percentage(_) => percentage,
    Length::Calc(formula) => {
      percentage || formula.resolve(&probe_sizing(100, 10.0)).components().1 == 0.0
    }
    Length::Math(expr) => percentage || !expr.has_percent(),
    _ => true,
  }
}

/// `length` in CSS px, with any percentage kept beside it, or `None` without a `sizing` it
/// resolves against.
fn length_item(length: Length, percentage: bool, sizing: Option<&SizingContext>) -> Option<Item> {
  let sizing = sizing?;

  // A math function that needs the percentage basis computes its other units and stays a function.
  if let Length::Math(expr) = length
    && expr.has_percent()
  {
    // A full interner leaves no tree to compute into, so the value is invalid rather than `auto`.
    let computed = expr.make_computed(sizing);

    if computed == Length::Auto {
      return None;
    }

    let mut serialized = String::new();

    computed.to_css(&mut serialized).ok()?;
    return Some(Item::Specified(serialized));
  }

  match length.to_layout_length(sizing) {
    LayoutLength::Fixed(px) if percentage => Some(Item::LengthPercentage {
      px: sizing.to_css(px),
      percent: 0.0,
    }),
    LayoutLength::Fixed(px) => Some(Item::Length(sizing.to_css(px))),
    LayoutLength::Percent(fraction) if percentage => Some(Item::LengthPercentage {
      px: 0.0,
      percent: fraction * 100.0,
    }),
    LayoutLength::Calc { px, percent } if percentage => Some(Item::LengthPercentage {
      px: sizing.to_css(px),
      percent: percent * 100.0,
    }),
    _ => None,
  }
}

impl Item {
  fn interpolate(
    &self,
    to: &Self,
    progress: f32,
    sizing: &SizingContext,
    current_color: Color,
  ) -> Option<Self> {
    let mix = |from: f32, to: f32| lerp(from, to, progress);

    Some(match (self, to) {
      (Self::Length(from), Self::Length(to)) => Self::Length(mix(*from, *to)),
      (
        Self::LengthPercentage { px, percent },
        Self::LengthPercentage {
          px: to_px,
          percent: to_percent,
        },
      ) => Self::LengthPercentage {
        px: mix(*px, *to_px),
        percent: mix(*percent, *to_percent),
      },
      (Self::Number(from), Self::Number(to)) => Self::Number(mix(*from, *to)),
      (Self::Integer(from), Self::Integer(to)) => Self::Integer(round_integer(mix(*from, *to))),
      (Self::Percentage(from), Self::Percentage(to)) => Self::Percentage(mix(*from, *to)),
      (Self::Angle(from), Self::Angle(to)) => Self::Angle(mix(*from, *to)),
      (Self::Time(from), Self::Time(to)) => Self::Time(mix(*from, *to)),
      (Self::Resolution(from), Self::Resolution(to)) => Self::Resolution(mix(*from, *to)),
      (Self::Color(from), Self::Color(to)) => Self::Color(ColorInput::interpolated(
        from,
        to,
        progress,
        sizing,
        current_color,
      )),
      (Self::Transforms(from), Self::Transforms(to)) => Self::Transforms(Transforms::interpolated(
        from,
        to,
        progress,
        sizing,
        current_color,
      )),
      _ => return None,
    })
  }
}

/// `alpha` to two decimals when they keep its 8-bit value, and to three otherwise, as Blink
/// serializes the alpha of a color.
fn serialized_alpha(alpha: u8) -> f32 {
  let fraction = f32::from(alpha) / 255.0;
  let two = (fraction * 100.0).round() / 100.0;

  if (two * 255.0).round() as u8 == alpha {
    two
  } else {
    (fraction * 1000.0).round() / 1000.0
  }
}

impl Display for Item {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    let number = |f: &mut Formatter<'_>, value: f32, unit: &str| {
      value.to_css(f)?;
      f.write_str(unit)
    };

    match self {
      Self::Length(px) => number(f, *px, "px"),
      Self::LengthPercentage { px, percent } if *percent == 0.0 => number(f, *px, "px"),
      Self::LengthPercentage { px, percent } if *px == 0.0 => number(f, *percent, "%"),
      Self::LengthPercentage { px, percent } => {
        f.write_str("calc(")?;
        number(f, *percent, "%")?;
        f.write_str(if *px < 0.0 { " - " } else { " + " })?;
        number(f, px.abs(), "px)")
      }
      Self::Number(value) | Self::Integer(value) => number(f, *value, ""),
      Self::Percentage(value) => number(f, *value, "%"),
      Self::Angle(degrees) => number(f, *degrees, "deg"),
      Self::Time(seconds) => number(f, *seconds, "s"),
      Self::Resolution(dppx) => number(f, *dppx, "dppx"),
      Self::Color(ColorInput::CurrentColor) => f.write_str("currentcolor"),
      Self::Color(ColorInput::Value(Color([r, g, b, 255]))) => write!(f, "rgb({r}, {g}, {b})"),
      Self::Color(ColorInput::Value(Color([r, g, b, alpha]))) => {
        write!(f, "rgba({r}, {g}, {b}, ")?;
        number(f, serialized_alpha(*alpha), ")")
      }
      Self::Transforms(transforms) => transforms.to_css(f),
      Self::Specified(text) => f.write_str(text),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::PropertySyntax;
  use crate::{
    style::{Color, SizingContext},
    viewport::Viewport,
  };

  /// Each value computed as `getComputedStyle` reads it in Chromium, at a font size of 20px in a
  /// viewport 1000px wide.
  #[test]
  fn computes_as_chromium() {
    let mut sizing = SizingContext::builder()
      .viewport(Viewport::new((1000, 800)))
      .build();

    sizing.font_size = 20.0;

    let cases = [
      ("<length>", "2em", "40px"),
      ("<length>", "calc(1in + 2px)", "98px"),
      ("<length>", "10vw", "100px"),
      ("<length-percentage>", "calc(10px + 5%)", "calc(5% + 10px)"),
      ("<length-percentage>", "50%", "50%"),
      ("<length-percentage>", "1em", "20px"),
      ("<number>", "calc(1 + 0.5)", "1.5"),
      ("<integer>", "calc(2.6)", "3"),
      ("<integer>", "3", "3"),
      ("<percentage>", "50%", "50%"),
      ("<angle>", "0.25turn", "90deg"),
      ("<angle>", "100grad", "90deg"),
      ("<time>", "150ms", "0.15s"),
      ("<resolution>", "192dpi", "2dppx"),
      ("<color>", "blue", "rgb(0, 0, 255)"),
      ("<color>", "#00ff0080", "rgba(0, 255, 0, 0.5)"),
      ("<color>", "currentcolor", "currentcolor"),
      (
        "<transform-function>",
        "translate(1em, 20px)",
        "translate(20px, 20px)",
      ),
      // Takumi keeps `scale(2)` as the `scale(2, 2)` it parses to.
      (
        "<transform-list>",
        "rotate(0.5turn) scale(2)",
        "rotate(180deg) scale(2, 2)",
      ),
      ("<custom-ident>", "foo", "foo"),
      ("<string>", "'hi'", "\"hi\""),
      // Chromium resolves the URL against the document, which a render has none of.
      ("<url>", "url(b.png)", "url(\"b.png\")"),
      ("<length>+", "1px 2em", "1px 40px"),
      ("<length>#", "1px, 2em", "1px, 40px"),
      ("auto | <length>", "auto", "auto"),
      ("auto | <length>", "3em", "60px"),
      (
        "<image>",
        "linear-gradient(red, blue)",
        "linear-gradient(red, blue)",
      ),
    ];
    let mismatches: Vec<_> = cases
      .iter()
      .filter_map(|&(syntax, value, chromium)| {
        let syntax: PropertySyntax = syntax.parse().expect("the syntax parses");
        let computed = syntax.compute(value, Some(&sizing));

        (computed.as_deref() != Some(chromium))
          .then(|| format!("{syntax} {value}: {computed:?}, Chromium {chromium}"))
      })
      .collect();

    assert!(mismatches.is_empty(), "{mismatches:#?}");
  }

  /// The edge cases of each type: CSS rounding, math that reduces to a percentage, a negative
  /// resolution, an escaped identifier, and the reserved keywords.
  #[test]
  fn computes_the_edge_cases() {
    let compute = |syntax: &str, value: &str| {
      syntax
        .parse::<PropertySyntax>()
        .expect("the syntax parses")
        .compute(value, None)
    };

    assert_eq!(compute("<integer>", "calc(-1.5)").as_deref(), Some("-1"));
    assert_eq!(compute("<integer>", "calc(2.5)").as_deref(), Some("3"));
    assert_eq!(
      compute("<percentage>", "calc(25% + 25%)").as_deref(),
      Some("50%")
    );
    assert_eq!(compute("<percentage>", "calc(10px + 5%)"), None);
    assert_eq!(compute("<percentage>", "calc(1em - 10px)"), None);
    assert_eq!(compute("<percentage>", "calc(1em - 1em + 5%)"), None);
    assert_eq!(compute("<resolution>", "-1dppx"), None);
    assert_eq!(
      compute("<custom-ident>", r"\31 foo").as_deref(),
      Some(r"\31 foo")
    );
    assert_eq!(compute("<custom-ident>", "revert-layer"), None);
    assert!("revert".parse::<PropertySyntax>().is_err());
    assert!("REVERT-LAYER | <length>".parse::<PropertySyntax>().is_err());
  }

  /// Math with absolute lengths computes without the element, and a gradient stop in `em` does
  /// not.
  #[test]
  fn tells_which_values_compute_without_the_element() {
    let independent = |syntax: &str, value: &str| {
      syntax
        .parse::<PropertySyntax>()
        .expect("the syntax parses")
        .is_computationally_independent(value)
    };

    assert!(independent("<length>", "calc(1px + 1px)"));
    assert!(independent("<image>", "linear-gradient(red 10px, blue)"));
    assert!(!independent("<image>", "linear-gradient(red 1em, blue)"));
    assert!(!independent("<length>", "10cqw"));
  }

  /// A registered value takes the math functions, computed as Chromium computes them.
  #[test]
  fn computes_math_as_chromium() {
    let compute = |syntax: &str, value: &str, font_size: f32| {
      let mut sizing = SizingContext::builder()
        .viewport(Viewport::new((1000, 800)))
        .build();

      sizing.font_size = font_size;
      syntax
        .parse::<PropertySyntax>()
        .expect("the syntax parses")
        .compute(value, Some(&sizing))
    };

    assert_eq!(
      compute("<angle>", "calc(1turn / 4)", 20.0).as_deref(),
      Some("90deg")
    );
    assert_eq!(
      compute("<angle>", "atan2(1, 1)", 16.0).as_deref(),
      Some("45deg")
    );
    assert_eq!(
      compute("<length>", "min(10px, 2em)", 20.0).as_deref(),
      Some("10px")
    );
    assert_eq!(compute("<length>", "min(10px, 20%)", 16.0), None);
    assert_eq!(
      compute("<length-percentage>", "min(10px, 20%)", 20.0).as_deref(),
      Some("min(10px, 20%)")
    );
    assert_eq!(
      compute("<length-percentage>", "clamp(1em, 5%, 30px)", 16.0).as_deref(),
      Some("clamp(16px, 5%, 30px)")
    );
    assert_eq!(
      compute("<integer>", "round(7.5)", 16.0).as_deref(),
      Some("8")
    );
    assert_eq!(
      compute("<integer>", "max(2, 3)", 16.0).as_deref(),
      Some("3")
    );
  }

  /// A turn and more interpolates through every angle on the way, as Chromium animates it.
  #[test]
  fn an_angle_interpolates_past_a_turn() {
    let syntax: PropertySyntax = "<angle>".parse().expect("the syntax parses");
    let sizing = SizingContext::builder()
      .viewport(Viewport::new((100, 100)))
      .build();
    let at = |progress| syntax.interpolate("0deg", "720deg", progress, &sizing, Color::black());

    assert_eq!(at(0.25).as_deref(), Some("180deg"));
    assert_eq!(at(0.75).as_deref(), Some("540deg"));
    assert_eq!(
      "<integer>"
        .parse::<PropertySyntax>()
        .expect("the syntax parses")
        .interpolate("-2", "-1", 0.5, &sizing, Color::black())
        .as_deref(),
      Some("-1")
    );
    assert_eq!(syntax.compute("0", None), None);
    assert_eq!(syntax.compute("none", None), None);
  }
}
