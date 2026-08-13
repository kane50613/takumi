---
packages:
  takumi-core:
    type: minor
---

### Snap border and outline widths to whole device pixels

`border-*-width`, `outline-width` and `outline-offset` now follow the CSS `snap a length as a border width` rule. Anything thinner than one device pixel draws as one, so a hairline stops fading in and out with its position. The rest rounds toward zero, so `1.5px` draws as `1px`.
