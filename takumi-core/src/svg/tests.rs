use std::fmt::Write as _;

use super::SvgDocument;
use crate::{
  resources::image::{SvgFill, SvgOp, SvgOpPaint},
  style::Color,
};

const BLACK: Color = Color([0, 0, 0, 255]);

fn svg(body: &str) -> String {
  format!(
    r##"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" viewBox="0 0 100 100" width="100" height="100">{body}</svg>"##
  )
}

fn ops(body: &str) -> Vec<SvgOp> {
  let document = SvgDocument::parse(&svg(body));

  assert!(document.is_ok(), "{document:?}");

  document
    .ok()
    .and_then(|document| document.ops(BLACK, None))
    .unwrap_or_default()
}

fn fill_color(fill: &SvgFill) -> Option<Color> {
  match fill.paint {
    SvgOpPaint::Color(color) => Some(color),
    _ => None,
  }
}

/// The color each drawn shape is filled with, in order.
fn fills(ops: &[SvgOp]) -> Vec<Option<Color>> {
  ops
    .iter()
    .filter_map(|op| match op {
      SvgOp::Draw { fill, .. } => Some(fill.as_ref().and_then(fill_color)),
      _ => None,
    })
    .collect()
}

fn red() -> Option<Color> {
  Some(Color([255, 0, 0, 255]))
}

fn blue() -> Option<Color> {
  Some(Color([0, 0, 255, 255]))
}

fn draw_count(ops: &[SvgOp]) -> usize {
  ops
    .iter()
    .filter(|op| matches!(op, SvgOp::Draw { .. }))
    .count()
}

#[test]
fn a_rule_beats_a_presentation_attribute_and_a_style_attribute_beats_a_rule() {
  let ops = ops(
    r#"<style>rect { fill: blue } .inline { fill: blue }</style>
       <rect width="10" height="10" fill="red"/>
       <rect class="inline" width="10" height="10" style="fill: red"/>
       <rect width="10" height="10" fill="red" style="fill: blue"/>"#,
  );

  assert_eq!(fills(&ops), [blue(), red(), blue()]);
}

#[test]
fn a_presentation_attribute_beats_the_inherited_value() {
  let ops = ops(
    r#"<g fill="blue"><rect width="10" height="10"/><rect width="10" height="10" fill="red"/></g>"#,
  );

  assert_eq!(fills(&ops), [blue(), red()]);
}

#[test]
fn the_host_color_is_what_current_color_is() {
  let document = SvgDocument::parse(&svg(
    r#"<rect width="10" height="10" fill="currentColor"/>"#,
  ));
  let color = Color([10, 20, 30, 255]);
  let ops = document
    .ok()
    .and_then(|document| document.ops(color, None))
    .unwrap_or_default();

  assert_eq!(fills(&ops), [Some(color)]);
}

#[test]
fn unknown_elements_count_for_sibling_selectors_and_draw_nothing() {
  let ops = ops(
    r#"<rect width="10" height="10"/><foo><rect width="5" height="5"/></foo><rect width="10" height="10"/>
       <style>rect:nth-child(3) { fill: red }</style>"#,
  );

  assert_eq!(fills(&ops), [Some(BLACK), red()]);
}

#[test]
fn an_instance_takes_the_rules_of_its_original_and_the_style_of_its_use() {
  let ops = ops(
    r##"<style>#target { stroke: blue }</style>
        <defs><rect id="target" width="10" height="10"/></defs>
        <g fill="red"><use xlink:href="#target"/></g>"##,
  );

  let drawn = ops.iter().find_map(|op| match op {
    SvgOp::Draw { fill, stroke, .. } => {
      Some((fill.as_ref().and_then(fill_color), stroke.is_some()))
    }
    _ => None,
  });

  assert_eq!(drawn, Some((red(), true)));
}

#[test]
fn a_clip_path_that_clips_itself_ends() {
  let ops = ops(
    r##"<clipPath id="c" clip-path="url(#c)"><rect width="10" height="10"/></clipPath>
        <rect width="20" height="20" clip-path="url(#c)"/>"##,
  );

  assert_eq!(draw_count(&ops), 1);
}

#[test]
fn clip_paths_that_clip_each_other_end() {
  let ops = ops(
    r##"<clipPath id="c" clip-path="url(#d)"><rect width="10" height="10"/></clipPath>
        <clipPath id="d" clip-path="url(#c)"><rect width="10" height="10"/></clipPath>
        <rect width="20" height="20" clip-path="url(#c)"/>"##,
  );

  assert_eq!(draw_count(&ops), 1);
}

#[test]
fn a_mask_that_masks_itself_ends() {
  let ops = ops(
    r##"<mask id="m" mask="url(#m)"><rect width="10" height="10" fill="white"/></mask>
        <rect width="20" height="20" mask="url(#m)"/>"##,
  );

  assert_eq!(draw_count(&ops), 1);
}

