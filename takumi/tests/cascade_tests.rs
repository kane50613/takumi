mod test_utils;

use takumi::{measure, prelude::*};
use test_utils::CONTEXT;

fn measure_with_css(node: Node, css: &str) -> MeasuredNode {
  let stylesheet = StyleSheet::parse_loosy(css);
  measure(
    RenderOptions::builder()
      .viewport(Viewport::new((1200, 630)))
      .node(node)
      .stylesheet(stylesheet.into())
      .fonts(&CONTEXT)
      .build(),
  )
  .unwrap()
}

/// A block carrying the `style` object a JS caller would send, so the value
/// takes the deserializing path rather than the typed builder.
fn styled_block(class: &str, style: serde_json::Value) -> Node {
  Node::container([])
    .with_class_name(class)
    .with_style(serde_json::from_value(style).expect("style should deserialize"))
}

fn block(class: &str) -> Node {
  Node::container([])
    .with_class_name(class)
    .with_style(Style::default().with(StyleDeclaration::display(Display::Block)))
}

#[test]
fn important_wins_over_higher_specificity_normal() {
  let root = Node::container([block("box")]);
  let result = measure_with_css(
    root,
    r#"
      .box { width: 100px !important; }
      div.box { width: 200px; }
    "#,
  );
  assert_eq!(result.children[0].width, 100.0);
}

#[test]
fn empty_rule_blocks_do_not_disturb_the_cascade() {
  let root = Node::container([block("box")]);
  let result = measure_with_css(
    root,
    r#"
      .box {}
      .box { width: 120px; }
      div.box {}
    "#,
  );
  assert_eq!(result.children[0].width, 120.0);
}

