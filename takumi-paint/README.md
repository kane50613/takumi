<div align="center">
  <img src="https://takumi.kane.tw/logo.svg" alt="Takumi" width="64" />

# takumi-paint

**Turn JSX, HTML, and CSS into a paint tree.**

Boxes, images, and text runs in paint order, with the values the renderer used. Feed it to a PPTX, Canvas, or native drawing API.

[Documentation](https://takumi.kane.tw/docs/paint-tree)

</div>

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

## What the tree holds

`takumi-paint` runs Takumi's layout in WebAssembly and walks the same stacking-context scene the image, SVG, and PDF backends paint. Instead of drawing, it records what each box would draw:

- **Box decoration**: the used background, border, shadows, and outline.
- **Image**: where a replaced image lands after `object-fit`.
- **Text runs**: every shaped run with the font, size, and color it was shaped with.

## Coordinates

- Every length is a device pixel.
- A node's `transform` is absolute.
- Everything inside a node is relative to its border box.

## Resolved and unresolved values

| Value                                                  | Form in the tree                   |
| ------------------------------------------------------ | ---------------------------------- |
| Colors                                                 | `[r, g, b, a]`                     |
| Linear and radial gradients                            | Resolved stops and geometry        |
| Conic gradients                                        | CSS text                           |
| `filter`, `backdrop-filter`, `mask-image`, `clip-path` | CSS text under `unresolvedEffects` |

The [paint tree reference](https://takumi.kane.tw/docs/paint-tree) lists every field.
