use takumi::prelude::{Length::*, *};

use crate::test_utils::run_fixture_test;

/// Han code points whose preferred glyph differs by language. The test font is a
/// subset of Source Han Sans carrying the `locl` variants, so each row draws the
/// language's own glyph: the same code points, shaped under `ja` / `zh` / `ko`.
const SAMPLE: &str = "直 骨 今 海 真 令 説 器";

const FONT: &str = "CJK Locl Test";

fn lang_row(lang: &'static str) -> Node {
  Node::container([
    Node::text(format!("{lang}: ")).with_style(
      Style::default()
        .with(StyleDeclaration::font_family(
          FontFamily::from_str("Geist").unwrap(),
        ))
        .with(StyleDeclaration::font_size(FontSize::Length(Px(40.0)))),
    ),
    Node::text(SAMPLE).with_lang(lang).with_style(
      Style::default()
        .with(StyleDeclaration::font_family(
          FontFamily::from_str(FONT).unwrap(),
        ))
        .with(StyleDeclaration::font_size(FontSize::Length(Px(40.0)))),
    ),
  ])
  .with_style(
    Style::default()
      .with(StyleDeclaration::display(Display::Flex))
      .with(StyleDeclaration::align_items(AlignItems::Center))
      .with_gap(SpacePair::from_pair(Px(12.0), Px(12.0))),
  )
}

#[test]
fn test_lang_han_unification() {
  let container = Node::container(["ja", "zh", "ko"].map(lang_row)).with_style(
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
      .with_gap(SpacePair::from_pair(Px(32.0), Px(32.0))),
  );

  run_fixture_test(container, "lang_han_unification");
}
