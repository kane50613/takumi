use std::{
  borrow::Cow,
  collections::HashMap,
  fs,
  io::{self, IsTerminal, Read, Write},
  path::{Path, PathBuf},
  str::FromStr,
  sync::Arc,
};

use anyhow::{Context, Result, anyhow, bail};
use arboard::{Clipboard, ImageData as ClipboardImage};
use base64::{Engine, engine::general_purpose::STANDARD};
use clap::{
  ArgAction, Parser, ValueEnum,
  builder::{
    Styles,
    styling::{AnsiColor, Effects},
  },
};
use parley::Language;
use scraper::{
  ElementRef, Html,
  node::{Element, Node as HtmlNode},
};
use takumi::{
  prelude::{
    Direction, DitheringAlgorithm, FontResource, Fonts, ImageData, ImageSource, Node,
    OutputFormat as TakumiOutputFormat, Quality, RenderOptions, Style, StyleDeclarationBlock,
    StyleSheet, Viewport, tw::TailwindValues,
  },
  render, write_image,
};
use url::Url;

const DEFAULT_FONT: &[u8] = include_bytes!("../../assets/fonts/geist/Geist[wght].woff2");
const KITTY_CHUNK_SIZE: usize = 4096;

#[derive(Debug, Parser)]
#[command(
  name = "takumi",
  version,
  about = "Render Takumi node JSON or static HTML to an image",
  styles = clap_styles(),
  after_help = "Examples:
  takumi scene.json
  takumi card.html
  takumi ./site
  takumi https://example.com/card.html
  takumi scene.json -o image.webp --format webp
  takumi - < scene.json --width 1200 --height 630 --clipboard
  takumi scene.json --image avatar=./avatar.png --font ./Inter.ttf"
)]
struct Cli {
  /// Takumi node JSON, static HTML file, directory with index.html, URL, or '-' for stdin.
  input: Option<String>,

  /// Save the encoded image to this path.
  #[arg(short, long)]
  output: Option<PathBuf>,

  /// Output image format. Inferred from --output when omitted.
  #[arg(short, long, value_enum)]
  format: Option<ImageFormat>,

  /// Quality for JPEG and lossy WebP.
  #[arg(short, long, value_parser = clap::value_parser!(u8).range(0..=100))]
  quality: Option<u8>,

  /// Render WebP losslessly. This is the default for WebP when --quality is not set.
  #[arg(long)]
  lossless: bool,

  /// CSS viewport width in pixels.
  #[arg(short = 'W', long, default_value_t = 1200)]
  width: u32,

  /// CSS viewport height in pixels.
  #[arg(short = 'H', long, default_value_t = 630)]
  height: u32,

  /// Device pixel ratio used for rasterization.
  #[arg(long, default_value_t = 1.0)]
  dpr: f32,

  /// Render animation styles at this timeline position.
  #[arg(long, default_value_t = 0)]
  time_ms: u64,

  /// Draw layout debug borders.
  #[arg(long)]
  debug: bool,

  /// Output dithering algorithm.
  #[arg(long, value_enum, default_value_t = Dither::None)]
  dithering: Dither,

  /// Register a font file. May be passed multiple times.
  #[arg(long = "font", value_name = "PATH")]
  fonts: Vec<PathBuf>,

  /// Skip the bundled Geist font.
  #[arg(long)]
  no_default_fonts: bool,

  /// Restrict render fallback families in order. May be passed multiple times.
  #[arg(long = "font-family", value_name = "NAME")]
  font_families: Vec<String>,

  /// Default BCP-47 language for shaping and line breaking.
  #[arg(long)]
  lang: Option<String>,

  /// Add a CSS stylesheet file. May be passed multiple times.
  #[arg(long = "stylesheet", value_name = "PATH")]
  stylesheets: Vec<PathBuf>,

  /// Add inline CSS stylesheet text. May be passed multiple times.
  #[arg(long = "css", value_name = "TEXT")]
  css: Vec<String>,

  /// Preload an image as SRC=PATH for image nodes that reference SRC.
  #[arg(long = "image", value_name = "SRC=PATH", value_parser = parse_image_arg)]
  images: Vec<ImageArg>,

