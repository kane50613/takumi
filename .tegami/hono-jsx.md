---
packages:
  "@takumi-rs/helpers": minor
  takumi-js: patch
  takumi-pdf: patch
---

# Render Hono JSX and `html` templates

`render`, `ImageResponse`, and takumi-pdf's `render` take Hono JSX and Hono's `html` template, and draw what Hono's `c.html()` would send, so a `Suspense` boundary shows its children rather than its fallback. A JSX element whose tag is `image`, `text`, or `container`, such as an SVG `<image>`, is no longer taken for a node tree, and `@takumi-rs/helpers` exports `isNode` for that check.

```tsx
/** @jsxImportSource hono/jsx */
import { ImageResponse } from "takumi-js/response";

export const GET = () => new ImageResponse(<div style={{ fontSize: 64 }}>Hello</div>);
```
