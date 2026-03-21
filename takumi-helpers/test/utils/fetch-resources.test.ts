import { beforeEach, describe, expect, mock, test } from "bun:test";
import { fetchResources } from "../../src/utils";

describe("fetchResources", () => {
  beforeEach(() => {
    mock.restore();
  });

  test("fetches multiple resources successfully", async () => {
    const mockFetch = mock((url: string) =>
      Promise.resolve(new Response(new TextEncoder().encode(`Data for ${url}`).buffer)),
    );

    const urls = ["https://example.com/1", "https://example.com/2"];
    const result = await fetchResources(urls, { fetch: mockFetch });

    expect(mockFetch).toHaveBeenCalledTimes(2);
    expect(result.length).toBe(2);
  });

  test("handles empty URL array", async () => {
    const mockFetch = mock(() => Promise.resolve(new Response()));
    const result = await fetchResources([], { fetch: mockFetch });

    expect(result.length).toBe(0);
    expect(mockFetch).not.toHaveBeenCalled();
  });

  test("passes abort signal to fetch", async () => {
    const mockFetch = mock((_url: string, init?: RequestInit) => {
      expect(init?.signal).toBeDefined();
      return Promise.resolve(new Response(new ArrayBuffer(0)));
    });

    await fetchResources(["https://example.com/test"], {
      fetch: mockFetch,
      timeout: 10000,
    });

    expect(mockFetch).toHaveBeenCalled();
  });

  test("times out slow requests", () => {
    const mockFetch = mock(async (_url: string, init?: RequestInit) => {
      await new Promise((resolve) => setTimeout(resolve, 100));

      if (init?.signal?.aborted) {
        throw new DOMException("The operation was aborted.", "AbortError");
      }

      return new Response(new ArrayBuffer(0));
    });

    expect(
      fetchResources(["https://example.com/slow"], {
        fetch: mockFetch,
        timeout: 1,
      }),
    ).rejects.toThrow();
  });

  test("rejects when any single fetch fails", () => {
    const mockFetch = mock((url: string) => {
      if (url === "https://example.com/bad") {
        throw new Error("Network error");
      }
      return Promise.resolve(new Response(new ArrayBuffer(0)));
    });

    expect(
      fetchResources(["https://example.com/good", "https://example.com/bad"], {
        fetch: mockFetch,
      }),
    ).rejects.toThrow("Network error");
  });

  test("validates HTTP status codes and rejects 404", () => {
    const mockFetch = mock((url: string) =>
      Promise.resolve(
        new Response(new ArrayBuffer(0), {
          status: url.includes("404") ? 404 : 200,
          statusText: url.includes("404") ? "Not Found" : "OK",
        }),
      ),
    );

    expect(
      fetchResources(["https://example.com/exists", "https://example.com/404"], {
        fetch: mockFetch,
      }),
    ).rejects.toThrow("HTTP 404");
  });

  test("validates HTTP status codes and rejects 500", () => {
    const mockFetch = mock(() =>
      Promise.resolve(
        new Response(new ArrayBuffer(0), {
          status: 500,
          statusText: "Internal Server Error",
        }),
      ),
    );

    expect(fetchResources(["https://example.com/error"], { fetch: mockFetch })).rejects.toThrow(
      "HTTP 500",
    );
  });

  test("with throwOnError=false, skips failed fetches", async () => {
    const mockFetch = mock((url: string) => {
      if (url.includes("bad")) {
        return Promise.resolve(new Response(new ArrayBuffer(0), { status: 404 }));
      }
      return Promise.resolve(new Response(new ArrayBuffer(10)));
    });

    const result = await fetchResources(
      ["https://example.com/good1", "https://example.com/bad", "https://example.com/good2"],
      { fetch: mockFetch, throwOnError: false },
    );

    expect(result.length).toBe(2);
    expect(result[0]?.src).toBe("https://example.com/good1");
    expect(result[1]?.src).toBe("https://example.com/good2");
  });

  test("handles binary data correctly", async () => {
    const mockFetch = mock(() => {
      const buffer = new Uint8Array([0xff, 0x00, 0xaa, 0x55]);
      return Promise.resolve(new Response(buffer.buffer));
    });

    const result = await fetchResources(["https://example.com/binary"], {
      fetch: mockFetch,
    });

    const item = result[0];
    expect(item).toEqual({
      src: "https://example.com/binary",
      data: new Uint8Array([0xff, 0x00, 0xaa, 0x55]).buffer,
    });
  });

  test("all requests share the same AbortSignal", async () => {
    const signals: AbortSignal[] = [];

    const mockFetch = mock((_url: string, init?: RequestInit) => {
      if (init?.signal) {
        signals.push(init.signal);
      }
      return Promise.resolve(new Response(new ArrayBuffer(0)));
    });

    await fetchResources(["https://example.com/1", "https://example.com/2"], {
      fetch: mockFetch,
    });

    // All requests share the same signal instance
    expect(signals.length).toBe(2);
    expect(signals[0]).toBe(signals[1]);
  });

  test("deduplicates URLs before fetching", async () => {
    const mockFetch = mock((url: string) =>
      Promise.resolve(new Response(new TextEncoder().encode(url).buffer)),
    );

    const urls = [
      "https://example.com/resource",
      "https://example.com/resource",
      "https://example.com/other",
    ];
    const result = await fetchResources(urls, { fetch: mockFetch });

    // Only unique URLs are fetched
    expect(mockFetch).toHaveBeenCalledTimes(2);
    expect(result.length).toBe(2);
  });

  test("uses data from cache if available", async () => {
    const cachedData = new TextEncoder().encode("Cached Data").buffer;
    const cache = new Map<string, ArrayBuffer>([["https://example.com/cached", cachedData]]);
    const mockFetch = mock(() => Promise.resolve(new Response()));

    const result = await fetchResources(["https://example.com/cached"], {
      cache,
      fetch: mockFetch,
    });

    expect(mockFetch).not.toHaveBeenCalled();
    expect(result.length).toBe(1);
    expect(result[0]?.data).toBe(cachedData);
  });

  test("populates cache after fetching", async () => {
    const cache = new Map<string, ArrayBuffer>();
    const mockFetch = mock((url: string) =>
      Promise.resolve(new Response(new TextEncoder().encode(`Data for ${url}`).buffer)),
    );

    const url = "https://example.com/new";
    await fetchResources([url], { cache, fetch: mockFetch });

    expect(cache.has(url)).toBe(true);
    const cached = cache.get(url);
    expect(new TextDecoder().decode(cached)).toBe(`Data for ${url}`);
  });
});
