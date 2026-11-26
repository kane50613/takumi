import { existsSync, readFileSync } from "node:fs";
import { inflateSync } from "node:zlib";
import { describe, expect, test } from "bun:test";
import { container, text } from "@takumi-rs/helpers";
import { Renderer, type DocumentInput, type PdfRenderOptions } from "../src/export";

const renderer = new Renderer();

const decoder = new TextDecoder("latin1");

function pdfHeader(pdf: Uint8Array): string {
  return decoder.decode(pdf.subarray(0, 5));
}

function pageCount(pdf: Uint8Array): number {
  const match = decoder.decode(pdf).match(/\/Count (\d+)/);

  return match ? Number(match[1]) : 0;
}

/** The first page's width and height in pt. */
function mediaBox(pdf: Uint8Array): [number, number] {
  const box = decoder.decode(pdf).match(/\/MediaBox\s*\[0 0 ([\d.]+) ([\d.]+)\]/);

  return [Number(box?.[1]), Number(box?.[2])];
}

/** The decoded contents of every Flate stream with a direct `/Length`. */
function flateStreams(pdf: Uint8Array): string[] {
  const streams: string[] = [];

  for (const match of decoder.decode(pdf).matchAll(/\/Length (\d+)[^]*?stream\r?\n/g)) {
    const start = match.index + match[0].length;

    try {
      streams.push(decoder.decode(inflateSync(pdf.subarray(start, start + Number(match[1])))));
    } catch {
      continue;
    }
  }

  return streams;
}

/** The bytes of the PDF string literals in `content`, with their escapes read. */
function literalStrings(content: string): string[] {
  const strings: string[] = [];
  const escapes: Record<string, string> = { n: "\n", r: "\r", t: "\t", b: "\b", f: "\f" };

  for (let index = 0; index < content.length; index++) {
    if (content[index] !== "(") {
      continue;
    }

    let value = "";
    let depth = 1;

    for (index++; index < content.length && depth > 0; index++) {
      const char = content[index] ?? "";

      if (char === "\\") {
        const octal = content.slice(index + 1).match(/^[0-7]{1,3}/)?.[0];

        if (octal) {
          value += String.fromCharCode(Number.parseInt(octal, 8));
          index += octal.length;
        } else {
          const escaped = content[index + 1] ?? "";

          value += escapes[escaped] ?? escaped;
          index++;
        }
        continue;
      }
      if (char === "(") {
        depth++;
      } else if (char === ")") {
        depth--;
        if (depth === 0) {
          break;
        }
      }
      value += char;
    }
    strings.push(value);
  }

  return strings;
}

/** The text a PDF's pages show, read through its `ToUnicode` maps. Reads documents set in one font. */
function pdfText(pdf: Uint8Array): string {
  const streams = flateStreams(pdf);
  const unicode = new Map<number, string>();

  for (const cmap of streams.filter((stream) => stream.includes("beginbfchar"))) {
    for (const [, code, target] of cmap.matchAll(/<([0-9A-Fa-f]{4})>\s*<([0-9A-Fa-f]+)>/g)) {
      const units = (target ?? "").match(/.{4}/g) ?? [];

      unicode.set(
        Number.parseInt(code ?? "", 16),
        String.fromCharCode(...units.map((unit) => Number.parseInt(unit, 16))),
      );
    }
  }

  return streams
    .filter((stream) => /\bTJ\b|\bTj\b/.test(stream))
    .flatMap((stream) => literalStrings(stream))
    .map((codes) => {
      let shown = "";

      for (let index = 0; index + 1 < codes.length; index += 2) {
        shown += unicode.get(codes.charCodeAt(index) * 256 + codes.charCodeAt(index + 1)) ?? "";
      }

      return shown;
    })
    .join("");
}

function rows(count: number) {
  return container({
    style: { display: "flex", flexDirection: "column", width: "100%" },
    children: Array.from({ length: count }, (_, i) => text(`Row ${i + 1}`, { fontSize: 16 })),
  });
}

const doc = container({
  style: { display: "flex", width: "100%", height: "100%", backgroundColor: "#ffffff" },
  children: [text("Hello PDF", { fontSize: 32 })],
});

