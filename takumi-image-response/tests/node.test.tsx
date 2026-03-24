import { describe, expect, mock, test } from "bun:test";
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

  test("should expose rendering errors through ready promise", async () => {
    const error = new Error("render failed");
    const renderer = {
      render: mock(async () => {
        throw error;
      }),
    } as any;

    const response = new ImageResponse(<div>Hello</div>, {
      renderer,
    });
    const ready = response.ready.catch((caughtError) => caughtError);
    const bodyResult = response.arrayBuffer().catch((caughtError) => caughtError);

    expect(await ready).toBe(error);
    expect(await bodyResult).toBe(error);
  });

  test("should render fallback image when onError is provided", async () => {
    const renderer = {
      render: mock(async () => {
        if (renderer.render.mock.calls.length === 1) {
          throw new Error("primary render failed");
        }

        return new Uint8Array([1, 2, 3, 4]);
      }),
    } as any;
    const onError = mock(() => <div>Fallback</div>);

    const response = new ImageResponse(<div>Hello</div>, {
      renderer,
      onError,
    });

    await expect(response.ready).resolves.toBeUndefined();
    await expect(response.arrayBuffer()).resolves.toBeInstanceOf(ArrayBuffer);
    expect(onError).toHaveBeenCalledTimes(1);
    expect(renderer.render).toHaveBeenCalledTimes(2);
  });
});