  /// Copy the rendered bitmap to the system clipboard.
  #[arg(long)]
  clipboard: bool,

  /// Disable terminal image preview.
  #[arg(long = "no-display", action = ArgAction::SetFalse)]
  preview: bool,

  /// Suppress status messages.
  #[arg(long)]
  quiet: bool,
}

#[derive(Clone, Debug)]
struct ImageArg {
  src: String,
  path: PathBuf,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum ImageFormat {
  Png,
  Jpeg,
  Webp,
  WebpLossless,
  Ico,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum Dither {
  None,
  OrderedBayer,
  FloydSteinberg,
}

fn clap_styles() -> Styles {
  Styles::styled()
    .header(AnsiColor::Green.on_default() | Effects::BOLD)
    .usage(AnsiColor::Green.on_default() | Effects::BOLD)
    .literal(AnsiColor::Cyan.on_default())
    .placeholder(AnsiColor::Yellow.on_default())
}

fn main() -> Result<()> {
  let cli = Cli::parse();
  cli.validate()?;

  let source = read_render_source(cli.input.as_deref())?;

  let mut fonts = Fonts::default();
  if !cli.no_default_fonts {
    fonts
      .register(FontResource::new(DEFAULT_FONT))
      .context("failed to register bundled Geist font")?;
  }
  for font_path in &cli.fonts {
    let data = fs::read(font_path)
      .with_context(|| format!("failed to read font {}", font_path.display()))?;
    fonts
      .register(FontResource::new(&data))
      .with_context(|| format!("failed to register font {}", font_path.display()))?;
  }

  let stylesheet = load_stylesheets(&cli, source.stylesheets)?;
  let mut images = source.images;
  images.extend(load_images(&cli.images)?);
  let lang = parse_lang(cli.lang.as_deref())?;
  let font_families = (!cli.font_families.is_empty()).then_some(cli.font_families.clone());

  let options = RenderOptions::builder()
    .viewport(Viewport::new((cli.width, cli.height)).with_device_pixel_ratio(cli.dpr))
    .node(source.node)
    .fonts(&fonts)
    .draw_debug_border(cli.debug)
    .images(images)
    .stylesheet(stylesheet)
    .time_ms(cli.time_ms)
    .dithering(cli.dithering.into())
    .font_families(font_families)
    .lang(lang)
    .build();

  let bitmap = render(options).context("render failed")?;

  if cli.clipboard {
    copy_to_clipboard(&bitmap).context("failed to copy image to clipboard")?;
    status(&cli, "copied", "bitmap copied to clipboard");
  }

  if let Some(output) = &cli.output {
    let format = resolve_output_format(cli.format, output, cli.quality, cli.lossless)?;
    let mut file =
      fs::File::create(output).with_context(|| format!("failed to create {}", output.display()))?;
    write_image(&bitmap, &mut file, format)
      .with_context(|| format!("failed to write {}", output.display()))?;
    status(&cli, "saved", &output.display().to_string());
  }

  if cli.preview && io::stdout().is_terminal() {
    let mut png = Vec::new();
    write_image(&bitmap, &mut png, TakumiOutputFormat::Png)
      .context("failed to encode PNG preview")?;
    write_kitty_image(&png).context("failed to write Kitty graphics preview")?;
    status(
      &cli,
      "rendered",
      &format!("{}x{} px", bitmap.width(), bitmap.height()),
    );
  } else if cli.preview && !cli.quiet {
    status(
      &cli,
      "skipped",
      "stdout is not a terminal; preview disabled",
    );
  }

  Ok(())
}

impl Cli {
  fn validate(&self) -> Result<()> {
    if self.width == 0 || self.height == 0 {
      bail!("--width and --height must be greater than zero");
    }
    if self.dpr <= 0.0 {
      bail!("--dpr must be greater than zero");
    }
    Ok(())
  }
}

impl From<Dither> for DitheringAlgorithm {
  fn from(value: Dither) -> Self {
    match value {
      Dither::None => Self::None,
      Dither::OrderedBayer => Self::OrderedBayer,
      Dither::FloydSteinberg => Self::FloydSteinberg,
    }
  }
}

struct RenderSource {
  node: Node,
  stylesheets: Vec<String>,
  images: HashMap<Arc<str>, ImageSource>,
}

#[derive(Clone, Debug)]
enum AssetBase {
  File(PathBuf),
  Url(Url),
  None,
}

fn read_render_source(input: Option<&str>) -> Result<RenderSource> {
  match input {
    Some(input) if input != "-" && is_http_url(input) => {
      let url = Url::parse(input).with_context(|| format!("invalid URL: {input}"))?;
      let html = fetch_text(url.as_str())?;
      parse_html_source(&html, AssetBase::Url(url))
    }
    Some(input) if input != "-" => {
      let path = Path::new(input);
      let (path, html_base) = resolve_input_path(path)?;
      let contents =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
      if is_html_path(&path) || looks_like_html(&contents) {
        parse_html_source(&contents, AssetBase::File(html_base))
      } else {
        parse_json_source(&contents)
      }
    }
    _ => {
      if io::stdin().is_terminal() {
        bail!("provide an input path/URL or pipe JSON/HTML to stdin");
      }
      let mut input = String::new();
      io::stdin()
        .read_to_string(&mut input)
        .context("failed to read stdin")?;
      if looks_like_html(&input) {
        parse_html_source(&input, AssetBase::None)
      } else {
        parse_json_source(&input)
      }
    }
  }
}

fn parse_json_source(json: &str) -> Result<RenderSource> {
  let node = serde_json::from_str::<Node>(json).context("failed to parse Takumi node JSON")?;
  Ok(RenderSource {
    node,
    stylesheets: Vec::new(),
    images: HashMap::new(),
  })
}

fn resolve_input_path(path: &Path) -> Result<(PathBuf, PathBuf)> {
  if path.is_dir() {
    let index = path.join("index.html");
    if !index.is_file() {
      bail!(
        "directory input must contain index.html: {}",
        path.display()
      );
    }
    return Ok((index, path.to_path_buf()));
  }

  let file = path.to_path_buf();
  let base = file
    .parent()
    .filter(|parent| !parent.as_os_str().is_empty())
    .unwrap_or_else(|| Path::new("."))
    .to_path_buf();
  Ok((file, base))
}

fn is_html_path(path: &Path) -> bool {
  matches!(
    path
      .extension()
      .and_then(|extension| extension.to_str())
      .map(str::to_ascii_lowercase)
      .as_deref(),
    Some("html" | "htm")
  )
}

fn looks_like_html(input: &str) -> bool {
  input.trim_start().starts_with('<')
}

fn is_http_url(input: &str) -> bool {
  input.starts_with("https://") || input.starts_with("http://")
}

fn load_stylesheets(cli: &Cli, mut sources: Vec<String>) -> Result<StyleSheet> {
  sources.reserve(cli.stylesheets.len() + cli.css.len());
  for path in &cli.stylesheets {
    sources.push(
      fs::read_to_string(path)
        .with_context(|| format!("failed to read stylesheet {}", path.display()))?,
    );
  }
  sources.extend(cli.css.iter().cloned());
  Ok(StyleSheet::parse_owned_list_loosy(sources))
}

fn parse_html_source(html: &str, base: AssetBase) -> Result<RenderSource> {
  let document = Html::parse_document(html);
  let mut context = HtmlContext {
    base,
    stylesheets: Vec::new(),
    images: HashMap::new(),
  };

  collect_document_styles(&document, &mut context)?;

  let body = find_first_element(&document, "body");
  let explicit_body = contains_tag(html, "body");
  let mut nodes = Vec::new();

  if explicit_body {
    if let Some(body) = body
      && let Some(node) = build_element_node(body, &mut context)?
    {
      nodes.push(node);
    }
  } else if let Some(body) = body {
    for child in body.children() {
      build_static_nodes(child, &mut context, &mut nodes)?;
    }
  } else {
    for child in document.tree.root().children() {
      build_static_nodes(child, &mut context, &mut nodes)?;
    }
  }

  let node = match nodes.len() {
    0 => Node::container([]),
    1 => nodes.remove(0),
    _ => Node::container(nodes).with_style(parse_style("width: 100%; height: 100%;")?),
  };

  Ok(RenderSource {
    node,
    stylesheets: context.stylesheets,
    images: context.images,
  })
}

struct HtmlContext {
  base: AssetBase,
  stylesheets: Vec<String>,
  images: HashMap<Arc<str>, ImageSource>,
}

type HtmlNodeRef<'a> = ego_tree::NodeRef<'a, HtmlNode>;

fn collect_document_styles(document: &Html, context: &mut HtmlContext) -> Result<()> {
  for child in document.tree.root().children() {
    collect_styles(child, context)?;
  }
  Ok(())
}

fn collect_styles(node: HtmlNodeRef<'_>, context: &mut HtmlContext) -> Result<()> {
  let HtmlNode::Element(element) = node.value() else {
    return Ok(());
  };

  match element.name() {
    "style" => {
      let css = node
        .children()
        .filter_map(|child| match child.value() {
          HtmlNode::Text(text) => Some(text.text.to_string()),
          _ => None,
        })
        .collect::<String>();
      if !css.is_empty() {
        context.stylesheets.push(css);
      }
      return Ok(());
    }
    "link" if is_stylesheet_link(element) => {
      if let Some(href) = element.attr("href") {
        context
          .stylesheets
          .push(load_text_asset(href, &context.base)?);
      }
      return Ok(());
    }
    _ => {}
  }

  for child in node.children() {
    collect_styles(child, context)?;
  }

  Ok(())
}

fn build_static_nodes(
  node: HtmlNodeRef<'_>,
  context: &mut HtmlContext,
  nodes: &mut Vec<Node>,
) -> Result<()> {
  match node.value() {
    HtmlNode::Text(text) => {
      let value = text.text.to_string();
      if !value.is_empty() {
        nodes.push(Node::text(value).with_preset(parse_style("display: inline;")?));
      }
    }
    HtmlNode::Element(_) => {
      if let Some(render_node) = build_element_node(node, context)? {
        nodes.push(render_node);
      }
    }
    _ => {}
  }
  Ok(())
}

fn build_element_node(node: HtmlNodeRef<'_>, context: &mut HtmlContext) -> Result<Option<Node>> {
  let HtmlNode::Element(element) = node.value() else {
    return Ok(None);
  };

  let tag = element.name();
  if matches!(
    tag,
    "style" | "script" | "template" | "head" | "meta" | "title" | "link"
  ) {
    return Ok(None);
  }

  if tag == "br" {
    return Ok(Some(apply_metadata(Node::text("\n"), element)?));
  }

  if tag == "img" {
    let Some(src) = element.attr("src") else {
      bail!("image element must have a src attribute");
    };
    preload_image(src, context)?;
    let data = ImageData::from((
      src.to_owned(),
      parse_dimension(element.attr("width")),
      parse_dimension(element.attr("height")),
    ));
    return Ok(Some(apply_metadata(Node::image(data), element)?));
  }

  if is_void_element(tag) {
    return Ok(None);
  }

  if tag == "svg" {
    let svg = ElementRef::wrap(node)
      .ok_or_else(|| anyhow!("failed to serialize svg element"))?
      .html();
    let data = ImageData::from((
      svg,
      parse_dimension(element.attr("width")),
      parse_dimension(element.attr("height")),
    ));
    return Ok(Some(apply_metadata(Node::image(data), element)?));
  }

  if let Some(text) = only_text_children(node)
    && !text.is_empty()
  {
    return Ok(Some(apply_metadata(Node::text(text), element)?));
  }

  let mut children = Vec::new();
  for child in node.children() {
    build_static_nodes(child, context, &mut children)?;
  }

  Ok(Some(apply_metadata(Node::container(children), element)?))
}

fn apply_metadata(mut node: Node, element: &Element) -> Result<Node> {
  let tag = element.name();
  node = node.with_tag_name(tag);

  if let Some(preset) = preset_style(tag)? {
    node = node.with_preset(preset);
  }
  if let Some(class_name) = element.attr("class") {
    node = node.with_class_name(class_name);
  }
  if let Some(id) = element.attr("id") {
    node = node.with_id(id);
  }
  if let Some(style) = element
    .attr("style")
    .filter(|style| !style.trim().is_empty())
  {
    node = node.with_style(parse_style(style)?);
  }
  if let Some(tw) = element.attr("tw").filter(|tw| !tw.trim().is_empty()) {
    node = node.with_tw(
      TailwindValues::from_str(tw).map_err(|error| anyhow!("invalid tw attribute: {error}"))?,
    );
  }
  if let Some(dir) = parse_direction(element.attr("dir")) {
    node = node.with_dir(dir);
  }
  if let Some(lang) = element.attr("lang") {
    node = node.with_lang(lang);
  }

  let attributes = element
    .attrs()
    .filter(|(name, value)| should_keep_attribute(name, value))
    .map(|(name, value)| (Box::<str>::from(name), Box::<str>::from(value)))
    .collect::<std::collections::BTreeMap<_, _>>();

  if !attributes.is_empty() {
    node = node.with_attributes(attributes);
  }

  Ok(node)
}

fn parse_style(input: &str) -> Result<Style> {
  let declarations = StyleDeclarationBlock::from_str(input)
    .with_context(|| format!("failed to parse inline style `{input}`"))?;
  Ok(Style::from(declarations))
}

fn preset_style(tag: &str) -> Result<Option<Style>> {
  let css = match tag {
    "html" => "display: block;",
    "body" => "margin: 8px; display: block;",
    "p" => "margin-top: 1em; margin-bottom: 1em; display: block;",
    "blockquote" | "figure" => {
      "margin-top: 1em; margin-bottom: 1em; margin-left: 40px; margin-right: 40px; display: block;"
    }
    "figcaption" | "article" | "aside" | "footer" | "header" | "hgroup" | "main" | "nav"
    | "section" | "div" => "display: block;",
    "address" => "font-style: italic; display: block;",
    "center" => "text-align: center; display: block;",
    "hr" => {
      "margin-top: 0.5em; margin-bottom: 0.5em; margin-left: auto; margin-right: auto; border-width: 1px; display: block;"
    }
    "h1" => {
      "font-size: 2em; margin-top: 0.67em; margin-bottom: 0.67em; margin-left: 0; margin-right: 0; font-weight: bold; display: block;"
    }
    "h2" => {
      "font-size: 1.5em; margin-top: 0.83em; margin-bottom: 0.83em; margin-left: 0; margin-right: 0; font-weight: bold; display: block;"
    }
    "h3" => {
      "font-size: 1.17em; margin-top: 1em; margin-bottom: 1em; margin-left: 0; margin-right: 0; font-weight: bold; display: block;"
    }
    "h4" => {
      "margin-top: 1.33em; margin-bottom: 1.33em; margin-left: 0; margin-right: 0; font-weight: bold; display: block;"
    }
    "h5" => {
      "font-size: 0.83em; margin-top: 1.67em; margin-bottom: 1.67em; margin-left: 0; margin-right: 0; font-weight: bold; display: block;"
    }
    "h6" => {
      "font-size: 0.67em; margin-top: 2.33em; margin-bottom: 2.33em; margin-left: 0; margin-right: 0; font-weight: bold; display: block;"
    }
    "u" => "text-decoration: underline;",
    "strong" | "b" => "font-weight: bold;",
    "i" | "em" | "cite" | "dfn" => "font-style: italic;",
    "code" | "kbd" | "samp" => "font-family: monospace;",
    "pre" => "font-family: monospace; white-space: pre; margin: 1em 0; display: block;",
    "mark" => "background-color: yellow; color: black;",
    "big" => "font-size: 1.2em;",
    "small" => "font-size: 0.8em;",
    "s" => "text-decoration: line-through;",
    _ => return Ok(None),
  };
  Ok(Some(parse_style(css)?))
}

fn parse_direction(dir: Option<&str>) -> Option<Direction> {
  match dir {
    Some("ltr") => Some(Direction::Ltr),
    Some("rtl") => Some(Direction::Rtl),
    _ => None,
  }
}

fn should_keep_attribute(name: &str, value: &str) -> bool {
  !value.is_empty()
    && !matches!(
      name,
      "class"
        | "className"
        | "id"
        | "style"
        | "tw"
        | "ref"
        | "key"
        | "dangerouslySetInnerHTML"
        | "suppressHydrationWarning"
    )
}

fn only_text_children(node: HtmlNodeRef<'_>) -> Option<String> {
  let mut text = String::new();

  for child in node.children() {
    match child.value() {
      HtmlNode::Text(value) => text.push_str(value.text.as_ref()),
      HtmlNode::Comment(_) => {}
      _ => return None,
    }
  }

  Some(text)
}

fn parse_dimension(value: Option<&str>) -> Option<f32> {
  value.and_then(|value| value.trim().parse::<f32>().ok())
}

fn find_first_element<'a>(document: &'a Html, name: &str) -> Option<HtmlNodeRef<'a>> {
  fn find_in<'a>(node: HtmlNodeRef<'a>, name: &str) -> Option<HtmlNodeRef<'a>> {
    if let HtmlNode::Element(element) = node.value()
      && element.name() == name
    {
      return Some(node);
    }

    node.children().find_map(|child| find_in(child, name))
  }

  document
    .tree
    .root()
    .children()
    .find_map(|child| find_in(child, name))
}

