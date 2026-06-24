import { describe, expect, mock, test } from "bun:test";
import { googleFont, googleFonts, subsetFonts } from "../src/fonts";
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

describe("googleFont", () => {
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

  test("resolves every weight to a keyed loader under one subsetOf", async () => {
    const fetchMock = mock((url: string) =>
      Promise.resolve(
        url.includes("/css2") ? new Response(twoWeightCss) : new Response(bytes(url)),
      ),
    );

    const fonts = await googleFont("Inter", { weight: [400, 700], fetch: fetchMock });

    expect(fonts).toHaveLength(2);
    expect(fonts.map((f) => f.weight).sort()).toEqual([400, 700]);
    expect(fonts.every((f) => f.subsetOf === "Inter")).toBe(true);
    expect(fonts.map((f) => f.key).sort()).toEqual([
      "https://fonts.gstatic.com/inter-400.woff2",
      "https://fonts.gstatic.com/inter-700.woff2",
    ]);
  });

  test("gives each coverage subset a distinct name so glyphs can't collide", async () => {
    const fonts = await googleFont("Inter", { fetch: mockInter() });

    // Same family, three faces — distinct names, each carrying its own range.
    expect(fonts.map((f) => f.name).sort()).toEqual([
      "Inter cyrillic",
      "Inter greek",
      "Inter latin",
    ]);
    expect(fonts.every((f) => f.subsetOf === "Inter")).toBe(true);
    expect(fonts.find((f) => f.name === "Inter latin")!.ranges).toEqual([[0x0, 0xff]]);
  });

  test("downloads bytes lazily — only the CSS is fetched up front", async () => {
    const fetchMock = mockInter();

    const fonts = await googleFont("Inter", { fetch: fetchMock });

    expect(fetchMock).toHaveBeenCalledTimes(1);

    await fonts[0]!.data();
    expect(fetchMock).toHaveBeenCalledTimes(2);
  });

  test("requests the right family/axis and a woff2 UA", async () => {
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

    await googleFont("Open Sans", {
      weight: [700, 400],
      style: ["normal", "italic"],
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

    const fonts = await googleFont("Inter", { weight: "100..900", fetch: fetchMock });

    expect(new URL(requestedUrl).searchParams.get("family")).toBe("Inter:wght@100..900");
    expect(fonts).toHaveLength(1);
    expect(fonts[0]!.weight).toBeUndefined();
  });
});

describe("googleFonts", () => {
  test("returns every subset, distinctly named, without filtering", async () => {
    const fonts = await googleFonts(["Inter"], { fetch: mockInter() });

    expect(fonts.map((f) => f.name).sort()).toEqual([
      "Inter cyrillic",
      "Inter greek",
      "Inter latin",
    ]);
  });

  test("requests every family in a single css2 call", async () => {
    let requestedUrl = "";
    const fetchMock = mock((url: string) => {
      if (url.includes("/css2")) requestedUrl = url;
      return Promise.resolve(
        url.includes("/css2") ? new Response(interCss) : new Response(bytes(url)),
      );
    });

    await googleFonts(["Inter", "Noto Sans JP"], { fetch: fetchMock });

    const families = new URL(requestedUrl).searchParams.getAll("family");
    expect(families).toEqual(["Inter:wght@400", "Noto Sans JP:wght@400"]);
    expect(fetchMock).toHaveBeenCalledTimes(1);
  });

  test("reuses the CSS cache across calls, fetching metadata once", async () => {
    const fetchMock = mockInter();
    const cache = new Map<string, string>();

    await googleFonts(["Inter"], { fetch: fetchMock, cache });
    await googleFonts(["Inter"], { fetch: fetchMock, cache });

    const cssCalls = fetchMock.mock.calls.filter(([url]) => (url as string).includes("/css2"));
    expect(cssCalls).toHaveLength(1);
  });

  test("downloads subset bytes lazily — only CSS up front", async () => {
    const fetchMock = mockInter();
    const fonts = await googleFonts(["Inter"], { fetch: fetchMock });

    expect(fetchMock).toHaveBeenCalledTimes(1);
    await fonts[0]!.data();
    expect(fetchMock).toHaveBeenCalledTimes(2);
  });
});

describe("subsetFonts", () => {
  test("keeps only the subsets the text renders", async () => {
    const fonts = await googleFonts(["Inter"], { fetch: mockInter() });

    expect(subsetFonts(fonts, "Hi").map((f) => f.name)).toEqual(["Inter latin"]);
    expect(subsetFonts(fonts, "Привет").map((f) => f.name)).toEqual(["Inter cyrillic"]);
    expect(
      subsetFonts(fonts, "Hi Привет Γειά")
        .map((f) => f.name)
        .sort(),
    ).toEqual(["Inter cyrillic", "Inter greek", "Inter latin"]);
  });

  test("scans a node tree for codepoints", async () => {
    const fonts = await googleFonts(["Inter"], { fetch: mockInter() });
    const node: Node = {
      type: "container",
      children: [
        { type: "text", text: "Hello" },
        { type: "container", children: [{ type: "text", text: "Γειά" }] },
      ],
    };

    expect(
      subsetFonts(fonts, node)
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
    expect(subsetFonts([fallback, greek], "Hi")).toEqual([fallback]);
  });

  test("regression: a multi-subset family can't tofu — covering subset survives, sibling drops", async () => {
    // The bug: same-named subsets collide so a glyph routes to a file lacking it.
    // The fix: googleFonts names subsets distinctly + subsetFonts keeps the covering one by range.
    const fonts = subsetFonts(await googleFonts(["Inter"], { fetch: mockInter() }), "Hello");

    expect(fonts).toHaveLength(1);
    expect(fonts[0]!.name).toBe("Inter latin");
    expect(fonts[0]!.subsetOf).toBe("Inter");
  });
});
