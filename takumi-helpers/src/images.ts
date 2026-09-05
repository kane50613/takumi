import type { CSSProperties } from "react";
import type { Node } from "./types";
import { defaultMaxFetchBytes, fetchOk, type FetchOptions, readBodyLimited } from "./fetch";
import { FetchDeadline } from "./fetch/deadline";

const cssUrlPattern = /url\(\s*(['"]?)(.*?)\1\s*\)/g;

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

function fetchImageData(
  url: string,
  options: FetchOptions,
  fetchCache?: ImageFetchCache,
): Promise<ArrayBuffer> {
  const maxBytes = options.maxBytes ?? defaultMaxFetchBytes;
  const { allowUrl } = options;

  const cached = fetchCache?.get(url);
  if (cached) {
    return new FetchDeadline(options).waitFor(cached).then((data) => {
      if (allowUrl && !allowUrl(url)) {
        throw new Error(`URL blocked by allowUrl policy: ${url}`);
      }

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
