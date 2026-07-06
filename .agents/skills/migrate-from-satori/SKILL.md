---
name: migrate-from-satori
description: Guides and automated instructions for migrating from Vercel's Satori, next/og, or @vercel/og to Takumi.
---

# Migrate Satori / next/og to Takumi

Upgrade instructions. Replace Satori layout limitations with Takumi advanced features.

## Feature Differences

| Feature        | Satori                           | Takumi                                                                   |
| -------------- | -------------------------------- | ------------------------------------------------------------------------ |
| Primary Output | SVG only (needs `resvg` wrapper) | PNG, JPEG, WebP, SVG, GIF, APNG                                          |
| CSS Layout     | Flexbox only                     | Flexbox, CSS Grid, block, inline, float                                  |
| Advanced CSS   | ✗                                | Grid, pseudo-elements, backdrop-filter, clip-path, text-fit, offset-path |
| Fonts          | Manual Buffer array              | System fonts, URLs, local paths, Buffers                                 |
| Tailwind       | Limited / wrapper needed         | Native Tailwind v4 (`tw`/`class`, arbitrary values)                      |

## Code Changes

### Next.js ImageResponse

Before:

```tsx
import { ImageResponse } from "next/og";
// ... returns new ImageResponse(...)
```

After:

```tsx
import { ImageResponse } from "takumi-js/response";
// ... returns new ImageResponse(...)
```

### Direct Rendering

Before:

```typescript
import satori from "satori";
import { Resvg } from "@resvg/resvg-js";
const svg = await satori(element, { width, height, fonts: [{ name: "A", data: buf }] });
const png = new Resvg(svg).render().asPng();
```

After:

```typescript
import { render } from "takumi-js";
const png = await render(element, {
  width,
  height,
  fonts: [{ name: "A", url: "https://url.woff2" }],
});
```

## Upgrades to Apply

- **CSS Grid**: Convert complex flex layouts to `tw="grid grid-cols-12 gap-4"`.
- **CSS Variables & Custom Selectors**: Embedded `<style>` rules, `:is()`, `:where()`, `::before`, `::after` work natively.
- **Auto Font Handling**: Drop manual loading scripts. Rely on system fonts or CDN URLs.
- **Advanced Visuals**: Use `backdrop-filter`, `clip-path`, and blend modes directly in styling.
