//! Runs the web-platform-tests CSS reftests: renders each test and its reference, and compares the
//! pixels as the WPT harness does.
//! <https://web-platform-tests.org/writing-tests/reftests.html>
//!
//! The tests come from the WPT checkout `scripts/wpt-checkout.sh` makes in `target/wpt`, or the one
//! `WPT_ROOT` points at; without either the test does nothing.
//! `tests/wpt-reftests.txt` lists the tests expected to pass as rules, where `dir/` covers every
//! test under it, a leading `!` expects failures, and the most specific rule decides; the test fails
//! when an expected pass stops passing. `WPT_UPDATE=1` rewrites it with the fewest rules for the
//! tests that pass now, `WPT_WRITE=path` writes those rules to `path` and still checks for
//! regressions, `WPT_VERBOSE=1` prints each
//! test that does not pass, `WPT_DUMP=dir` writes each failing test's rendering and its
//! reference's there, `WPT_REPORT=path` writes each test's outcome, time, and error as
//! tab-separated lines, and `WPT=fragment,...` runs only the tests whose paths contain one of the
//! fragments.

mod test_utils;

use std::{
  collections::{BTreeMap, BTreeSet, HashMap, HashSet},
  env, fs,
  panic::{self, AssertUnwindSafe},
  path::{Path, PathBuf},
  process,
  sync::{
    Arc, Mutex, MutexGuard, PoisonError,
    atomic::{AtomicBool, AtomicUsize, Ordering},
  },
  thread,
  time::{Duration, Instant},
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

/// The run reports progress every this many finished tests.
const PROGRESS_EVERY: usize = 500;

/// How long one test may run before the run names it. Most take milliseconds, so a test past this
/// is caught in a layout bug.
const SLOW: Duration = Duration::from_secs(15);

/// How long one test may run before the run gives up on it and fails, as a stuck thread cannot be
/// stopped.
const HUNG: Duration = Duration::from_secs(120);

const EXPECTATIONS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/wpt-reftests.txt");

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
  let discovered = reftests(&root.join("css"));
  let all: Vec<String> = discovered
    .iter()
    .map(|path| runner.relative(path))
    .collect();
  let tests: Vec<&PathBuf> = discovered
    .iter()
    .filter(|path| matches_fragments(only.as_deref(), &path.to_string_lossy()))
    .collect();

  assert!(
    only.is_none() || !tests.is_empty(),
    "WPT={only:?} selects no reftests"
  );
  let started = Instant::now();
  let finished = AtomicUsize::new(0);
  let watchdog = Watchdog::default();
  let runs: BTreeMap<String, TestRun> = thread::scope(|scope| {
    scope.spawn(|| watchdog.watch());

    let runs = tests
      .par_iter()
      .map(|path| {
        let name = runner.relative(path);

        watchdog.start(&name);

        let run = runner.run(path);

        watchdog.finish(&name);

        let finished = finished.fetch_add(1, Ordering::Relaxed) + 1;

        if finished.is_multiple_of(PROGRESS_EVERY) {
          eprintln!(
            "WPT reftests: {finished}/{} after {:.0?}",
            tests.len(),
            started.elapsed()
          );
        }

        (name, run)
      })
      .collect();

    watchdog.stop();
    runs
  });

  if let Some(path) = env::var_os("WPT_REPORT") {
    let report: String = runs
      .iter()
      .map(|(name, run)| {
        format!(
          "{name}\t{:?}\t{}\t{}\n",
          run.outcome,
          run.elapsed.as_millis(),
          run.reason.as_deref().unwrap_or_default()
        )
      })
      .collect();

    fs::write(&path, report).unwrap();
  }

  let outcomes: BTreeMap<String, Outcome> = runs
    .into_iter()
    .map(|(name, run)| (name, run.outcome))
    .collect();
  let mut counts = BTreeMap::new();

  for outcome in outcomes.values() {
    *counts.entry(*outcome).or_insert(0) += 1;
  }

  eprintln!("WPT reftests: {counts:?}");

  let passing = |name: &String| outcomes.get(name) == Some(&Outcome::Pass);

  if let Some(path) = env::var_os("WPT_WRITE") {
    let passed: BTreeSet<&String> = outcomes.keys().filter(|name| passing(name)).collect();

    Expectations::compress(&passed, &all).write(Path::new(&path));
  }

  let expectations = Expectations::read(Path::new(EXPECTATIONS));
  let mut expected = expectations.names(&all);

  if env::var_os("WPT_UPDATE").is_some() {
    // A run keeps what it did not run, and the tests it ran that pass.
    expected.retain(|name| !outcomes.contains_key(*name));
    expected.extend(outcomes.keys().filter(|name| passing(name)));
    Expectations::compress(&expected, &all).write(Path::new(EXPECTATIONS));

    return;
  }

  // A pattern that matches no test regressed too, unless `WPT` left its tests out.
  let mut regressed: Vec<String> = expectations
    .unmatched(&all)
    .filter(|pattern| matches_fragments(only.as_deref(), pattern))
    .map(|pattern| format!("{pattern}: not found"))
    .collect();

  regressed.extend(
    expected
      .iter()
      .filter_map(|name| match outcomes.get(*name) {
        Some(Outcome::Pass) | None => None,
        Some(outcome) => Some(format!("{name}: {outcome:?}")),
      }),
  );

  assert!(regressed.is_empty(), "{regressed:#?}");
}

