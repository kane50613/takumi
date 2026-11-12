//! `takumi paint`: what painting the document draws, as text or JSON to read instead of an image.

use std::{error::Error, fmt::Write, sync::Arc};

use clap::ValueEnum;
use serde_json::Value;
use takumi::prelude::{Fonts, RenderNeeds};
use takumi_core::paint_tree::{
  DrawPart, Drawable, Effects, ElementInfo, NodeKind, Paint, PaintDocument, PaintNode, PaintRect,
  PaintStep, PaintTreeOptions, Rgba, Role, Shape, TextRun, UnpaintedReason, paint_tree,
  paint_tree_needs,
};

use crate::{
  document::Document,
  image::ViewportArgs,
  input::{InputArgs, OutputArgs},
  product::{Images, Product},
};

/// Describes what painting an HTML file draws: each box, text, and image, where it lands, and
/// what it is filled with.
#[derive(clap::Args)]
#[command(after_help = "Examples:
  takumi paint card.html -o -
  takumi paint card.html -o - --detail drawables
  takumi paint card.html -o card.json --format json --detail full

Each line of the text format is one node, indented under its parent:
  box div#card.panel 0,0 1200x630 background #0f172a
    text h1 64,80 820x72 \"Quarterly report\" Geist 64px #ffffff
`offscreen` marks a node outside the canvas, with the edge it is past, `empty` one without an
area, and `draws nothing` text or an image that paints no pixel, as `visibility: hidden` leaves
it. An element `opacity: 0` hides has no node, and is listed under `not painted`. An element
with `display: none` makes no box and is not listed, unless it is the root element, which is
listed under `not painted`.")]
pub(crate) struct PaintArgs {
  #[command(flatten)]
  pub(crate) input: InputArgs,

  #[command(flatten)]
  pub(crate) output: OutputArgs,

  #[command(flatten)]
  viewport: ViewportArgs,

  /// How much to describe: `nodes` for one line a node, `drawables` for each thing a node
  /// draws as well, and `full` for the paint order and, in JSON, every glyph.
  #[arg(long, value_enum, default_value_t = Detail::Nodes)]
  detail: Detail,

  /// The format to write, in place of the one the output's extension names. Text unless the
  /// output ends in `.json`.
  #[arg(long, value_enum)]
  format: Option<PaintFormat>,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum Detail {
  Nodes,
  Drawables,
  Full,
}

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum PaintFormat {
  Text,
  Json,
}

/// A paint description with its format settled.
pub(crate) struct PaintProduct {
  viewport: takumi::prelude::Viewport,
  detail: Detail,
  format: PaintFormat,
}

impl PaintArgs {
  /// The product these flags describe, its format read from `--format` or the output's extension.
  pub(crate) fn product(&self) -> PaintProduct {
    let format = self.format.unwrap_or_else(|| {
      if self.output.extension().as_deref() == Some("json") {
        PaintFormat::Json
      } else {
        PaintFormat::Text
      }
    });

    PaintProduct {
      viewport: self.viewport.device_viewport(),
      detail: self.detail,
      format,
    }
  }
}

impl PaintProduct {
  fn options<'f>(
    &self,
    document: &Document,
    fonts: &'f Fonts,
    images: Images,
  ) -> PaintTreeOptions<'f> {
    PaintTreeOptions::builder()
      .viewport(self.viewport)
      .node(document.node.clone())
      .fonts(fonts)
      .images(images)
      .stylesheet(Arc::clone(&document.stylesheet))
      .build()
  }

  /// The document as JSON, without what `self.detail` leaves out: the drawables and the paint
  /// order below `drawables`, and the glyphs below `full`.
  fn json(&self, document: &PaintDocument) -> Result<String, Box<dyn Error>> {
    fn remove(value: &mut Value, keys: &[&str]) {
      match value {
        Value::Object(fields) => {
          for key in keys {
            fields.remove(*key);
          }
          for field in fields.values_mut() {
            remove(field, keys);
          }
        }
        Value::Array(items) => {
          for item in items {
            remove(item, keys);
          }
        }
        _ => {}
      }
    }

    let mut value = serde_json::to_value(document)?;

    match self.detail {
      Detail::Nodes => remove(
        &mut value,
        &["drawables", "mask", "outline", "glyphs", "steps"],
      ),
      Detail::Drawables => remove(&mut value, &["glyphs", "steps"]),
      Detail::Full => {}
    }

    Ok(serde_json::to_string_pretty(&value)? + "\n")
  }

  /// The document as an indented outline, one node a line.
  fn text(&self, document: &PaintDocument) -> String {
    let mut outline = Outline {
      document,
      detail: self.detail,
      text: String::new(),
    };

    let _ = writeln!(
      outline.text,
      "canvas {}x{}",
      number(document.width),
      number(document.height)
    );
    if !document.nodes.is_empty() {
      outline.node(0, 0);
    }
    if !document.unpainted.is_empty() {
      outline.text.push_str("not painted\n");
    }
    for unpainted in &document.unpainted {
      let _ = writeln!(
        outline.text,
        "  {} {}",
        selector(Some(&unpainted.element)),
        match unpainted.reason {
          UnpaintedReason::OpacityZero => "opacity: 0",
          UnpaintedReason::DisplayNone => "display: none",
        }
      );
    }
    if self.detail == Detail::Full {
      outline.steps();
    }

    outline.text
  }
}

