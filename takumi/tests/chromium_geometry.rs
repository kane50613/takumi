//! Compares where takumi draws each character of the HTML fixtures with where Chromium draws it,
//! as `bun chromium-geometry` recorded in `tests/fixtures-chromium/`.
//!
//! A character matches when Chromium draws the same character within `X_TOLERANCE` of its left
//! edge and `Y_TOLERANCE` of its top. `tests/chromium-geometry.json` holds how many characters
//! each fixture leaves unmatched on each side, and the test fails when a count grows.
//! `CHROMIUM_GEOMETRY_UPDATE=1` rewrites it with the current counts, and
//! `CHROMIUM_GEOMETRY_VERBOSE=1` prints each character takumi places differently. `FIXTURE` selects
//! fixtures as it does for `html_fixtures`.

mod test_utils;

use std::{collections::BTreeMap, env, fs, path::Path};

use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use serde::{Deserialize, Serialize};
use takumi::prelude::Affine;
use takumi_core::paint_tree::{NodeKind, PaintTreeOptions, TextRun, paint_tree};
use test_utils::{CONTEXT, HtmlFixture, TEST_IMAGES, html_fixture_paths};

/// Chromium places text in `LayoutUnit`s of 1/64px, and takumi's shaping rounds differently.
const X_TOLERANCE: f32 = 0.5;
/// Less than a line apart, so a character on the wrong line never matches.
const Y_TOLERANCE: f32 = 4.0;

const EXPECTATIONS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/chromium-geometry.json");

#[derive(Deserialize)]
struct Reference {
  characters: Vec<(String, f32, f32)>,
}

/// A character's left edge and top on the canvas.
struct Placed {
  character: char,
  x: f32,
  top: f32,
}

impl Placed {
  fn distance(&self, other: &Self) -> f32 {
    (self.x - other.x).abs() + (self.top - other.top).abs()
  }
}

/// The characters one side draws that the other does not.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
struct Unmatched {
  takumi: usize,
  chromium: usize,
}

#[test]
fn chromium_geometry() {
  let only = env::var("FIXTURE").ok();
  let verbose = env::var_os("CHROMIUM_GEOMETRY_VERBOSE").is_some();
  let references = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures-chromium");
  let measured: BTreeMap<String, Unmatched> = html_fixture_paths(only.as_deref())
    .par_iter()
    .filter_map(|path| {
      let name = path.file_stem()?.to_string_lossy().into_owned();
      let reference = fs::read_to_string(references.join(format!("{name}.json"))).ok()?;
      let reference: Reference = serde_json::from_str(&reference).unwrap();
      let takumi = takumi_characters(&HtmlFixture::read(path).unwrap());

      Some((
        name.clone(),
        Unmatched::of(&name, &takumi, &reference, verbose),
      ))
    })
    .collect();
  let mut expected: BTreeMap<String, Unmatched> = fs::read_to_string(EXPECTATIONS)
    .ok()
    .map(|text| serde_json::from_str(&text).unwrap())
    .unwrap_or_default();

  if env::var_os("CHROMIUM_GEOMETRY_UPDATE").is_some() {
    expected.extend(measured);
    expected.retain(|_, unmatched| unmatched.takumi + unmatched.chromium > 0);
    fs::write(
      EXPECTATIONS,
      format!("{}\n", serde_json::to_string_pretty(&expected).unwrap()),
    )
    .unwrap();

    return;
  }

  let regressed: Vec<String> = measured
    .iter()
    .filter_map(|(name, unmatched)| {
      let allowed = expected.get(name).copied().unwrap_or_default();

      (unmatched.takumi > allowed.takumi || unmatched.chromium > allowed.chromium)
        .then(|| format!("{name}: {unmatched:?}, expected at most {allowed:?}"))
    })
    .collect();

  assert!(regressed.is_empty(), "{regressed:#?}");
}

impl Unmatched {
  /// Pairs each of `takumi`'s characters with the nearest unpaired one Chromium draws within the
  /// tolerances.
  fn of(name: &str, takumi: &[Placed], reference: &Reference, verbose: bool) -> Self {
    let mut chromium: Vec<Option<Placed>> = reference
      .characters
      .iter()
      .filter_map(|(text, x, top)| {
        Some(Some(Placed {
          character: text.chars().next()?,
          x: *x,
          top: *top,
        }))
      })
      .collect();
    let mut unmatched_takumi = 0;

    for placed in takumi {
      let nearest = chromium
        .iter()
        .enumerate()
        .filter_map(|(index, candidate)| {
          let candidate = candidate.as_ref()?;
          let dx = (candidate.x - placed.x).abs();

          (candidate.character == placed.character
            && dx <= X_TOLERANCE
            && (candidate.top - placed.top).abs() <= Y_TOLERANCE)
            .then_some((index, dx))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1));

      match nearest {
        Some((index, _)) => chromium[index] = None,
        None => {
          unmatched_takumi += 1;

          if verbose {
            let closest = chromium
              .iter()
              .flatten()
              .filter(|candidate| candidate.character == placed.character)
              .min_by(|a, b| placed.distance(a).total_cmp(&placed.distance(b)));

            eprintln!(
              "{name}: {:?} at ({:.2}, {:.2}), Chromium's nearest at {:?}",
              placed.character,
              placed.x,
              placed.top,
              closest.map(|candidate| (candidate.x, candidate.top)),
            );
          }
        }
      }
    }

    if verbose {
      for placed in chromium.iter().flatten() {
        eprintln!(
          "{name}: {:?} at ({:.2}, {:.2}) only in Chromium",
          placed.character, placed.x, placed.top,
        );
      }
    }

    Self {
      takumi: unmatched_takumi,
      chromium: chromium.iter().flatten().count(),
    }
  }
}

/// Where takumi draws the first character of each glyph cluster, leaving out white space.
fn takumi_characters(fixture: &HtmlFixture) -> Vec<Placed> {
  let tree = paint_tree(
    PaintTreeOptions::builder()
      .viewport(fixture.viewport)
      .fonts(&CONTEXT)
      .node(fixture.node.clone())
      .images(TEST_IMAGES.clone())
      .stylesheet(fixture.stylesheet.clone().into())
      .build(),
  )
  .unwrap();

  tree
    .document
    .nodes
    .iter()
    .flat_map(|node| {
      let runs: &[TextRun] = match &node.kind {
        NodeKind::Text { runs, .. } => runs,
        _ => &[],
      };

      runs
        .iter()
        .flat_map(move |run| run_characters(run, node.transform))
    })
    .collect()
}

fn run_characters(run: &TextRun, node_transform: [f32; 6]) -> Vec<Placed> {
  let utf16: Vec<u16> = run.text.encode_utf16().collect();
  let transform = Affine::from_cols_array(node_transform)
    * run
      .transform
      .map_or(Affine::IDENTITY, Affine::from_cols_array);
  let mut seen = Vec::new();

  run
    .glyphs
    .iter()
    .filter_map(|glyph| {
      let cluster = glyph.cluster? as usize;

      if seen.contains(&cluster) {
        return None;
      }

      seen.push(cluster);

      let character = char::decode_utf16(utf16.get(cluster..)?.iter().copied())
        .next()?
        .ok()?;

      if character.is_whitespace() {
        return None;
      }

      let (x, top) = transform.transform_point(run.x + glyph.x, run.y - run.ascent);

      Some(Placed { character, x, top })
    })
    .collect()
}
