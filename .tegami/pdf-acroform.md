---
packages:
  "takumi-pdf":
    type: minor
  "@takumi-rs/wasm":
    type: minor
  "takumi-js":
    type: patch
---

### Render form controls as fillable PDF fields

`form: true` emits `<input>`, `<textarea>` and `<select>` as AcroForm fields:
text, multiline, password, check box, radio group and drop-down. The field
takes its name, value, flags and options from the HTML attributes, its colors
and alignment from the CSS, and its screen-reader name from `aria-label`, a
`<label for>`, `title` or `placeholder`. Tagged output wraps each widget in a
`Form` structure element.

Form controls now lay out as block-level boxes and `<option>` no longer paints
its text into the page. Two controls that share a `name` without being one
radio group now fail the render.
