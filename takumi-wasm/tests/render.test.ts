import { afterAll, describe, expect, test } from "bun:test";
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { container, image, text } from "@takumi-rs/helpers";
import { Renderer } from "../bundlers/node";

const assetsRoot = join(import.meta.dir, "../../assets");
const fontsRoot = join(assetsRoot, "fonts");
const imagesRoot = join(assetsRoot, "images");
const fontFiles = [
  "manrope/manrope-latin-wght-normal.woff2",
  "plus-jakarta-sans/PlusJakartaSans-VariableFont_wght.woff2",
  "archivo/Archivo-VariableFont_wdth,wght.ttf",
  "twemoji/TwemojiMozilla-colr.woff2",
] as const;
const fonts = await Promise.all(
  fontFiles.map(async (file) => await readFile(join(fontsRoot, file))),
);
const renderer = new Renderer();
const rendererWithoutDefaultFonts = new Renderer();

afterAll(() => {
  renderer.free();
  rendererWithoutDefaultFonts.free();
});

const localImagePath = join(imagesRoot, "yeecord.png");
const manropeFont = await readFile(join(fontsRoot, fontFiles[0]));

const localImage = await readFile(localImagePath);
const dataUri = `data:image/png;base64,${Buffer.from(localImage).toString("base64")}`;

const node = container({
  children: [
    image({
      src: dataUri,
      width: 96,
      height: 96,
      style: {
        borderRadius: "25%",
      },
    }),
    text("Data URI"),
  ],
  style: {
    justifyContent: "center",
    alignItems: "center",
    gap: "1.5rem",
    fontSize: "1.5rem",
    backgroundColor: "white",
    width: "100%",
    height: "100%",
  },
});

describe("setup", () => {
  test(`registerFont (${fonts.length})`, async () => {
    expect(await Promise.all(fonts.map((font) => renderer.registerFont(font)))).toHaveLength(
      fonts.length,
    );
  });

  test("registerFont without default fonts", async () => {
    await rendererWithoutDefaultFonts.registerFont({
      name: "Manrope",
      data: manropeFont,
      weight: 400,
      style: "normal",
    });
  });
});

