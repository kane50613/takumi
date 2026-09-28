---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Paint in CSS paint order, as Chrome does

- Positioned boxes and stacking contexts at `z-index: auto` paint above later in-flow siblings, and floats above in-flow block backgrounds, following CSS 2.1 Appendix E. A `position: relative` box nudged over the next block no longer disappears under it.
- Text paints after the backgrounds of every in-flow block and float in its stacking context, so overflowing text stays above the next block's background. Flex and grid items still paint whole, like inline blocks.
- A float inside text paints before that text.
- A box with `overflow: hidden` paints its text above the backgrounds of later siblings. A positioned box whose containing block sits outside it escapes its clip, even under an `opacity` in between.
- A filtered box inside an `overflow: hidden` parent stays inside the parent's edges in the image output.
