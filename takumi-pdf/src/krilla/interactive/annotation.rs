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

use crate::krilla::chunk_container::{ChunkContainer, FieldSlot};
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
    let (owner, on_state) = match &self.annotation_type {
      // A radio group is one field owning every button as a kid, so the
      // buttons themselves carry no field name.
      AnnotationType::Widget(widget) => match &widget.field {
        FormField::Radio { export, on } => {
          let (group, state) = chunk_container.radio_group(sc, widget, export, *on, root_ref);

          (Some(WidgetOwner::Group(group)), state)
        }
        field => (
          Some(WidgetOwner::Field(chunk_container.field_slot(
            sc,
            &widget.name,
            root_ref,
          ))),
          field.on_state(),
        ),
      },
      AnnotationType::Link(_) => (None, String::new()),
    };
    let appearance = match &self.annotation_type {
      AnnotationType::Widget(widget) => {
        Some(widget.write_appearance(sc, chunk_container, &on_state))
      }
      AnnotationType::Link(_) => None,
    };
    let chunk = &mut chunk_container.non_stream.annotations;
    let mut annotation = chunk
      .indirect(root_ref)
      .start::<pdf_writer::writers::Annotation>();

    self.annotation_type.serialize_type(
      sc,
      &mut annotation,
      page_height,
      page_ref,
      owner.as_ref(),
      &on_state,
    )?;

    if let Some(appearance) = appearance {
      appearance.serialize(&mut annotation);
    }
    let always_print = match &self.annotation_type {
      AnnotationType::Link(link) => link.border.is_none(),
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
    page_ref: Ref,
    owner: Option<&WidgetOwner>,
    on_state: &str,
  ) -> KrillaResult<()> {
    match (self, owner) {
      (AnnotationType::Link(link), _) => link.serialize_type(sc, annotation, page_height),
      (AnnotationType::Widget(widget), Some(owner)) => {
        widget.serialize_type(annotation, page_height, page_ref, owner, on_state);
        Ok(())
      }
      (AnnotationType::Widget(_), None) => Ok(()),
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

/// How a widget's own CSS paints it, mirrored into the field so a viewer
/// regenerating the appearance lands somewhere close to the rendered box.
pub struct WidgetStyle {
  /// Text color.
  pub color: [f32; 3],
  /// Text size in points.
  pub font_size: f32,
  /// `/Q`: 0 left, 1 center, 2 right.
  pub align: i32,
  /// `/Ff` ReadOnly.
  pub read_only: bool,
  /// `/Ff` Required.
  pub required: bool,
  /// `/Ff` NoExport.
  pub no_export: bool,
}

/// The kind of form field a widget annotation carries.
pub enum FormField {
  /// A text field with the value it starts at.
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
    /// Whether this button is the selected one.
    on: bool,
    /// What this button submits, written to the group's `/Opt`. The `/AP`
    /// state name is the button's place in the group instead, so a value a
    /// PDF name cannot carry still survives.
    export: String,
  },
  /// A drop-down or list box.
  Choice {
    /// Every option as the value it submits and the text it shows.
    options: Vec<(String, String)>,
    /// Which options start selected, by their place in `options`.
    selected: Vec<usize>,
    /// Whether more than one option can be selected.
    multi: bool,
    /// Whether the options lay out as a list box rather than a closed
    /// drop-down.
    list: bool,
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
    if style.no_export {
      flags |= 1 << 2;
    }
    match self {
      Self::Text {
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
      Self::Radio { .. } => flags |= (1 << 14) | (1 << 15),
      Self::Choice { multi, list, .. } => {
        if !*list {
          flags |= 1 << 17;
        }
        if *multi {
          flags |= 1 << 21;
        }
      }
      Self::CheckBox { .. } => {}
    }

    flags
  }

  /// The text a choice field's appearance draws: every option as a row of a
  /// list box, or what a drop-down's selected option shows, which is not what
  /// it submits.
  fn display(&self) -> String {
    let Self::Choice {
      options,
      selected,
      list,
      ..
    } = self
    else {
      return String::new();
    };
    let shown = match list {
      true => options
        .iter()
        .map(|(_, display)| display.as_str())
        .collect(),
      false => selected
        .iter()
        .filter_map(|&index| options.get(index))
        .map(|(_, display)| display.as_str())
        .collect::<Vec<_>>(),
    };

    shown.join("\n")
  }

  /// The `/AP` state name the field files its "on" appearance under.
  fn on_state(&self) -> String {
    match self {
      Self::CheckBox { export, .. } if export == "Off" => "0".to_string(),
      Self::CheckBox { export, .. } => export.clone(),
      Self::Text { .. } | Self::Radio { .. } | Self::Choice { .. } => String::new(),
    }
  }
}

/// The appearance streams of one widget: a single stream for a text field, or
/// one per state for a check box and radio button.
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

/// Who writes a widget's field entries.
pub(crate) enum WidgetOwner {
  /// The widget is the field itself, at this place in the hierarchy.
  Field(FieldSlot),
  /// A radio group holds the field entries; the button only says which state
  /// it shows.
  Group(Ref),
}

/// A form field widget annotation, merged with the field dictionary it names.
pub struct WidgetAnnotation {
  pub(crate) rect: Rect,
  /// The HTML name, which `/T` spells one period-delimited segment at a time.
  pub(crate) name: String,
  pub(crate) field: FormField,
  pub(crate) style: WidgetStyle,
  /// `/TU`, the name a reader announces the field by.
  pub(crate) description: Option<String>,
  /// `/Lang`, which PDF/UA requires on an annotation carrying text.
  lang: Option<String>,
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

    format!(
      "/{FORM_FONT} {} Tf {red} {green} {blue} rg",
      self.style.font_size
    )
  }

  pub(crate) fn value_is_encodable(&self) -> bool {
    match &self.field {
      FormField::Text {
        value,
        password: false,
        ..
      } => value.chars().all(|character| {
        matches!(character, '\t' | '\n' | '\r') || win_ansi_byte(character).is_some()
      }),
      _ => true,
    }
  }

  fn write_appearance(
    &self,
    sc: &mut SerializeContext,
    chunk_container: &mut ChunkContainer,
    on_state: &str,
  ) -> WidgetAppearance {
    let width = self.rect.right() - self.rect.left();
    let height = self.rect.bottom() - self.rect.top();
    let bbox = PdfRect::new(0.0, 0.0, width, height);
    let size = self.style.font_size;
    let (content, off) = match &self.field {
      FormField::Text {
        value,
        multiline,
        password,
        ..
      } => {
        // A password control shows one asterisk per character, so the field
        // it becomes does too.
        let drawn = match password {
          true => "*".repeat(value.chars().count()),
          false => value.clone(),
        };

        (
          text_appearance(&drawn, *multiline, width, height, size, &self.style),
          None,
        )
      }
      FormField::Choice { list, .. } => (
        text_appearance(
          &self.field.display(),
          *list,
          width,
          height,
          size,
          &self.style,
        ),
        None,
      ),
      FormField::CheckBox { .. } => (
        check_mark(width, height, self.style.color),
        Some(sc.new_ref()),
      ),
      FormField::Radio { .. } => (dot(width, height, self.style.color), Some(sc.new_ref())),
    };
    // The font reference is taken before the stream opens, which borrows the
    // same chunk.
    let font = matches!(
      self.field,
      FormField::Text { .. } | FormField::Choice { .. }
    )
    .then(|| chunk_container.form_font(sc));

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
      .form_xobject(on, content.as_bytes());

    stream.bbox(bbox);

    if let Some(font) = font {
      stream
        .resources()
        .fonts()
        .pair(Name(FORM_FONT.as_bytes()), font);
    }
    stream.finish();

    WidgetAppearance {
      on,
      off,
      on_state: on_state.to_string(),
    }
  }

  fn serialize_type(
    &self,
    annotation: &mut pdf_writer::writers::Annotation,
    page_height: f32,
    page_ref: Ref,
    owner: &WidgetOwner,
    on_state: &str,
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
    // The page already paints the control's border and background. Leaving
    // `/MK` out keeps a regenerated appearance transparent, so a rounded
    // corner or a two-tone border survives the redraw.
    annotation.border(0.0, 0.0, 0.0, None);

    if let Some(lang) = &self.lang {
      annotation.pair(Name(b"Lang"), TextStr(lang));
    }

    match owner {
      WidgetOwner::Group(group) => {
        annotation.pair(Name(b"Parent"), *group);
      }
      WidgetOwner::Field(slot) => {
        if let Some(parent) = slot.parent {
          annotation.pair(Name(b"Parent"), parent);
        }
        annotation.pair(Name(b"T"), TextStr(&slot.partial));

        if let Some(mapping) = &slot.mapping {
          annotation.pair(Name(b"TM"), TextStr(mapping));
        }
        annotation.pair(Name(b"Ff"), self.field.flags(&self.style));

        if let Some(description) = &self.description {
          annotation.pair(Name(b"TU"), TextStr(description));
        }
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
      FormField::CheckBox { on, export } => {
        let state = match on {
          true => Name(on_state.as_bytes()),
          false => Name(b"Off"),
        };

        annotation.pair(Name(b"FT"), Name(b"Btn"));
        annotation.pair(Name(b"V"), state);
        annotation.pair(Name(b"DV"), state);
        annotation.pair(Name(b"AS"), state);

        if export == "Off" {
          annotation
            .insert(Name(b"Opt"))
            .array()
            .item(TextStr(export));
        }
      }
      FormField::Choice {
        options, selected, ..
      } => {
        annotation.pair(Name(b"FT"), Name(b"Ch"));
        annotation.pair(Name(b"DA"), Str(self.default_appearance().as_bytes()));
        annotation.pair(Name(b"Q"), self.style.align);

        let exports = selected
          .iter()
          .filter_map(|&index| options.get(index))
          .map(|(export, _)| TextStr(export))
          .collect::<Vec<_>>();

        for key in [Name(b"V"), Name(b"DV")] {
          // A list box holding more than one selection writes them as an
          // array; one selection is written as the value it is.
          match exports.as_slice() {
            [] => {}
            [one] => {
              annotation.pair(key, *one);
            }
            many => {
              annotation.insert(key).array().items(many.iter().copied());
            }
          }
        }
        let mut opt = annotation.insert(Name(b"Opt")).array();

        for (export, display) in options {
          match export == display {
            // An option whose submitted value differs from its label writes
            // both, export first, as the spec pairs them.
            false => {
              opt
                .push()
                .array()
                .items([TextStr(export), TextStr(display)]);
            }
            true => {
              opt.item(TextStr(display));
            }
          }
        }
        opt.finish();
      }
      // The group owns `/FT` and `/V`; the button only says which state it
      // shows.
      FormField::Radio { on, .. } => {
        annotation.pair(
          Name(b"AS"),
          match on {
            true => Name(on_state.as_bytes()),
            false => Name(b"Off"),
          },
        );
      }
    }
  }
}

/// The resource name the form's shared face is registered under, in `/DR` and
/// in every appearance stream.
pub(crate) const FORM_FONT: &str = "Helv";

/// The whole content stream a text field's appearance draws.
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
     BT /{FORM_FONT} {size} Tf {red} {green} {blue} rg {text} ET Q EMC"
  )
}

