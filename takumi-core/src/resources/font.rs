use std::{
  borrow::Cow,
  cell::RefCell,
  collections::{BTreeSet, HashMap, HashSet, hash_map::Entry},
  fmt::{self, Debug, Formatter},
  iter::once,
  rc::Rc,
  str::FromStr,
  sync::Arc,
};

use parley::{
  FontFamilyName, GenericFamily as ParleyGenericFamily, GlyphRun, LayoutContext, TextStyle,
  TreeBuilder,
  fontique::{
    Attributes, Blob, Collection, CollectionOptions, FallbackKey, FontInfo, FontInfoOverride,
    FontStyle, FontWeight, FontWidth, QueryFamily, QueryStatus, Script, ScriptExt,
  },
};
use skrifa::{
  FontRef, MetadataProvider,
  instance::{LocationRef, Size},
  raw::types::{F2Dot14, Tag},
};
use thiserror::Error;
use xxhash_rust::xxh3::{Xxh3, xxh3_64};

use crate::{
  context::RenderContext,
  layout::inline::{InlineBrush, InlineLayout},
  resources::{
    glyph::{BOLD_THRESHOLD, GlyphResolveContext, ResolvedGlyph, synthesis_embolden_strength},
    glyph_cache::resolved_glyph,
  },
  style::{FontFamily, FontStyle as CssFontStyle},
};

/// Errors that can occur during font loading and conversion.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum FontError {
  /// Error occurred during WOFF conversion
  #[cfg(any(feature = "woff", feature = "woff2"))]
  #[error("WOFF conversion failed: {0}")]
  Woff(#[source] Box<dyn std::error::Error + Send + Sync + 'static>),
  /// Unsupported Font Format
  #[error("Unsupported font format")]
  UnsupportedFormat,
  /// Font index is invalid
  #[error("Font index is invalid")]
  InvalidFontIndex,
  /// The string is not a CSS generic font family keyword
  #[error("Unknown generic font family keyword")]
  UnknownGenericFamily,
}

/// Supported font formats for loading and processing
#[derive(Copy, Clone)]
#[non_exhaustive]
pub(crate) enum FontFormat {
  #[cfg(feature = "woff")]
  /// Web Open Font Format (WOFF) - compressed web font format
  Woff,
  #[cfg(feature = "woff2")]
  /// Web Open Font Format 2 (WOFF2) - improved compression web font format
  Woff2,
  /// TrueType Font format - standard desktop font format
  Ttf,
  /// OpenType Font format - extended font format with advanced typography
  Otf,
  /// TrueType Collection - multiple fonts in one file
  Ttc,
}

fn load_font(source: Cow<'_, [u8]>, format_hint: Option<FontFormat>) -> Result<Vec<u8>, FontError> {
  let format = if let Some(format) = format_hint {
    format
  } else {
    guess_font_format(&source)?
  };

  match format {
    FontFormat::Ttf | FontFormat::Otf | FontFormat::Ttc => Ok(source.into_owned()),
    #[cfg(feature = "woff2")]
    FontFormat::Woff2 => {
      let ttf = wuff::decompress_woff2(&source).map_err(|e| FontError::Woff(Box::new(e)))?;
      Ok(ttf)
    }
    #[cfg(feature = "woff")]
    FontFormat::Woff => {
      let ttf = wuff::decompress_woff1(&source).map_err(|e| FontError::Woff(Box::new(e)))?;
      Ok(ttf)
    }
  }
}

fn guess_font_format(source: &[u8]) -> Result<FontFormat, FontError> {
  if source.len() < 4 {
    return Err(FontError::UnsupportedFormat);
  }

  match &source[0..4] {
    #[cfg(feature = "woff2")]
    b"wOF2" => Ok(FontFormat::Woff2),
    #[cfg(feature = "woff")]
    b"wOFF" => Ok(FontFormat::Woff),
    [0x00, 0x01, 0x00, 0x00] => Ok(FontFormat::Ttf),
    b"OTTO" => Ok(FontFormat::Otf),
    b"ttcf" => Ok(FontFormat::Ttc),
    _ => Err(FontError::UnsupportedFormat),
  }
}

thread_local! {
  static LAYOUT_CONTEXT: RefCell<LayoutContext<InlineBrush>> = RefCell::new(LayoutContext::new());
}

fn resolved_glyph_cache_key(
  font_id: u64,
  font_index: u32,
  font_size: f32,
  coords: &[F2Dot14],
  embolden: Option<f32>,
  skew: Option<f32>,
  glyph_id: u32,
) -> u64 {
  let mut h = Xxh3::new();
  h.update(&font_id.to_le_bytes());
  h.update(&font_index.to_le_bytes());
  h.update(&font_size.to_le_bytes());
  for c in coords {
    h.update(&c.to_bits().to_le_bytes());
  }
  match embolden {
    Some(e) => {
      h.update(&[1u8]);
      h.update(&e.to_le_bytes());
    }
    None => h.update(&[0u8]),
  }
  match skew {
    Some(s) => {
      h.update(&[1u8]);
      h.update(&s.to_le_bytes());
    }
    None => h.update(&[0u8]),
  }
  h.update(&glyph_id.to_le_bytes());
  h.digest()
}

fn with_layout_context<R>(f: impl FnOnce(&mut LayoutContext<InlineBrush>) -> R) -> R {
  LAYOUT_CONTEXT.with(|cell| match cell.try_borrow_mut() {
    Ok(mut ctx) => f(&mut ctx),
    Err(_) => f(&mut LayoutContext::new()),
  })
}

/// A font family produced by [`Fonts::register`], with the faces it contains.
#[derive(Clone, Debug, serde::Serialize)]
pub struct RegisteredFamily {
  /// Family name as stored by the font system (normalized; reflects any override).
  pub name: String,
  /// Faces registered under this family.
  pub faces: Vec<RegisteredFace>,
}

