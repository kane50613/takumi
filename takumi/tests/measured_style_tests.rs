mod test_utils;

use takumi::{measure, prelude::*};
use test_utils::CONTEXT;

fn measure_with_include_styles(
  html: &str,
  viewport: Viewport,
  include_styles: bool,
) -> MeasuredNode {
  let node = Node::from_html(html, FromHtmlOptions::default()).expect("parse");

  measure(
    RenderOptions::builder()
      .viewport(viewport)
      .node(node)
      .fonts(&CONTEXT)
      .include_styles(include_styles)
      .build(),
  )
  .unwrap()
}

fn measure_with_styles(html: &str) -> MeasuredNode {
  measure_with_include_styles(html, Viewport::new((1200, 630)), true)
}

fn measure_with_renamed_face(html: &str, family: &str) -> MeasuredNode {
  let bytes = std::fs::read(
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
      .join("../assets/fonts/geist/Geist[wght].woff2"),
  )
  .expect("read font");
  let mut fonts = Fonts::default();

  fonts
    .register(FontResource::new(bytes).override_info(FontOverride {
      family_name: Some(family.into()),
      ..FontOverride::default()
    }))
    .expect("register font");

  let node = Node::from_html(html, FromHtmlOptions::default()).expect("parse");

  measure(
    RenderOptions::builder()
      .viewport(Viewport::new((1200, 630)))
      .node(node)
      .fonts(&fonts)
      .include_styles(true)
      .build(),
  )
  .unwrap()
}

#[test]
fn run_style_reports_the_family_the_face_was_registered_under() {
  let measured = measure_with_renamed_face(
    r#"<div style="width:400px; font-family:'Deck Sans'; font-size:20px">hello</div>"#,
    "Deck Sans",
  );

  let run = measured.runs.first().expect("a measured run");
  let style = run.style.as_ref().expect("run style");

  // The face's own `name` table says "Geist"; the caller named it something
  // else, and that is the name a consumer checking for a substitution needs.
  assert_eq!(style.font_family, "Deck Sans");
}

#[test]
fn run_style_reports_the_opacity_of_the_inline_element_it_came_from() {
  let measured = measure_with_styles(
    r#"<div style="width:400px; font-size:20px">plain <span style="opacity:0.4">faded</span></div>"#,
  );

  // The span generates no box, so its opacity reaches a consumer only through the run. The div's
  // own opacity stays 1: multiplying the two is the consumer's job, as it is for nested boxes.
  let faded = measured
    .runs
    .iter()
    .find(|run| run.text.contains("faded"))
    .expect("the faded run");

  assert_eq!(measured.style.as_ref().expect("node style").opacity, 1.0);
  assert_eq!(faded.style.as_ref().expect("run style").opacity, 0.4);
}

#[test]
fn a_floated_box_reports_the_style_it_paints_with() {
  let measured = measure_with_styles(
    r#"<div style="width:400px; font-size:20px"><div style="float:left; width:40px; height:40px; background:#C2410C"></div>text</div>"#,
  );

  // A float is positioned by the inline layout rather than walked into, so its style reaches a
  // consumer only if the box it is handed back as carries it.
  let floated = measured
    .children
    .iter()
    .find_map(|child| child.style.as_ref())
    .expect("the floated box's style");

  assert_eq!(floated.background_color.as_deref(), Some("rgb(194, 65, 12)"));
}

#[test]
fn styles_are_absent_by_default() {
  let html = r#"<div style="width:200px; color:#14110f"><span>hello</span></div>"#;
  let measured = measure_with_include_styles(html, Viewport::new((1200, 630)), false);

  assert!(measured.style.is_none());
  assert!(measured.children.iter().all(|child| child.style.is_none()));
  assert!(
    measured
      .children
      .iter()
      .flat_map(|child| child.runs.iter())
      .all(|run| run.style.is_none())
  );
}

#[test]
fn reports_resolved_color_and_font_metrics() {
  let measured = measure_with_styles(
    r#"<div style="width:400px; font-size:26px; line-height:2; letter-spacing:0.1em; color:#14110f; text-align:center; text-transform:uppercase">hello</div>"#,
  );
  let style = measured.style.expect("root style");

  assert_eq!(style.color, "rgb(20, 17, 15)");
  assert_eq!(style.font_size, 26.0);
  assert_eq!(style.line_height, Some(52.0));
  assert!((style.letter_spacing - 2.6).abs() <= 0.01);
  assert_eq!(style.text_align, "center");
  assert_eq!(style.text_transform, "uppercase");
  assert_eq!(style.display, "block");
  assert_eq!(style.position, "static");
  assert_eq!(style.visibility, "visible");
  assert_eq!(style.font_weight, 400.0);
  assert_eq!(style.opacity, 1.0);
}

#[test]
fn resolves_current_color_against_the_inherited_value() {
  let measured = measure_with_styles(
    r#"<div style="color:#b3261e"><div style="width:100px; height:10px; border:2px solid currentColor"></div></div>"#,
  );
  let child = measured.children[0].style.as_ref().expect("child style");

  assert_eq!(child.color, "rgb(179, 38, 30)");
  assert!(
    child
      .border_colors
      .as_ref()
      .expect("border colors")
      .iter()
      .all(|color| color == "rgb(179, 38, 30)")
  );
}

