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
          persistentImages: [
            {
              src: "test://ctor-arraybuffer",
              data: imageArrayBuffer,
            },
            {
              src: "test://ctor-uint8array",
              data: imageUint8Array,
            },
          ],
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

  test("putPersistentImage accepts Buffer, Uint8Array, and ArrayBuffer", async () => {
    const renderer = new Renderer();

    expect(
      renderer.putPersistentImage({ src: "test://img-buffer", data: imageBuffer }),
    ).resolves.toBeUndefined();
    expect(
      renderer.putPersistentImage({ src: "test://img-uint8array", data: imageUint8Array }),
    ).resolves.toBeUndefined();
    expect(
      renderer.putPersistentImage({ src: "test://img-arraybuffer", data: imageArrayBuffer }),
    ).resolves.toBeUndefined();
  });

  test("render fetchedResources accepts Buffer, Uint8Array, and ArrayBuffer", async () => {
    const renderer = new Renderer();

    const fromBuffer = await renderer.render(imageNode, {
      width: 64,
      height: 64,
      fetchedResources: [
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
      fetchedResources: [
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
      fetchedResources: [
        {
          src: "test://binary-input-image",
          data: imageArrayBuffer,
        },
      ],
    });
    expect(fromArrayBuffer).toBeInstanceOf(Buffer);
  });
});
