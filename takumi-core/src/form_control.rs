use std::borrow::Cow;

use crate::layout::node::{Node, NodeKind};

/// The state of an `<input>` element's `type` attribute.
/// <https://html.spec.whatwg.org/multipage/input.html#attr-input-type>
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InputType {
  Hidden,
  Text,
  Search,
  Tel,
  Url,
  Email,
  Password,
  Date,
  Month,
  Week,
  Time,
  DateTimeLocal,
  Number,
  Range,
  Color,
  Checkbox,
  Radio,
  File,
  Submit,
  Image,
  Reset,
  Button,
}

impl InputType {
  /// The type of `node` when it is an `<input>`.
  pub(crate) fn of(node: &Node) -> Option<Self> {
    is_element(node, "input").then(|| Self::parse(node.attribute("type")))
  }

  /// A missing, invalid, or unknown keyword is the Text state.
  fn parse(keyword: Option<&str>) -> Self {
    const KEYWORDS: [(&str, InputType); 22] = [
      ("hidden", InputType::Hidden),
      ("text", InputType::Text),
      ("search", InputType::Search),
      ("tel", InputType::Tel),
      ("url", InputType::Url),
      ("email", InputType::Email),
      ("password", InputType::Password),
      ("date", InputType::Date),
      ("month", InputType::Month),
      ("week", InputType::Week),
      ("time", InputType::Time),
      ("datetime-local", InputType::DateTimeLocal),
      ("number", InputType::Number),
      ("range", InputType::Range),
      ("color", InputType::Color),
      ("checkbox", InputType::Checkbox),
      ("radio", InputType::Radio),
      ("file", InputType::File),
      ("submit", InputType::Submit),
      ("image", InputType::Image),
      ("reset", InputType::Reset),
      ("button", InputType::Button),
    ];

    keyword
      .and_then(|keyword| {
        KEYWORDS
          .iter()
          .find(|(name, _)| name.eq_ignore_ascii_case(keyword))
      })
      .map_or(Self::Text, |&(_, input_type)| input_type)
  }

  /// Blink's `TextFieldInputType`: a type edited in a single-line inner editor.
  pub(crate) fn is_text_field(self) -> bool {
    matches!(
      self,
      Self::Text
        | Self::Search
        | Self::Tel
        | Self::Url
        | Self::Email
        | Self::Password
        | Self::Number
    )
  }

  /// Whether the `placeholder` attribute applies, which is a text field's.
  pub(crate) fn supports_placeholder(self) -> bool {
    self.is_text_field()
  }

  /// Whether the `readonly` attribute applies.
  fn applies_readonly(self) -> bool {
    self.is_text_field()
      || matches!(
        self,
        Self::Date | Self::Month | Self::Week | Self::Time | Self::DateTimeLocal
      )
  }

  /// Whether the `required` attribute applies.
  fn applies_required(self) -> bool {
    self.applies_readonly() || matches!(self, Self::Checkbox | Self::Radio | Self::File)
  }

  /// The value sanitization algorithm of a text field's type, which leaves other types' values
  /// as they are.
  pub(crate) fn sanitize_value(self, value: &str) -> Cow<'_, str> {
    match self {
      Self::Number => {
        if value.is_empty() || is_valid_number(value) {
          Cow::Borrowed(value)
        } else {
          Cow::Borrowed("")
        }
      }
      Self::Url => strip_line_breaks(trim_html_spaces(value)),
      Self::Email => {
        let value = strip_line_breaks(value);

        Cow::Owned(trim_html_spaces(&value).to_owned())
      }
      Self::Text | Self::Search | Self::Tel | Self::Password => strip_line_breaks(value),
      _ => Cow::Borrowed(value),
    }
  }
}

/// Whether `node` is the HTML element named `tag`.
pub(crate) fn is_element(node: &Node, tag: &str) -> bool {
  node
    .tag_name()
    .is_some_and(|name| name.eq_ignore_ascii_case(tag))
}

/// Whether `node` has the boolean attribute `name`.
pub(crate) fn has_attribute(node: &Node, name: &str) -> bool {
  node.attribute(name).is_some()
}

