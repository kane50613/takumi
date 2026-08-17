//! Full-canvas showcase renders for the visual effects docs page. Each test
//! produces one 1200x630 hero image; the CSS doubles as the documented recipe.

use takumi::prelude::*;

use crate::{
  style_filter_reference::filter_url,
  test_utils::{CONTEXT, TEST_IMAGES, create_test_viewport, run_fixture_test_with_css},
};

fn run_showcase(node: Node, css: &str, fixture_name: &str) {
  let stylesheet = format!(
    r#"
    .stage {{
      position: relative;
      display: flex;
      width: 100%;
      height: 100%;
      align-items: center;
      justify-content: center;
      font-family: Geist;
    }}
    .layer {{ position: absolute; inset: 0; }}
    {css}
  "#
  );

  let options = RenderOptions::builder()
    .viewport(create_test_viewport())
    .node(node)
    .fonts(&CONTEXT)
    .images(TEST_IMAGES.clone())
    .stylesheet(StyleSheet::parse(&stylesheet).unwrap().into())
    .build();

  run_fixture_test_with_css(options, &stylesheet, fixture_name);
}

fn stage(children: Vec<Node>) -> Node {
  Node::container(children).with_class_name("stage")
}

fn layer(extra_class: &str) -> Node {
  Node::container(vec![]).with_class_name(format!("layer {extra_class}"))
}

/// Grain overlay: `feTurbulence` fills the element with fractal noise, the
/// color matrix knocks it down to faint white specks, and `overlay` blends it
/// into whatever sits below.
fn grain_filter() -> String {
  filter_url(
    r#"<filter x="0" y="0" width="100%" height="100%"><feTurbulence type="fractalNoise" baseFrequency="0.8" numOctaves="2" stitchTiles="stitch"/><feColorMatrix type="matrix" values="0 0 0 0 0.9 0 0 0 0 0.9 0 0 0 0 0.9 0 0 0 0.22 0"/></filter>"#,
  )
}

