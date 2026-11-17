import { afterEach, describe, expect, mock, spyOn, test } from "bun:test";
import { prepareSource, readSource, renderPrepared } from "../src/prepared";
import type { ListedFont, PrepareExtras } from "../src/prepared";
import type { FontLoader } from "../src/renderer";
import { isTakumiError, type RenderReport, type RenderWarning } from "../src/warning";
import { fontFromUrl, type Node } from "@takumi-rs/helpers";
import { RendererStore } from "../src/store";
import { fontBackend } from "./font-backend";

const bytes = () => new Uint8Array([1, 2, 3]);

afterEach(() => mock.restore());

/** Answers the global `fetch` with `fetch`, which goes through the renderer's default store as a caller's own `fetch` would not. */
const answerFetch = (fetch: (input: string | URL | Request) => Promise<Response>) => {
  spyOn(globalThis, "fetch").mockImplementation(
    Object.assign(fetch, { preconnect: globalThis.fetch.preconnect }),
  );
};
const textNode = (text: string): Node => ({ type: "text", text });

/** The characters the `missing-glyphs` warning of `report` lists, none without one. */
const missingOf = (report: RenderReport) =>
  report.warnings.flatMap((warning) =>
    warning.code === "missing-glyphs" ? warning.characters : [],
  );

const rangedFont = (name: string, unicodeRange: [number, number][]): FontLoader => ({
  name,
  unicodeRange,
  data: () => bytes(),
});

/** The listed fonts a render of `text` loads, as a backend's needs reports them: each whose range covers a character of it. */
const fontsFor = (text: string, listed: ListedFont[] = []) =>
  listed.flatMap((font, id) =>
    [...text].some((character) => {
      const codepoint = character.codePointAt(0) ?? 0;
      const ranges: [number, number][] = font.unicodeRange ?? [[0, 0x10ffff]];

      return ranges.some(([from, to]) => codepoint >= from && codepoint <= to);
    })
      ? [id]
      : [],
  );

/** Reads `node`, whose text stands in for the characters a backend reads, then renders nothing. */
const render = async (
  fonts: ReturnType<typeof fontBackend>["fonts"],
  extras: PrepareExtras & { fontFamilies?: string[] },
  node: Node,
) =>
  renderPrepared(
    fonts,
    await readSource(fonts, node, {}, extras, ({ fonts: listed }) => ({
      images: [],
      fonts: fontsFor(node.type === "text" ? node.text : "", listed),
    })),
    { signal: extras.signal },
    () => undefined,
  );

/** Mirrors the per-backend `Renderer` wiring so the shared body is exercised through all methods. */
const stub = () => {
  const { fonts } = fontBackend();
  const inner = {
    render: mock((_node: Node, _options: unknown, _signal?: AbortSignal) => "png"),
    renderSvg: mock((_node: Node, _options: unknown, _signal?: AbortSignal) => "svg"),
    measure: mock((_node: Node, _options: unknown, _signal?: AbortSignal) => "measured"),
    renderAnimation: mock((_options: unknown, _signal?: AbortSignal) => "anim"),
  };
  const characters = (nodes: Node[]) =>
    nodes.map((node) => (node.type === "text" ? node.text : "")).join("");
  const draw =
    (call: typeof inner.render) =>
    async (node: Node, options: { signal?: AbortSignal; fonts?: FontLoader[] } = {}) =>
      renderPrepared(
        fonts,
        await readSource(fonts, node, {}, options, ({ fonts: listed }) => ({
          images: [],
          fonts: fontsFor(characters([node]), listed),
        })),
        { signal: options.signal },
        (source, opts, signal) => call(source, opts, signal),
      );

  return {
    inner,
    render: draw(inner.render),
    renderSvg: draw(inner.renderSvg),
    measure: draw(inner.measure),
    async renderAnimation(options: {
      scenes: { node: Node }[];
      signal?: AbortSignal;
      fonts?: FontLoader[];
    }) {
      const { scenes, ...extras } = options;
      const prepared = await readSource(fonts, scenes, {}, extras, ({ fonts: listed }) => ({
        images: [],
        fonts: fontsFor(characters(scenes.map((scene) => scene.node)), listed),
      }));

      return renderPrepared(fonts, prepared, { signal: options.signal }, (sources, opts, signal) =>
        inner.renderAnimation({ ...opts, scenes: sources }, signal),
      );
    },
  };
};

describe("FontRegistry", () => {
  test("hands a deprecated ranges loader to the backend as written", async () => {
    const { backend, fonts } = fontBackend();

    await fonts.register({ name: "Fam", ranges: [[0x41, 0x5a]], data: () => bytes() });

    expect(backend.registerFont.mock.calls[0]?.[0]).toMatchObject({ ranges: [[0x41, 0x5a]] });
  });
});