/// A single face within a [`RegisteredFamily`].
#[derive(Clone, Debug, serde::Serialize)]
pub struct RegisteredFace {
  /// Weight class, typically `1.0..=1000.0`.
  pub weight: f32,
  /// CSS `font-style` value (`normal`, `italic`, or `oblique [<angle>deg]`).
  pub style: String,
  /// Width as a percentage of normal (e.g. `100.0`).
  pub width: f32,
  /// Index of the face within its source collection.
  pub index: u32,
}

fn font_style_css(style: FontStyle) -> String {
  match style {
    FontStyle::Normal => "normal".to_string(),
    FontStyle::Italic => "italic".to_string(),
    FontStyle::Oblique(None) => "oblique".to_string(),
    FontStyle::Oblique(Some(angle)) => format!("oblique {angle}deg"),
  }
}

/// The subset families under one logical family, ordered by the rank each declared and
/// then by name. The shaper walks this order and takes the first subset whose `cmap`
/// covers a cluster, so the rank is what keeps a codepoint two subsets both encode from
/// landing in the wrong one.
pub(crate) type SubsetGroup = BTreeSet<(u32, String)>;

/// Registered families split by whether they carry color glyph tables, so a
/// variation-selector segment can put the presentation-matching class first.
pub(crate) struct FontClasses {
  pub(crate) color: HashSet<String>,
  /// Color families in registration order.
  pub(crate) color_order: Vec<String>,
  /// Monochrome families in registration order.
  pub(crate) mono_order: Vec<String>,
}

const SBIX: Tag = Tag::new(b"sbix");
const COLR: Tag = Tag::new(b"COLR");
const CPAL: Tag = Tag::new(b"CPAL");
const CBDT: Tag = Tag::new(b"CBDT");
const CBLC: Tag = Tag::new(b"CBLC");

/// Color tables in the pairs Blink's `ColorTableLookup` accepts.
fn has_color_table(font: &FontRef) -> bool {
  font.table_data(SBIX).is_some()
    || (font.table_data(COLR).is_some() && font.table_data(CPAL).is_some())
    || (font.table_data(CBDT).is_some() && font.table_data(CBLC).is_some())
}

/// The registry of fonts available to a renderer.
///
/// Registration is the only mutation; afterwards the assembled parley context is immutable
/// and shared as `&self` across concurrent renders. Each render takes a cheap working clone
/// (see [`RenderContext`]) for parley's `&mut` query API, so no per-thread or global state
/// is needed.
#[derive(Clone)]
pub struct Fonts {
  inner: parley::FontContext,
  /// Maps a logical family name (the name authors write in `font-family`) to the
  /// unique internal names of the subset families registered under it, keyed by
  /// [`FontResource::subset_rank`] then name. Populated by [`FontResource::subset_of`];
  /// consulted when a render expands a `font-family` into its per-coverage subset stack.
  /// A `BTreeSet` so the stack never depends on registration arrival order, which callers
  /// racing concurrent registrations do not control. Shared (immutable after registration)
  /// so a render can read it without borrowing the parley context.
  groups: Arc<HashMap<String, SubsetGroup>>,
  /// Every registered family name in registration order. The fallback bucket is built from
  /// this so its per-script priority is deterministic; `fontique`'s `family_names()` iterates
  /// a `HashMap` (hash order), which would otherwise make font selection vary per render.
  order: Vec<String>,
  /// Families registered via [`FontResource::last_resort`], appended after `order` in the
  /// fallback bucket regardless of registration arrival.
  last_resort_order: Vec<String>,
  /// Families with at least one face carrying a color glyph table.
  color_names: HashSet<String>,
  /// Lazily built face store for SVG `<text>`; cleared on registration.
  #[cfg(feature = "svg")]
  svg_db: Option<Arc<crate::resvg::usvg::fontdb::Database>>,
  /// Stamped from a process-wide counter on every registration, so
  /// font-dependent caches (SVG `<text>` trees, their rasterizations) can
  /// tell any two registry states apart, including across `Fonts` instances.
  revision: u64,
}

impl Default for Fonts {
  fn default() -> Self {
    Self {
      inner: parley::FontContext {
        collection: Collection::new(CollectionOptions {
          system_fonts: false,
          shared: false,
        }),
        source_cache: Default::default(),
      },
      groups: Arc::new(HashMap::new()),
      order: Vec::new(),
      last_resort_order: Vec::new(),
      color_names: HashSet::new(),
      #[cfg(feature = "svg")]
      svg_db: None,
      revision: 0,
    }
  }
}

/// A render-local font handle. `groups` sits outside the `RefCell` so a render can expand
/// `font-family` without taking the parley-context borrow that building the tree holds.
#[derive(Clone)]
pub struct FontsSnapshot {
  context: Rc<RefCell<Fonts>>,
  pub(crate) groups: Arc<HashMap<String, SubsetGroup>>,
  pub(crate) classes: Arc<FontClasses>,
}

impl FontsSnapshot {
  /// Mutable access to the render-local parley context. Callers must not re-enter while the
  /// borrow is held (layout measures inline boxes before building the parley tree).
  pub(crate) fn with_context<R>(&self, f: impl FnOnce(&mut Fonts) -> R) -> R {
    f(&mut self.context.borrow_mut())
  }

  /// Face store for SVG `<text>` conversion, built once per snapshot.
  #[cfg(feature = "svg")]
  pub(crate) fn svg_fontdb(&self) -> Arc<crate::resvg::usvg::fontdb::Database> {
    self.with_context(Fonts::svg_fontdb)
  }

  /// Registration revision of the underlying font registry.
  #[cfg(feature = "svg")]
  pub(crate) fn revision(&self) -> u64 {
    self.with_context(|fonts| fonts.revision)
  }
}