#[test]
fn omits_initial_paint_values() {
  let measured = measure_with_styles(r#"<div style="width:100px; height:10px"></div>"#);
  let style = measured.style.expect("root style");

  assert!(style.background_color.is_none());
  assert!(style.background_image.is_none());
  assert!(style.box_shadow.is_none());
  assert!(style.border_widths.is_none());
  assert!(style.border_colors.is_none());
  assert!(style.border_radius.is_none());
  assert!(style.z_index.is_none());
  assert!(style.padding.is_none());
}

#[test]
fn reports_the_structural_properties_a_writer_needs() {
  let measured = measure_with_styles(
    r#"<div style="display:flex; position:relative"><ul style="list-style-type:square"><li style="padding:4px 8px">one</li></ul></div>"#,
  );
  let root = measured.style.as_ref().expect("root style");

  assert_eq!(root.display, "flex");
  assert_eq!(root.position, "relative");

  let list = measured.children[0].style.as_ref().expect("list style");
  assert_eq!(list.list_style_type, "square");

  let item = measured.children[0].children[0]
    .style
    .as_ref()
    .expect("item style");
  assert_eq!(item.padding.expect("padding"), [4.0, 8.0, 4.0, 8.0]);
}

#[test]
fn omits_an_empty_background_image_list() {
  // The `background` shorthand leaves an empty image list behind, which serializes to the
  // `none` keyword; an absent field is what a consumer has to be able to test for.
  let measured =
    measure_with_styles(r#"<div style="width:100px; height:10px; background:#ffffff"></div>"#);
  let style = measured.style.expect("root style");

  assert_eq!(
    style.background_color.as_deref(),
    Some("rgb(255, 255, 255)")
  );
  assert!(style.background_image.is_none());
}

#[test]
fn reports_used_border_widths_and_radii() {
  let measured = measure_with_styles(
    r#"<div style="width:100px; height:10px; border-top:4px solid red; border-right:2px dashed blue; border-bottom-style:none; border-bottom-width:8px; border-radius:6px 0 0 0"></div>"#,
  );
  let style = measured.style.expect("root style");

  assert_eq!(
    style.border_widths.expect("border widths"),
    [4.0, 2.0, 0.0, 0.0]
  );
  assert_eq!(
    style.border_radius.expect("border radius"),
    [6.0, 0.0, 0.0, 0.0]
  );
}

#[test]
fn serializes_backgrounds_and_shadows_as_css() {
  let measured = measure_with_styles(
    r#"<div style="width:100px; height:10px; background-color:rgba(0,0,0,0.5); background-image:linear-gradient(90deg, #fff, #000); box-shadow:0 2px 4px #000; opacity:0.5; z-index:3"></div>"#,
  );
  let style = measured.style.expect("root style");

  assert_eq!(
    style.background_color.as_deref(),
    Some("rgba(0, 0, 0, 0.501961)")
  );
  assert_eq!(
    style.background_image.as_deref(),
    Some("linear-gradient(90deg, rgb(255, 255, 255), rgb(0, 0, 0))")
  );
  assert_eq!(
    style.box_shadow.as_deref(),
    Some("0px 2px 4px rgb(0, 0, 0)")
  );
  assert_eq!(style.opacity, 0.5);
  assert_eq!(style.z_index, Some(3));
}

#[test]
fn run_style_reports_the_shaped_face() {
  let measured = measure_with_styles(
    r#"<div style="width:400px; font-family:Geist; font-size:20px; color:#14110f">hello <span style="font-weight:700; font-style:italic; color:#b3261e">world</span></div>"#,
  );

  let runs: Vec<_> = measured
    .runs
    .iter()
    .filter_map(|run| run.style.as_ref().map(|style| (run.text.as_str(), style)))
    .collect();
  assert!(runs.len() >= 2, "expected two styled runs, got {runs:?}");

  let (_, first) = runs.first().expect("first run");
  assert_eq!(first.font_family, "Geist");
  assert_eq!(first.font_size, 20.0);
  assert_eq!(first.font_weight, 400.0);
  assert_eq!(first.font_style, "normal");
  assert_eq!(first.color, "rgb(20, 17, 15)");

  let (_, bold) = runs.last().expect("last run");
  assert_eq!(bold.font_weight, 700.0);
  assert_eq!(bold.font_style, "italic");
  assert_eq!(bold.color, "rgb(179, 38, 30)");
}

#[test]
fn style_lengths_follow_the_device_pixel_ratio() {
  let viewport = Viewport::new((2400, 1260)).with_device_pixel_ratio(2.0);
  let measured = measure_with_include_styles(
    r#"<div style="width:100px; font-size:20px; line-height:1.5; border:3px solid red">hello</div>"#,
    viewport,
    true,
  );
  let style = measured.style.expect("root style");

  assert_eq!(style.font_size, 40.0);
  assert_eq!(style.line_height, Some(60.0));
  assert_eq!(style.border_widths.expect("border widths"), [6.0; 4]);
}