impl Product for PaintProduct {
  fn needs(
    &self,
    document: &Document,
    fonts: &Fonts,
    images: &Images,
  ) -> Result<RenderNeeds, Box<dyn Error>> {
    Ok(paint_tree_needs(self.options(
      document,
      fonts,
      images.clone(),
    )))
  }

  fn render(
    &self,
    document: Document,
    _bands: Vec<Document>,
    fonts: &Fonts,
    images: Images,
  ) -> Result<Vec<u8>, Box<dyn Error>> {
    let tree = paint_tree(self.options(&document, fonts, images))?;
    let described = match self.format {
      PaintFormat::Json => self.json(&tree.document)?,
      PaintFormat::Text => self.text(&tree.document),
    };

    Ok(described.into_bytes())
  }

  fn is_text(&self) -> bool {
    true
  }
}

/// A length to two decimals, without trailing zeros.
fn number(value: f32) -> String {
  let rounded = (value * 100.0).round() / 100.0;

  if rounded == 0.0 {
    "0".into()
  } else {
    rounded.to_string()
  }
}

fn rect(rect: &PaintRect) -> String {
  format!(
    "{},{} {}x{}",
    number(rect.x),
    number(rect.y),
    number(rect.width),
    number(rect.height)
  )
}

/// `#rrggbb`, with the alpha as two more digits when the colour is not opaque.
fn color(&[red, green, blue, alpha]: &Rgba) -> String {
  if alpha == u8::MAX {
    format!("#{red:02x}{green:02x}{blue:02x}")
  } else {
    format!("#{red:02x}{green:02x}{blue:02x}{alpha:02x}")
  }
}

fn paint(paint: &Paint) -> String {
  match paint {
    Paint::Color { color: rgba } => color(rgba),
    Paint::LinearGradient { stops, .. } => format!("linear-gradient({} stops)", stops.len()),
    Paint::RadialGradient { stops, .. } => format!("radial-gradient({} stops)", stops.len()),
    Paint::ConicGradient { stops, .. } => format!("conic-gradient({} stops)", stops.len()),
    Paint::Image { image, .. } => format!("image {}", source(&image.src)),
    Paint::Pattern { tile, .. } => format!("pattern of {}", self::paint(tile)),
  }
}

fn shape(shape: &Shape) -> String {
  match shape {
    Shape::Rect { rect: bounds } => format!("rect {}", rect(bounds)),
    Shape::RoundedRect { rect: bounds, .. } => format!("rounded-rect {}", rect(bounds)),
    Shape::Path { .. } => "path".into(),
  }
}

