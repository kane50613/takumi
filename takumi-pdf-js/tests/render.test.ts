import { readFileSync } from "node:fs";
import { expect, test } from "bun:test";
import { container, text } from "@takumi-rs/helpers";
import { PdfRenderer } from "../bundlers/node.mjs";

const renderer = new PdfRenderer();

const decoder = new TextDecoder("latin1");

function pageCount(pdf: Uint8Array): number {
  const match = decoder.decode(pdf).match(/\/Count (\d+)/);

  return match ? Number(match[1]) : 0;
}

const doc = container({
  style: { display: "flex", width: "100%", height: "100%", backgroundColor: "#ffffff" },
  children: [text("Hello PDF", { fontSize: 32 })],
});

test("renders a single fixed page", async () => {
  const pdf = await renderer.render(doc, { viewport: { width: 600, height: 300 } });

  expect(pdf).toBeInstanceOf(Uint8Array);
  expect(decoder.decode(pdf.subarray(0, 5))).toBe("%PDF-");
  expect(pageCount(pdf)).toBe(1);
});

test("defaults to paged A4 without options", async () => {
  const pdf = await renderer.render(doc);

  expect(decoder.decode(pdf.subarray(0, 5))).toBe("%PDF-");
  expect(pageCount(pdf)).toBe(1);
});

test("renders an HTML string with its own stylesheet", async () => {
  const pdf = await renderer.render(
    `<style>.title { font-size: 32px }</style><div class="title">Hello PDF</div>`,
    { viewport: { width: 600, height: 300 } },
  );

  expect(decoder.decode(pdf.subarray(0, 5))).toBe("%PDF-");
  expect(pageCount(pdf)).toBe(1);
});

// One red pixel per raster format, inline so the suite carries no binary
// fixtures. Each is `sharp({ create: { width: 1, height: 1, channels: 4,
// background: "#dc3c3c" } }).toFormat(format)`.
const pixels = {
  png: "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAACXBIWXMAAAPoAAAD6AG1e1JrAAAADUlEQVQImWO4Y2PzHwAFoAJUkZ/O4AAAAABJRU5ErkJggg==",
  jpeg: "/9j/2wBDAAYEBQYFBAYGBQYHBwYIChAKCgkJChQODwwQFxQYGBcUFhYaHSUfGhsjHBYWICwgIyYnKSopGR8tMC0oMCUoKSj/2wBDAQcHBwoIChMKChMoGhYaKCgoKCgoKCgoKCgoKCgoKCgoKCgoKCgoKCgoKCgoKCgoKCgoKCgoKCgoKCgoKCgoKCj/wAARCAABAAEDASIAAhEBAxEB/8QAFQABAQAAAAAAAAAAAAAAAAAAAAX/xAAUEAEAAAAAAAAAAAAAAAAAAAAA/8QAFQEBAQAAAAAAAAAAAAAAAAAABQf/xAAUEQEAAAAAAAAAAAAAAAAAAAAA/9oADAMBAAIRAxEAPwCIAAVt/9k=",
  webp: "UklGRjoAAABXRUJQVlA4IC4AAACQAQCdASoBAAEAAUAmJaACdLoAA5gA/urUf/cdgbrxb/9zgf92T/+2T/1IAAAA",
  gif: "R0lGODlhAQABAIAAAExpcdw8PCH5BAUAAAAALAAAAAABAAEAAAICTAEAOw==",
};

// The decoders are takumi-core features that `raster-images` forwards, and this
// build compiles takumi-core's defaults out: a format missing from that forward
// fails the whole render rather than dropping the image.
test.each(Object.entries(pixels))("embeds pre-fetched %s bytes", async (format, base64) => {
  const src = `https://example.test/pixel.${format}`;
  const pdf = await renderer.render(`<img src="${src}" style="width: 64px">`, {
    viewport: { width: 80, height: 80 },
    images: [{ src, data: new Uint8Array(Buffer.from(base64, "base64")) }],
  });
  const bytes = decoder.decode(pdf);

  expect(bytes).toContain("/Subtype/Image");
  // The XObject carries the decoded pixel, not a placeholder.
  expect(bytes).toContain("/Width 1");
  expect(bytes).toContain("/Height 1");
});

test("paginates and substitutes footer counters", async () => {
  // The bundled face is latin only, and `trad-chinese-informal` counts in
  // Chinese numerals. The face registered for it stays on a renderer of its
  // own so the other tests keep the bundled set.
  const counting = new PdfRenderer();

  await counting.registerFont(
    new Uint8Array(
      readFileSync(
        new URL("../../assets/fonts/noto-sans/NotoSansTC-VariableFont_wght.woff2", import.meta.url),
      ),
    ),
  );

  const rows = container({
    style: { display: "flex", flexDirection: "column", width: "100%" },
    children: Array.from({ length: 60 }, (_, i) => text(`Row ${i + 1}`, { fontSize: 16 })),
  });
  const pdf = await counting.render(rows, {
    size: { width: 400, height: 300 },
    margin: 24,
    footer: container({
      style: { display: "flex", columnGap: 3, fontSize: 12 },
      children: [
        text("Page"),
        container({ className: "pageNumber" }),
        text("of"),
        container({ className: "totalPages trad-chinese-informal" }),
      ],
    }),
  });

  expect(pageCount(pdf)).toBeGreaterThan(1);
});

