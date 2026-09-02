//! PDF annotations, allowing you to add extra "content" to specific pages.
//!
//! PDF has the concept of annotations, which allow you to associate certain regions of
//! a page with an "annotation". The PDF reference defines many different actions, however,
//! krilla does not and never will expose all of them. As of right now, the only annotations
//! that are supported are "link annotations", which allow you associate a certain region of
//! the page with a link.

use core::f32;

use pdf_writer::types::AnnotationFlags;
use pdf_writer::{Finish, Name, Rect as PdfRect, Ref, Str, TextStr};

use crate::krilla::chunk_container::ChunkContainer;
use crate::krilla::color::Color;
use crate::krilla::configure::{PdfVersion, ValidationError};
use crate::krilla::error::KrillaResult;
use crate::krilla::geom::{Quadrilateral, Rect};
use crate::krilla::interactive::action::Action;
use crate::krilla::interactive::destination::Destination;
use crate::krilla::page::page_root_transform;
use crate::krilla::serialize::SerializeContext;
use crate::krilla::surface::Location;

/// An annotation.
pub struct Annotation {
  pub(crate) annotation_type: AnnotationType,
  pub(crate) alt: Option<String>,
  pub(crate) struct_parent: Option<i32>,
  pub(crate) location: Option<Location>,
}

impl Annotation {
  /// Create a new link annotation with some alt text.
  ///
  /// Note that the alt text might be required in some cases, for example
  /// when exporting to PDF/UA.
  pub fn new_link(annotation: LinkAnnotation, alt_text: Option<String>) -> Self {
    Self {
      annotation_type: AnnotationType::Link(annotation),
      alt: alt_text,
      struct_parent: None,
      location: None,
    }
  }

  /// Create a new form widget annotation.
  pub fn new_widget(annotation: WidgetAnnotation, alt_text: Option<String>) -> Self {
    Self {
      annotation_type: AnnotationType::Widget(annotation),
      alt: alt_text,
      struct_parent: None,
      location: None,
    }
  }

  /// Sets the location of the annotation.
  pub fn with_location(mut self, location: Option<Location>) -> Self {
    self.location = location;
    self
  }
}

impl From<LinkAnnotation> for Annotation {
  fn from(value: LinkAnnotation) -> Self {
    Self {
      annotation_type: AnnotationType::Link(value),
      alt: None,
      struct_parent: None,
      location: None,
    }
  }
}

impl Annotation {
  pub(crate) fn serialize(
    &self,
    sc: &mut SerializeContext,
    chunk_container: &mut ChunkContainer,
    root_ref: Ref,
    page_height: f32,
    page_ref: Ref,
  ) -> KrillaResult<()> {
    let mut parent = None;
    let appearance = match &self.annotation_type {
      AnnotationType::Widget(widget) => {
        match &widget.field {
          // A radio group is one field owning every button as a kid, so the
          // buttons themselves carry no field name.
          FormField::Radio { index, export, on } => {
            parent = Some(chunk_container.radio_group(sc, widget, *index, export, *on, root_ref));
          }
          _ => chunk_container.form_fields.push(root_ref),
        }
        Some(widget.write_appearance(sc, chunk_container))
      }
      AnnotationType::Link(_) => None,
    };
    let chunk = &mut chunk_container.non_stream.annotations;
    let mut annotation = chunk
      .indirect(root_ref)
      .start::<pdf_writer::writers::Annotation>();

    self
      .annotation_type
      .serialize_type(sc, &mut annotation, page_height, parent, page_ref)?;

    if let Some(appearance) = appearance {
      appearance.serialize(&mut annotation);
    }

    let always_print = match &self.annotation_type {
      AnnotationType::Link(l) => l.border.is_none(),
      // A field a reader fills in has to come out of the printer.
      AnnotationType::Widget(_) => true,
    };
    // Only set the print flag when really necessary (only PDF/A). Don't
    // set it by default, so annotations with color borders will be shown
    // on a screen but not printed.
    // TODO: No need to write the print flag even if it is `None`,
    // only for PDF/A.
    if always_print
      || sc
        .serialize_settings()
        .configuration
        .validators()
        .requires_annotation_flags()
    {
      annotation.flags(AnnotationFlags::PRINT);
    }

    if let Some(struct_parent) = self.struct_parent {
      annotation.struct_parent(struct_parent);
    }

    if let Some(alt_text) = &self.alt {
      annotation.contents(TextStr(alt_text));
    }

    if self.alt.as_ref().is_none_or(String::is_empty) {
      sc.register_validation_error(ValidationError::MissingAnnotationAltText(self.location));
    }

    annotation.finish();

    Ok(())
  }
}

