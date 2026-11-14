import { describe, expect, mock, test } from "bun:test";
import { MemoryStore } from "../src/fetch/store";
import { fontFromUrl, googleFonts, subsetFonts } from "../src/fonts";
import type { Node } from "../src/types";

const bytes = (s: string) => new TextEncoder().encode(s).buffer;

/** Realistic multi-subset CSS: three same-named faces split by `unicode-range`. */
const interCss = `
  /* cyrillic */
  @font-face {
    font-family: 'Inter';
    font-style: normal;
    font-weight: 400;
    src: url(https://fonts.gstatic.com/inter-cyrillic.woff2) format('woff2');
    unicode-range: U+0400-045F;
  }
  /* greek */
  @font-face {
    font-family: 'Inter';
    font-style: normal;
    font-weight: 400;
    src: url(https://fonts.gstatic.com/inter-greek.woff2) format('woff2');
    unicode-range: U+0370-03FF;
  }
  /* latin */
  @font-face {
    font-family: 'Inter';
    font-style: normal;
    font-weight: 400;
    src: url(https://fonts.gstatic.com/inter-latin.woff2) format('woff2');
    unicode-range: U+0000-00FF;
  }
`;

const cssFetch = (css: string) =>
  mock((url: string, _init?: RequestInit) =>
    Promise.resolve(url.includes("/css2") ? new Response(css) : new Response(bytes(url))),
  );

const mockInter = () => cssFetch(interCss);

/** The `{family} {subset}` a subset's key starts with. */
const subsetLabel = (font: { key: string }) => font.key.split(":")[0];

/** The last css2 request a `cssFetch` mock received. */
const cssRequest = (fetchMock: ReturnType<typeof cssFetch>) => {
  const [url = "", init] = fetchMock.mock.calls.findLast(([url]) => url.includes("/css2")) ?? [];

  return { url: new URL(url), init };
};