/// Helvetica's advance widths for the printable ASCII range, in thousandths
/// of an em, from the face's own metrics. A character outside that range is
/// approximated at 500, which only moves where a line wraps or centers.
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
      Some(index) => u32::from(HELVETICA_WIDTHS.get(index as usize).copied().unwrap_or(500)),
      None => 0,
    })
    .sum();

  thousandths as f32 / 1000.0 * size
}

/// A `Tj` run per line of the value.
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
      win_ansi_literal(line)
    ));
  }

  content
}

/// Greedy word wrap at a drawn width, keeping the line breaks the value
/// already has.
fn wrap(value: &str, usable: f32, size: f32) -> Vec<String> {
  value
    .lines()
    .flat_map(|line| wrap_line(line, usable, size))
    .collect()
}

/// One source line wrapped at a drawn width; an empty one stays one empty
/// line.
fn wrap_line(value: &str, usable: f32, size: f32) -> Vec<String> {
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
  if !line.is_empty() || lines.is_empty() {
    lines.push(line);
  }

  lines
}

/// The value as a PDF literal string in WinAnsiEncoding, which is what the
/// appearance's face reads. A character the encoding cannot hold is dropped,
/// so no viewer draws a stray byte in its place.
fn win_ansi_literal(value: &str) -> String {
  let mut literal = String::with_capacity(value.len());

  for character in value.chars() {
    let Some(byte) = win_ansi_byte(character) else {
      continue;
    };

    match byte {
      b'(' | b')' | b'\\' => {
        literal.push('\\');
        literal.push(byte as char);
      }
      0x20..=0x7E => literal.push(byte as char),
      _ => literal.push_str(&format!("\\{byte:03o}")),
    }
  }

  literal
}

