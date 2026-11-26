//! The PDF render options a binding reads, and the render, measure, and needs passes they drive.

mod date;
mod metadata;

use std::{collections::HashMap, fmt::Display, iter::once, sync::Arc};

use serde::Deserialize;
use takumi_core::{
  Fonts,
  layout::{needs::RenderNeeds, node::Node},
  resources::{cache::Cache, image::ImageSource as DecodedImage},
  style::{Color, CssSource, FontFamily, FromCssStr, Lang, StyleSheet},
  viewport::Viewport,
};
use takumi_pdf::{
  Attachment, Band, MeasureOptions, MeasuredSize, PageMargin, PageMargins, PageOptions,
  PageOverride, PageRange, PageRules, PdfMetadata, PdfOptions, PdfStandard, Tagging, UncoveredText,
};

use crate::{
  FontScope, append_css,
  input::{ImageSource, decode_images, retain_svg},
  parse_lang,
  source_cache::ParsedSource,
  stylesheet,
};
use metadata::{AttachmentInput, MetadataInput, PdfaInput, TaggedInput};

fn message(error: impl Display) -> String {
  error.to_string()
}

/// Explicit page or viewport dimensions in CSS px.
#[derive(Deserialize, Clone, Copy)]
struct Dimensions {
  width: f32,
  height: f32,
}

/// Viewport for single-page output. A missing height sizes the page to the
/// laid-out content.
#[derive(Deserialize, Clone, Copy)]
struct ViewportInput {
  width: f32,
  height: Option<f32>,
}

/// A page size: a preset name (`"a4"`, `"letter"`) or explicit dimensions.
#[derive(Deserialize)]
#[serde(untagged)]
enum SizeInput {
  Named(String),
  Dimensions(Dimensions),
}

/// A page margin: one value for all sides, or per-side values (a side left out
/// is `auto`).
#[derive(Deserialize)]
#[serde(untagged)]
enum MarginInput {
  Uniform(SideInput),
  Sides {
    top: Option<SideInput>,
    right: Option<SideInput>,
    bottom: Option<SideInput>,
    left: Option<SideInput>,
  },
}

/// One side: a length in px, or a keyword for a size the renderer works out.
#[derive(Deserialize)]
#[serde(untagged)]
enum SideInput {
  Px(f32),
  Keyword(MarginKeyword),
}

/// A margin the renderer sizes itself.
#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum MarginKeyword {
  /// The space the band on that side takes.
  Auto,
}

impl SideInput {
  fn margin(&self) -> PageMargin {
    match self {
      Self::Px(value) => PageMargin::Px(*value),
      Self::Keyword(MarginKeyword::Auto) => PageMargin::Auto,
    }
  }
}

fn side_margin(side: &Option<SideInput>) -> PageMargin {
  side.as_ref().map(SideInput::margin).unwrap_or_default()
}

fn resolve_page(
  size: Option<&SizeInput>,
  landscape: bool,
  margin: Option<&MarginInput>,
) -> Result<PageOptions, String> {
  let mut page = match size {
    None => PageOptions::A4,
    Some(SizeInput::Dimensions(dimensions)) => PageOptions {
      width: dimensions.width,
      height: dimensions.height,
      ..PageOptions::A4
    },
    Some(SizeInput::Named(name)) => match name.to_ascii_lowercase().as_str() {
      "a3" => PageOptions::A3,
      "a4" => PageOptions::A4,
      "a5" => PageOptions::A5,
      "b4" => PageOptions::B4,
      "b5" => PageOptions::B5,
      "jis-b4" => PageOptions::JIS_B4,
      "jis-b5" => PageOptions::JIS_B5,
      "ledger" => PageOptions::LEDGER,
      "legal" => PageOptions::LEGAL,
      "letter" => PageOptions::LETTER,
      other => {
        return Err(format!("unknown page size: {other}"));
      }
    },
  };

  if landscape {
    page = page.landscape();
  }
  match margin {
    None => {}
    Some(MarginInput::Uniform(value)) => {
      let side = value.margin();

      page.margin = PageMargins {
        top: side,
        right: side,
        bottom: side,
        left: side,
      };
    }
    Some(MarginInput::Sides {
      top,
      right,
      bottom,
      left,
    }) => {
      page.margin = PageMargins {
        top: side_margin(top),
        right: side_margin(right),
        bottom: side_margin(bottom),
        left: side_margin(left),
      };
    }
  }
  Ok(page)
}

