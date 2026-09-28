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

`paint()` lays out the document. The tree iterates its boxes in paint order, so drawing is one loop.

```tsx
import { paint, type Rgba } from "takumi-paint";

const tree = await paint(
  <div style={{ width: 640, padding: 32, background: "#F7F3EC", flexDirection: "column", gap: 8 }}>
    <div style={{ fontSize: 40, fontWeight: 700, color: "#B3261E" }}>Hello paint</div>
    <div>Every box and text run, with the values the renderer used.</div>
  </div>,
  { width: 640 },
);

function draw(ctx: CanvasRenderingContext2D) {
  for (const node of tree) {
    ctx.setTransform(...node.matrix);
    if (node.background?.color) {
      ctx.fillStyle = rgba(node.background.color);
      ctx.fillRect(0, 0, node.width, node.height);
    }
    for (const run of node.textRuns) {
      ctx.save();
      if (run.transform) ctx.transform(...run.transform);
      ctx.font = `${run.font.weight} ${run.fontSize}px ${run.font.family}`;
      ctx.fillStyle = rgba(run.color);
      ctx.fillText(run.text, run.x, run.y);
      ctx.restore();
    }
  }
  ctx.resetTransform();
}

const rgba = ([r, g, b, a]: Rgba) => `rgb(${r} ${g} ${b} / ${a / 255})`;
```

## What the tree holds

`takumi-paint` runs Takumi's layout in WebAssembly and walks the same stacking-context scene the image, SVG, and PDF backends paint. Instead of drawing, it records what each box would draw:

- `background`, `border`, `shadows`, `outline`: the used decorations of each box.
- `image`: where a replaced image lands after `object-fit`.
- `textRuns`: every shaped run with the font, size, and color it was shaped with.

## Coordinates

- Every length is a device pixel.
- A node's `x` and `y` place its border box on the canvas. `transform` appears only when the box is rotated, scaled, or skewed. `node.matrix` returns whichever of the two applies.
- Everything inside a node is relative to its border box.

## Resolved and unresolved values

| Value                                                  | Form in the tree                   |
| ------------------------------------------------------ | ---------------------------------- |
| Colors                                                 | `[r, g, b, a]`                     |
| Linear and radial gradients                            | Resolved stops and geometry        |
| Conic gradients                                        | CSS text                           |
| `filter`, `backdrop-filter`, `mask-image`, `clip-path` | CSS text under `unresolvedEffects` |

The [paint tree reference](https://takumi.kane.tw/docs/paint-tree/reference) lists every field.
