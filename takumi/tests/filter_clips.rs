use takumi::{prelude::*, render};

fn case(overflow: &str, filter: &str, extra: &str, content: &str) -> (Node, StyleSheet) {
  let html = format!("<div class='outer'><div class='source'>{content}</div></div>");
  let css = format!(
    ".outer {{ position: absolute; left: 80px; top: 40px; width: 160px; height: 160px; overflow: {overflow}; }}
     .source {{ position: absolute; left: -60px; top: 20px; width: 120px; height: 120px; background: white; filter: {filter}; }}
     {extra}"
  );
  let node = Node::from_html(
    &html,
    FromHtmlOptions::builder()
      .presets(StylePresets::empty())
      .build(),
  )
  .unwrap();

  (node, StyleSheet::parse(&css).unwrap())
}

fn render_case(overflow: &str, filter: &str, extra: &str, content: &str) -> Bitmap {
  let (node, stylesheet) = case(overflow, filter, extra, content);
  let fonts = Fonts::default();

  render(
    RenderOptions::builder()
      .viewport(Viewport::new((320, 240)))
      .node(node)
      .stylesheet(stylesheet.into())
      .fonts(&fonts)
      .build(),
  )
  .unwrap()
}

#[test]
fn svg_clips_filter_output_after_blurring() {
  let (node, stylesheet) = case(
    "visible",
    "blur(20px)",
    ".source { clip-path: inset(0 0 0 60px); }",
    "",
  );
  let fonts = Fonts::default();
  let svg = takumi_svg::render(
    takumi_svg::SvgOptions::builder()
      .viewport(Viewport::new((320, 240)))
      .node(node)
      .stylesheet(stylesheet.into())
      .fonts(&fonts)
      .build(),
  )
  .unwrap();
  let image = crate::test_utils::rasterize_svg(svg, 320, 240);
  let pixels = image.as_raw().as_chunks::<4>().0;

  assert_eq!(
    pixels[120 * 320 + 79][3],
    0,
    "clip-path must clip the output"
  );
  assert!(
    pixels[120 * 320 + 80][3] > 240,
    "clip-path removed filter input"
  );
}

#[test]
fn ancestor_overflow_clips_filtered_output() {
  for filter in [
    "blur(20px)",
    "brightness(0.5)",
    "drop-shadow(40px 0 12px white)",
    "drop-shadow(-40px 0 12px white)",
    "blur(8px) drop-shadow(30px 0 10px white)",
  ] {
    for extra in [
      "",
      ".outer { opacity: 0.5; }",
      ".source { opacity: 0.5; }",
      ".outer, .source { display: block; }",
      ".source { transform: rotate(15deg); opacity: 0.5; }",
    ] {
      let visible = render_case("visible", filter, extra, "");
      let hidden = render_case("hidden", filter, extra, "");
      let visible = visible.as_raw().as_chunks::<4>().0;
      let hidden = hidden.as_raw().as_chunks::<4>().0;

      assert!(visible[120 * 320 + 80][3] > 0, "{filter}: no ink");

      for y in 40..200 {
        for x in 80..240 {
          assert_eq!(
            visible[y * 320 + x],
            hidden[y * 320 + x],
            "{filter}, {extra}, ({x}, {y}): ancestor overflow clipped the filter input"
          );
        }
      }

      assert_eq!(hidden[120 * 320 + 79][3], 0, "{filter}: output escaped");
    }
  }
}

#[test]
fn descendant_overflow_still_clips_filter_input() {
  let extra = ".source { background: transparent; } .inner { position: absolute; left: 40px; top: 20px; width: 40px; height: 80px; overflow: hidden; } .ink { position: absolute; left: -40px; width: 80px; height: 80px; background: white; }";
  let clipped = render_case(
    "hidden",
    "blur(10px)",
    extra,
    "<div class='inner'><div class='ink'></div></div>",
  );
  let expected = render_case(
    "hidden",
    "blur(10px)",
    ".source { background: transparent; } .ink { position: absolute; left: 40px; top: 20px; width: 40px; height: 80px; background: white; }",
    "<div class='ink'></div>",
  );

  assert!(
    clipped.as_raw() == expected.as_raw(),
    "descendant overflow must clip before filtering"
  );
  assert!(
    clipped.as_raw()[(120 * 320 + 80) * 4 + 3] > 0,
    "no ink painted"
  );
}

#[test]
fn displaced_shadow_reads_off_canvas_source() {
  let reference = render_case("hidden", "drop-shadow(100px 0 12px white)", "", "");
  let off_canvas = render_case(
    "hidden",
    "drop-shadow(300px 0 12px white)",
    ".source { left: -260px; }",
    "",
  );
  let reference = reference.as_raw().as_chunks::<4>().0;
  let off_canvas = off_canvas.as_raw().as_chunks::<4>().0;

  assert!(reference[120 * 320 + 180][3] > 0, "no shadow painted");

  for y in 80..160 {
    for x in 160..240 {
      assert_eq!(reference[y * 320 + x], off_canvas[y * 320 + x]);
    }
  }
}

