---
packages:
  cargo:takumi-core: minor
---

### Seal the `selectors` crate behind a private matching module

Selector matching lives in takumi-core's crate-private `matching` module,
generic over a caller-implemented `MatchableNode`. `CssRule`, `LayerName`,
`Ident`, `SelectorImpl`, `PseudoClass`, and `PseudoElement` are `pub(crate)`,
keeping the `selectors` crate out of the public API.
