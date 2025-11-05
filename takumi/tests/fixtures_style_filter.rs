use serde_json::{Value, from_value};
use takumi::layout::{
  node::{ContainerNode, ImageNode, NodeKind, TextNode},
  style::{LengthUnit::*, *},
};

mod test_utils;
use test_utils::run_style_width_test;

#[test]
fn test_style_filter_on_image_node() {
  let effects = [
    "grayscale(75%)",
    "opacity(30%)",
    "contrast(75%)",
    "brightness(150%)",
    "invert(50%)",
    "hue-rotate(90deg)",
    "saturate(0.3)",
  ];

  let container = ContainerNode {
    tw: None,
    style: Some(
      StyleBuilder::default()
        .width(Percentage(100.0))
        .height(Percentage(100.0))
        .flex_wrap(FlexWrap::Wrap)
        .gap(SpacePair::from_single(Rem(1.0)))
        .justify_content(JustifyContent::Center)
        .align_items(AlignItems::Center)
        .background_color(ColorInput::Value(Color::white()))
        .build()
        .unwrap(),
    ),
    children: Some(
      effects
        .iter()
        .map(|effect| {
          ContainerNode {
            tw: None,
            style: Some(
              StyleBuilder::default()
                .flex_direction(FlexDirection::Column)
                .align_items(AlignItems::Center)
                .font_size(CssOption::some(Rem(1.5)))
                .build()
                .unwrap(),
            ),
            children: Some(vec![
              ImageNode {
                tw: None,
                src: "assets/images/yeecord.png".into(),
                style: Some(
                  StyleBuilder::default()
                    .width(Px(128.0))
                    .height(Px(128.0))
                    .filter(CssOption::some(
                      from_value::<Filters>(Value::String(effect.to_string())).unwrap(),
                    ))
                    .build()
                    .unwrap(),
                ),
                width: None,
                height: None,
              }
              .into(),
              TextNode {
                tw: None,
                style: None,
                text: effect.to_string(),
              }
              .into(),
            ]),
          }
          .into()
        })
        .collect::<Vec<NodeKind>>(),
    ),
  };

  run_style_width_test(container.into(), "tests/fixtures/style_filter.webp");
}
