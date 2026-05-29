import { describe, expect, mock, test } from "bun:test";
import { font, googleFont } from "../src/fonts";

const bytes = (s: string) => new TextEncoder().encode(s).buffer;

describe("font", () => {
  test("passes raw bytes through with descriptor", async () => {
    const data = new Uint8Array([1, 2, 3]);
    const result = await font(data, { name: "Inter", weight: 700 });
    expect(result).toEqual({ name: "Inter", weight: 700, data });
  });

  test("fetches a URL source", async () => {
    const fetchMock = mock(() => Promise.resolve(new Response(bytes("ttf"))));
    const result = await font(
      "https://fonts.example.com/a.woff2",
      { name: "A" },
      { fetch: fetchMock },
    );
    expect(fetchMock).toHaveBeenCalled();
    expect(new TextDecoder().decode(result.data as ArrayBuffer)).toBe("ttf");
  });

  test("throws on non-ok response", () => {
    const fetchMock = mock(() => Promise.resolve(new Response(null, { status: 404 })));
    expect(
      font("https://fonts.example.com/missing.woff2", {}, { fetch: fetchMock }),
    ).rejects.toThrow("HTTP 404");
  });
});

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

  test("resolves every weight in the CSS to a descriptor", async () => {
    const fetchMock = mock((url: string) =>
      Promise.resolve(
        url.endsWith(".css2") || url.includes("/css2")
          ? new Response(css)
          : new Response(bytes(url)),
      ),
    );

    const fonts = await googleFont("Inter", { weights: [400, 700], fetch: fetchMock });

    expect(fonts).toHaveLength(2);
    expect(fonts.map((f) => f.weight).sort()).toEqual([400, 700]);
    expect(fonts.every((f) => f.name === "Inter")).toBe(true);
  });

  test("requests the right family/axis and a woff2 UA", async () => {
    let requestedUrl = "";
    let ua: string | null = null;
    const fetchMock = mock((url: string, init?: RequestInit) => {
      if (url.includes("/css2")) {
        requestedUrl = url;
        ua = new Headers(init?.headers).get("User-Agent");
        return Promise.resolve(new Response(css));
      }
      return Promise.resolve(new Response(bytes(url)));
    });

    await googleFont("Open Sans", {
      weights: [700, 400],
      styles: ["normal", "italic"],
      fetch: fetchMock,
    });

    expect(requestedUrl).toContain("family=Open+Sans:ital,wght@0,400;0,700;1,400;1,700");
    expect(ua).toContain("Chrome");
  });
});
