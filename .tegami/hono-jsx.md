---
packages:
  "@takumi-rs/helpers": minor
  takumi-js: patch
  takumi-pdf: patch
---

# Render Hono JSX and `html` templates

`render`, `ImageResponse`, and takumi-pdf's `render` draw Hono JSX and Hono's `html` template as Hono's `c.html()` would send them.

- Context reaches its consumers, and `Suspense` and `ErrorBoundary` behave as they do in Hono.
- A JSX element whose tag is `image`, `text`, or `container`, such as an SVG `<image>`, is no longer taken for a node tree.

```tsx
/** @jsxImportSource hono/jsx */
import { Suspense } from "hono/jsx/streaming";
import { ImageResponse } from "takumi-js/response";

const Title = async () => <h1>{await getTitle()}</h1>;

new ImageResponse(
  <Suspense fallback={<h1>Loading</h1>}>
    <Title />
  </Suspense>,
);
// Before: draws the markup as text, one character per node
// After: draws the title
```
