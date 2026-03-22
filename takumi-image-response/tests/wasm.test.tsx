import { describe, expect, test } from "bun:test";
import { join } from "node:path";
import { file } from "bun";
import { ImageResponse } from "../src/response";

const module = new URL(import.meta.resolve("@takumi-rs/wasm/takumi_wasm_bg.wasm"), import.meta.url);

const geist = await file(
  join(import.meta.dirname, "../../assets/fonts/geist/Geist[wght].woff2"),
).arrayBuffer();

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
});