/// The rules `tests/wpt-reftests.txt` lists: `path` expects one test to pass and `dir/` every test
/// under it, and a leading `!` expects them to fail instead. The most specific rule for a test
/// decides, so a directory that mostly passes is one rule and its few failures.
struct Expectations {
  /// Whether each rule expects a pass, by the path or `dir/` it covers.
  rules: BTreeMap<String, bool>,
}

impl Expectations {
  const HEADER: &str = "# The WPT reftests expected to pass: `path` is one test and `dir/` every test under it, and\n# a leading `!` expects a failure. The most specific rule decides. `WPT_UPDATE=1 cargo test --test\n# wpt_reftests` rewrites it.\n";

  fn read(path: &Path) -> Self {
    let rules = fs::read_to_string(path)
      .unwrap_or_default()
      .lines()
      .map(str::trim)
      .filter(|line| !line.is_empty() && !line.starts_with('#'))
      .map(|line| match line.strip_prefix('!') {
        Some(rule) => (rule.to_owned(), false),
        None => (line.to_owned(), true),
      })
      .collect();

    Self { rules }
  }

  fn write(&self, path: &Path) {
    let mut text = Self::HEADER.to_owned();

    for (rule, passes) in &self.rules {
      if !passes {
        text.push('!');
      }

      text.push_str(rule);
      text.push('\n');
    }

    fs::write(path, text).unwrap();
  }

  /// Whether the most specific rule for `name` expects it to pass; no rule expects a failure.
  fn passes(&self, name: &str) -> bool {
    if let Some(&passes) = self.rules.get(name) {
      return passes;
    }

    name
      .rmatch_indices('/')
      .find_map(|(index, _)| self.rules.get(&name[..=index]).copied())
      .unwrap_or(false)
  }

  /// The fewest rules that expect exactly `passing` among `all` to pass: each directory states
  /// whichever of pass and fail most of what it holds does, below the state of its parent.
  fn compress(passing: &BTreeSet<&String>, all: &[String]) -> Self {
    let mut tree = Directory::default();

    for name in all {
      tree.insert(name, passing.contains(name));
    }

    let mut rules = BTreeMap::new();

    tree.encode("", false, &mut rules);

    Self { rules }
  }