const twoPages = {
  size: { width: 400, height: 300 },
  margin: 24,
  footer: container({
    style: { display: "flex", columnGap: 3, fontSize: 12 },
    children: [text("Page"), container({ className: "pageNumber" }), text("of")],
  }),
} satisfies PdfRenderOptions;

test("renders a single fixed page", async () => {
  const pdf = await renderer.renderPdf(doc, { viewport: { width: 600, height: 300 } });

  expect(pdf).toBeInstanceOf(Uint8Array);
  expect(pdfHeader(pdf)).toBe("%PDF-");
  expect(pageCount(pdf)).toBe(1);
  expect(mediaBox(pdf)).toEqual([450, 225]);
  expect(pdfText(pdf)).toBe("Hello PDF");
});

test("defaults to paged A4 without options", async () => {
  const pdf = await renderer.renderPdf(doc);

  expect(pageCount(pdf)).toBe(1);
  expect(mediaBox(pdf).map(Math.round)).toEqual([595, 842]);
});

test("renders an HTML string with its own stylesheet", async () => {
  const pdf = await renderer.renderPdf(
    `<style>.title { font-size: 32px }</style><div class="title">Hello PDF</div>`,
    { viewport: { width: 600, height: 300 } },
  );

  expect(pageCount(pdf)).toBe(1);
  expect(pdfText(pdf)).toBe("Hello PDF");
});

test("paginates and fills the footer's page counters", async () => {
  const pdf = await renderer.renderPdf(rows(20), twoPages);
  const shown = pdfText(pdf);

  expect(pageCount(pdf)).toBe(2);
  expect(mediaBox(pdf)).toEqual([300, 225]);
  expect(shown).toContain("Row 1");
  expect(shown).toContain("Row 20");
  expect(shown).toContain("Page1of");
  expect(shown).toContain("Page2of");
});

test("parses an HTML footer", async () => {
  const pdf = await renderer.renderPdf(rows(20), {
    ...twoPages,
    footer: `<style>.f { font-size: 12px }</style><div class="f">Footer</div>`,
  });

  expect(pdfText(pdf).match(/Footer/g)).toHaveLength(2);
});

test("renders a prepared document as the unprepared one", async () => {
  const prepared = await renderer.preparePdf(rows(20), twoPages);

  expect(await renderer.renderPdf(prepared)).toEqual(await renderer.renderPdf(rows(20), twoPages));
});

test("rejects a viewport with paged options", async () => {
  const mixed = JSON.parse('{ "viewport": { "width": 600, "height": 300 }, "size": "a4" }');

  await expect(renderer.renderPdf(doc, mixed)).rejects.toThrow("viewport is mutually exclusive");
});

const wasmBuilt = existsSync(
  new URL("../../takumi-pdf-js/pkg/takumi_pdf_wasm_bg.wasm", import.meta.url),
);

describe.skipIf(!wasmBuilt)("matches takumi-pdf", async () => {
  const { PdfRenderer } = await import("takumi-pdf");
  const wasm = new PdfRenderer();
  const cases: [string, DocumentInput, PdfRenderOptions][] = [
    ["a fixed page", doc, { viewport: { width: 600, height: 300 } }],
    ["paged A4", doc, {}],
    ["two pages with a footer", rows(20), twoPages],
    [
      "HTML",
      `<style>.title { font-size: 32px }</style><div class="title">Hello PDF</div>`,
      { size: "letter", metadata: { title: "Hello", creationDate: "2026-01-02" } },
    ],
  ];

  for (const [name, input, options] of cases) {
    test(name, async () => {
      const native = await renderer.renderPdf(input, options);
      const web = await wasm.render(input, options);

      expect(pageCount(native)).toBe(pageCount(web));
      expect(mediaBox(native)).toEqual(mediaBox(web));
      expect(pdfText(native)).toBe(pdfText(web));
      expect(new Uint8Array(native)).toEqual(web);
    });
  }
});

test("reads a registered font's text", async () => {
  const font = readFileSync(
    new URL("../../assets/fonts/noto-sans/NotoSansTC-VariableFont_wght.woff2", import.meta.url),
  );
  const local = new Renderer();

  await local.registerFont({ name: "Noto Sans TC", data: font });

  const pdf = await local.renderPdf(text("你好", { fontFamily: "Noto Sans TC" }), {
    viewport: { width: 200, height: 100 },
  });

  expect(pdfText(pdf)).toBe("你好");
});
