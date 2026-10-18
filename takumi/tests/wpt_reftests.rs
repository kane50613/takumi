//! Runs the web-platform-tests CSS reftests: renders each test and its reference, and compares the
//! pixels as the WPT harness does.
//! <https://web-platform-tests.org/writing-tests/reftests.html>
//!
//! The tests come from the WPT checkout `scripts/wpt-checkout.sh` makes in `target/wpt`, or the one
//! `WPT_ROOT` points at; without either the test does nothing.
//! `tests/wpt-reftests.json` lists the tests that pass, and the test fails when one of them stops
//! passing. `WPT_UPDATE=1` rewrites it with the tests that pass now, `WPT_VERBOSE=1` prints each
//! test that does not pass, `WPT_DUMP=dir` writes each failing test's rendering and its
//! reference's there, and `WPT=fragment,...` runs only the tests whose paths contain one of the
//! fragments.

mod test_utils;

use std::{
  collections::{BTreeMap, BTreeSet, HashMap},
  env, fs,
  panic::{self, AssertUnwindSafe},
  path::{Path, PathBuf},
  sync::Arc,
};

use image::RgbaImage;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use takumi::{
  prelude::{
    FontOverride, FontResource, Fonts, FromHtml, FromHtmlOptions, ImageCacheMode, ImageSource,
    Node, RenderOptions, StyleSheet, Viewport,
  },
  render,
};
use takumi_core::resources::image::ResourceCache;
use test_utils::{CONTEXT, matches_fragments};

/// The WPT harness's viewport.
const VIEWPORT: (u32, u32) = (800, 600);

const EXPECTATIONS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/wpt-reftests.json");

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Outcome {
  Pass,
  Fail,
  /// The test or its reference could not be read or rendered.
  Error,
  /// Rendering panicked.
  Crash,
}

#[test]
fn wpt_reftests() {
  let root = env::var_os("WPT_ROOT").map_or_else(
    || Path::new(env!("CARGO_MANIFEST_DIR")).join("../target/wpt"),
    PathBuf::from,
  );

  if !root.join("css").is_dir() {
    eprintln!(
      "no WPT checkout at {}; run scripts/wpt-checkout.sh",
      root.display()
    );

    return;
  }

  let only = env::var("WPT").ok();
  let runner = Runner::new(&root);
  let tests: Vec<PathBuf> = reftests(&root.join("css"))
    .into_iter()
    .filter(|path| matches_fragments(only.as_deref(), &path.to_string_lossy()))
    .collect();
  let outcomes: BTreeMap<String, Outcome> = tests
    .par_iter()
    .map(|path| (runner.relative(path), runner.run(path)))
    .collect();
  let mut counts = BTreeMap::new();

  for outcome in outcomes.values() {
    *counts.entry(*outcome).or_insert(0) += 1;
  }

  eprintln!("WPT reftests: {counts:?}");

  let mut expected: BTreeSet<String> = fs::read_to_string(EXPECTATIONS)
    .ok()
    .map(|text| serde_json::from_str(&text).unwrap())
    .unwrap_or_default();

  if env::var_os("WPT_UPDATE").is_some() {
    expected.retain(|name| !outcomes.contains_key(name));
    expected.extend(
      outcomes
        .iter()
        .filter(|(_, outcome)| **outcome == Outcome::Pass)
        .map(|(name, _)| name.clone()),
    );
    fs::write(
      EXPECTATIONS,
      format!("{}\n", serde_json::to_string_pretty(&expected).unwrap()),
    )
    .unwrap();

    return;
  }

  let regressed: Vec<String> = expected
    .iter()
    .filter_map(|name| {
      let outcome = outcomes.get(name)?;

      (*outcome != Outcome::Pass).then(|| format!("{name}: {outcome:?}"))
    })
    .collect();

  assert!(regressed.is_empty(), "{regressed:#?}");
}