  /// The tests among `all` the rules expect to pass.
  fn names<'a>(&self, all: &'a [String]) -> BTreeSet<&'a String> {
    all.iter().filter(|name| self.passes(name)).collect()
  }

  /// The rules that cover none of `all`.
  fn unmatched<'s>(&'s self, all: &'s [String]) -> impl Iterator<Item = &'s String> + 's {
    let covered: HashSet<&str> = all
      .iter()
      .flat_map(|name| {
        name
          .match_indices('/')
          .map(|(index, _)| &name[..=index])
          .chain([name.as_str()])
      })
      .collect();

    self
      .rules
      .keys()
      .filter(move |rule| !covered.contains(rule.as_str()))
  }
}

/// The tests under one directory, by whether each passes, for [`Expectations::compress`].
#[derive(Default)]
struct Directory {
  tests: BTreeMap<String, bool>,
  directories: BTreeMap<String, Directory>,
}

impl Directory {
  fn insert(&mut self, path: &str, passes: bool) {
    match path.split_once('/') {
      Some((directory, rest)) => self
        .directories
        .entry(format!("{directory}/"))
        .or_default()
        .insert(rest, passes),
      None => {
        self.tests.insert(path.to_owned(), passes);
      }
    }
  }

  /// The fewest rules for this directory under a parent that states `inherited`, and the state
  /// it states itself.
  fn cost(&self, inherited: bool) -> (usize, bool) {
    [inherited, !inherited]
      .into_iter()
      .map(|state| {
        let own = usize::from(state != inherited);
        let tests = self
          .tests
          .values()
          .filter(|&&passes| passes != state)
          .count();
        let directories: usize = self
          .directories
          .values()
          .map(|directory| directory.cost(state).0)
          .sum();

        (own + tests + directories, state)
      })
      .min_by_key(|&(cost, _)| cost)
      .unwrap_or((0, inherited))
  }

  /// Writes the rules [`Self::cost`] counts for this directory at `path`, under `inherited`.
  fn encode(&self, path: &str, inherited: bool, rules: &mut BTreeMap<String, bool>) {
    let (_, state) = self.cost(inherited);

    if state != inherited {
      rules.insert(path.to_owned(), state);
    }

    for (name, &passes) in &self.tests {
      if passes != state {
        rules.insert(format!("{path}{name}"), passes);
      }
    }

    for (name, directory) in &self.directories {
      directory.encode(&format!("{path}{name}"), state, rules);
    }
  }
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
        && fs::read_to_string(&path)
          .is_ok_and(|source| !references(&source).is_empty() && !changes_itself(&source))
      {
        found.push(path);
      }
    }
  }

  found.sort();

  found
}

/// Whether an inline script in `source` changes the document after it loads. takumi runs no
/// scripts, so such a test cannot show what it tests. Approximate: it looks for the DOM and style
/// APIs these tests use, not for every way a script can change a document.
fn changes_itself(source: &str) -> bool {
  const MUTATIONS: [&str; 12] = [
    ".style",
    "classList",
    "className",
    "setAttribute",
    "removeAttribute",
    "textContent",
    "innerHTML",
    "appendChild",
    "removeChild",
    "insertBefore",
    ".remove(",
    ".value",
  ];

  source.match_indices("<script").any(|(start, _)| {
    let Some((tag, rest)) = source[start..].split_once('>') else {
      return false;
    };
    let body = rest.split_once("</script>").map_or(rest, |(body, _)| body);

    !tag.contains("src=") && MUTATIONS.iter().any(|mutation| body.contains(mutation))
  })
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
  positioned_tags(source, name)
    .into_iter()
    .map(|(_, attributes)| attributes)
    .collect()
}