/// What the matched face still needs to reach the requested style, once variable
/// axes have been applied: a faux bold stroke, a faux oblique skew, or neither.
pub(crate) struct RunSynthesis {
  /// Stroke width in px for synthetic bold.
  pub embolden: Option<f32>,
  /// Synthetic oblique angle in degrees.
  pub skew: Option<f32>,
}

/// Shared by the raster glyph cache and the PDF emitter so both fake the same faces.
pub(crate) fn run_synthesis(run: &GlyphRun<'_, InlineBrush>) -> RunSynthesis {
  let has_emoji_cluster = run
    .run()
    .visual_clusters()
    .any(|cluster| cluster.is_emoji());

  RunSynthesis {
    // Only synthesize bold at the CSS bold threshold (>= 600), matching browsers; a lighter
    // requested weight keeps the regular face rather than faux-bolding it.
    embolden: (!has_emoji_cluster
      && run.run().synthesis().embolden()
      && run.run().font_attrs().weight.value() >= BOLD_THRESHOLD
      && run.style().brush.font_synthesis.weight.is_allowed())
    .then_some(synthesis_embolden_strength(run.run().font_size())),
    skew: run
      .run()
      .synthesis()
      .skew()
      .filter(|_| !has_emoji_cluster)
      .filter(|_| run.style().brush.font_synthesis.style.is_allowed())
      .map(|degrees| -degrees),
  }
}

/// User-space variation coordinates the run was shaped at, e.g. `[(*b"wght", 700.0)]`.
/// Fontique writes the requested weight and width here when it instances a variable face.
pub(crate) fn run_variations(run: &GlyphRun<'_, InlineBrush>) -> Vec<([u8; 4], f32)> {
  run
    .run()
    .synthesis()
    .variation_settings()
    .iter()
    .map(|(tag, value)| (tag.to_be_bytes(), *value))
    .collect()
}

impl Fonts {
  /// Face store for SVG `<text>` conversion, sharing this collection's font
  /// bytes. Registration order drives fallback priority.
  #[cfg(feature = "svg")]
  pub(crate) fn svg_fontdb(&mut self) -> Arc<crate::resvg::usvg::fontdb::Database> {
    if let Some(database) = &self.svg_db {
      return database.clone();
    }

    let database = Arc::new(self.build_svg_fontdb());

    self.svg_db = Some(database.clone());
    database
  }

  #[cfg(feature = "svg")]
  fn build_svg_fontdb(&mut self) -> crate::resvg::usvg::fontdb::Database {
    use crate::resvg::usvg::fontdb::{Database, Family, Stretch, Style, Weight};

    fn stretch_bucket(width: FontWidth) -> Stretch {
      match width.ratio() {
        r if r <= 0.5625 => Stretch::UltraCondensed,
        r if r <= 0.6875 => Stretch::ExtraCondensed,
        r if r <= 0.8125 => Stretch::Condensed,
        r if r < 1.0 => Stretch::SemiCondensed,
        r if r < 1.125 => Stretch::Normal,
        r if r < 1.25 => Stretch::SemiExpanded,
        r if r < 1.5 => Stretch::Expanded,
        r if r < 2.0 => Stretch::ExtraExpanded,
        _ => Stretch::UltraExpanded,
      }
    }

    let mut database = Database::default();
    let names: Vec<String> = self
      .order
      .iter()
      .chain(self.last_resort_order.iter())
      .cloned()
      .collect();

    for name in names {
      let Some(family) = self.inner.collection.family_by_name(&name) else {
        continue;
      };

      for font in family.fonts().to_vec() {
        let Some(data) = font.load(Some(&mut self.inner.source_cache)) else {
          continue;
        };
        let style = match font.style() {
          FontStyle::Normal => Style::Normal,
          FontStyle::Italic => Style::Italic,
          FontStyle::Oblique(_) => Style::Oblique,
        };

        database.push_face(
          name.clone(),
          style,
          Weight(font.weight().value() as u16),
          stretch_bucket(font.width()),
          font.has_optical_size_axis(),
          data,
          font.index(),
        );
      }
    }

    for (generic, parley_generic) in [
      (Family::Serif, ParleyGenericFamily::Serif),
      (Family::SansSerif, ParleyGenericFamily::SansSerif),
      (Family::Cursive, ParleyGenericFamily::Cursive),
      (Family::Fantasy, ParleyGenericFamily::Fantasy),
      (Family::Monospace, ParleyGenericFamily::Monospace),
    ] {
      let ids: Vec<_> = self
        .inner
        .collection
        .generic_families(parley_generic)
        .collect();

      for id in ids {
        let Some(family) = self.inner.collection.family(id) else {
          continue;
        };
        let name = family.name().to_string();

        database.register_generic(generic, &name);
      }
    }

    database
  }

  /// Render-local snapshot with no extra fallbacks.
  pub fn snapshot(&self) -> FontsSnapshot {
    self.snapshot_with_fallbacks(None)
  }

