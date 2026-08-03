# takumi-pdf

HTML/JSX to paged PDF with selectable, searchable text and embedded subset fonts. Runs takumi's layout engine and a vector PDF backend in WebAssembly — no Chromium, no native binaries.

```tsx
import { render } from "takumi-pdf";

const pdf = await render(
  <div style={{ display: "flex", flexDirection: "column", padding: 32 }}>
    <h1>Invoice #1042</h1>
  </div>,
);
```

Without options the document flows across A4 pages with a 48px (half-inch) margin. The options mirror Puppeteer's `page.pdf()`: `size` (`"a4"`, `"letter"`, or `{ width, height }` in px), `landscape`, `margin` (a number or `{ top, right, bottom, left }`), and repeating `header`/`footer` bands with the same page-number contract as Chromium print templates: elements classed `pageNumber` or `totalPages` receive the counter as text, with an optional CSS `@counter-style` name (`cjk-decimal`, `lower-roman`, ...) in the class list to format it. A fixed `viewport: { width, height }` renders one clipped page instead, where percentage heights resolve against the viewport — use it for single-page designs that rely on percentage heights, like certificates. CSS `@page` rules are not supported; page geometry comes from these options.

Pagination honors `break-before: page`, `break-after: page`, `break-inside: avoid`, and `box-decoration-break`.

```tsx
const pdf = await render(report, {
  size: "a4",
  footer: (
    <div style={{ fontSize: 12 }}>
      第 <span className="pageNumber trad-chinese-informal" /> 頁, page{" "}
      <span className="pageNumber" /> of <span className="totalPages" />
    </div>
  ),
  fonts: ["https://takumi.kane.tw/fonts/geist.woff2"],
});
```

Counter styles: `decimal` (default), `decimal-leading-zero`, `lower-roman`, `upper-roman`, `cjk-decimal`, `trad-chinese-informal`, and `cjk-ideographic`.

Works on Node, Bun, and Cloudflare Workers. See the [documentation](https://takumi.kane.tw/docs/) for the shared node/JSX model.
