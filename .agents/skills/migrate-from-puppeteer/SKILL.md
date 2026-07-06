---
name: migrate-from-puppeteer
description: Guides and automated instructions for migrating from Puppeteer, Playwright, or node-html-to-image to Takumi.
---

# Migrate Puppeteer / Playwright to Takumi

Replace resource-heavy headless browser screenshots with native Takumi rendering.

## Key Advantages

- **Resources**: 300MB+ RAM vs single function call.
- **Cold Starts**: Milliseconds vs seconds.
- **Edge Support**: Runs in Cloudflare Workers/Edge (WASM). Puppeteer cannot.
- **Packaging**: No Chromium binary size issues in serverless.

## Code Changes

### Static Image Generation

Before (Puppeteer):

```typescript
import puppeteer from "puppeteer";
const browser = await puppeteer.launch();
const page = await browser.newPage();
await page.setViewport({ width: 1200, height: 630 });
await page.setContent(`<html><body><h1>Hello</h1></body></html>`);
const buffer = await page.screenshot({ type: "png" });
await browser.close();
```

After (Takumi):

```typescript
import { render } from "takumi-js";
const buffer = await render(
  <div tw="w-full h-full flex items-center justify-center bg-black text-white">
    <h1 tw="text-6xl font-bold">Hello</h1>
  </div>,
  { width: 1200, height: 630 }
);
```

### Animation (GIF / WebP)

Before:
Capturing multiple page frames in a loop and encoding manually.
After:

```typescript
import { renderAnimation } from "takumi-js";
const buffer = await renderAnimation({
  width: 400,
  height: 400,
  fps: 30,
  format: "webp",
  scenes: [
    {
      durationMs: 1000,
      node: <div tw="animate-spin bg-blue-500 w-32 h-32" />,
    },
  ],
});
```

## Migration Constraints

- **No Client JS**: Takumi does not execute `<script>` blocks. Fetch data first, then render layout tree with static state.
- **CSS Support**: Standard layout rules (Flexbox, Grid, absolute/relative, pseudo-elements) work. Complex browser features (iframe embed, canvas rendering, custom scrollbars) are unsupported.
- **Fonts**: Declare fonts explicitly in `RenderOptions` instead of relying on default OS font lookups.
