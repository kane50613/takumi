<div align="center">
  <img src="https://takumi.kane.tw/logo.svg" alt="Takumi" width="64" />

# takumi-paint

**Turn JSX, HTML, and CSS into a paint tree.**

Boxes, images, and text runs with the values the renderer used, in paint order. Feed it to a PPTX, Canvas, or native drawing API.

[Documentation](https://takumi.kane.tw/docs/paint-tree)

</div>

## How it works

`takumi-paint` runs Takumi's layout in WebAssembly and walks the same stacking-context scene the image, SVG, and PDF backends paint. Instead of drawing, it records what each box would draw: the used background, border, shadows, and outline; where an image lands after `object-fit`; and every shaped text run with the face, size, and color it was shaped with.

Every length is a device pixel. A node's `transform` is absolute; everything inside a node is relative to its border box.

## Install

```bash
npm install takumi-paint @takumi-rs/helpers
```

## Quick start

```tsx
import { renderPaintTree, type PaintNode, type PaintTextRun } from "takumi-paint";

const tree = await renderPaintTree(
  `<style>.big { font: 700 40px Georgia; color: #B3261E }</style>
   <div style="width: 640px; padding: 32px; background: #F7F3EC">
     <div class="big">4,2 %</div>
     <p>hello <b>world</b></p>
   </div>`,
  { width: 640 },
);

function* runs(node: PaintNode): Iterable<PaintTextRun> {
  yield* node.runs ?? [];
  for (const child of node.children ?? []) yield* runs(child);
}

for (const run of runs(tree.root)) {
  const font = tree.fonts[run.fontIndex];
  console.log(run.text, font?.family, font?.weight, run.fontSize, run.color);
}
```

## What is resolved and what is not

Colors are `[r, g, b, a]`. Linear and radial gradients come with their resolved stops and geometry. A conic gradient is carried as CSS text. `filter`, `backdrop-filter`, `mask-image`, and `clip-path` are carried as CSS text under `unresolvedEffects`.