fn contains_tag(html: &str, tag: &str) -> bool {
  let needle = format!("<{tag}");
  html.to_ascii_lowercase().contains(&needle)
}

fn is_stylesheet_link(element: &Element) -> bool {
  element.attr("rel").is_some_and(|rel| {
    rel
      .split_ascii_whitespace()
      .any(|part| part.eq_ignore_ascii_case("stylesheet"))
  })
}

fn is_void_element(tag: &str) -> bool {
  matches!(
    tag,
    "area" | "base" | "col" | "embed" | "hr" | "input" | "param" | "source" | "track" | "wbr"
  )
}

fn preload_image(src: &str, context: &mut HtmlContext) -> Result<()> {
  if src.starts_with("data:") || src.trim_start().starts_with("<svg") {
    return Ok(());
  }
  if context.images.contains_key(src) {
    return Ok(());
  }

  let bytes = load_binary_asset(src, &context.base)?;
  let image = ImageSource::from_bytes(&bytes)
    .with_context(|| format!("failed to decode image asset `{src}`"))?;
  context.images.insert(Arc::from(src), image);
  Ok(())
}

fn load_text_asset(src: &str, base: &AssetBase) -> Result<String> {
  match resolve_asset(src, base)? {
    ResolvedAsset::File(path) => {
      fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))
    }
    ResolvedAsset::Url(url) => fetch_text(url.as_str()),
  }
}