describe("readSource and renderPrepared abort policy", () => {
  test("a pre-aborted signal rejects before resources resolve", async () => {
    const { fonts } = fontBackend();
    const resolve = spyOn(fonts, "resolveResources");

    await expect(render(fonts, { signal: AbortSignal.abort() }, textNode("A"))).rejects.toThrow();
    expect(resolve).not.toHaveBeenCalled();
  });

  test("a signal aborted during resolution rejects after resources resolve", async () => {
    const { fonts } = fontBackend();
    const resolve = spyOn(fonts, "resolveResources");
    const controller = new AbortController();
    const font: FontLoader = {
      name: "Fam",
      data: () => {
        controller.abort();
        return bytes();
      },
    };

    await expect(
      render(fonts, { fonts: [font], signal: controller.signal }, textNode("A")),
    ).rejects.toThrow();
    expect(resolve).toHaveBeenCalled();
  });
});

describe("readSource and renderPrepared resource forwarding", () => {
  test("fonts, images and fontFamilies reach resolveResources", async () => {
    const { fonts } = fontBackend();
    const resolve = spyOn(fonts, "resolveResources");
    const font = rangedFont("Fam", [[0x41, 0x5a]]);
    const images = [{ src: "a", data: bytes() }];

    await render(fonts, { fonts: [font], images, fontFamilies: ["Explicit"] }, textNode("A"));

    expect(resolve).toHaveBeenCalledWith(
      [font],
      [{ src: "a", data: bytes(), cache: undefined }],
      ["Explicit"],
      expect.any(Function),
      { files: undefined, signal: undefined, implicitFamilies: false },
    );
  });

  test("fonts accepts a promise of the list", async () => {
    const { fonts } = fontBackend();
    const resolve = spyOn(fonts, "resolveResources");
    const font = rangedFont("Fam", [[0x41, 0x5a]]);

    await render(fonts, { fonts: Promise.resolve([font]) }, textNode("A"));

    expect(resolve).toHaveBeenCalledWith([font], [], undefined, expect.any(Function), {
      files: undefined,
      signal: undefined,
      implicitFamilies: false,
    });
  });

  test("the characters the backend reads drive font subsetting", async () => {
    const { fonts } = fontBackend();
    const resolve = spyOn(fonts, "resolveResources");
    const font = rangedFont("Fam", [[0x4e00, 0x4e00]]);

    await render(fonts, { fonts: [font] }, textNode("0"));

    expect(resolve).toHaveBeenCalledWith([], [], undefined, expect.any(Function), {
      files: undefined,
      signal: undefined,
      implicitFamilies: false,
    });
  });
});

describe("shared Renderer body", () => {
  test("every method forwards resolved resources to its inner call", async () => {
    const backend = stub();
    const font = rangedFont("Fam", [[0x41, 0x5a]]);
    const node = textNode("A");

    await backend.render(node, { fonts: [font] });
    await backend.renderSvg(node, { fonts: [font] });
    await backend.measure(node, { fonts: [font] });

    for (const call of [backend.inner.render, backend.inner.renderSvg, backend.inner.measure]) {
      expect(call).toHaveBeenCalledWith(node, { fontFamilies: ["Fam"], images: [] }, undefined);
    }
  });

  test("a pre-aborted signal keeps the inner call from running", async () => {
    const backend = stub();

    await expect(backend.render(textNode("A"), { signal: AbortSignal.abort() })).rejects.toThrow();
    expect(backend.inner.render).not.toHaveBeenCalled();
  });

  test("renderAnimation subsets fonts across every scene node", async () => {
    const backend = stub();
    const fontA = rangedFont("A", [[0x41, 0x41]]);
    const fontB = rangedFont("B", [[0x42, 0x42]]);

    const scenes = [{ node: textNode("A") }, { node: textNode("B") }];
    await backend.renderAnimation({ scenes, fonts: [fontA, fontB] });

    expect(backend.inner.renderAnimation).toHaveBeenCalledWith(
      { scenes, fontFamilies: ["A", "B"], images: [] },
      undefined,
    );
  });
});

describe("@font-face files", () => {
  test("prepare loads each reached file, and a failed one moves its rule on to the next src", async () => {
    const { fonts, files, backend } = fontBackend();
    const fetch = mock(async (input: string | URL | Request) =>
      String(input).endsWith("missing.woff2")
        ? new Response(null, { status: 404 })
        : new Response(bytes()),
    );
    const reads = mock(({ failedFontFiles = [] }: { failedFontFiles?: string[] }) => ({
      images: [],
      fonts: [],
      fontFiles: failedFontFiles.includes("https://fonts.test/missing.woff2")
        ? ["https://fonts.test/face.woff2"]
        : ["https://fonts.test/missing.woff2"],
    }));
    const call = mock((_source: unknown, _options: { failedFontFiles?: string[] }) => undefined);

    const prepared = await prepareSource(fonts, textNode("a"), {}, { images: { fetch } }, reads);

    await renderPrepared(fonts, prepared, {}, call);

    expect(backend.cacheFontFile).toHaveBeenCalledTimes(1);
    expect(files.has("https://fonts.test/face.woff2")).toBe(true);
    expect(call.mock.calls[0]?.[1].failedFontFiles).toEqual(["https://fonts.test/missing.woff2"]);
  });
});