/// Every reftest under `dir`: an HTML or XHTML file that links a reference, and is not one.
fn reftests(dir: &Path) -> Vec<PathBuf> {
  let mut found = Vec::new();
  let mut pending = vec![dir.to_path_buf()];

  while let Some(dir) = pending.pop() {
    let Ok(entries) = fs::read_dir(&dir) else {
      continue;
    };

    for entry in entries.flatten() {
      let path = entry.path();
      let name = entry.file_name().to_string_lossy().into_owned();

      if path.is_dir() {
        if !matches!(
          name.as_str(),
          "reference" | "support" | "resources" | "tools"
        ) {
          pending.push(path);
        }

        continue;
      }

      let is_document = ["html", "htm", "xht", "xhtml"]
        .iter()
        .any(|extension| path.extension().is_some_and(|ext| ext == *extension));
      let stem = path.file_stem().unwrap_or_default().to_string_lossy();
      let is_reference =
        stem.ends_with("-ref") || stem.ends_with("-notref") || stem.contains("-ref-");

      if is_document
        && !is_reference
        && fs::read_to_string(&path).is_ok_and(|source| !references(&source).is_empty())
      {
        found.push(path);
      }
    }
  }

  found.sort();

  found
}

/// The references `source` links, each with whether the test must match it.
fn references(source: &str) -> Vec<(String, bool)> {
  tags(source, "link")
    .into_iter()
    .filter_map(|attributes| {
      let matches = match attributes.get("rel")?.as_str() {
        "match" => true,
        "mismatch" => false,
        _ => return None,
      };

      Some((attributes.get("href")?.clone(), matches))
    })
    .collect()
}

/// The attributes of each `<name ...>` tag in `source`, by lowercase name.
fn tags(source: &str, name: &str) -> Vec<HashMap<String, String>> {
  let lower = source.to_ascii_lowercase();
  let opening = format!("<{name}");
  let mut found = Vec::new();
  let mut from = 0;

  while let Some(start) = lower[from..].find(&opening).map(|index| from + index) {
    let after = start + opening.len();
    let Some(end) = source[after..].find('>').map(|index| after + index) else {
      break;
    };

    from = end;

    if !source[after..]
      .chars()
      .next()
      .is_some_and(|next| next.is_whitespace() || next == '/' || next == '>')
    {
      continue;
    }

    found.push(attributes(&source[after..end]));
  }

  found
}

/// The attributes of a tag's inside, `rel="match" href=a.html`.
fn attributes(inside: &str) -> HashMap<String, String> {
  let mut found = HashMap::new();
  let mut rest = inside.trim_start_matches('/').trim();

  while !rest.is_empty() {
    let name_end = rest
      .find(|c: char| c == '=' || c.is_whitespace() || c == '/')
      .unwrap_or(rest.len());
    let name = rest[..name_end].to_ascii_lowercase();

    rest = rest[name_end..].trim_start();

    let value = if let Some(after) = rest.strip_prefix('=') {
      let after = after.trim_start();
      let (value, remainder) = match after.chars().next() {
        Some(quote @ ('"' | '\'')) => {
          let inner = &after[1..];
          let close = inner.find(quote).unwrap_or(inner.len());

          (&inner[..close], inner.get(close + 1..).unwrap_or(""))
        }
        _ => {
          let end = after.find(char::is_whitespace).unwrap_or(after.len());

          (&after[..end], &after[end..])
        }
      };

      rest = remainder;

      value.to_owned()
    } else {
      String::new()
    };

    if !name.is_empty() {
      found.insert(name, value);
    }

    rest = rest.trim_start_matches('/').trim_start();
  }

  found
}

/// How far a rendering may stray from its reference: the largest difference in any channel, and
/// how many pixels differ, each as an inclusive range.
/// <https://web-platform-tests.org/writing-tests/reftests.html#fuzzy-matching>
#[derive(Debug, Clone, Copy, PartialEq)]
struct Fuzzy {
  max_difference: (u8, u8),
  total_pixels: (u32, u32),
}

