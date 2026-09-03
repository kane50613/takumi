---
packages:
  takumi-html:
    type: minor
  "@takumi-rs/helpers":
    type: minor
  takumi-core:
    type: minor
---

### Style form controls the way Blink does

`<input>`, `<textarea>`, `<select>`, `<button>`, `<option>` and `<optgroup>`
take their default styles from Blink's stylesheet. An `input[type=…]` preset
replaces the `input` one for that type. A closed `<select>` shows the option it
starts on and keeps its option list out of the flow; `multiple` or a `size`
above one lays the options out as a list box. In JSX, `defaultValue`,
`defaultChecked` and `htmlFor` reach the node as `value`, `checked` and `for`,
and a `<select>`'s `value` picks its options. `Node::option_state`,
`Node::option_label`, `Node::is_list_box` and `OptionState` carry the rules.
