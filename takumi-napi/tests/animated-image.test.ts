import { describe, expect, test } from "bun:test";
import { Renderer } from "../src/export";

const renderer = new Renderer();

/** A 20px white square with a 4px red square at (8, 8). */
const marker =
  '<div style="position: relative; width: 20px; height: 20px; background: white"><div style="position: absolute; left: 8px; top: 8px; width: 4px; height: 4px; background: red"></div></div>';

/** Where the red pixels of a raw render `width` device pixels wide land, in CSS pixels. */
function redBox(pixels: Uint8Array, width: number, devicePixelRatio: number) {
  const xs: number[] = [];
  const ys: number[] = [];

  for (let index = 0; index < pixels.length; index += 4) {
    const [red = 0, green = 0] = pixels.subarray(index, index + 2);

    if (red > 128 && green < 128) {
      xs.push((index / 4) % width);
      ys.push(Math.floor(index / 4 / width));
    }
  }

  if (xs.length === 0) {
    return undefined;
  }

  const left = Math.min(...xs);
  const top = Math.min(...ys);

  return {
    x: left / devicePixelRatio,
    y: top / devicePixelRatio,
    width: (Math.max(...xs) + 1 - left) / devicePixelRatio,
    height: (Math.max(...ys) + 1 - top) / devicePixelRatio,
  };
}

describe.each(["gif", "apng", "webp"] as const)("an animated %s", (format) => {
  const encoded = renderer.renderAnimation({
    width: 20,
    height: 20,
    fps: 1,
    format,
    scenes: [{ node: marker, durationMs: 1000 }],
  });

  test("draws across its box at each device pixel ratio", async () => {
    const data = await encoded;

    for (const devicePixelRatio of [1, 2]) {
      const pixels = await renderer.render(
        { type: "image", src: data },
        { devicePixelRatio, format: "raw" },
      );

      expect(redBox(pixels, 20 * devicePixelRatio, devicePixelRatio)).toEqual({
        x: 8,
        y: 8,
        width: 4,
        height: 4,
      });
    }
  });

  test("draws as a background image", async () => {
    const data = await encoded;
    const pixels = await renderer.render(
      {
        type: "container",
        style: {
          width: 20,
          height: 20,
          backgroundImage: "url(marker)",
          backgroundSize: "100% 100%",
        },
      },
      { width: 20, height: 20, format: "raw", images: [{ src: "marker", data }] },
    );

    expect(redBox(pixels, 20, 1)).toEqual({ x: 8, y: 8, width: 4, height: 4 });
  });
});
