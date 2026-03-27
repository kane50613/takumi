import { describe, expect, mock, test } from "bun:test";
import { join } from "node:path";
import { file } from "bun";
import { createImageResponse, ImageResponse } from "../../takumi-js/src/response";

const module = new URL(import.meta.resolve("@takumi-rs/wasm/takumi_wasm_bg.wasm"), import.meta.url);

const geist = await file(
  join(import.meta.dirname, "../../assets/fonts/geist/Geist[wght].woff2"),
).arrayBuffer();
const icon = await file(join(import.meta.dirname, "../../assets/images/yeecord.png")).arrayBuffer();

describe("ImageResponse", () => {
  test("should not crash", async () => {
    const response = new ImageResponse(<div tw="bg-black w-4 h-4" />, {
      module,
    });

    expect(response.status).toBe(200);
    expect(response.headers.get("content-type")).toBe("image/webp");

    expect(await response.arrayBuffer()).toBeDefined();
  });

  test("should set content-type", async () => {
    const response = new ImageResponse(<div tw="bg-black w-4 h-4 text-white">Hello</div>, {
      width: 100,
      height: 100,
      format: "png",
      module,
      fonts: [
        {
          data: geist,
          name: "Geist",
        },
      ],
    });

    expect(response.headers.get("content-type")).toBe("image/png");
    expect(await response.arrayBuffer()).toBeDefined();
  });

  test("should resolve concurrent requests via Promise.all without hanging", async () => {
    const promises = Array.from({ length: 100 }).map(async (_, i) => {
      const response = new ImageResponse(
        <div tw="bg-black w-4 h-4 text-white">Concurrent {i}</div>,
        {
          module,
          fonts: [
            {
              data: geist,
              name: "Geist",
            },
          ],
        },
      );
      const buffer = await response.arrayBuffer();
      return buffer;
    });

    const buffers = await Promise.all(promises);
    expect(buffers).toHaveLength(100);
    for (const buffer of buffers) {
      expect(buffer).toBeDefined();
    }
  });

  test("should cache lazy font and image loaders across requests", async () => {
    const loadFont = mock(async () => geist);
    const loadImage = mock(async () => icon);

    const promises = Array.from({ length: 8 }).map(async (_, i) => {
      const response = new ImageResponse(
        <div tw="flex items-center gap-2 bg-black text-white">
          <img src="icon.png" alt="" width={8} height={8} />
          <div>Concurrent {i}</div>
        </div>,
        {
          module,
          fonts: [
            {
              data: loadFont,
              name: "Geist",
            },
          ],
          persistentImages: [
            {
              data: loadImage,
              src: "icon.png",
            },
          ],
        },
      );

      return response.arrayBuffer();
    });

    const buffers = await Promise.all(promises);

    expect(buffers).toHaveLength(8);
    expect(loadFont).toHaveBeenCalledTimes(1);
    expect(loadImage).toHaveBeenCalledTimes(1);
  });

  test("should isolate caches per createImageResponse instance", async () => {
    const loadFont = mock(async () => geist);
    const createResponseA = createImageResponse({
      fonts: [
        {
          data: loadFont,
          key: "geist",
          name: "Geist",
        },
      ],
      module,
    });
    const createResponseB = createImageResponse({
      fonts: [
        {
          data: loadFont,
          key: "geist",
          name: "Geist",
        },
      ],
      module,
    });

    await Promise.all([
      createResponseA(<div tw="text-white">A</div>).arrayBuffer(),
      createResponseA(<div tw="text-white">A2</div>).arrayBuffer(),
      createResponseB(<div tw="text-white">B</div>).arrayBuffer(),
    ]);

    expect(loadFont).toHaveBeenCalledTimes(2);
  });
});
