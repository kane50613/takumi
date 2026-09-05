import { expect, mock, test } from "bun:test";
import { fetchOk } from "../src/fetch";

test.each([
  [301, "POST", "GET"],
  [302, "POST", "GET"],
  [303, "POST", "GET"],
  [303, "PUT", "GET"],
  [303, "HEAD", "HEAD"],
  [303, "GET", "GET"],
  [301, "PUT", "PUT"],
  [302, "PUT", "PUT"],
  [307, "POST", "POST"],
  [308, "POST", "POST"],
])("%i redirects %s as %s", async (status, method, expectedMethod) => {
  const headers = new Headers({
    "content-type": "text/plain",
    "content-language": "en",
    "content-encoding": "identity",
    "content-location": "/body",
    "content-length": "7",
    "x-request-id": "kept",
  });
  const body = method === "HEAD" || method === "GET" ? undefined : "payload";
  const fetch = mock(async (_url: string, init?: RequestInit) => {
    if (fetch.mock.calls.length === 1) {
      return new Response(null, { status, headers: { location: "/next" } });
    }
    expect(init?.method).toBe(expectedMethod);
    expect(init?.body ?? undefined).toBe(method === expectedMethod ? body : undefined);
    for (const [name, value] of headers) {
      expect(new Headers(init?.headers).get(name)).toBe(
        method !== expectedMethod && name.startsWith("content-") ? null : value,
      );
    }
    return new Response("ok");
  });
  await fetchOk("https://example.com/start", {
    fetch,
    allowUrl: () => true,
    init: { method, body, headers },
  });
  expect(fetch).toHaveBeenCalledTimes(2);
  expect(headers.get("content-type")).toBe("text/plain");
});

test("keeps same-origin credentials and removes them permanently after an origin change", async () => {
  const headers = new Headers({
    authorization: "Bearer secret",
    cookie: "session=secret",
    "proxy-authorization": "Basic secret",
    "x-request-id": "kept",
  });
  const destinations = [
    "https://first.test/same",
    "https://second.test/cross",
    "https://first.test/return",
  ];
  const fetch = mock(async (_url: string, init?: RequestInit) => {
    for (const [name, value] of headers) {
      expect(new Headers(init?.headers).get(name)).toBe(
        fetch.mock.calls.length <= 2 || name === "x-request-id" ? value : null,
      );
    }
    const location = destinations[fetch.mock.calls.length - 1];
    return location
      ? new Response(null, { status: 307, headers: { location } })
      : new Response("ok");
  });
  await fetchOk("https://first.test/start", { fetch, allowUrl: () => true, init: { headers } });
  expect(fetch).toHaveBeenCalledTimes(4);
  expect(headers.get("authorization")).toBe("Bearer secret");
});

test.each([300, 304, 305, 306])("does not follow status %i", async (status) => {
  const fetch = mock(async () => new Response(null, { status, headers: { location: "/next" } }));
  await expect(
    fetchOk("https://example.com/start", { fetch, allowUrl: () => true }),
  ).rejects.toThrow(`HTTP ${status}`);
  expect(fetch).toHaveBeenCalledTimes(1);
});

test("rejects a streamed body that cannot be replayed", async () => {
  const fetch = mock(
    async () => new Response(null, { status: 307, headers: { location: "/next" } }),
  );
  await expect(
    fetchOk("https://example.com/start", {
      fetch,
      allowUrl: () => true,
      init: { method: "POST", body: new ReadableStream() },
    }),
  ).rejects.toThrow("Cannot replay a streamed request body");
  expect(fetch).toHaveBeenCalledTimes(1);
});