describe("render", () => {
  test("webp", async () => {
    const result = await renderer.render(node, {
      width: 1200,
      height: 630,
      format: "webp",
    });

    expect(result).toBeInstanceOf(Uint8Array);
  });

  test("png", async () => {
    const result = await renderer.render(node, {
      width: 1200,
      height: 630,
      format: "png",
    });

    expect(result).toBeInstanceOf(Uint8Array);
  });

  test("jpeg 75%", async () => {
    const result = await renderer.render(node, {
      width: 1200,
      height: 630,
      format: "jpeg",
      quality: 75,
    });

    expect(result).toBeInstanceOf(Uint8Array);
  });

  test("jpeg 100%", async () => {
    const result = await renderer.render(node, {
      width: 1200,
      height: 630,
      format: "jpeg",
      quality: 100,
    });

    expect(result).toBeInstanceOf(Uint8Array);
  });

  test("ico", async () => {
    const result = await renderer.render(node, {
      width: 256,
      height: 256,
      format: "ico",
    });

    expect(result).toBeInstanceOf(Uint8Array);
    expect(Buffer.from(result.subarray(0, 4))).toEqual(Buffer.from([0, 0, 1, 0]));
  });

  test("raw RGBA image source", async () => {
    const data = new Uint8Array(8 * 8 * 4);
    for (let i = 0; i < data.length; i += 4) {
      data[i] = 255;
      data[i + 3] = 128;
    }

    const result = await renderer.render(
      container({
        children: [image({ src: { width: 8, height: 8, data } })],
        style: { width: "100%", height: "100%", backgroundColor: "black" },
      }),
      { width: 8, height: 8, format: "raw" },
    );

    // 50% red premultiplied over black.
    expect(Math.abs((result[0] ?? 0) - 128)).toBeLessThanOrEqual(2);
    expect(result[1]).toBe(0);
    expect(result[3]).toBe(255);
  });

  test("raw RGBA image source with premultiplied bytes", async () => {
    const data = new Uint8Array(8 * 8 * 4);
    for (let i = 0; i < data.length; i += 4) {
      data[i] = 128;
      data[i + 3] = 128;
    }

    const result = await renderer.render(
      container({
        children: [image({ src: { width: 8, height: 8, data, premultiplied: true } })],
        style: { width: "100%", height: "100%", backgroundColor: "black" },
      }),
      { width: 8, height: 8, format: "raw" },
    );

    expect(Math.abs((result[0] ?? 0) - 128)).toBeLessThanOrEqual(2);
    expect(result[3]).toBe(255);
  });

  test("raw RGBA image source rejects mismatched length", async () => {
    await expect(
      renderer.render(
        container({ children: [image({ src: { width: 8, height: 8, data: new Uint8Array(4) } })] }),
        {
          width: 8,
          height: 8,
        },
      ),
    ).rejects.toThrow();
  });

  test("auto-calculated dimensions", async () => {
    const result = await renderer.render(node, {
      format: "png",
    });

    expect(result).toBeInstanceOf(Uint8Array);
  });

  test("with debug borders", async () => {
    const result = await renderer.render(node, {
      width: 1200,
      height: 630,
      format: "png",
      drawDebugBorder: true,
    });

    expect(result).toBeInstanceOf(Uint8Array);
  });

  test("with device pixel ratio 2.0", async () => {
    const result = await renderer.render(node, {
      width: 1200,
      height: 630,
      format: "png",
      devicePixelRatio: 2.0,
    });

    expect(result).toBeInstanceOf(Uint8Array);
  });

  test("with fetched resources", async () => {
    const result = await renderer.render(node, {
      width: 1200,
      height: 630,
      format: "png",
      images: [
        {
          src: "../assets/images/yeecord.png",
          data: new Uint8Array(localImage),
        },
      ],
    });

    expect(result).toBeInstanceOf(Uint8Array);
  });

  test("with no options provided", async () => {
    const result = await renderer.render(node);

    expect(result).toBeInstanceOf(Uint8Array);
  });

  test("with default fonts disabled", async () => {
    const result = await rendererWithoutDefaultFonts.render(
      text({
        text: "Manrope",
        style: {
          fontFamily: "Manrope",
          fontSize: "2rem",
        },
      }),
      {
        width: 400,
        height: 120,
        format: "png",
      },
    );

    expect(result).toBeInstanceOf(Uint8Array);
  });
});

