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

  const redirectTo = (location: string) =>
    new Response(null, { status: 302, headers: { location } });

  const routedFetch = (routes: Record<string, () => Response>) =>
    mock((url: string) => {
      const route = routes[url];
      if (!route) {
        throw new Error(`unexpected fetch: ${url}`);
      }
      return Promise.resolve(route());
    });

  test("rejects a redirect to a disallowed url without fetching it", async () => {
    const fetchMock = routedFetch({
      "https://allowed.example.com/a.png": () => redirectTo("http://169.254.169.254/meta"),
    });

    await expect(
      prepareImages({
        node: tree("https://allowed.example.com/a.png"),
        fetch: fetchMock,
        allowUrl: (url) => url.startsWith("https://allowed.example.com/"),
      }),
    ).rejects.toThrow(/blocked by allowUrl/);
    expect(fetchMock).toHaveBeenCalledTimes(1);
  });

  test("follows an allowed redirect chain and checks every hop", async () => {
    const payload = new TextEncoder().encode("pixels");
    const fetchMock = routedFetch({
      "https://allowed.example.com/a.png": () => redirectTo("/b.png"),
      "https://allowed.example.com/b.png": () => new Response(streamOf(payload)),
    });
    const checked: string[] = [];

    const images = await prepareImages({
      node: tree("https://allowed.example.com/a.png"),
      fetch: fetchMock,
      allowUrl: (url) => {
        checked.push(url);
        return url.startsWith("https://allowed.example.com/");
      },
    });

    expect(images.map((image) => new Uint8Array(image.data))).toEqual([payload]);
    expect(checked).toEqual([
      "https://allowed.example.com/a.png",
      "https://allowed.example.com/b.png",
    ]);
  });

  test("rejects a redirect chain longer than the hop cap", async () => {
    const fetchMock = mock((url: string) =>
      Promise.resolve(redirectTo(`${url.split("?")[0]}?hop=${Math.random()}`)),
    );

    await expect(
      prepareImages({
        node: tree("https://allowed.example.com/loop.png"),
        fetch: fetchMock,
        allowUrl: () => true,
      }),
    ).rejects.toThrow(/Too many redirects/);
  });

  test("without allowUrl a redirecting fetch keeps default handling", async () => {
    const payload = new TextEncoder().encode("pixels");
    const fetchMock = mock((_url: string, init?: RequestInit) => {
      expect(init?.redirect).toBeUndefined();
      return Promise.resolve(new Response(streamOf(payload)));
    });

    const images = await prepareImages({
      node: tree("https://anywhere.example.com/a.png"),
      fetch: fetchMock,
    });

    expect(images.map((image) => new Uint8Array(image.data))).toEqual([payload]);
    expect(fetchMock).toHaveBeenCalledTimes(1);
  });
});

describe("shared fetchCache policy enforcement", () => {
  test("cached images recheck the entry URL, not the original redirect chain", async () => {
    const entry = "https://allowed.example.com/image.png";
    const destination = "https://other.example.com/image.png";
    const payload = new Uint8Array([1, 2, 3]);
    const fetchMock = mock((url: string) =>
      Promise.resolve(
        url === entry
          ? new Response(null, { status: 302, headers: { location: destination } })
          : new Response(payload),
      ),
    );
    const node = tree(entry);
    const fetchCache = new Map<string, Promise<ArrayBuffer>>();
    await prepareImages({ node, fetchCache, fetch: fetchMock, allowUrl: () => true });
    const checked: string[] = [];
    const allowUrl = (url: string) => {
      checked.push(url);
      return url === entry;
    };

    const images = await prepareImages({ node, fetchCache, fetch: fetchMock, allowUrl });
    expect(images.map((image) => new Uint8Array(image.data))).toEqual([payload]);
    expect(checked).toEqual([entry]);
    expect(fetchMock).toHaveBeenCalledTimes(2);

    await expect(
      prepareImages({ node, fetchCache: new Map(), fetch: fetchMock, allowUrl }),
    ).rejects.toThrow("URL blocked by allowUrl policy");
    expect(fetchMock).toHaveBeenCalledTimes(3);
  });

  test("a cache hit re-runs a stricter maxBytes and keeps the entry reusable", async () => {
    const fetchMock = mock(() => Promise.resolve(new Response(streamOf(new Uint8Array(60)))));
    const fetchCache = new Map<string, Promise<ArrayBuffer>>();
    const node = tree("https://example.com/cached.png");

    await prepareImages({ node, fetchCache, fetch: fetchMock, maxBytes: 100 });

    await expect(
      prepareImages({ node, fetchCache, fetch: fetchMock, maxBytes: 50 }),
    ).rejects.toThrow(/exceeds 50 bytes/);

    const images = await prepareImages({ node, fetchCache, fetch: fetchMock, maxBytes: 100 });

    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(images.map((image) => image.data.byteLength)).toEqual([60]);
  });

  test("a cache hit re-runs a stricter allowUrl and keeps the entry reusable", async () => {
    const payload = new TextEncoder().encode("pixels");
    const fetchMock = mock(() => Promise.resolve(new Response(streamOf(payload))));
    const fetchCache = new Map<string, Promise<ArrayBuffer>>();
    const node = tree("https://allowed.example.com/cached.png");

    await prepareImages({ node, fetchCache, fetch: fetchMock, allowUrl: () => true });

    await expect(
      prepareImages({ node, fetchCache, fetch: fetchMock, allowUrl: () => false }),
    ).rejects.toThrow(/blocked by allowUrl/);

    const images = await prepareImages({
      node,
      fetchCache,
      fetch: fetchMock,
      allowUrl: () => true,
    });

    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(images.map((image) => new Uint8Array(image.data))).toEqual([payload]);
  });

  test("an entry fetched without allowUrl is still checked against its entry url", async () => {
    const fetchMock = mock(() => Promise.resolve(new Response(new Uint8Array(8))));
    const fetchCache = new Map<string, Promise<ArrayBuffer>>();
    const node = tree("http://169.254.169.254/meta.png");

    await prepareImages({ node, fetchCache, fetch: fetchMock });

    await expect(
      prepareImages({
        node,
        fetchCache,
        fetch: fetchMock,
        allowUrl: (url) => new URL(url).hostname === "allowed.example.com",
      }),
    ).rejects.toThrow(/blocked by allowUrl/);
    expect(fetchMock).toHaveBeenCalledTimes(1);
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