fn load_binary_asset(src: &str, base: &AssetBase) -> Result<Vec<u8>> {
  match resolve_asset(src, base)? {
    ResolvedAsset::File(path) => {
      fs::read(&path).with_context(|| format!("failed to read {}", path.display()))
    }
    ResolvedAsset::Url(url) => fetch_bytes(url.as_str()),
  }
}

enum ResolvedAsset {
  File(PathBuf),
  Url(Url),
}

fn resolve_asset(src: &str, base: &AssetBase) -> Result<ResolvedAsset> {
  if is_http_url(src) {
    return Ok(ResolvedAsset::Url(
      Url::parse(src).with_context(|| format!("invalid asset URL: {src}"))?,
    ));
  }

  match base {
    AssetBase::File(root) => {
      let relative = local_asset_path(src).trim_start_matches('/');
      Ok(ResolvedAsset::File(root.join(relative)))
    }
    AssetBase::Url(base) => {
      Ok(ResolvedAsset::Url(base.join(src).with_context(|| {
        format!("failed to resolve `{src}` against {base}")
      })?))
    }
    AssetBase::None => bail!("cannot resolve relative asset `{src}` without an input path or URL"),
  }
}

fn local_asset_path(src: &str) -> &str {
  let query = src.find('?').unwrap_or(src.len());
  let fragment = src.find('#').unwrap_or(src.len());
  &src[..query.min(fragment)]
}

