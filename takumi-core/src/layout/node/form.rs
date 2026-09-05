//! What a form control's attributes say about the field it starts as.

use super::Node;

/// The attributes that decide which `<option>`s a `<select>` starts on.
#[derive(Debug, Clone, Copy)]
pub struct OptionState {
  /// Whether the option carries `selected`.
  pub selected: bool,
  /// Whether the option carries `disabled`.
  pub disabled: bool,
}

impl OptionState {
  /// The options a `<select>` starts on, by their place in `options`.
  ///
  /// Follows HTML's selectedness setting algorithm: a `multiple` select keeps
  /// every `selected` option, any other keeps the last one, and a closed
  /// drop-down with none falls back to its first enabled option.
  /// https://html.spec.whatwg.org/multipage/form-elements.html#selectedness-setting-algorithm
  pub fn chosen(options: &[Self], multiple: bool, closed: bool) -> Vec<usize> {
    if multiple {
      return options
        .iter()
        .enumerate()
        .filter(|(_, option)| option.selected)
        .map(|(index, _)| index)
        .collect();
    }

    options
      .iter()
      .rposition(|option| option.selected)
      .or_else(|| {
        closed
          .then(|| options.iter().position(|option| !option.disabled))
          .flatten()
      })
      .into_iter()
      .collect()
  }
}

impl Node {
  /// This element's selection state, when it is an `<option>`.
  pub fn option_state(&self) -> Option<OptionState> {
    self
      .tag_name()
      .filter(|tag| tag.eq_ignore_ascii_case("option"))?;

    Some(OptionState {
      selected: self.attribute("selected").is_some(),
      disabled: self.attribute("disabled").is_some(),
    })
  }

  /// The `label` an `<option>` shows in place of its text, when it carries a
  /// non-empty one.
  pub fn option_label(&self) -> Option<&str> {
    self.attribute("label").filter(|label| !label.is_empty())
  }

  /// Whether a `<select>` lays its options out as a list box, which `multiple`
  /// or a `size` above one asks for. Any other select is a closed drop-down
  /// showing one option.
  pub fn is_list_box(&self) -> bool {
    self.attribute("multiple").is_some()
      || self
        .attribute("size")
        .map(str::trim)
        .filter(|size| size.bytes().all(|byte| byte.is_ascii_digit()))
        .and_then(|size| size.parse::<u32>().ok())
        .is_some_and(|size| size > 1)
  }
}