/// One entry of `pageRanges`: a 1-based page number, or an inclusive span.
#[derive(Deserialize, Clone, Copy)]
#[serde(untagged)]
enum PageRangeInput {
  Single(usize),
  Span {
    from: Option<usize>,
    to: Option<usize>,
  },
}

impl From<PageRangeInput> for PageRange {
  fn from(input: PageRangeInput) -> Self {
    match input {
      PageRangeInput::Single(page) => PageRange::single(page),
      PageRangeInput::Span { from, to } => PageRange { from, to },
    }
  }
}

/// The paper color, parsed from a CSS color.
fn page_background(color: Option<&str>) -> Result<Option<Color>, String> {
  color
    .map(|color| {
      Color::from_css_str(color).map_err(|_| format!("backgroundColor is not a color: {color}"))
    })
    .transpose()
}

/// Options for rendering a PDF, as a binding reads them from JavaScript.
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PdfRenderOptions {
  /// Fixed viewport for single-page output: percentage heights resolve
  /// against it and overflowing content is clipped. Mutually exclusive with
  /// the paged fields.
  viewport: Option<ViewportInput>,
  /// Page size for paged output. Defaults to A4.
  size: Option<SizeInput>,
  /// Swaps the page's width and height, including explicit sizes.
  landscape: Option<bool>,
  /// Page margin in CSS px: one number or per-side values.
  margin: Option<MarginInput>,
  /// Paper color as a CSS color, painted under everything on every page.
  background_color: Option<String>,
  /// Band repeated at the top of every page. Nodes classed `pageNumber` /
  /// `totalPages` receive the counters, optionally formatted by a
  /// supported `@counter-style` name in the same class list.
  header: Option<BandInput>,
  /// Band repeated at the bottom of every page; same class hooks as `header`.
  footer: Option<BandInput>,
  /// What some pages draw differently from the rest.
  pages: Option<PageRulesInput>,
  /// The pages the output keeps, 1-based. Page counters keep their full-output
  /// numbers.
  page_ranges: Option<Vec<PageRangeInput>>,
  /// Pre-fetched images keyed by URL.
  images: Option<Vec<ImageSource>>,
  /// CSS to apply before layout: stylesheet text, or a rule written as an
  /// object.
  css: Option<Vec<CssSource>>,
  /// Per-render font stack: ordered family names used as the fallback chain.
  pub font_families: Option<Vec<String>>,
  /// The fonts the render lists and the ones that failed.
  #[serde(default)]
  pub font_scope: FontScope,
  /// Default BCP-47 language tag applied to the root.
  lang: Option<String>,
  /// Document metadata written to the PDF's info dictionary.
  metadata: Option<MetadataInput>,
  /// Generates a PDF outline (bookmarks) from `h1`–`h6` headings.
  outline: Option<bool>,
  /// PDF/A conformance level: "2a", "2b", "2u", "3a", "3b", "3u", "4" or
  /// "4f".
  pdfa: Option<PdfaInput>,
  /// Structure-tree emission: `false`, `true` (default), or `"ua1"` / `"ua2"`
  /// to also validate against that accessibility standard.
  tagged: Option<TaggedInput>,
  /// Files attached to the document.
  attachments: Option<Vec<AttachmentInput>>,
  /// What a character no registered font covers turns into: `"error"`
  /// (default), `"placeholder"` or `"blank"`.
  uncovered_text: Option<UncoveredTextInput>,
}

/// `uncoveredText` names accepted from JS.
#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
enum UncoveredTextInput {
  Error,
  Placeholder,
  Blank,
}

impl From<UncoveredTextInput> for UncoveredText {
  fn from(input: UncoveredTextInput) -> Self {
    match input {
      UncoveredTextInput::Error => Self::Error,
      UncoveredTextInput::Placeholder => Self::Placeholder,
      UncoveredTextInput::Blank => Self::Blank,
    }
  }
}

/// A band as the caller writes it: a node, HTML, or `false` for none.
#[derive(Deserialize)]
#[serde(untagged)]
enum BandInput {
  Off(False),
  Node(Box<Node>),
  Html(String),
}

impl BandInput {
  /// The tree this band draws, once `resolve_bands` parsed its HTML.
  fn node(&self) -> Option<&Node> {
    match self {
      Self::Node(node) => Some(node),
      Self::Off(False) | Self::Html(_) => None,
    }
  }
}

