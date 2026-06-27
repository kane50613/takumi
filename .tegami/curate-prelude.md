---
packages:
  "cargo:takumi": major
---

### Curate the prelude to a stable value-type surface

Replace the `style::*` glob with an explicit list, dropping the internal
`FromCss`/`ToCss` parser traits and render-fast-path helpers from the frozen
prelude.
