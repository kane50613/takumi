import { describe, expect, mock, test } from "bun:test";
import ImageResponse from "../src/response";

describe("ImageResponse", () => {
  test("should not crash", async () => {
    const response = new ImageResponse(<div>Hello</div>);

    expect(response.status).toBe(200);
    expect(response.headers.get("content-type")).toBe("image/png");

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
      onError: mock(),
    });
    const ready = response.ready.catch((caughtError) => caughtError);
    const bodyResult = response.arrayBuffer().catch((caughtError) => caughtError);

    expect(await ready).toBe(error);
    expect(await bodyResult).toBe(error);
  });

  test("should call onError when rendering fails", async () => {
    const error = new Error("primary render failed");
    const renderer = {
      render: mock(async () => {
        throw error;
      }),
    } as any;

    const onError = mock();

    const response = new ImageResponse(<div>Hello</div>, {
      renderer,
      onError,
    });

    const readyResult = response.ready.catch((caughtError) => caughtError);
    const bodyResult = response.arrayBuffer().catch((caughtError) => caughtError);

    expect(await readyResult).toBe(error);
    expect(await bodyResult).toBe(error);
    expect(onError).toHaveBeenCalledTimes(1);
    expect(onError.mock.calls[0]?.[0]).toBe(error);
  });
});