/// An image's `src`, with the data of a `data:` URL left out.
fn source(src: &str) -> String {
  match src.split_once(',') {
    Some((header, data)) if header.starts_with("data:") => {
      format!("{header},… ({} bytes)", data.len())
    }
    _ => src.to_owned(),
  }
}

/// A selector that names `element` as its author would: the tag, `#id`, and each `.class`.
fn selector(element: Option<&ElementInfo>) -> String {
  let Some(element) = element else {
    return "(anonymous)".into();
  };
  let mut selector = element.tag_name.clone().unwrap_or_default();

  if let Some(id) = &element.id {
    let _ = write!(selector, "#{id}");
  }
  for class in element
    .class_name
    .iter()
    .flat_map(|classes| classes.split_whitespace())
  {
    let _ = write!(selector, ".{class}");
  }

  if selector.is_empty() {
    format!("(node {:?})", element.path)
  } else {
    selector
  }
}

fn kind(node: &PaintNode) -> &'static str {
  match &node.kind {
    NodeKind::Box { .. } => "box",
    NodeKind::Text { .. } => "text",
    NodeKind::Image { .. } => "image",
  }
}

fn role(role: Role) -> &'static str {
  match role {
    Role::Background => "background",
    Role::Border => "border",
    Role::BoxShadow => "box-shadow",
    Role::Outline => "outline",
    Role::Image => "image",
    Role::Text => "text",
    Role::TextShadow => "text-shadow",
    Role::TextStroke => "text-stroke",
    Role::TextDecoration => "text-decoration",
    Role::InlineBackground => "inline-background",
  }
}

/// The outline being written.
struct Outline<'d> {
  document: &'d PaintDocument,
  detail: Detail,
  text: String,
}