describe("@font-face files a prepared render loaded", () => {
  const url = "https://fonts.test/face.woff2";
  const prepare = async (
    fonts: ReturnType<typeof fontBackend>["fonts"],
    fetch: (input: string | URL | Request) => Promise<Response>,
  ) => {
    answerFetch(fetch);

    return prepareSource(fonts, textNode("a"), {}, {}, () => ({
      images: [],
      fonts: [],
      fontFiles: [url],
    }));
  };

  test("stay leased while the render runs", async () => {
    const { fonts, backend, leases } = fontBackend();
    const fetch = mock(async () => new Response(bytes()));
    const prepared = await prepare(fonts, fetch);
    let leased: number[] = [];

    await renderPrepared(fonts, prepared, {}, () => {
      leased = [...leases];
    });

    expect(fetch).toHaveBeenCalledTimes(1);
    expect(backend.cachedFontFile).toHaveBeenCalledWith(url, leased[0]);
  });

  test("load again from the store when the renderer evicted them since prepare", async () => {
    const { fonts, files, backend } = fontBackend();
    const fetch = mock(async () => new Response(bytes()));
    const call = mock((_source: unknown, _options: { failedFontFiles?: string[] }) => undefined);
    const prepared = await prepare(fonts, fetch);

    files.clear();
    await renderPrepared(fonts, prepared, {}, call);

    expect(fetch).toHaveBeenCalledTimes(1);
    expect(backend.cacheFontFile).toHaveBeenCalledTimes(2);
    expect(files.has(url)).toBe(true);
    expect(call.mock.calls[0]?.[1].failedFontFiles).toBeUndefined();
  });

  test("move their rule on when loading them again fails", async () => {
    const { fonts, files } = fontBackend(new RendererStore({ store: null }));
    const fetch = mock(async () => new Response(bytes()));
    const call = mock((_source: unknown, _options: { failedFontFiles?: string[] }) => undefined);
    const prepared = await prepare(fonts, fetch);

    files.clear();
    fetch.mockImplementation(async () => new Response(null, { status: 404 }));
    await renderPrepared(fonts, prepared, {}, call);

    expect(call.mock.calls[0]?.[1].failedFontFiles).toEqual([url]);
  });
});

test("prepare loads each @font-face file once even when the cache releases it", async () => {
  const { fonts } = fontBackend();
  const fetch = mock(async () => new Response(bytes()));
  const reads = mock(() => ({
    images: [],
    fonts: [],
    fontFiles: ["https://fonts.test/a.woff2", "https://fonts.test/b.woff2"],
  }));

  await prepareSource(fonts, textNode("a"), {}, { images: { fetch } }, reads);

  expect(fetch).toHaveBeenCalledTimes(2);
  expect(reads).toHaveBeenCalledTimes(2);
});

test("a listed subset without the glyph its unicodeRange claims loads the next family", async () => {
  const { fonts, held } = fontBackend();
  const subset = (name: string) => ({
    name,
    unicodeRange: [[0x100, 0x24f]] as [number, number][],
    data: mock(bytes),
  });
  const sans = subset("Sans");
  const serif = subset("Serif");
  // Sans claims Ĕ but has no glyph for it, which only loading it shows.
  const reads = mock(({ fonts: listed = [] }: { fonts?: ListedFont[] }) => ({
    images: [],
    fonts: listed[0] && held.has(listed[0].key) ? [0, 1] : [0],
  }));
  const call = mock((_source: unknown, _options: { fontFamilies?: string[] }) => undefined);

  await renderPrepared(
    fonts,
    await readSource(fonts, textNode("Ĕ"), {}, { fonts: [sans, serif] }, reads),
    {},
    call,
  );

  expect(sans.data).toHaveBeenCalledTimes(1);
  expect(serif.data).toHaveBeenCalledTimes(1);
  expect(reads).toHaveBeenCalledTimes(3);
  expect(call.mock.calls[0]?.[1].fontFamilies).toEqual(["Sans", "Serif"]);
});

test("a render loads listed subsets but leaves @font-face files to prepare", async () => {
  const { fonts } = fontBackend();
  const fetch = spyOn(globalThis, "fetch");
  const sans = { name: "Sans", unicodeRange: [[0x41, 0x5a]] as [number, number][], data: bytes };
  const reads = mock(() => ({
    images: [],
    fonts: [0],
    fontFiles: ["https://fonts.test/a.woff2"],
  }));

  await readSource(fonts, textNode("A"), {}, { fonts: [sans] }, reads);

  expect(reads).toHaveBeenCalledTimes(2);
  expect(fetch).not.toHaveBeenCalled();
  fetch.mockRestore();
});

