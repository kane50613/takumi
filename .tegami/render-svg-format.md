---
packages:
  "takumi-js": minor
---

# `render` and `ImageResponse` take `format: "svg"`

`render` returns the SVG document as UTF-8 bytes, the same SVG `renderSvg` draws. `ImageResponse` sends it with the `image/svg+xml` content type. The raster-only options `quality`, `lossless`, `dithering`, `drawDebugBorder`, and `devicePixelRatio` do not apply to SVG, so TypeScript rejects them next to `format: "svg"`.

`renderSvg` and `RenderSvgOptions` keep working and will be removed in v3.

#### Migration

```diff
- const svg = await renderSvg(card, { width: 1200, height: 630 });
+ const bytes = await render(card, { width: 1200, height: 630, format: "svg" });
+ const svg = new TextDecoder().decode(bytes);
```
