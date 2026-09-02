---
packages:
  takumi-pdf:
    type: minor
  "@takumi-rs/wasm":
    type: minor
  takumi-js:
    type: patch
---

### Render text controls as fillable PDF fields

`form: true` emits `<input>` and `<textarea>` as AcroForm text fields. The field
takes its name, value and flags from the HTML attributes, its colors and
alignment from the CSS, and its screen-reader name from `aria-label`,
`aria-labelledby`, a `<label>`, `title` or `placeholder`. Two controls sharing a
`name` now fail the render with `PdfError::DuplicateFieldName`.