#[test]
fn own_shape_masks_clip_filtered_output() {
  let reference = render_case("visible", "blur(20px)", "", "");
  let reference = reference.as_raw().as_chunks::<4>().0;

  for mask in [
    "clip-path: inset(0 0 0 60px)",
    "mask-image: linear-gradient(to right, transparent 50%, black 50%)",
  ] {
    let masked = render_case(
      "visible",
      "blur(20px)",
      &format!(".source {{ {mask}; }}"),
      "",
    );
    let masked = masked.as_raw().as_chunks::<4>().0;

    assert_eq!(masked[120 * 320 + 79][3], 0, "{mask}: output escaped");

    for y in 100..140 {
      for x in 80..120 {
        assert_eq!(
          reference[y * 320 + x],
          masked[y * 320 + x],
          "{mask}, ({x}, {y}): the shape mask clipped the filter input"
        );
      }
    }
  }
}

#[test]
fn mask_image_respects_the_rounded_border_box() {
  let image = render_case(
    "visible",
    "blur(20px)",
    ".source { border-radius: 50%; mask-image: linear-gradient(to right, transparent 50%, black 50%); }",
    "",
  );
  let pixels = image.as_raw().as_chunks::<4>().0;

  assert_eq!(
    pixels[60 * 320 + 90][3],
    0,
    "the mask escaped the rounded border"
  );
  assert!(pixels[120 * 320 + 80][3] > 0, "the mask removed the center");
}

#[test]
fn backdrop_effect_respects_opacity_and_ancestor_clip() {
  let render_backdrop = |effect: &str| {
    let html = "<div class='root'><div class='outer'><div class='glass'></div></div></div>";
    let css = format!(
      ".root {{ width: 320px; height: 240px; background: red; }}
       .outer {{ position: absolute; left: 100px; top: 40px; width: 80px; height: 160px; overflow: hidden; }}
       .glass {{ position: absolute; left: -40px; width: 160px; height: 160px; {effect} }}"
    );
    let fonts = Fonts::default();
    let node = Node::from_html(
      html,
      FromHtmlOptions::builder()
        .presets(StylePresets::empty())
        .build(),
    )
    .unwrap();

    render(
      RenderOptions::builder()
        .viewport(Viewport::new((320, 240)))
        .node(node)
        .stylesheet(StyleSheet::parse(&css).unwrap().into())
        .fonts(&fonts)
        .build(),
    )
    .unwrap()
  };
  let plain = render_backdrop("");
  let invisible = render_backdrop("opacity: 0; backdrop-filter: invert(1);");
  let half = render_backdrop("opacity: 0.5; backdrop-filter: invert(1);");
  let filtered = render_backdrop("backdrop-filter: invert(1);");
  let plain = plain.as_raw().as_chunks::<4>().0;
  let invisible = invisible.as_raw().as_chunks::<4>().0;
  let half = half.as_raw().as_chunks::<4>().0;
  let filtered = filtered.as_raw().as_chunks::<4>().0;

  assert_eq!(invisible[120 * 320 + 120], plain[120 * 320 + 120]);
  assert!(
    half[120 * 320 + 120]
      .iter()
      .zip([127u8, 128, 128, 255])
      .all(|(&actual, expected)| actual.abs_diff(expected) <= 1)
  );
  assert_eq!(filtered[120 * 320 + 80], plain[120 * 320 + 80]);
  assert_ne!(filtered[120 * 320 + 120], plain[120 * 320 + 120]);
}

#[test]
fn nested_backdrops_do_not_read_the_filtered_parent_backdrop() {
  let html = "<div style='display:flex;position:relative;width:320px;height:240px;background:red'><div style='display:flex;position:absolute;left:20px;top:20px;width:280px;height:200px;backdrop-filter:invert(1)'><div style='display:flex;position:absolute;left:40px;top:40px;width:160px;height:80px;backdrop-filter:invert(1)'></div></div></div>";
  let fonts = Fonts::default();
  let node = Node::from_html(
    html,
    FromHtmlOptions::builder()
      .presets(StylePresets::empty())
      .build(),
  )
  .unwrap();
  let image = render(
    RenderOptions::builder()
      .viewport(Viewport::new((320, 240)))
      .node(node)
      .fonts(&fonts)
      .build(),
  )
  .unwrap();
  let pixels = image.as_raw().as_chunks::<4>().0;

  assert_eq!(pixels[100 * 320 + 120], [0, 255, 255, 255]);
}
