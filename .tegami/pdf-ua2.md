---
packages:
  takumi-pdf:
    type: minor
---

### Validate against PDF/UA-2

`tagged: "ua2"` validates the structure tree against PDF/UA-2. It pairs with `pdfa: "4"`, `pdfa: "4f"`, or plain PDF. The types reject the PDF 1.7 levels, because PDF/UA-2 is PDF 2.0 only.

A render without `lang` now fails under `"ua2"`. The standard requires a document language.
