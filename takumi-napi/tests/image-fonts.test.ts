import { describe, expect, mock, test } from "bun:test";
import { Renderer } from "../src/export";

const square = (fill: string) =>
  new TextEncoder().encode(
    `<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"><rect width="8" height="8" fill="${fill}"/></svg>`,
  );

/** Whether the raw RGBA pixels hold a pixel close to `[red, green, blue]`. */
const hasColor = (pixels: Uint8Array, [red, green, blue]: [number, number, number]) => {
  for (let index = 0; index < pixels.length; index += 4) {
    if (
      Math.abs((pixels[index] ?? 0) - red) < 8 &&
      Math.abs((pixels[index + 1] ?? 0) - green) < 8 &&
      Math.abs((pixels[index + 2] ?? 0) - blue) < 8 &&
      (pixels[index + 3] ?? 0) > 248
    ) {
      return true;
    }
  }

  return false;
};

const options = { width: 64, height: 40, format: "raw" as const };
const html = `<p style="margin: 0; font-size: 32px">😀🚀</p>`;

describe("image fonts", () => {
  test("needs reports the clusters an image font lacks", async () => {
    const needs = await new Renderer().needs(html, {
      ...options,
      imageFonts: [{ name: "Emoji", glyphs: [{ cluster: "😀", data: square("red") }] }],
    });

    expect(needs.glyphs).toEqual([{ name: "Emoji", clusters: ["🚀"] }]);
  });

  test("a render draws each cluster's image", async () => {
    const pixels = await new Renderer().render(html, {
      ...options,
      imageFonts: [{ name: "Emoji", glyphs: [{ cluster: "😀", data: square("red") }] }],
    });

    expect(hasColor(pixels, [255, 0, 0])).toBe(true);
  });

  test("prepare loads the clusters a source draws, leaving out one that fails", async () => {
    const fetch = mock(async (input: RequestInfo | URL) =>
      String(input).endsWith("/1f680.svg")
        ? new Response("not found", { status: 404 })
        : new Response(square("blue")),
    );
    const renderer = new Renderer();
    const prepared = await renderer.prepare(html, {
      ...options,
      images: { fetch },
      imageFonts: [
        {
          name: "Emoji",
          load: (cluster) => `https://cdn.test/${cluster.codePointAt(0)?.toString(16)}.svg`,
        },
      ],
    });
    const pixels = await renderer.render(prepared, { format: "raw" });

    expect(fetch.mock.calls.map(([url]) => String(url)).sort()).toEqual([
      "https://cdn.test/1f600.svg",
      "https://cdn.test/1f680.svg",
    ]);
    expect(hasColor(pixels, [0, 0, 255])).toBe(true);
  });
});
