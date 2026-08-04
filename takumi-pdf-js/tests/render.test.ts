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

test("paginates and substitutes footer counters", async () => {
  const rows = container({
    style: { display: "flex", flexDirection: "column", width: "100%" },
    children: Array.from({ length: 60 }, (_, i) => text(`Row ${i + 1}`, { fontSize: 16 })),
  });
  const pdf = await renderer.render(rows, {
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

test("auto-height viewport sizes the page to content", async () => {
  const rows = container({
    style: { display: "flex", flexDirection: "column", width: "100%" },
    children: Array.from({ length: 40 }, (_, i) => text(`Row ${i + 1}`, { fontSize: 16 })),
  });
  const pdf = await renderer.render(rows, { viewport: { width: 300 } });

  expect(pageCount(pdf)).toBe(1);

  const box = decoder.decode(pdf).match(/\/MediaBox\s*\[0 0 ([\d.]+) ([\d.]+)\]/);
  const [width, height] = [Number(box?.[1]), Number(box?.[2])];

  expect(height).toBeGreaterThan(width);
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

test("rejects viewport combined with paged options", () => {
  expect(
    renderer.render(doc, {
      viewport: { width: 600, height: 300 },
      size: "a4",
    } as never),
  ).rejects.toThrow("mutually exclusive");
});