/// A type of annotation.
pub enum AnnotationType {
  /// A link annotation.
  Link(LinkAnnotation),
  /// A form field widget annotation.
  Widget(WidgetAnnotation),
}

impl AnnotationType {
  fn serialize_type(
    &self,
    sc: &mut SerializeContext,
    annotation: &mut pdf_writer::writers::Annotation,
    page_height: f32,
    parent: Option<Ref>,
    page_ref: Ref,
  ) -> KrillaResult<()> {
    match self {
      AnnotationType::Link(l) => l.serialize_type(sc, annotation, page_height),
      AnnotationType::Widget(w) => {
        w.serialize_type(annotation, page_height, parent, page_ref);
        Ok(())
      }
    }
  }
}

/// An annotation target.
pub enum Target {
  /// A destination within the document.
  Destination(Destination),
  /// An action to be performed.
  Action(Action),
}

/// Border of a link annotation.
pub struct LinkBorder {
  pub(crate) width: f32,
  pub(crate) color: Color,
}

impl LinkBorder {
  /// Create a new link annotation border.
  ///
  /// `width`: The width of the border in pt.
  /// `color`: The color of the border.
  pub fn new(width: f32, color: Color) -> Self {
    Self { width, color }
  }
}

/// A link annotation.
pub struct LinkAnnotation {
  pub(crate) rect: Rect,
  pub(crate) quad_points: Option<Vec<Quadrilateral>>,
  pub(crate) target: Target,
  pub(crate) border: Option<LinkBorder>,
}

impl LinkAnnotation {
  /// Create a new link annotation.
  ///
  /// `rect`: The bounding box of the link annotation that it should cover on the page.
  /// `target`: The target of the link annotation.
  pub fn new(rect: Rect, target: Target) -> Self {
    Self {
      rect,
      quad_points: None,
      target,
      border: None,
    }
  }

  /// Create a new link annotation.
  ///
  /// `target`: The target of the link annotation.
  /// `quad_points`: An array of quadrilaterals that define where the link
  /// annotation should be activated. This is useful if you for example have
  /// a link annotation that is broken to one or multiple lines.
  pub fn new_with_quad_points(quad_points: Vec<Quadrilateral>, target: Target) -> Self {
    assert!(!quad_points.is_empty());

    let mut min_x = f32::INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut max_y = f32::NEG_INFINITY;

    for point in quad_points.iter().flat_map(|q| q.0) {
      min_x = min_x.min(point.x);
      min_y = min_y.min(point.y);
      max_x = max_x.max(point.x);
      max_y = max_y.max(point.y);
    }

    // Expand the bounding box by a little. There is a bug in adobe acrobat
    // that sometimes prevents the quadpoints from being used if the quad
    // points lie exactly on the bounding rectangle.
    const EPSILON: f32 = 0.001;
    let rect = Rect::from_ltrb(
      min_x - EPSILON,
      min_y - EPSILON,
      max_x + EPSILON,
      max_y + EPSILON,
    )
    .unwrap();

    Self {
      rect,
      quad_points: Some(quad_points),
      target,
      border: None,
    }
  }

  /// Set a border for this link annotation. The border will be visible on
  /// screen but not when printed, unless when exporting with PDF/A standard.
  pub fn with_border(self, border: LinkBorder) -> Self {
    Self {
      border: Some(border),
      ..self
    }
  }

