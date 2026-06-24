import { describe, expect, mock, test } from "bun:test";
import { googleFonts, subsetFonts } from "../src/fonts";
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

  test("resolves every weight to a keyed loader under one subsetOf", async () => {
    const fetchMock = mock((url: string) =>
      Promise.resolve(
        url.includes("/css2") ? new Response(twoWeightCss) : new Response(bytes(url)),
      ),
    );

    const fonts = await googleFonts({
      families: [{ family: "Inter", weight: [400, 700] }],
      fetch: fetchMock,
    });

    expect(fonts.map((f) => f.weight).sort()).toEqual([400, 700]);
    expect(fonts.every((f) => f.subsetOf === "Inter")).toBe(true);
    // Keys are stable composites of identity, not the woff2 URL, and unique per face.
    expect(new Set(fonts.map((f) => f.key)).size).toBe(2);
    expect(fonts.every((f) => !f.key.includes("gstatic"))).toBe(true);
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
      families: [{ family: "Open Sans", weight: [700, 400], style: ["normal", "italic"] }],
      fetch: fetchMock,
    });

    expect(new URL(requestedUrl).searchParams.get("family")).toBe(
      "Open Sans:ital,wght@0,400;0,700;1,400;1,700",
    );
    expect(ua).toContain("Chrome");
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
      families: [{ family: "Inter", weight: "100..900" }],
      fetch: fetchMock,
    });

    expect(new URL(requestedUrl).searchParams.get("family")).toBe("Inter:wght@100..900");
    expect(fonts).toHaveLength(1);
    expect(fonts[0]!.weight).toBeUndefined();
  });

  test("requests every family in a single css2 call", async () => {
    let requestedUrl = "";
    const fetchMock = mock((url: string) => {
      if (url.includes("/css2")) requestedUrl = url;
      return Promise.resolve(
        url.includes("/css2") ? new Response(interCss) : new Response(bytes(url)),
      );
    });

    await googleFonts({ families: ["Inter", "Noto Sans JP"], fetch: fetchMock });

    const families = new URL(requestedUrl).searchParams.getAll("family");
    expect(families).toEqual(["Inter:wght@400", "Noto Sans JP:wght@400"]);
    expect(fetchMock).toHaveBeenCalledTimes(1);
  });

  test("passes text and display through to the CSS request", async () => {
    let requestedUrl = "";
    const fetchMock = mock((url: string) => {
      if (url.includes("/css2")) requestedUrl = url;
      return Promise.resolve(
        url.includes("/css2") ? new Response(interCss) : new Response(bytes(url)),
      );
    });

    await googleFonts({ families: ["Inter"], text: "Hello", display: "swap", fetch: fetchMock });

    const params = new URL(requestedUrl).searchParams;
    expect(params.get("text")).toBe("Hello");
    expect(params.get("display")).toBe("swap");
  });

  test("reuses the CSS cache across calls, fetching metadata once", async () => {
    const fetchMock = mockInter();
    const cache = new Map<string, string>();

    await googleFonts({ families: ["Inter"], fetch: fetchMock, cache });
    await googleFonts({ families: ["Inter"], fetch: fetchMock, cache });

    const cssCalls = fetchMock.mock.calls.filter(([url]) => (url as string).includes("/css2"));
    expect(cssCalls).toHaveLength(1);
  });

  test("downloads subset bytes lazily — only CSS up front", async () => {
    const fetchMock = mockInter();
    const fonts = await googleFonts({ families: ["Inter"], fetch: fetchMock });

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
