---
packages:
  "npm:@takumi-rs/helpers": minor
---

### Type `googleFonts` families against the Google Fonts catalog

`GoogleFontFamily` now knows every Google Font, so known families autocomplete
their available `weight`, `style`, and variable `axes`. Any other string still
passes, keeping private or brand-new families working.

Set a variable axis per family, e.g. `{ family: "Inter", axes: { opsz: "14..32" } }`.
The catalog is generated at build time and is types-only, so the runtime bundle is
unchanged.