describe("fonts with cache none", () => {
  const latin: [number, number][] = [[0x20, 0x7e]];
  const font = (name: string, cache?: "auto" | "none") => ({
    name,
    unicodeRange: latin,
    data: () => bytes(),
    cache,
  });

  test("are dropped once the render that loaded them settles", async () => {
    const { held, backend, fonts } = fontBackend();
    const once = font("Noto", "none");

    await render(fonts, { fonts: [once] }, textNode("a"));

    expect(backend.dropFonts).toHaveBeenCalledTimes(1);
    expect(held.size).toBe(0);

    await render(fonts, { fonts: [once] }, textNode("a"));

    expect(backend.cacheFont).toHaveBeenCalledTimes(2);
  });

  test("a subset a read loaded but the render left out goes too", async () => {
    const { held, fonts } = fontBackend();
    // The first read reaches Sans, which lacks the glyph its range claims, so later reads reach Serif.
    const reads = mock(({ fonts: listed = [] }: { fonts?: ListedFont[] }) => ({
      images: [],
      fonts: listed[0] && held.has(listed[0].key) ? [1] : [0],
    }));

    await renderPrepared(
      fonts,
      await readSource(
        fonts,
        textNode("a"),
        {},
        { fonts: [font("Sans", "none"), font("Serif", "none")] },
        reads,
      ),
      {},
      () => undefined,
    );

    expect(reads).toHaveBeenCalledTimes(3);
    expect(held.size).toBe(0);
  });

  test("use a font the renderer holds without dropping it", async () => {
    const { held, backend, fonts } = fontBackend();
    const kept = font("Inter");

    await render(fonts, { fonts: [kept] }, textNode("a"));
    await render(fonts, { fonts: [font("Inter", "none")] }, textNode("a"));

    expect(backend.cacheFont).toHaveBeenCalledTimes(1);
    expect(backend.dropFonts).not.toHaveBeenCalled();
    expect(held.size).toBe(1);
  });

  test("stay while another render leases them, and go with the last one", async () => {
    const { held, backend, fonts } = fontBackend();
    const once = font("Noto", "none");
    const lease = backend.leaseFonts();

    await render(fonts, { fonts: [once] }, textNode("a"));

    expect(held.size).toBe(1);

    backend.releaseFonts(lease);
    await render(fonts, { fonts: [once] }, textNode("a"));

    expect(backend.cacheFont).toHaveBeenCalledTimes(1);
    expect(held.size).toBe(0);
  });

  test("are kept once a render without the policy uses them", async () => {
    const { held, fonts } = fontBackend();
    const kept = font("Inter");
    const lease = fonts.leased(() =>
      render(fonts, { fonts: [font("Inter", "none")] }, textNode("a")),
    );

    await render(fonts, { fonts: [kept] }, textNode("a"));
    await lease;
    await render(fonts, { fonts: [font("Inter", "none")] }, textNode("a"));

    expect(held.size).toBe(1);
  });

  test("go with the last render to settle, though it lists other fonts", async () => {
    const { held, fonts } = fontBackend();
    const loading = Promise.withResolvers<void>();
    const later = render(
      fonts,
      { fonts: [{ ...font("Inter", "none"), data: () => loading.promise.then(bytes) }] },
      textNode("a"),
    );

    await render(fonts, { fonts: [font("Noto", "none")] }, textNode("a"));

    expect(held.size).toBe(1);

    loading.resolve();
    await later;

    expect(held.size).toBe(0);
  });

  test("are dropped when prepare is aborted after loading them", async () => {
    const { held, fonts } = fontBackend();
    const controller = new AbortController();
    const aborting = {
      ...font("Noto", "none"),
      data: () => {
        controller.abort(new Error("cancelled"));

        return bytes();
      },
    };
    const prepared = prepareSource(
      fonts,
      textNode("a"),
      {},
      { fonts: [aborting], signal: controller.signal },
      ({ fonts: listed }) => ({ images: [], fonts: fontsFor("a", listed) }),
    );

    await expect(prepared).rejects.toThrow("cancelled");
    expect(held.size).toBe(0);
  });

  test("are dropped when they finish loading after their render failed", async () => {
    const { held, fonts } = fontBackend();
    const loading = Promise.withResolvers<void>();
    const failed = render(
      fonts,
      {
        fonts: [
          { ...font("Noto", "none"), data: () => Promise.reject(new Error("unreachable")) },
          { ...font("Inter", "none"), data: () => loading.promise.then(bytes) },
        ],
      },
      textNode("a"),
    );

    await expect(failed).rejects.toThrow("unreachable");
    loading.resolve();
    await Bun.sleep(0);

    expect(held.size).toBe(0);
  });
});

