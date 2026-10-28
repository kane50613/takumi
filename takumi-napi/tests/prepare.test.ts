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
    const bytes = await readFile(
      join(import.meta.dir, "../../assets/fonts/geist/Geist[wght].woff2"),
    );
    const subset = (key: string, range: [number, number]) => ({
      name: "Prepared",
      key: `prepare-${key}`,
      unicodeRange: [range],
      data: mock(() => bytes),
    });
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
