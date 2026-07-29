import { describe, expect, mock, spyOn, test } from "bun:test";
import { createContext, useContext } from "react";
import ImageResponse from "../src/response";
import { render } from "../src";
import type { Node } from "@takumi-rs/helpers";

describe("ImageResponse", () => {
  test("should accept Takumi Node input in render()", async () => {
    const node: Node = {
      type: "container",
      children: [{ type: "text", text: "Hello from node" }],
    };
    const mockedImage = new Uint8Array([1, 2, 3]);
    const renderer = {
      render: mock(async (inputNode) => {
        expect(inputNode).toEqual(node);
        return mockedImage;
      }),
    } as any;

    const output = await render(node, { renderer });

    expect(output).toEqual(mockedImage);
    expect(renderer.render).toHaveBeenCalledTimes(1);
  });

  test("rejects without invoking the renderer when the signal is already aborted", async () => {
    const node: Node = { type: "container", children: [{ type: "text", text: "x" }] };
    const renderer = { render: mock(async () => new Uint8Array()) } as any;
    const controller = new AbortController();
    controller.abort();

    expect(render(node, { renderer, signal: controller.signal })).rejects.toThrow();
    expect(renderer.render).not.toHaveBeenCalled();
  });

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

  test("should set ico content-type", async () => {
    const response = new ImageResponse(<div>Hello</div>, {
      width: 128,
      height: 128,
      format: "ico",
    });

    expect(response.headers.get("content-type")).toBe("image/x-icon");
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

  test("should render escaped fontFeatureSettings in react-dom/server fallback trees", async () => {
    const GreetingContext = createContext("Fallback");
    const Message = () => <span>{useContext(GreetingContext)}</span>;

    const response = new ImageResponse(
      <GreetingContext.Provider value="Hello">
        <div style={{ display: "flex", fontFeatureSettings: "'ss01' on" }}>
          <Message />
        </div>
      </GreetingContext.Provider>,
      {
        width: 400,
        height: 200,
        onError() {},
      },
    );

    expect(response.arrayBuffer()).resolves.toBeDefined();
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

  test("does not emit an unhandledRejection when ready is never awaited", async () => {
    const error = new Error("font load failed");

    const rejections: unknown[] = [];
    const handler = (reason: unknown) => {
      rejections.push(reason);
    };
    process.on("unhandledRejection", handler);

    try {
      const response = new ImageResponse(<div>Hello</div>, {
        fonts: [{ key: "failing-font", data: () => Promise.reject(error) }],
        onError() {},
      });
      await response.arrayBuffer().catch(() => {});
      await Bun.sleep(50);

      expect(rejections).toHaveLength(0);
      await expect(response.ready).rejects.toThrow();
    } finally {
      process.off("unhandledRejection", handler);
    }
  });

  test("should buffer the bytes and etag them", async () => {
    const image = new Uint8Array([1, 2, 3]);
    const renderer = { render: mock(async () => image) } as any;

    const response = await ImageResponse.buffered(<div>Hello</div>, { renderer });
    const body = new Uint8Array(await response.arrayBuffer());
    const digest = await crypto.subtle.digest("SHA-256", body);
    const hex = Array.from(new Uint8Array(digest), (byte) =>
      byte.toString(16).padStart(2, "0"),
    ).join("");

    expect(body).toEqual(image);
    expect(response.headers.get("etag")).toBe(`"${hex}"`);
    expect(response.headers.get("content-type")).toBe("image/png");
  });

  test("should keep an etag passed through headers", async () => {
    const renderer = { render: mock(async () => new Uint8Array([1, 2, 3])) } as any;

    const response = await ImageResponse.buffered(<div>Hello</div>, {
      renderer,
      headers: { etag: `"pinned"` },
    });

    expect(response.headers.get("etag")).toBe(`"pinned"`);
  });

  test("should reject with the render error even when onError rejects", async () => {
    const error = new Error("render failed");
    const renderer = {
      render: mock(async () => {
        throw error;
      }),
    } as any;
    const onError = mock(() => Promise.reject(new Error("logging failed")));
    const consoleError = spyOn(console, "error").mockImplementation(() => {});

    try {
      await expect(ImageResponse.buffered(<div>Hello</div>, { renderer, onError })).rejects.toBe(
        error,
      );
    } finally {
      consoleError.mockRestore();
    }

    expect(onError).toHaveBeenCalledTimes(1);
  });

  test("should not crash on social template with pre-wrap and emoji", async () => {
    const posts = [
      {
        user: "Sarah Jenkins",
        handle: "@sjenkins",
        time: "2h",
        content:
          "Just deployed our new rendering pipeline! The performance gains are absolutely incredible. Seeing a 40% reduction in TTFB across all endpoints.",
        likes: "1.2K",
        comments: "34",
        showIcons: true,
      },
      {
        user: "Mike Chen",
        handle: "@mike_codes",
        time: "4h",
        content:
          "Is anyone else obsessed with how clean the Satori APIs are? Moving our open graph image generation to Edge has been a game changer for our latency.",
        likes: "856",
        comments: "12",
        showIcons: false,
      },
      {
        user: "Design Daily",
        handle: "@designdaily",
        time: "7h",
        content:
          "Remember to check your contrast ratios! Accessibility isn't an afterthought, it's a fundamental part of good software design.",
        likes: "4.5K",
        comments: "128",
        showIcons: false,
      },
    ] as const;

    const response = new ImageResponse(
      <div
        style={{
          display: "flex",
          flexDirection: "column",
          width: "100%",
          height: "100%",
          backgroundColor: "#15202b",
          padding: "40px",
          fontFamily: "'Geist', sans-serif",
        }}
      >
        <div
          style={{
            display: "flex",
            flexDirection: "column",
            gap: "24px",
            flex: 1,
          }}
        >
          {posts.map((post, i) => (
            <div
              key={post.user}
              style={{
                display: "flex",
                flexDirection: "column",
                backgroundColor: "#192734",
                borderRadius: "16px",
                padding: "24px",
                border: "1px solid #38444d",
              }}
            >
              <div
                style={{
                  display: "flex",
                  alignItems: "center",
                  marginBottom: "16px",
                }}
              >
                <div
                  style={{
                    width: "56px",
                    height: "56px",
                    borderRadius: "28px",
                    backgroundColor: `hsl(${i * 60 + 200}, 70%, 50%)`,
                    marginRight: "16px",
                  }}
                />
                <div style={{ display: "flex", flexDirection: "column" }}>
                  <span
                    style={{
                      fontSize: "20px",
                      fontWeight: 700,
                      color: "#ffffff",
                    }}
                  >
                    {post.user}
                  </span>
                  <span style={{ fontSize: "18px", color: "#8899a6" }}>
                    {post.handle} · {post.time}
                  </span>
                </div>
              </div>

              <span
                style={{
                  fontSize: "22px",
                  color: "#ffffff",
                  lineHeight: "1.5",
                  marginBottom: "20px",
                  whiteSpace: "pre-wrap",
                }}
              >
                {post.content}
                {post.showIcons && " 🚀🔥"}
              </span>

              <div style={{ display: "flex", gap: 24, color: "#8899a6" }}>
                <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
                  💬<span style={{ fontSize: "18px" }}>{post.comments}</span>
                </div>
                <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
                  🔁
                  <span style={{ fontSize: "18px" }}>
                    {Math.floor(Number.parseInt(post.likes, 10) / 4)}
                  </span>
                </div>
                <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
                  ❤️
                  <span style={{ fontSize: "18px" }}>{post.likes}</span>
                </div>
              </div>
            </div>
          ))}
        </div>
      </div>,
      {
        width: 1200,
        height: 630,
        emoji: "twemoji",
      },
    );

    expect(response.arrayBuffer()).resolves.toBeDefined();
  });
});