/// Whether `node` is a listed form-associated element the `disabled` attribute applies to.
/// <https://html.spec.whatwg.org/multipage/forms.html#concept-fe-disabled>
pub(crate) fn is_disableable_form_control(node: &Node) -> bool {
  ["button", "input", "select", "textarea"]
    .iter()
    .any(|tag| is_element(node, tag))
}

/// Whether `node` matches `:read-write` on its own, ignoring the editing hosts above it: a
/// mutable `<input>` the `readonly` attribute applies to, or a mutable `<textarea>`.
/// <https://html.spec.whatwg.org/multipage/semantics-other.html#selector-read-write>
pub(crate) fn is_mutable_text_control(node: &Node, is_disabled: bool) -> bool {
  let applies = match InputType::of(node) {
    Some(input_type) => input_type.applies_readonly(),
    None => is_element(node, "textarea"),
  };

  applies && !is_disabled && !has_attribute(node, "readonly")
}

/// Whether `node` is a form control the `required` attribute applies to, and its value.
pub(crate) fn required_state(node: &Node) -> Option<bool> {
  let applies = match InputType::of(node) {
    Some(input_type) => input_type.applies_required(),
    None => is_element(node, "select") || is_element(node, "textarea"),
  };

  applies.then(|| has_attribute(node, "required"))
}

/// Whether `node` is an `<input>` or `<textarea>` presenting its placeholder: it has a
/// `placeholder` attribute and an empty value.
/// <https://html.spec.whatwg.org/multipage/semantics-other.html#selector-placeholder-shown>
pub(crate) fn is_placeholder_shown(node: &Node) -> bool {
  if !has_attribute(node, "placeholder") {
    return false;
  }

  match InputType::of(node) {
    Some(input_type) => {
      input_type.supports_placeholder()
        && input_type
          .sanitize_value(node.attribute("value").unwrap_or_default())
          .is_empty()
    }
    None => is_element(node, "textarea") && !has_text(node),
  }
}

/// Whether `node` or a descendant holds any text.
pub(crate) fn has_text(node: &Node) -> bool {
  let mut stack = vec![node];

  while let Some(node) = stack.pop() {
    match &node.kind {
      NodeKind::Text(data) if !data.text.is_empty() => return true,
      NodeKind::Container { children } => stack.extend(children),
      _ => {}
    }
  }

  false
}

/// Blink's `ParseToDoubleForNumberType`: a valid floating-point number that is finite.
/// <https://html.spec.whatwg.org/multipage/common-microsyntaxes.html#valid-floating-point-number>
fn is_valid_number(value: &str) -> bool {
  value.starts_with(|first: char| first == '-' || first == '.' || first.is_ascii_digit())
    && !value.ends_with('.')
    && value.parse::<f64>().is_ok_and(f64::is_finite)
}

fn strip_line_breaks(value: &str) -> Cow<'_, str> {
  if value.contains(['\r', '\n']) {
    Cow::Owned(value.replace(['\r', '\n'], ""))
  } else {
    Cow::Borrowed(value)
  }
}

fn trim_html_spaces(value: &str) -> &str {
  value.trim_matches(['\t', '\n', '\u{c}', '\r', ' '])
}

#[cfg(test)]
mod tests {
  use std::time::{Duration, Instant};

  use super::*;
  use crate::{matching::match_stylesheets_view, style::StyleSheet, viewport::Viewport};

  fn element(tag: &str, id: &str, attributes: &[(&str, &str)], children: Vec<Node>) -> Node {
    with_metadata(Node::container(children), tag, id, attributes)
  }

  fn text_element(tag: &str, id: &str, attributes: &[(&str, &str)], text: &str) -> Node {
    with_metadata(Node::text(text), tag, id, attributes)
  }

  fn with_metadata(node: Node, tag: &str, id: &str, attributes: &[(&str, &str)]) -> Node {
    node.with_tag_name(tag).with_id(id).with_attributes(
      attributes
        .iter()
        .map(|&(name, value)| (name.into(), value.into()))
        .collect(),
    )
  }

