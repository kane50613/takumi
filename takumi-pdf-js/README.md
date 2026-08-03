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

Without options the document flows across A4 pages. Fixed single-page output takes `width`/`height`; paged output takes `page` (`"a4"`, `"letter"`, or `{ width, height, margin, landscape }`) plus repeating `header`/`footer` bands where text may use the `{page}` and `{pages}` placeholders.

Pagination honors `break-before: page`, `break-after: page`, `break-inside: avoid`, and `box-decoration-break`.

```ts
const pdf = await render(report, {
  page: "a4",
  footer: text("Page {page} of {pages}", { fontSize: 12 }),
  fonts: ["https://takumi.kane.tw/fonts/geist.woff2"],
});
```

Works on Node, Bun, and Cloudflare Workers. See the [documentation](https://takumi.kane.tw/docs/) for the shared node/JSX model.