impl Outline<'_> {
  fn indent(&mut self, depth: usize) {
    for _ in 0..depth {
      self.text.push_str("  ");
    }
  }

  fn node(&mut self, index: usize, depth: usize) {
    let Some(node) = self.document.nodes.get(index) else {
      return;
    };

    self.indent(depth);

    let _ = write!(
      self.text,
      "{} {} {}",
      kind(node),
      selector(node.element.as_ref()),
      rect(&node.bounds)
    );

    self.summary(node);
    self.text.push('\n');

    if self.detail >= Detail::Drawables {
      self.drawables(node, depth + 1);
    }
    for &child in &node.children {
      self.node(child, depth + 1);
    }
  }

  /// What a node line says after the node's place: its text, image, or fills, how it composites,
  /// and whether it can show at all.
  fn summary(&mut self, node: &PaintNode) {
    match &node.kind {
      NodeKind::Text { runs, .. } => {
        // A node that draws no run at all still names its text, marked `draws nothing`.
        let draws_any = (0..runs.len()).any(|index| draws_run(&node.drawables, index));
        let content: String = runs
          .iter()
          .enumerate()
          .filter(|(index, _)| !draws_any || draws_run(&node.drawables, *index))
          .map(|(_, run)| run.text.as_str())
          .collect();
        let _ = write!(self.text, " {content:?}");

        if let Some(run) = runs
          .iter()
          .enumerate()
          .find(|(index, _)| !draws_any || draws_run(&node.drawables, *index))
          .map(|(_, run)| run)
        {
          let _ = write!(self.text, " {}", self.font(run));
        }
        if let Some(fill) = first_paint(&node.drawables, Role::Text) {
          let _ = write!(self.text, " {fill}");
        }
      }
      NodeKind::Image { image } => {
        let _ = write!(
          self.text,
          " src {} natural {}x{}",
          source(&image.src),
          number(image.width),
          number(image.height)
        );
      }
      NodeKind::Box {
        effects,
        overflow_clip,
        ..
      } => {
        if let Some(fill) = first_paint(&node.drawables, Role::Background) {
          let _ = write!(self.text, " background {fill}");
        }
        if has_role(&node.drawables, Role::Border) {
          self.text.push_str(" border");
        }
        if has_role(&node.drawables, Role::BoxShadow) {
          self.text.push_str(" box-shadow");
        }
        if overflow_clip.is_some() {
          self.text.push_str(" clips-overflow");
        }
        if let Some(effects) = effects {
          self.effects(effects);
        }
      }
    }

    let [a, b, c, d, ..] = node.transform;

    if [a, b, c, d] != [1.0, 0.0, 0.0, 1.0] {
      let _ = write!(
        self.text,
        " transform matrix({})",
        node.transform.map(number).join(",")
      );
    }

    let bounds = &node.bounds;
    let side = if bounds.x >= self.document.width {
      Some("right")
    } else if bounds.x + bounds.width <= 0.0 {
      Some("left")
    } else if bounds.y >= self.document.height {
      Some("bottom")
    } else if bounds.y + bounds.height <= 0.0 {
      Some("top")
    } else {
      None
    };

    if bounds.width <= 0.0 || bounds.height <= 0.0 {
      self.text.push_str(" empty");
    } else if let Some(side) = side {
      let _ = write!(self.text, " offscreen past the {side} edge");
    } else if node.drawables.is_empty() && !matches!(node.kind, NodeKind::Box { .. }) {
      self.text.push_str(" draws nothing");
    }
  }

  fn effects(&mut self, effects: &Effects) {
    if effects.opacity < 1.0 {
      let _ = write!(self.text, " opacity {}", number(effects.opacity));
    }
    if effects.blend_mode != "normal" {
      let _ = write!(self.text, " blend {}", effects.blend_mode);
    }
    if !effects.filters.is_empty() {
      let _ = write!(self.text, " filters {}", effects.filters.len());
    }
    if !effects.backdrop_filters.is_empty() {
      let _ = write!(
        self.text,
        " backdrop-filters {}",
        effects.backdrop_filters.len()
      );
    }
    if effects.clip.is_some() {
      self.text.push_str(" clipped");
    }
    if effects.mask.is_some() {
      self.text.push_str(" masked");
    }
  }

  /// A run's font as `family size`, with its weight when it is not regular.
  fn font(&self, run: &TextRun) -> String {
    let font = self.document.fonts.get(run.font);
    let family = font
      .and_then(|font| font.family.as_deref())
      .unwrap_or("(unnamed font)");
    let weight = font
      .map(|font| font.weight)
      .filter(|&weight| weight != 400.0)
      .map_or_else(String::new, |weight| format!(" {}", number(weight)));

    format!("{family}{weight} {}px", number(run.font_size))
  }

  fn drawables(&mut self, node: &PaintNode, depth: usize) {
    for drawable in &node.drawables {
      self.drawable(node, drawable, depth);
    }
    if let NodeKind::Box { outline, .. } = &node.kind {
      for drawable in outline {
        self.drawable(node, drawable, depth);
      }
    }
  }

  fn drawable(&mut self, node: &PaintNode, drawable: &Drawable, depth: usize) {
    self.indent(depth);

    match drawable {
      Drawable::Fill {
        role: of,
        shape: filled,
        paint: fill,
        clips,
        ..
      } => {
        let _ = write!(
          self.text,
          "- fill {} {} {}",
          role(*of),
          shape(filled),
          paint(fill)
        );

        if !clips.is_empty() {
          let _ = write!(self.text, " clipped by {}", clips.len());
        }
      }
      Drawable::Stroke {
        role: of,
        shape: stroked,
        paint: fill,
        ..
      } => {
        let _ = write!(
          self.text,
          "- stroke {} {} {}",
          role(*of),
          shape(stroked),
          paint(fill)
        );
      }
      Drawable::Shadow {
        role: of,
        offset,
        blur,
        color: rgba,
        ..
      } => {
        let _ = write!(
          self.text,
          "- shadow {} offset {},{} blur {} {}",
          role(*of),
          number(offset.x),
          number(offset.y),
          number(*blur),
          color(rgba)
        );
      }
      Drawable::Glyphs {
        role: of,
        run,
        paint: fill,
        ..
      } => {
        let _ = write!(self.text, "- glyphs {} {}", role(*of), paint(fill));

        if let NodeKind::Text { runs, .. } = &node.kind
          && let Some(run) = runs.get(*run)
        {
          let _ = write!(
            self.text,
            " {:?} at {},{} line {} {}",
            run.text,
            number(run.x),
            number(run.y),
            run.line,
            self.font(run)
          );
        }
      }
      Drawable::Image {
        role: of,
        image,
        rect: bounds,
        ..
      } => {
        let _ = write!(
          self.text,
          "- image {} {} {}",
          role(*of),
          rect(bounds),
          source(&image.src)
        );
      }
      Drawable::Masked {
        role: of,
        mask,
        content,
      } => {
        let _ = writeln!(self.text, "- masked {}", role(*of));

        for drawable in content {
          self.drawable(node, drawable, depth + 1);
        }
        self.indent(depth + 1);
        self.text.push_str("- mask\n");
        for drawable in mask {
          self.drawable(node, drawable, depth + 2);
        }
        return;
      }
      Drawable::Group { opacity, drawables } => {
        let _ = writeln!(self.text, "- group opacity {}", number(*opacity));

        for drawable in drawables {
          self.drawable(node, drawable, depth + 1);
        }
        return;
      }
    }

    self.text.push('\n');
  }

  /// The paint order: each step a renderer takes, bottom to top.
  fn steps(&mut self) {
    self.text.push_str("paint order\n");

    for (index, step) in self.document.steps.iter().enumerate() {
      let (action, node) = match step {
        PaintStep::Draw {
          node,
          part: DrawPart::Drawables,
        } => ("draw", node),
        PaintStep::Draw {
          node,
          part: DrawPart::Outline,
        } => ("draw outline", node),
        PaintStep::BeginGroup { node } => ("begin group", node),
        PaintStep::EndGroup { node } => ("end group", node),
        PaintStep::BeginClip { node } => ("begin clip", node),
        PaintStep::EndClip { node } => ("end clip", node),
      };
      let Some(node) = self.document.nodes.get(*node) else {
        continue;
      };
      let _ = writeln!(
        self.text,
        "  {index} {action} {} {}",
        kind(node),
        selector(node.element.as_ref())
      );
    }
  }
}

