---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Paint text above the backgrounds of the blocks around it

Within a stacking context, text now paints after the backgrounds of every in-flow block and after the floats, as CSS 2.1 Appendix E orders them. Text that overflows its block now stays above the next block's background, and a box's own text stays above its negative `z-index` children. Flex and grid items still paint whole, like inline blocks.
