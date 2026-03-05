import { describe, expect, test } from "bun:test";
import { readFile } from "node:fs/promises";
import { container, image, text } from "@takumi-rs/helpers";
import { Glob } from "bun";
import { extractResourceUrls, Renderer, type RenderOptions } from "../index";

const glob = new Glob("../assets/fonts/**/*.{woff2,ttf}");
const files = await Array.fromAsync(glob.scan());

const fontBuffers = await Promise.all(
  files.map(async (file) => await Bun.file(file).arrayBuffer()),
);

const renderer = new Renderer({
  fonts: [
    {
      data: await Bun.file(
        "../assets/fonts/plus-jakarta-sans/PlusJakartaSans-VariableFont_wght.woff2",
      ).arrayBuffer(),
      name: "Plus Jakarta Sans",
      style: "normal",
    },
  ],
});

const remoteUrl = "https://yeecord.com/img/logo.png";
const localImagePath = "../assets/images/yeecord.png";

const imageBuffer = await Bun.file(localImagePath).arrayBuffer();

const dataUri = `data:image/png;base64,${Buffer.from(imageBuffer).toString(
  "base64",
)}`;

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

  new Renderer({
    fonts: [font],
    persistentImages: [
      {
        src: localImagePath,
        data: imageBuffer,
      },
    ],
  });
});

test("no crash without fonts and images", () => {
  new Renderer();
});

describe("setup", () => {
  test("loadFonts", async () => {
    const count = await renderer.loadFonts(fontBuffers);
    expect(count).toBe(files.length);
  });

  test("putPersistentImage", async () => {
    await renderer.putPersistentImage(localImagePath, imageBuffer);
  });
});

describe("extractResourceUrls", () => {
  test("extractResourceUrls", () => {
    const tasks = extractResourceUrls(node);
    expect(tasks).toEqual([remoteUrl]);
  });

  test("extracts nested backgroundImage URLs", () => {
    const nestedBackgroundUrl = "https://placehold.co/80x80/22c55e/white";
    const nestedNode = container({
      children: [
        container({
          style: {
            backgroundImage: `url(${nestedBackgroundUrl})`,
            width: 80,
            height: 80,
          },
        }),
      ],
      style: {
        width: 100,
        height: 100,
      },
    });

    const tasks = extractResourceUrls(nestedNode);
    expect(tasks).toEqual([nestedBackgroundUrl]);
  });
});

describe("render", () => {
  const options: RenderOptions = {
    width: 1200,
    height: 630,
    fetchedResources: [
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
});

describe("renderAnimation", () => {
  const frame = {
    node,
    durationMs: 1000,
  };

  test("gif", async () => {
    const result = await renderer.renderAnimation([frame], {
      width: 1200,
      height: 630,
      format: "gif",
    });

    expect(result).toBeInstanceOf(Buffer);
    expect(result.subarray(0, 6).toString("ascii")).toMatch(/^GIF8[79]a$/);
  });

  test("rejects quality > 100", () => {
    expect(
      renderer.renderAnimation([frame], {
        width: 1200,
        height: 630,
        format: "gif",
        quality: 101,
      }),
    ).rejects.toThrow();
  });
});

describe("clean up", () => {
  test("clearImageStore", () => renderer.clearImageStore());
});