  fn serialize_type(
    &self,
    sc: &mut SerializeContext,
    annotation: &mut pdf_writer::writers::Annotation,
    page_height: f32,
  ) -> KrillaResult<()> {
    annotation.subtype(pdf_writer::types::AnnotationType::Link);

    let actual_rect = self
      .rect
      .transform(page_root_transform(page_height))
      .unwrap();
    annotation.rect(actual_rect.to_pdf_rect());
    annotation.border(
      0.0,
      0.0,
      self.border.as_ref().map_or(0.0, |x| x.width),
      None,
    );

    if let Some(border) = &self.border {
      match border.color.to_regular() {
        crate::krilla::color::RegularColor::Rgb(rgb) => {
          let [r, g, b] = rgb.to_pdf_color();
          annotation.color_rgb(r, g, b);
        }
        crate::krilla::color::RegularColor::Cmyk(cmyk) => {
          let [c, m, y, k] = cmyk.to_pdf_color();
          annotation.color_cmyk(c, m, y, k);
        }
        crate::krilla::color::RegularColor::Luma(gray) => {
          annotation.color_gray(gray.to_pdf_color());
        }
      }
    }

    if sc.serialize_settings().pdf_version() >= PdfVersion::Pdf16 {
      self.quad_points.as_ref().map(|p| {
        annotation.quad_points(p.iter().flat_map(|q| q.0).flat_map(|p| {
          let mut p = p.to_tsp();
          page_root_transform(page_height).to_tsp().map_point(&mut p);
          [p.x, p.y]
        }))
      });
    }

    match &self.target {
      Target::Destination(destination) => {
        destination.serialize(sc, annotation.insert(Name(b"Dest")))
      }
      Target::Action(action) => action.serialize(sc, annotation.action()),
    }
  }
}

/// The appearance streams of one widget: a single stream for a text or choice
/// field, or one per state for a check box and radio button.
pub(crate) struct WidgetAppearance {
  on: Ref,
  off: Option<Ref>,
  on_state: String,
}

impl WidgetAppearance {
  fn serialize(&self, annotation: &mut pdf_writer::writers::Annotation) {
    let mut appearance = annotation.appearance();

    match self.off {
      Some(off) => {
        let mut states = appearance.normal().streams();

        states.pair(Name(self.on_state.as_bytes()), self.on);
        states.pair(Name(b"Off"), off);
        states.finish();
      }
      None => appearance.normal().stream(self.on),
    }
    appearance.finish();
  }
}

/// How a widget's own CSS paints it, mirrored into the field so a viewer
/// regenerating the appearance lands somewhere close to the rendered box.
pub struct WidgetStyle {
  /// Text color.
  pub color: [f32; 3],
  /// Text size in points.
  pub font_size: f32,
  /// `/MK /BG`, from `background-color`.
  pub background: Option<[f32; 3]>,
  /// `/MK /BC`, from `border-color`.
  pub border: Option<([f32; 3], f32)>,
  /// `/Q`: 0 left, 1 center, 2 right.
  pub align: i32,
  /// `/Ff` ReadOnly.
  pub read_only: bool,
  /// `/Ff` Required.
  pub required: bool,
}

/// The kind of form field a widget annotation carries.
pub enum FormField {
  /// A text field with its current value.
  Text {
    /// The field's value.
    value: String,
    /// Whether the field wraps onto more than one line.
    multiline: bool,
    /// Whether the field hides what is typed into it.
    password: bool,
    /// `/MaxLen`.
    max_len: Option<i32>,
  },
  /// A check box.
  CheckBox {
    /// Whether the box is ticked.
    on: bool,
    /// The `/AP` state name the ticked box carries, which is also what a
    /// submitted form sends.
    export: String,
  },
  /// One button of a radio group.
  Radio {
    /// This button's place in the group, which names its `/AP` state. The
    /// readable value lives in the group's `/Opt`, so a value carrying
    /// characters a PDF name cannot hold survives.
    index: usize,
    /// What this button submits, written to the group's `/Opt`.
    export: String,
    /// Whether this button is the selected one.
    on: bool,
  },
  /// A drop-down or list box.
  Choice {
    /// The selected option's display text.
    value: String,
    /// Every option as its display text and the value it submits.
    options: Vec<(String, Option<String>)>,
    /// Whether more than one option can be selected, which also makes the
    /// field a list box rather than a drop-down.
    multi: bool,
  },
}

impl FormField {
  pub(crate) fn flags(&self, style: &WidgetStyle) -> i32 {
    let mut flags = 0;

    if style.read_only {
      flags |= 1;
    }
    if style.required {
      flags |= 1 << 1;
    }
    match self {
      FormField::Text {
        multiline,
        password,
        ..
      } => {
        if *multiline {
          flags |= 1 << 12;
        }
        if *password {
          flags |= 1 << 13;
        }
      }
      FormField::Radio { .. } => flags |= (1 << 14) | (1 << 15),
      FormField::Choice { multi, .. } => match multi {
        true => flags |= 1 << 21,
        false => flags |= 1 << 17,
      },
      FormField::CheckBox { .. } => {}
    }

    flags
  }