impl Fuzzy {
  const EXACT: Self = Self {
    max_difference: (0, 0),
    total_pixels: (0, 0),
  };

  /// The allowance `source`'s `<meta name=fuzzy>` gives the reference at `reference`.
  fn of(source: &str, reference: &str) -> Self {
    tags(source, "meta")
      .into_iter()
      .filter(|attributes| attributes.get("name").is_some_and(|name| name == "fuzzy"))
      .filter_map(|attributes| attributes.get("content").cloned())
      .find_map(|content| {
        let (target, ranges) = match content.split_once(':') {
          Some((target, ranges)) if !target.contains('=') => (Some(target.trim()), ranges),
          _ => (None, content.as_str()),
        };

        if target.is_some_and(|target| !reference.ends_with(target)) {
          return None;
        }

        let mut parts = ranges.split(';').map(|part| {
          part
            .split_once('=')
            .map_or(part, |(_, range)| range)
            .trim()
            .to_owned()
        });
        let range = |text: Option<String>| -> Option<(u32, u32)> {
          let text = text?;
          let (low, high) = text.split_once('-').unwrap_or((&text, &text));

          Some((low.trim().parse().ok()?, high.trim().parse().ok()?))
        };
        let (low, high) = range(parts.next())?;

        Some(Self {
          max_difference: (low.min(255) as u8, high.min(255) as u8),
          total_pixels: range(parts.next())?,
        })
      })
      .unwrap_or(Self::EXACT)
  }

  /// Whether `test` and `reference`, both opaque RGBA, are within this allowance of each other.
  fn allows(self, test: &[u8], reference: &[u8]) -> bool {
    let mut max_difference = 0;
    let mut total_pixels = 0;

    for (test, reference) in test
      .as_chunks::<4>()
      .0
      .iter()
      .zip(reference.as_chunks::<4>().0)
    {
      let difference = test
        .iter()
        .zip(reference)
        .map(|(a, b)| a.abs_diff(*b))
        .max()
        .unwrap_or(0);

      if difference > 0 {
        max_difference = max_difference.max(difference);
        total_pixels += 1;
      }
    }

    (max_difference == 0 && total_pixels == 0)
      || (self.max_difference.0..=self.max_difference.1).contains(&max_difference)
        && (self.total_pixels.0..=self.total_pixels.1).contains(&total_pixels)
  }
}

/// What every test renders with: the test fonts plus WPT's Ahem, and the images the suites use.
struct Runner {
  root: PathBuf,
  verbose: bool,
  /// Where a failing test's rendering and its reference's go, as PNG.
  dump: Option<PathBuf>,
  fonts: Fonts,
  images: ResourceCache,
}

impl Runner {
  fn new(root: &Path) -> Self {
    let mut fonts = CONTEXT.clone();

    if let Ok(ahem) = fs::read(root.join("fonts/Ahem.ttf")) {
      fonts
        .register(FontResource::new(ahem).override_info(FontOverride {
          family_name: Some("Ahem".into()),
          ..Default::default()
        }))
        .unwrap();
    }

    Self {
      root: root.to_path_buf(),
      verbose: env::var_os("WPT_VERBOSE").is_some(),
      dump: env::var_os("WPT_DUMP").map(PathBuf::from),
      fonts,
      images: ResourceCache::default(),
    }
  }

  fn relative(&self, path: &Path) -> String {
    path
      .strip_prefix(&self.root)
      .unwrap_or(path)
      .to_string_lossy()
      .replace('\\', "/")
  }

  /// Where `href`, as written in the document at `from`, points.
  fn resolve(&self, from: &Path, href: &str) -> PathBuf {
    let href = href.split(['?', '#']).next().unwrap_or(href);

    match href.strip_prefix('/') {
      Some(absolute) => self.root.join(absolute),
      None => from.parent().unwrap_or(&self.root).join(href),
    }
  }