  /// Render-local snapshot whose fallback bucket carries the given families.
  pub fn snapshot_with_fallbacks(&self, fallbacks: Option<&FontFamily>) -> FontsSnapshot {
    let mut cloned = self.inner.clone();

    let mut family_ids = if let Some(names) = fallbacks {
      // A name may be a logical subset family; expand it to its registered subset names so
      // the fallback bucket carries the whole stack, matching `font-family` expansion.
      let mut family_ids = Vec::new();
      for name in names.names() {
        let FontFamilyName::Named(literal_name) = name else {
          continue;
        };

        match self.groups.get(&*literal_name) {
          Some(subsets) => {
            family_ids.extend(
              subsets
                .iter()
                .filter_map(|(_, name)| cloned.collection.family_id(name)),
            );
          }
          None => family_ids.extend(cloned.collection.family_id(&literal_name)),
        }
      }
      family_ids
    } else {
      // Registration order, not `family_names()` (hash order), so font selection is stable.
      self
        .order
        .iter()
        .filter_map(|name| cloned.collection.family_id(name))
        .collect()
    };

    // Last-resort families close the bucket so they only serve uncovered text.
    family_ids.extend(
      self
        .last_resort_order
        .iter()
        .filter_map(|name| cloned.collection.family_id(name)),
    );

    for (script, _) in Script::all_samples() {
      cloned.collection.set_fallbacks(
        FallbackKey::new(*script, None),
        family_ids.clone().into_iter(),
      );
    }

    let (color_order, mono_order) = self
      .order
      .iter()
      .cloned()
      .partition(|name| self.color_names.contains(name));

    FontsSnapshot {
      context: Rc::new(RefCell::new(Self {
        inner: cloned,
        groups: self.groups.clone(),
        order: self.order.clone(),
        last_resort_order: self.last_resort_order.clone(),
        color_names: self.color_names.clone(),
        #[cfg(feature = "svg")]
        svg_db: self.svg_db.clone(),
        revision: self.revision,
      })),
      groups: self.groups.clone(),
      classes: Arc::new(FontClasses {
        color: self.color_names.clone(),
        color_order,
        mono_order,
      }),
    }
  }

  pub(crate) fn resolve_glyphs(
    &self,
    run: &GlyphRun<'_, InlineBrush>,
    font_ref: FontRef,
    glyph_ids: impl Iterator<Item = u32> + Clone,
  ) -> HashMap<u32, Arc<ResolvedGlyph>> {
    let font_size = run.run().font_size();
    let normalized_coords = run
      .run()
      .normalized_coords()
      .iter()
      .copied()
      .map(F2Dot14::from_bits)
      .collect::<Vec<_>>();
    let RunSynthesis { embolden, skew } = run_synthesis(run);

    let font_id = run.run().font().data.id();
    let font_index = run.run().font().index;
    let resolver = GlyphResolveContext {
      outline_glyphs: font_ref.outline_glyphs(),
      color_glyphs: font_ref.color_glyphs(),
      bitmap_strikes: font_ref.bitmap_strikes(),
      font_size,
      size: Size::new(font_size),
      location: LocationRef::new(&normalized_coords),
      embolden,
      skew,
    };

    let mut result: HashMap<u32, Arc<ResolvedGlyph>> = HashMap::new();
    for glyph_id in glyph_ids {
      if let Entry::Vacant(slot) = result.entry(glyph_id) {
        let key = resolved_glyph_cache_key(
          font_id,
          font_index,
          font_size,
          &normalized_coords,
          embolden,
          skew,
          glyph_id,
        );
        if let Some(glyph) = resolved_glyph(key, || resolver.resolve_glyph(glyph_id)) {
          slot.insert(glyph);
        }
      }
    }

    result
  }

  /// Registers a font, decoding it and skipping fonts already registered in this
  /// context (deduped by content + family name). Returns the families it produced.
  pub fn register(&mut self, font: FontResource) -> Result<Vec<RegisteredFamily>, FontError> {
    #[cfg(feature = "svg")]
    {
      self.svg_db = None;
    }

    static REVISION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

    self.revision = REVISION.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

    let FontResource {
      source,
      info_override,
      generic_family,
      subset_of,
      subset_rank,
      last_resort,
    } = font;

    let blob = source.into_blob()?;
    let axes = info_override.as_ref().map(FontOverride::resolved_axes);
    let info_override = info_override
      .as_ref()
      .zip(axes.as_deref())
      .map(|(info, axes)| info.to_parley(axes));
    let mut registered_fonts = self
      .inner
      .collection
      .register_fonts(blob.clone(), info_override);

    // fontique returns families in hash order; keep a multi-family file in face order.
    registered_fonts
      .sort_by_key(|(_, faces)| faces.iter().map(FontInfo::index).min().unwrap_or(u32::MAX));

    let mut families = Vec::with_capacity(registered_fonts.len());
    for (family, faces) in registered_fonts {
      let is_color = faces.iter().any(|face| {
        FontRef::from_index(blob.data(), face.index()).is_ok_and(|font| has_color_table(&font))
      });
      let faces = faces
        .iter()
        .map(|face| RegisteredFace {
          weight: face.weight().value(),
          style: font_style_css(face.style()),
          width: face.width().percentage(),
          index: face.index(),
        })
        .collect();
      let name = self
        .inner
        .collection
        .family_name(family)
        .unwrap_or_default()
        .to_string();

      let order = if last_resort {
        &mut self.last_resort_order
      } else {
        &mut self.order
      };

      if !order.contains(&name) {
        order.push(name.clone());
      }

      if is_color {
        self.color_names.insert(name.clone());
      }

      if let Some(logical) = &subset_of {
        Arc::make_mut(&mut self.groups)
          .entry(logical.clone())
          .or_default()
          .insert((subset_rank, name.clone()));
      }

      families.push(RegisteredFamily { name, faces });

      if let Some(generic_family) = generic_family {
        self
          .inner
          .collection
          .append_generic_families(generic_family.into_parlance(), once(family));
      }
    }

    Ok(families)
  }
}

impl RenderContext {
  /// First available font's line spacing for `families`/`attributes`, scaled to `font_size`.
  pub(crate) fn first_font_line_spacing<'a>(
    &self,
    families: impl IntoIterator<Item = QueryFamily<'a>>,
    attributes: Attributes,
    font_size: f32,
  ) -> Option<f32> {
    self.fonts.with_context(|fonts| {
      let mut query = fonts.inner.collection.query(&mut fonts.inner.source_cache);
      let mut result = None;

      query.set_families(families);
      query.set_attributes(attributes);

      query.matches_with(|font| {
        let Ok(font_ref) = FontRef::from_index(font.blob.data(), font.index) else {
          return QueryStatus::Continue;
        };
        let metrics = font_ref.metrics(Size::new(font_size), LocationRef::default());
        result = Some(metrics.ascent + metrics.descent + metrics.leading);
        QueryStatus::Stop
      });

      result
    })
  }

  /// Builds an inline layout with the given root style.
  pub(crate) fn tree_builder(
    &self,
    root_style: TextStyle<'_, '_, InlineBrush>,
    func: impl FnOnce(&mut TreeBuilder<'_, InlineBrush>),
  ) -> (InlineLayout, String) {
    self.fonts.with_context(|fonts| {
      with_layout_context(|layout| {
        let mut builder = layout.tree_builder(&mut fonts.inner, 1.0, true, &root_style);
        func(&mut builder);
        builder.build()
      })
    })
  }
}