  /// The `/AP` state name the field files its "on" appearance under. A radio
  /// button uses its index, so a value a PDF name cannot carry still round
  /// trips through the group's `/Opt`.
  pub(crate) fn on_state(&self) -> String {
    match self {
      FormField::CheckBox { export, .. } => escape_name(export),
      FormField::Radio { index, .. } => index.to_string(),
      FormField::Text { .. } | FormField::Choice { .. } => String::new(),
    }
  }
}

/// A form field widget annotation, merged with its field dictionary unless it
/// belongs to a radio group, which owns the field and holds this as a kid.
pub struct WidgetAnnotation {
  pub(crate) rect: Rect,
  pub(crate) name: String,
  pub(crate) field: FormField,
  pub(crate) style: WidgetStyle,
  /// `/TU`, the name a reader announces the field by.
  pub(crate) description: Option<String>,
  /// `/Lang`, which PDF/UA requires on an annotation carrying text.
  pub(crate) lang: Option<String>,
}

/// Escapes the characters a PDF literal string gives meaning to.
fn escape_literal(value: &str) -> String {
  let mut escaped = String::with_capacity(value.len());

  for character in value.chars() {
    if matches!(character, '(' | ')' | '\\') {
      escaped.push('\\');
    }
    escaped.push(character);
  }

  escaped
}

impl WidgetAnnotation {
  /// Create a new widget annotation for the field named `name`.
  pub fn new(rect: Rect, name: String, field: FormField, style: WidgetStyle) -> Self {
    Self {
      rect,
      name,
      field,
      style,
      description: None,
      lang: None,
    }
  }

  /// Sets `/TU`, which a screen reader announces in place of the field name.
  pub fn with_description(mut self, description: Option<String>) -> Self {
    self.description = description;
    self
  }

  /// Sets the field's natural language.
  pub fn with_lang(mut self, lang: Option<String>) -> Self {
    self.lang = lang;
    self
  }

  /// The `/DA` default appearance string, which a viewer reuses when it
  /// regenerates the field after an edit.
  fn default_appearance(&self) -> String {
    let [red, green, blue] = self.style.color;

    format!("/Helv {} Tf {red} {green} {blue} rg", self.style.font_size)
  }

  /// Writes this widget's appearance streams, and the shared base-14 font the
  /// text ones draw with.
  fn write_appearance(
    &self,
    sc: &mut SerializeContext,
    chunk_container: &mut ChunkContainer,
  ) -> WidgetAppearance {
    let width = self.rect.right() - self.rect.left();
    let height = self.rect.bottom() - self.rect.top();
    let bbox = PdfRect::new(0.0, 0.0, width, height);
    let size = self.style.font_size;

    let on_state = self.field.on_state();
    let (on_content, off, draws_text) = match &self.field {
      FormField::Text {
        value, multiline, ..
      } => (
        text_appearance(value, *multiline, width, height, size, &self.style),
        None,
        !value.is_empty(),
      ),
      FormField::Choice { value, .. } => (
        text_appearance(value, false, width, height, size, &self.style),
        None,
        !value.is_empty(),
      ),
      FormField::CheckBox { .. } => (
        check_mark(width, height, self.style.color),
        Some(sc.new_ref()),
        false,
      ),
      FormField::Radio { .. } => (
        dot(width, height, self.style.color),
        Some(sc.new_ref()),
        false,
      ),
    };
    // The font reference is taken before the stream opens, which borrows the
    // same chunk.
    let font = draws_text.then(|| chunk_container.form_font(sc));

    if let Some(off) = off {
      chunk_container
        .non_stream
        .annotations
        .form_xobject(off, b"")
        .bbox(bbox)
        .finish();
    }

    let on = sc.new_ref();
    let mut stream = chunk_container
      .non_stream
      .annotations
      .form_xobject(on, on_content.as_bytes());

    stream.bbox(bbox);

    // A blank field draws no text, so it needs no font, and a document of
    // blank fields carries no unembedded one for a validator to reject.
    if let Some(font) = font {
      stream.resources().fonts().pair(Name(b"Helv"), font);
    }
    stream.finish();

    WidgetAppearance { on, off, on_state }
  }

  /// Writes the `/MK` appearance characteristics a viewer redraws the field
  /// from, and the border width they are stroked with.
  fn serialize_appearance_characteristics(&self, annotation: &mut pdf_writer::writers::Annotation) {
    let mut characteristics = annotation.appearance_characteristics();

    if let Some([red, green, blue]) = self.style.background {
      characteristics.background_color_rgb(red, green, blue);
    }
    if let Some(([red, green, blue], _)) = self.style.border {
      characteristics.border_color_rgb(red, green, blue);
    }
    characteristics.finish();

    let width = self.style.border.map(|(_, width)| width).unwrap_or(0.0);

    annotation.border(0.0, 0.0, width, None);
  }

