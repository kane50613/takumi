<div align="center">
  <img src="https://takumi.kane.tw/logo.svg" alt="Takumi" width="64" />

# takumi-paint

**Lay out JSX, HTML, and CSS. Get back what to draw instead of pixels.**

Every box, gradient, shadow, text run, and image, in paint order, with each CSS value resolved.

[Documentation](https://takumi.kane.tw/docs/paint) · [Reference](https://takumi.kane.tw/docs/paint/reference)

</div>

## What it is for

Takumi's other packages end in a PNG, an SVG, or a PDF. Use `takumi-paint` when you own the output:

- Write slides where text stays editable, such as PPTX.
- Draw onto Canvas, Skia, or a native drawing API.
- Read where layout put each box and line of text.

It runs the same layout as the image, SVG, and PDF renderers, in WebAssembly. Where they draw, it records the **paint tree**.

## Install

```bash
npm install takumi-paint
```

## Quick start

```tsx
import { paint } from "takumi-paint";

const tree = await paint(
  <div id="card" style={{ width: 240, padding: 16, background: "#F7F3EC" }}>
    Hello <b>world</b>
  </div>,
  { width: 320 },
);
```

`tree.nodes` lists every node. Each node lists what it draws:

```tsx
const card = tree.nodes.find((node) => node.type === "box" && node.element?.id === "card");
console.log(card?.drawables[0]); // { type: "fill", role: "background", shape: { type: "rect", … }, … }

for (const node of tree.nodes) {
  if (node.type !== "text") continue;
  for (const run of node.runs) console.log(run.text, run.font.family, run.fontSize);
}
```

`tree.steps` is the draw order. Replay it from the bottom up:

```ts
for (const step of tree.steps) {
  if (step.type !== "draw") continue; // begin-group, end-group, begin-clip, and end-clip wrap draws
  ctx.setTransform(...step.node.transform);
  for (const drawable of step.drawables) draw(ctx, drawable);
}
```

`paint()` also takes an HTML string or a Takumi node tree.

## What the tree holds

| Part      | Contents                                                                    |
| --------- | --------------------------------------------------------------------------- |
| Nodes     | `box`, `text`, and `image`, each placed on the canvas by its `transform`    |
| Drawables | Fills, strokes, shadows, glyph runs, and images, tagged with their CSS role |
| Paints    | Colors, gradients, images, and tiled patterns                               |
| Text runs | Text, font, size, line height, and baseline, plus glyphs and outlines       |
| Steps     | Draws, clips, and groups for opacity, blending, filters, and masks          |

Lengths are device pixels.

## Learn more

| Need                           | Read                                                     |
| ------------------------------ | -------------------------------------------------------- |
| See the tree, step by step     | [Paint](https://takumi.kane.tw/docs/paint)               |
| Draw nodes in order            | [Drawing](https://takumi.kane.tw/docs/paint/drawing)     |
| Place text or lay it out again | [Text](https://takumi.kane.tw/docs/paint/text)           |
| Check options and field types  | [Reference](https://takumi.kane.tw/docs/paint/reference) |

## License

MIT or Apache-2.0

Bundled third-party code is listed in the [third-party notices](https://github.com/kane50613/takumi/blob/master/THIRD-PARTY-NOTICES.md).
