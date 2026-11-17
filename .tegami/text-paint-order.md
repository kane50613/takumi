---
packages:
  takumi-core: minor
  takumi-raster: minor
  takumi-svg: minor
  takumi-pdf: minor
---

# `paint-order: stroke` draws `-webkit-text-stroke` under the text

On HTML text, `paint-order: stroke` or `stroke fill` now draws the stroke first and the fill on top, as Chrome does, so a thick outline no longer eats into the letters. Images, SVG output, PDFs, and the paint tree all follow it.

```css
h1 {
  -webkit-text-stroke: 8px black;
  paint-order: stroke;
}
```

- The fill and the stroke each paint over a whole run, as in Chrome. With `normal`, a letter's fill no longer covers the previous letter's stroke where they overlap.
- `PaintOrder::text_phases` returns the order, and `InlineBrush` carries the run's `paint_order`.
