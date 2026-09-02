//! Form controls collected from the scene, and the widget annotations they
//! become.

use std::collections::HashMap;

use takumi_core::{
  layout::{node::Node, tree::RenderNode},
  style::{Color, TextAlign},
};

use crate::{
  emitter::DocumentState,
  krilla::{
    annotation::{Annotation, FormField, WidgetAnnotation, WidgetStyle},
    geom::Rect as KrillaRect,
    page::Page,
  },
  options::{PT_PER_PX, PdfError},
  tags::{raw_text, text_content},
  window::Window,
};

/// A field name one of the drawn pages emitted.
pub(crate) struct EmittedField {
  pub(crate) name: String,
  /// Whether this field may share its name with another.
  pub(crate) shares_name: bool,
  pub(crate) disabled: bool,
}

/// A form control box in content coordinates.
pub(crate) struct FieldTarget {
  /// The HTML `name`, which the field exports under.
  pub(crate) name: String,
  pub(crate) rect: KrillaRect,
  field: FieldKind,
  style: FieldStyle,
  label: FieldLabel,
  /// Source-node path, so the widget can join that node's `Form` element.
  pub(crate) path: Vec<usize>,
}

/// The control an element asks for, before its value is read.
enum ControlKind {
  Text { multiline: bool, password: bool },
  CheckBox,
  Radio,
  Choice { multi: bool },
}

impl ControlKind {
  /// The control this element becomes, or `None` when it is not one a
  /// standalone document can make fillable.
  fn of(source: &Node) -> Option<Self> {
    let tag = source.tag_name()?;

    if tag.eq_ignore_ascii_case("textarea") {
      return Some(Self::Text {
        multiline: true,
        password: false,
      });
    }
    if tag.eq_ignore_ascii_case("select") {
      return Some(Self::Choice {
        multi: source.attribute("multiple").is_some(),
      });
    }
    if !tag.eq_ignore_ascii_case("input") {
      return None;
    }
    let kind = source.attribute("type").unwrap_or("text");

    if kind.eq_ignore_ascii_case("checkbox") {
      return Some(Self::CheckBox);
    }
    if kind.eq_ignore_ascii_case("radio") {
      return Some(Self::Radio);
    }
    // A button carries an action and `file` a local path, neither of which a
    // document can bind; `hidden` has no box for a widget to cover.
    const UNSUPPORTED: [&str; 6] = ["hidden", "submit", "reset", "button", "image", "file"];

    (!UNSUPPORTED
      .iter()
      .any(|unsupported| kind.eq_ignore_ascii_case(unsupported)))
    .then(|| Self::Text {
      multiline: false,
      password: kind.eq_ignore_ascii_case("password"),
    })
  }
}

/// Whether this element becomes a fillable field.
pub(crate) fn is_form_control(source: &Node) -> bool {
  ControlKind::of(source).is_some()
}

/// A control with the value it starts at.
enum FieldKind {
  Text {
    value: String,
    multiline: bool,
    password: bool,
    max_len: Option<i32>,
  },
  CheckBox {
    on: bool,
    export: String,
  },
  Radio {
    on: bool,
    export: String,
  },
  Choice {
    /// Every option as the value it submits and the text it shows.
    options: Vec<ChoiceOption>,
    /// Which options start selected, by their place in `options`.
    selected: Vec<usize>,
    /// Whether more than one option can be picked, which also turns the
    /// drop-down into a list box.
    multi: bool,
  },
}

/// One `<option>`: the value it submits and the text it shows.
pub(crate) struct ChoiceOption {
  pub(crate) export: String,
  pub(crate) display: String,
}

impl FieldKind {
  fn of(source: &Node, node: &RenderNode) -> Option<Self> {
    let on = source.attribute("checked").is_some();

    Some(match ControlKind::of(source)? {
      ControlKind::CheckBox => Self::CheckBox {
        on,
        export: export_value(source),
      },
      ControlKind::Radio => Self::Radio {
        on,
        export: export_value(source),
      },
      ControlKind::Choice { multi } => {
        let options = options(node);
        let mut selected = selected_options(node);

        // A closed drop-down always shows one option, so the first stands in
        // when the markup names none.
        if selected.is_empty() && !multi && !options.is_empty() {
          selected.push(0);
        }

        Self::Choice {
          options,
          selected,
          multi,
        }
      }
      ControlKind::Text {
        multiline,
        password,
      } => Self::Text {
        // A `<textarea>` holds its initial value as its text, not as an
        // attribute.
        value: match multiline {
          true => raw_text(node),
          false => source.attribute("value").unwrap_or_default().to_string(),
        },
        multiline,
        password,
        max_len: max_len(source),
      },
    })
  }