describe("the renderer's download store", () => {
  const url = "https://images.test/a.png";
  const prepare = (
    fonts: ReturnType<typeof fontBackend>["fonts"],
    fetch: (input: string | URL | Request) => Promise<Response>,
  ) => {
    answerFetch(fetch);

    return prepareSource(fonts, textNode("a"), {}, {}, () => ({
      images: [url],
      fonts: [],
    }));
  };

  test("serves an image a later render asks for again", async () => {
    const fetch = mock(async () => new Response(bytes()));
    const { fonts } = fontBackend();

    await prepare(fonts, fetch);
    await prepare(fonts, fetch);

    expect(fetch).toHaveBeenCalledTimes(1);
    expect(fonts.store.stats()).toMatchObject({ hits: 1, misses: 1, max: 32 * 1024 * 1024 });
  });

  test("keeps nothing when cache.store is null", async () => {
    const fetch = mock(async () => new Response(bytes()));
    const { fonts } = fontBackend(new RendererStore({ store: null }));

    await prepare(fonts, fetch);
    await prepare(fonts, fetch);

    expect(fetch).toHaveBeenCalledTimes(2);
    expect(fonts.store.stats()).toBeNull();
  });

  test("uses a store set later, and finds the default store again", async () => {
    const fetch = mock(async () => new Response(bytes()));
    const { fonts } = fontBackend();
    const setMemory = mock((_memory: object) => {});

    await prepare(fonts, fetch);
    fonts.store.apply({ store: null, maxBytes: 1 }, setMemory);
    await prepare(fonts, fetch);
    fonts.store.apply({}, setMemory);
    await prepare(fonts, fetch);

    expect(fetch).toHaveBeenCalledTimes(2);
    expect(setMemory.mock.calls).toEqual([[{ maxBytes: 1 }], [{}]]);
  });

  test("keeps the store when the binding rejects the options", () => {
    const store = new RendererStore();
    const before = store.current;

    expect(() =>
      store.apply({ store: null }, () => {
        throw new Error("invalid");
      }),
    ).toThrow("invalid");
    expect(store.current).toBe(before);
  });
});

describe("optional fonts", () => {
  const font = (name: string) => ({ name, data: () => bytes() });
  const reads = () => ({ images: [], fonts: [] });
  const broken = (name: string) => ({
    name,
    optional: true,
    data: mock(() => Promise.reject(new Error(`${name} is unreachable`))),
  });

  test("leave a render that goes on without them", async () => {
    const { fonts } = fontBackend();
    const missing = broken("Missing");
    const call = mock((_source: unknown, _options: object) => undefined);

    await renderPrepared(
      fonts,
      await readSource(fonts, textNode("a"), {}, { fonts: [font("Inter"), missing] }, reads),
      {},
      call,
    );

    expect(call.mock.calls[0]?.[1]).toEqual({ fontFamilies: ["Inter"], images: [] });
  });

  test("that a needs pass reaches are not loaded again once they failed", async () => {
    const { fonts } = fontBackend();
    const latin: [number, number][] = [[0x20, 0x7e]];
    const missing = { ...broken("Missing"), unicodeRange: latin };
    const call = mock((_source: unknown, _options: object) => undefined);
    const listedReads = ({ fonts: listed }: { fonts?: ListedFont[] }) => ({
      images: [],
      fonts: fontsFor("a", listed),
    });

    await renderPrepared(
      fonts,
      await prepareSource(fonts, textNode("a"), {}, { fonts: [missing] }, listedReads),
      {},
      call,
    );

    expect(missing.data).toHaveBeenCalledTimes(1);
    expect(call.mock.calls[0]?.[1]).toEqual({ fontFamilies: [], images: [] });
  });

  test("that fail leave their characters to the next listed family", async () => {
    const { fonts } = fontBackend();
    const latin: [number, number][] = [[0x20, 0x7e]];
    const missing = { ...broken("Missing"), unicodeRange: latin };
    const inter = { name: "Inter", unicodeRange: latin, data: mock(bytes) };
    const call = mock((_source: unknown, _options: object) => undefined);
    // As the needs pass does, the first named family that claims a character serves it.
    const firstReads = ({ fonts: listed = [] }: { fonts?: ListedFont[] }) => ({
      images: [],
      fonts: fontsFor("a", listed)
        .filter((id) => listed[id]?.name !== undefined)
        .slice(0, 1),
    });

    await renderPrepared(
      fonts,
      await prepareSource(fonts, textNode("a"), {}, { fonts: [missing, inter] }, firstReads),
      {},
      call,
    );

    expect(inter.data).toHaveBeenCalledTimes(1);
    expect(call.mock.calls[0]?.[1]).toEqual({ fontFamilies: ["Inter"], images: [] });
  });

  test("that fail beside a font under the same key leave that font", async () => {
    const { fonts } = fontBackend();
    const required = { name: "Inter", data: mock(bytes) };
    const call = mock((_source: unknown, _options: object) => undefined);

    await renderPrepared(
      fonts,
      await prepareSource(fonts, textNode("a"), {}, { fonts: [broken("Inter"), required] }, reads),
      {},
      call,
    );

    expect(call.mock.calls[0]?.[1]).toEqual({ fontFamilies: ["Inter"], images: [] });
  });

  test("that fail during a render are not loaded again", async () => {
    const { fonts, held } = fontBackend();
    const data = mock(async () => bytes());
    const flaky = { name: "Flaky", optional: true, data };
    const prepared = await prepareSource(fonts, textNode("a"), {}, { fonts: [flaky] }, reads);

    data.mockImplementation(() => Promise.reject(new Error("gone")));
    held.clear();
    await renderPrepared(fonts, prepared, {}, () => undefined);
    await renderPrepared(fonts, prepared, {}, () => undefined);

    expect(data).toHaveBeenCalledTimes(2);
  });

  test("refused by allowUrl leave the render without them", async () => {
    const { fonts, held } = fontBackend();
    const url = "https://fonts.test/refused.woff2";
    const refused = { ...fontFromUrl(url, { allowUrl: () => false }), optional: true };

    await prepareSource(fonts, textNode("a"), {}, { fonts: [refused] }, reads);

    expect(held.size).toBe(0);
  });

  test("without optional still reject the render", async () => {
    const { fonts } = fontBackend();
    const { optional: _, ...required } = broken("Required");

    await expect(render(fonts, { fonts: [required] }, textNode("a"))).rejects.toThrow(
      "Required is unreachable",
    );
  });
});

