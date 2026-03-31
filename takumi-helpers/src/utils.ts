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

    if (current.type === "image" && isFetchableResourceUrl(current.src)) {
      urls.add(current.src);
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

export type FetchResourcesOptions = {
  /**
   * Timeout in milliseconds.
   * @default 5000
   */
  timeout?: number;
  /**
   * Custom fetch function.
   * @default {globalThis.fetch}
   */
  fetch?: (input: string, init?: RequestInit) => Promise<Response>;
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
 * Fetches multiple resources concurrently.
 * Validates HTTP status codes and automatically deduplicates URLs.
 *
 * @param urls - URLs to fetch
 * @param options - Fetch options
 * @returns Array of { src: string, data: ArrayBuffer }
 */
export async function fetchResources(urls: string[], options?: FetchResourcesOptions) {
  const signal = AbortSignal.timeout(options?.timeout ?? defaultTimeout);
  const fetch = options?.fetch ?? globalThis.fetch;
  const throwOnError = options?.throwOnError ?? true;

  // Deduplicate URLs to avoid redundant fetches
  const uniqueUrls = [...new Set(urls)];

  const promises = uniqueUrls.map(async (url) => {
    // Check cache first if provided
    if (options?.cache?.has(url)) {
      const cached = options.cache.get(url);
      if (cached) {
        return { src: url, data: cached };
      }
    }

    const response = await fetch(url, { signal });

    // Validate HTTP status
    if (!response.ok) {
      throw new Error(`HTTP ${response.status}: ${response.statusText} for ${url}`);
    }

    const buffer = await response.arrayBuffer();

    // Store in cache if provided
    options?.cache?.set(url, buffer);

    return { src: url, data: buffer };
  });

  if (throwOnError) {
    // Original behavior: throw on any error
    return Promise.all(promises);
  }

  // Graceful error handling: return successful fetches only
  const results = await Promise.allSettled(promises);
  return results
    .filter(
      (r): r is PromiseFulfilledResult<{ src: string; data: ArrayBuffer }> =>
        r.status === "fulfilled",
    )
    .map((r) => r.value);
}
