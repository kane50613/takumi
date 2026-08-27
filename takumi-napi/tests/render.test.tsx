import { describe, expect, test } from "bun:test";
import { readFile } from "node:fs/promises";
import { container, image, text } from "@takumi-rs/helpers";
import { fromJsx } from "@takumi-rs/helpers/jsx";
import { Glob } from "bun";
import { Renderer, type RenderOptions } from "../src/export";

const glob = new Glob("../assets/fonts/**/*.{woff2,ttf}");
const files = await Array.fromAsync(glob.scan());

const fontBuffers = await Promise.all(
  files.map(async (file) => await Bun.file(file).arrayBuffer()),
);

const renderer = new Renderer();

const remoteUrl = "https://yeecord.com/img/logo.png";
const localImagePath = "../assets/images/yeecord.png";

const imageBuffer = await Bun.file(localImagePath).arrayBuffer();

const dataUri = `data:image/png;base64,${Buffer.from(imageBuffer).toString("base64")}`;

const node = container({
  children: [
    image({
      src: remoteUrl,
      width: 96,
      height: 96,
      style: {
        borderRadius: "50%",
      },
    }),
    text("Remote"),
    image({
      src: localImagePath,
      width: 96,
      height: 96,
      style: {
        borderRadius: "25%",
      },
    }),
    text("Local"),
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
    backgroundColor: "white",
    width: "100%",
    height: "100%",
  },
});

test("Renderer initialization with fonts and images", async () => {
  const font = await readFile("../assets/fonts/geist/Geist[wght].woff2");

  const renderer = new Renderer();
  await renderer.registerFont(font);
});

test("no crash without fonts and images", () => {
  new Renderer();
});

describe("setup", () => {
  test("registerFont", async () => {
    const registered = await Promise.all(fontBuffers.map((font) => renderer.registerFont(font)));
    expect(registered).toHaveLength(files.length);
  });
});

describe("render", () => {
  const options: RenderOptions = {
    width: 1200,
    height: 630,
    images: [
      {
        src: remoteUrl,
        data: imageBuffer,
      },
    ],
  };

  test("webp 75% Quality", async () => {
    const result = await renderer.render(node, {
      ...options,
      format: "webp",
      quality: 75,
    });

    expect(result).toBeInstanceOf(Buffer);
  });

  test("webp 100% Quality", async () => {
    const result = await renderer.render(node, {
      ...options,
      format: "webp",
      quality: 100,
    });

    expect(result).toBeInstanceOf(Buffer);
  });

  test("png", async () => {
    const result = await renderer.render(node, {
      ...options,
      format: "png",
    });

    expect(result).toBeInstanceOf(Buffer);
  });

  test("jpeg 75% Quality", async () => {
    const result = await renderer.render(node, {
      ...options,
      format: "jpeg",
      quality: 75,
    });

    expect(result).toBeInstanceOf(Buffer);
  });

  test("jpeg 100% Quality", async () => {
    const result = await renderer.render(node, {
      ...options,
      format: "jpeg",
      quality: 100,
    });

    expect(result).toBeInstanceOf(Buffer);
  });

  test("ico", async () => {
    const result = await renderer.render(node, {
      ...options,
      width: 256,
      height: 256,
      format: "ico",
    });

    expect(result).toBeInstanceOf(Buffer);
    expect(result.subarray(0, 4)).toEqual(Buffer.from([0, 0, 1, 0]));
  });

  test("images group form with cache default", async () => {
    const result = await renderer.render(node, {
      width: 1200,
      height: 630,
      format: "png",
      images: {
        cache: "none",
        sources: [{ src: remoteUrl, data: imageBuffer }],
      },
    });

    expect(result).toBeInstanceOf(Buffer);
  });

  test("auto-calculated dimensions", async () => {
    const result = await renderer.render(node, {
      format: "png",
    });

    expect(result).toBeInstanceOf(Buffer);
  });

  test("with debug borders", async () => {
    const result = await renderer.render(node, {
      ...options,
      format: "png",
      drawDebugBorder: true,
    });

    expect(result).toBeInstanceOf(Buffer);
  });

  test("with device pixel ratio 2.0", async () => {
    const result = await renderer.render(node, {
      ...options,
      format: "png",
      devicePixelRatio: 2.0,
    });

    expect(result).toBeInstanceOf(Buffer);
  });

  test("with no options provided", async () => {
    const result = await renderer.render(node);

    expect(result).toBeInstanceOf(Buffer);
  });

  test("does not panic when inline text contains a nested flex span", async () => {
    const { node, stylesheets } = await fromJsx(
      <div
        style={{
          display: "flex",
          width: "100%",
          height: "100%",
          backgroundColor: "#15202b",
          padding: "40px",
        }}
      >
        <span
          style={{
            fontSize: "22px",
            color: "#ffffff",
            lineHeight: "1.5",
          }}
        >
          Just deployed our new rendering pipeline!
          <span
            style={{
              display: "flex",
              gap: "4px",
              marginLeft: "8px",
              color: "#fcd34d",
            }}
          >
            <span>Rocket</span>
            <span>Sparkles</span>
          </span>
        </span>
      </div>,
    );

    const result = await renderer.render(node, {
      width: 1200,
      height: 630,
      format: "png",
      css: stylesheets,
    });

    expect(result).toBeInstanceOf(Buffer);
  });

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

  test("measures through the deprecated stylesheets alias", async () => {
    const node = { type: "container", tagName: "div" } as const;
    const sheet = "div { width: 120px; height: 40px; }";

    const [viaCss, viaAlias] = await Promise.all([
      renderer.measure(node, { width: 200, height: 100, css: [sheet] }),
      renderer.measure(node, { width: 200, height: 100, stylesheets: [sheet] }),
    ]);

    expect(viaAlias).toEqual(viaCss);
    expect(viaAlias.width).toBe(120);
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

describe("renderAnimation", () => {
  const scene = {
    node,
    durationMs: 1000,
  };

  test("gif", async () => {
    const result = await renderer.renderAnimation({
      scenes: [scene],
      width: 1200,
      height: 630,
      fps: 1,
      format: "gif",
    });

    expect(result).toBeInstanceOf(Buffer);
    expect(result.subarray(0, 6).toString("ascii")).toMatch(/^GIF8[79]a$/);
  });

  test("clamps quality > 100", async () => {
    const result = await renderer.renderAnimation({
      scenes: [scene],
      width: 1200,
      height: 630,
      fps: 1,
      format: "webp",
      quality: 101,
    });

    expect(result).toBeInstanceOf(Buffer);
    expect(result.subarray(0, 4).toString("ascii")).toBe("RIFF");
    expect(result.subarray(8, 12).toString("ascii")).toBe("WEBP");
  });
});