  fn run(&self, path: &Path) -> Outcome {
    let (outcome, reason) = match panic::catch_unwind(AssertUnwindSafe(|| self.compare(path))) {
      Ok(Ok(true)) => (Outcome::Pass, None),
      Ok(Ok(false)) => (Outcome::Fail, None),
      Ok(Err(reason)) => (Outcome::Error, Some(reason)),
      Err(_) => (Outcome::Crash, None),
    };

    if self.verbose && outcome != Outcome::Pass {
      eprintln!(
        "{}: {outcome:?}{}",
        self.relative(path),
        reason
          .map(|reason| format!(", {reason}"))
          .unwrap_or_default(),
      );
    }

    outcome
  }

  /// Whether the test at `path` renders as its references say it should.
  fn compare(&self, path: &Path) -> Result<bool, String> {
    let source = fs::read_to_string(path).map_err(|error| error.to_string())?;
    let test = self.render(path, &source)?;

    for (href, matches) in references(&source) {
      let reference_path = self.resolve(path, &href);
      let reference = if href == "about:blank" {
        self.render(path, "")?
      } else {
        fs::read_to_string(&reference_path)
          .map_err(|error| format!("{href}: {error}"))
          .and_then(|reference| self.render(&reference_path, &reference))?
      };

      if Fuzzy::of(&source, &href).allows(&test, &reference) != matches {
        if let Some(dump) = &self.dump {
          let name = self.relative(path).replace('/', "__");
          let (width, height) = VIEWPORT;

          for (suffix, pixels) in [("test", test.clone()), ("ref", reference)] {
            RgbaImage::from_raw(width, height, pixels)
              .ok_or("render size")?
              .save(dump.join(format!("{name}-{suffix}.png")))
              .map_err(|error| error.to_string())?;
          }
        }

        return Ok(false);
      }
    }

    Ok(true)
  }

  /// The document at `path` rendered onto white, as RGBA.
  fn render(&self, path: &Path, source: &str) -> Result<Vec<u8>, String> {
    let linked = tags(source, "link").into_iter().filter_map(|attributes| {
      attributes
        .get("rel")
        .is_some_and(|rel| rel.eq_ignore_ascii_case("stylesheet"))
        .then(|| fs::read_to_string(self.resolve(path, attributes.get("href")?)).ok())?
    });
    // An XHTML document's parser takes a `<style>` block's CDATA section apart before CSS sees it.
    let inline = source.split("<style").skip(1).filter_map(|block| {
      let (_, inner) = block.split_once('>')?;
      let (sheet, _) = inner.split_once("</style>")?;

      Some(sheet.replace("<![CDATA[", "").replace("]]>", ""))
    });
    let stylesheet = StyleSheet::parse_list_loosy(linked.chain(inline));

    let images: HashMap<Arc<str>, ImageSource> = tags(source, "img")
      .into_iter()
      .filter_map(|attributes| {
        let src = attributes.get("src")?;
        let data = fs::read(self.resolve(path, src)).ok()?;
        let image = self
          .images
          .get_or_decode(&data, ImageCacheMode::Auto)
          .ok()?;

        Some((Arc::from(src.as_str()), image))
      })
      .collect();
    let node = Node::from_html(source, FromHtmlOptions::default())
      .map_err(|error| format!("parse: {error}"))?;
    let bitmap = render(
      RenderOptions::builder()
        .viewport(Viewport::new(VIEWPORT))
        .node(node)
        .fonts(&self.fonts)
        .images(images)
        .stylesheet(stylesheet.into())
        .build(),
    )
    .map_err(|error| format!("render: {error}"))?;

    Ok(
      bitmap
        .as_raw()
        .as_chunks::<4>()
        .0
        .iter()
        .flat_map(|&[red, green, blue, alpha]| {
          let alpha = u32::from(alpha);
          let over_white =
            |channel: u8| ((u32::from(channel) * alpha + 255 * (255 - alpha)) / 255) as u8;

          [over_white(red), over_white(green), over_white(blue), 255]
        })
        .collect(),
    )
  }
}
