import { describe, expect, test } from "bun:test";
import { writeFile } from "node:fs/promises";
import { container, image, percentage, rem, text } from "@takumi-rs/helpers";
import { Glob } from "bun";
import { Renderer } from "../index";

const renderer = new Renderer();

const remoteUrl = "https://yeecord.com/img/logo.png";
const localImagePath = "../assets/images/yeecord.png";

const remoteImage = await fetch(remoteUrl).then((r) => r.arrayBuffer());

const localImage = await Bun.file(localImagePath).arrayBuffer();
const dataUri = `data:image/png;base64,${Buffer.from(localImage).toString(
  "base64",
)}`;

const node = container({
  children: [
    image({
      src: remoteUrl,
      width: 96,
      height: 96,
      style: {
        borderRadius: percentage(50),
      },
    }),
    text("Remote"),
    image({
      src: localImagePath,
      width: 96,
      height: 96,
      style: {
        borderRadius: percentage(25),
      },
    }),
    text("Local"),
    image({
      src: dataUri,
      width: 96,
      height: 96,
      style: {
        borderRadius: percentage(25),
      },
    }),
    text("Data URI"),
  ],
  style: {
    justifyContent: "center",
    alignItems: "center",
    gap: rem(1.5),
    backgroundColor: 0xffffff,
    width: percentage(100),
    height: percentage(100),
  },
});

test("Renderer initialization with fonts and images", async () => {
  const font = await Bun.file(
    "../assets/fonts/noto-sans/NotoSansTC-Bold.woff",
  ).arrayBuffer();

  new Renderer({
    fonts: [font],
    persistentImages: [
      {
        src: remoteUrl,
        data: remoteImage,
      },
      {
        src: localImagePath,
        data: localImage,
      },
    ],
    debug: true,
  });
});

describe("setup", () => {
  test("loadFontsAsync", async () => {
    const glob = new Glob("../assets/fonts/**/*.{woff2,ttf}");
    const files = await Array.fromAsync(glob.scan());

    const buffers = await Promise.all(
      files.map((file) => Bun.file(file).arrayBuffer()),
    );

    const count = await renderer.loadFontsAsync(buffers);
    expect(count).toBe(files.length);
  });

  test("putPersistentImageAsync / local", async () => {
    await renderer.putPersistentImageAsync(localImagePath, localImage);
  });

  test("putPersistentImageAsync / remote", async () => {
    await renderer.putPersistentImageAsync(remoteUrl, remoteImage);
  });
});

describe("renderAsync", () => {
  const options = {
    width: 1200,
    height: 630,
  };

  test("webp", async () => {
    const result = await renderer.renderAsync(node, {
      ...options,
      format: "WebP",
    });

    await writeFile("./test.webp", result);

    expect(result).toBeInstanceOf(Buffer);
  });

  test("png", async () => {
    const result = await renderer.renderAsync(node, {
      ...options,
      format: "Png",
    });

    await writeFile("./test.png", result);

    expect(result).toBeInstanceOf(Buffer);
  });

  test("jpeg 75%", async () => {
    const result = await renderer.renderAsync(node, {
      ...options,
      format: "Jpeg",
      quality: 75,
    });

    await writeFile("./test-75.jpg", result);

    expect(result).toBeInstanceOf(Buffer);
  });

  test("jpeg 100%", async () => {
    const result = await renderer.renderAsync(node, {
      ...options,
      format: "Jpeg",
      quality: 100,
    });

    await writeFile("./test-100.jpg", result);

    expect(result).toBeInstanceOf(Buffer);
  });
});

describe("clean up", () => {
  test("clearImageStore", () => renderer.clearImageStore());
});

test("transform property (rotate)", async () => {
  const transformedNode = container({
    style: {
      width: 400,
      height: 400,
      backgroundColor: 0xff0000,
      justifyContent: "center",
      alignItems: "center",
      transform: [{ rotate: 45 }],
      // represent percentage units using helper
      transformOrigin: [percentage(50), percentage(50)],
    },
    children: [
      text("Rotated!", {
        fontSize: 32,
        color: 0xffffff,
      }),
    ],
  });

  const result = await renderer.renderAsync(transformedNode, {
    width: 600,
    height: 600,
    format: "Png",
  });

  await writeFile("./test-transform.png", result);

  expect(result).toBeInstanceOf(Buffer);
});

test("transform property (scale uniform)", async () => {
  const nodeScaled = container({
    style: {
      width: 200,
      height: 200,
      backgroundColor: 0x00ff00,
      justifyContent: "center",
      alignItems: "center",
      transform: [{ scale: [2, 2] }],
      transformOrigin: [percentage(50), percentage(50)],
    },
    children: [text("Scale 2x", { fontSize: 20, color: 0x000000 })],
  });
  const result = await renderer.renderAsync(nodeScaled, {
    width: 500,
    height: 500,
    format: "Png",
  });

  await writeFile("./test-transform-scale-uniform.png", result);

  expect(result).toBeInstanceOf(Buffer);
});

test("transform property (scale non-uniform)", async () => {
  const nodeScaled = container({
    style: {
      width: 200,
      height: 200,
      backgroundColor: 0x0000ff,
      justifyContent: "center",
      alignItems: "center",
      transform: [{ scale: [2, 0.5] }],
      transformOrigin: [percentage(50), percentage(50)],
    },
    children: [text("Scale 2x/0.5x", { fontSize: 20, color: 0xffffff })],
  });
  const result = await renderer.renderAsync(nodeScaled, {
    width: 500,
    height: 500,
    format: "Png",
  });

  await writeFile("./test-transform-scale-non-uniform.png", result);

  expect(result).toBeInstanceOf(Buffer);
});

test("transform property (translate)", async () => {
  const nodeTranslated = container({
    style: {
      width: 300,
      height: 300,
      backgroundColor: 0xffff00,
      justifyContent: "center",
      alignItems: "center",
      transform: [{ translate: [50, 30] }],
      transformOrigin: [percentage(0), percentage(0)],
    },
    children: [text("Translate", { fontSize: 24, color: 0x000000 })],
  });
  const result = await renderer.renderAsync(nodeTranslated, {
    width: 500,
    height: 500,
    format: "Png",
  });

  await writeFile("./test-transform-translate.png", result);

  expect(result).toBeInstanceOf(Buffer);
});

test("transform property (skew)", async () => {
  const nodeSkew = container({
    style: {
      width: 300,
      height: 200,
      backgroundColor: 0xff00ff,
      justifyContent: "center",
      alignItems: "center",
      transform: [{ skew: [15, 5] }],
      transformOrigin: [percentage(50), percentage(50)],
    },
    children: [text("Skew", { fontSize: 32, color: 0xffffff })],
  });
  const result = await renderer.renderAsync(nodeSkew, {
    width: 500,
    height: 500,
    format: "Png",
  });

  await writeFile("./test-transform-skew.png", result);

  expect(result).toBeInstanceOf(Buffer);
});