describe("a prepared render's report", () => {
  const missing = (characters: string[]) => ({
    images: [],
    fonts: [],
    missingCharacters: characters,
    unresolvedFamilies: ["Absent"],
  });

  test("holds what the last read found no font for", async () => {
    const { fonts } = fontBackend();
    const prepared = await prepareSource(fonts, textNode("中"), {}, {}, () => missing(["中"]));

    expect(prepared.report).toEqual({
      warnings: [
        {
          code: "missing-glyphs",
          message:
            'No font draws "中". Add a font that covers it to `fonts`, or return one from `onMissingGlyphs`.',
          characters: ["中"],
        },
        {
          code: "unresolved-family",
          message:
            'No font provides the family "Absent". Add a font named "Absent" to `fonts`, or correct the name.',
          family: "Absent",
        },
      ],
    });
  });

  test("reads again once the fonts the render lists are loaded", async () => {
    const { fonts, held } = fontBackend();
    const reads = mock((_request: object) => missing(held.size > 0 ? [] : ["中"]));
    const prepared = await prepareSource(
      fonts,
      textNode("中"),
      {},
      { fonts: [{ name: "CJK", data: bytes }] },
      reads,
    );

    expect(missingOf(prepared.report)).toEqual([]);
    expect(reads.mock.calls.at(-1)).toEqual([
      { images: [], fontFamilies: ["CJK"], failedFontFiles: [] },
    ]);
  });

  test("names the images left out under throwOnError: false and the @font-face files that failed", async () => {
    const { fonts } = fontBackend();
    const fetch = mock(async () => new Response(null, { status: 404 }));
    const prepared = await prepareSource(
      fonts,
      textNode("a"),
      {},
      { images: { fetch, throwOnError: false } },
      () => ({ images: ["https://images.test/a.png"], fonts: [], fontFiles: ["face.woff2"] }),
    );

    expect(prepared.report.warnings).toEqual([
      {
        code: "image-load-failed",
        message: expect.stringContaining("Failed to load the image https://images.test/a.png: "),
        url: "https://images.test/a.png",
        cause: expect.any(Error),
      },
      {
        code: "font-load-failed",
        message: expect.stringContaining("Failed to load the font face.woff2: "),
        source: "face.woff2",
        cause: expect.any(Error),
      },
    ]);
  });

  test("finds missing characters in the images it downloaded", async () => {
    const { fonts } = fontBackend();
    const fetch = mock(async () => new Response(bytes()));
    // Text inside an SVG image is read only once the image is in the request.
    const reads = mock(({ images = [] }: { images?: unknown[] }) => ({
      ...missing(images.length > 0 ? ["中"] : []),
      images: ["https://images.test/text.svg"],
    }));
    const onMissingGlyphs = mock((_characters: string[]) => undefined);

    await prepareSource(fonts, textNode("a"), {}, { images: { fetch }, onMissingGlyphs }, reads);

    expect(onMissingGlyphs.mock.calls).toEqual([[["中"]]]);
  });

  test("loads the @font-face files that text in a downloaded image names", async () => {
    const { fonts } = fontBackend();
    const face = "https://fonts.test/svg-face.woff2";
    const fetch = mock(async (_input: string | URL | Request) => new Response(bytes()));
    const reads = mock(({ images = [] }: { images?: unknown[] }) => ({
      ...missing([]),
      images: ["https://images.test/text.svg"],
      fontFiles: images.length > 0 ? [face] : [],
    }));

    await prepareSource(fonts, textNode("a"), {}, { images: { fetch } }, reads);

    expect(fetch.mock.calls.map(([input]) => String(input))).toContain(face);
  });

  test("onMissingGlyphs loads a font under the key of one that failed", async () => {
    const { fonts, held } = fontBackend();
    const reads = mock(() => missing(held.has("cjk") ? [] : ["中"]));
    const replacement = { key: "cjk", name: "CJK", data: mock(bytes) };
    const failing = {
      key: "cjk",
      name: "CJK",
      optional: true,
      data: () => Promise.reject(new Error("down")),
    };
    const prepared = await prepareSource(
      fonts,
      textNode("中"),
      {},
      { fonts: [failing], onMissingGlyphs: () => [replacement] },
      reads,
    );

    expect(replacement.data).toHaveBeenCalled();
    expect(missingOf(prepared.report)).toEqual([]);
  });

  test("onMissingGlyphs fonts join the registered chain when the render names no fonts", async () => {
    const { fonts, held, backend } = fontBackend();
    const reads = mock(() => missing(held.has("cjk") ? [] : ["中"]));
    const call = mock((_source: unknown, _options: { fontFamilies?: string[] }) => undefined);
    const prepared = await prepareSource(
      fonts,
      textNode("中"),
      {},
      { onMissingGlyphs: () => [{ key: "cjk", name: "CJK", data: bytes }] },
      reads,
    );

    await renderPrepared(fonts, prepared, {}, call);

    expect(held.has("cjk")).toBe(true);
    expect(call.mock.calls[0]?.[1].fontFamilies).toBeUndefined();
    expect(backend.orderFonts).toHaveBeenLastCalledWith(["cjk"]);
  });

  test("onMissingGlyphs adds its fonts to the render and reads once more", async () => {
    const { fonts, held } = fontBackend();
    const reads = mock(() => missing(held.has("cjk") ? [] : ["中"]));
    const onMissingGlyphs = mock((_characters: string[]) => [
      { key: "cjk", name: "CJK", data: bytes },
    ]);
    const call = mock((_source: unknown, _options: { fontFamilies?: string[] }) => undefined);
    const prepared = await prepareSource(
      fonts,
      textNode("中"),
      {},
      { fonts: [{ name: "Latin", data: bytes }], fontFamilies: ["Latin"], onMissingGlyphs },
      reads,
    );

    await renderPrepared(fonts, prepared, {}, call);

    expect(onMissingGlyphs.mock.calls).toEqual([[["中"]]]);
    expect(missingOf(prepared.report)).toEqual([]);
    expect(reads).toHaveBeenCalledTimes(3);
    expect(call.mock.calls[0]?.[1].fontFamilies).toEqual(["Latin", "CJK"]);
  });
});