  /// Whether this control may share its name, which only the buttons of one
  /// radio group do.
  fn shares_name(&self) -> bool {
    matches!(self, Self::Radio { .. })
  }

  fn to_form_field(&self) -> FormField {
    match self {
      Self::Text {
        value,
        multiline,
        password,
        max_len,
      } => FormField::Text {
        value: value.clone(),
        multiline: *multiline,
        password: *password,
        max_len: *max_len,
      },
      Self::CheckBox { on, export } => FormField::CheckBox {
        on: *on,
        export: export.clone(),
      },
      Self::Radio { on, export } => FormField::Radio {
        on: *on,
        export: export.clone(),
      },
      Self::Choice {
        options,
        selected,
        multi,
      } => FormField::Choice {
        options: options
          .iter()
          .map(|option| (option.export.clone(), option.display.clone()))
          .collect(),
        selected: selected.clone(),
        multi: *multi,
      },
    }
  }
}

/// Every `<option>` under a `<select>`. Layout moves the source children into
/// the render tree, so they are read from there.
fn options(node: &RenderNode) -> Vec<ChoiceOption> {
  let mut options = Vec::new();

  visit_options(node, &mut |option, _| options.push(option));
  options
}

/// The places of the options carrying `selected`.
fn selected_options(node: &RenderNode) -> Vec<usize> {
  let mut selected = Vec::new();
  let mut index = 0;

  visit_options(node, &mut |_, on| {
    if on {
      selected.push(index);
    }
    index += 1;
  });
  selected
}

fn visit_options(node: &RenderNode, visit: &mut impl FnMut(ChoiceOption, bool)) {
  for child in node.children.as_deref().unwrap_or_default() {
    let Some(source) = child.node.as_ref() else {
      continue;
    };

    if !source
      .tag_name()
      .is_some_and(|tag| tag.eq_ignore_ascii_case("option"))
    {
      visit_options(child, visit);
      continue;
    }
    let display = text_content(child);

    visit(
      ChoiceOption {
        export: source
          .attribute("value")
          .map(str::to_string)
          .unwrap_or_else(|| display.clone()),
        display,
      },
      source.attribute("selected").is_some(),
    );
  }
}

/// What a button submits. HTML sends `on` only when the control carries no
/// `value` at all, so an empty one stays empty.
fn export_value(source: &Node) -> String {
  source.attribute("value").unwrap_or("on").to_string()
}

/// `/MaxLen` is a positive integer, and HTML ignores a negative `maxlength`
/// too.
fn max_len(source: &Node) -> Option<i32> {
  source
    .attribute("maxlength")?
    .parse::<i32>()
    .ok()
    .filter(|len| *len > 0)
}

/// Where a field's accessible name can come from.
struct FieldLabel {
  /// The `aria-labelledby` id while the element it names is still unread, then
  /// that element's text.
  labelled_by: Option<String>,
  aria_label: Option<String>,
  /// The `id` a `<label for>` would point at.
  id: Option<String>,
  /// The text of the `<label>` wrapping the control.
  wrapping: Option<String>,
  fallback: Option<String>,
}

impl FieldLabel {
  fn of(source: &Node) -> Self {
    Self {
      labelled_by: source.attribute("aria-labelledby").map(str::to_string),
      aria_label: source.attribute("aria-label").map(str::to_string),
      id: source.id().map(str::to_string),
      wrapping: None,
      fallback: ["title", "placeholder"]
        .into_iter()
        .find_map(|attribute| source.attribute(attribute))
        .map(str::to_string),
    }
  }

  /// The name a reader announces, in the order HTML resolves one.
  fn resolve(&self, labels: &HashMap<String, String>) -> Option<String> {
    self
      .labelled_by
      .clone()
      .or_else(|| self.aria_label.clone())
      .or_else(|| self.id.as_ref().and_then(|id| labels.get(id)).cloned())
      .or_else(|| self.wrapping.clone())
      .or_else(|| self.fallback.clone())
  }
}

/// The CSS a control paints with, which its field carries too.
struct FieldStyle {
  color: [f32; 3],
  font_size: f32,
  /// `/Q`: 0 left, 1 center, 2 right.
  align: i32,
  read_only: bool,
  required: bool,
  /// HTML keeps a disabled control out of the submission, which `/Ff` NoExport
  /// does.
  disabled: bool,
}

impl FieldStyle {
  fn of(node: &RenderNode, source: &Node) -> Self {
    let style = &node.context.style;
    let color = style.color.resolve(Color([0, 0, 0, 255]));

    Self {
      color: pdf_color(color).unwrap_or([0.0, 0.0, 0.0]),
      font_size: node.context.sizing.font_size * PT_PER_PX,
      align: match style.text_align {
        TextAlign::Center => 1,
        TextAlign::Right | TextAlign::End => 2,
        _ => 0,
      },
      read_only: source.attribute("readonly").is_some(),
      required: source.attribute("required").is_some(),
      disabled: source.attribute("disabled").is_some(),
    }
  }