/// Bytes the caller already holds behind an `Arc`, taken as they are.
type SharedBytes = Arc<dyn AsRef<[u8]> + Send + Sync>;

enum FontBytes<'a> {
  Inline(Cow<'a, [u8]>),
  Shared(SharedBytes),
}

impl FontBytes<'_> {
  fn into_owned(self) -> Vec<u8> {
    match self {
      Self::Inline(bytes) => bytes.into_owned(),
      Self::Shared(bytes) => (*bytes).as_ref().to_vec(),
    }
  }
}

impl Debug for FontBytes<'_> {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    f.debug_tuple("FontBytes")
      .field(&self.as_ref().len())
      .finish()
  }
}

impl AsRef<[u8]> for FontBytes<'_> {
  fn as_ref(&self) -> &[u8] {
    match self {
      Self::Inline(bytes) => bytes,
      Self::Shared(bytes) => (**bytes).as_ref(),
    }
  }
}

/// A font source buffer. Construct from raw bytes via `From`, or from bytes the
/// caller keeps alive elsewhere via [`FontSource::from_shared`]; woff/woff2 are
/// decompressed internally when the font is registered.
#[derive(Debug)]
pub struct FontSource<'a> {
  bytes: FontBytes<'a>,
  /// Whether `bytes` is already decompressed (woff/woff2 expanded to raw sfnt).
  is_decoded: bool,
  /// Blob id to use in place of hashing the decoded bytes, set by
  /// [`FontSource::from_static`].
  cache_id: Option<u64>,
}

impl<'a, T> From<T> for FontSource<'a>
where
  T: Into<Cow<'a, [u8]>>,
{
  fn from(value: T) -> Self {
    Self {
      bytes: FontBytes::Inline(value.into()),
      is_decoded: false,
      cache_id: None,
    }
  }
}

impl<'a> FontSource<'a> {
  /// Takes shared bytes as they are, so registering the font copies nothing: a
  /// memory-mapped file stays paged from disk and the process never holds a second
  /// copy on the heap. Only sfnt (ttf/otf/ttc) is passed through — woff and woff2
  /// still decompress into a fresh buffer.
  pub fn from_shared(bytes: Arc<dyn AsRef<[u8]> + Send + Sync>) -> Self {
    Self {
      bytes: FontBytes::Shared(bytes),
      is_decoded: false,
      cache_id: None,
    }
  }

  /// Takes bytes that live as long as the process, `include_bytes!` above all.
  /// Nothing is copied, and the blob id comes from the address and length rather
  /// than from the content, so registering a 30 MiB face never reads through it.
  /// A face embedded in the binary is then paged in one glyph at a time.
  ///
  /// The same slice always yields the same id, which is what the glyph caches
  /// need. Two faces with the same bytes may still land on separate ids, and
  /// resolve their glyphs separately, if one of them arrives as a `Vec`.
  pub fn from_static(bytes: &'static [u8]) -> Self {
    let mut id = Xxh3::new();

    id.update(&bytes.as_ptr().addr().to_le_bytes());
    id.update(&bytes.len().to_le_bytes());

    Self {
      bytes: FontBytes::Shared(Arc::new(bytes)),
      is_decoded: false,
      cache_id: Some(id.digest()),
    }
  }

  /// Whether the bytes can go to the font system untouched.
  fn is_sfnt(&self) -> bool {
    matches!(
      guess_font_format(self.bytes.as_ref()),
      Ok(FontFormat::Ttf | FontFormat::Otf | FontFormat::Ttc)
    )
  }

  fn into_decoded(self) -> Result<Self, FontError> {
    if self.is_decoded || (matches!(self.bytes, FontBytes::Shared(_)) && self.is_sfnt()) {
      return Ok(Self {
        is_decoded: true,
        ..self
      });
    }

    Ok(Self {
      bytes: FontBytes::Inline(Cow::Owned(load_font(
        Cow::Owned(self.bytes.into_owned()),
        None,
      )?)),
      is_decoded: true,
      cache_id: self.cache_id,
    })
  }

  fn into_blob(self) -> Result<Blob<u8>, FontError> {
    let passthrough = self.is_decoded || self.is_sfnt();
    let cache_id = self.cache_id;
    let decoded: SharedBytes = match self.bytes {
      FontBytes::Shared(bytes) if passthrough => bytes,
      bytes => Arc::new(load_font(Cow::Owned(bytes.into_owned()), None)?),
    };

    // `Blob::new` draws its id from a global counter, and that id keys the shared glyph
    // caches. Registering the same face again — a second renderer, a rebuilt one — would
    // then miss every glyph it had already resolved, so the id comes from the content.
    let id = cache_id.unwrap_or_else(|| xxh3_64((*decoded).as_ref()));

    Ok(Blob::from_raw_parts(decoded, id))
  }
}

impl<'a> AsRef<[u8]> for FontSource<'a> {
  fn as_ref(&self) -> &[u8] {
    self.bytes.as_ref()
  }
}

/// A CSS generic font family a registered font fulfills, exposed as named
/// constants so callers need not depend on `parley`.
/// <https://drafts.csswg.org/css-fonts/#generic-font-families>
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GenericFamily(ParleyGenericFamily);

