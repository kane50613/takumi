---
packages:
  takumi-pdf:
    type: minor
  "@takumi-rs/wasm":
    type: minor
---

### Render text controls as fillable PDF fields

Set `form: true` to turn named inputs and textareas into editable AcroForm fields. Invalid field names, text outside WinAnsiEncoding, and PDF/A or PDF/UA combinations reject the render.