/// WinAnsiEncoding is Latin-1 with the C1 range replaced by the punctuation
/// and symbols listed in PDF 32000 annex D.
fn win_ansi_byte(character: char) -> Option<u8> {
  const C1: [char; 27] = [
    '\u{20AC}', '\u{201A}', '\u{0192}', '\u{201E}', '\u{2026}', '\u{2020}', '\u{2021}', '\u{02C6}',
    '\u{2030}', '\u{0160}', '\u{2039}', '\u{0152}', '\u{017D}', '\u{2018}', '\u{2019}', '\u{201C}',
    '\u{201D}', '\u{2022}', '\u{2013}', '\u{2014}', '\u{02DC}', '\u{2122}', '\u{0161}', '\u{203A}',
    '\u{0153}', '\u{017E}', '\u{0178}',
  ];
  const C1_BYTES: [u8; 27] = [
    0x80, 0x82, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89, 0x8A, 0x8B, 0x8C, 0x8E, 0x91, 0x92, 0x93,
    0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9A, 0x9B, 0x9C, 0x9E, 0x9F,
  ];

  match character {
    ' '..='~' => Some(character as u8),
    '\u{A0}'..='\u{FF}' => Some(character as u8),
    _ => C1
      .iter()
      .position(|&mapped| mapped == character)
      .map(|index| C1_BYTES[index]),
  }
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
