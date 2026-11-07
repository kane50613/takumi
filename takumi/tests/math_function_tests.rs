use takumi::prelude::*;

use crate::test_utils::{block, measure_with_css};

/// Each math function resolves as Chromium resolves it in `margin-left`: in a 400px wide box at a
/// 20px font size, in a viewport 1200px wide.
#[test]
fn math_functions_resolve_as_chromium() {
  let cases = [
    ("calc(-1em * (1 - 0.8))", -4.0),
    ("calc(0.5em + 1em * 0.8)", 26.0),
    ("calc((10px + 20px) * 2)", 60.0),
    ("calc(-1em * 0.25)", -5.0),
    // Chromium clamps to `LayoutUnit::Max()`, which `getComputedStyle` rounds to 3.35544e+07.
    ("calc(10px / 0)", 33554432.0),
    ("min(10px, 20%)", 10.0),
    ("max(10px, 20%)", 80.0),
    ("clamp(50px, 20%, 100px)", 80.0),
    ("clamp(50px, 5%, 100px)", 50.0),
    ("clamp(100px, 50%, 50px)", 100.0),
    ("min(1em, 30px)", 20.0),
    ("max(2em, 3vw, 10px)", 40.0),
    ("calc(min(10px, 5%) + 2px)", 12.0),
    ("round(37px, 10px)", 40.0),
    ("round(up, 31px, 10px)", 40.0),
    ("round(down, 39px, 10px)", 30.0),
    ("round(to-zero, -39px, 10px)", -30.0),
    ("round(35px, 10px)", 40.0),
    ("mod(37px, 10px)", 7.0),
    ("mod(-37px, 10px)", 3.0),
    ("rem(-37px, 10px)", -7.0),
    ("abs(-25px)", 25.0),
    ("calc(10px * sign(-3px))", -10.0),
    ("hypot(30px, 40px)", 50.0),
    ("calc(100px * sin(30deg))", 50.0),
    ("calc(100px * cos(0.25turn))", 0.0),
    ("calc(100px * tan(45deg))", 100.0),
    ("calc(1px * pow(2, 5))", 32.0),
    ("calc(1px * sqrt(144))", 12.0),
    ("calc(1px * log(8, 2))", 3.0),
    ("calc(1px * exp(0))", 1.0),
    ("calc(100px * pi / pi)", 100.0),
    ("calc(1px * atan2(1, 1) / 1deg)", 45.0),
    ("min(10%, 3em)", 40.0),
    ("clamp(10px, 1em + 10%, 200px)", 60.0),
    ("calc(50% - min(20px, 10%))", 180.0),
  ];
  let mismatches: Vec<_> = cases
    .iter()
    .enumerate()
    .filter_map(|(index, &(value, chromium))| {
      let css = format!(
        ".c {{ display: block; width: 400px; font-size: 20px }} .t{index} {{ display: block; height: 1px; margin-left: {value} }}"
      );
      let measured = measure_with_css(
        Node::container([Node::container([])
          .with_class_name(format!("t{index}"))])
        .with_class_name("c"),
        &css,
      );
      let left = measured.children[0].transform[4] - measured.transform[4];

      ((left - chromium).abs() > 0.5).then(|| format!("{value}: {left}, Chromium {chromium}"))
    })
    .collect();

  assert!(mismatches.is_empty(), "{mismatches:#?}");
}

/// `linear()` eases as Chromium eases it, at 0, 250, 500 and 750ms into a 1s animation that grows
/// a box from 0 to 400px. A single point is invalid, which leaves the default `ease`.
#[test]
fn linear_easing_as_chromium() {
  let cases = [
    ("linear(0, 1)", [0.0, 100.0, 200.0, 300.0]),
    ("linear(0, 0.25 75%, 1)", [0.0, 33.3281, 66.6562, 100.0]),
    ("linear(0, 0.5 25% 75%, 1)", [0.0, 200.0, 200.0, 200.0]),
    ("linear(0, 1 50%, 0)", [0.0, 200.0, 400.0, 200.0]),
    ("linear(0 20%, 1 60%)", [0.0, 50.0, 300.0, 550.0]),
    ("linear(0, 0.3, 0.6 80%, 1)", [0.0, 75.0, 150.0, 225.0]),
    ("linear(0.5)", [0.0, 163.391, 320.953, 384.172]),
    ("linear(0, 2 50%, 1 40%)", [0.0, 400.0, 400.0, 400.0]),
  ];
  let mismatches: Vec<_> = cases
    .iter()
    .flat_map(|&(easing, widths)| {
      [0, 250, 500, 750].into_iter().zip(widths).filter_map(move |(delay, chromium)| {
        let css = format!(
          "@keyframes g {{ from {{ width: 0px }} to {{ width: 400px }} }}
           .box {{ display: block; height: 1px; animation: g 1000ms -{delay}ms; animation-timing-function: {easing} }}"
        );
        let width = measure_with_css(Node::container([block("box")]), &css).children[0].width;

        ((width - chromium).abs() > 0.5).then(|| format!("{easing} at {delay}ms: {width}, Chromium {chromium}"))
      })
    })
    .collect();

  assert!(mismatches.is_empty(), "{mismatches:#?}");

  let many_points = format!("linear({})", vec!["0.5"; 5_000].join(", "));

  assert!(AnimationTimingFunction::from_css_str(&many_points).is_err());

  for (easing, chromium) in [
    ("linear(0, 1)", "linear(0 0%, 1 100%)"),
    ("linear(0, 0.25 75%, 1)", "linear(0 0%, 0.25 75%, 1 100%)"),
    (
      "linear(0, 0.5 25% 75%, 1)",
      "linear(0 0%, 0.5 25%, 0.5 75%, 1 100%)",
    ),
    ("linear(0, 1 50%, 0)", "linear(0 0%, 1 50%, 0 100%)"),
    ("linear(0 20%, 1 60%)", "linear(0 20%, 1 60%)"),
    (
      "linear(0, 0.3, 0.6 80%, 1)",
      "linear(0 0%, 0.3 40%, 0.6 80%, 1 100%)",
    ),
    ("linear(0, 2 50%, 1 40%)", "linear(0 0%, 2 50%, 1 50%)"),
  ] {
    let mut css = String::new();

    AnimationTimingFunction::from_css_str(easing)
      .expect("the easing parses")
      .to_css(&mut css)
      .expect("the easing serializes");
    assert_eq!(css, chromium);
  }
}
