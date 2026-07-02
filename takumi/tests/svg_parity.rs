//! Visual-parity gate for the SVG backend: the SVG backend exists to reproduce
//! the raster picture, so every fixture's emitted SVG, rasterized with resvg, must
//! match the raster WebP golden within tolerance. This turns silent divergence
//! into a test failure.
//!
//! Cross-rasterizer noise (anti-aliasing, gradient interpolation, image scaling)
//! keeps this from being pixel-exact, so the gate is a share-of-pixels-within-
//! tolerance floor. Fixtures the SVG backend still gets wrong are listed in
//! `KNOWN_DIVERGENT` with the reason; that list is the SVG backend's debt and is
//! meant to shrink. A fixture that climbs back above the floor must leave the list
//! (the test fails if a listed fixture now passes), so the debt can't go stale.

use std::{collections::HashMap, fs, path::Path};

use image::ImageReader;
use rayon::prelude::*;
use resvg::{
  tiny_skia::Pixmap,
  usvg::{Options, Transform, Tree},
};

/// Per-channel tolerance counted as "matching".
const TOL: i32 = 16;
/// Minimum share of pixels within tolerance for a non-divergent fixture.
const FLOOR: f32 = 90.0;

/// Fixtures the SVG backend does not yet reproduce, with the divergence cause.
/// Shrink this as the backend improves; entries are `(name, why)`.
const KNOWN_DIVERGENT: &[(&str, &str)] = &[(
  "style_backdrop_filter",
  "backdrop-filter: opacity() semantics differ: raster replaces the backdrop \
   pixels, svg (paint-over model, no erase) composites the filtered copy over \
   the original",
)];

/// Straight-alpha composite over white. Transparent pixels become white, so two
/// pixels that look identical on a white page compare equal regardless of the RGB
/// left under a zero alpha (webp keeps dirty RGB there, resvg zeroes it).
fn over_white(p: [u8; 4]) -> [u8; 3] {
  let a = p[3] as f32 / 255.0;
  [0, 1, 2].map(|i| (p[i] as f32 * a + 255.0 * (1.0 - a)).round() as u8)
}

fn within(a: [u8; 4], b: [u8; 4]) -> bool {
  over_white(a)
    .iter()
    .zip(over_white(b).iter())
    .all(|(x, y)| (*x as i32 - *y as i32).abs() <= TOL)
}

/// Share of pixels within tolerance, or `None` if the fixture can't be compared
/// (unparseable svg, zero size, dimension mismatch).
fn parity(svg_path: &Path, webp_path: &Path) -> Option<f32> {
  let svg = fs::read_to_string(svg_path).ok()?;
  let tree = Tree::from_str(&svg, &Options::default()).ok()?;
  let size = tree.size();
  let (w, h) = (size.width().round() as u32, size.height().round() as u32);
  if w == 0 || h == 0 {
    return None;
  }
  let mut svg_px = Pixmap::new(w, h)?;
  resvg::render(&tree, Transform::identity(), &mut svg_px.as_mut());

  let webp = ImageReader::open(webp_path).ok()?.decode().ok()?.to_rgba8();
  if webp.width() != w || webp.height() != h {
    return None;
  }

  let mut total = 0u32;
  let mut ok = 0u32;
  for y in 0..h {
    for x in 0..w {
      let s = svg_px.pixel(x, y)?.demultiply();
      total += 1;
      if within(
        [s.red(), s.green(), s.blue(), s.alpha()],
        webp.get_pixel(x, y).0,
      ) {
        ok += 1;
      }
    }
  }
  Some(ok as f32 / total as f32 * 100.0)
}

#[test]
fn svg_matches_raster_within_tolerance() {
  let known: HashMap<&str, &str> = KNOWN_DIVERGENT.iter().copied().collect();
  let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures-generated");

  let svgs: Vec<_> = fs::read_dir(&dir)
    .unwrap()
    .filter_map(|e| {
      let path = e.ok()?.path();
      (path.extension().and_then(|x| x.to_str()) == Some("svg")).then_some(path)
    })
    .collect();

  let (mut below_floor, recovered): (Vec<(String, f32)>, Vec<String>) = svgs
    .par_iter()
    .filter_map(|svg_path| {
      let name = svg_path.file_stem()?.to_str()?.to_string();
      let pct = parity(svg_path, &dir.join(format!("{name}.webp")))?;
      let listed = known.contains_key(name.as_str());
      match (pct < FLOOR, listed) {
        (true, false) => Some((Some((name, pct)), None)),
        (false, true) => Some((None, Some(name))),
        _ => None,
      }
    })
    .fold(
      || (Vec::new(), Vec::new()),
      |(mut bf, mut rec), (b, r)| {
        bf.extend(b);
        rec.extend(r);
        (bf, rec)
      },
    )
    .reduce(
      || (Vec::new(), Vec::new()),
      |(mut bf, mut rec), (b, r)| {
        bf.extend(b);
        rec.extend(r);
        (bf, rec)
      },
    );

  below_floor.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
  let mut msg = String::new();
  if !below_floor.is_empty() {
    msg.push_str(&format!(
      "\n{} fixture(s) below the {FLOOR}% parity floor (svg diverges from raster):\n",
      below_floor.len()
    ));
    for (name, pct) in &below_floor {
      msg.push_str(&format!("  {pct:6.2}%  {name}\n"));
    }
    msg.push_str("Fix the svg backend, or add to KNOWN_DIVERGENT with a reason.\n");
  }
  if !recovered.is_empty() {
    msg.push_str(&format!(
      "\n{} fixture(s) now meet the floor and must leave KNOWN_DIVERGENT:\n  {}\n",
      recovered.len(),
      recovered.join(", ")
    ));
  }
  assert!(msg.is_empty(), "{msg}");
}