impl GenericFamily {
  /// `serif`
  pub const SERIF: Self = Self(ParleyGenericFamily::Serif);
  /// `sans-serif`
  pub const SANS_SERIF: Self = Self(ParleyGenericFamily::SansSerif);
  /// `monospace`
  pub const MONOSPACE: Self = Self(ParleyGenericFamily::Monospace);
  /// `cursive`
  pub const CURSIVE: Self = Self(ParleyGenericFamily::Cursive);
  /// `fantasy`
  pub const FANTASY: Self = Self(ParleyGenericFamily::Fantasy);
  /// `system-ui`
  pub const SYSTEM_UI: Self = Self(ParleyGenericFamily::SystemUi);
  /// `ui-serif`
  pub const UI_SERIF: Self = Self(ParleyGenericFamily::UiSerif);
  /// `ui-sans-serif`
  pub const UI_SANS_SERIF: Self = Self(ParleyGenericFamily::UiSansSerif);
  /// `ui-monospace`
  pub const UI_MONOSPACE: Self = Self(ParleyGenericFamily::UiMonospace);
  /// `ui-rounded`
  pub const UI_ROUNDED: Self = Self(ParleyGenericFamily::UiRounded);
  /// `emoji`
  pub const EMOJI: Self = Self(ParleyGenericFamily::Emoji);
  /// `math`
  pub const MATH: Self = Self(ParleyGenericFamily::Math);
  /// `fangsong`
  pub const FANG_SONG: Self = Self(ParleyGenericFamily::FangSong);

  pub(crate) fn into_parlance(self) -> ParleyGenericFamily {
    self.0
  }
}

impl FromStr for GenericFamily {
  type Err = FontError;

  /// Parses a CSS generic family keyword (e.g. `monospace`, `sans-serif`).
  fn from_str(keyword: &str) -> Result<Self, Self::Err> {
    ParleyGenericFamily::parse(keyword)
      .map(Self)
      .ok_or(FontError::UnknownGenericFamily)
  }
}

/// Overrides for a registered font's metadata, letting callers rename its
/// family or pin weight/style/width/axes regardless of what the font file
/// itself declares.
#[derive(Debug, Default, Clone)]
pub struct FontOverride {
  /// Family name to register the font under, instead of its embedded name.
  pub family_name: Option<Arc<str>>,
  /// Font weight (CSS numeric, e.g. `400.0`) to use instead of the embedded one.
  pub weight: Option<f32>,
  /// Font style (slant) to use instead of the embedded one.
  pub style: Option<CssFontStyle>,
  /// Font width as a percentage (e.g. `100.0` for normal) to use instead of the
  /// embedded one.
  pub width: Option<f32>,
  /// Default values for named variation axes (four-byte OpenType tags). Axes not
  /// present in the font are ignored, as are tags that are not valid.
  pub axes: Vec<(String, f32)>,
}

impl FontOverride {
  fn resolved_axes(&self) -> Vec<(Tag, f32)> {
    self
      .axes
      .iter()
      .filter_map(|(tag, value)| {
        Tag::new_checked(tag.as_bytes())
          .ok()
          .map(|tag| (tag, *value))
      })
      .collect()
  }

  fn to_parley<'a>(&'a self, axes: &'a [(Tag, f32)]) -> FontInfoOverride<'a> {
    FontInfoOverride {
      family_name: self.family_name.as_deref(),
      width: self.width.map(FontWidth::from_percentage),
      style: self.style.map(CssFontStyle::into_parlance),
      weight: self.weight.map(FontWeight::new),
      axes: (!axes.is_empty()).then_some(axes),
    }
  }
}

#[derive(Debug)]
/// Information of a font resource
pub struct FontResource<'a> {
  source: FontSource<'a>,
  info_override: Option<FontOverride>,
  generic_family: Option<GenericFamily>,
  /// Logical family this font is a coverage subset of (see [`FontResource::subset_of`]).
  subset_of: Option<String>,
  /// Where this subset sits in its group's fallback order (see [`FontResource::subset_rank`]).
  subset_rank: u32,
  /// Sorts after every normal family in default fallback selection (see
  /// [`FontResource::last_resort`]).
  last_resort: bool,
}

impl<'a> FontResource<'a> {
  /// Create a new font to load
  pub fn new(source: impl Into<FontSource<'a>>) -> Self {
    Self {
      source: source.into(),
      info_override: None,
      generic_family: None,
      subset_of: None,
      subset_rank: 0,
      last_resort: false,
    }
  }

  /// Set font metadata overrides
  pub fn override_info(self, info_override: FontOverride) -> Self {
    Self {
      info_override: Some(info_override),
      ..self
    }
  }

  /// Set generic family for the font
  pub fn generic_family(self, generic_family: GenericFamily) -> Self {
    Self {
      generic_family: Some(generic_family),
      ..self
    }
  }

  /// Marks this font as a coverage subset of the logical family `logical`.
  ///
  /// Subsets sharing a logical family must each register under a UNIQUE family name
  /// (via [`FontResource::override_info`]) so the font system keeps them as distinct
  /// families — same-named faces collapse into one and never fall through on coverage.
  /// A render then expands `font-family: {logical}` into all its subsets, ordered by
  /// [`FontResource::subset_rank`], letting the shaper pick the first that covers each
  /// cluster.
  pub fn subset_of(self, logical: impl Into<String>) -> Self {
    Self {
      subset_of: Some(logical.into()),
      ..self
    }
  }

  /// Sets where this subset sits in its group's fallback order. Lowest is tried first;
  /// subsets sharing a rank order by family name.
  ///
  /// Coverage alone does not settle which subset serves a codepoint, because a subset's
  /// `cmap` is usually wider than the range it was cut for — Google Fonts encodes the
  /// ASCII space and several Latin capitals in its Cyrillic and Greek subsets. Ranking
  /// the subsets by the range they declare is what makes the shaper resolve those shared
  /// codepoints the way the `unicode-range` descriptor would in a browser.
  pub fn subset_rank(self, rank: u32) -> Self {
    Self {
      subset_rank: rank,
      ..self
    }
  }