#[test]
fn a_mask_whose_content_uses_it_ends() {
  let ops = ops(
    r##"<mask id="m"><rect width="10" height="10" fill="white" mask="url(#m)"/></mask>
        <rect width="20" height="20" mask="url(#m)"/>"##,
  );

  assert_eq!(draw_count(&ops), 1);
}

#[test]
fn a_use_bomb_lays_out_within_the_instance_budget() {
  let mut body = String::from(r#"<defs><rect id="g0" width="1" height="1"/>"#);

  for level in 1..=10 {
    let _ = write!(body, r#"<g id="g{level}">"#);

    for _ in 0..10 {
      let _ = write!(body, r##"<use xlink:href="#g{}"/>"##, level - 1);
    }

    body.push_str("</g>");
  }

  body.push_str(r##"</defs><use xlink:href="#g10"/>"##);

  let drawn = draw_count(&ops(&body));

  assert!(drawn > 0);
  assert!(drawn <= 100_000, "{drawn}");
}

/// A document whose `use` elements each name the group before it, `fan_out` times, over `levels`
/// groups, around `leaf`.
fn use_pyramid(defs: &str, leaf: &str, levels: usize, fan_out: usize) -> String {
  let mut body = format!(r#"<defs>{defs}<g id="g0">{leaf}</g>"#);

  for level in 1..=levels {
    let _ = write!(body, r#"<g id="g{level}">"#);

    for _ in 0..fan_out {
      let _ = write!(body, r##"<use xlink:href="#g{}"/>"##, level - 1);
    }

    body.push_str("</g>");
  }

  let _ = write!(body, r##"</defs><use xlink:href="#g{levels}"/>"##);
  body
}

/// Runs `lay_out` on a thread with room for the deepest nesting a document may have.
fn with_deep_stack<R: Send + 'static>(lay_out: impl FnOnce() -> R + Send + 'static) -> Option<R> {
  std::thread::Builder::new()
    .stack_size(16 << 20)
    .spawn(lay_out)
    .ok()?
    .join()
    .ok()
}

#[test]
fn a_chain_of_five_thousand_uses_ends() {
  let mut body = String::from(r#"<defs><rect id="g0" width="1" height="1"/>"#);

  for hop in 1..=5000 {
    let _ = write!(
      body,
      r##"<g id="g{hop}"><use xlink:href="#g{}"/></g>"##,
      hop - 1
    );
  }

  body.push_str(r##"</defs><use xlink:href="#g5000"/>"##);

  assert_eq!(with_deep_stack(move || draw_count(&ops(&body))), Some(0));
}

#[test]
fn a_mask_referenced_a_hundred_thousand_times_paints_within_the_budget() {
  let mask_content = r#"<rect width="1" height="1" fill="white"/>"#.repeat(50);
  let defs = format!(
    r#"<mask id="m" maskUnits="userSpaceOnUse" x="0" y="0" width="10" height="10">{mask_content}</mask>"#
  );
  let body = use_pyramid(
    &defs,
    r##"<rect width="1" height="1" mask="url(#m)"/>"##,
    3,
    47,
  );
  let painted = ops(&body);

  // The masks outgrow what a document may paint, so it paints nothing.
  assert!(painted.is_empty());
}

#[test]
fn an_image_used_ten_thousand_times_is_decoded_once() {
  let png = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==";
  let image = format!(r#"<image width="1" height="1" xlink:href="data:image/png;base64,{png}"/>"#);
  let painted = ops(&use_pyramid("", &image, 2, 100));
  let images = painted
    .iter()
    .filter(|op| matches!(op, SvgOp::Image { .. }))
    .count();

  assert_eq!(images, 10_000);
}

#[test]
fn deeply_nested_groups_do_not_overflow_the_stack() {
  let depth = 500;
  let body = format!(
    "{}<rect width=\"10\" height=\"10\"/>{}",
    "<g>".repeat(depth),
    "</g>".repeat(depth)
  );

  // A debug build spends several kilobytes of stack on each level, so the test thread gets more
  // than the default.
  let drawn = std::thread::Builder::new()
    .stack_size(4 << 20)
    .spawn(move || draw_count(&ops(&body)))
    .map(|handle| handle.join().ok());

  assert_eq!(drawn.ok().flatten(), Some(1));
}

#[test]
fn text_and_filters_are_left_to_the_legacy_renderer() {
  for body in [
    r#"<text>hi</text>"#,
    r##"<filter id="f"><feOffset dx="1"/></filter><rect width="1" height="1" filter="url(#f)"/>"##,
    r#"<style>rect { filter: blur(2px) }</style><rect width="1" height="1"/>"#,
  ] {
    let document = SvgDocument::parse(&svg(body));

    assert!(
      document.is_ok_and(|document| document.needs_legacy_renderer()),
      "{body}"
    );
  }
}
