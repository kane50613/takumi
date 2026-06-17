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
const rendererWithoutDefaultFonts = new Renderer({ loadDefaultFonts: false });

afterAll(() => {
  renderer.free();
  rendererWithoutDefaultFonts.free();
});

const localImagePath = join(imagesRoot, "yeecord.png");
const [manropeFont] = fonts;

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
  test(`loadFonts (${fonts.length})`, async () => {
    expect(await renderer.loadFonts(fonts)).toBe(fonts.length);
  });

  test("loadFont without default fonts", () => {
    rendererWithoutDefaultFonts.loadFont({
      name: "Manrope",
      data: manropeFont,
      weight: 400,
      style: "normal",
    });
  });

  test("putPersistentImage", () => {
    renderer.putPersistentImage({
      src: localImagePath,
      data: new Uint8Array(localImage),
    });
  });

  test("putPersistentImage caches by src", async () => {
    using cacheRenderer = new Renderer();
    let loadCount = 0;
    const source = {
      src: `${localImagePath}?cached`,
      data: async () => {
        loadCount += 1;
        return new Uint8Array(localImage);
      },
    };

    await Promise.all([
      cacheRenderer.putPersistentImage(source),
      cacheRenderer.putPersistentImage(source),
      cacheRenderer.putPersistentImage(source),
    ]);

    expect(loadCount).toBe(1);
  });

  test("clearImageStore resets persistent image cache", () => {
    using cacheRenderer = new Renderer();
    let loadCount = 0;
    const source = {
      src: `${localImagePath}?clear-cache`,
      data: () => {
        loadCount += 1;
        return new Uint8Array(localImage);
      },
    };

    cacheRenderer.putPersistentImage(source);
    cacheRenderer.clearImageStore();
    cacheRenderer.putPersistentImage(source);

    expect(loadCount).toBe(2);
  });
});

