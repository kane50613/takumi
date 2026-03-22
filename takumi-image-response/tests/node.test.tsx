import { describe, expect, test } from "bun:test";
import ImageResponse from "../src/response";

describe("ImageResponse", () => {
  test("should not crash", async () => {
    const response = new ImageResponse(<div>Hello</div>);

    expect(response.status).toBe(200);
    expect(response.headers.get("content-type")).toBe("image/webp");

    expect(await response.arrayBuffer()).toBeDefined();
  });

  test("should set content-type", async () => {
    const response = new ImageResponse(<div>Hello</div>, {
      width: 100,
      height: 100,
      format: "png",
    });

    expect(response.headers.get("content-type")).toBe("image/png");
    expect(await response.arrayBuffer()).toBeDefined();
  });

  test("should resolve concurrent requests via Promise.all without hanging", async () => {
    const promises = Array.from({ length: 100 }).map(async (_, i) => {
      const response = new ImageResponse(<div>Concurrent {i}</div>);
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
