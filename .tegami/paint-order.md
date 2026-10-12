---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Paint in CSS paint order, as Chrome does

- Positioned boxes and `z-index: auto` stacking contexts paint above later in-flow siblings. A `position: relative` box nudged over the next block stays visible.
- Floats paint above in-flow block backgrounds, and before the text they sit in.
- Text paints above every block background in its stacking context, so overflowing text stays visible.
- A box with `overflow: hidden` paints its text above later siblings' backgrounds.
- A positioned box whose containing block is outside a clip escapes the clip.
- A filtered box inside an `overflow: hidden` parent stays inside the parent.
- PDF applies a scaled or rotated clipping box's transform once, not twice.