#[test]
fn descendant_selector_matches_after_sibling_subtrees() {
  let plain = Node::container([block("probe")]);
  let outer = Node::container([block("probe")]).with_class_name("outer");
  let trailing = Node::container([block("probe")]);
  let root = Node::container([plain, outer, trailing]);
  let result = measure_with_css(root, r#".outer .probe { width: 150px; }"#);

  let default_width = result.children[0].children[0].width;
  assert_ne!(default_width, 150.0);
  assert_eq!(result.children[1].children[0].width, 150.0);
  assert_eq!(result.children[2].children[0].width, default_width);
}

fn tw_block(class: &str, tw: &str) -> Node {
  use std::str::FromStr;

  Node::container([])
    .with_class_name(class)
    .with_tw(TailwindValues::from_str(tw).expect("tailwind values should parse"))
}

#[test]
fn tw_sits_below_author_rules() {
  let root = Node::container([tw_block("box", "block w-64")]);
  let result = measure_with_css(root, r#".box { width: 100px; }"#);

  assert_eq!(result.children[0].width, 100.0);
}

/// Unlayered important rules sort below every layer in the reversed important
/// order, so `tw` beats them.
#[test]
fn important_tw_beats_unlayered_author_rules() {
  let root = Node::container([
    tw_block("box", "block w-64!"),
    tw_block("shout", "block w-64!"),
  ]);
  let result = measure_with_css(
    root,
    r#"
      .box { width: 100px; }
      .shout { width: 100px !important; }
    "#,
  );

  assert_eq!(result.children[0].width, 256.0);
  assert_eq!(result.children[1].width, 256.0);
}

#[test]
fn tw_reads_theme_tokens_from_the_stylesheet() {
  let root = Node::container([
    tw_block("box", "block w-gutter"),
    tw_block("box", "block w-64"),
  ]);
  let result = measure_with_css(
    root,
    r#":root { --spacing-gutter: 10rem; --spacing: 0.5rem; }"#,
  );

  assert_eq!(result.children[0].width, 160.0);
  assert_eq!(result.children[1].width, 512.0);
}

/// `tw` is the last declared layer, so preflight wrapped in `@layer base`
/// resets defaults without beating utilities.
#[test]
fn tw_beats_named_layer_rules() {
  let root = Node::container([tw_block("box", "block w-64")]);
  let result = measure_with_css(
    root,
    r#"@layer base { *, ::after, ::before { box-sizing: border-box; margin: 0; padding: 0; border: 0 solid; } * { width: 50px; } }"#,
  );

  assert_eq!(result.children[0].width, 256.0);
}

/// A named layer's important half also outranks `tw`, which is declared last.
#[test]
fn important_layered_rules_beat_important_tw() {
  let result = measure_with_css(
    Node::container([tw_block("box", "block w-64!")]),
    "@layer base { .box { width: 120px !important; } }",
  );

  assert_eq!(result.children[0].width, 120.0);
}

fn tagged(tag: &str, preset: Style) -> Node {
  Node::container([block("probe")])
    .with_tag_name(tag)
    .with_preset(preset.with(StyleDeclaration::display(Display::Block)))
}

#[test]
fn preflight_clears_preset_padding() {
  let preset = Style::default().with(StyleDeclaration::padding_top(Length::Px(30.0)));

  let without = measure_with_css(Node::container([tagged("h1", preset.clone())]), "");
  assert_eq!(without.children[0].height, 30.0);

  let with_preflight = measure_with_css(
    Node::container([tagged("h1", preset)]),
    r#"@import "tailwindcss";"#,
  );
  assert_eq!(with_preflight.children[0].height, 0.0);
}

/// Preflight resets `font-size` on headings only, so a preset that sizes any
/// other element still reaches the cascade.
#[test]
fn preflight_keeps_preset_font_size_outside_headings() {
  let preset = Style::default().with(StyleDeclaration::font_size(FontSize::Length(Length::Px(
    8.0,
  ))));
  let css = r#"@import "tailwindcss"; .probe { width: 2em; }"#;

  let paragraph = measure_with_css(Node::container([tagged("p", preset.clone())]), css);
  assert_eq!(paragraph.children[0].children[0].width, 16.0);

  let heading = measure_with_css(Node::container([tagged("h1", preset)]), css);
  assert_eq!(heading.children[0].children[0].width, 32.0);
}

/// Preflight resets heading fonts to `inherit`, not to the initial size.
#[test]
fn preflight_inherits_the_parent_font_size_on_headings() {
  let preset = Style::default().with(StyleDeclaration::font_size(FontSize::Length(Length::Px(
    8.0,
  ))));
  let heading = measure_with_css(
    Node::container([tagged("h1", preset)]),
    r#"@import "tailwindcss"; :root { font-size: 20px; } .probe { width: 2em; }"#,
  );

  assert_eq!(heading.children[0].children[0].width, 40.0);
}

/// css-cascade-5 sorts element-attached styles before cascade layers, so an
/// important inline declaration outranks an important rule in any layer.
#[test]
fn important_inline_wins_over_important_rules() {
  let style = serde_json::json!({ "display": "block", "width": "55px !important" });
  let unlayered = measure_with_css(
    Node::container([styled_block("box", style.clone())]),
    ".box { width: 100px !important; }",
  );
  let layered = measure_with_css(
    Node::container([styled_block("box", style)]),
    "@layer a { .box { width: 100px !important; } }",
  );

  assert_eq!(unlayered.children[0].width, 55.0);
  assert_eq!(layered.children[0].width, 55.0);
}

/// An HTML `style` attribute reaches the cascade through `parse_loosy`, and a
/// `var()` value defers rather than parsing, so the marker has to survive the
/// scan that spots the function.
#[test]
fn important_wins_from_a_deferred_inline_value() {
  let styled = Node::container([])
    .with_class_name("box")
    .with_style(Style::from(StyleDeclarationBlock::parse_loosy(
      "display: block; width: var(--w) !important",
    )));
  let result = measure_with_css(
    Node::container([styled]),
    ":root { --w: 55px; } .box { width: 100px !important; }",
  );

  assert_eq!(result.children[0].width, 55.0);
}

/// Important author declarations outrank animations, which outrank normal ones.
#[test]
fn important_inline_wins_over_an_animation() {
  let css = r#"
    @keyframes grow { from { width: 300px; } to { width: 300px; } }
    .box { animation: grow 10s; }
  "#;
  let normal = measure_with_css(
    Node::container([styled_block(
      "box",
      serde_json::json!({ "display": "block", "width": "55px" }),
    )]),
    css,
  );
  let important = measure_with_css(
    Node::container([styled_block(
      "box",
      serde_json::json!({ "display": "block", "width": "55px !important" }),
    )]),
    css,
  );

  assert_eq!(normal.children[0].width, 300.0);
  assert_eq!(important.children[0].width, 55.0);
}

/// Preflight's `[hidden]` rule is important, so it survives a `tw` utility
/// that is important too.
#[test]
fn preflight_hides_the_hidden_attribute() {
  use std::str::FromStr;

  let hidden = Node::container([])
    .with_attributes([("hidden".into(), "".into())].into_iter().collect())
    .with_tw(TailwindValues::from_str("block w-64!").expect("tailwind values should parse"));
  let result = measure_with_css(Node::container([hidden]), r#"@import "tailwindcss";"#);

  assert_eq!(result.children[0].width, 0.0);
}

/// `until-found` is the one `hidden` value Preflight leaves visible.
#[test]
fn preflight_keeps_hidden_until_found_visible() {
  use std::str::FromStr;

  let node = Node::container([])
    .with_attributes(
      [("hidden".into(), "until-found".into())]
        .into_iter()
        .collect(),
    )
    .with_tw(TailwindValues::from_str("block w-64!").expect("tailwind values should parse"));
  let result = measure_with_css(Node::container([node]), r#"@import "tailwindcss";"#);

  assert_eq!(result.children[0].width, 256.0);
}
