//! Runs `takumi paint` and reads what it says of a document.

use std::{
  env, fs,
  path::Path,
  process::{self, Command},
};

const PAGE: &str = r#"<style>
  body { margin: 0 }
  #card { width: 300px; height: 100px; background: #0f172a; color: white; overflow: hidden }
  .ghost { position: absolute; left: 900px; top: 0; width: 20px; height: 20px; background: red }
</style>
<div id="card"><h1 class="title big">Hello</h1><div class="ghost"></div><p style="height: 0"></p><section style="opacity: 0; position: absolute; top: 0">faded <b>deep</b></section><p class="unseen" style="visibility: hidden; position: absolute; top: 0; margin: 0">unseen</p><span>shown<em style="opacity: 0">inline</em><i style="visibility: hidden">hidden</i></span></div>"#;

fn paint(name: &str, arguments: &[&str]) -> String {
  paint_html(name, PAGE, arguments)
}

fn paint_html(name: &str, html: &str, arguments: &[&str]) -> String {
  let dir = env::temp_dir().join(format!("takumi-cli-paint-{name}-{}", process::id()));

  fs::create_dir_all(&dir).unwrap();
  fs::write(dir.join("page.html"), html).unwrap();

  let output = Command::new(env!("CARGO_BIN_EXE_takumi"))
    .arg("paint")
    .arg(dir.join("page.html"))
    .args(["-o", "-", "--width", "300", "--height", "100"])
    .args(arguments)
    .output()
    .expect("runs takumi");

  assert!(
    output.status.success(),
    "{}",
    String::from_utf8_lossy(&output.stderr)
  );
  String::from_utf8(output.stdout).unwrap()
}

#[test]
fn the_outline_names_each_node_where_it_lands_and_what_fills_it() {
  let outline = paint("nodes", &[]);
  let line = |needle: &str| {
    outline
      .lines()
      .find(|line| line.contains(needle))
      .unwrap_or_else(|| panic!("nothing holds {needle:?} in:\n{outline}"))
  };

  assert_eq!(outline.lines().next(), Some("canvas 300x100"));
  assert!(line("div#card").contains("0,0 300x100 background #0f172a clips-overflow"));
  assert!(line("text h1.title.big").contains("\"Hello\""));
  assert!(line("text h1.title.big").contains("#ffffff"));
  assert!(line("div.ghost").ends_with("background #ff0000 offscreen past the right edge"));
  assert!(line("text p.unseen").ends_with("\"unseen\" Geist 16px draws nothing"));
  assert!(
    outline.ends_with("not painted\n  section opacity: 0\n  em opacity: 0\n"),
    "{outline}"
  );
  assert!(!outline.contains("deep"));
  assert!(line("\"shown").contains("\"shown\""), "{outline}");
  // A child sits one level under its parent.
  assert!(line("box h1.title.big").starts_with("      box "));
  assert!(!outline.contains("- fill"));
  assert!(!outline.contains("paint order"));
}

#[test]
fn each_detail_adds_to_the_one_before() {
  let drawables = paint("drawables", &["--detail", "drawables"]);
  let full = paint("full", &["--detail", "full"]);

  assert!(drawables.contains("- fill background rect 0,0 300x100 #0f172a"));
  assert!(drawables.contains("- glyphs text #ffffff \"Hello\""));
  assert!(!drawables.contains("paint order"));
  assert!(full.starts_with(&drawables));
  assert!(full.contains("paint order\n  0 draw box div#card\n  1 begin clip box div#card"));
}

#[test]
fn json_leaves_out_what_the_detail_does() {
  let nodes = paint("json-nodes", &["--format", "json"]);
  let full = paint("json-full", &["--format", "json", "--detail", "full"]);
  let parsed: serde_json::Value = serde_json::from_str(&nodes).unwrap();

  assert_eq!(parsed["width"], 300.0);
  assert!(
    parsed["nodes"]
      .as_array()
      .is_some_and(|nodes| !nodes.is_empty())
  );
  assert!(!nodes.contains("\"drawables\""));
  assert!(!nodes.contains("\"glyphs\""));
  assert!(!nodes.contains("\"steps\""));
  assert!(full.contains("\"drawables\""));
  assert!(full.contains("\"glyphs\""));
  assert!(full.contains("\"steps\""));
}

#[test]
fn json_nodes_hold_no_mask_drawables() {
  let masked = r#"<div style="width: 50px; height: 50px; background: red; mask-image: linear-gradient(black, transparent)"></div>"#;
  let nodes = paint_html("json-mask", masked, &["--format", "json"]);
  let drawables = paint_html(
    "json-mask-drawables",
    masked,
    &["--format", "json", "--detail", "drawables"],
  );

  assert!(drawables.contains("\"mask\""), "{drawables}");
  assert!(!nodes.contains("\"mask\""), "{nodes}");
}

/// Writes the outline and the JSON of the page to `tests/fixtures-generated/`, which pin the
/// output.
#[test]
fn the_outline_and_its_json_write_their_goldens() {
  let goldens = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures-generated");

  fs::write(
    goldens.join("paint-outline.txt"),
    paint("golden-text", &["--detail", "full"]),
  )
  .unwrap();
  fs::write(
    goldens.join("paint-nodes.json"),
    paint("golden-json", &["--format", "json"]),
  )
  .unwrap();
}

#[test]
fn drawables_open_groups_and_name_each_draw_part() {
  let html = r#"<div style="outline: 2px solid red; width: 100px; height: 20px">x</div><p><span style="visibility: hidden; font-size: 40px">big</span><span style="font-size: 12px">small</span></p><p>a <span style="opacity: 0.5">half</span></p>"#;
  let full = paint_html("parts", html, &["--detail", "full"]);
  let line = |needle: &str| {
    full
      .lines()
      .find(|line| line.contains(needle))
      .unwrap_or_else(|| panic!("nothing holds {needle:?} in:\n{full}"))
  };

  assert!(line("\"small\" Geist").contains("Geist 12px"), "{full}");
  assert!(
    line("- group opacity 0.5").starts_with("        - group"),
    "{full}"
  );
  assert!(
    line("\"half\" at").starts_with("          - glyphs"),
    "{full}"
  );
  assert!(line(" draw outline ").ends_with("box div"), "{full}");
}