// Aurora mesh: stacked soft radial ellipses all fading out around 68%, a
// blurred conic veil, film grain, and an oklch longer-hue gradient title.
#[test]
fn test_showcase_aurora_grain() {
  let css = format!(
    r#"
    .stage {{ background-color: #05010f; flex-direction: column; gap: 18px; }}
    .mesh {{
      background-image:
        radial-gradient(ellipse 80% 60% at 70% 20%, rgba(139, 92, 246, 0.55), transparent 68%),
        radial-gradient(ellipse 70% 60% at 20% 80%, rgba(236, 72, 153, 0.45), transparent 68%),
        radial-gradient(ellipse 60% 50% at 60% 65%, rgba(45, 212, 191, 0.4), transparent 68%),
        radial-gradient(ellipse 65% 40% at 30% 30%, rgba(99, 102, 241, 0.5), transparent 68%);
    }}
    .veil {{
      inset: 10%;
      background-image: conic-gradient(from 120deg at 50% 30%, #3a29ff, #2dd4bf, #ec4899, #8b5cf6, #3a29ff);
      filter: blur(80px) saturate(1.4);
      opacity: 0.4;
    }}
    .grain {{ filter: {grain}; mix-blend-mode: overlay; }}
    .title {{
      font-size: 108px;
      font-weight: 800;
      letter-spacing: -3px;
      background-image: linear-gradient(to right in oklch longer hue, #ff5f6d, #ffc371);
      background-clip: text;
      color: transparent;
    }}
    .subtitle {{ font-family: "Geist Mono"; font-size: 26px; color: rgba(255, 255, 255, 0.55); }}
  "#,
    grain = grain_filter()
  );

  let root = stage(vec![
    layer("mesh"),
    layer("veil"),
    layer("grain"),
    Node::text("Aurora".to_string()).with_class_name("title"),
    Node::text("mesh gradients + film grain".to_string()).with_class_name("subtitle"),
  ]);

  run_showcase(root, &css, "showcase_aurora_grain");
}

// CRT terminal: neon text-shadow stacks, chromatic aberration, scanlines, and
// a radial vignette.
#[test]
fn test_showcase_neon_terminal() {
  let css = r#"
    .stage { background-color: #050508; flex-direction: column; gap: 20px; }
    .neon {
      font-size: 96px;
      font-weight: 800;
      color: #fff;
      text-shadow: 0 0 6px #fff, 0 0 18px #ff2d95, 0 0 42px #ff2d95, 0 0 90px #ff2d95;
    }
    .chromatic {
      font-family: "Geist Mono";
      font-size: 30px;
      color: rgba(210, 255, 244, 0.9);
      text-shadow: -2px 0 rgba(255, 0, 85, 0.7), 2px 0 rgba(0, 255, 255, 0.7);
    }
    .scanlines {
      background-image: repeating-linear-gradient(180deg, rgba(0, 0, 0, 0) 0px, rgba(0, 0, 0, 0) 3px, rgba(0, 0, 0, 0.3) 3px, rgba(0, 0, 0, 0.3) 5px);
    }
    .vignette {
      background-image: radial-gradient(ellipse at 50% 50%, transparent 55%, rgba(0, 0, 0, 0.6) 100%);
    }
  "#;

  let root = stage(vec![
    Node::text("NEON".to_string()).with_class_name("neon"),
    Node::text("> scanlines & chromatic aberration_".to_string()).with_class_name("chromatic"),
    layer("scanlines"),
    layer("vignette"),
  ]);

  run_showcase(root, css, "showcase_neon_terminal");
}

// Y2K chrome: a hard-stop metal band gradient clipped to the glyphs, a
// specular-lighting bevel from the filter graph, over a starburst backdrop.
#[test]
fn test_showcase_chrome_text() {
  let bevel = filter_url(
    r##"<filter x="-20%" y="-20%" width="140%" height="140%"><feGaussianBlur stdDeviation="1.4" in="SourceGraphic" result="blur"/><feSpecularLighting surfaceScale="1.6" specularConstant="0.9" specularExponent="40" lighting-color="#ffffff" in="blur" result="spec"><fePointLight x="600" y="-60" z="900"/></feSpecularLighting><feComposite operator="in" in="spec" in2="SourceAlpha" result="rim"/><feMerge><feMergeNode in="SourceGraphic"/><feMergeNode in="rim"/></feMerge></filter>"##,
  );
  let css = format!(
    r#"
    .stage {{
      background-color: #0b0617;
      background-image:
        repeating-conic-gradient(from 0deg at 50% 130%, rgba(94, 80, 163, 0.35) 0deg 5deg, rgba(11, 6, 23, 0) 5deg 10deg),
        radial-gradient(ellipse 80% 60% at 50% 0%, rgba(120, 180, 255, 0.25), transparent 70%);
      flex-direction: column;
      gap: 8px;
    }}
    .chrome {{
      font-size: 172px;
      font-weight: 900;
      letter-spacing: -6px;
      background-image: linear-gradient(180deg, #0b1020 0%, #34304b 14%, #c3b9d5 26%, #ffffff 34%, #5a6cae 48%, #ffffff 56%, #e8ecff 84%, #c3b9d5 90%, #211c35 100%);
      background-clip: text;
      color: transparent;
      filter: {bevel};
    }}
    .tagline {{
      font-family: "Geist Mono";
      font-size: 26px;
      letter-spacing: 10px;
      color: rgba(195, 185, 213, 0.8);
    }}
  "#
  );

  let root = stage(vec![
    Node::text("CHROME!".to_string()).with_class_name("chrome"),
    Node::text("Y2K SPECULAR BEVEL".to_string()).with_class_name("tagline"),
  ]);

  run_showcase(root, &css, "showcase_chrome_text");
}

// Halftone: a dot screen from a tiled radial gradient multiplied with a tone
// ramp and crushed by contrast(); `lighten` re-inks the black dots.
#[test]
fn test_showcase_halftone() {
  let css = r#"
    .stage { background-color: #fff; isolation: isolate; }
    .dots {
      background-image: radial-gradient(circle closest-side, #666, #fff), linear-gradient(115deg, #333, #fff);
      background-size: 14px 14px, 100% 100%;
      background-repeat: space, no-repeat;
      background-blend-mode: multiply;
      filter: contrast(14);
    }
    .ink { background-color: #e11d48; mix-blend-mode: lighten; }
    .poster {
      padding: 18px 54px;
      background-color: #fffdf5;
      border: 6px solid #111;
      border-radius: 18px;
      box-shadow: 14px 14px 0 #111;
      font-size: 132px;
      font-weight: 900;
      letter-spacing: -4px;
      color: #111;
    }
  "#;

  let root = stage(vec![
    layer("dots"),
    layer("ink"),
    Node::text("HALFTONE".to_string()).with_class_name("poster"),
  ]);

  run_showcase(root, css, "showcase_halftone");
}

// Landing-page hero: blueprint grid faded by a mask, corner glows, and a
// neo-brutalist card with a hard offset shadow.
#[test]
fn test_showcase_grid_hero() {
  let css = r#"
    .stage { background-color: #030409; }
    .grid {
      background-image:
        linear-gradient(rgba(255, 255, 255, 0.09) 1px, transparent 1px),
        linear-gradient(90deg, rgba(255, 255, 255, 0.09) 1px, transparent 1px);
      background-size: 40px 40px;
      mask-image: radial-gradient(ellipse 70% 60% at 50% 100%, #000 60%, transparent 100%);
    }
    .glow {
      background-image:
        radial-gradient(ellipse 100% 100% at 100% 100%, rgba(34, 197, 94, 0.18), transparent 60%),
        radial-gradient(ellipse 100% 100% at 0% 0%, rgba(56, 189, 248, 0.14), transparent 60%);
    }
    .card {
      display: flex;
      flex-direction: column;
      gap: 10px;
      padding: 44px 56px;
      background-color: #fdfdf7;
      border: 4px solid #111;
      box-shadow: 18px 18px 0 #22c55e;
    }
    .kicker {
      font-family: "Geist Mono";
      font-size: 22px;
      letter-spacing: 8px;
      color: #16a34a;
    }
    .headline { font-size: 84px; font-weight: 900; letter-spacing: -2px; color: #111; }
  "#;

  let root = stage(vec![
    layer("grid"),
    layer("glow"),
    Node::container(vec![
      Node::text("TAKUMI / SHOWCASE".to_string()).with_class_name("kicker"),
      Node::text("Grid Hero".to_string()).with_class_name("headline"),
    ])
    .with_class_name("card"),
  ]);

  run_showcase(root, css, "showcase_grid_hero");
}