  /// The ids of the elements under `root` that `selector` matches, in tree order.
  fn matching_ids(root: &Node, selector: &str) -> Vec<String> {
    fn tree_order(root: &Node) -> Vec<&Node> {
      let mut nodes = Vec::new();
      let mut stack = vec![root];

      while let Some(node) = stack.pop() {
        nodes.push(node);
        stack.extend(node.children().unwrap_or_default().iter().rev());
      }

      nodes
    }

    let stylesheet = StyleSheet::parse(&format!("{selector} {{ width: 1px }}")).unwrap();
    let matched = match_stylesheets_view(root, &[&stylesheet], Viewport::default());
    tree_order(root)
      .iter()
      .zip(&matched)
      .filter(|(_, matched)| !matched.element().unlayered_normal().is_empty())
      .filter_map(|(node, _)| node.id().map(str::to_owned))
      .collect()
  }

  #[test]
  fn unknown_types_are_text() {
    assert_eq!(InputType::parse(None), InputType::Text);
    assert_eq!(InputType::parse(Some("bogus")), InputType::Text);
    assert_eq!(InputType::parse(Some("NUMBER")), InputType::Number);
    assert_eq!(
      InputType::parse(Some("datetime-local")),
      InputType::DateTimeLocal
    );
  }

  #[test]
  fn number_values_must_be_finite_valid_numbers() {
    for valid in ["-100.11", "0", ".5", "-.5", "1e3", "1E-3"] {
      assert_eq!(InputType::Number.sanitize_value(valid), valid);
    }

    for invalid in [
      "abc", "+1", " 1", "1 ", "1.", "inf", "-inf", "NaN", "1e999", "-",
    ] {
      assert_eq!(InputType::Number.sanitize_value(invalid), "", "{invalid}");
    }
  }

  #[test]
  fn text_values_lose_line_breaks() {
    assert_eq!(InputType::Text.sanitize_value("a\r\nb\nc"), "abc");
    assert_eq!(InputType::Password.sanitize_value(" a\n"), " a");
    assert_eq!(InputType::Url.sanitize_value(" a\nb "), "ab");
    assert_eq!(InputType::Email.sanitize_value(" a@b\n "), "a@b");
    assert_eq!(InputType::Date.sanitize_value("a\nb"), "a\nb");
  }

  #[test]
  fn disabled_reaches_through_fieldsets_but_not_their_first_legend() {
    let root = element(
      "form",
      "form",
      &[],
      vec![
        element("input", "plain", &[], vec![]),
        element("input", "own", &[("disabled", "")], vec![]),
        element(
          "fieldset",
          "fieldset",
          &[("disabled", "")],
          vec![
            element(
              "legend",
              "legend",
              &[],
              vec![element("input", "in-legend", &[], vec![])],
            ),
            element(
              "legend",
              "second-legend",
              &[],
              vec![element("input", "in-second-legend", &[], vec![])],
            ),
            element(
              "fieldset",
              "nested",
              &[],
              vec![element("textarea", "deep", &[], vec![])],
            ),
          ],
        ),
        element(
          "select",
          "select",
          &[],
          vec![
            element(
              "optgroup",
              "group",
              &[("disabled", "")],
              vec![text_element("option", "grouped", &[], "a")],
            ),
            text_element("option", "option", &[], "b"),
          ],
        ),
        element("div", "div", &[("disabled", "")], vec![]),
      ],
    );

    assert_eq!(
      matching_ids(&root, ":disabled"),
      [
        "own",
        "fieldset",
        "in-second-legend",
        "nested",
        "deep",
        "group",
        "grouped"
      ]
    );
    assert_eq!(
      matching_ids(&root, ":enabled"),
      ["plain", "in-legend", "select", "option"]
    );
  }