fn has_role(drawables: &[Drawable], role: Role) -> bool {
  drawables
    .iter()
    .any(|drawable| drawable_role(drawable) == Some(role))
}

fn drawable_role(drawable: &Drawable) -> Option<Role> {
  match drawable {
    Drawable::Fill { role, .. }
    | Drawable::Stroke { role, .. }
    | Drawable::Shadow { role, .. }
    | Drawable::Glyphs { role, .. }
    | Drawable::Image { role, .. }
    | Drawable::Masked { role, .. } => Some(*role),
    Drawable::Group { .. } => None,
  }
}

/// Whether any of `drawables` draws the glyphs of the text run at `index`.
fn draws_run(drawables: &[Drawable], index: usize) -> bool {
  drawables.iter().any(|drawable| match drawable {
    Drawable::Glyphs { run, .. } => *run == index,
    Drawable::Group { drawables, .. }
    | Drawable::Masked {
      content: drawables, ..
    } => draws_run(drawables, index),
    _ => false,
  })
}

/// What the first drawable of `role` is filled with.
fn first_paint(drawables: &[Drawable], role: Role) -> Option<String> {
  drawables.iter().find_map(|drawable| match drawable {
    Drawable::Fill {
      role: of,
      paint: fill,
      ..
    }
    | Drawable::Glyphs {
      role: of,
      paint: fill,
      ..
    } if *of == role => Some(paint(fill)),
    Drawable::Group { drawables, .. }
    | Drawable::Masked {
      content: drawables, ..
    } => first_paint(drawables, role),
    _ => None,
  })
}