test("keeps a face the page counter needs but the document never uses", async () => {
  // `fonts` are filtered by whether their range covers the render, and a
  // chinese counter is the only thing on the page outside latin.
  const pdf = await renderer.render(
    container({
      style: { display: "flex", flexDirection: "column", width: "100%" },
      children: Array.from({ length: 60 }, (_, i) => text(`Row ${i + 1}`, { fontSize: 16 })),
    }),
    {
      size: { width: 400, height: 300 },
      margin: 24,
      fonts: [
        {
          name: "Counter CJK",
          ranges: [[0x4e00, 0x9fff]],
          data: () =>
            new Uint8Array(
              readFileSync(
                new URL(
                  "../../assets/fonts/noto-sans/NotoSansTC-VariableFont_wght.woff2",
                  import.meta.url,
                ),
              ),
            ),
        },
      ],
      footer: container({
        style: { display: "flex", fontSize: 12 },
        children: [container({ className: "totalPages trad-chinese-informal" })],
      }),
    },
  );

  expect(pageCount(pdf)).toBeGreaterThan(1);
});

test("auto-height viewport sizes the page to content", async () => {
  const rows = container({
    style: { display: "flex", flexDirection: "column", width: "100%" },
    children: Array.from({ length: 40 }, (_, i) => text(`Row ${i + 1}`, { fontSize: 16 })),
  });
  const pdf = await renderer.render(rows, { viewport: { width: 300 } });

  expect(pageCount(pdf)).toBe(1);

  const box = decoder.decode(pdf).match(/\/MediaBox\s*\[0 0 ([\d.]+) ([\d.]+)\]/);
  const [width, height] = [Number(box?.[1]), Number(box?.[2])];

  // Page geometry is written in pt: 300px at 96 dpi is 225pt.
  expect(width).toBe(225);
  // 40 rows of 16px text need at least 40 line boxes of font-size height.
  expect(height).toBeGreaterThanOrEqual(40 * 16 * 0.75);
});

test("letter landscape", async () => {
  const pdf = await renderer.render(doc, { size: "letter", landscape: true });

  expect(pageCount(pdf)).toBe(1);
});

test("accepts case-insensitive presets and per-side margins", async () => {
  const pdf = await renderer.render(doc, {
    size: "A4" as "a4",
    margin: { top: 60, bottom: 40 },
  });

  expect(pageCount(pdf)).toBe(1);
});

test("rejects an unknown size keyword", () => {
  expect(renderer.render(doc, { size: "tabloid" as "a4" })).rejects.toThrow("unknown page size");
});

test("measures a band at full page width with counters filled", async () => {
  const footer = container({
    style: { display: "flex", columnGap: 3, fontSize: 12 },
    children: [text("Page"), container({ className: "pageNumber" })],
  });
  const size = await renderer.measure(footer, { size: { width: 400, height: 300 } });

  expect(size.width).toBe(400);
  expect(size.height).toBeGreaterThanOrEqual(12);
});

test("measure defaults to paged A4", async () => {
  const size = await renderer.measure(doc);

  // A4 at 96 dpi is 793.7px wide; the layout viewport truncates to 793.
  expect(size.width).toBe(793);
  expect(size.height).toBeGreaterThan(0);
});

test("rejects viewport combined with paged options", () => {
  expect(
    renderer.render(doc, {
      viewport: { width: 600, height: 300 },
      size: "a4",
    } as never),
  ).rejects.toThrow("mutually exclusive");
});

test("embeds attachments from string and Uint8Array data", async () => {
  const pdf = await renderer.render(doc, {
    pdfa: "3b",
    metadata: { title: "Invoice", creationDate: "2026-08-06" },
    attachments: [
      {
        name: "factur-x.xml",
        data: '<invoice total="1290"/>',
        mimeType: "application/xml",
        description: "Factur-X invoice data",
        relationship: "alternative",
      },
      {
        name: "readings.csv",
        data: new TextEncoder().encode("a,b\n1,2"),
        mimeType: "text/csv",
        description: "Raw readings",
      },
    ],
  });
  const bytes = decoder.decode(pdf);

  expect(bytes).toContain("factur-x.xml");
  expect(bytes).toContain("readings.csv");
  expect(bytes).toContain("/AFRelationship");
});

test("rejects duplicate attachment names", async () => {
  const attachment = { name: "dup.txt", data: "x" };

  await expect(renderer.render(doc, { attachments: [attachment, attachment] })).rejects.toThrow(
    "DuplicateAttachment",
  );
});

test("rejects an invalid attachment mime type", async () => {
  await expect(
    renderer.render(doc, { attachments: [{ name: "a.txt", data: "x", mimeType: "nope" }] }),
  ).rejects.toThrow("InvalidMimeType");
});

test("rejects an invalid attachment modificationDate", async () => {
  await expect(
    renderer.render(doc, {
      attachments: [{ name: "a.txt", data: "x", modificationDate: "yesterday" }],
    }),
  ).rejects.toThrow("modificationDate");
});
