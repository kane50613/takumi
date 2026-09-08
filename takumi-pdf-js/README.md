<div align="center">
  <img src="https://takumi.kane.tw/logo.svg" alt="Takumi" width="64" />

# takumi-pdf

**Render paged PDFs from JSX, HTML, and CSS. No headless browser.**

Build invoices and reports with CSS or Tailwind. The renderer writes vector PDF with selectable text.

[PDF documentation](https://takumi.kane.tw/docs/pdf) · [Playground](https://takumi.kane.tw/playground)

</div>

## How it works

`takumi-pdf` runs Takumi's layout and PDF engine in WebAssembly on Node.js, Bun, and Cloudflare Workers. It does not launch a browser process.

Pass JSX, an HTML string, or a Takumi node tree with CSS. The renderer returns vector PDF bytes with searchable text and embedded subset fonts.

## Install

```bash
npm install takumi-pdf @takumi-rs/helpers
# or
bun add takumi-pdf @takumi-rs/helpers
```

## Quick start

```tsx
import { writeFile } from "node:fs/promises";
import { googleFonts } from "@takumi-rs/helpers";
import { render } from "takumi-pdf";
import { PageNumber, TotalPages } from "takumi-pdf/primitives";

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
        Page <PageNumber /> of <TotalPages />
      </div>
    ),
  },
);

await writeFile("invoice.pdf", pdf);
```

`render()` returns `Promise<Uint8Array>`. Paged output defaults to A4, and `margin` defaults to `"auto"` on all sides. Without headers or footers, margins are 37.8px. Top and bottom margins can expand to fit header or footer bands. Content flows across pages automatically.

## Choose the next guide

| Task                                                | Guide                                                                                                                               |
| --------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------- |
| Set page size, margins, and runtime imports         | [PDF getting started](https://takumi.kane.tw/docs/pdf)                                                                              |
| Control page breaks and keep content together       | [Pagination](https://takumi.kane.tw/docs/pdf/pagination)                                                                            |
| Reserve space for repeated headers and page numbers | [Headers and footers](https://takumi.kane.tw/docs/pdf/headers-and-footers)                                                          |
| Create a certificate, ticket, or receipt            | [Single-page output](https://takumi.kane.tw/docs/pdf/single-page)                                                                   |
| Load fonts and remote image bytes                   | [Fonts and images](https://takumi.kane.tw/docs/pdf/fonts-and-images)                                                                |
| Add hyperlinks, bookmarks, and a table of contents  | [Links and metadata](https://takumi.kane.tw/docs/pdf/links-and-outline)                                                             |
| Configure archival or accessible output             | [PDF/A and PDF/UA](https://takumi.kane.tw/docs/pdf/pdf-a)                                                                           |
| Embed invoice XML or another file                   | [Attachments](https://takumi.kane.tw/docs/pdf/attachments)                                                                          |
| Replace a browser or document renderer              | [From Puppeteer](https://takumi.kane.tw/docs/pdf/from-puppeteer) · [From react-pdf](https://takumi.kane.tw/docs/pdf/from-react-pdf) |

## Before using an existing template

- **Fonts:** Takumi does not read system fonts. Register fonts that cover your text.
- **Images:** Fetch remote document images yourself and pass their bytes through `images`.
- **Page geometry:** Use `size`, `landscape`, and `margin`. CSS `@page` rules are not supported.
- **Effects:** PDF output rejects CSS `filter: blur()`, `drop-shadow()`, and `backdrop-filter`. Prepare those effects as images before rendering.
- **Validation:** Tagged PDF is enabled by default. Choose the conformance options your document needs and validate the result.

See the [PDF renderer comparison](https://takumi.kane.tw/docs/pdf/comparison) for recorded benchmarks and rendering differences. Runnable examples cover [invoices and receipts](https://github.com/kane50613/takumi/tree/master/example/generate-invoice) and [Factur-X invoices](https://github.com/kane50613/takumi/tree/master/example/e-invoice).

## License

MIT or Apache-2.0