/// The one boolean a band takes.
#[derive(Deserialize)]
#[serde(try_from = "bool")]
struct False;

impl TryFrom<bool> for False {
  type Error = String;

  fn try_from(value: bool) -> Result<Self, Self::Error> {
    match value {
      false => Ok(Self),
      true => Err("a band is a node, HTML, or false; true names nothing".into()),
    }
  }
}

impl From<BandInput> for Band {
  fn from(input: BandInput) -> Self {
    match input {
      BandInput::Off(False) => Self::Off,
      BandInput::Node(node) => Self::from(*node),
      // `resolve_bands` parses every HTML band into a node before the options convert.
      BandInput::Html(_) => {
        debug_assert!(false, "an HTML band reached conversion unparsed");

        Self::Off
      }
    }
  }
}

/// What one kind of page draws instead of the document's own setting.
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PageOverrideInput {
  header: Option<BandInput>,
  footer: Option<BandInput>,
}

impl From<PageOverrideInput> for PageOverride {
  fn from(input: PageOverrideInput) -> Self {
    Self {
      header: input.header.map(Band::from),
      footer: input.footer.map(Band::from),
    }
  }
}

/// Overrides keyed by the pages they cover.
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PageRulesInput {
  first: Option<PageOverrideInput>,
  last: Option<PageOverrideInput>,
  odd: Option<PageOverrideInput>,
  even: Option<PageOverrideInput>,
}

impl PdfRenderOptions {
  /// Every band, the page rules' included.
  fn bands_mut(&mut self) -> impl Iterator<Item = &mut BandInput> {
    let rules = self.pages.iter_mut().flat_map(|pages| {
      [
        &mut pages.first,
        &mut pages.last,
        &mut pages.odd,
        &mut pages.even,
      ]
      .into_iter()
      .flatten()
      .flat_map(|rule| [&mut rule.header, &mut rule.footer])
    });

    [&mut self.header, &mut self.footer]
      .into_iter()
      .chain(rules)
      .flatten()
  }
}

impl From<PageRulesInput> for PageRules {
  fn from(input: PageRulesInput) -> Self {
    let rule = |rule: Option<PageOverrideInput>| rule.map(PageOverride::from).unwrap_or_default();

    Self {
      first: rule(input.first),
      last: rule(input.last),
      odd: rule(input.odd),
      even: rule(input.even),
    }
  }
}

/// Splits the options into single-page viewport or paged geometry, rejecting
/// a mix of both.
fn resolve_geometry(
  options: &PdfRenderOptions,
) -> Result<(Option<Viewport>, Option<PageOptions>), String> {
  let paged_field_set = options.size.is_some()
    || options.landscape.is_some()
    || options.margin.is_some()
    || options.header.is_some()
    || options.footer.is_some()
    || options.pages.is_some()
    || options.page_ranges.is_some();

  match options.viewport {
    Some(_) if paged_field_set => Err(
      "viewport is mutually exclusive with the paged options (size, landscape, margin, header, footer, pages, pageRanges)"
        .to_owned(),
    ),
    Some(input) => Ok((
      Some(Viewport::new((
        input.width as u32,
        input.height.map(|height| height as u32),
      ))),
      None,
    )),
    None => Ok((
      None,
      Some(resolve_page(
        options.size.as_ref(),
        options.landscape.unwrap_or(false),
        options.margin.as_ref(),
      )?),
    )),
  }
}

/// The layout inputs a render, a measure, and a needs pass share.
struct Layout {
  viewport: Option<Viewport>,
  page: Option<PageOptions>,
  images: HashMap<Arc<str>, DecodedImage>,
  lang: Option<Lang>,
}

/// What every tree of a document lays out with as [`takumi_pdf::measure`] lays it out.
struct MeasureInputs {
  layout: Layout,
  fonts: Arc<Fonts>,
  stylesheet: Arc<StyleSheet>,
  cache: Cache,
  font_families: Option<FontFamily>,
}

impl MeasureInputs {
  fn options(&self, node: Node) -> MeasureOptions<'_> {
    MeasureOptions {
      viewport: self.layout.viewport,
      fonts: &self.fonts,
      node,
      stylesheet: self.stylesheet.clone(),
      images: self.layout.images.clone(),
      cache: self.cache.clone(),
      page: self.layout.page,
      font_families: self.font_families.clone(),
      lang: self.layout.lang,
    }
  }
}