  #[test]
  fn checked_follows_markup_checkedness_and_selectedness() {
    let root = element(
      "div",
      "root",
      &[],
      vec![
        element(
          "input",
          "box",
          &[("type", "checkbox"), ("checked", "")],
          vec![],
        ),
        element("input", "text", &[("checked", "")], vec![]),
        element(
          "input",
          "first-radio",
          &[("type", "radio"), ("name", "a"), ("checked", "")],
          vec![],
        ),
        element(
          "input",
          "last-radio",
          &[("type", "RADIO"), ("name", "a"), ("checked", "")],
          vec![],
        ),
        element(
          "form",
          "form",
          &[],
          vec![element(
            "input",
            "other-form-radio",
            &[("type", "radio"), ("name", "a"), ("checked", "")],
            vec![],
          )],
        ),
        element(
          "select",
          "picked",
          &[],
          vec![
            text_element("option", "picked-1", &[("selected", "")], "1"),
            text_element("option", "picked-2", &[("selected", "")], "2"),
          ],
        ),
        element(
          "select",
          "defaulted",
          &[],
          vec![
            text_element("option", "defaulted-1", &[("disabled", "")], "1"),
            text_element("option", "defaulted-2", &[], "2"),
          ],
        ),
        element(
          "select",
          "list",
          &[("size", "3")],
          vec![text_element("option", "list-1", &[], "1")],
        ),
        element(
          "select",
          "multiple",
          &[("multiple", "")],
          vec![
            text_element("option", "multiple-1", &[("selected", "")], "1"),
            text_element("option", "multiple-2", &[("selected", "")], "2"),
          ],
        ),
      ],
    );

    assert_eq!(
      matching_ids(&root, ":checked"),
      [
        "box",
        "last-radio",
        "other-form-radio",
        "picked-2",
        "defaulted-2",
        "multiple-1",
        "multiple-2",
      ]
    );
  }

  #[test]
  fn indeterminate_matches_unchecked_radio_groups_and_valueless_progress() {
    let root = element(
      "div",
      "root",
      &[],
      vec![
        element("input", "lone", &[("type", "radio"), ("name", "a")], vec![]),
        element(
          "input",
          "grouped",
          &[("type", "radio"), ("name", "b")],
          vec![],
        ),
        element(
          "input",
          "checked",
          &[("type", "radio"), ("name", "b"), ("checked", "")],
          vec![],
        ),
        element("input", "box", &[("type", "checkbox")], vec![]),
        element("progress", "bar", &[], vec![]),
        element("progress", "valued", &[("value", "1")], vec![]),
      ],
    );

    assert_eq!(matching_ids(&root, ":indeterminate"), ["lone", "bar"]);
  }

  #[test]
  fn placeholder_shown_needs_a_placeholder_and_an_empty_sanitized_value() {
    let root = element(
      "div",
      "root",
      &[],
      vec![
        element("input", "empty", &[("placeholder", "p")], vec![]),
        element(
          "input",
          "filled",
          &[("placeholder", "p"), ("value", "v")],
          vec![],
        ),
        element(
          "input",
          "bad-number",
          &[("type", "number"), ("placeholder", "p"), ("value", "x")],
          vec![],
        ),
        element(
          "input",
          "date",
          &[("type", "date"), ("placeholder", "p")],
          vec![],
        ),
        element("input", "none", &[], vec![]),
        element("textarea", "area", &[("placeholder", "p")], vec![]),
        text_element("textarea", "full-area", &[("placeholder", "p")], "x"),
      ],
    );

    assert_eq!(
      matching_ids(&root, ":placeholder-shown"),
      ["empty", "bad-number", "area"]
    );
  }

  #[test]
  fn read_write_matches_mutable_text_controls_and_editable_content() {
    let root = element(
      "div",
      "root",
      &[],
      vec![
        element("input", "text", &[], vec![]),
        element("input", "readonly", &[("readonly", "")], vec![]),
        element("input", "disabled", &[("disabled", "")], vec![]),
        element("input", "box", &[("type", "checkbox")], vec![]),
        element("textarea", "area", &[], vec![]),
        element(
          "div",
          "editable",
          &[("contenteditable", "")],
          vec![
            element("span", "inside", &[], vec![]),
            element("span", "opted-out", &[("contenteditable", "false")], vec![]),
          ],
        ),
      ],
    );

    assert_eq!(
      matching_ids(&root, ":read-write"),
      ["text", "area", "editable", "inside"]
    );
    assert_eq!(
      matching_ids(&root, "input:read-only"),
      ["readonly", "disabled", "box"]
    );
  }

