import type { CSSProperties } from "react";
import type { Node } from "./types";

const defaultTimeout = 5000;
const cssUrlPattern = /url\(\s*(['"]?)(.*?)\1\s*\)/g;

function isFetchableResourceUrl(value: string): boolean {
  return value.startsWith("https://") || value.startsWith("http://");
}

function collectCssUrls(value: unknown, urls: Set<string>) {
  if (typeof value === "string") {
    for (const match of value.matchAll(cssUrlPattern)) {
      const url = match[2]?.trim();
      if (url && isFetchableResourceUrl(url)) {
        urls.add(url);
      }
    }
  } else if (Array.isArray(value)) {
    for (const item of value) {
      collectCssUrls(item, urls);
    }
  }
}

export function extractResourceUrls(node: Node): string[] {
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
      if (typeof current.src === "string" && isFetchableResourceUrl(current.src)) {
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

export type FetchLike = (input: string, init?: RequestInit) => Promise<Response>;

export type FetchOptions = {
  /** Custom fetch implementation. @default globalThis.fetch */
  fetch?: FetchLike;
  /** Abort the request after this many milliseconds. */
  timeout?: number;
};

/** Fetches a URL, applying a timeout signal and throwing on a non-OK status. */
export async function fetchOk(url: string, options: FetchOptions & { init?: RequestInit } = {}) {
  const fetchImpl = options.fetch ?? globalThis.fetch;
  const signal =
    options.timeout === undefined ? options.init?.signal : AbortSignal.timeout(options.timeout);
  const response = await fetchImpl(url, { ...options.init, signal });
  if (!response.ok) {
    throw new Error(`HTTP ${response.status} ${response.statusText} fetching ${url}`);
  }
  return response;
}

export type FetchResourcesOptions = FetchOptions & {
  /**
   * Whether to throw on any fetch failure. If false, returns only successful fetches.
   * @default true
   */
  throwOnError?: boolean;
  /**
   * Cache for fetched resources.
   * Custom features (like LRU, TTL, etc.) can be implemented by providing an extended `Map<string, ArrayBuffer>`.
   */
  cache?: Pick<Map<string, ArrayBuffer>, "has" | "get" | "set">;
};

/**
 * Fetches multiple resources concurrently, deduplicating URLs.
 *
 * @param urls - URLs to fetch
 * @param options - Fetch options; `timeout` defaults to 5000ms
 * @returns Array of { src: string, data: ArrayBuffer }
 */
export async function fetchResources(urls: string[], options?: FetchResourcesOptions) {
  const throwOnError = options?.throwOnError ?? true;
  // Per-request timeout so one slow URL can't consume the whole batch's budget.
  const timeout = options?.timeout ?? defaultTimeout;

  const promises = [...new Set(urls)].map(async (url) => {
    if (options?.cache?.has(url)) {
      const cached = options.cache.get(url);
      if (cached) {
        return { src: url, data: cached };
      }
    }

    const buffer = await fetchOk(url, { fetch: options?.fetch, timeout }).then((r) =>
      r.arrayBuffer(),
    );
    options?.cache?.set(url, buffer);
    return { src: url, data: buffer };
  });

  if (throwOnError) {
    return Promise.all(promises);
  }

  const results = await Promise.allSettled(promises);
  return results
    .filter(
      (r): r is PromiseFulfilledResult<{ src: string; data: ArrayBuffer }> =>
        r.status === "fulfilled",
    )
    .map((r) => r.value);
}