impl PdfRenderOptions {
  /// Parses every HTML band with `parse`, then appends the `<style>` CSS of the document and its
  /// bands to `css`.
  pub fn resolve_bands<E>(
    &mut self,
    mut document_css: Vec<String>,
    mut parse: impl FnMut(&str) -> Result<ParsedSource, E>,
  ) -> Result<(), E> {
    for band in self.bands_mut() {
      if let BandInput::Html(html) = band {
        let (parsed, css) = parse(html)?;

        document_css.extend(css);
        *band = BandInput::Node(Box::new(parsed));
      }
    }

    self.css = append_css(self.css.take(), document_css);

    Ok(())
  }

  fn layout(&self, cache: &Cache) -> Result<Layout, String> {
    let images =
      decode_images(cache, self.images.as_deref().unwrap_or_default()).map_err(message)?;
    let (viewport, page) = resolve_geometry(self)?;
    let lang = parse_lang(self.lang.as_deref()).map_err(message)?;

    Ok(Layout {
      viewport,
      page,
      images,
      lang,
    })
  }

  fn measure_inputs(&mut self, fonts: Arc<Fonts>, cache: &Cache) -> Result<MeasureInputs, String> {
    Ok(MeasureInputs {
      layout: self.layout(cache)?,
      fonts,
      stylesheet: stylesheet(cache, self.css.take(), Vec::new()).map_err(message)?,
      cache: cache.clone(),
      font_families: self.font_families.take().map(FontFamily::from_names),
    })
  }

  /// Renders `node` to PDF bytes, its bands resolved by [`Self::resolve_bands`].
  pub fn render(self, node: Node, fonts: &Fonts, cache: &Cache) -> Result<Vec<u8>, String> {
    let Layout {
      viewport,
      page,
      images,
      lang,
    } = self.layout(cache)?;

    takumi_pdf::render(PdfOptions {
      viewport,
      fonts,
      node,
      stylesheet: stylesheet(cache, self.css, Vec::new()).map_err(message)?,
      images,
      cache: cache.clone(),
      page,
      page_ranges: self
        .page_ranges
        .map(|ranges| ranges.into_iter().map(PageRange::from).collect()),
      background_color: page_background(self.background_color.as_deref())?,
      header: self.header.map(Band::from).and_then(Band::node),
      footer: self.footer.map(Band::from).and_then(Band::node),
      pages: self.pages.map(PageRules::from).unwrap_or_default(),
      font_families: self.font_families.map(FontFamily::from_names),
      lang,
      metadata: self.metadata.map(PdfMetadata::try_from).transpose()?,
      producer: None,
      outline: self.outline.unwrap_or(false),
      standard: self.pdfa.map(PdfStandard::from).unwrap_or_default(),
      tagged: self.tagged.map(Tagging::from).unwrap_or_default(),
      attachments: self
        .attachments
        .unwrap_or_default()
        .into_iter()
        .map(Attachment::try_from)
        .collect::<Result<_, _>>()?,
      uncovered_text: self
        .uncovered_text
        .map(UncoveredText::from)
        .unwrap_or_default(),
    })
    .map_err(message)
  }

  /// Lays out `node` with `fonts` without rendering it, as [`takumi_pdf::measure`] does.
  pub fn measure(
    mut self,
    node: Node,
    fonts: Arc<Fonts>,
    cache: &Cache,
  ) -> Result<MeasuredSize, String> {
    let inputs = self.measure_inputs(fonts, cache)?;

    takumi_pdf::measure(inputs.options(node)).map_err(message)
  }

  /// The images `node` and the bands look up and the pending fonts in `fonts` their text loads.
  pub fn needs(
    mut self,
    node: Node,
    fonts: Arc<Fonts>,
    cache: &Cache,
  ) -> Result<RenderNeeds, String> {
    retain_svg(&mut self.images);

    let inputs = self.measure_inputs(fonts, cache)?;
    let bands = self
      .bands_mut()
      .filter_map(|band| band.node().cloned())
      .collect::<Vec<_>>();

    once(node)
      .chain(bands)
      .map(|tree| takumi_pdf::needs(inputs.options(tree)))
      .collect::<Result<RenderNeeds, _>>()
      .map_err(message)
  }
}