  fn serialize_type(
    &self,
    annotation: &mut pdf_writer::writers::Annotation,
    page_height: f32,
    parent: Option<Ref>,
    page_ref: Ref,
  ) {
    annotation.subtype(pdf_writer::types::AnnotationType::Widget);
    // A reader looking for a field's page should not have to scan every
    // page's `/Annots` to find it.
    annotation.pair(Name(b"P"), page_ref);

    let actual_rect = self
      .rect
      .transform(page_root_transform(page_height))
      .unwrap();

    annotation.rect(actual_rect.to_pdf_rect());
    self.serialize_appearance_characteristics(annotation);

    if let Some(lang) = &self.lang {
      annotation.pair(Name(b"Lang"), TextStr(lang));
    }

    if let Some(parent) = parent {
      annotation.pair(Name(b"Parent"), parent);
    } else {
      annotation.pair(Name(b"T"), TextStr(&self.name));
      annotation.pair(Name(b"Ff"), self.field.flags(&self.style));

      if let Some(description) = &self.description {
        annotation.pair(Name(b"TU"), TextStr(description));
      }
    }

    match &self.field {
      FormField::Text { value, max_len, .. } => {
        annotation.pair(Name(b"FT"), Name(b"Tx"));
        annotation.pair(Name(b"DA"), Str(self.default_appearance().as_bytes()));
        annotation.pair(Name(b"Q"), self.style.align);
        annotation.pair(Name(b"V"), TextStr(value));
        // HTML's `value` is what the field resets to, which is also where it
        // starts.
        annotation.pair(Name(b"DV"), TextStr(value));

        if let Some(max_len) = max_len {
          annotation.pair(Name(b"MaxLen"), *max_len);
        }
      }
      FormField::Choice { value, options, .. } => {
        annotation.pair(Name(b"FT"), Name(b"Ch"));
        annotation.pair(Name(b"DA"), Str(self.default_appearance().as_bytes()));
        annotation.pair(Name(b"V"), TextStr(value));
        annotation.pair(Name(b"DV"), TextStr(value));

        let mut opt = annotation.insert(Name(b"Opt")).array();

        for (display, export) in options {
          match export {
            // An option whose submitted value differs from its label writes
            // both, export first, as the spec pairs them.
            Some(export) => {
              opt
                .push()
                .array()
                .items([TextStr(export), TextStr(display)]);
            }
            None => {
              opt.item(TextStr(display));
            }
          }
        }
        opt.finish();
      }
      FormField::CheckBox { on, .. } => {
        let on_state = self.field.on_state();
        let state = match on {
          true => Name(on_state.as_bytes()),
          false => Name(b"Off"),
        };

        annotation.pair(Name(b"FT"), Name(b"Btn"));
        annotation.pair(Name(b"V"), state);
        annotation.pair(Name(b"DV"), state);
        annotation.pair(Name(b"AS"), state);
      }
      FormField::Radio { on, .. } => {
        let on_state = self.field.on_state();
        let state = match on {
          true => Name(on_state.as_bytes()),
          false => Name(b"Off"),
        };

        annotation.pair(Name(b"AS"), state);
      }
    }
  }
}

/// The whole content stream a text or choice field's appearance draws.
fn text_appearance(
  value: &str,
  multiline: bool,
  width: f32,
  height: f32,
  size: f32,
  style: &WidgetStyle,
) -> String {
  if value.is_empty() {
    return String::new();
  }
  let [red, green, blue] = style.color;
  let text = draw_value(value, multiline, width, height, size, style.align);

  format!(
    "/Tx BMC q 0 0 {width} {height} re W n \
     BT /Helv {size} Tf {red} {green} {blue} rg {text} ET Q EMC"
  )
}

/// Escapes the characters a PDF name cannot carry, as `#` and two hex digits.
fn escape_name(value: &str) -> String {
  let mut escaped = String::with_capacity(value.len());

  for byte in value.bytes() {
    match byte {
      b'!'..=b'~'
        if !matches!(
          byte,
          b'#' | b'/' | b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'{' | b'}' | b'%'
        ) =>
      {
        escaped.push(byte as char);
      }
      _ => escaped.push_str(&format!("#{byte:02X}")),
    }
  }

  escaped
}

