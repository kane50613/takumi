---
packages:
  cargo:takumi:
    replay:
      - exit-prerelease(cargo:takumi)
---

### Represent the `none`/`normal` initial values of `max-*` and gaps

`max-width` and `max-height` are now a `MaxSize` value whose initial is `None`
(unbounded), instead of borrowing `Length`'s `auto`. `column-gap`, `row-gap`, and
the `gap` shorthand are now a `Gap` value whose initial is `Normal`. Rendering is
unchanged — `none` resolves like the old unbounded default and `normal` computes
to `0` — but the values now round-trip through `to_css` as `none`/`normal`.