/// [`tags`], each with where it starts in `source`.
fn positioned_tags(source: &str, name: &str) -> Vec<(usize, HashMap<String, String>)> {
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

    found.push((start, attributes(&source[after..end])));
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

  /// The allowance `source`'s `<meta name=fuzzy>` gives the reference at `reference`: one naming
  /// that reference wins over one naming none.
  fn of(source: &str, reference: &str) -> Self {
    let allowances: Vec<(Option<String>, String)> = tags(source, "meta")
      .into_iter()
      .filter(|attributes| attributes.get("name").is_some_and(|name| name == "fuzzy"))
      .filter_map(|attributes| attributes.get("content").cloned())
      .map(|content| match content.split_once(':') {
        Some((target, ranges)) if !target.contains('=') => {
          (Some(target.trim().to_owned()), ranges.to_owned())
        }
        _ => (None, content),
      })
      .collect();
    let for_reference = allowances.iter().find(|(target, _)| {
      target
        .as_deref()
        .is_some_and(|target| reference.ends_with(target))
    });
    let generic = allowances.iter().find(|(target, _)| target.is_none());

    for_reference
      .or(generic)
      .and_then(|(_, ranges)| Self::parse(ranges))
      .unwrap_or(Self::EXACT)
  }

  /// The allowance in a `<meta name=fuzzy>` content's ranges, `maxDifference=1;totalPixels=0-2`.
  fn parse(ranges: &str) -> Option<Self> {
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
/// Names the tests that run past [`SLOW`] while they still run, and fails the run once one passes
/// [`HUNG`].
#[derive(Default)]
struct Watchdog {
  running: Mutex<HashMap<String, (Instant, bool)>>,
  stopped: AtomicBool,
}

impl Watchdog {
  fn start(&self, name: &str) {
    self.lock().insert(name.to_owned(), (Instant::now(), false));
  }

  fn finish(&self, name: &str) {
    self.lock().remove(name);
  }

  fn stop(&self) {
    self.stopped.store(true, Ordering::Relaxed);
  }

  fn lock(&self) -> MutexGuard<'_, HashMap<String, (Instant, bool)>> {
    self.running.lock().unwrap_or_else(PoisonError::into_inner)
  }

  fn watch(&self) {
    while !self.stopped.load(Ordering::Relaxed) {
      thread::sleep(Duration::from_secs(1));

      for (name, (started, reported)) in self.lock().iter_mut() {
        let elapsed = started.elapsed();

        if elapsed > HUNG {
          eprintln!("{name}: still running after {elapsed:.0?}, giving up");
          process::exit(1);
        }

        if elapsed > SLOW && !*reported {
          eprintln!("{name}: still running after {elapsed:.0?}");
          *reported = true;
        }
      }
    }
  }
}

/// How one test ran.
struct TestRun {
  outcome: Outcome,
  /// Why the test or its reference could not render, for [`Outcome::Error`].
  reason: Option<String>,
  elapsed: Duration,
}

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

  fn run(&self, path: &Path) -> TestRun {
    let started = Instant::now();
    let (outcome, reason) = match panic::catch_unwind(AssertUnwindSafe(|| self.compare(path))) {
      Ok(Ok(true)) => (Outcome::Pass, None),
      Ok(Ok(false)) => (Outcome::Fail, None),
      Ok(Err(reason)) => (Outcome::Error, Some(reason)),
      Err(_) => (Outcome::Crash, None),
    };

    let elapsed = started.elapsed();

    if elapsed > SLOW {
      eprintln!("{}: slow, took {elapsed:.1?}", self.relative(path));
    }

    if self.verbose && outcome != Outcome::Pass {
      eprintln!(
        "{}: {outcome:?}{}",
        self.relative(path),
        reason
          .as_deref()
          .map(|reason| format!(", {reason}"))
          .unwrap_or_default(),
      );
    }

    TestRun {
      outcome,
      reason,
      elapsed,
    }
  }

  /// Whether the test at `path` renders as its references say it should: like at least one
  /// `match` reference, and unlike every `mismatch` one.
  fn compare(&self, path: &Path) -> Result<bool, String> {
    let source = fs::read_to_string(path).map_err(|error| error.to_string())?;
    let test = self.render(path, &source)?;
    let mut has_match_reference = false;
    let mut matched = false;

    for (href, matches) in references(&source) {
      let reference_path = self.resolve(path, &href);
      let reference = if href == "about:blank" {
        self.render(path, "")?
      } else {
        fs::read_to_string(&reference_path)
          .map_err(|error| format!("{href}: {error}"))
          .and_then(|reference| self.render(&reference_path, &reference))?
      };

      let is_alike = Fuzzy::of(&source, &href).allows(&test, &reference);

      has_match_reference |= matches;
      matched |= matches && is_alike;

      if is_alike != matches {
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

        if !matches {
          return Ok(false);
        }
      }
    }

    Ok(!has_match_reference || matched)
  }

  /// The document at `path` rendered onto white, as RGBA.
  fn render(&self, path: &Path, source: &str) -> Result<Vec<u8>, String> {
    let linked = positioned_tags(source, "link")
      .into_iter()
      .filter_map(|(start, attributes)| {
        let is_stylesheet = attributes
          .get("rel")
          .is_some_and(|rel| rel.eq_ignore_ascii_case("stylesheet"));
        let sheet = fs::read_to_string(self.resolve(path, attributes.get("href")?)).ok()?;

        is_stylesheet.then_some((start, sheet))
      });
    // An XHTML document's parser takes a `<style>` block's CDATA section apart before CSS sees it.
    let inline = source.match_indices("<style").filter_map(|(start, _)| {
      let (_, inner) = source[start..].split_once('>')?;
      let (sheet, _) = inner.split_once("</style>")?;

      Some((start, sheet.replace("<![CDATA[", "").replace("]]>", "")))
    });
    // Later sheets win ties, so they keep document order.
    let mut sheets: Vec<(usize, String)> = linked.chain(inline).collect();

    sheets.sort_by_key(|(start, _)| *start);

    let stylesheet = StyleSheet::parse_list_loosy(sheets.into_iter().map(|(_, sheet)| sheet));

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

#[test]
fn the_most_specific_rule_decides() {
  let expectations = Expectations {
    rules: [
      ("css/a/", true),
      ("css/a/b/", false),
      ("css/a/b/c.html", true),
      ("css/a/d.html", false),
    ]
    .map(|(rule, passes)| (rule.to_owned(), passes))
    .into(),
  };

  assert!(expectations.passes("css/a/x.html"));
  assert!(!expectations.passes("css/a/d.html"));
  assert!(!expectations.passes("css/a/b/x.html"));
  assert!(expectations.passes("css/a/b/c.html"));
  assert!(!expectations.passes("css/z.html"));
}

#[test]
fn compress_folds_a_fully_passing_directory() {
  let all: Vec<String> = [
    "css/a/x.html",
    "css/a/y.html",
    "css/b/x.html",
    "css/b/y.html",
  ]
  .map(String::from)
  .to_vec();
  let passing: BTreeSet<&String> = [&all[0], &all[1], &all[2]].into_iter().collect();
  let expectations = Expectations::compress(&passing, &all);

  assert_eq!(
    expectations.rules,
    [
      ("css/a/".to_owned(), true),
      ("css/b/x.html".to_owned(), true)
    ]
    .into()
  );
  assert_eq!(expectations.names(&all), passing);
}

/// A directory that mostly passes is one rule and its failures.
#[test]
fn compress_lists_the_failures_of_a_mostly_passing_directory() {
  let all: Vec<String> = ["w", "x", "y", "z"]
    .map(|name| format!("css/a/{name}.html"))
    .to_vec();
  let passing: BTreeSet<&String> = all[..3].iter().collect();
  let expectations = Expectations::compress(&passing, &all);

  assert_eq!(
    expectations.rules,
    [
      ("css/a/".to_owned(), true),
      ("css/a/z.html".to_owned(), false)
    ]
    .into()
  );
  assert_eq!(expectations.names(&all), passing);
}