fn fetch_text(url: &str) -> Result<String> {
  ureq::get(url)
    .call()
    .with_context(|| format!("failed to fetch {url}"))?
    .into_string()
    .with_context(|| format!("failed to read response text from {url}"))
}

fn fetch_bytes(url: &str) -> Result<Vec<u8>> {
  let response = ureq::get(url)
    .call()
    .with_context(|| format!("failed to fetch {url}"))?;
  let mut bytes = Vec::new();
  response
    .into_reader()
    .read_to_end(&mut bytes)
    .with_context(|| format!("failed to read response bytes from {url}"))?;
  Ok(bytes)
}

fn load_images(args: &[ImageArg]) -> Result<HashMap<Arc<str>, ImageSource>> {
  let mut images = HashMap::with_capacity(args.len());
  for arg in args {
    let bytes = fs::read(&arg.path)
      .with_context(|| format!("failed to read image {}", arg.path.display()))?;
    let source = ImageSource::from_bytes(&bytes)
      .with_context(|| format!("failed to decode image {}", arg.path.display()))?;
    images.insert(Arc::from(arg.src.as_str()), source);
  }
  Ok(images)
}

fn parse_lang(lang: Option<&str>) -> Result<Option<Language>> {
  lang
    .map(|value| Language::parse(value).map_err(|_| anyhow!("invalid BCP-47 language: {value}")))
    .transpose()
}

