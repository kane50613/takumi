import { expect, mock, test } from "bun:test";
import { render, renderAnimation } from "../src";

const square = `<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"><rect width="8" height="8" fill="#f00"/></svg>`;
const twemoji = "https://cdn.jsdelivr.net/gh/jdecked/twemoji@17.0.2/assets/svg/1f600.svg";
const html = '<p style="margin: 0; font-size: 32px">😀</p>';

/** A fetch that serves a red square for every URL. */
const cdn = () => mock(async (_url: string | URL | Request) => new Response(square));

/** Whether the raw RGBA pixels hold an opaque red pixel. */
const hasRed = (pixels: Uint8Array) => {
  for (let index = 0; index < pixels.length; index += 4) {
    if (
      (pixels[index] ?? 0) > 248 &&
      (pixels[index + 1] ?? 0) < 8 &&
      (pixels[index + 2] ?? 0) < 8 &&
      (pixels[index + 3] ?? 0) > 248
    ) {
      return true;
    }
  }

  return false;
};

test("draws each emoji as a glyph of the Twemoji image font", async () => {
  const fetch = cdn();
  const pixels = await render(html, {
    width: 40,
    height: 40,
    format: "raw",
    images: { fetch },
  });

  expect(fetch.mock.calls.map(([url]) => String(url))).toEqual([twemoji]);
  expect(hasRed(pixels)).toBe(true);
});

test("leaves emoji to the fonts with from-font", async () => {
  const fetch = cdn();

  await render(html, {
    width: 40,
    height: 40,
    format: "raw",
    emoji: "from-font",
    images: { fetch },
  });

  expect(fetch).not.toHaveBeenCalled();
});

test("fetches the emoji every scene of an animation draws", async () => {
  const fetch = cdn();

  await renderAnimation({
    width: 40,
    height: 40,
    fps: 1,
    format: "gif",
    images: { fetch },
    scenes: [
      { node: html, durationMs: 1000 },
      { node: html.replace("😀", "🚀"), durationMs: 1000 },
    ],
  });

  expect(fetch.mock.calls.map(([url]) => String(url)).sort()).toEqual([
    twemoji,
    "https://cdn.jsdelivr.net/gh/jdecked/twemoji@17.0.2/assets/svg/1f680.svg",
  ]);
});