describe("googleFonts", () => {
  const twoWeightCss = `
    @font-face {
      font-family: 'Inter';
      font-style: normal;
      font-weight: 400;
      src: url(https://fonts.gstatic.com/inter-400.woff2) format('woff2');
    }
    @font-face {
      font-family: 'Inter';
      font-style: normal;
      font-weight: 700;
      src: url(https://fonts.gstatic.com/inter-700.woff2) format('woff2');
    }
  `;

  test("propagates the family's generic claim to every subset", async () => {
    const fonts = await googleFonts({
      families: [{ name: "Inter", generic: "sans-serif" }],
      fetch: mockInter(),
    });

    expect(fonts.length).toBeGreaterThan(0);
    expect(fonts.every((f) => f.generic === "sans-serif")).toBe(true);
    expect(fonts.every((f) => f.key.endsWith(":sans-serif"))).toBe(true);

    const plain = await googleFonts({ families: ["Inter"], fetch: mockInter() });
    expect(plain.every((f) => f.generic === undefined)).toBe(true);
  });

  test("registers every coverage subset under its family with its own range", async () => {
    const fonts = await googleFonts({ families: ["Inter"], fetch: mockInter() });

    expect(fonts.every((f) => f.name === "Inter")).toBe(true);
    expect(fonts.map(subsetLabel).sort()).toEqual(["Inter cyrillic", "Inter greek", "Inter latin"]);
    expect(fonts.find((f) => subsetLabel(f) === "Inter latin")?.unicodeRange).toEqual([
      [0x0, 0xff],
    ]);
  });

  test("resolves every weight to a keyed loader under one family", async () => {
    const fetchMock = cssFetch(twoWeightCss);

    const fonts = await googleFonts({
      families: [{ name: "Inter", weight: [400, 700] }],
      fetch: fetchMock,
    });

    expect(fonts.map((f) => f.weight).sort()).toEqual([400, 700]);
    expect(fonts.every((f) => f.name === "Inter")).toBe(true);
    // Keys are stable composites of identity, not the woff2 URL, and unique per face.
    expect(new Set(fonts.map((f) => f.key)).size).toBe(2);
    expect(fonts.every((f) => !f.key.includes("gstatic"))).toBe(true);
  });

  test("collapses a variable font's shared woff2 into one weightless face", async () => {
    // A variable font: both weight blocks share one woff2 url.
    const variableCss = `
      @font-face {
        font-family: 'Public Sans';
        font-style: normal;
        font-weight: 400;
        src: url(https://fonts.gstatic.com/public-sans-variable.woff2) format('woff2');
        unicode-range: U+0000-00FF;
      }
      @font-face {
        font-family: 'Public Sans';
        font-style: normal;
        font-weight: 700;
        src: url(https://fonts.gstatic.com/public-sans-variable.woff2) format('woff2');
        unicode-range: U+0000-00FF;
      }
    `;
    const fetchMock = cssFetch(variableCss);

    const fonts = await googleFonts({
      families: [{ name: "Public Sans", weight: [400, 700] }],
      fetch: fetchMock,
    });

    expect(fonts).toHaveLength(1);
    expect(fonts[0]?.weight).toBeUndefined();

    // A static font keeps a distinct url per weight, so faces stay split.
    const staticMock = cssFetch(twoWeightCss);
    const staticFonts = await googleFonts({
      families: [{ name: "Inter", weight: [400, 700] }],
      fetch: staticMock,
    });
    expect(staticFonts.map((f) => f.weight).sort()).toEqual([400, 700]);
  });

  test("builds the family/axis and sends a woff2 UA", async () => {
    const fetchMock = cssFetch(twoWeightCss);

    await googleFonts({
      families: [{ name: "Open Sans", weight: [700, 400], style: ["normal", "italic"] }],
      fetch: fetchMock,
    });

    const { url, init } = cssRequest(fetchMock);

    expect(url.searchParams.get("family")).toBe("Open Sans:ital,wght@0,400;0,700;1,400;1,700");
    expect(new Headers(init?.headers).get("User-Agent") ?? "").toContain("Chrome");
  });

  const captureFamily = () => {
    const fetchMock = cssFetch(twoWeightCss);

    return { fetchMock, family: () => cssRequest(fetchMock).url.searchParams.get("family") };
  };

  test("emits a custom variable axis in the css2 tuple", async () => {
    const { fetchMock, family } = captureFamily();

    await googleFonts({
      families: [{ name: "Inter", weight: "100..900", axes: { opsz: "14..32" } }],
      fetch: fetchMock,
    });

    expect(family()).toBe("Inter:opsz,wght@14..32,100..900");
  });

  test("orders ital, custom axis, and wght as css2 requires", async () => {
    const { fetchMock, family } = captureFamily();

    await googleFonts({
      families: [
        { name: "Inter", weight: 400, style: ["normal", "italic"], axes: { opsz: "14..32" } },
      ],
      fetch: fetchMock,
    });

    expect(family()).toBe("Inter:ital,opsz,wght@0,14..32,400;1,14..32,400");
  });

  test("sorts uppercase custom axes before lowercase ones", async () => {
    const { fetchMock, family } = captureFamily();

    // An unknown family takes the loose form, whose axes accept any tag.
    await googleFonts({
      families: [{ name: "Made Up Sans", weight: 400, axes: { opsz: 14, CASL: 0.5 } }],
      fetch: fetchMock,
    });

    expect(family()).toBe("Made Up Sans:CASL,opsz,wght@0.5,14,400");
  });

  test("drops reserved ital and wght keys a loose family put in axes", async () => {
    const { fetchMock, family } = captureFamily();

    await googleFonts({
      families: [{ name: "Made Up Sans", weight: 400, axes: { wght: 999, ital: 1, opsz: 18 } }],
      fetch: fetchMock,
    });

    expect(family()).toBe("Made Up Sans:opsz,wght@18,400");
  });

  test("a weight range requests the variable font and leaves weight unset", async () => {
    const variableCss = `
      @font-face {
        font-family: 'Inter';
        font-style: normal;
        font-weight: 100 900;
        src: url(https://fonts.gstatic.com/inter-variable.woff2) format('woff2');
      }
    `;
    const fetchMock = cssFetch(variableCss);

    const fonts = await googleFonts({
      families: [{ name: "Inter", weight: "100..900" }],
      fetch: fetchMock,
    });

    expect(cssRequest(fetchMock).url.searchParams.get("family")).toBe("Inter:wght@100..900");
    expect(fonts).toHaveLength(1);
    expect(fonts[0]?.weight).toBeUndefined();
  });

  test("requests every family in a single css2 call", async () => {
    const fetchMock = mockInter();

    await googleFonts({
      families: ["Inter", "Noto Sans JP"],
      fetch: fetchMock,
      cache: new Map(),
    });

    const families = cssRequest(fetchMock).url.searchParams.getAll("family");
    expect(families).toEqual(["Inter:wght@400", "Noto Sans JP:wght@400"]);
    expect(fetchMock).toHaveBeenCalledTimes(1);
  });

  test("passes display through to the CSS request", async () => {
    const fetchMock = mockInter();

    await googleFonts({ families: ["Inter"], display: "swap", fetch: fetchMock });

    expect(cssRequest(fetchMock).url.searchParams.get("display")).toBe("swap");
  });

  test("reuses the CSS cache across calls, fetching metadata once", async () => {
    const fetchMock = mockInter();
    const cache = new Map<string, Promise<string>>();

    await googleFonts({ families: ["Inter"], fetch: fetchMock, cache });
    await googleFonts({ families: ["Inter"], fetch: fetchMock, cache });

    const cssCalls = fetchMock.mock.calls.filter(([url]) => url.includes("/css2"));
    expect(cssCalls).toHaveLength(1);
  });

  test("caches CSS process-wide when no cache is passed", async () => {
    const fetchMock = mockInter();

    // A unique family so no other test has warmed the shared default cache for its URL.
    await googleFonts({ families: ["Roboto Flex"], fetch: fetchMock });
    await googleFonts({ families: ["Roboto Flex"], fetch: fetchMock });

    const cssCalls = fetchMock.mock.calls.filter(([url]) => url.includes("/css2"));
    expect(cssCalls).toHaveLength(1);
  });

  test("downloads subset bytes lazily — only CSS up front", async () => {
    const fetchMock = mockInter();
    const cache = new Map<string, Promise<string>>();
    const fonts = await googleFonts({ families: ["Inter"], fetch: fetchMock, cache });

    expect(fetchMock).toHaveBeenCalledTimes(1);
    await fonts[0]!.data();
    expect(fetchMock).toHaveBeenCalledTimes(2);
  });

  test("isolates default CSS caches by fetch implementation", async () => {
    const first = mockInter();
    const second = mock(() => Promise.resolve(new Response(twoWeightCss)));

    expect(await googleFonts({ families: ["Inter"], fetch: first })).toHaveLength(3);
    expect(await googleFonts({ families: ["Inter"], fetch: second })).toHaveLength(2);
    expect(second).toHaveBeenCalledTimes(1);
  });

  test("keeps CSS in the store it is given, which drops the least recently used", async () => {
    const fetchMock = mockInter();
    const probe = new MemoryStore();
    const load = (weight: number, store: MemoryStore) =>
      googleFonts({ families: [{ name: "Inter", weight }], fetch: fetchMock, store });

    await load(100, probe);

    // Each weight's URL is as long, so every entry weighs the same.
    const store = new MemoryStore({ maxBytes: 2 * probe.usage().current });

    for (const weight of [100, 200, 300, 200]) {
      await load(weight, store);
    }

    expect(fetchMock).toHaveBeenCalledTimes(4);
    expect(store.usage().evictions).toBe(1);

    await load(100, store);
    expect(fetchMock).toHaveBeenCalledTimes(5);
  });

  test("keeps no CSS larger than the store", async () => {
    const css = `/*${"x".repeat(2048)}*/${interCss}`;
    const fetchMock = mock(() => Promise.resolve(new Response(css)));
    const store = new MemoryStore({ maxBytes: 2048 });

    await googleFonts({ families: ["Inter"], fetch: fetchMock, store });
    await googleFonts({ families: ["Inter"], fetch: fetchMock, store });
    expect(fetchMock).toHaveBeenCalledTimes(2);
    expect(store.usage().current).toBe(0);
  });

  test("keeps subset files in the store with the CSS", async () => {
    const fetchMock = mockInter();
    const store = new MemoryStore();
    const [first] = await googleFonts({ families: ["Inter"], fetch: fetchMock, store });

    await first?.data();
    await first?.data();
    expect(fetchMock).toHaveBeenCalledTimes(2);

    const disabled = mockInter();
    const [uncached] = await googleFonts({ families: ["Inter"], fetch: disabled, store: null });

    await uncached?.data();
    await uncached?.data();
    await googleFonts({ families: ["Inter"], fetch: disabled, store: null });
    expect(disabled).toHaveBeenCalledTimes(4);
  });

  test("shares concurrent default requests and retries failures", async () => {
    const response = Promise.withResolvers<Response>();
    const fetchMock = mock(() => response.promise);
    const first = googleFonts({ families: ["Inter"], fetch: fetchMock });
    const second = googleFonts({ families: ["Inter"], fetch: fetchMock });
    const settled = Promise.allSettled([first, second]);
    expect(fetchMock).toHaveBeenCalledTimes(1);
    response.reject(new Error("Request failed"));
    expect((await settled).map((result) => result.status)).toEqual(["rejected", "rejected"]);

    fetchMock.mockImplementation(() => Promise.resolve(new Response(interCss)));
    expect(await googleFonts({ families: ["Inter"], fetch: fetchMock })).toHaveLength(3);
    expect(fetchMock).toHaveBeenCalledTimes(2);
  });

  test("applies each caller's policies after warming the CSS cache", async () => {
    const fetchMock = mockInter();
    await googleFonts({ families: ["Inter"], fetch: fetchMock });

    await expect(
      googleFonts({ families: ["Inter"], fetch: fetchMock, maxBytes: 1 }),
    ).rejects.toThrow("exceeds 1 bytes");
    await expect(
      googleFonts({ families: ["Inter"], fetch: fetchMock, allowUrl: () => false }),
    ).rejects.toThrow("blocked by allowUrl");
    await expect(
      googleFonts({ families: ["Inter"], fetch: fetchMock, signal: AbortSignal.abort() }),
    ).rejects.toThrow();
    expect(fetchMock).toHaveBeenCalledTimes(1);
  });

  test("keeps a replacement cache entry when an older request fails", async () => {
    const response = Promise.withResolvers<Response>();
    const cache = new Map<string, Promise<string>>();
    const pending = googleFonts({
      families: ["Inter"],
      fetch: () => response.promise,
      cache,
    });
    const [url] = cache.keys();
    if (!url) {
      throw new Error("Expected an in-flight cache entry");
    }
    const replacement = Promise.resolve(interCss);
    cache.set(url, replacement);
    response.reject(new Error("Request failed"));

    await expect(pending).rejects.toThrow("Request failed");
    expect(cache.get(url)).toBe(replacement);
  });

  // Google's answers to `text=組選` and `text=ぁ` for Noto Sans JP: one variable face each, its
  // `unicode-range` naming the requested characters.
  const textCss = {
    組選: `@font-face {
  font-family: 'Noto Sans JP';
  font-style: normal;
  font-weight: 100 900;
  src: url(https://fonts.gstatic.com/l/font?kit=-F62fjtqLzI2JPCgQBnw7HFowxoMNeLhbmut&skey=72472b0eb8793570&v=v57) format('woff2');
  unicode-range: U+7d44, U+9078;
}`,
    ぁ: `@font-face {
  font-family: 'Noto Sans JP';
  font-style: normal;
  font-weight: 100 900;
  src: url(https://fonts.gstatic.com/l/font?kit=-F62fjtqLzI2JPCgQBnw7HFowxoJMdbk&skey=72472b0eb8793570&v=v57) format('woff2');
  unicode-range: U+3041;
}`,
  };
  const textFetch = () =>
    mock((url: string, _init?: RequestInit) => {
      const requested = new URL(url).searchParams.get("text") ?? "";

      return Promise.resolve(
        new Response(
          requested === "組選" ? textCss.組選 : requested === "ぁ" ? textCss.ぁ : interCss,
        ),
      );
    });

  test("requests text as each character once, sorted", async () => {
    const fetchMock = textFetch();

    await googleFonts({
      families: [{ name: "Noto Sans JP", weight: "100..900" }],
      text: "選組選",
      fetch: fetchMock,
      store: null,
    });

    const { url } = cssRequest(fetchMock);

    expect(url.searchParams.get("text")).toBe("組選");
    expect(url.searchParams.get("family")).toBe("Noto Sans JP:wght@100..900");
  });

  test("keys the subsets of different text apart", async () => {
    const fetchMock = textFetch();
    const load = (text: string) =>
      googleFonts({
        families: [{ name: "Noto Sans JP", weight: "100..900" }],
        text,
        fetch: fetchMock,
      });
    const [kanji] = await load("組選");
    const [kana] = await load("ぁ");

    expect(kanji?.unicodeRange).toEqual([
      [0x7d44, 0x7d44],
      [0x9078, 0x9078],
    ]);
    expect(kana?.unicodeRange).toEqual([[0x3041, 0x3041]]);
    expect(kanji?.key).not.toBe(kana?.key);
    expect(new Set([kanji?.key, kana?.key]).size).toBe(2);
  });

  test("loads range subsets when the encoded text runs past 6000 characters", async () => {
    const fetchMock = textFetch();
    // Each kanji encodes to 9 characters, so 666 of them fit and 667 do not.
    const kanji = (count: number) =>
      Array.from({ length: count }, (_, index) => String.fromCodePoint(0x4e00 + index)).join("");
    const textOf = async (text: string) => {
      await googleFonts({ families: ["Noto Sans JP"], text, fetch: fetchMock, store: null });

      return cssRequest(fetchMock).url.searchParams.get("text");
    };

    expect(await textOf(kanji(666))).toHaveLength(666);
    expect(await textOf(kanji(667))).toBeNull();
  });

  test("gives every subset the cache policy", async () => {
    const fonts = await googleFonts({ families: ["Inter"], fetch: mockInter(), cache: "none" });

    expect(fonts.map((font) => font.cache)).toEqual(["none", "none", "none"]);
    expect((await googleFonts({ families: ["Inter"], fetch: mockInter() }))[0]?.cache).toBe(
      undefined,
    );
  });

  test("returns nothing for an empty families list without a request", async () => {
    const fetchMock = mockInter();

    expect(await googleFonts({ families: [], fetch: fetchMock })).toEqual([]);
    expect(fetchMock).not.toHaveBeenCalled();
  });
});

