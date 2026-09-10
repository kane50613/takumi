---
"takumi": minor
---

# Add flow-root, self-position alignment, balanced flex wrapping, and `contain`

`display: flow-root` makes a box a block formatting context root. `align-items`, `align-self`, `justify-items` and `justify-self` take `self-start` and `self-end`. `flex-wrap` takes `balance`, paired with the new `flex-line-count` longhand. `contain` takes `none`, `strict`, `content`, and any of `size`, `inline-size`, `layout`, `style` and `paint`, of which `layout` and `paint` are the two that affect layout.
