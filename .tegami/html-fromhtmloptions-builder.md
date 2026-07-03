---
packages:
  cargo:takumi-html: minor
---

### Build `FromHtmlOptions` with a builder

`FromHtmlOptions` fields are now `pub(crate)` and the struct is
`#[non_exhaustive]`; construct it via `FromHtmlOptions::builder()` (or
`default()`). The `with_presets`, `with_tailwind_property`, and `with_max_depth`
methods are gone.
