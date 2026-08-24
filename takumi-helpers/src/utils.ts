import type { CSSProperties } from "react";
import type { Node } from "./types";

const defaultFetchTimeout = 30_000;
const maxRedirectHops = 5;
export const defaultMaxFetchBytes = 32 * 1024 * 1024;
const cssUrlPattern = /url\(\s*(['"]?)(.*?)\1\s*\)/g;

export type FetchLike = (input: string, init?: RequestInit) => Promise<Response>;

export type FetchOptions = {
  /** Custom fetch implementation. @default globalThis.fetch */
  fetch?: FetchLike;
  /** Abort the request after this many milliseconds; `0` or negative disables it. @default 30000 */
  timeout?: number;
  /** Caller abort signal, combined with the timeout. */
  signal?: AbortSignal;
  /** Reject bodies larger than this many bytes. @default 33554432 (32 MiB) */
  maxBytes?: number;
  /** Return false to skip fetching a URL (e.g. SSRF allowlist). */
  allowUrl?: (url: string) => boolean;
};

/** Fetches a URL, applying a timeout signal and throwing on a non-OK status. */
export async function fetchOk(url: string, options: FetchOptions & { init?: RequestInit } = {}) {
  if (options.allowUrl && !options.allowUrl(url)) {
    throw new Error(`URL blocked by allowUrl policy: ${url}`);
  }

  const fetchImpl = options.fetch ?? globalThis.fetch;
  const timeout = options.timeout ?? defaultFetchTimeout;
  const timeoutSignal = timeout <= 0 ? undefined : AbortSignal.timeout(timeout);
  const signals = [options.signal, options.init?.signal, timeoutSignal].filter(
    (s): s is AbortSignal => s !== undefined,
  );
  const signal = signals.length ? AbortSignal.any(signals) : undefined;
  const init = { ...options.init, signal };
  const response = options.allowUrl
    ? await followRedirectsWithPolicy(url, options.allowUrl, fetchImpl, init)
    : await fetchImpl(url, init);
  if (!response.ok) {
    throw new Error(`HTTP ${response.status} ${response.statusText} fetching ${url}`);
  }
  return response;
}

/** Follows redirects manually so `allowUrl` is enforced on every hop, not just the first URL. */
async function followRedirectsWithPolicy(
  url: string,
  allowUrl: (url: string) => boolean,
  fetchImpl: FetchLike,
  init: RequestInit,
): Promise<Response> {
  let current = url;
  for (let hop = 0; hop < maxRedirectHops; hop++) {
    const response = await fetchImpl(current, { ...init, redirect: "manual" });
    const location = response.headers.get("location");
    if (response.status < 300 || response.status >= 400 || !location) {
      return response;
    }

    await response.body?.cancel().catch(() => {});
    current = new URL(location, current).toString();
    if (!allowUrl(current)) {
      throw new Error(`URL blocked by allowUrl policy: ${current}`);
    }
  }
  throw new Error(`Too many redirects fetching ${url}`);
}

/** Reads a response body, rejecting once it exceeds `maxBytes` (by content-length or streamed size). */
export async function readBodyLimited(response: Response, maxBytes: number): Promise<ArrayBuffer> {
  const declared = Number(response.headers.get("content-length"));
  if (Number.isFinite(declared) && declared > maxBytes) {
    throw new Error(`Response exceeds ${maxBytes} bytes (content-length ${declared})`);
  }

  const body = response.body;
  if (!body) {
    return response.arrayBuffer();
  }

  const reader = body.getReader();
  const chunks: Uint8Array[] = [];
  let total = 0;
  for (;;) {
    const { done, value } = await reader.read();
    if (done) {
      break;
    }

    total += value.byteLength;
    if (total > maxBytes) {
      await reader.cancel().catch(() => {});
      throw new Error(`Response exceeds ${maxBytes} bytes`);
    }
    chunks.push(value);
  }

  const out = new Uint8Array(total);
  let offset = 0;
  for (const chunk of chunks) {
    out.set(chunk, offset);
    offset += chunk.byteLength;
  }
  return out.buffer;
}

function isRemoteUrl(value: string): boolean {
  return value.startsWith("https://") || value.startsWith("http://");
}

function collectCssUrls(value: unknown, urls: Set<string>) {
  if (typeof value === "string") {
    for (const match of value.matchAll(cssUrlPattern)) {
      const url = match[2]?.trim();
      if (url && isRemoteUrl(url)) {
        urls.add(url);
      }
    }
  } else if (Array.isArray(value)) {
    for (const item of value) {
      collectCssUrls(item, urls);
    }
  }
}

/**
 * Every remote image URL a node tree references: `<img src>`, `backgroundImage`, `maskImage`,
 * `listStyleImage`.
 */
function extractImageUrls(node: Node): string[] {
  const urls = new Set<string>();

  const visit = (current: Node) => {
    const collectStyleUrls = (style: CSSProperties | undefined) => {
      if (!style) {
        return;
      }

      collectCssUrls(style.backgroundImage, urls);
      collectCssUrls(style.maskImage, urls);
      collectCssUrls(style.listStyleImage, urls);
    };

    collectStyleUrls(current.style);
    collectStyleUrls(current.preset);
    collectCssUrls(current.tw, urls);

    if (current.type === "image") {
      if (typeof current.src === "string" && isRemoteUrl(current.src)) {
        urls.add(current.src);
      }
      return;
    }

    if (current.type === "container") {
      for (const child of current.children ?? []) {
        visit(child);
      }
    }
  };

  visit(node);
  return [...urls];
}

/**
 * A cache of image fetches keyed by URL. Sharing one across renders deduplicates concurrent
 * requests for the same URL (single-flight) and reuses their bytes. Any object with `Map`-like
 * `get`/`set`/`delete` works, so LRU/TTL policies can be plugged in.
 */
export interface ImageFetchCache {
  get(url: string): Promise<ArrayBuffer> | undefined;
  set(url: string, data: Promise<ArrayBuffer>): unknown;
  delete(url: string): unknown;
}

/** Fetches a URL's bytes, coalescing concurrent requests for the same URL through `cache`. A
 * rejected fetch is evicted so a later call can retry instead of replaying the failure. Cached
 * entries pass the current call's `allowUrl` and `maxBytes` too: a shared cache can hold bytes
 * fetched under another call's options. */
function fetchImageData(
  url: string,
  options: FetchOptions,
  fetchCache?: ImageFetchCache,
): Promise<ArrayBuffer> {
  const maxBytes = options.maxBytes ?? defaultMaxFetchBytes;

  const cached = fetchCache?.get(url);
  if (cached) {
    if (options.allowUrl && !options.allowUrl(url)) {
      return Promise.reject(new Error(`URL blocked by allowUrl policy: ${url}`));
    }

    return cached.then((data) => {
      if (data.byteLength > maxBytes) {
        throw new Error(`Response exceeds ${maxBytes} bytes`);
      }
      return data;
    });
  }

  const promise = fetchOk(url, options)
    .then((response) => readBodyLimited(response, maxBytes))
    .catch((error) => {
      fetchCache?.delete(url);
      throw error;
    });

  fetchCache?.set(url, promise);
  return promise;
}

/** A fetched image entry: its source URL and raw bytes. */
export interface FetchedImage {
  src: string;
  data: ArrayBuffer;
}

export type PrepareImagesOptions<T extends { src: string } = FetchedImage> = FetchOptions & {
  /** The node tree(s) whose remote images to fetch. */
  node: Node | Node[];
  /** Pre-fetched entries; their URLs are not re-fetched. */
  sources?: T[];
  /** Single-flight byte cache shared across renders. */
  fetchCache?: ImageFetchCache;
  /** Throw on any fetch failure; if `false`, failed URLs are dropped. @default true */
  throwOnError?: boolean;
};

/**
 * Collects every remote image a node tree references, fetches the ones not already in `sources`,
 * and returns them as entries ready to hand to a renderer.
 */
export async function prepareImages<T extends { src: string } = FetchedImage>({
  node,
  sources = [],
  fetchCache,
  fetch,
  timeout,
  signal,
  maxBytes,
  allowUrl,
  throwOnError = true,
}: PrepareImagesOptions<T>): Promise<(T | FetchedImage)[]> {
  const nodes = Array.isArray(node) ? node : [node];
  const provided = new Map<string, T>();

  for (const image of sources) {
    provided.set(image.src, image);
  }

  const urls = [...new Set(nodes.flatMap(extractImageUrls))].filter((url) => !provided.has(url));
  const fetchOptions: FetchOptions = { fetch, timeout, signal, maxBytes, allowUrl };

  const tasks = urls.map(async (src): Promise<FetchedImage> => ({
    src,
    data: await fetchImageData(src, fetchOptions, fetchCache),
  }));
  const fetched = throwOnError
    ? await Promise.all(tasks)
    : (await Promise.allSettled(tasks))
        .filter((result) => result.status === "fulfilled")
        .map((result) => result.value);

  return [...provided.values(), ...fetched];
}
