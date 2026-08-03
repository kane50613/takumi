# takumi-pdf

Render JSX to paged PDF in WebAssembly. No Chromium or native binary.

## Install

```bash
npm install takumi-pdf
# or
bun add takumi-pdf
```

## Quick start

```tsx
import { render } from "takumi-pdf";
import { writeFile } from "node:fs/promises";

const pdf = await render(<h1>Invoice #1042</h1>);

await writeFile("invoice.pdf", pdf);
```

`render()` returns `Uint8Array` PDF bytes. Output is paged A4 with a 48px margin by default; content flows onto as many pages as it needs. Text stays selectable and searchable, with fonts subset and embedded.

See the [Takumi documentation](https://takumi.kane.tw/docs/) for the shared node and JSX model.

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

Headers and footers repeat on every page. Elements with `pageNumber` or `totalPages` in their class list receive the counter as text, the same contract as Chromium print templates.

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

`viewport` cannot be combined with `size`, `landscape`, `margin`, `header`, or `footer`.

## Fonts

Pass a font URL or font bytes. Registered fonts are deduplicated across calls.

```tsx
const pdf = await render(doc, {
  fonts: [
    "https://example.com/Inter-Regular.woff2",
    { name: "Brand Sans", weight: 700, data: fontBytes },
  ],
  fontFamilies: ["Brand Sans", "sans-serif"],
});
```

Reuse a `PdfRenderer` when an application renders many documents:

```tsx
import { PdfRenderer } from "takumi-pdf";

const renderer = new PdfRenderer();
await renderer.registerFont("https://example.com/Inter-Regular.woff2");

const pdf = await renderer.render(doc);
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
