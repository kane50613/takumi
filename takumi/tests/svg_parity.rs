//! Visual-parity gate for the SVG backend: the SVG backend exists to reproduce
//! the raster picture, so every fixture's emitted SVG, rasterized by takumi itself, must
//! match the raster WebP golden within tolerance. This turns silent divergence
//! into a test failure.
//!
//! Vector-versus-raster noise (anti-aliasing, gradient interpolation, image scaling)
//! keeps this from being pixel-exact, so the gate is a share-of-pixels-within-
//! tolerance floor. Fixtures the SVG backend still gets wrong are listed in
//! `KNOWN_DIVERGENT` with the reason; that list is the SVG backend's debt and is
//! meant to shrink. A fixture that climbs back above the floor must leave the list
//! (the test fails if a listed fixture now passes), so the debt can't go stale.

mod test_utils;

use std::{collections::HashMap, convert::identity, fs, path::Path};

use rayon::{iter::Either, prelude::*};
use test_utils::{GENERATED_DIR, decode_rgba, rasterize_svg};

/// Per-channel tolerance counted as "matching".
const TOL: i32 = 16;
/// Minimum share of pixels within tolerance for a non-divergent fixture.
const FLOOR: f32 = 90.0;

/// Fixtures the SVG backend does not yet reproduce, with the divergence cause.
/// Shrink this as the backend improves; entries are `(name, why)`.
const KNOWN_DIVERGENT: &[(&str, &str)] = &[
  (
    "style_background_size_auto_axis_round",
    "raster is the approximate side here: it snaps the 78.75px round tiles to \
     whole pixels, so each tile drifts up to half a pixel from svg's exact \
     pattern and the outlines disagree (82.6%, floor is 90%)",
  ),
  (
    "showcase_halftone",
    "contrast(14) magnifies the gradient sampling differences between the svg \
     and raster into different halftone dots (80.6%, floor is 90%)",
  ),
  (
    "showcase_chrome_text",
    "feSpecularLighting bevel: raster lights the rasterized layer's alpha, \
     svg lights the vector glyph alpha; the rim highlight lands differently \
     (88.2%, floor is 90%)",
  ),
];

/// Straight-alpha composite over white. Transparent pixels become white, so two
/// pixels that look identical on a white page compare equal regardless of the RGB
/// left under a zero alpha.
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

/// Share of pixels within tolerance, or `None` if the fixture can't be compared.
fn parity(svg_path: &Path, webp_path: &Path) -> Option<f32> {
  let svg = fs::read_to_string(svg_path).ok()?;
  let (width, height, webp) = decode_rgba(&fs::read(webp_path).ok()?);

  if width == 0 || height == 0 {
    return None;
  }

  let bitmap = rasterize_svg(svg, width, height);
  let pixels = bitmap.as_raw().as_chunks::<4>().0;
  let matching = pixels
    .iter()
    .zip(webp.as_chunks::<4>().0)
    .filter(|&(&svg, &raster)| within(svg, raster))
    .count();

  Some(matching as f32 / pixels.len() as f32 * 100.0)
}

#[test]
fn svg_matches_raster_within_tolerance() {
  let known: HashMap<&str, &str> = KNOWN_DIVERGENT.iter().copied().collect();
  let dir = Path::new(GENERATED_DIR);

  let svgs: Vec<_> = fs::read_dir(dir)
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
        (true, false) => Some(Either::Left((name, pct))),
        (false, true) => Some(Either::Right(name)),
        _ => None,
      }
    })
    .partition_map(identity);

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