  fn to_widget(&self) -> WidgetStyle {
    WidgetStyle {
      color: self.color,
      font_size: self.font_size,
      align: self.align,
      read_only: self.read_only || self.disabled,
      required: self.required,
      no_export: self.disabled,
    }
  }
}

/// The color as PDF's three components, absent when fully transparent.
fn pdf_color(color: Color) -> Option<[f32; 3]> {
  let [red, green, blue, alpha] = color.0;

  (alpha > 0).then(|| [red, green, blue].map(|channel| channel as f32 / 255.0))
}

impl FieldTarget {
  /// The control this box holds, or `None` when it is not a named one.
  pub(crate) fn of(
    node: &RenderNode,
    source: &Node,
    rect: KrillaRect,
    path: &[usize],
  ) -> Option<Self> {
    let field = FieldKind::of(source, node)?;
    let name = source
      .attribute("name")
      .or_else(|| source.id())
      .filter(|name| !name.is_empty())?;

    Some(Self {
      name: name.to_string(),
      rect,
      field,
      style: FieldStyle::of(node, source),
      label: FieldLabel::of(source),
      path: path.to_vec(),
    })
  }

  /// The `aria-labelledby` id this field still has to resolve.
  pub(crate) fn labelled_by(&self) -> Option<&str> {
    self.label.labelled_by.as_deref()
  }

  /// Replaces the `aria-labelledby` id with the text of the element it names.
  pub(crate) fn set_labelled_by(&mut self, text: Option<String>) {
    self.label.labelled_by = text;
  }

  /// Sets the text of the `<label>` wrapping the control.
  pub(crate) fn set_wrapping_label(&mut self, text: Option<String>) {
    self.label.wrapping = text;
  }

  /// The widget annotation for this field at the box it takes on one page.
  fn annotation(
    &self,
    rect: KrillaRect,
    labels: &HashMap<String, String>,
    lang: Option<&str>,
  ) -> Result<Annotation, PdfError> {
    let described = self.label.resolve(labels);

    if self.name.split('.').any(str::is_empty) {
      return Err(PdfError::InvalidFieldName(self.name.clone()));
    }
    let widget = WidgetAnnotation::new(
      rect,
      self.name.clone(),
      self.field.to_form_field(),
      self.style.to_widget(),
    )
    .with_description(described.clone())
    .with_lang(lang.map(str::to_string));

    if !widget.value_is_encodable() {
      return Err(PdfError::UnsupportedFormValue(self.name.clone()));
    }

    Ok(Annotation::new_widget(
      widget,
      Some(described.unwrap_or_else(|| self.name.clone())),
    ))
  }
}

/// Adds this page's slice of every form field as widget annotations.
pub(crate) fn add_field_annotations(
  page: &mut Page,
  fields: &[FieldTarget],
  labels: &HashMap<String, String>,
  window: Window,
  offset: (f32, f32),
  state: &DocumentState,
) {
  if !state.form {
    return;
  }
  let (y0, y1) = window.y.unwrap_or((f32::NEG_INFINITY, f32::INFINITY));

  // A control stays off a page break, so the page holding its top holds all
  // of it. One taller than the page keeps that page too, cut at the bottom
  // the way the box itself is painted.
  for field in fields {
    if field.rect.top() < y0 || field.rect.top() >= y1 {
      continue;
    }
    let Some(rect) = KrillaRect::from_ltrb(
      (field.rect.left() + offset.0) * PT_PER_PX,
      (field.rect.top() - y0 + offset.1) * PT_PER_PX,
      (field.rect.right() + offset.0) * PT_PER_PX,
      (field.rect.bottom().min(y1) - y0 + offset.1) * PT_PER_PX,
    ) else {
      continue;
    };
    state.field_names.borrow_mut().push(EmittedField {
      name: field.name.clone(),
      shares_name: field.field.shares_name(),
      disabled: field.style.disabled,
    });

    let annotation = match field.annotation(rect, labels, state.lang) {
      Ok(annotation) => annotation,
      Err(error) => {
        state.issues.borrow_mut().failure.get_or_insert(error);
        continue;
      }
    };

    match state.tags.as_ref() {
      Some(tags) => {
        let identifier = page.add_tagged_annotation(annotation);

        tags.borrow_mut().record_annotation(&field.path, identifier);
      }
      None => page.add_annotation(annotation),
    }
  }
}