describe("renderAsDataUrl", () => {
  test("with timeMs applied to stylesheet animation", async () => {
    const animated = await renderer.measure(
      {
        type: "container",
        tagName: "div",
      },
      {
        width: 200,
        height: 100,
        timeMs: 500,
        css: [
          `
            div {
              width: 100px;
              animation-name: grow;
              animation-duration: 1000ms;
              animation-timing-function: linear;
              animation-fill-mode: both;
            }

            @keyframes grow {
              from { width: 100px; }
              to { width: 200px; }
            }
          `,
        ],
      },
    );

    expect(animated.width).toBe(150);
  });

  test("default format (png)", async () => {
    const result = await renderer.renderAsDataUrl(node, { width: 1200, height: 630 });

    expect(result).toMatch(/^data:image\/png;base64,/);
    expect(result.length).toBeGreaterThan(100);
  });

  test("webp format", async () => {
    const result = await renderer.renderAsDataUrl(node, {
      width: 1200,
      height: 630,
      format: "webp",
    });

    expect(result).toMatch(/^data:image\/webp;base64,/);
    expect(result.length).toBeGreaterThan(100);
  });

  test("jpeg format with quality", async () => {
    const result = await renderer.renderAsDataUrl(node, {
      width: 1200,
      height: 630,
      format: "jpeg",
      quality: 75,
    });

    expect(result).toMatch(/^data:image\/jpeg;base64,/);
    expect(result.length).toBeGreaterThan(100);
  });

  test("png format explicit", async () => {
    const result = await renderer.renderAsDataUrl(node, {
      width: 1200,
      height: 630,
      format: "png",
    });

    expect(result).toMatch(/^data:image\/png;base64,/);
    expect(result.length).toBeGreaterThan(100);
  });

  test("ico format", async () => {
    const result = await renderer.renderAsDataUrl(node, {
      width: 256,
      height: 256,
      format: "ico",
    });

    expect(result).toMatch(/^data:image\/x-icon;base64,/);
    expect(result.length).toBeGreaterThan(100);
  });

  test("renderAsDataUrl with debug borders", async () => {
    const result = await renderer.renderAsDataUrl(node, {
      width: 1200,
      height: 630,
      format: "png",
      drawDebugBorder: true,
    });

    expect(result).toMatch(/^data:image\/png;base64,/);
    expect(result.length).toBeGreaterThan(100);
  });

  test("renderAsDataUrl with device pixel ratio", async () => {
    const result = await renderer.renderAsDataUrl(node, {
      width: 1200,
      height: 630,
      format: "png",
      devicePixelRatio: 2.0,
    });

    expect(result).toMatch(/^data:image\/png;base64,/);
    expect(result.length).toBeGreaterThan(100);
  });

  test("renderAsDataUrl with fetched resources", async () => {
    const result = await renderer.renderAsDataUrl(node, {
      width: 1200,
      height: 630,
      format: "png",
      images: [
        {
          src: "../assets/images/yeecord.png",
          data: new Uint8Array(localImage),
        },
      ],
    });

    expect(result).toMatch(/^data:image\/png;base64,/);
    expect(result.length).toBeGreaterThan(100);
  });

  describe("renderAnimation", () => {
    const scene = {
      node,
      durationMs: 1000,
    };

    test("webp", async () => {
      const result = await renderer.renderAnimation({
        scenes: [scene],
        width: 1200,
        height: 630,
        format: "webp",
        fps: 1,
      });

      expect(result).toBeInstanceOf(Uint8Array);
    });

    test("apng", async () => {
      const result = await renderer.renderAnimation({
        scenes: [scene],
        width: 1200,
        height: 630,
        format: "apng",
        fps: 1,
      });

      expect(result).toBeInstanceOf(Uint8Array);
    });

    test("gif", async () => {
      const result = await renderer.renderAnimation({
        scenes: [scene],
        width: 1200,
        height: 630,
        format: "gif",
        fps: 1,
      });

      expect(result).toBeInstanceOf(Uint8Array);
      expect(Buffer.from(result.subarray(0, 6)).toString("ascii")).toMatch(/^GIF8[79]a$/);
    });

    test("applies structured keyframes over the timeline", async () => {
      const base = { width: "128px", height: "128px", backgroundColor: "#ef4444" } as const;
      const staticNode = container({ style: base });
      const animatedNode = container({
        style: {
          ...base,
          animationName: "grow",
          animationDuration: "1000ms",
          animationIterationCount: "infinite",
        },
      });
      const keyframes = {
        grow: { "0%": { borderRadius: "0px" }, "100%": { borderRadius: "64px" } },
      };

      const staticOut = await renderer.renderAnimation({
        scenes: [{ node: staticNode, durationMs: 1000 }],
        width: 128,
        height: 128,
        format: "webp",
        fps: 12,
      });
      const animatedOut = await renderer.renderAnimation({
        scenes: [{ node: animatedNode, durationMs: 1000 }],
        width: 128,
        height: 128,
        format: "webp",
        fps: 12,
        keyframes,
      });

      // The static timeline dedups to one frame; the animated one keeps distinct
      // frames, so its output is much larger.
      expect(animatedOut.length).toBeGreaterThan(staticOut.length * 2);
    });
  });

  test("with structured keyframes in render options", async () => {
    const animated = await renderer.measure(
      {
        type: "container",
        tagName: "div",
      },
      {
        width: 200,
        height: 100,
        timeMs: 500,
        css: [
          `
            div {
              width: 100px;
              animation-name: grow;
              animation-duration: 1000ms;
              animation-timing-function: linear;
              animation-fill-mode: both;
            }
          `,
        ],
        keyframes: {
          grow: {
            from: {
              width: "100px",
            },
            to: {
              width: "200px",
            },
          },
        },
      },
    );

    expect(animated.width).toBe(150);
  });
});
