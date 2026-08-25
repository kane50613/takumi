import { describe, expect, mock, test } from "bun:test";
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

const mockInter = () =>
  mock((url: string) =>
    Promise.resolve(url.includes("/css2") ? new Response(interCss) : new Response(bytes(url))),
  );

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

  test("gives each coverage subset a distinct name so glyphs can't collide", async () => {
    const fonts = await googleFonts({ families: ["Inter"], fetch: mockInter() });

    // Same family, three faces — distinct names, each carrying its own range.
    expect(fonts.map((f) => f.name).sort()).toEqual([
      "Inter cyrillic",
      "Inter greek",
      "Inter latin",
    ]);
    expect(fonts.every((f) => f.subsetOf === "Inter")).toBe(true);
    expect(fonts.find((f) => f.name === "Inter latin")!.ranges).toEqual([[0x0, 0xff]]);
  });

  test("ranks each subset by the lowest codepoint it declares", async () => {
    const fonts = await googleFonts({ families: ["Inter"], fetch: mockInter() });
    const ranked = [...fonts].sort((a, b) => a.subsetRank - b.subsetRank).map((f) => f.name);

    // Google's Cyrillic and Greek subsets also encode the ASCII space and the Latin
    // capitals, so `latin` has to be reached first or those codepoints defect to them.
    expect(ranked[0]).toBe("Inter latin");
    expect(fonts.find((f) => f.name === "Inter latin")!.subsetRank).toBe(0);
  });

  test("resolves every weight to a keyed loader under one subsetOf", async () => {
    const fetchMock = mock((url: string) =>
      Promise.resolve(
        url.includes("/css2") ? new Response(twoWeightCss) : new Response(bytes(url)),
      ),
    );

    const fonts = await googleFonts({
      families: [{ name: "Inter", weight: [400, 700] }],
      fetch: fetchMock,
    });

    expect(fonts.map((f) => f.weight).sort()).toEqual([400, 700]);
    expect(fonts.every((f) => f.subsetOf === "Inter")).toBe(true);
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
    const fetchMock = mock((url: string) =>
      Promise.resolve(url.includes("/css2") ? new Response(variableCss) : new Response(bytes(url))),
    );

    const fonts = await googleFonts({
      families: [{ name: "Public Sans", weight: [400, 700] }],
      fetch: fetchMock,
    });

    expect(fonts).toHaveLength(1);
    expect(fonts[0]?.weight).toBeUndefined();

    // A static font keeps a distinct url per weight, so faces stay split.
    const staticMock = mock((url: string) =>
      Promise.resolve(
        url.includes("/css2") ? new Response(twoWeightCss) : new Response(bytes(url)),
      ),
    );
    const staticFonts = await googleFonts({
      families: [{ name: "Inter", weight: [400, 700] }],
      fetch: staticMock,
    });
    expect(staticFonts.map((f) => f.weight).sort()).toEqual([400, 700]);
  });

  test("builds the family/axis and sends a woff2 UA", async () => {
    let requestedUrl = "";
    let ua = "";
    const fetchMock = mock((url: string, init?: RequestInit) => {
      if (url.includes("/css2")) {
        requestedUrl = url;
        ua = new Headers(init?.headers).get("User-Agent") ?? "";
        return Promise.resolve(new Response(twoWeightCss));
      }
      return Promise.resolve(new Response(bytes(url)));
    });

    await googleFonts({
      families: [{ name: "Open Sans", weight: [700, 400], style: ["normal", "italic"] }],
      fetch: fetchMock,
    });

    expect(new URL(requestedUrl).searchParams.get("family")).toBe(
      "Open Sans:ital,wght@0,400;0,700;1,400;1,700",
    );
    expect(ua).toContain("Chrome");
  });

  const captureFamily = () => {
    let requestedUrl = "";
    const fetchMock = mock((url: string) => {
      if (url.includes("/css2")) requestedUrl = url;
      return Promise.resolve(new Response(twoWeightCss));
    });
    return { fetchMock, family: () => new URL(requestedUrl).searchParams.get("family") };
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
    let requestedUrl = "";
    const fetchMock = mock((url: string) => {
      if (url.includes("/css2")) requestedUrl = url;
      return Promise.resolve(
        url.includes("/css2") ? new Response(variableCss) : new Response(bytes(url)),
      );
    });

    const fonts = await googleFonts({
      families: [{ name: "Inter", weight: "100..900" }],
      fetch: fetchMock,
    });

    expect(new URL(requestedUrl).searchParams.get("family")).toBe("Inter:wght@100..900");
    expect(fonts).toHaveLength(1);
    expect(fonts[0]?.weight).toBeUndefined();
  });

  test("requests every family in a single css2 call", async () => {
    let requestedUrl = "";
    const fetchMock = mock((url: string) => {
      if (url.includes("/css2")) requestedUrl = url;
      return Promise.resolve(
        url.includes("/css2") ? new Response(interCss) : new Response(bytes(url)),
      );
    });

    await googleFonts({
      families: ["Inter", "Noto Sans JP"],
      fetch: fetchMock,
      cache: new Map(),
    });

    const families = new URL(requestedUrl).searchParams.getAll("family");
    expect(families).toEqual(["Inter:wght@400", "Noto Sans JP:wght@400"]);
    expect(fetchMock).toHaveBeenCalledTimes(1);
  });

  test("passes display through to the CSS request", async () => {
    let requestedUrl = "";
    const fetchMock = mock((url: string) => {
      if (url.includes("/css2")) requestedUrl = url;
      return Promise.resolve(
        url.includes("/css2") ? new Response(interCss) : new Response(bytes(url)),
      );
    });

    await googleFonts({ families: ["Inter"], display: "swap", fetch: fetchMock });

    const params = new URL(requestedUrl).searchParams;
    expect(params.get("display")).toBe("swap");
  });

  test("reuses the CSS cache across calls, fetching metadata once", async () => {
    const fetchMock = mockInter();
    const cache = new Map<string, Promise<string>>();

    await googleFonts({ families: ["Inter"], fetch: fetchMock, cache });
    await googleFonts({ families: ["Inter"], fetch: fetchMock, cache });

    const cssCalls = fetchMock.mock.calls.filter(([url]) => (url as string).includes("/css2"));
    expect(cssCalls).toHaveLength(1);
  });

  test("a CSS cache hit rechecks allowUrl and keeps the entry reusable", async () => {
    const fetchMock = mockInter();
    const cache = new Map<string, Promise<string>>();

    await googleFonts({ families: ["Inter"], fetch: fetchMock, cache });
    expect(
      googleFonts({ families: ["Inter"], fetch: fetchMock, cache, allowUrl: () => false }),
    ).rejects.toThrow(/blocked by allowUrl/);
    await googleFonts({ families: ["Inter"], fetch: fetchMock, cache });

    const cssCalls = fetchMock.mock.calls.filter(([url]) => (url as string).includes("/css2"));
    expect(cssCalls).toHaveLength(1);
  });

  test("a CSS cache hit rechecks maxBytes and keeps the entry reusable", async () => {
    const fetchMock = mockInter();
    const cache = new Map<string, Promise<string>>();

    await googleFonts({ families: ["Inter"], fetch: fetchMock, cache });
    expect(
      googleFonts({ families: ["Inter"], fetch: fetchMock, cache, maxBytes: 1 }),
    ).rejects.toThrow("Response exceeds 1 bytes");
    await googleFonts({ families: ["Inter"], fetch: fetchMock, cache });

    const cssCalls = fetchMock.mock.calls.filter(([url]) => (url as string).includes("/css2"));
    expect(cssCalls).toHaveLength(1);
  });

  test("caches CSS process-wide when no cache is passed", async () => {
    const fetchMock = mockInter();

    // A unique family so no other test has warmed the shared default cache for its URL.
    await googleFonts({ families: ["Roboto Flex"], fetch: fetchMock });
    await googleFonts({ families: ["Roboto Flex"], fetch: fetchMock });

    const cssCalls = fetchMock.mock.calls.filter(([url]) => (url as string).includes("/css2"));
    expect(cssCalls).toHaveLength(1);
  });

  test("the process-wide CSS cache rechecks allowUrl", async () => {
    const fetchMock = mockInter();
    const families = ["Policy Cache Test"];

    await googleFonts({ families, fetch: fetchMock });
    expect(googleFonts({ families, fetch: fetchMock, allowUrl: () => false })).rejects.toThrow(
      /blocked by allowUrl/,
    );

    const cssCalls = fetchMock.mock.calls.filter(([url]) => (url as string).includes("/css2"));
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

  test("returns nothing for an empty families list without a request", async () => {
    const fetchMock = mockInter();

    expect(await googleFonts({ families: [], fetch: fetchMock })).toEqual([]);
    expect(fetchMock).not.toHaveBeenCalled();
  });
});

