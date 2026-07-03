---
packages:
  cargo:takumi-css: minor
  cargo:takumi-core: minor
---

### Match selectors in takumi-css and seal the `selectors` crate

Selector matching moved out of takumi-core's `layout::matching` into a new
`matching` module in takumi-css, generic over a caller-implemented
`MatchableNode`. `CssRule`, `LayerName`, `Ident`, `SelectorImpl`,
`PseudoClass`, and `PseudoElement` are now `pub(crate)`, keeping the
`selectors` crate out of the public API.