  #[test]
  fn required_and_optional_apply_only_where_required_does() {
    let root = element(
      "div",
      "root",
      &[],
      vec![
        element("input", "required", &[("required", "")], vec![]),
        element("input", "optional", &[], vec![]),
        element(
          "input",
          "hidden",
          &[("type", "hidden"), ("required", "")],
          vec![],
        ),
        element("input", "range", &[("type", "range")], vec![]),
        element("select", "select", &[("required", "")], vec![]),
        element("textarea", "area", &[], vec![]),
        element("div", "div", &[("required", "")], vec![]),
      ],
    );

    assert_eq!(matching_ids(&root, ":required"), ["required", "select"]);
    assert_eq!(matching_ids(&root, ":optional"), ["optional", "area"]);
  }

  /// Asserts that matching `selector` against `tree(4 * n)` costs about four times as much as
  /// against `tree(n)`, not the sixteen times a quadratic pass would.
  fn assert_matches_linearly(tree: impl Fn(usize) -> Node, selector: &str) {
    fn fastest_match(root: &Node, selector: &str) -> Duration {
      (0..3)
        .map(|_| {
          let start = Instant::now();

          matching_ids(root, selector);
          start.elapsed()
        })
        .min()
        .unwrap()
    }

    let ratio = fastest_match(&tree(8_000), selector).as_secs_f64()
      / fastest_match(&tree(2_000), selector).as_secs_f64();

    assert!(ratio < 8.0, "4x the elements took {ratio:.1}x as long");
  }

  #[test]
  fn matching_many_radios_and_options_stays_linear() {
    let tree = |count: usize| {
      let radios = (0..count).map(|index| {
        element(
          "input",
          "radio",
          &[
            ("type", "radio"),
            ("name", if index % 2 == 0 { "a" } else { "b" }),
          ],
          vec![],
        )
      });
      let options = (0..count).map(|_| text_element("option", "option", &[], "o"));

      element(
        "form",
        "form",
        &[],
        radios
          .chain([element("select", "select", &[], options.collect())])
          .collect(),
      )
    };
    let small = tree(2_000);

    assert_eq!(matching_ids(&small, ":indeterminate").len(), 2_000);
    assert_eq!(matching_ids(&small, ":checked").len(), 1);
    assert_matches_linearly(tree, ":checked, :indeterminate");
  }

  #[test]
  fn matching_inside_a_late_legend_of_a_disabled_fieldset_stays_linear() {
    let tree = |count: usize| {
      let inputs = (0..count).map(|_| element("input", "input", &[], vec![]));
      let divs = (0..count).map(|_| element("div", "div", &[], vec![]));

      element(
        "fieldset",
        "fieldset",
        &[("disabled", "")],
        divs
          .chain([
            element("legend", "legend", &[], inputs.collect()),
            element(
              "legend",
              "second",
              &[],
              vec![element("input", "late", &[], vec![])],
            ),
          ])
          .collect(),
      )
    };

    assert_eq!(matching_ids(&tree(2), "input:disabled"), ["late"]);
    assert_matches_linearly(tree, ":disabled, :enabled");
  }

  #[test]
  fn placeholder_shown_finds_text_deep_inside_a_textarea() {
    let mut text = Node::text("x");

    for _ in 0..100_000 {
      text = Node::container([text]);
    }

    let root = element(
      "div",
      "root",
      &[],
      vec![
        element("textarea", "deep", &[("placeholder", "p")], vec![text]),
        element(
          "textarea",
          "empty",
          &[("placeholder", "p")],
          vec![Node::text(""), Node::container([])],
        ),
      ],
    );

    assert_eq!(matching_ids(&root, ":placeholder-shown"), ["empty"]);
  }
}
