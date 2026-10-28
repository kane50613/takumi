import { describe, expect, mock, test } from "bun:test";
import { type NeedsRequest, prepareSource, renderPrepared } from "../src/renderer";
import type { ImageFont } from "../src/renderer";
import { fontBackend } from "./font-backend";

const svg = (label: string) => new TextEncoder().encode(`<svg id="${label}"/>`);

/** A fetch that serves an SVG for each URL under `https://cdn.test/` but `missing`, and 404 otherwise. */
const cdnFetch = mock(async (input: RequestInfo | URL) => {
  const url = String(input);

  return url.endsWith("/missing.svg")
    ? new Response("not found", { status: 404 })
    : new Response(svg(url));
});

/** Prepares a source whose needs report `clusters` for the image font `Emoji`, and returns the image fonts a render gets. */
async function prepared(clusters: string[], glyphs: ImageFont["glyphs"] = []) {
  const { fonts } = fontBackend();
  const requests: NeedsRequest[] = [];
  const source = await prepareSource(
    fonts,
    "source",
    {},
    {
      images: { fetch: cdnFetch },
      imageFonts: [
        {
          name: "Emoji",
          glyphs,
          load: (cluster) =>
            cluster === "🚫" ? "https://cdn.test/missing.svg" : `https://cdn.test/${cluster}.svg`,
        },
      ],
    },
    (request) => {
      requests.push(request);

      return {
        images: [],
        characters: clusters.join(""),
        faces: [],
        glyphs: [{ name: "Emoji", clusters }],
      };
    },
  );
  let imageFonts: ImageFont[] | undefined;

  await renderPrepared(fonts, source, {}, (_source, options) => {
    imageFonts = options.imageFonts;
  });

  return { requests, imageFonts };
}

describe("prepare with image fonts", () => {
  test("asks needs for each image font by name", async () => {
    const { requests } = await prepared([]);

    expect(requests.map((request) => request.imageFonts)).toEqual([[{ name: "Emoji" }]]);
  });

  test("loads the clusters needs reports, leaving out one that fails to fetch", async () => {
    const { imageFonts } = await prepared(["😀", "🚫"]);

    expect(imageFonts?.map((font) => font.name)).toEqual(["Emoji"]);
    expect(imageFonts?.[0]?.glyphs?.map((glyph) => glyph.cluster)).toEqual(["😀"]);
    expect(new TextDecoder().decode(imageFonts?.[0]?.glyphs?.[0]?.data as ArrayBuffer)).toBe(
      '<svg id="https://cdn.test/😀.svg"/>',
    );
  });

  test("keeps the glyphs it is given without loading them again", async () => {
    cdnFetch.mockClear();

    const held = { cluster: "😀", data: svg("held") };
    const { imageFonts } = await prepared(["😀"], [held]);

    expect(imageFonts?.[0]?.glyphs).toEqual([held]);
    expect(cdnFetch).not.toHaveBeenCalled();
  });
});
