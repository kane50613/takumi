import { describe, expect, mock, test } from "bun:test";
import { googleFont, googleFontSubsets } from "../src/fonts";
import type { Node } from "../src/types";

const bytes = (s: string) => new TextEncoder().encode(s).buffer;

describe("googleFont", () => {
  const css = `
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

  test("resolves every weight in the CSS to a keyed loader", async () => {
    const fetchMock = mock((url: string) =>
      Promise.resolve(url.includes("/css2") ? new Response(css) : new Response(bytes(url))),
    );

    const fonts = await googleFont("Inter", { weight: [400, 700], fetch: fetchMock });

    expect(fonts).toHaveLength(2);
    expect(fonts.map((f) => f.weight).sort()).toEqual([400, 700]);
    expect(fonts.every((f) => f.name === "Inter")).toBe(true);
    expect(fonts.map((f) => f.key).sort()).toEqual([
      "https://fonts.gstatic.com/inter-400.woff2",
      "https://fonts.gstatic.com/inter-700.woff2",
    ]);
  });

  test("downloads bytes lazily — only the CSS is fetched up front", async () => {
    const robotoCss = `
      @font-face {
        font-family: 'Roboto';
        font-style: normal;
        font-weight: 400;
        src: url(https://fonts.gstatic.com/roboto-400.woff2) format('woff2');
      }
    `;
    const fetchMock = mock((url: string) =>
      Promise.resolve(url.includes("/css2") ? new Response(robotoCss) : new Response(bytes(url))),
    );

    const fonts = await googleFont("Roboto", { weight: 400, fetch: fetchMock });

    // Only the CSS request so far — no font bytes fetched until data() is called.
    expect(fetchMock).toHaveBeenCalledTimes(1);

    const [loader] = fonts;
    const data = await loader!.data();
    expect(fetchMock).toHaveBeenCalledTimes(2);
    expect(new TextDecoder().decode(data)).toBe("https://fonts.gstatic.com/roboto-400.woff2");
  });

  test("attaches an abort signal to every request when timeout is set", async () => {
    const inits: (RequestInit | undefined)[] = [];
    const fetchMock = mock((url: string, init?: RequestInit) => {
      inits.push(init);
      return Promise.resolve(url.includes("/css2") ? new Response(css) : new Response(bytes(url)));
    });

    const fonts = await googleFont("Inter", { weight: 400, timeout: 5000, fetch: fetchMock });
    await fonts[0]!.data();

    expect(inits).toHaveLength(2);
    expect(inits.every((init) => init?.signal instanceof AbortSignal)).toBe(true);
  });

  test("requests the right family/axis and a woff2 UA", async () => {
    let requestedUrl = "";
    let ua = "";
    const fetchMock = mock((url: string, init?: RequestInit) => {
      if (url.includes("/css2")) {
        requestedUrl = url;
        ua = new Headers(init?.headers).get("User-Agent") ?? "";
        return Promise.resolve(new Response(css));
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

  test("a single weight builds a `wght@<n>` axis", async () => {
    let requestedUrl = "";
    const fetchMock = mock((url: string) => {
      if (url.includes("/css2")) requestedUrl = url;
      return Promise.resolve(url.includes("/css2") ? new Response(css) : new Response(bytes(url)));
    });

    await googleFont("Inter", { weight: 700, fetch: fetchMock });

    expect(new URL(requestedUrl).searchParams.get("family")).toBe("Inter:wght@700");
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
    expect(fonts[0]!.key).toBe("https://fonts.gstatic.com/inter-variable.woff2");
  });
});

describe("googleFontSubsets", () => {
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

  const mockFetch = () =>
    mock((url: string) =>
      Promise.resolve(url.includes("/css2") ? new Response(interCss) : new Response(bytes(url))),
    );

  test("keeps only the intersecting subsets, each uniquely named under one subsetOf", async () => {
    const fonts = await googleFontSubsets("Hi Привет Γειά", ["Inter"], { fetch: mockFetch() });

    expect(fonts.map((f) => f.name).sort()).toEqual([
      "Inter cyrillic",
      "Inter greek",
      "Inter latin",
    ]);
    expect(fonts.every((f) => f.subsetOf === "Inter")).toBe(true);
  });

  test("requests every family in a single css2 call", async () => {
    let requestedUrl = "";
    const fetchMock = mock((url: string) => {
      if (url.includes("/css2")) requestedUrl = url;
      return Promise.resolve(
        url.includes("/css2") ? new Response(interCss) : new Response(bytes(url)),
      );
    });

    await googleFontSubsets("Hi", ["Inter", "Noto Sans JP"], { fetch: fetchMock });

    const families = new URL(requestedUrl).searchParams.getAll("family");
    expect(families).toEqual(["Inter:wght@400", "Noto Sans JP:wght@400"]);
    expect(fetchMock).toHaveBeenCalledTimes(1);
  });

  test("scans a node tree for codepoints", async () => {
    const node: Node = {
      type: "container",
      children: [
        { type: "text", text: "Hello" },
        { type: "container", children: [{ type: "text", text: "Γειά" }] },
      ],
    };

    const fonts = await googleFontSubsets(node, ["Inter"], { fetch: mockFetch() });

    expect(fonts.map((f) => f.name).sort()).toEqual(["Inter greek", "Inter latin"]);
  });

  test("reuses the CSS cache across renders, fetching metadata once", async () => {
    const fetchMock = mockFetch();
    const cache = new Map<string, string>();

    await googleFontSubsets("Hello", ["Inter"], { fetch: fetchMock, cache });
    await googleFontSubsets("Привет", ["Inter"], { fetch: fetchMock, cache });

    // One CSS fetch total — the second render hits the cache.
    const cssCalls = fetchMock.mock.calls.filter(([url]) => (url as string).includes("/css2"));
    expect(cssCalls).toHaveLength(1);
  });

  test("downloads subset bytes lazily — only CSS up front", async () => {
    const fetchMock = mockFetch();
    const fonts = await googleFontSubsets("Hello", ["Inter"], { fetch: fetchMock });

    expect(fetchMock).toHaveBeenCalledTimes(1);
    const data = await fonts[0]!.data();
    expect(fetchMock).toHaveBeenCalledTimes(2);
    expect(new TextDecoder().decode(data)).toBe("https://fonts.gstatic.com/inter-latin.woff2");
  });
});
