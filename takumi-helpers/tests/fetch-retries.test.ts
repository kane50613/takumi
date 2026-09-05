import { expect, mock, test } from "bun:test";
import { fontFromUrl, googleFonts } from "../src/fonts";
import { fetchOk } from "../src/fetch";

const fontCss =
  "@font-face{font-family:'Inter';font-weight:400;src:url(https://fonts.gstatic.com/inter.woff2);unicode-range:U+0000-00FF}";

test("calls fetch without binding the request owner, including retries", async () => {
  let attempts = 0;
  const fetch = function (this: unknown) {
    expect(this).toBeUndefined();
    return Promise.resolve(new Response(null, { status: ++attempts === 1 ? 503 : 200 }));
  };
  expect((await fetchOk("https://example.com/image.png", { fetch })).status).toBe(200);
  expect(attempts).toBe(2);
});

test("recovers from a transient Google Fonts timeout", async () => {
  const fetch = mock()
    .mockRejectedValueOnce(new DOMException("The operation timed out", "TimeoutError"))
    .mockResolvedValue(new Response(fontCss));
  expect(await googleFonts({ families: ["Inter"], fetch })).toHaveLength(1);
  expect(fetch).toHaveBeenCalledTimes(2);
});

test("retries a font server error before sharing the CSS result", async () => {
  const fetch = mock()
    .mockResolvedValueOnce(new Response(null, { status: 503 }))
    .mockResolvedValue(new Response(fontCss));
  const [first, second] = await Promise.all([
    googleFonts({ families: ["Inter"], fetch }),
    googleFonts({ families: ["Inter"], fetch }),
  ]);
  expect(first).toHaveLength(1);
  expect(second.map((font) => font.key)).toEqual(first.map((font) => font.key));
  expect(fetch).toHaveBeenCalledTimes(2);
});

test("retries a font file connection failure", async () => {
  const fetch = mock()
    .mockRejectedValueOnce(new TypeError("fetch failed"))
    .mockResolvedValue(new Response(new Uint8Array([1, 2, 3])));
  expect(
    new Uint8Array(await fontFromUrl("https://example.com/font.woff2", { fetch }).data()),
  ).toEqual(new Uint8Array([1, 2, 3]));
  expect(fetch).toHaveBeenCalledTimes(2);
});

test("stops after three attempts", async () => {
  const fetch = mock(() => Promise.reject(new TypeError("fetch failed")));
  await expect(fetchOk("https://example.com/font.woff2", { fetch })).rejects.toThrow(
    "fetch failed",
  );
  expect(fetch).toHaveBeenCalledTimes(3);
});

test("does not retry permanent errors or non-idempotent requests", async () => {
  for (const status of [400, 401, 403, 404]) {
    const fetch = mock(() => Promise.resolve(new Response(null, { status })));
    await expect(fetchOk("https://example.com/font.woff2", { fetch })).rejects.toThrow(
      `HTTP ${status}`,
    );
    expect(fetch).toHaveBeenCalledTimes(1);
  }
  const fetch = mock(() => Promise.resolve(new Response(null, { status: 503 })));
  await expect(
    fetchOk("https://example.com/submit", { fetch, init: { method: "POST" } }),
  ).rejects.toThrow("HTTP 503");
  expect(fetch).toHaveBeenCalledTimes(1);
});

test("the timeout bounds retry backoff as well as fetch", async () => {
  const fetch = mock(() => Promise.resolve(new Response(null, { status: 503 })));
  await expect(fetchOk("https://example.com/font.woff2", { fetch, timeout: 10 })).rejects.toThrow();
  expect(fetch).toHaveBeenCalledTimes(1);
});

test("caller cancellation stops retry backoff", async () => {
  const controller = new AbortController();
  const fetch = mock(() => Promise.resolve(new Response(null, { status: 503 })));
  const pending = fetchOk("https://example.com/font.woff2", { fetch, signal: controller.signal });
  await Promise.resolve();
  controller.abort(new Error("Stopped by caller"));
  await expect(pending).rejects.toThrow("Stopped by caller");
  expect(fetch).toHaveBeenCalledTimes(1);
});

test("does not shorten a server's long Retry-After instruction", async () => {
  const fetch = mock(() =>
    Promise.resolve(new Response(null, { status: 429, headers: { "retry-after": "120" } })),
  );
  await expect(fetchOk("https://example.com/font.woff2", { fetch })).rejects.toThrow("HTTP 429");
  expect(fetch).toHaveBeenCalledTimes(1);
});

test("recovers from rate limiting with a short Retry-After", async () => {
  const fetch = mock()
    .mockResolvedValueOnce(new Response(null, { status: 429, headers: { "retry-after": "0" } }))
    .mockResolvedValue(new Response(fontCss));
  expect(await googleFonts({ families: ["Inter"], fetch })).toHaveLength(1);
  expect(fetch).toHaveBeenCalledTimes(2);
});

test("retries do not bypass redirect policy", async () => {
  const fetch = mock()
    .mockResolvedValueOnce(new Response(null, { status: 503 }))
    .mockResolvedValue(
      new Response(null, { status: 302, headers: { location: "https://blocked.test/font" } }),
    );
  await expect(
    fetchOk("https://example.com/font", {
      fetch,
      allowUrl: (url) => new URL(url).hostname === "example.com",
    }),
  ).rejects.toThrow("URL blocked by allowUrl policy");
  expect(fetch).toHaveBeenCalledTimes(2);
  expect(fetch.mock.calls.every(([url]) => url === "https://example.com/font")).toBeTrue();
});