fn resolve_output_format(
  requested: Option<ImageFormat>,
  output: &Path,
  quality: Option<u8>,
  lossless: bool,
) -> Result<TakumiOutputFormat> {
  let format = match requested {
    Some(format) => format,
    None => infer_format(output).ok_or_else(|| {
      anyhow!(
        "could not infer output format from {}; pass --format",
        output.display()
      )
    })?,
  };

  Ok(match format {
    ImageFormat::Png => TakumiOutputFormat::Png,
    ImageFormat::Jpeg => TakumiOutputFormat::Jpeg {
      quality: quality.map_or_else(Quality::default, Quality::new),
    },
    ImageFormat::Webp if lossless || quality.is_none() => TakumiOutputFormat::WebPLossless,
    ImageFormat::Webp => TakumiOutputFormat::WebP {
      quality: quality.map_or_else(Quality::default, Quality::new),
    },
    ImageFormat::WebpLossless => TakumiOutputFormat::WebPLossless,
    ImageFormat::Ico => TakumiOutputFormat::Ico,
  })
}

fn infer_format(path: &Path) -> Option<ImageFormat> {
  match path
    .extension()
    .and_then(|extension| extension.to_str())
    .map(str::to_ascii_lowercase)
    .as_deref()
  {
    Some("png") => Some(ImageFormat::Png),
    Some("jpg" | "jpeg") => Some(ImageFormat::Jpeg),
    Some("webp") => Some(ImageFormat::Webp),
    Some("ico") => Some(ImageFormat::Ico),
    _ => None,
  }
}