describe("subsetFonts", () => {
  test("keeps only the subsets the text renders", async () => {
    const fonts = await googleFonts({ families: ["Inter"], fetch: mockInter() });

    expect(subsetFonts({ fonts, source: "Hi" }).map((f) => f.name)).toEqual(["Inter latin"]);
    expect(subsetFonts({ fonts, source: "Привет" }).map((f) => f.name)).toEqual(["Inter cyrillic"]);
    expect(
      subsetFonts({ fonts, source: "Hi Привет Γειά" })
        .map((f) => f.name)
        .sort(),
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

    expect(
      subsetFonts({ fonts, source: node })
        .map((f) => f.name)
        .sort(),
    ).toEqual(["Inter greek", "Inter latin"]);
  });

  test("keeps range-less fallbacks (full fonts, text= subsets) regardless of content", () => {
    const fallback = { name: "Local", weight: 400, data: () => Promise.resolve(bytes("x")) };
    const greek = {
      name: "Inter greek",
      ranges: [[0x370, 0x3ff]] as [number, number][],
      data: () => Promise.resolve(bytes("g")),
    };

    // "Hi" is Latin only: the ranged Greek subset drops, the range-less fallback stays.
    expect(subsetFonts({ fonts: [fallback, greek], source: "Hi" })).toEqual([fallback]);
  });

  test("keeps a bare URL string (no ranges) regardless of content", () => {
    const url = "https://example.com/Inter.woff2";

    expect(subsetFonts({ fonts: [url], source: "Hi" })).toEqual([url]);
  });

  test("regression: a multi-subset family can't tofu — covering subset survives, sibling drops", async () => {
    // The bug: same-named subsets collide so a glyph routes to a file lacking it.
    // The fix: googleFonts names subsets distinctly + subsetFonts keeps the covering one by range.
    const fonts = subsetFonts({
      fonts: await googleFonts({ families: ["Inter"], fetch: mockInter() }),
      source: "Hello",
    });

    expect(fonts).toHaveLength(1);
    expect(fonts[0]!.name).toBe("Inter latin");
    expect(fonts[0]!.subsetOf).toBe("Inter");
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
