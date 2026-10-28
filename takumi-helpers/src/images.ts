import type { CSSProperties } from "react";
import type { Node } from "./types";
import { defaultMaxFetchBytes, type FetchOptions } from "./fetch";
import { fetchBytes } from "./fetch/bytes";
import { FetchDeadline } from "./fetch/deadline";
import { BoundedCache, perFetch } from "./fetch/cache";
import { isEmojiImageUrl } from "./emoji";

const cssUrlPattern = /url\(\s*(['"]?)(.*?)\1\s*\)/g;

function isRemoteUrl(value: string): boolean {
  return /^https?:\/\//i.test(value);
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

function collectStyleUrls(style: CSSProperties | undefined, urls: Set<string>) {
  if (!style) {
    return;
  }

  collectCssUrls(style.backgroundImage, urls);
  collectCssUrls(style.maskImage, urls);
  collectCssUrls(style.listStyleImage, urls);
}

/** Adds every remote image URL the tree references through `src` or image-bearing styles. */
function collectImageUrls(node: Node, urls: Set<string>) {
  collectStyleUrls(node.style, urls);
  collectStyleUrls(node.preset, urls);
  collectCssUrls(node.tw, urls);

  if (node.type === "image") {
    if (typeof node.src === "string" && isRemoteUrl(node.src)) {
      urls.add(node.src);
    }
    return;
  }

  if (node.type === "container") {
    for (const child of node.children ?? []) {
      collectImageUrls(child, urls);
    }
  }
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

/** The most a shared emoji fetch downloads, whatever the caller allows; emoji SVGs run a few KB. */
const maxSharedImageBytes = 1024 * 1024;

/** Emoji SVGs fetched without a `fetchCache`, which providers serve at stable URLs. */
const emojiImageCache = perFetch(
  () =>
    new BoundedCache<ArrayBuffer>({
      maxBytes: 8 * 1024 * 1024,
      maxEntries: 1024,
      sizeOf: (data) => data.byteLength,
    }),
);

function withinMaxBytes(data: ArrayBuffer, maxBytes: number): ArrayBuffer {
  if (data.byteLength > maxBytes) {
    throw new Error(`Response exceeds ${maxBytes} bytes`);
  }

  return data;
}

/**
 * Fetches `url` through a cache other callers share. The shared request carries no caller's
 * `signal` or `timeout`, so a caller that gives up ends only its own wait; it stops at the default
 * timeout and {@link maxSharedImageBytes}, and leaves the cache when it fails.
 */
function fetchSharedImageData(
  url: string,
  options: FetchOptions,
  cache: ImageFetchCache,
): Promise<ArrayBuffer> {
  let pending = cache.get(url);

  if (!pending) {
    options.signal?.throwIfAborted();
    pending = fetchBytes(url, { fetch: options.fetch, maxBytes: maxSharedImageBytes });
    cache.set(url, pending);
  }

  const maxBytes = options.maxBytes ?? defaultMaxFetchBytes;

  return new FetchDeadline(options).waitFor(pending).then((data) => withinMaxBytes(data, maxBytes));
}

function fetchImageData(
  url: string,
  options: FetchOptions,
  fetchCache?: ImageFetchCache,
): Promise<ArrayBuffer> {
  const maxBytes = options.maxBytes ?? defaultMaxFetchBytes;

  const cached = fetchCache?.get(url);
  if (cached) {
    return new FetchDeadline(options).waitFor(cached).then((data) => {
      if (options.allowUrl && !options.allowUrl(url)) {
        throw new Error(`URL blocked by allowUrl policy: ${url}`);
      }

      return withinMaxBytes(data, maxBytes);
    });
  }

  const promise = fetchBytes(url, options).catch((error) => {
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
  /** Image URLs to fetch. A relative one is fetched only through a custom `fetch`, which can resolve it. */
  urls?: Iterable<string>;
  /**
   * The node tree(s) whose remote images to fetch, read from `src`, inline styles, and `tw`
   * classes. Images a stylesheet or a `<style>` element names are not collected.
   */
  node?: Node | Node[];
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
  urls,
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
  const provided = new Map(sources.map((image) => [image.src, image]));
  const referenced = new Set(urls);

  for (const root of node === undefined ? [] : Array.isArray(node) ? node : [node]) {
    collectImageUrls(root, referenced);
  }

  const missing = [...referenced].filter(
    (url) => !provided.has(url) && (fetch !== undefined || isRemoteUrl(url)),
  );
  const fetchOptions: FetchOptions = { fetch, timeout, signal, maxBytes, allowUrl };

  const tasks = missing.map(async (src): Promise<FetchedImage> => ({
    src,
    data:
      !fetchCache && !allowUrl && isEmojiImageUrl(src)
        ? await fetchSharedImageData(src, fetchOptions, emojiImageCache(fetch ?? globalThis.fetch))
        : await fetchImageData(src, fetchOptions, fetchCache),
  }));
  const fetched = throwOnError
    ? await Promise.all(tasks)
    : (await Promise.allSettled(tasks))
        .filter((result) => result.status === "fulfilled")
        .map((result) => result.value);

  return [...provided.values(), ...fetched];
}