describe("subsetFonts", () => {
  test("keeps only the subsets the text renders", async () => {
    const fonts = await googleFonts({ families: ["Inter"], fetch: mockInter() });

    expect(subsetFonts({ fonts, source: "Hi" }).map(subsetLabel)).toEqual(["Inter latin"]);
    expect(subsetFonts({ fonts, source: "Привет" }).map(subsetLabel)).toEqual(["Inter cyrillic"]);
    expect(subsetFonts({ fonts, source: "Hi Привет Γειά" }).map(subsetLabel).sort()).toEqual([
      "Inter cyrillic",
      "Inter greek",
      "Inter latin",
    ]);
  });

  test("keeps the subsets HTML character references decode to", async () => {
    const fonts = await googleFonts({ families: ["Inter"], fetch: mockInter() });

    expect(
      subsetFonts({ fonts, source: "<p>&#x41F;&#1088;&Gamma;</p>" }).map(subsetLabel).sort(),
    ).toEqual(["Inter cyrillic", "Inter greek", "Inter latin"]);
  });

  test("scans a node tree for codepoints", async () => {
    const fonts = await googleFonts({ families: ["Inter"], fetch: mockInter() });
    const node: Node = {
      type: "container",
      children: [
        { type: "text", text: "Hello" },
        { type: "container", children: [{ type: "text", text: "Γειά" }] },
      ],
    };

    expect(subsetFonts({ fonts, source: node }).map(subsetLabel).sort()).toEqual([
      "Inter greek",
      "Inter latin",
    ]);
  });

  test("keeps range-less fallbacks (full fonts, text= subsets) regardless of content", () => {
    const fallback = { name: "Local", weight: 400, data: () => Promise.resolve(bytes("x")) };
    const greek = {
      name: "Inter",
      unicodeRange: [[0x370, 0x3ff]] as [number, number][],
      data: () => Promise.resolve(bytes("g")),
    };

    // "Hi" is Latin only: the ranged Greek subset drops, the range-less fallback stays.
    expect(subsetFonts({ fonts: [fallback, greek], source: "Hi" })).toEqual([fallback]);
  });

  test("still reads the deprecated ranges", () => {
    const greek = { name: "Inter", ranges: [[0x370, 0x3ff]] as [number, number][] };

    expect(subsetFonts({ fonts: [greek], source: "Hi" })).toEqual([]);
    expect(subsetFonts({ fonts: [greek], source: "Γειά" })).toEqual([greek]);
  });

  test("keeps the subsets that checking every codepoint against every range keeps", () => {
    let seed = 1;
    const random = (limit: number) => {
      seed = (seed * 1_103_515_245 + 12_345) % 2 ** 31;

      return seed % limit;
    };
    const codepoint = () => (random(4) === 0 ? 0x1f000 + random(0x1000) : random(0x3000));
    const fonts = Array.from({ length: 400 }, (_, index) => ({
      name: `font ${index}`,
      unicodeRange: Array.from({ length: random(4) }, (): [number, number] => {
        const lo = codepoint();

        return [lo, random(5) === 0 ? lo - 1 : lo + random(64)];
      }),
    }));

    for (let round = 0; round < 50; round++) {
      const source = String.fromCodePoint(...Array.from({ length: random(40) }, codepoint));
      const codepoints = [...source].map((ch) => ch.codePointAt(0) ?? 0);
      const covering = fonts.filter(
        ({ unicodeRange }) =>
          unicodeRange.length === 0 ||
          codepoints.some((cp) => unicodeRange.some(([lo, hi]) => cp >= lo && cp <= hi)),
      );

      expect(subsetFonts({ fonts, source })).toEqual(covering);
    }
  });

  test("keeps a bare URL string (no ranges) regardless of content", () => {
    const url = "https://example.com/Inter.woff2";

    expect(subsetFonts({ fonts: [url], source: "Hi" })).toEqual([url]);
  });

  test("regression: a multi-subset family can't tofu — covering subset survives, sibling drops", async () => {
    // The bug: a glyph routes to a subset file lacking it.
    // The fix: each subset carries its unicode-range + subsetFonts keeps the covering one.
    const fonts = subsetFonts({
      fonts: await googleFonts({ families: ["Inter"], fetch: mockInter() }),
      source: "Hello",
    });

    expect(fonts.map(subsetLabel)).toEqual(["Inter latin"]);
    expect(fonts[0]?.unicodeRange).toEqual([[0x0, 0xff]]);
  });
});

describe("fontFromUrl", () => {
  test("keys by URL and fetches the bytes on demand", async () => {
    const url = "https://example.com/Inter.woff2";
    const fetchMock = mock((u: RequestInfo | URL) =>
      Promise.resolve(new Response(bytes(String(u)))),
    );
    const original = globalThis.fetch;
    globalThis.fetch = Object.assign(fetchMock, { preconnect: () => Promise.resolve() });

    try {
      const loader = fontFromUrl(url);
      expect(loader.key).toBe(url);
      expect(fetchMock).not.toHaveBeenCalled();

      const data = await loader.data();
      expect(new TextDecoder().decode(data)).toBe(url);
      expect(fetchMock).toHaveBeenCalledTimes(1);
    } finally {
      globalThis.fetch = original;
    }
  });
});