describe("a render's warnings", () => {
  const image = "https://images.test/a.png";
  const failing = mock(async () => new Response(null, { status: 404 }));
  /** Reads `characters` as drawn by no font, and an image and a `@font-face` file the fetch fails. */
  const reads = (characters: string[]) => () => ({
    images: [image],
    fonts: [],
    fontFiles: ["face.woff2"],
    missingCharacters: characters,
    unresolvedFamilies: ["Absent"],
  });
  const prepare = (characters: string[], extras: PrepareExtras) =>
    prepareSource(
      fontBackend().fonts,
      textNode("a"),
      {},
      { images: { fetch: failing, throwOnError: false }, ...extras },
      reads(characters),
    );

  test("go to onWarning, one for each thing the render could not draw", async () => {
    const warn = spyOn(console, "warn");
    const onWarning = mock((_warning: RenderWarning) => {});
    const prepared = await prepare(["中", "文"], { onWarning });

    expect(onWarning.mock.calls.map(([warning]) => warning.code)).toEqual([
      "missing-glyphs",
      "unresolved-family",
      "image-load-failed",
      "font-load-failed",
    ]);
    expect(onWarning.mock.calls.map(([warning]) => warning)).toEqual(prepared.report.warnings);
    expect(prepared.report.warnings[0]).toEqual({
      code: "missing-glyphs",
      message:
        'No font draws "中文". Add a font that covers them to `fonts`, or return one from `onMissingGlyphs`.',
      characters: ["中", "文"],
    });
    expect(warn).not.toHaveBeenCalled();
  });

  test("print once each through console.warn without onWarning, across renders", async () => {
    const warn = spyOn(console, "warn").mockImplementation(() => {});

    await prepareSource(fontBackend().fonts, textNode("鬱"), {}, {}, () => ({
      images: [],
      fonts: [],
      missingCharacters: ["鬱"],
    }));
    await prepareSource(fontBackend().fonts, textNode("鬱"), {}, {}, () => ({
      images: [],
      fonts: [],
      missingCharacters: ["鬱"],
    }));

    expect(warn.mock.calls).toEqual([
      [
        '[takumi] No font draws "鬱". Add a font that covers it to `fonts`, or return one from `onMissingGlyphs`.',
      ],
    ]);
  });

  test("reject a strict render with a TakumiError holding every warning, after onWarning saw them", async () => {
    const onWarning = mock((_warning: RenderWarning) => {});
    const error: unknown = await prepare(["中"], { onWarning, strict: true }).catch(
      (caught: unknown) => caught,
    );

    expect(isTakumiError(error)).toBe(true);

    if (!isTakumiError(error, "ERR_TAKUMI_STRICT")) {
      throw new Error("expected a strict error");
    }

    expect(error).toBeInstanceOf(Error);
    expect(error.name).toBe("TakumiError");
    expect(error.warnings).toEqual(onWarning.mock.calls.map(([warning]) => warning));
    expect(error.message).toBe(`${error.warnings[0]?.message} (and 3 more)`);
    expect(Object.keys(error)).toEqual(expect.arrayContaining(["name", "code", "warnings"]));
  });

  test("reject a strict render for an image left out under throwOnError: false", async () => {
    const error: unknown = await prepareSource(
      fontBackend().fonts,
      textNode("a"),
      {},
      { images: { fetch: failing, throwOnError: false }, onWarning: () => {}, strict: true },
      () => ({ images: [image], fonts: [] }),
    ).catch((caught: unknown) => caught);

    expect(isTakumiError(error, "ERR_TAKUMI_STRICT") && error.warnings).toEqual([
      {
        code: "image-load-failed",
        message: expect.stringContaining(`Failed to load the image ${image}: `),
        url: image,
        cause: expect.any(Error),
      },
    ]);
  });

  test("leave a strict render without warnings alone", async () => {
    const prepared = await prepareSource(
      fontBackend().fonts,
      textNode("a"),
      {},
      { strict: true },
      () => ({
        images: [],
        fonts: [],
      }),
    );

    expect(prepared.report).toEqual({ warnings: [] });
  });

  test("reject a render without prepare with what onWarning throws, before it draws", async () => {
    const { fonts } = fontBackend();
    const thrown = new Error("from onWarning");
    const call = mock(() => undefined);
    const read = readSource(
      fonts,
      textNode("中"),
      {},
      {
        onWarning: () => {
          throw thrown;
        },
      },
      () => ({ images: [], fonts: [], missingCharacters: ["中"] }),
    );

    await expect(read.then((prepared) => renderPrepared(fonts, prepared, {}, call))).rejects.toBe(
      thrown,
    );
    expect(call).not.toHaveBeenCalled();
  });

  test("are recognized when another copy of takumi raised them", () => {
    const other = Object.assign(new Error("other copy"), {
      [Symbol.for("takumi.error")]: true,
      code: "ERR_TAKUMI_STRICT",
      warnings: [],
    });

    expect(isTakumiError(other, "ERR_TAKUMI_STRICT")).toBe(true);
    expect(isTakumiError(new Error("plain"))).toBe(false);
    expect(isTakumiError({ code: "ERR_TAKUMI_STRICT" })).toBe(false);
  });
});