fn copy_to_clipboard(bitmap: &takumi::prelude::Bitmap) -> Result<()> {
  let mut clipboard = Clipboard::new()?;
  clipboard.set_image(ClipboardImage {
    width: bitmap.width() as usize,
    height: bitmap.height() as usize,
    bytes: Cow::Borrowed(bitmap.as_raw()),
  })?;
  Ok(())
}

fn write_kitty_image(png: &[u8]) -> Result<()> {
  let encoded = STANDARD.encode(png);
  let mut stdout = io::stdout().lock();
  let mut chunks = encoded.as_bytes().chunks(KITTY_CHUNK_SIZE).peekable();
  let mut first = true;

  while let Some(chunk) = chunks.next() {
    let more = chunks.peek().is_some();
    let m = u8::from(more);
    if first {
      write!(stdout, "\x1b_Ga=T,f=100,q=2,m={m};")?;
      first = false;
    } else {
      write!(stdout, "\x1b_Gm={m};")?;
    }
    stdout.write_all(chunk)?;
    write!(stdout, "\x1b\\")?;
  }

  writeln!(stdout)?;
  stdout.flush()?;
  Ok(())
}

fn status(cli: &Cli, label: &str, message: &str) {
  if cli.quiet {
    return;
  }
  eprintln!("\x1b[32m{label:>8}\x1b[0m {message}");
}

fn parse_image_arg(value: &str) -> Result<ImageArg, String> {
  let (src, path) = value
    .split_once('=')
    .ok_or_else(|| "expected SRC=PATH".to_owned())?;
  if src.is_empty() {
    return Err("SRC must not be empty".to_owned());
  }
  if path.is_empty() {
    return Err("PATH must not be empty".to_owned());
  }
  Ok(ImageArg {
    src: src.to_owned(),
    path: PathBuf::from(path),
  })
}
