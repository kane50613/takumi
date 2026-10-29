import { readFileSync } from "node:fs";
import { join } from "node:path";
import { expect, mock, test } from "bun:test";
import { PdfRenderer, prepare, render } from "takumi-pdf";

const renderer = new PdfRenderer();
const png = readFileSync(
  join(import.meta.dir, "../../assets/images/luma-cover-0dfbf65d-0f58-4941-947c-d84a5b131dc0.jpeg"),
);
const decoder = new TextDecoder("latin1");

test("fetches the images the document and its bands draw", async () => {
  const fetch = mock(async (_url: string) => new Response(png));
  const prepared = await renderer.prepare(
    '<style>.logo { width: 10px; height: 10px; background-image: url(https://example.com/body.png) }</style><div class="logo"></div>',
    {
      header: '<img src="https://example.com/header.png" width="10" height="10">',
      images: { fetch },
    },
  );

  expect(fetch.mock.calls.map(([url]) => url).sort()).toEqual([
    "https://example.com/body.png",
    "https://example.com/header.png",
  ]);
  expect(decoder.decode((await renderer.render(prepared)).subarray(0, 5))).toBe("%PDF-");
});

test("writes the file with the options render takes", async () => {
  const prepared = await prepare("<h1>Title</h1>");
  const pdf = await render(prepared, { outline: true });

  expect(decoder.decode(pdf)).toContain("/Outlines");
});

test("measures the document it prepared", async () => {
  const html = '<div style="width: 120px; height: 30px"></div>';

  expect(await renderer.measure(await renderer.prepare(html))).toEqual(
    await renderer.measure(html),
  );
});

test("stops when its signal aborts", async () => {
  const controller = new AbortController();

  controller.abort();

  await expect(renderer.prepare("<p>hi</p>", { signal: controller.signal })).rejects.toThrow();
});

test("a render stops when its signal aborts before reading the document", async () => {
  const data = mock(() => png);
  const controller = new AbortController();

  controller.abort();

  await expect(
    renderer.render("<p>hi</p>", {
      signal: controller.signal,
      images: [{ src: "https://example.com/a.jpeg", data }],
    }),
  ).rejects.toThrow();
  expect(data).not.toHaveBeenCalled();
});

test("loads the font subsets only the bands' text uses", async () => {
  const bytes = readFileSync(join(import.meta.dir, "../../assets/fonts/geist/Geist[wght].woff2"));
  const subset = (key: string, range: [number, number]) => ({
    name: "Band Subset",
    key,
    unicodeRange: [range],
    data: mock(() => bytes),
  });
  const cyrillic = subset("band-cyrillic", [0x400, 0x4ff]);
  const greek = subset("band-greek", [0x370, 0x3ff]);
  const armenian = subset("band-armenian", [0x530, 0x58f]);

  await new PdfRenderer().prepare("<p>a</p>", {
    footer: "<p>Ж</p>",
    pages: { first: { header: "<p>α</p>" } },
    fonts: [cyrillic, greek, armenian],
  });

  expect(cyrillic.data).toHaveBeenCalledTimes(1);
  expect(greek.data).toHaveBeenCalledTimes(1);
  expect(armenian.data).not.toHaveBeenCalled();
});
