mod test_utils;

use std::str::FromStr;

use takumi::prelude::*;
use test_utils::{block, measure_with_css};

/// A block carrying the `style` object a JS caller would send, so the value
/// takes the deserializing path rather than the typed builder.
fn styled_block(class: &str, style: serde_json::Value) -> Node {
  Node::container([])
    .with_class_name(class)
    .with_style(serde_json::from_value(style).expect("style should deserialize"))
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

fn tw_block(class: &str, values: &str) -> Node {
  Node::container([])
    .with_class_name(class)
    .with_tw(tw(values))
}

fn tw(values: &str) -> TailwindValues {
  TailwindValues::from_str(values).expect("tailwind values should parse")
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
  let hidden = Node::container([])
    .with_attributes([("hidden".into(), "".into())].into_iter().collect())
    .with_tw(tw("block w-64!"));
  let result = measure_with_css(Node::container([hidden]), r#"@import "tailwindcss";"#);

  assert_eq!(result.children[0].width, 0.0);
}

/// `until-found` is the one `hidden` value Preflight leaves visible.
#[test]
fn preflight_keeps_hidden_until_found_visible() {
  let node = Node::container([])
    .with_attributes(
      [("hidden".into(), "until-found".into())]
        .into_iter()
        .collect(),
    )
    .with_tw(tw("block w-64!"));
  let result = measure_with_css(Node::container([node]), r#"@import "tailwindcss";"#);

  assert_eq!(result.children[0].width, 256.0);
}

/// `!important` on a custom property marks the declaration rather than ending
/// up inside the value, so `var()` substitutes a usable one.
#[test]
fn important_custom_property_keeps_its_value() {
  let result = measure_with_css(
    Node::container([block("box")]),
    ".box { --w: 50px !important; width: var(--w); }",
  );
  assert_eq!(result.children[0].width, 50.0);
}

/// An important custom property outranks a normal one of higher specificity,
/// and loses to an important one of higher specificity.
#[test]
fn important_custom_property_takes_part_in_the_cascade() {
  let beats_specificity = measure_with_css(
    Node::container([block("box extra")]),
    r#"
      .box { --w: 50px !important; }
      .box.extra { --w: 100px; }
      .box { width: var(--w); }
    "#,
  );
  assert_eq!(beats_specificity.children[0].width, 50.0);

  let loses_to_specificity = measure_with_css(
    Node::container([block("box extra")]),
    r#"
      .box { --w: 150px !important; }
      .box.extra { --w: 75px !important; }
      .box { width: var(--w); }
    "#,
  );
  assert_eq!(loses_to_specificity.children[0].width, 75.0);
}

/// Important declarations reverse layer order, so a layered one outranks an
/// unlayered one and an earlier layer outranks a later one.
#[test]
fn important_custom_property_reverses_layer_order() {
  let result = measure_with_css(
    Node::container([block("box")]),
    r#"
      @layer base { .box { --w: 70px !important; } }
      .box { --w: 140px !important; }
      .box { width: var(--w); }
    "#,
  );
  assert_eq!(result.children[0].width, 70.0);
}

/// A custom property keeps its value verbatim, so a `!` that does not start a
/// trailing `!important` stays in the value and the declaration survives.
#[test]
fn a_bang_inside_a_custom_value_is_not_an_importance_marker() {
  for css in [
    ".box { --w: 50px !x; width: var(--w, 90px); }",
    ".box { --w: 50px ! 60px; width: var(--w, 90px); }",
  ] {
    let result = measure_with_css(Node::container([block("box")]), css);

    // The name is defined, so the fallback never applies and the substituted
    // value is not a width.
    assert_ne!(result.children[0].width, 90.0, "case: {css}");
  }
}

/// #1645: a `--tw-*` name an `@property` rule registers keeps the rule's
/// initial value, instead of being dropped as unregistered utility state.
#[test]
fn a_registered_tw_property_keeps_its_initial_value() {
  let root = Node::container([block("box")]);
  let result = measure_with_css(
    root,
    r#"
      @property --tw-box-width {
        syntax: "<length>";
        inherits: false;
        initial-value: 120px;
      }
      .box { width: var(--tw-box-width); }
    "#,
  );
  assert_eq!(result.children[0].width, 120.0);
}

/// The same registration read by the element the stylesheet declares it for,
/// rather than by a descendant.
#[test]
fn a_registered_property_reaches_the_root_element() {
  let result = measure_with_css(
    block("box"),
    r#"
      @property --box-width {
        syntax: "<length>";
        inherits: false;
        initial-value: 140px;
      }
      .box { width: var(--box-width); }
    "#,
  );
  assert_eq!(result.width, 140.0);
}

/// A `--tw-*` name the utility engine never wrote is the author's own variable,
/// so it inherits like any unregistered custom property.
#[test]
fn an_author_tw_named_variable_still_inherits() {
  let root = Node::container([block("box")]);
  let result = measure_with_css(
    root,
    r#"
      :root { --tw-box-width: 160px; }
      .box { width: var(--tw-box-width); }
    "#,
  );
  assert_eq!(result.children[0].width, 160.0);
}

/// `@apply` writes the same utility state through a stylesheet rule, so it has
/// to stop at its element the way the `tw` attribute's does. An inherited stop
/// list is not a length, so the width declaration would fall over.
#[test]
fn applied_gradient_state_stops_at_its_element() {
  let root = Node::container([block("child")]).with_class_name("hero");
  let result = measure_with_css(
    root,
    r#"
      .hero { @apply bg-linear-to-r from-red-500 to-blue-500; }
      .child { width: var(--tw-gradient-stops, 90px); }
    "#,
  );
  assert_eq!(result.children[0].width, 90.0);
}

/// Element state belongs to the element that set it, not to its subtree, so a
/// descendant's own variable of the same name inherits normally.
#[test]
fn element_state_does_not_claim_the_whole_subtree() {
  let inner = Node::container([block("leaf")])
    .with_class_name("mid")
    .with_style(Style::default().with(StyleDeclaration::display(Display::Block)));
  let root = Node::container([inner])
    .with_class_name("hero")
    .with_tw(tw("translate-x-4"));
  let result = measure_with_css(
    root,
    r#"
      .mid { --tw-translate-x: 70px; }
      .leaf { width: var(--tw-translate-x, 10px); }
    "#,
  );
  assert_eq!(result.children[0].children[0].width, 70.0);
}

/// An important utility takes the block through the importance split, which the
/// element state has to survive.
#[test]
fn important_utility_state_stops_at_its_element() {
  let root = Node::container([block("leaf")])
    .with_class_name("hero")
    .with_tw(tw("!translate-x-4"));
  let result = measure_with_css(root, ".leaf { width: var(--tw-translate-x, 80px); }");

  assert_eq!(result.children[0].width, 80.0);
}

/// The same importance split on the `@apply` path.
#[test]
fn important_applied_state_stops_at_its_element() {
  let root = Node::container([block("leaf")]).with_class_name("hero");
  let result = measure_with_css(
    root,
    r#"
      .hero { @apply !translate-x-4; }
      .leaf { width: var(--tw-translate-x, 80px); }
    "#,
  );
  assert_eq!(result.children[0].width, 80.0);
}

/// A registration is keyed by name, so an author value that outranks the
/// utility on the same element stops there too, as it would in a browser
/// holding Tailwind's own `@property` rule.
#[test]
fn an_author_value_over_utility_state_stops_at_its_element() {
  let root = Node::container([block("leaf")])
    .with_class_name("hero")
    .with_tw(tw("translate-x-4"));
  let result = measure_with_css(
    root,
    r#"
      .hero { --tw-translate-x: 55px; }
      .leaf { width: var(--tw-translate-x, 80px); }
    "#,
  );
  assert_eq!(result.children[0].width, 80.0);
}

/// An inline `--tw-` name is the author's, so it inherits.
#[test]
fn an_inline_tw_named_variable_still_inherits() {
  let hero = Node::container([block("leaf")]).with_style(
    serde_json::from_value(serde_json::json!({
      "display": "block",
      "--tw-translate-x": "65px"
    }))
    .expect("style should deserialize"),
  );
  let result = measure_with_css(hero, ".leaf { width: var(--tw-translate-x, 80px); }");

  assert_eq!(result.children[0].width, 65.0);
}

/// The plainest shape: state a `tw` attribute writes stops at its element.
#[test]
fn utility_state_stops_at_its_element() {
  let root = Node::container([block("leaf")])
    .with_class_name("hero")
    .with_tw(tw("translate-x-4"));
  let result = measure_with_css(root, ".leaf { width: var(--tw-translate-x, 80px); }");

  assert_eq!(result.children[0].width, 80.0);
}
