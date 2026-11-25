use std::str::FromStr;

use takumi::prelude::*;

use crate::test_utils::{block, measure_with_css};

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
fn important_shorthand_wins_over_higher_specificity_normal() {
  let root = Node::container([block("box")]);
  let result = measure_with_css(
    root,
    r#"
      .box { box-sizing: content-box; width: 100px; border: 10px solid red !important; }
      div.box { border: none; }
    "#,
  );
  assert_eq!(result.children[0].width, 120.0);
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
  let plain = block_of([block("probe")]);
  let outer = block_of([block("probe")]).with_class_name("outer");
  let trailing = block_of([block("probe")]);
  let root = Node::container([plain, outer, trailing]);
  let result = measure_with_css(root, r#".outer .probe { width: 150px; }"#);

  let default_width = result.children[0].children[0].width;
  assert_ne!(default_width, 150.0);
  assert_eq!(result.children[1].children[0].width, 150.0);
  assert_eq!(result.children[2].children[0].width, default_width);
}

/// A `display: none` subtree is skipped without styling, and rules still reach the nodes after it.
#[test]
fn rules_match_the_nodes_after_a_hidden_subtree() {
  let hidden = Node::container([block("probe"), block_of([block("probe"), Node::text("x")])])
    .with_style(Style::default().with(StyleDeclaration::display(Display::None)));
  let root = Node::container([hidden, block("after")]);
  let result = measure_with_css(root, ".probe { width: 10px } .after { width: 150px }");

  assert_eq!(result.children.len(), 1);
  assert_eq!(result.children[0].width, 150.0);
}

/// A block container of `children`.
fn block_of(children: impl Into<Vec<Node>>) -> Node {
  Node::container(children)
    .with_style(Style::default().with(StyleDeclaration::display(Display::Block)))
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

  assert!(result.children.is_empty());
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

#[test]
fn structural_pseudo_classes_skip_text_nodes() {
  let root = Node::container([
    Node::text(" "),
    block("probe"),
    Node::text(" "),
    block("probe"),
    Node::text(" "),
  ]);
  let result = measure_with_css(
    root,
    r#"
      .probe:first-child { width: 100px; }
      .probe:last-child { width: 200px; }
    "#,
  );
  let widths: Vec<f32> = result.children.iter().map(|child| child.width).collect();

  assert!(widths.contains(&100.0), "{widths:?}");
  assert!(widths.contains(&200.0), "{widths:?}");
}

#[test]
fn all_resets_what_earlier_rules_set() {
  let root = Node::container([block("box reset")]);
  let result = measure_with_css(
    root,
    r#"
      .box { width: 100px; }
      .reset { all: initial; }
    "#,
  );

  assert_eq!(result.children[0].width, result.width);
}

#[test]
fn all_unset_in_a_where_rule_yields_to_every_other_rule() {
  let root = Node::container([block("box")]);
  let result = measure_with_css(
    root,
    r#"
      .box { width: 100px; }
      :where(*) { all: unset; }
    "#,
  );

  assert_eq!(result.children[0].width, 100.0);
}

#[test]
fn a_var_that_substitutes_to_nothing_drops_the_earlier_value() {
  let root = Node::container([block("box")]);
  let result = measure_with_css(
    root,
    r#"
      .box { width: 100px; width: var(--missing); }
    "#,
  );

  assert_eq!(result.children[0].width, result.width);
}

/// Boxes that match the same rules share a resolved style only when their `tw` classes and the
/// variants their place admits agree.
#[test]
fn tw_boxes_share_styles_only_when_their_classes_agree() {
  let root = block_of([
    tw_block("row", "block w-[10px] last:w-[40px]"),
    tw_block("row", "block w-[20px]"),
    tw_block("row", "block w-[10px] last:w-[40px]"),
    tw_block("row", "block w-[10px] last:w-[40px]"),
  ]);
  let widths: Vec<f32> = measure_with_css(root, "")
    .children
    .iter()
    .map(|child| child.width)
    .collect();

  assert_eq!(widths, [10.0, 20.0, 10.0, 40.0]);
}

/// A CSS-wide keyword on a custom property resolves in the cascade instead of becoming its value:
/// `inherit` takes the parent's, `initial` the registered initial value or none, and `unset`
/// either one as the property inherits.
#[test]
fn css_wide_keywords_resolve_on_custom_properties() {
  let registered = "@property --w { syntax: '<length>'; inherits: false; initial-value: 10px; }";

  for (css, width) in [
    (".box { --w: inherit; width: var(--w); }".to_owned(), 50.0),
    (
      ".box { --w: initial; width: var(--w, 70px); }".to_owned(),
      70.0,
    ),
    (".box { --w: unset; width: var(--w); }".to_owned(), 50.0),
    (
      format!("{registered} .box {{ --w: inherit; width: var(--w); }}"),
      50.0,
    ),
    (
      format!("{registered} .box {{ --w: unset; width: var(--w); }}"),
      10.0,
    ),
  ] {
    let root = Node::container([block("box")]).with_class_name("outer");
    let result = measure_with_css(root, &format!(".outer {{ --w: 50px; }} {css}"));

    assert_eq!(result.children[0].width, width, "{css}");
  }
}

/// A `filter: url(#id)` names an element of the same document, which HTML has none of, so it
/// paints as no filter does.
#[test]
fn a_filter_url_to_a_fragment_leaves_an_html_element_unfiltered() {
  use crate::test_utils::render_node;

  let page = |filter: &str| {
    Node::from_html(
      &format!(
        r#"<div style="width:40px;height:40px;background-color:rgb(10,120,200);{filter}"></div>"#
      ),
      FromHtmlOptions::default(),
    )
    .expect("parses")
  };
  let viewport = Viewport::new((60, 60));
  let plain = render_node(page(""), viewport);
  let filtered = render_node(page("filter:url(#missing)"), viewport);

  assert_eq!(plain.as_raw(), filtered.as_raw());
}

/// A registered custom property animates as its syntax types it, and `var()` reads the animated
/// value; an unregistered one switches half way, as Chromium animates them.
#[test]
fn an_animated_custom_property_interpolates_when_registered() {
  let width_at = |registered: bool, delay_ms: u32, from: &str, to: &str| {
    let registration = if registered {
      "@property --w { syntax: '<length>'; inherits: false; initial-value: 0px }
       @property --n { syntax: '<number>'; inherits: false; initial-value: 0 }"
    } else {
      ""
    };
    let css = format!(
      "{registration}
       @keyframes grow {{ from {{ --w: {from}; --n: {from} }} to {{ --w: {to}; --n: {to} }} }}
       .box {{ font-size: 10px; animation: grow 1000ms linear -{delay_ms}ms; width: var(--w); height: calc(var(--n) * 1px) }}"
    );
    let measured = measure_with_css(
      Node::container([styled_block(
        "box",
        serde_json::json!({ "display": "block" }),
      )]),
      &css,
    );

    (measured.children[0].width, measured.children[0].height)
  };

  assert_eq!(width_at(true, 250, "100px", "300px").0, 150.0);
  assert_eq!(width_at(true, 750, "100px", "300px").0, 250.0);
  assert_eq!(width_at(true, 500, "1em", "3em").0, 20.0);
  assert_eq!(width_at(true, 250, "100", "300").1, 150.0);
  assert_eq!(width_at(false, 250, "100px", "300px").0, 100.0);
  assert_eq!(width_at(false, 750, "100px", "300px").0, 300.0);
}

/// What a registered custom property gives `width`, set in the stylesheet `css` on `.box`.
fn registered_width(css: &str) -> f32 {
  measure_with_css(
    Node::container([styled_block(
      "box",
      serde_json::json!({ "display": "block" }),
    )]),
    css,
  )
  .children[0]
    .width
}

/// The cascade around a registered custom property, as Chromium cascades it.
#[test]
fn a_registered_custom_property_cascades_as_chromium_does() {
  let size = "@property --size { syntax: '<length>'; inherits: false; initial-value: 10px }";
  let count = "@property --n { syntax: '<integer>'; inherits: false; initial-value: 0 }";

  // A reference reads the value the syntax leaves.
  assert_eq!(
    registered_width(&format!(
      "{size} .box {{ --size: red; --alias: var(--size); width: var(--alias) }}"
    )),
    10.0
  );
  // The element itself reads the computed integer.
  assert_eq!(
    registered_width(&format!(
      "{count} .box {{ --n: calc(2.6); width: calc(var(--n) * 1px) }}"
    )),
    3.0
  );
  // An important declaration outranks an animation.
  assert_eq!(
    registered_width(&format!(
      "{size} @keyframes grow {{ to {{ --size: 300px }} }}
       .box {{ --size: 40px !important; width: var(--size); animation: grow 1000ms linear -500ms }}"
    )),
    40.0
  );
  // A keyframe's `var()` resolves before it interpolates.
  assert_eq!(
    registered_width(&format!(
      "{size} @keyframes grow {{ from {{ --size: var(--start) }} to {{ --size: var(--end) }} }}
       .box {{ --start: 100px; --end: 300px; width: var(--size); animation: grow 1000ms linear -250ms }}"
    )),
    150.0
  );
  // A keyframe that leaves the property out does not split its interpolation.
  assert_eq!(
    registered_width(&format!(
      "{size} @keyframes grow {{ from {{ --size: 0px }} 50% {{ opacity: 0.5 }} to {{ --size: 100px }} }}
       .box {{ width: var(--size); animation: grow 1000ms linear -250ms }}"
    )),
    25.0
  );
  // An unregistered property with no value before switches to the keyframe's half way.
  assert_eq!(
    registered_width(
      "@keyframes grow { to { --w: 100px } }
       .box { width: var(--w, 5px); animation: grow 1000ms linear -750ms }"
    ),
    100.0
  );
}

/// An animation property that reads an animated custom property keeps its value from before the
/// animation, as Chromium keeps it: `g` runs over the declared 1s, not the 2s `a` animates `--d` to.
#[test]
fn an_animated_custom_property_leaves_the_animation_properties_alone() {
  let measured = measure_with_css(
    Node::container([styled_block(
      "box",
      serde_json::json!({ "display": "block", "height": "1px" }),
    )]),
    "@keyframes g { from { width: 0px } to { width: 400px } }
     @keyframes a { from { --d: 2s } to { --d: 2s } }
     .box { --d: 1s; animation-name: g, a; animation-duration: var(--d), 1s;
       animation-timing-function: linear; animation-delay: -500ms }",
  );

  assert_eq!(measured.children[0].width, 200.0);

  // The taint reaches an alias of the animated property, and survives an important replay.
  for duration in [
    "--duration: var(--d); animation-duration: var(--duration), 1s",
    "animation-duration: var(--d), 1s !important",
  ] {
    let measured = measure_with_css(
      Node::container([styled_block(
        "box",
        serde_json::json!({ "display": "block", "height": "1px" }),
      )]),
      &format!(
        "@keyframes g {{ from {{ width: 0px }} to {{ width: 400px }} }}
         @keyframes a {{ from {{ --d: 2s }} to {{ --d: 2s }} }}
         .box {{ --d: 1s; animation-name: g, a; {duration};
           animation-timing-function: linear; animation-delay: -500ms }}"
      ),
    );

    assert_eq!(measured.children[0].width, 200.0, "{duration}");
  }
}

/// On the document root, `rem` reads the root's own font size, so a registered `rem` length that
/// `font-size` reads makes a cycle, as Chromium breaks it.
#[test]
fn the_root_font_size_breaks_a_cycle_with_a_registered_rem_length() {
  let root = |declarations: &str| {
    let html = Node::from_html("<html></html>", FromHtmlOptions::default()).expect("parses");
    let measured = measure_with_css(
      html,
      &format!(
        "@property --x {{ syntax: '<length>'; inherits: false; initial-value: 0px }}
         html {{ display: block; height: 1em; {declarations} }}"
      ),
    );

    (measured.width, measured.height)
  };

  assert_eq!(
    root("--x: 2rem; font-size: var(--x); width: var(--x)"),
    (0.0, 16.0)
  );
  assert_eq!(
    root("--x: 2rem; font-size: 20px; width: var(--x)"),
    (40.0, 20.0)
  );
}

/// A registered length in a font-relative unit that `font-size` reads makes a cycle, which leaves
/// the property at its initial value and `font-size` inherited, as Chromium breaks it.
#[test]
fn font_size_breaks_a_cycle_with_a_font_relative_registered_length() {
  let size = |declarations: &str| {
    let css = format!(
      "@property --x {{ syntax: '<length>'; inherits: false; initial-value: 0px }}
       .p {{ font-size: 10px }}
       .box {{ --x: 2em; width: var(--x); height: 1em; {declarations} }}"
    );
    let measured = measure_with_css(
      Node::container([styled_block(
        "box",
        serde_json::json!({ "display": "block" }),
      )])
      .with_class_name("p"),
      &css,
    );

    (measured.children[0].width, measured.children[0].height)
  };

  assert_eq!(size("font-size: var(--x)"), (0.0, 10.0));
  assert_eq!(size("font-size: calc(var(--x) + 1px)"), (0.0, 10.0));
  assert_eq!(size("font-size: 30px"), (60.0, 30.0));
  assert_eq!(size("font-size: var(--x) !important"), (0.0, 10.0));
  assert_eq!(size("font-size: var(--x); font-size: 30px"), (60.0, 30.0));
  assert_eq!(size("font-size: 30px; font-size: var(--x)"), (0.0, 10.0));
  assert_eq!(
    size("--alias: var(--x); font-size: var(--alias)"),
    (0.0, 10.0)
  );
  assert_eq!(
    size("--alias: var(--x); font-size: var(--x); width: var(--alias)"),
    (0.0, 10.0)
  );
  assert_eq!(size("font-size: var(--prefix--x, 25px)"), (50.0, 25.0));
}

/// `light-dark()` follows the used color scheme, which the viewport's preference picks among the
/// schemes `color-scheme` lists.
#[test]
fn light_dark_follows_the_preferred_scheme_color_scheme_lists() {
  use crate::test_utils::render_node;

  let first_pixel = |color_scheme: &str, preferred: ColorScheme| {
    let page = Node::from_html(
      &format!(
        r#"<div style="width:4px;height:4px;color-scheme:{color_scheme};background-color:light-dark(rgb(255,0,0),rgb(0,0,255))"></div>"#
      ),
      FromHtmlOptions::default(),
    )
    .expect("parses");
    let image = render_node(page, Viewport::new((4, 4)).with_color_scheme(preferred));

    image.as_raw()[..4].to_vec()
  };
  let (red, blue) = (vec![255, 0, 0, 255], vec![0, 0, 255, 255]);

  for (color_scheme, preferred, expected) in [
    ("light dark", ColorScheme::Dark, &blue),
    ("light dark", ColorScheme::Light, &red),
    ("dark", ColorScheme::Light, &blue),
    ("light", ColorScheme::Dark, &red),
    ("normal", ColorScheme::Dark, &red),
  ] {
    assert_eq!(
      &first_pixel(color_scheme, preferred),
      expected,
      "color-scheme: {color_scheme} preferring {preferred:?}"
    );
  }
}

/// A `color-mix()` of `currentColor` in `color` mixes the parent's colour once, and descendants
/// inherit the mixed colour, as Blink's `Color::ApplyValue` resolves it.
#[test]
fn color_mix_of_current_color_in_color_mixes_the_parent_colour() {
  use crate::test_utils::render_node;

  let page = Node::from_html(
    r#"<div style="color:rgb(0,0,255)"><div style="color:color-mix(in srgb,currentColor,rgb(255,0,0))"><div style="width:4px;height:4px;background-color:currentColor"></div></div></div>"#,
    FromHtmlOptions::default(),
  )
  .expect("parses");
  let image = render_node(page, Viewport::new((4, 4)));

  assert_eq!(image.as_raw()[..4], [128, 0, 128, 255]);
}

/// The first pixel of a 4px block with `class` under `css`, rendered at time 0.
fn first_pixel_with_css(class: &str, css: &str) -> Vec<u8> {
  let image = takumi::render(
    RenderOptions::builder()
      .viewport(Viewport::new((4, 4)))
      .node(styled_block(
        class,
        serde_json::json!({ "width": "4px", "height": "4px" }),
      ))
      .stylesheet(StyleSheet::parse_loosy(css).into())
      .fonts(&crate::test_utils::CONTEXT)
      .build(),
  )
  .unwrap();

  image.as_raw()[..4].to_vec()
}

/// `color-scheme` cascades its declarations, its animation, and its `!important` declarations
/// before the colours that read it, as Blink's `ApplyHighPriority` applies it, so every
/// `light-dark()` resolves in the scheme that wins.
#[test]
fn light_dark_resolves_in_the_color_scheme_that_wins_the_cascade() {
  const KEYFRAMES: &str = "
    @keyframes to-dark { from, to { color-scheme: dark } }
    @keyframes to-light { from, to { color-scheme: light } }
    @keyframes fill { from, to { background-color: light-dark(rgb(255,0,0), rgb(0,0,255)) } }
    .static { background-color: light-dark(rgb(255,0,0), rgb(0,0,255)) }";
  let (red, blue) = (vec![255, 0, 0, 255], vec![0, 0, 255, 255]);

  for (rule, expected) in [
    (".a { animation: to-dark 1s paused }", &blue),
    (".a { animation: fill 1s paused, to-dark 1s paused }", &blue),
    (".a { animation: to-dark 1s paused, fill 1s paused }", &blue),
    (
      ".a { color-scheme: light !important; animation: to-dark 1s paused }",
      &red,
    ),
    (
      ".a { color-scheme: light !important; animation: to-dark 1s paused, fill 1s paused }",
      &red,
    ),
    (
      ".a { color-scheme: dark; animation: to-light 1s paused, fill 1s paused }",
      &red,
    ),
    (
      ".a { color-scheme: light; animation: fill 1s paused, to-dark 1s paused }",
      &blue,
    ),
  ] {
    let class = if rule.contains("fill 1s") {
      "a"
    } else {
      "a static"
    };

    assert_eq!(
      &first_pixel_with_css(class, &format!("{KEYFRAMES} {rule}")),
      expected,
      "{rule}"
    );
  }
}

/// A keyframed custom property resolves its `light-dark()` in the scheme an animation of
/// `color-scheme` gives.
#[test]
fn animated_custom_property_resolves_light_dark_in_the_animated_color_scheme() {
  let css = "
    @property --ink { syntax: '<color>'; inherits: false; initial-value: black }
    @keyframes ink { from, to { color-scheme: dark; --ink: light-dark(rgb(255,0,0), rgb(0,0,255)) } }
    .a { background-color: var(--ink); animation: ink 1s paused }
    .b { color-scheme: light !important; background-color: var(--ink); animation: ink 1s paused }";

  assert_eq!(first_pixel_with_css("a", css), [0, 0, 255, 255]);
  assert_eq!(first_pixel_with_css("b", css), [255, 0, 0, 255]);
}

/// The width and height `.box` measures to under `css`.
fn box_size(css: &str) -> (f32, f32) {
  let measured = measure_with_css(Node::container([block("box")]), css);

  (measured.children[0].width, measured.children[0].height)
}

/// `color` and the font longhands are high priority, so they apply before the other longhands and
/// against the parent, as Blink's `ApplyHighPriority` applies them, animated or not.
#[test]
fn animations_read_the_font_and_colour_the_high_priority_longhands_give() {
  // An animated element's `em` font size reads its parent's font size.
  assert_eq!(
    box_size(
      "@keyframes fade { from { opacity: 0 } to { opacity: 1 } }
       .box { font-size: 2em; height: 1em; animation: fade 1s }"
    )
    .1,
    32.0
  );
  // An animated font size interpolates an `em` against the parent's font size.
  assert_eq!(
    box_size(
      "@keyframes f { from { font-size: 1em } to { font-size: 40px } }
       .box { font-size: 30px; height: 1em; animation: f 1000ms linear -500ms }"
    )
    .1,
    28.0
  );
  // Another animation reads the animated font size.
  assert_eq!(
    box_size(
      "@keyframes f { from, to { font-size: 20px } }
       @keyframes w { from { width: 10em } to { width: 100px } }
       .box { animation: w 1000ms linear -500ms, f 1s }"
    )
    .0,
    150.0
  );
  // Another animation reads the animated colour as `currentColor`.
  assert_eq!(
    first_pixel_with_css(
      "a",
      "@keyframes c { from, to { color: rgb(0,0,255) } }
       @keyframes b { from { background-color: currentColor } to { background-color: rgb(0,255,0) } }
       .a { color: rgb(255,0,0); animation: b 1s paused, c 1s paused }"
    ),
    [0, 0, 255, 255]
  );
}

/// Where `.box`, a 10px block under `css`, starts from its parent's left edge.
fn box_left(css: &str) -> f32 {
  measure_with_css(
    Node::container([block("box")]),
    &format!(".box {{ width: 10px; height: 10px }} {css}"),
  )
  .children[0]
    .transform[4]
}

/// Each longhand takes its last declaration in cascade order, as Blink's `StyleCascade` picks it.
/// A logical property counts as the physical one it sets in the element's own `direction`, decided
/// first wherever it is declared, and a shorthand with `var()` as each longhand it sets.
#[test]
fn each_longhand_takes_its_last_declaration() {
  assert_eq!(
    box_left(".box { margin-left: 10px; margin-inline-start: 20px }"),
    20.0
  );
  assert_eq!(
    box_left(".box { margin-inline-start: 20px; margin-left: 10px }"),
    10.0
  );
  assert_eq!(
    box_left(".box { margin-left: 7px; margin-inline-end: 20px; direction: rtl }"),
    20.0
  );
  assert_eq!(
    box_left(".box { margin-inline-end: 20px; margin-left: 7px; direction: rtl }"),
    7.0
  );

  let font = |declarations: &str| {
    box_size(&format!(
      ".box {{ --f: 10px serif; height: 1em; {declarations} }}"
    ))
    .1
  };

  assert_eq!(font("font: var(--f); font-size: 30px"), 30.0);
  assert_eq!(font("font-size: 30px; font: var(--f)"), 10.0);
}

/// A registered custom property in a cycle with `font-size` takes its `unset` value, as Blink's
/// `ApplyUnset` gives it: the parent's when it inherits, not its initial value.
#[test]
fn a_registered_property_in_a_font_size_cycle_is_unset() {
  let measured = measure_with_css(
    Node::container([Node::container([block("box")]).with_class_name("parent")]),
    "@property --x { syntax: '<length>'; inherits: true; initial-value: 5px }
     .parent { --x: 7px }
     .box { --x: 2em; font-size: var(--x); width: var(--x) }",
  );

  assert_eq!(measured.children[0].children[0].width, 7.0);
}

/// The width and top margin of `.box`, a parsed `<p>`, under `css`.
fn paragraph(css: &str) -> (f32, f32) {
  let node =
    Node::from_html(r#"<p class="box">x</p>"#, FromHtmlOptions::default()).expect("parses");
  let measured = measure_with_css(Node::container([node]), css);

  (
    measured.children[0].width,
    measured.children[0].transform[5],
  )
}

/// `revert`, `revert-layer`, and `revert-rule` roll a property back to a weaker declaration, as
/// Blink's `StyleCascade` resolves them, or to `unset` without one.
#[test]
fn revert_keywords_roll_back_to_a_weaker_declaration() {
  // `revert` reaches the user agent's `margin-top: 1em`, past every author rule.
  assert_eq!(
    paragraph(
      "@layer a { .box { margin-top: 3px } } .box { margin-top: 5px } p.box { margin-top: revert }"
    )
    .1,
    16.0
  );
  // `revert-layer` reaches the layer below, and an important one the normal declarations.
  assert_eq!(
    paragraph("@layer a { .box { width: 30px } } .box { width: 50px; width: revert-layer }").0,
    30.0
  );
  assert_eq!(
    paragraph(".box { width: 30px } p.box { width: revert-layer !important }").0,
    30.0
  );
  // `revert-rule` reaches another rule, past its own earlier declaration.
  assert_eq!(
    paragraph(".box { width: 30px } p.box { width: 70px; width: revert-rule }").0,
    30.0
  );
  // Without a weaker declaration, a revert keyword is `unset`.
  assert_eq!(paragraph(".box { width: revert-layer }").0, 1200.0);
  // A custom property rolls back too.
  assert_eq!(
    paragraph(
      "@layer a { .box { --w: 40px } } .box { --w: 90px; --w: revert-layer; width: var(--w) }"
    )
    .0,
    40.0
  );
}

/// The widths of `.card`, its `.a` paragraph, and the `.b` paragraph in its `.content` box, in
/// `<section class="outer"><div class="card">…</div></section>` under `css`.
fn card_widths(css: &str) -> [f32; 3] {
  let node = Node::from_html(
    r#"<section class="outer"><div class="card"><p class="a">x</p><div class="content"><p class="b">y</p></div></div></section>"#,
    FromHtmlOptions::default(),
  )
  .expect("parses");
  let measured = measure_with_css(Node::container([node]), css);
  let card = &measured.children[0].children[0];

  [
    card.width,
    card.children[0].width,
    card.children[1].children[0].width,
  ]
}

/// `@scope` limits its rules to the elements under a root and above a limit, and a rule from a
/// closer root wins after specificity, as Blink's `ElementRuleCollector` sorts them.
#[test]
fn scope_rules_apply_between_their_root_and_limit() {
  assert_eq!(
    card_widths("@scope (.card) { p { width: 30px } }"),
    [1200.0, 30.0, 30.0]
  );
  assert_eq!(
    card_widths("@scope (.card) to (.content) { p { width: 30px } }"),
    [1200.0, 30.0, 1200.0]
  );
  // `:scope` is the root, which a selector without it never matches.
  assert_eq!(
    card_widths("@scope (.card) { :scope { width: 100px } .card { width: 50px } }")[0],
    100.0
  );
  // A closer root wins over a later rule, and any root over an unscoped rule.
  assert_eq!(
    card_widths("@scope (.card) { p { width: 20px } } @scope (.outer) { p { width: 10px } }")[1],
    20.0
  );
  assert_eq!(
    card_widths("@scope (.card) { p { width: 20px } } p { width: 50px }")[1],
    20.0
  );
  // Specificity still comes first.
  assert_eq!(
    card_widths("@scope (.card) { p { width: 20px } } p.a { width: 50px }")[1],
    50.0
  );
}

/// The size of `.box`, a parsed `<div>` holding text, under `css`.
fn text_box_size(css: &str) -> (f32, f32) {
  let node = Node::from_html(
    r#"<div class="box">hello world</div>"#,
    FromHtmlOptions::default(),
  )
  .expect("parses");
  let measured = measure_with_css(Node::container([node]), css);

  (measured.children[0].width, measured.children[0].height)
}

/// `container-type` contains a box's size, as Blink's `ShouldApplyInlineSizeContainment` and
/// `ShouldApplyBlockSizeContainment` have it, so the content gives it none.
#[test]
fn container_type_contains_the_size_its_content_would_give() {
  let (width, height) = text_box_size(".box { display: inline-block }");

  assert!(width > 0.0 && height > 0.0);
  assert_eq!(
    text_box_size(".box { display: inline-block; container-type: inline-size; padding: 2px }").0,
    4.0
  );
  assert_eq!(text_box_size(".box { container-type: size }").1, 0.0);
  assert_eq!(
    text_box_size(".box { container-type: inline-size }").1,
    height
  );
}

/// The width of `.target` in a `.card` container under `css`.
fn target_in_card(css: &str) -> f32 {
  let node = Node::from_html(
    r#"<div class="card"><p class="target">x</p></div>"#,
    FromHtmlOptions::default(),
  )
  .expect("parses");
  let measured = measure_with_css(Node::container([node]), css);

  measured.children[0].children[0].width
}

/// `@container` rules read the size of the nearest container that `container-type` lets answer
/// them and that has the name they ask for, as Blink evaluates container queries.
#[test]
fn container_queries_read_the_nearest_matching_container() {
  let rule = "@container (min-width: 200px) { .target { width: 50px } }";

  assert_eq!(
    target_in_card(&format!(
      ".card {{ container-type: inline-size; width: 300px }} {rule}"
    )),
    50.0
  );
  assert_eq!(
    target_in_card(&format!(
      ".card {{ container-type: inline-size; width: 100px }} {rule}"
    )),
    100.0
  );
  // Without a container, the query does not hold.
  assert_eq!(
    target_in_card(&format!(".card {{ width: 300px }} {rule}")),
    300.0
  );
  // A name picks the container.
  assert_eq!(
    target_in_card(
      ".card { container: card / inline-size; width: 300px }
       @container card (min-width: 200px) { .target { width: 50px } }"
    ),
    50.0
  );
  assert_eq!(
    target_in_card(
      ".card { container: card / inline-size; width: 300px }
       @container other (min-width: 200px) { .target { width: 50px } }"
    ),
    300.0
  );
  // The block axis needs `container-type: size`.
  assert_eq!(
    target_in_card(
      ".card { container-type: inline-size; width: 300px; height: 300px }
       @container (min-height: 10px) { .target { width: 50px } }"
    ),
    300.0
  );
  assert_eq!(
    target_in_card(
      ".card { container-type: size; width: 300px; height: 300px }
       @container (min-height: 10px) and (inline-size > 200px) { .target { width: 50px } }"
    ),
    50.0
  );
  // A media query cannot read `inline-size`.
  assert_eq!(
    target_in_card(
      ".card { width: 300px } @media (min-inline-size: 1px) { .target { width: 50px } }"
    ),
    300.0
  );
}

/// A container whose size an outer container's query sets answers the queries inside it at that
/// size, as Blink sizes it before styling what is inside.
#[test]
fn a_container_answers_at_the_size_an_outer_query_gives_it() {
  let node = Node::from_html(
    r#"<div class="outer"><div class="inner"><p class="target">x</p></div></div>"#,
    FromHtmlOptions::default(),
  )
  .expect("parses");
  let measured = measure_with_css(
    Node::container([node]),
    ".outer { container-type: inline-size; width: 400px }
     .inner { container-type: inline-size; width: 100px }
     @container (min-width: 300px) { .inner { width: 250px } }
     @container (min-width: 200px) { .target { width: 50px } }",
  );
  let inner = &measured.children[0].children[0];

  assert_eq!(inner.width, 250.0);
  assert_eq!(inner.children[0].width, 50.0);
}
