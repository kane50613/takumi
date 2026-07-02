import type { CSSProperties } from "react";
import type { Node } from "./types";

const defaultFetchTimeout = 5000;
const cssUrlPattern = /url\(\s*(['"]?)(.*?)\1\s*\)/g;

export type FetchLike = (input: string, init?: RequestInit) => Promise<Response>;

export type FetchOptions = {
  /** Custom fetch implementation. @default globalThis.fetch */
  fetch?: FetchLike;
  /** Abort the request after this many milliseconds. */
  timeout?: number;
  /** Caller abort signal, combined with the timeout. */
  signal?: AbortSignal;
};

/** Fetches a URL, applying a timeout signal and throwing on a non-OK status. */
export async function fetchOk(url: string, options: FetchOptions & { init?: RequestInit } = {}) {
  const fetchImpl = options.fetch ?? globalThis.fetch;
  const timeoutSignal =
    options.timeout === undefined ? undefined : AbortSignal.timeout(options.timeout);
  const signals = [options.signal, options.init?.signal, timeoutSignal].filter(
    (s): s is AbortSignal => s !== undefined,
  );
  const signal = signals.length ? AbortSignal.any(signals) : undefined;
  const response = await fetchImpl(url, { ...options.init, signal });
  if (!response.ok) {
    throw new Error(`HTTP ${response.status} ${response.statusText} fetching ${url}`);
  }
  return response;
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

/** Every remote image URL a node tree references: `<img src>`, `backgroundImage`, `maskImage`. */
function extractImageUrls(node: Node): string[] {
  const urls = new Set<string>();

  const visit = (current: Node) => {
    const collectStyleUrls = (style: CSSProperties | undefined) => {
      if (!style) {
        return;
      }

      collectCssUrls(style.backgroundImage, urls);
      collectCssUrls(style.maskImage, urls);
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
 * rejected fetch is evicted so a later call can retry instead of replaying the failure. */
function fetchImageData(
  url: string,
  options: FetchOptions,
  fetchCache?: ImageFetchCache,
): Promise<ArrayBuffer> {
  const cached = fetchCache?.get(url);
  if (cached) {
    return cached;
  }

  const promise = fetchOk(url, options)
    .then((response) => response.arrayBuffer())
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
  timeout = defaultFetchTimeout,
  signal,
  throwOnError = true,
}: PrepareImagesOptions<T>): Promise<(T | FetchedImage)[]> {
  const nodes = Array.isArray(node) ? node : [node];
  const provided = new Map<string, T>();

  for (const image of sources) {
    provided.set(image.src, image);
  }

  const urls = [...new Set(nodes.flatMap(extractImageUrls))].filter((url) => !provided.has(url));
  const fetchOptions: FetchOptions = { fetch, timeout, signal };

  const tasks = urls.map(
    async (src): Promise<FetchedImage> => ({
      src,
      data: await fetchImageData(src, fetchOptions, fetchCache),
    }),
  );
  const fetched = throwOnError
    ? await Promise.all(tasks)
    : (await Promise.allSettled(tasks))
        .filter((result) => result.status === "fulfilled")
        .map((result) => result.value);

  return [...provided.values(), ...fetched];
}
