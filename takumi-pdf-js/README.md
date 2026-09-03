<div align="center">
  <img src="https://takumi.kane.tw/logo.svg" alt="Takumi" width="64" />

# takumi-pdf

**Render paged PDFs from JSX, HTML, and CSS. No headless browser.**

Build invoices and reports with CSS or Tailwind. The renderer writes vector PDF with selectable text.

[Documentation](https://takumi.kane.tw/docs/) · [Playground](https://takumi.kane.tw/playground)

</div>

## Why

Browser-based PDF generation starts a browser process and ships a Chrome installation. `takumi-pdf` compiles Takumi's layout and PDF code to WebAssembly.

Pass JSX or a Takumi node tree with CSS. The renderer returns vector PDF bytes with searchable text and embedded subset fonts.

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

Headers and footers repeat on every page. `<PageNumber />` and `<TotalPages />` place the counters; the `format` prop picks a CSS counter style.

```tsx
import { render } from "takumi-pdf";
import { PageNumber, TotalPages } from "takumi-pdf/primitives";

const pdf = await render(report, {
  footer: (
    <div style={{ fontSize: 12 }}>
      第 <PageNumber format="trad-chinese-informal" /> 頁,共{" "}
      <TotalPages format="trad-chinese-informal" /> 頁
    </div>
  ),
});
```

The primitives render class hooks, the same `pageNumber` / `totalPages` names Chromium's print templates use, so HTML input writes `<span class="pageNumber"></span>` directly.

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
import { render } from "takumi-pdf";

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

## Tables

`<table>` markup lays out on shared column tracks, so column x positions stay identical across pages. A `<thead>` paints again at the top of every page its table continues onto, when it is at most a quarter of the page tall and no header cell spans into the body.

```tsx
const pdf = await render(
  <table>
    <thead>
      <tr>
        <th>Name</th>
        <th>Qty</th>
      </tr>
    </thead>
    <tbody>{rows}</tbody>
  </table>,
);
```

## Links, outline, and metadata

Anchors with an `href` become clickable link annotations. `<TargetPageNumber />` prints the page a link's target lands on, which is what a table of contents needs. `outline: true` builds PDF bookmarks from `h1` through `h6` headings. `metadata` fills the document properties:

```tsx
const pdf = await render(report, {
  outline: true,
  lang: "en",
  metadata: {
    title: "Annual report 2026",
    authors: ["Acme Inc."],
    creationDate: "2026-08-06",
  },
});
```

Omit `metadata` to keep output byte-identical across runs.

## Tagged output and PDF/A

Output is **tagged by default**: HTML semantics (`h1` through `h6`, `p`, `img` with `alt`, `a`, lists) become a PDF structure tree, like Chromium's print-to-PDF. Set `tagged: "ua1"` to validate against PDF/UA-1, or `tagged: false` to drop the tree when file size matters more than accessibility.

`<table>` markup lays out and paints, but carries no `Table` structure elements yet.

`pdfa` renders archival output. Validation runs during rendering. A document that cannot conform fails with the violated rule instead of writing a broken file. Every level, and PDF/UA-1, passes [veraPDF](https://verapdf.org).

```tsx
const pdf = await render(report, {
  pdfa: "2a",
  tagged: "ua1",
  lang: "en",
  metadata: { title: "Annual report", creationDate: "2026-08-06" },
});
```

| Level                    | What it adds                          |
| ------------------------ | ------------------------------------- |
| `"2b"` / `"2u"`          | Basic conformance / Unicode mapping.  |
| `"2a"` / `"3a"`          | A tagged structure tree.              |
| `"3b"` / `"3u"` / `"3a"` | Arbitrary file attachments.           |
| `"4"`                    | The PDF 2.0 revision of the standard. |
| `"4f"`                   | PDF 2.0 with file attachments.        |

Invalid combinations are **TypeScript type errors**. See the [PDF/A docs](https://takumi.kane.tw/docs/pdf/pdf-a) for the structure-tree mapping and required metadata.

## Attachments

Attach files with `attachments`. They appear in the viewer's attachment panel. Combine with `pdfa: "3b"` for ZUGFeRD and Factur-X electronic invoices:

```tsx
const pdf = await render(invoice, {
  pdfa: "3b",
  metadata: { title: "Invoice 1042", creationDate: "2026-08-06" },
  attachments: [
    {
      name: "factur-x.xml",
      data: xml,
      mimeType: "application/xml",
      description: "Factur-X invoice data",
      relationship: "alternative",
    },
  ],
});
```

The PDF/A-3 levels require `mimeType`, `description`, and a modification date on each attachment. `metadata.creationDate` serves as the date fallback.

## Fillable fields

`form: true` turns `<input>` and `<textarea>` into AcroForm fields a reader can fill in. Left off, the same markup draws as the static boxes its CSS describes, so one template covers both the form and the printed copy.

```tsx
const pdf = await render(
  <form>
    <label htmlFor="name">Full name</label>
    <input id="name" name="name" defaultValue="Kane" required />

    <label>
      Notes
      <textarea name="notes" maxLength={200} />
    </label>
  </form>,
  { form: true },
);
```

Fields come from the HTML attributes you already write.

| HTML                                                                   | PDF                                                       |
| ---------------------------------------------------------------------- | --------------------------------------------------------- |
| `name`                                                                 | The field name                                            |
| `value`, a `<textarea>`'s text                                         | The value the field starts and resets to                  |
| `required`, `readonly`, `disabled`                                     | Field flags. A disabled field stays out of the submission |
| `maxlength`                                                            | The longest value a reader may type                       |
| `<input type="password">`                                              | A password field, drawn masked                            |
| `aria-label`, `aria-labelledby`, `<label>`, `title`, `placeholder`     | The name a screen reader announces                        |
| `color`, `font-size`, `background-color`, `border-color`, `text-align` | How a reader redraws the field after an edit              |

A control paints through the normal CSS pipeline, so its border, background and radius are whatever the stylesheet says. The widget draws only the value on top. Once a reader edits that value, it redraws the field from the colors and size in the table above, and a rounded corner or a gradient does not survive.

Two controls may not share a `name`. That fails the render rather than merging into one field showing the same value twice. A period in a `name` spells the field hierarchy: `user.name` is the field `name` under `user`.

A control taller than a page keeps its field on the page it starts on, cut at that page's bottom.

An `<input>` whose `type` is `submit`, `reset`, `button`, `image`, `file` or `hidden` draws as a plain box and gets no field.

Field values draw with the standard Helvetica face in WinAnsiEncoding, which no PDF/A or PDF/UA level accepts unembedded. A character outside that encoding is dropped from the drawn value; `/V` still carries it in full.

## Measuring

`measure()` lays out a tree without rendering and returns its size in CSS px. Use it to size a header or footer band before setting `margin`:

```tsx
import { measure } from "takumi-pdf";

const { height } = await measure(footer, { size: "a4" });
```

## Images and runtimes

`takumi-pdf` runs on Node.js, Bun, and Cloudflare Workers.

SVG images embed as vectors, not rasterized bitmaps.

The renderer does not fetch remote images. Pass pre-fetched bytes for image URLs in the document:

```tsx
const pdf = await render(doc, {
  images: [{ src: "https://example.com/logo.png", data: logoBytes }],
});
```

`@page` CSS rules are not supported. Set page geometry with `size`, `landscape`, and `margin`.

## License

MIT or Apache-2.0