describe("fonts listed as URLs", () => {
  const url = "https://fonts.test/listed.woff2";
  const reads = () => ({ images: [], fonts: [] });
  const prepare = (
    fonts: ReturnType<typeof fontBackend>["fonts"],
    images: NonNullable<PrepareExtras["images"]>,
  ) => prepareSource(fonts, textNode("a"), {}, { fonts: [url], images }, reads);

  test("download with the render's fetch", async () => {
    const global = mock(async () => new Response(bytes()));
    const fetch = mock(async (_input: string | URL | Request) => new Response(bytes()));
    const { fonts, held } = fontBackend();

    answerFetch(global);
    await prepare(fonts, { fetch });

    expect(fetch.mock.calls.map(([input]) => String(input))).toEqual([url]);
    expect(global).not.toHaveBeenCalled();
    expect(held.has(url)).toBe(true);
  });

  test("are refused by the render's allowUrl", async () => {
    const global = mock(async () => new Response(bytes()));
    const { fonts } = fontBackend();

    answerFetch(global);

    await expect(prepare(fonts, { allowUrl: () => false })).rejects.toThrow(
      `URL blocked by allowUrl policy: ${url}`,
    );
    expect(global).not.toHaveBeenCalled();
  });

  test("stop waiting on another render's download when their own signal aborts", async () => {
    const started = Promise.withResolvers<void>();
    const hanging = mock(async () => {
      started.resolve();

      return new Promise<Response>(() => {});
    });
    const { fonts } = fontBackend();
    const controller = new AbortController();

    void prepare(fonts, { fetch: hanging, timeout: 0 });
    await started.promise;

    const waiting = prepareSource(
      fonts,
      textNode("a"),
      {},
      { fonts: [url], images: { fetch: hanging, timeout: 0 }, signal: controller.signal },
      reads,
    );

    // Let the waiting render reach the pending download before its signal aborts.
    await Bun.sleep(10);
    controller.abort(new Error("cancelled"));

    await expect(waiting).rejects.toThrow("cancelled");
  });

  test("download again with their own fetch when another render's download fails", async () => {
    const started = Promise.withResolvers<void>();
    const gate = Promise.withResolvers<void>();
    const failing = mock(async () => {
      started.resolve();
      await gate.promise;

      return new Response(null, { status: 404 });
    });
    const working = mock(async () => new Response(bytes()));
    const { fonts, held } = fontBackend();
    const first = prepare(fonts, { fetch: failing });

    await started.promise;

    const second = prepare(fonts, { fetch: working });

    await Bun.sleep(5);
    gate.resolve();

    await expect(first).rejects.toThrow("HTTP 404");
    await second;
    expect(working).toHaveBeenCalledTimes(1);
    expect(held.has(url)).toBe(true);
  });
});
