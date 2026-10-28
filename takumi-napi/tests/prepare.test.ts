import { afterEach, describe, expect, mock, spyOn, test } from "bun:test";
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { Renderer as RendererInternal } from "../index";
import { Renderer } from "../src/export";

const renderer = new Renderer();
const html = `<style>.a { width: 4px; height: 4px; background-image: url(https://example.com/a.png) }</style><div class="a"></div>`;
const png = await renderer.render(
  { type: "container", style: { width: 2, height: 2, backgroundColor: "red" } },
  { width: 2, height: 2 },
);

const fontBytes = await readFile(
  join(import.meta.dir, "../../assets/fonts/geist/Geist[wght].woff2"),
);

/** A lazily loaded face of the `Prepared` family covering `range`. */
const subset = (key: string, range: [number, number]) => ({
  name: "Prepared",
  key: `prepare-${key}`,
  unicodeRange: [range],
  data: mock(() => fontBytes),
});

afterEach(() => {
  mock.restore();
});

describe("prepare", () => {
  test("fetches the images a stylesheet names and renders as a render given them", async () => {
    const fetch = mock(async (_url: string) => new Response(png));
    const prepared = await renderer.prepare(html, { width: 4, height: 4, images: { fetch } });

    expect(fetch.mock.calls.map(([url]) => url)).toEqual(["https://example.com/a.png"]);

    const direct = await renderer.render(html, {
      width: 4,
      height: 4,
      images: [{ src: "https://example.com/a.png", data: png }],
    });

    expect(Buffer.compare(await renderer.render(prepared), direct)).toBe(0);
  });

  test("hands a relative URL to a caller's fetch only", async () => {
    const relative = `<img src="/local.png" width="2" height="2">`;
    const fetch = mock(async (_url: string) => new Response(png));
    const global = spyOn(globalThis, "fetch");

    await renderer.prepare(relative, { width: 2, height: 2, images: { fetch } });
    await renderer.prepare(relative, { width: 2, height: 2, images: { throwOnError: false } });

    expect(fetch.mock.calls.map(([url]) => url)).toEqual(["/local.png"]);
    expect(global).not.toHaveBeenCalled();
  });

  test("encodes with the options render takes", async () => {
    const prepared = await renderer.prepare("<p>hi</p>", { width: 8, height: 8 });
    const jpeg = await renderer.render(prepared, { format: "jpeg", quality: 50 });

    expect([...jpeg.subarray(0, 2)]).toEqual([0xff, 0xd8]);
  });

  test("measures and draws an SVG of what it laid out", async () => {
    const prepared = await renderer.prepare("<p>hi</p>", { width: 40, height: 20 });

    expect(await renderer.measure(prepared)).toEqual(
      await renderer.measure("<p>hi</p>", { width: 40, height: 20 }),
    );
    expect(await renderer.renderSvg(prepared)).toStartWith("<svg");
  });

  test("loads the font subsets the text uses, again only on a renderer without them", async () => {
    const latin = subset("latin", [0x20, 0x7f]);
    const greek = subset("greek", [0x370, 0x3ff]);
    const own = new Renderer();
    const prepared = await own.prepare("<p>a</p>", {
      width: 20,
      height: 20,
      fonts: [latin, greek],
    });

    await own.render(prepared);

    expect(latin.data).toHaveBeenCalledTimes(1);
    expect(greek.data).not.toHaveBeenCalled();

    await new Renderer().render(prepared);

    expect(latin.data).toHaveBeenCalledTimes(2);
  });

  test("loads the font subset a fetched SVG image's text needs", async () => {
    const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20"><text y="15">Ж</text></svg>`;
    const fetch = mock(async (_url: string) => new Response(svg));
    const latin = subset("svg-latin", [0x20, 0x7f]);
    const cyrillic = subset("svg-cyrillic", [0x400, 0x4ff]);
    const greek = subset("svg-greek", [0x370, 0x3ff]);

    await new Renderer().prepare(
      '<img src="https://example.com/chart.svg" width="20" height="20">',
      {
        width: 20,
        height: 20,
        fonts: [latin, cyrillic, greek],
        images: { fetch },
      },
    );

    expect(cyrillic.data).toHaveBeenCalledTimes(1);
    expect(greek.data).not.toHaveBeenCalled();
  });

  test("loads only the family the stack reaches first for a character", async () => {
    const face = (name: string) => ({
      name,
      key: `stack-${name}`,
      unicodeRange: [[0x20, 0x7f]] as [number, number][],
      data: mock(() => fontBytes),
    });
    const first = face("First");
    const second = face("Second");

    await new Renderer().prepare('<p style="font-family: First, Second">Hello</p>', {
      width: 40,
      height: 20,
      fonts: [first, second],
    });

    expect(first.data).toHaveBeenCalledTimes(1);
    expect(second.data).not.toHaveBeenCalled();
  });

  test("loads a weight only when text uses it", async () => {
    const weight = (value: number) => ({
      name: "Weighted",
      key: `weight-${value}`,
      weight: value,
      unicodeRange: [[0x20, 0x7f]] as [number, number][],
      data: mock(() => fontBytes),
    });
    const regular = weight(400);
    const bold = weight(700);
    const renderer = new Renderer();
    const html = '<p style="font-family: Weighted">Hello</p>';

    await renderer.prepare(html, { width: 40, height: 20, fonts: [regular, bold] });

    expect(regular.data).toHaveBeenCalledTimes(1);
    expect(bold.data).not.toHaveBeenCalled();

    await renderer.prepare(`${html}<b style="font-family: Weighted">Hi</b>`, {
      width: 40,
      height: 20,
      fonts: [regular, bold],
    });

    expect(bold.data).toHaveBeenCalledTimes(1);
  });

  test("stops when its signal aborts", async () => {
    const controller = new AbortController();

    controller.abort();

    await expect(
      renderer.prepare("<p>hi</p>", { width: 8, height: 8, signal: controller.signal }),
    ).rejects.toThrow();
  });

  test("leaves a render without font subsets to read nothing ahead", async () => {
    const needs = spyOn(RendererInternal.prototype, "needs");

    await renderer.render("<p>hi</p>", { width: 8, height: 8 });

    expect(needs).not.toHaveBeenCalled();
  });
});
