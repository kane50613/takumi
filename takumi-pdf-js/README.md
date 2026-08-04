<div align="center">
  <img src="https://takumi.kane.tw/logo.svg" alt="Takumi" width="64" />

# takumi-pdf

**Render JSX to paged, selectable-text PDF. WebAssembly, no Chromium.**

Invoices, reports, and receipts from the Takumi renderer, with CSS layout and vector PDF output.

[Documentation](https://takumi.kane.tw/docs/) · [Playground](https://takumi.kane.tw/playground)

</div>

## Why

Browser-based PDF generation brings the Chromium serverless tax: browser cold starts, browser memory, a large browser binary, and headless-browser work before a document can render. It is a capable screenshot pipeline, but it is still a browser.

`takumi-pdf` uses Takumi's own layout and PDF rendering primitives, compiled to WebAssembly. There is no browser process. Render JSX or a Takumi node tree with CSS and Tailwind classes, then receive vector PDF bytes with selectable, searchable text and embedded subset fonts.

## Install

```bash
npm install takumi-pdf
# or
bun add takumi-pdf
```

## Quick start

```tsx
import { writeFile } from "node:fs/promises";
import { googleFonts } from "@takumi-rs/helpers";
import { render } from "takumi-pdf";

const pdf = await render(
  <main tw="flex flex-col gap-4">
    <h1 tw="text-2xl font-bold">Invoice INV-2026-001</h1>
    <div tw="flex justify-between border-t border-gray-200 pt-2 font-bold">
      <span>Total</span>
      <span>$1,250.00</span>
    </div>
  </main>,
  {
    size: "a4",
    fonts: await googleFonts(["Inter"]),
    footer: (
      <div tw="flex w-full justify-center text-[10px] text-gray-500">
        Page <span className="pageNumber" /> of <span className="totalPages" />
      </div>
    ),
  },
);

await writeFile("invoice.pdf", pdf);
```

`render()` returns `Promise<Uint8Array>`. Paged output defaults to A4 with a uniform 48px margin; content flows across as many pages as it needs.

## Page setup

```tsx
const pdf = await render(report, {
  size: "letter",
  landscape: true,
  margin: { top: 48, right: 32, bottom: 48, left: 32 },
});
```

| Option      | Type                                           | Default | Description                                                  |
| ----------- | ---------------------------------------------- | ------- | ------------------------------------------------------------ |
| `size`      | `"a4"`, `"letter"`, or `{ width, height }`     | `"a4"`  | Page size in CSS px at 96 dpi. Presets ignore case.          |
| `landscape` | `boolean`                                      | `false` | Swaps page width and height, including explicit sizes.       |
| `margin`    | `number` or `{ top?, right?, bottom?, left? }` | `48`    | A number applies to all sides. Missing object sides are `0`. |

## Headers and footers

Headers and footers repeat on every page. Elements whose class list includes `pageNumber` or `totalPages` receive the counter as text.

```tsx
const pdf = await render(report, {
  footer: (
    <div style={{ fontSize: 12 }}>
      Page <span className="pageNumber" /> of <span className="totalPages" />
    </div>
  ),
});
```

Add a CSS counter-style name to the class list to format the number:

```tsx
footer: (
  <div style={{ fontSize: 12 }}>
    第 <span className="pageNumber trad-chinese-informal" /> 頁,共{" "}
    <span className="totalPages trad-chinese-informal" /> 頁
  </div>
),
```

| Counter style                               | Example       |
| ------------------------------------------- | ------------- |
| `decimal` (default)                         | `12`          |
| `decimal-leading-zero`                      | `07`          |
| `lower-roman` / `upper-roman`               | `xii` / `XII` |
| `cjk-decimal`                               | `一二`        |
| `trad-chinese-informal` / `cjk-ideographic` | `十二`        |

## Single-page viewport

Use `viewport` for a fixed one-page PDF, such as a certificate or card. Percentage heights resolve against the viewport and overflow is clipped, like an image render.

```tsx
const pdf = await render(<div style={{ width: "100%", height: "100%" }}>Certificate</div>, {
  viewport: { width: 1123, height: 794 },
});
```

Omit `height` to size the single page to its content, like a thermal receipt. Percentage heights do not resolve there.

```tsx
const pdf = await render(receipt, { viewport: { width: 302 } });
```

`viewport` cannot be combined with `size`, `landscape`, `margin`, `header`, or `footer`.

## Fonts

Pass a font URL, font bytes, or the `googleFonts` helper. Registered fonts are deduplicated across calls.

```tsx
import { googleFonts } from "@takumi-rs/helpers";

const pdf = await render(doc, {
  fonts: [...(await googleFonts(["Inter"])), { name: "Brand Sans", weight: 700, data: fontBytes }],
  fontFamilies: ["Brand Sans", "Inter", "sans-serif"],
});
```

Reuse a `PdfRenderer` when an application renders many documents:

```tsx
import { PdfRenderer } from "takumi-pdf";

const renderer = new PdfRenderer();
await renderer.registerFont("https://example.com/Inter-Regular.woff2");

const pdf = await renderer.render(doc);
renderer.free();
```

## Pagination CSS

```tsx
const pdf = await render(
  <article>
    <h1 style={{ breakBefore: "page" }}>Chapter two</h1>
    <section style={{ breakInside: "avoid" }}>Keep this together.</section>
  </article>,
);
```

| Property                      | Effect                                                  |
| ----------------------------- | ------------------------------------------------------- |
| `break-before: page`          | Starts the element on a new page.                       |
| `break-after: page`           | Starts the following content on a new page.             |
| `break-inside: avoid`         | Keeps the element on one page when it fits.             |
| `box-decoration-break: clone` | Repeats borders and backgrounds on every page fragment. |

## Images and runtimes

`takumi-pdf` runs on Node.js, Bun, and Cloudflare Workers.

The renderer does not fetch remote images. Pass pre-fetched bytes for image URLs in the document:

```tsx
const pdf = await render(doc, {
  images: [{ src: "https://example.com/logo.png", data: logoBytes }],
});
```

`@page` CSS rules are not supported. Set page geometry with `size`, `landscape`, and `margin`.

## License

MIT or Apache-2.0