/// Helvetica's advance widths for the printable ASCII range, in thousandths
/// of an em, from the face's own metrics. The appearance streams draw with
/// this face, so their alignment measures with its widths.
const HELVETICA_WIDTHS: [u16; 95] = [
  278, 278, 355, 556, 556, 889, 667, 191, 333, 333, 389, 584, 278, 333, 278, 278, 556, 556, 556,
  556, 556, 556, 556, 556, 556, 556, 278, 278, 584, 584, 584, 556, 1015, 667, 667, 722, 722, 667,
  611, 778, 722, 278, 500, 667, 556, 833, 722, 778, 667, 778, 722, 667, 611, 722, 667, 944, 667,
  667, 611, 278, 278, 278, 469, 556, 333, 556, 556, 500, 556, 556, 278, 556, 556, 222, 222, 500,
  222, 833, 556, 556, 556, 556, 333, 500, 278, 556, 500, 722, 500, 500, 500, 334, 260, 334, 584,
];

/// The width the appearance's face draws this text at.
fn text_width(text: &str, size: f32) -> f32 {
  let thousandths: u32 = text
    .chars()
    .map(|character| match u32::from(character).checked_sub(32) {
      Some(index) => u32::from(
        HELVETICA_WIDTHS
          .get(index as usize)
          .copied()
          // A character outside the range cannot be drawn by this face
          // either; the average advance keeps the estimate honest.
          .unwrap_or(500),
      ),
      None => 0,
    })
    .sum();

  thousandths as f32 / 1000.0 * size
}

/// A `Tj` run per line, laid out inside a widget of this size.
fn draw_value(
  value: &str,
  multiline: bool,
  width: f32,
  height: f32,
  size: f32,
  align: i32,
) -> String {
  let usable = (width - 4.0).max(0.0);
  let lines = match multiline {
    false => vec![value.to_string()],
    true => wrap(value, usable, size),
  };
  let mut content = String::new();
  let top = height - size;

  for (index, line) in lines.iter().enumerate() {
    let x = match align {
      1 => ((width - text_width(line, size)) / 2.0).max(2.0),
      2 => (width - text_width(line, size) - 2.0).max(2.0),
      _ => 2.0,
    };
    let y = match multiline {
      true => top - index as f32 * size * 1.2,
      false => (height - size) / 2.0 + size * 0.22,
    };

    content.push_str(&format!(
      "1 0 0 1 {x} {y} Tm ({}) Tj ",
      escape_literal(line)
    ));
  }

  content
}

/// Greedy word wrap at a drawn width.
fn wrap(value: &str, usable: f32, size: f32) -> Vec<String> {
  let mut lines = Vec::new();
  let mut line = String::new();

  for word in value.split_whitespace() {
    let candidate = match line.is_empty() {
      true => word.to_string(),
      false => format!("{line} {word}"),
    };

    if !line.is_empty() && text_width(&candidate, size) > usable {
      lines.push(std::mem::take(&mut line));
      line.push_str(word);
    } else {
      line = candidate;
    }
  }
  if !line.is_empty() {
    lines.push(line);
  }

  lines
}

/// The filled dot a radio button's on state draws.
fn dot(width: f32, height: f32, color: [f32; 3]) -> String {
  let [red, green, blue] = color;
  let (x, y) = (width / 2.0, height / 2.0);
  let radius = width.min(height) * 0.28;
  // The handle length that turns four cubics into a circle.
  let handle = radius * 0.5523;

  format!(
    "q {red} {green} {blue} rg {right} {y} m {right} {up} {ox} {top} {x} {top} c \
     {mx} {top} {left} {up} {left} {y} c {left} {down} {mx} {bottom} {x} {bottom} c \
     {ox} {bottom} {right} {down} {right} {y} c f Q",
    right = x + radius,
    left = x - radius,
    top = y + radius,
    bottom = y - radius,
    up = y + handle,
    down = y - handle,
    ox = x + handle,
    mx = x - handle,
  )
}

/// The check mark a check box's on state draws.
fn check_mark(width: f32, height: f32, color: [f32; 3]) -> String {
  let [red, green, blue] = color;

  format!(
    "q {red} {green} {blue} RG 1.2 w {} {} m {} {} l {} {} l S Q",
    width * 0.2,
    height * 0.5,
    width * 0.42,
    height * 0.25,
    width * 0.8,
    height * 0.75,
  )
}
