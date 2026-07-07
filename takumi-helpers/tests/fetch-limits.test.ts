import { describe, expect, mock, test } from "bun:test";
import { fontFromUrl } from "../src/fonts";
import type { Node } from "../src/types";
import { prepareImages } from "../src/utils";

const tree = (...srcs: string[]): Node => ({
  type: "container",
  children: srcs.map((src) => ({ type: "image", src })),
});

const streamOf = (...chunks: Uint8Array[]): ReadableStream<Uint8Array> =>
  new ReadableStream({
    start(controller) {
      for (const chunk of chunks) {
        controller.enqueue(chunk);
      }
      controller.close();
    },
  });

describe("fetch byte caps", () => {
  test("rejects an oversized content-length before reading the body", async () => {
    const fetchMock = mock(() =>
      Promise.resolve(new Response(new Uint8Array(8), { headers: { "content-length": "1000" } })),
    );

    await expect(
      prepareImages({ node: tree("https://example.com/big.png"), fetch: fetchMock, maxBytes: 100 }),
    ).rejects.toThrow(/exceeds 100 bytes/);
  });

  test("rejects a streamed body that grows past maxBytes with no content-length", async () => {
    const fetchMock = mock(() =>
      Promise.resolve(new Response(streamOf(new Uint8Array(60), new Uint8Array(60)))),
    );

    await expect(
      prepareImages({
        node: tree("https://example.com/stream.png"),
        fetch: fetchMock,
        maxBytes: 100,
      }),
    ).rejects.toThrow(/exceeds 100 bytes/);
  });

  test("resolves a body under the cap with the correct bytes", async () => {
    const payload = new TextEncoder().encode("hello");
    const fetchMock = mock(() => Promise.resolve(new Response(streamOf(payload))));

    const images = await prepareImages({
      node: tree("https://example.com/ok.png"),
      fetch: fetchMock,
      maxBytes: 100,
    });

    expect(images.map((image) => new Uint8Array(image.data))).toEqual([payload]);
  });
});

describe("allowUrl policy", () => {
  test("never calls fetch for a blocked url", async () => {
    const fetchMock = mock(() => Promise.resolve(new Response(new Uint8Array())));

    await expect(
      prepareImages({
        node: tree("https://blocked.example.com/a.png"),
        fetch: fetchMock,
        allowUrl: () => false,
      }),
    ).rejects.toThrow(/blocked by allowUrl/);
    expect(fetchMock).not.toHaveBeenCalled();
  });
});

describe("default fetch timeout", () => {
  test("fontFromUrl rejects a hanging host within the timeout", async () => {
    const fetchMock = mock(
      (_url: string, init?: RequestInit) =>
        new Promise<Response>((_resolve, reject) => {
          init?.signal?.addEventListener("abort", () => reject(new Error("aborted")));
        }),
    );

    const loader = fontFromUrl("https://slow.example.com/font.woff2", {
      fetch: fetchMock,
      timeout: 50,
    });

    await expect(loader.data()).rejects.toThrow();
  });
});
