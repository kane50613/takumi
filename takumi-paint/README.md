<div align="center">
  <img src="https://takumi.kane.tw/logo.svg" alt="Takumi" width="64" />

# takumi-paint

**Turn JSX, HTML, and CSS into a paint tree.**

Shapes, paints, text runs, and images, with every CSS value resolved. Feed it to a PPTX, Canvas, or native drawing API.

[Documentation](https://takumi.kane.tw/docs/paint-tree)

</div>

## Install

```bash
npm install takumi-paint @takumi-rs/helpers
```

## Quick start

```tsx
import { paint } from "takumi-paint";

const tree = await paint(
  `<style>.big { font: 700 40px Georgia; color: #B3261E }</style>
   <div id="card" style="width: 640px; padding: 32px; background: #F7F3EC">
     <div class="big">4,2 %</div>
     <p>hello <b>world</b></p>
   </div>`,
  { width: 640 },
);

[...tree].find((node) => node.element?.id === "card")?.drawables; // the background fill, with its resolved shape and color

for (const node of tree) {
  if (node.type !== "text") continue;
  for (const run of node.runs)
    console.log(run.text, run.font.family, run.font.weight, run.fontSize);
}

for (const step of tree.paintSteps()) {
  // draw, begin-group/end-group, and begin-clip/end-clip, in paint order
}
```

## What the tree holds

`takumi-paint` runs Takumi's layout in WebAssembly and walks the same stacking-context scene the image, SVG, and PDF backends paint. Instead of drawing, it records what each node would draw, with every CSS value resolved:

- **Shapes**: rectangles, rounded rectangles, and SVG path data.
- **Paints**: colors, gradients with sRGB stops, images, and tiled patterns.
- **Drawables**: fills, strokes, shadows, glyph runs, and images, each tagged with the role it plays.
- **Effects**: opacity, blend mode, filters, clip, and mask for each composited group.

Every length is a device pixel. A node's `transform` maps its local space, where its drawables sit, onto the canvas.

The [paint tree reference](https://takumi.kane.tw/docs/paint-tree/reference) covers the vocabulary and what stays unresolved.