  /// Sorts this font's families after every normal family in default fallback
  /// selection, so they only serve text no registered font covers.
  pub fn last_resort(self) -> Self {
    Self {
      last_resort: true,
      ..self
    }
  }

  /// Convert to resolved font resource, decompressing woff2/woff into a raw buffer.
  pub fn into_resolved(self) -> Result<Self, FontError> {
    let source = self.source.into_decoded()?;
    Ok(Self { source, ..self })
  }
}

#[cfg(test)]
#[allow(clippy::panic, clippy::unwrap_used)]
mod tests {
  use std::{fs::File, io::Read, path::Path};

  use super::*;
  use crate::style::FromCssStr;

  fn read_font_asset(relative: &str) -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
    let mut bytes = Vec::new();
    let mut file = File::open(&path)
      .unwrap_or_else(|error| panic!("failed to open test font {}: {error}", path.display()));
    file
      .read_to_end(&mut bytes)
      .unwrap_or_else(|error| panic!("failed to read test font {}: {error}", path.display()));
    bytes
  }

  fn geist_bytes() -> Vec<u8> {
    read_font_asset("../assets/fonts/geist/Geist[wght].woff2")
  }

  fn geist_mono_bytes() -> Vec<u8> {
    read_font_asset("../assets/fonts/geist/GeistMono[wght].woff2")
  }

  fn register_named(fonts: &mut Fonts, bytes: Vec<u8>, family_name: &str) -> Vec<RegisteredFamily> {
    fonts
      .register(FontResource::new(bytes).override_info(FontOverride {
        family_name: Some(family_name.into()),
        ..Default::default()
      }))
      .unwrap()
  }

  #[test]
  fn register_returns_family_with_at_least_one_face() {
    let mut fonts = Fonts::default();
    let families = register_named(&mut fonts, geist_bytes(), "Geist Test");

    assert_eq!(families.len(), 1);
    assert_eq!(families[0].name, "Geist Test");
    assert!(!families[0].faces.is_empty());
  }

  #[test]
  fn shared_sfnt_bytes_reach_the_font_system_uncopied() {
    let sfnt = load_font(Cow::Owned(geist_bytes()), None).unwrap();
    let bytes: Arc<dyn AsRef<[u8]> + Send + Sync> = Arc::new(sfnt);
    let address = (*bytes).as_ref().as_ptr();
    let blob = FontSource::from_shared(Arc::clone(&bytes))
      .into_blob()
      .unwrap();

    assert_eq!(blob.data().as_ptr(), address);
  }

  #[test]
  fn static_bytes_keep_one_blob_id_without_reading_them() {
    static TTF: &[u8] = &[0x00, 0x01, 0x00, 0x00];
    static OTF: &[u8] = b"OTTO";

    let id = |bytes| FontSource::from_static(bytes).into_blob().unwrap().id();

    // Same slice, same id, so a re-registered face keeps the glyphs it resolved.
    assert_eq!(id(TTF), id(TTF));
    assert_ne!(id(TTF), id(OTF));
  }

  #[test]
  fn static_sfnt_bytes_reach_the_font_system_uncopied() {
    let sfnt: &'static [u8] = load_font(Cow::Owned(geist_bytes()), None).unwrap().leak();
    let blob = FontSource::from_static(sfnt).into_blob().unwrap();

    assert_eq!(blob.data().as_ptr(), sfnt.as_ptr());
  }

  #[test]
  fn a_static_woff2_face_keeps_its_id_through_decompression() {
    let woff2: &'static [u8] = geist_bytes().leak();
    let registered = FontSource::from_static(woff2).into_blob().unwrap();
    let resolved_first = FontSource::from_static(woff2)
      .into_decoded()
      .unwrap()
      .into_blob()
      .unwrap();

    // The bytes the font system holds are the decompressed sfnt either way, so only
    // the id carried past decompression keeps the two from being separate faces.
    assert_ne!(registered.data().as_ptr(), woff2.as_ptr());
    assert_eq!(registered.id(), resolved_first.id());
  }

  #[test]
  fn shared_woff2_bytes_still_decompress() {
    let mut fonts = Fonts::default();
    let bytes: Arc<dyn AsRef<[u8]> + Send + Sync> = Arc::new(geist_bytes());
    let families = fonts
      .register(
        FontResource::new(FontSource::from_shared(bytes)).override_info(FontOverride {
          family_name: Some("Shared Geist".into()),
          ..Default::default()
        }),
      )
      .unwrap();

    assert_eq!(families[0].name, "Shared Geist");
    assert!(!families[0].faces.is_empty());
  }

  /// Concatenates sfnt fonts into a ttc, rebasing each table directory's offsets.
  fn build_ttc(fonts: &[Vec<u8>]) -> Vec<u8> {
    let header_len = 12 + 4 * fonts.len();
    let mut ttc = Vec::new();
    ttc.extend_from_slice(b"ttcf");
    ttc.extend_from_slice(&1u16.to_be_bytes());
    ttc.extend_from_slice(&0u16.to_be_bytes());
    ttc.extend_from_slice(&(fonts.len() as u32).to_be_bytes());

    let mut base = header_len as u32;
    for font in fonts {
      ttc.extend_from_slice(&base.to_be_bytes());
      base += font.len() as u32;
    }

    for font in fonts {
      let base = ttc.len() as u32;
      let mut font = font.clone();
      let table_count = u16::from_be_bytes([font[4], font[5]]) as usize;
      for entry in 0..table_count {
        let offset_at = 12 + entry * 16 + 8;
        let offset = u32::from_be_bytes(font[offset_at..offset_at + 4].try_into().unwrap());
        font[offset_at..offset_at + 4].copy_from_slice(&(offset + base).to_be_bytes());
      }
      ttc.extend_from_slice(&font);
    }

    ttc
  }

  #[test]
  fn multi_family_file_registers_in_face_order() {
    let geist = load_font(Cow::Owned(geist_bytes()), None).unwrap();
    let mono = load_font(Cow::Owned(geist_mono_bytes()), None).unwrap();
    let ttc = build_ttc(&[geist, mono]);

    for _ in 0..32 {
      let mut fonts = Fonts::default();
      let names: Vec<String> = fonts
        .register(FontResource::new(ttc.clone()))
        .unwrap()
        .into_iter()
        .map(|family| family.name)
        .collect();

      assert_eq!(names, ["Geist", "Geist Mono"]);
      assert_eq!(fonts.order, names);
    }
  }

  #[test]
  fn registering_same_bytes_twice_is_idempotent_in_order() {
    let mut fonts = Fonts::default();
    register_named(&mut fonts, geist_bytes(), "Geist Test");
    register_named(&mut fonts, geist_bytes(), "Geist Test");

    // Registration order dedups by name: registering the same family name twice does not
    // produce a second entry in `order`, so default fallback selection stays stable.
    assert_eq!(
      fonts
        .order
        .iter()
        .filter(|name| *name == "Geist Test")
        .count(),
      1
    );
  }

  #[test]
  fn font_override_family_name_renames_family() {
    let mut fonts = Fonts::default();
    let families = register_named(&mut fonts, geist_bytes(), "Renamed Family");

    assert_eq!(families[0].name, "Renamed Family");
    assert!(fonts.order.contains(&"Renamed Family".to_string()));
  }

  #[test]
  fn subset_of_groups_multiple_families_under_logical_name() {
    let mut fonts = Fonts::default();
    fonts
      .register(
        FontResource::new(geist_bytes())
          .override_info(FontOverride {
            family_name: Some("Subset A".into()),
            ..Default::default()
          })
          .subset_of("Logical"),
      )
      .unwrap();
    fonts
      .register(
        FontResource::new(geist_mono_bytes())
          .override_info(FontOverride {
            family_name: Some("Subset B".into()),
            ..Default::default()
          })
          .subset_of("Logical"),
      )
      .unwrap();

    let subsets = fonts.groups.get("Logical").expect("logical group present");
    assert_eq!(
      subsets,
      &BTreeSet::from([(0, "Subset A".to_string()), (0, "Subset B".to_string())])
    );

    let snapshot =
      fonts.snapshot_with_fallbacks(Some(&FontFamily::from_css_str("Logical").unwrap()));
    assert_eq!(snapshot.groups.get("Logical"), Some(subsets));
  }

  /// The rank a subset declares outranks its family name, so a group whose coverage order
  /// runs against the alphabet still resolves shared codepoints to the intended subset.
  #[test]
  fn subset_rank_orders_the_group_ahead_of_the_family_name() {
    let mut fonts = Fonts::default();
    fonts
      .register(
        FontResource::new(geist_bytes())
          .override_info(FontOverride {
            family_name: Some("Subset A".into()),
            ..Default::default()
          })
          .subset_of("Logical")
          .subset_rank(1),
      )
      .unwrap();
    fonts
      .register(
        FontResource::new(geist_mono_bytes())
          .override_info(FontOverride {
            family_name: Some("Subset B".into()),
            ..Default::default()
          })
          .subset_of("Logical")
          .subset_rank(0),
      )
      .unwrap();

    let subsets = fonts.groups.get("Logical").expect("logical group present");
    assert_eq!(
      subsets.iter().map(|(_, name)| name).collect::<Vec<_>>(),
      ["Subset B", "Subset A"]
    );
  }

  #[test]
  fn registration_order_defines_default_fallbacks() {
    let mut fonts = Fonts::default();
    register_named(&mut fonts, geist_bytes(), "B Family");
    register_named(&mut fonts, geist_mono_bytes(), "A Family");

    assert_eq!(
      fonts.order,
      vec!["B Family".to_string(), "A Family".to_string()]
    );
  }

  #[test]
  fn last_resort_families_sort_after_normal_registrations() {
    let mut fonts = Fonts::default();
    fonts
      .register(
        FontResource::new(geist_bytes())
          .override_info(FontOverride {
            family_name: Some("Embedded".into()),
            ..Default::default()
          })
          .last_resort(),
      )
      .unwrap();
    register_named(&mut fonts, geist_mono_bytes(), "User Font");

    // Registered first, but selected last: a last-resort family never shadows caller fonts.
    assert_eq!(fonts.order, vec!["User Font".to_string()]);
    assert_eq!(fonts.last_resort_order, vec!["Embedded".to_string()]);
  }

  #[test]
  fn same_bytes_produce_the_same_blob_id() {
    let first = FontSource::from(geist_bytes()).into_blob().unwrap();
    // Decoded up front rather than during `into_blob`, so the id has to come from the
    // decoded sfnt and not from whatever the caller happened to hand over.
    let second = FontSource::from(geist_bytes())
      .into_decoded()
      .unwrap()
      .into_blob()
      .unwrap();
    let other = FontSource::from(geist_mono_bytes()).into_blob().unwrap();

    // The glyph caches key on this id, so a re-registered face has to keep the entries
    // it already resolved.
    assert_eq!(first.id(), second.id());
    assert_ne!(first.id(), other.id());
  }

  #[test]
  fn unknown_family_in_fallbacks_does_not_panic() {
    let fonts = Fonts::default();
    let unknown = FontFamily::from_css_str("Never Registered").unwrap();

    // Must not panic: an unresolved name simply yields an empty fallback bucket.
    let _snapshot = fonts.snapshot_with_fallbacks(Some(&unknown));
  }
}
