const defaultTimeout = 5000;

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
};

/**
 * Fetches multiple resources concurrently and returns them as a Map.
 * Validates HTTP status codes and automatically deduplicates URLs.
 *
 * @param urls - URLs to fetch
 * @param options - Fetch options
 * @returns Map of URL to ArrayBuffer
 */
export async function fetchResources(
  urls: string[],
  options?: FetchResourcesOptions,
) {
  const signal = AbortSignal.timeout(options?.timeout ?? defaultTimeout);
  const fetch = options?.fetch ?? globalThis.fetch;
  const throwOnError = options?.throwOnError ?? true;

  // Deduplicate URLs to avoid redundant fetches
  const uniqueUrls = [...new Set(urls)];

  const promises = uniqueUrls.map(async (url) => {
    const response = await fetch(url, { signal });

    // Validate HTTP status
    if (!response.ok) {
      throw new Error(
        `HTTP ${response.status}: ${response.statusText} for ${url}`,
      );
    }

    const buffer = await response.arrayBuffer();
    return [url, buffer] as const;
  });

  if (throwOnError) {
    // Original behavior: throw on any error
    const resources = await Promise.all(promises);
    return new Map(resources);
  }

  // Graceful error handling: return successful fetches only
  const results = await Promise.allSettled(promises);
  const successful = results
    .filter(
      (r): r is PromiseFulfilledResult<readonly [string, ArrayBuffer]> =>
        r.status === "fulfilled",
    )
    .map((r) => r.value);

  return new Map(successful);
}
