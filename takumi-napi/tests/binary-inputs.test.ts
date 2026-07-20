import { describe, expect, test } from "bun:test";
import { container, image } from "@takumi-rs/helpers";
import { Renderer } from "../src/export";

const fontArrayBuffer = await Bun.file("../assets/fonts/geist/Geist[wght].woff2").arrayBuffer();
const imageArrayBuffer = await Bun.file("../assets/images/yeecord.png").arrayBuffer();

const fontBuffer = Buffer.from(fontArrayBuffer);
const fontUint8Array = new Uint8Array(fontArrayBuffer.slice(0));

const imageBuffer = Buffer.from(imageArrayBuffer);
const imageUint8Array = new Uint8Array(imageArrayBuffer.slice(0));

const imageNode = container({
  style: {
    width: 64,
    height: 64,
  },
  children: [
    image({
      src: "test://binary-input-image",
      width: 64,
      height: 64,
    }),
  ],
});

const recoveryNode = container({ style: { width: 8, height: 8, backgroundColor: "black" } });

const withImageBytes = (data: Uint8Array) =>
  container({
    style: { width: 64, height: 64 },
    children: [image({ src: data, width: 64, height: 64 })],
  });

// Undecodable image sources are skipped at paint like a browser's broken
// image, so renders complete instead of rejecting; these cases pin that no
// input aborts the process and the renderer stays usable afterwards.
describe("malformed binary inputs", () => {
  const renderer = new Renderer();

  const expectRecovery = async () => {
    const recovered = await renderer.render(recoveryNode, { width: 8, height: 8 });
    expect(recovered).toBeInstanceOf(Buffer);
  };

  test("corrupt PNG bytes render without crashing", async () => {
    const corrupt = new Uint8Array(72);
    corrupt.set([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
    for (let i = 8; i < corrupt.length; i++) {
      corrupt[i] = (i * 37) % 256;
    }

    const result = await renderer.render(withImageBytes(corrupt), { width: 64, height: 64 });
    expect(result).toBeInstanceOf(Buffer);
    await expectRecovery();
  });

  test("truncated font rejects", async () => {
    await expect(renderer.registerFont(fontArrayBuffer.slice(0, 100))).rejects.toThrow();
    await expectRecovery();
  });

  test("malformed inline SVG bytes render without crashing", async () => {
    const bytes = new TextEncoder().encode("<svg garbage");

    const result = await renderer.render(withImageBytes(bytes), { width: 64, height: 64 });
    expect(result).toBeInstanceOf(Buffer);
    await expectRecovery();
  });

  test("truncated GIF bytes render without crashing", async () => {
    const truncated = new Uint8Array(16);
    truncated.set(new TextEncoder().encode("GIF89a"));

    const result = await renderer.render(withImageBytes(truncated), { width: 64, height: 64 });
    expect(result).toBeInstanceOf(Buffer);
    await expectRecovery();
  });
});

describe("binary inputs", () => {
  test("registerFont accepts Buffer, Uint8Array, and ArrayBuffer", async () => {
    const renderer = new Renderer();

    const registered = await Promise.all(
      [fontBuffer, fontUint8Array, fontArrayBuffer].map((font) => renderer.registerFont(font)),
    );

    expect(registered).toHaveLength(3);
    expect(registered.every((families) => families.some((family) => Boolean(family.name)))).toBe(
      true,
    );
  });

  test("render accepts inline image bytes as src", async () => {
    const renderer = new Renderer();

    const reference = await renderer.render(imageNode, {
      width: 64,
      height: 64,
      images: [{ src: "test://binary-input-image", data: imageUint8Array }],
    });

    const inline = (src: Uint8Array | ArrayBuffer) =>
      container({
        style: { width: 64, height: 64 },
        children: [image({ src, width: 64, height: 64 })],
      });

    const fromUint8Array = await renderer.render(inline(imageUint8Array), {
      width: 64,
      height: 64,
    });
    expect(fromUint8Array).toBeInstanceOf(Buffer);
    expect(Buffer.compare(fromUint8Array, reference)).toBe(0);

    const fromArrayBuffer = await renderer.render(inline(imageArrayBuffer), {
      width: 64,
      height: 64,
    });
    expect(Buffer.compare(fromArrayBuffer, reference)).toBe(0);
  });

  test("render accepts raw RGBA pixels as src", async () => {
    const renderer = new Renderer();
    const solid = (r: number, a: number) => {
      const data = new Uint8Array(8 * 8 * 4);
      for (let i = 0; i < data.length; i += 4) {
        data[i] = r;
        data[i + 3] = a;
      }
      return data;
    };
    const over = (src: Parameters<typeof image>[0]["src"]) =>
      container({
        style: { width: 8, height: 8, backgroundColor: "black" },
        children: [image({ src })],
      });
    const centerOf = async (src: Parameters<typeof image>[0]["src"]) => {
      const raw = await renderer.render(over(src), { width: 8, height: 8, format: "raw" });
      return [...raw.subarray(0, 4)];
    };

    // Straight 50% red premultiplies to ~128; premultiplied bytes pass through.
    const straight = await centerOf({ width: 8, height: 8, data: solid(255, 128) });
    expect(Math.abs((straight[0] ?? 0) - 128)).toBeLessThanOrEqual(2);
    expect(straight[3]).toBe(255);

    const premultiplied = await centerOf({
      width: 8,
      height: 8,
      data: solid(128, 128),
      premultiplied: true,
    });
    expect(Math.abs((premultiplied[0] ?? 0) - 128)).toBeLessThanOrEqual(2);

    // An explicit undefined must deserialize like an absent field.
    const explicitUndefined = await centerOf({
      width: 8,
      height: 8,
      data: solid(255, 128),
      premultiplied: undefined,
    });
    expect(explicitUndefined).toEqual(straight);

    await expect(
      renderer.render(over({ width: 8, height: 8, data: new Uint8Array(4) }), {
        width: 8,
        height: 8,
      }),
    ).rejects.toThrow();
  });

  test("render accepts inline SVG markup as bytes", async () => {
    const renderer = new Renderer();
    const svg =
      '<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64"><rect width="64" height="64" fill="red"/></svg>';

    const inline = (src: string | Uint8Array) =>
      container({
        style: { width: 64, height: 64 },
        children: [image({ src, width: 64, height: 64 })],
      });

    const fromString = await renderer.render(inline(svg), { width: 64, height: 64 });
    const fromBytes = await renderer.render(inline(new TextEncoder().encode(svg)), {
      width: 64,
      height: 64,
    });

    expect(fromBytes).toBeInstanceOf(Buffer);
    expect(Buffer.compare(fromBytes, fromString)).toBe(0);
  });

  test("render images accepts Buffer, Uint8Array, and ArrayBuffer", async () => {
    const renderer = new Renderer();

    const fromBuffer = await renderer.render(imageNode, {
      width: 64,
      height: 64,
      images: [
        {
          src: "test://binary-input-image",
          data: imageBuffer,
        },
      ],
    });
    expect(fromBuffer).toBeInstanceOf(Buffer);

    const fromUint8Array = await renderer.render(imageNode, {
      width: 64,
      height: 64,
      images: [
        {
          src: "test://binary-input-image",
          data: imageUint8Array,
        },
      ],
    });
    expect(fromUint8Array).toBeInstanceOf(Buffer);

    const fromArrayBuffer = await renderer.render(imageNode, {
      width: 64,
      height: 64,
      images: [
        {
          src: "test://binary-input-image",
          data: imageArrayBuffer,
        },
      ],
    });
    expect(fromArrayBuffer).toBeInstanceOf(Buffer);
  });
});
