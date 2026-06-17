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

describe("binary inputs", () => {
  test("constructor accepts ArrayBuffer and Uint8Array", () => {
    expect(
      () =>
        new Renderer({
          fonts: [fontArrayBuffer],
        }),
    ).not.toThrow();
  });

  test("loadFontSync accepts Buffer, Uint8Array, and ArrayBuffer", () => {
    const renderer = new Renderer();

    expect(() => renderer.loadFontSync(fontBuffer)).not.toThrow();
    expect(() => renderer.loadFontSync(fontUint8Array)).not.toThrow();
    expect(() => renderer.loadFontSync(fontArrayBuffer)).not.toThrow();
  });

  test("loadFonts accepts Buffer, Uint8Array, and ArrayBuffer", async () => {
    const renderer = new Renderer();

    const count = await renderer.loadFonts([fontBuffer, fontUint8Array, fontArrayBuffer]);

    expect(count).toBe(3);
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