describe("render", () => {
  test("webp", () => {
    const result = renderer.render(node, {
      width: 1200,
      height: 630,
      format: "webp",
    });

    expect(result).toBeInstanceOf(Uint8Array);
  });

  test("png", () => {
    const result = renderer.render(node, {
      width: 1200,
      height: 630,
      format: "png",
    });

    expect(result).toBeInstanceOf(Uint8Array);
  });

  test("jpeg 75%", () => {
    const result = renderer.render(node, {
      width: 1200,
      height: 630,
      format: "jpeg",
      quality: 75,
    });

    expect(result).toBeInstanceOf(Uint8Array);
  });

  test("jpeg 100%", () => {
    const result = renderer.render(node, {
      width: 1200,
      height: 630,
      format: "jpeg",
      quality: 100,
    });

    expect(result).toBeInstanceOf(Uint8Array);
  });

  test("ico", () => {
    const result = renderer.render(node, {
      width: 256,
      height: 256,
      format: "ico",
    });

    expect(result).toBeInstanceOf(Uint8Array);
    expect(Buffer.from(result.subarray(0, 4))).toEqual(Buffer.from([0, 0, 1, 0]));
  });

  test("auto-calculated dimensions", () => {
    const result = renderer.render(node, {
      format: "png",
    });

    expect(result).toBeInstanceOf(Uint8Array);
  });

  test("with debug borders", () => {
    const result = renderer.render(node, {
      width: 1200,
      height: 630,
      format: "png",
      drawDebugBorder: true,
    });

    expect(result).toBeInstanceOf(Uint8Array);
  });

  test("with device pixel ratio 2.0", () => {
    const result = renderer.render(node, {
      width: 1200,
      height: 630,
      format: "png",
      devicePixelRatio: 2.0,
    });

    expect(result).toBeInstanceOf(Uint8Array);
  });

  test("with fetched resources", () => {
    const result = renderer.render(node, {
      width: 1200,
      height: 630,
      format: "png",
      fetchedResources: [
        {
          src: "../assets/images/yeecord.png",
          data: new Uint8Array(localImage),
        },
      ],
    });

    expect(result).toBeInstanceOf(Uint8Array);
  });

  test("with no options provided", () => {
    const result = renderer.render(node);

    expect(result).toBeInstanceOf(Uint8Array);
  });

  test("with default fonts disabled", () => {
    const result = rendererWithoutDefaultFonts.render(
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
  test("with timeMs applied to stylesheet animation", () => {
    const animated = renderer.measure(
      {
        type: "container",
        tagName: "div",
      },
      {
        width: 200,
        height: 100,
        timeMs: 500,
        stylesheets: [
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

  test("default format (png)", () => {
    const result = renderer.renderAsDataUrl(node, { width: 1200, height: 630 });

    expect(result).toMatch(/^data:image\/png;base64,/);
    expect(result.length).toBeGreaterThan(100);
  });

  test("webp format", () => {
    const result = renderer.renderAsDataUrl(node, {
      width: 1200,
      height: 630,
      format: "webp",
    });

    expect(result).toMatch(/^data:image\/webp;base64,/);
    expect(result.length).toBeGreaterThan(100);
  });

  test("jpeg format with quality", () => {
    const result = renderer.renderAsDataUrl(node, {
      width: 1200,
      height: 630,
      format: "jpeg",
      quality: 75,
    });

    expect(result).toMatch(/^data:image\/jpeg;base64,/);
    expect(result.length).toBeGreaterThan(100);
  });

  test("png format explicit", () => {
    const result = renderer.renderAsDataUrl(node, {
      width: 1200,
      height: 630,
      format: "png",
    });

    expect(result).toMatch(/^data:image\/png;base64,/);
    expect(result.length).toBeGreaterThan(100);
  });

  test("ico format", () => {
    const result = renderer.renderAsDataUrl(node, {
      width: 256,
      height: 256,
      format: "ico",
    });

    expect(result).toMatch(/^data:image\/x-icon;base64,/);
    expect(result.length).toBeGreaterThan(100);
  });

  test("renderAsDataUrl with debug borders", () => {
    const result = renderer.renderAsDataUrl(node, {
      width: 1200,
      height: 630,
      format: "png",
      drawDebugBorder: true,
    });

    expect(result).toMatch(/^data:image\/png;base64,/);
    expect(result.length).toBeGreaterThan(100);
  });

  test("renderAsDataUrl with device pixel ratio", () => {
    const result = renderer.renderAsDataUrl(node, {
      width: 1200,
      height: 630,
      format: "png",
      devicePixelRatio: 2.0,
    });

    expect(result).toMatch(/^data:image\/png;base64,/);
    expect(result.length).toBeGreaterThan(100);
  });

  test("renderAsDataUrl with fetched resources", () => {
    const result = renderer.renderAsDataUrl(node, {
      width: 1200,
      height: 630,
      format: "png",
      fetchedResources: [
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

    test("webp", () => {
      const result = renderer.renderAnimation({
        scenes: [scene],
        width: 1200,
        height: 630,
        format: "webp",
        fps: 1,
      });

      expect(result).toBeInstanceOf(Uint8Array);
    });

    test("apng", () => {
      const result = renderer.renderAnimation({
        scenes: [scene],
        width: 1200,
        height: 630,
        format: "apng",
        fps: 1,
      });

      expect(result).toBeInstanceOf(Uint8Array);
    });

    test("gif", () => {
      const result = renderer.renderAnimation({
        scenes: [scene],
        width: 1200,
        height: 630,
        format: "gif",
        fps: 1,
      });

      expect(result).toBeInstanceOf(Uint8Array);
      expect(Buffer.from(result.subarray(0, 6)).toString("ascii")).toMatch(/^GIF8[79]a$/);
    });
  });

  describe("encodeFrames", () => {
    test("with stylesheet keyframes", () => {
      const result = renderer.encodeFrames(
        [
          {
            node: {
              type: "container",
              tagName: "div",
            },
            durationMs: 1000,
          },
        ],
        {
          width: 200,
          height: 100,
          format: "gif",
          stylesheets: [
            `
              div {
                width: 100px;
                height: 100px;
                background: red;
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

      expect(result).toBeInstanceOf(Uint8Array);
      expect(Buffer.from(result.subarray(0, 6)).toString("ascii")).toMatch(/^GIF8[79]a$/);
    });
  });

  test("with structured keyframes in render options", () => {
    const animated = renderer.measure(
      {
        type: "container",
        tagName: "div",
      },
      {
        width: 200,
        height: 100,
        timeMs: 500,
        stylesheets: [
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
