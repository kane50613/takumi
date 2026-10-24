use std::fmt::Write as _;

use super::SvgDocument;
use crate::{
  fonts::shaping::testing::{GEIST, register},
  geometry::Point,
  resources::{
    font::{Fonts, FontsSnapshot},
    image::{SvgFill, SvgOp, SvgOpPaint},
  },
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

fn filter_count(ops: &[SvgOp]) -> usize {
  ops
    .iter()
    .filter(|op| matches!(op, SvgOp::PushFilter(_)))
    .count()
}

#[test]
fn a_filter_url_or_a_filter_function_makes_a_filter_layer() {
  for body in [
    r##"<filter id="f"><feOffset dx="1"/></filter><rect width="1" height="1" filter="url(#f)"/>"##,
    r#"<style>rect { filter: blur(2px) }</style><rect width="1" height="1"/>"#,
    r#"<rect width="10" height="10" style="filter: drop-shadow(2px 2px 1px red)"/>"#,
  ] {
    assert_eq!(filter_count(&ops(body)), 1, "{body}");
  }
}

#[test]
fn a_filter_that_names_nothing_hides_the_element_and_none_leaves_it() {
  let missing = ops(r##"<rect width="10" height="10" filter="url(#missing)"/>"##);
  let none = ops(r#"<rect width="10" height="10" filter="none"/>"#);

  assert_eq!(draw_count(&missing), 0);
  assert_eq!((draw_count(&none), filter_count(&none)), (1, 0));
}

#[test]
fn a_switch_ignores_required_features() {
  let ops = ops(
    r#"<switch><rect requiredFeatures="bogus" width="10" height="10" fill="red"/><rect width="10" height="10" fill="blue"/></switch>"#,
  );

  assert_eq!(fills(&ops), [red()]);
}

#[test]
fn only_the_presentation_attributes_blink_maps_are_styled() {
  let ops = SvgDocument::parse(
    r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10" width="10" height="10" background-color="red"><rect width="5" height="5"/></svg>"#,
  )
  .ok()
  .and_then(|document| document.ops(BLACK, None))
  .unwrap_or_default();

  assert_eq!(fills(&ops), [Some(BLACK)]);
}

#[test]
fn a_use_sizes_a_symbol_against_the_viewport_around_it() {
  let ops = ops(
    r##"<symbol id="s" viewBox="0 0 10 10"><rect width="10" height="10"/></symbol>
        <use href="#s" width="50%" height="50%"/>"##,
  );
  let scale = ops.iter().find_map(|op| match op {
    SvgOp::PushTransform(transform) => Some(transform.a),
    _ => None,
  });

  assert!(
    scale.is_some_and(|scale| (scale - 5.0).abs() < 1e-4),
    "{scale:?}"
  );
}

#[test]
fn a_gradient_resolves_its_percentages_against_each_viewport_that_uses_it() {
  let ops = ops(
    r##"<linearGradient id="g" gradientUnits="userSpaceOnUse" x2="50%">
          <stop offset="0" stop-color="red"/><stop offset="1" stop-color="blue"/>
        </linearGradient>
        <rect width="10" height="10" fill="url(#g)"/>
        <svg width="40" height="40" viewBox="0 0 40 40"><rect width="10" height="10" fill="url(#g)"/></svg>"##,
  );
  let ends: Vec<f32> = ops
    .iter()
    .filter_map(|op| match op {
      SvgOp::Draw {
        fill:
          Some(SvgFill {
            paint: SvgOpPaint::Linear { end, .. },
            ..
          }),
        ..
      } => Some(end.x),
      _ => None,
    })
    .collect();

  assert_eq!(ends, [50.0, 20.0]);
}

/// The right edge of each clip path pushed, in order.
fn clip_widths(ops: &[SvgOp]) -> Vec<f32> {
  ops
    .iter()
    .filter_map(|op| match op {
      SvgOp::PushClip { path, .. } => path
        .iter()
        .map(|command| {
          let mut width: f32 = 0.0;

          command.map_points(|point| {
            width = width.max(point.x);
            point
          });
          width
        })
        .reduce(f32::max),
      _ => None,
    })
    .collect()
}

#[test]
fn a_clip_path_clipped_in_bounding_box_units_is_not_shared_between_shapes() {
  let widths = clip_widths(&ops(
    r##"<clipPath id="outer" clip-path="url(#inner)"><rect width="100" height="100"/></clipPath>
        <clipPath id="inner" clipPathUnits="objectBoundingBox"><rect width="1" height="1"/></clipPath>
        <rect width="10" height="10" clip-path="url(#outer)"/>
        <rect width="20" height="20" clip-path="url(#outer)"/>"##,
  ));

  assert!(
    widths.contains(&10.0) && widths.contains(&20.0),
    "{widths:?}"
  );
}

#[test]
fn a_clip_path_resolves_its_percentages_in_the_viewport_of_each_shape() {
  let widths = clip_widths(&ops(
    r##"<clipPath id="c"><rect width="50%" height="50%"/></clipPath>
        <rect width="100" height="100" clip-path="url(#c)"/>
        <svg width="40" height="40"><rect width="40" height="40" clip-path="url(#c)"/></svg>"##,
  ));

  assert_eq!((widths.first(), widths.last()), (Some(&50.0), Some(&20.0)));
}

#[test]
fn a_rect_radius_below_zero_clamps_to_a_square_corner() {
  let ops = ops(r#"<rect width="20" height="20" style="rx: calc(10% - 20px)"/>"#);
  let first = ops.iter().find_map(|op| match op {
    SvgOp::Draw { path, .. } => path.first().cloned(),
    _ => None,
  });
  let mut start = None;

  first.map(|command| {
    command.map_points(|point| {
      start.get_or_insert(point);
      point
    })
  });

  assert_eq!(start.map(|point| point.x), Some(0.0));
}

#[test]
fn a_long_chain_of_clip_paths_is_walked_once_for_all_its_references() {
  let mut body = String::new();

  for link in 0..1000 {
    let _ = write!(
      body,
      r##"<clipPath id="c{link}" clip-path="url(#c{})"><rect width="1" height="1"/></clipPath>"##,
      link + 1
    );
  }

  body.push_str(r##"<clipPath id="c1000"><rect width="1" height="1"/></clipPath>"##);
  body.push_str(&r##"<rect width="1" height="1" clip-path="url(#c0)"/>"##.repeat(1000));

  // The chain is deeper than a document may nest, so the shapes are hidden, but the walk ends.
  assert!(with_deep_stack(move || draw_count(&ops(&body))).is_some());
}

fn geist() -> FontsSnapshot {
  let mut fonts = Fonts::default();

  register(&mut fonts, GEIST, "Geist");
  fonts.snapshot()
}

/// The ops of a document whose `font-family` is Geist, set on the root so that text inherits it.
fn text_ops(body: &str) -> Vec<SvgOp> {
  let markup = svg(&format!(
    r#"<g font-family="Geist" font-size="20">{body}</g>"#
  ));

  SvgDocument::parse(&markup)
    .ok()
    .and_then(|document| document.ops(BLACK, Some(&geist())))
    .unwrap_or_default()
}

/// The horizontal and vertical extent of what the ops draw, ignoring transforms.
fn extent(ops: &[SvgOp]) -> Option<(f32, f32, f32, f32)> {
  let mut points = Vec::new();

  for op in ops {
    if let SvgOp::Draw { path, .. } = op {
      for command in path {
        command.map_points(|point| {
          points.push(point);

          point
        });
      }
    }
  }

  let fold = |value: fn(&Point<f32>) -> f32, merge: fn(f32, f32) -> f32| {
    points.iter().map(value).reduce(merge)
  };

  Some((
    fold(|point| point.x, f32::min)?,
    fold(|point| point.x, f32::max)?,
    fold(|point| point.y, f32::min)?,
    fold(|point| point.y, f32::max)?,
  ))
}

#[test]
fn text_draws_nothing_without_a_font() {
  let markup = svg(r#"<text x="10" y="50">Hi</text>"#);
  let ops = SvgDocument::parse(&markup)
    .ok()
    .and_then(|document| document.ops(BLACK, None))
    .unwrap_or_default();

  assert_eq!(draw_count(&ops), 0);
}

#[test]
fn text_is_drawn_as_filled_glyph_outlines() {
  let ops = text_ops(r#"<text x="10" y="50">Hi</text>"#);

  assert_eq!(fills(&ops), [Some(BLACK)]);
  assert!(extent(&ops).is_some_and(|(left, ..)| left >= 10.0));
}

#[test]
fn x_places_text_and_text_anchor_moves_it_back() {
  let start = extent(&text_ops(r#"<text x="50" y="50">Hi</text>"#));
  let middle = extent(&text_ops(
    r#"<text x="50" y="50" text-anchor="middle">Hi</text>"#,
  ));
  let end = extent(&text_ops(
    r#"<text x="50" y="50" text-anchor="end">Hi</text>"#,
  ));
  let (Some(start), Some(middle), Some(end)) = (start, middle, end) else {
    panic!("text is drawn");
  };

  assert!(start.0 >= 50.0);
  assert!(middle.0 < 50.0 && middle.1 > 50.0);
  assert!(end.1 <= 50.5);
}

#[test]
fn dx_and_dy_shift_the_characters_that_follow() {
  let plain = extent(&text_ops(r#"<text x="10" y="50">Hi</text>"#));
  let shifted = extent(&text_ops(
    r#"<text x="10" y="50"><tspan dx="7" dy="-5">Hi</tspan></text>"#,
  ));
  let (Some(plain), Some(shifted)) = (plain, shifted) else {
    panic!("text is drawn");
  };

  assert!((shifted.0 - plain.0 - 7.0).abs() < 0.01);
  assert!((shifted.2 - plain.2 + 5.0).abs() < 0.01);
}

#[test]
fn white_space_collapses_as_xml_space_says() {
  let collapsed = extent(&text_ops("<text x=\"0\" y=\"50\">a \n   b</text>"));
  let single = extent(&text_ops(r#"<text x="0" y="50">a b</text>"#));
  let preserved = extent(&text_ops(
    "<text x=\"0\" y=\"50\" xml:space=\"preserve\">a   b</text>",
  ));

  assert_eq!(collapsed, single);
  assert!(preserved.is_some_and(|preserved| preserved.1 > collapsed.map_or(0.0, |it| it.1)));
}

#[test]
fn rotate_turns_the_characters() {
  let upright = extent(&text_ops(r#"<text x="10" y="50">I</text>"#));
  let turned = extent(&text_ops(r#"<text x="10" y="50" rotate="90">I</text>"#));
  let (Some(upright), Some(turned)) = (upright, turned) else {
    panic!("text is drawn");
  };

  // A turned upright stroke is wider than tall.
  assert!(upright.3 - upright.2 > upright.1 - upright.0);
  assert!(turned.3 - turned.2 < turned.1 - turned.0);
}

#[test]
fn a_text_path_sets_text_along_a_curve() {
  let ops = text_ops(
    r##"<defs><path id="p" d="M 10 80 Q 50 0 90 80"/></defs>
        <text><textPath href="#p">Curved text</textPath></text>"##,
  );
  let (_, _, top, bottom) = extent(&ops).expect("text is drawn");

  assert!(bottom - top > 20.0);
}

#[test]
fn a_tref_draws_nothing_as_blink_removed_it() {
  let ops = text_ops(
    r##"<defs><text id="t">Hi</text></defs><text x="10" y="50"><tref href="#t"/></text>"##,
  );

  assert_eq!(draw_count(&ops), 0);
}

#[test]
fn a_hidden_tspan_hides_the_spans_below_it() {
  let ops = text_ops(
    r#"<text x="10" y="50"><tspan display="none">a<tspan display="inline">b</tspan></tspan></text>"#,
  );

  assert_eq!(draw_count(&ops), 0);
}

#[test]
fn a_position_list_with_an_invalid_item_is_ignored() {
  let valid = extent(&text_ops(r#"<text x="40" y="50">a</text>"#));
  let invalid = extent(&text_ops(r#"<text x="40 foo" y="50">a</text>"#));

  assert!(valid.is_some_and(|(left, ..)| left >= 40.0));
  assert!(invalid.is_some_and(|(left, ..)| left < 40.0));
}

#[test]
fn stretching_zero_width_glyphs_draws_finite_outlines() {
  let ops = text_ops(
    "<text x=\"10\" y=\"50\" textLength=\"50\" lengthAdjust=\"spacingAndGlyphs\">\u{200B}</text>",
  );

  for op in &ops {
    if let SvgOp::Draw { path, .. } = op {
      for command in path {
        command.map_points(|point| {
          assert!(point.x.is_finite() && point.y.is_finite());
          point
        });
      }
    }
  }
}

#[test]
fn text_decoration_adds_a_line() {
  let plain = draw_count(&text_ops(r#"<text x="10" y="50">Hi</text>"#));
  let underlined = draw_count(&text_ops(
    r#"<text x="10" y="50" text-decoration="underline">Hi</text>"#,
  ));

  assert_eq!(underlined, plain + 1);
}

#[test]
fn hidden_text_is_not_drawn() {
  let ops = text_ops(r#"<text x="10" y="50" display="none">Hi</text>"#);

  assert_eq!(draw_count(&ops), 0);
}

#[test]
fn a_tref_bomb_shapes_within_the_text_budget() {
  let target = "a".repeat(20_000);
  let trefs = r##"<tref href="#t"/>"##.repeat(100);
  let ops = text_ops(&format!(
    r##"<defs><text id="t">{target}</text></defs><text x="0" y="50">{trefs}</text>"##
  ));

  assert!(ops.is_empty());
}

#[test]
fn ten_thousand_trefs_to_a_large_text_end_within_the_budget() {
  let target = "a".repeat(100_000);
  let trefs = r##"<tref href="#t"/>"##.repeat(10_000);
  let ops = text_ops(&format!(
    r##"<defs><text id="t">{target}</text></defs><text x="0" y="50">{trefs}</text>"##
  ));

  assert!(ops.is_empty());
}

#[test]
fn ten_thousand_text_paths_to_a_huge_path_end_within_the_budget() {
  let path = format!("M 0 0 {}", "L 1 1 L 2 0 ".repeat(50_000));
  let text_paths = r##"<textPath href="#p">a</textPath>"##.repeat(10_000);
  let ops = text_ops(&format!(
    r##"<defs><path id="p" d="{path}"/></defs><text>{text_paths}</text>"##
  ));

  assert!(ops.is_empty());
}

#[test]
fn a_long_position_list_used_many_times_is_parsed_once() {
  let positions = "1 ".repeat(200_000);
  let uses = r##"<use href="#t"/>"##.repeat(2_000);

  text_ops(&format!(
    r##"<defs><text id="t" x="{positions}" rotate="{positions}" y="50">a</text></defs>{uses}"##
  ));
}

#[test]
fn text_on_a_path_of_huge_coordinates_ends() {
  let text = "a".repeat(2_000);
  let ops = text_ops(&format!(
    r##"<defs><path id="p" d="M 0 0 C 1e30 -1e30 -1e30 1e30 1e30 1e30"/></defs><text><textPath href="#p">{text}</textPath></text>"##
  ));

  assert!(draw_count(&ops) <= 2_000);
}

#[test]
fn a_text_of_a_thousand_tspans_used_a_thousand_times_ends_within_the_instance_budget() {
  let spans = "<tspan/>".repeat(1000);
  let uses = r##"<use href="#t"/>"##.repeat(1000);
  let ops = text_ops(&format!(
    r##"<defs><text id="t" y="50">{spans}</text></defs>{uses}"##
  ));

  assert_eq!(draw_count(&ops), 0);
}

#[test]
fn bitmap_glyphs_stop_at_the_image_byte_budget() {
  let mut fonts = Fonts::default();

  register(
    &mut fonts,
    "../assets/fonts/noto-sans/NotoColorEmoji.ttf",
    "Emoji",
  );

  let emoji = "\u{1F600}".repeat(5_000);
  let markup = svg(&format!(
    r#"<text font-family="Emoji" font-size="200" y="50">{emoji}</text>"#
  ));
  let ops = SvgDocument::parse(&markup)
    .ok()
    .and_then(|document| document.ops(BLACK, Some(&fonts.snapshot())))
    .unwrap_or_default();
  let images = ops
    .iter()
    .filter(|op| matches!(op, SvgOp::Image { .. }))
    .count();

  assert!(images > 0 && images < 5_000, "{images}");
}

#[test]
fn a_hundred_thousand_glyphs_paint_within_the_budget() {
  let text = "g@%&amp;".repeat(25_000);
  let ops = text_ops(&format!(r#"<text x="0" y="50">{text}</text>"#));

  // Their outlines outgrow what a document may paint, so it paints nothing.
  assert!(ops.is_empty());
}

#[test]
fn nested_decorated_groups_around_nested_tspans_do_not_cost_quadratic_time() {
  let depth = 250;
  let groups = r#"<g text-decoration="underline">"#.repeat(depth);
  let spans = "<tspan>a".repeat(depth);
  let body = format!(
    r#"{groups}<text x="0" y="50">{spans}{}</text>{}"#,
    "</tspan>".repeat(depth),
    "</g>".repeat(depth),
  );

  assert!(with_deep_stack(move || draw_count(&text_ops(&body))).is_some());
}

#[test]
fn a_plain_space_attribute_is_not_xml_space() {
  let preserved = extent(&text_ops(
    r#"<text x="0" y="50" space="preserve">a   b</text>"#,
  ));
  let collapsed = extent(&text_ops(r#"<text x="0" y="50">a b</text>"#));

  assert_eq!(preserved, collapsed);
}

#[test]
fn a_tspan_with_x_inside_a_text_path_stays_on_the_path() {
  let ops = text_ops(
    r##"<defs><path id="p" d="M 0 0 L 100 100"/></defs>
        <text><textPath href="#p"><tspan>a</tspan><tspan x="50">b</tspan></textPath></text>"##,
  );
  let (_, _, _, bottom) = extent(&ops).expect("text is drawn");

  assert!(bottom > 30.0, "{bottom}");
}

#[test]
fn dx_follows_the_characters_it_names_after_a_hidden_span() {
  let plain = extent(&text_ops(
    r#"<text x="10" y="50">a<tspan display="none">b</tspan>cd</text>"#,
  ));
  let shifted = extent(&text_ops(
    r#"<text x="10" y="50" dx="0 0 0 40">a<tspan display="none">b</tspan>cd</text>"#,
  ));
  let (Some(plain), Some(shifted)) = (plain, shifted) else {
    panic!("text is drawn");
  };

  assert!(
    (shifted.1 - plain.1 - 40.0).abs() < 0.01,
    "{plain:?} {shifted:?}"
  );
}

#[test]
fn a_negative_dx_on_a_text_path_hides_only_the_character_it_moves_off_the_path() {
  let on_path = |attributes: &str, text: &str| {
    let ops = text_ops(&format!(
      r##"<defs><path id="p" d="M 0 50 L 100 50"/></defs>
          <text {attributes}><textPath href="#p">{text}</textPath></text>"##
    ));

    ops
      .iter()
      .find_map(|op| match op {
        SvgOp::Draw { path, .. } => Some(path.len()),
        _ => None,
      })
      .unwrap_or_default()
  };

  assert!(on_path("", "ac") > 0);
  assert_eq!(on_path(r#"dx="0 -60 80""#, "abc"), on_path("", "ac"));
}

#[test]
fn a_hundred_thousand_positions_and_a_long_dash_array_end_within_the_budget() {
  let positions: String = (0..100_000).map(|index| format!("{index} ")).collect();
  let dashes: String = (0..100_000).map(|_| "1 ").collect();
  let characters = "a".repeat(100_000);
  let ops = text_ops(&format!(
    r#"<text x="{positions}" y="50" stroke="black" stroke-dasharray="{dashes}">{characters}</text>"#
  ));

  assert!(draw_count(&ops) <= 100_000);
}

#[test]
fn spans_with_one_font_kern_across_their_boundary() {
  let whole = extent(&text_ops(r#"<text x="20" y="64">AVAV</text>"#));
  let split = extent(&text_ops(
    r#"<text x="20" y="64"><tspan>AV</tspan><tspan>AV</tspan></text>"#,
  ));

  assert_eq!(whole, split);
}
