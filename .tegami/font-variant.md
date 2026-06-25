---
packages:
  "cargo:takumi-css": patch
  "cargo:takumi-core": patch
---

### Support `font-variant` properties

Add `font-variant` and its `font-variant-ligatures`, `font-variant-numeric`, `font-variant-east-asian`, `font-variant-caps`, and `font-variant-position` longhands. Each maps to OpenType features and resolves before `font-feature-settings`, which still wins on a tag conflict. `font-variant-alternates` and `font-variant-emoji` are out of scope, and missing features are not synthesized.
