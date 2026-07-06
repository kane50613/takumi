use takumi::prelude::{Length::*, *};

use crate::test_utils::run_fixture_test;

/// Han code points whose preferred glyph differs by language. The test font is a
/// subset of Source Han Sans carrying the `locl` variants, so each row draws the
/// language's own glyph: the same code points, shaped under `ja` / `zh` / `ko`.
const SAMPLE: &str = "直 骨 今 海 真 令 説 器";

const FONT: &str = "CJK Locl Test";

fn lang_row(lang: Lang) -> Result<Node> {
  Ok(
    Node::container([
      Node::text(format!("{}: ", lang.as_str())).with_style(
        Style::default()
          .with(StyleDeclaration::font_family(
            FontFamily::from_css_str("Geist").unwrap(),
          ))
          .with(StyleDeclaration::font_size(FontSize::Length(Px(40.0)))),
      ),
      Node::text(SAMPLE).with_lang(lang).with_style(
        Style::default()
          .with(StyleDeclaration::font_family(
            FontFamily::from_css_str(FONT).unwrap(),
          ))
          .with(StyleDeclaration::font_size(FontSize::Length(Px(40.0)))),
      ),
    ])
    .with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Flex))
        .with(StyleDeclaration::align_items(AlignItems::Center))
        .with_gap(SpacePair::from_pair(Px(12.0).into(), Px(12.0).into())),
    ),
  )
}

#[test]
fn test_lang_han_unification() -> Result<()> {
  let rows = ["ja", "zh", "ko"]
    .iter()
    .map(|lang| Lang::parse(lang).and_then(lang_row))
    .collect::<Result<Vec<_>>>()?;

  let container = Node::container(rows).with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::flex_direction(FlexDirection::Column))
      .with(StyleDeclaration::background_color(ColorInput::Value(
        Color([255, 255, 255, 255]),
      )))
      .with(StyleDeclaration::width(Percentage(100.0)))
      .with(StyleDeclaration::height(Percentage(100.0)))
      .with_padding(Sides([Px(40.0); 4]))
      .with(StyleDeclaration::justify_content(JustifyContent::Center))
      .with_gap(SpacePair::from_pair(Px(32.0).into(), Px(32.0).into())),
  );

  Ok(run_fixture_test(container, "lang_han_unification"))
}
