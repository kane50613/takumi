import { describe, expect, mock, test } from "bun:test";
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { Renderer } from "../src/export";

const fontBytes = await readFile(
  join(import.meta.dir, "../../assets/fonts/geist/Geist[wght].woff2"),
);

const renderer = new Renderer();
const png = await renderer.render(
  { type: "container", style: { width: 2, height: 2, backgroundColor: "red" } },
  { width: 2, height: 2 },
);

/** A fetch that answers every URL with `png`, recording the URLs. */
const fetchPng = () => mock(async (_url: string) => new Response(png));

const fetched = (fetch: ReturnType<typeof fetchPng>) => fetch.mock.calls.map(([url]) => url).sort();

describe("needs", () => {
  test("reads images after the cascade", async () => {
    const html = `<style>
      .bg { background-image: url(https://example.com/bg.png) }
      .hidden { display: none }
      p { text-transform: uppercase }
      p::before { content: "→" }
    </style>
    <div class="bg"></div>
    <div class="hidden"><img src="https://example.com/hidden.png">z</div>
    <img src="https://example.com/img.png">
    <p>ab</p>`;
    const fetch = fetchPng();

    await renderer.prepare(html, { width: 100, height: 100, images: { fetch } });

    expect(fetched(fetch)).toEqual(["https://example.com/bg.png", "https://example.com/img.png"]);
  });

  test("reads a node tree the same way", async () => {
    const fetch = fetchPng();

    await renderer.prepare(
      {
        type: "container",
        style: { backgroundImage: "url(https://example.com/a.png)" },
        children: [{ type: "text", text: "hi" }],
      },
      { width: 10, height: 10, images: { fetch } },
    );

    expect(fetched(fetch)).toEqual(["https://example.com/a.png"]);
  });

  test("reads every scene of an animation, styled by every scene's <style>", async () => {
    const fetch = fetchPng();
    const prepared = await renderer.prepareAnimation({
      width: 10,
      height: 10,
      fps: 10,
      scenes: [
        {
          node: '<style>.b { width: 4px; height: 4px; background-image: url(https://example.com/b.png) }</style><div class="a"></div>',
          durationMs: 100,
        },
        { node: '<div class="b"></div><img src="https://example.com/c.png">', durationMs: 100 },
      ],
      images: { fetch },
    });

    expect(fetched(fetch)).toEqual(["https://example.com/b.png", "https://example.com/c.png"]);
    expect((await renderer.renderAnimation(prepared)).length).toBeGreaterThan(0);
  });

  test("loads the font subsets only a later scene uses", async () => {
    const subset = (key: string, range: [number, number]) => ({
      name: "Scene Subset",
      key: `scene-${key}`,
      unicodeRange: [range],
      data: mock(() => fontBytes),
    });
    const cyrillic = subset("cyrillic", [0x400, 0x4ff]);
    const greek = subset("greek", [0x370, 0x3ff]);

    await new Renderer().renderAnimation({
      width: 20,
      height: 20,
      fps: 10,
      scenes: [
        { node: "<p>a</p>", durationMs: 100 },
        { node: "<p>Ж</p>", durationMs: 100 },
      ],
      fonts: [cyrillic, greek],
    });

    expect(cyrillic.data).toHaveBeenCalledTimes(1);
    expect(greek.data).not.toHaveBeenCalled();
  });
});

describe("font subsets", () => {
  test("load for characters only a stylesheet adds", async () => {
    const bytes = await readFile(
      join(import.meta.dir, "../../assets/fonts/geist/Geist[wght].woff2"),
    );
    const latin = {
      name: "Subset",
      key: "latin",
      unicodeRange: [[0x20, 0x7f]] as [number, number][],
      data: mock(() => bytes),
    };
    const cyrillic = {
      name: "Subset",
      key: "cyrillic",
      unicodeRange: [[0x400, 0x4ff]] as [number, number][],
      data: mock(() => bytes),
    };
    const greek = {
      name: "Subset",
      key: "greek",
      unicodeRange: [[0x370, 0x3ff]] as [number, number][],
      data: mock(() => bytes),
    };

    await new Renderer().render("<div>a</div>", {
      width: 20,
      height: 20,
      css: ["div::before { content: 'Ж' }"],
      fonts: [latin, cyrillic, greek],
    });

    expect(latin.data).toHaveBeenCalledTimes(1);
    expect(cyrillic.data).toHaveBeenCalledTimes(1);
    expect(greek.data).not.toHaveBeenCalled();
  });
});

describe("@font-face", () => {
  test("prepare downloads the files the text reaches, moving past a failed src it retries later", async () => {
    const fontRenderer = new Renderer();
    const fetch = mock(async (url: string) =>
      url.endsWith("missing.woff2") ? new Response(null, { status: 404 }) : new Response(fontBytes),
    );
    const html = `<style>
      @font-face {
        font-family: Face;
        src: url(https://fonts.test/missing.woff2), url(https://fonts.test/face.woff2);
      }
      @font-face { font-family: Unused; src: url(https://fonts.test/unused.woff2); }
      p { font-family: Face }
    </style>
    <p>Hello</p>`;

    await fontRenderer.prepare(html, { width: 100, height: 100, images: { fetch } });

    expect(fetch.mock.calls.map(([url]) => url).sort()).toEqual([
      "https://fonts.test/face.woff2",
      "https://fonts.test/missing.woff2",
    ]);

    await fontRenderer.prepare(html, { width: 100, height: 100, images: { fetch } });

    // The loaded file stays with the renderer, and the failed one is tried again.
    expect(fetch.mock.calls.slice(2).map(([url]) => url)).toEqual([
      "https://fonts.test/missing.woff2",
    ]);
  });
});

describe("@font-face rendering", () => {
  test("draws with the font a rule loads, as the fonts option does", async () => {
    const manrope = await readFile(
      join(import.meta.dir, "../../assets/fonts/manrope/manrope-latin-wght-normal.woff2"),
    );
    const fetch = mock(async (_url: string) => new Response(manrope));
    const options = { width: 200, height: 60, format: "png" as const };
    const page = (css: string) =>
      `<style>${css} p { font: 24px Face; margin: 0 }</style><p>Hello</p>`;

    const fromRule = new Renderer();
    const ruled = await fromRule.render(
      await fromRule.prepare(
        page("@font-face { font-family: Face; src: url(https://fonts.test/manrope.woff2) }"),
        { ...options, images: { fetch } },
      ),
      options,
    );
    const fromOption = new Renderer();
    const listed = await fromOption.render(
      await fromOption.prepare(page(""), {
        ...options,
        fonts: [{ name: "Face", data: manrope }],
      }),
      options,
    );
    const fallback = await new Renderer().render(page(""), options);

    expect(Buffer.compare(ruled, listed)).toBe(0);
    expect(Buffer.compare(ruled, fallback)).not.toBe(0);
  });
});
