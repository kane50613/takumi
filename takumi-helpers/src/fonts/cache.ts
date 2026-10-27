import type { FetchOptions } from "../fetch";
import { fetchBytes } from "../fetch/bytes";
import { BoundedCache, perFetch } from "../fetch/cache";

const chromeUserAgent =
  "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

const maxCssBytes = 2 * 1024 * 1024;

const maxCachedCssBytes = 1024 * 1024;
const maxCachedCssEntries = 64;

const stylesheetCache = perFetch(
  () =>
    new BoundedCache<string>({
      maxBytes: maxCachedCssBytes,
      maxEntries: maxCachedCssEntries,
      sizeOf: (css) => css.length * 2,
    }),
);

function readStylesheet(url: string, options: FetchOptions) {
  return fetchBytes(
    url,
    { ...options, init: { headers: { "User-Agent": chromeUserAgent } } },
    maxCssBytes,
  ).then((buffer) => new TextDecoder().decode(buffer));
}

/** Fetches a font stylesheet, caching it per `fetch` function unless a `signal` or `allowUrl` is set. */
export function loadFontStylesheet(
  url: string,
  options: FetchOptions & {
    cache?: Pick<Map<string, Promise<string>>, "get" | "set" | "delete">;
  },
): Promise<string> {
  options.signal?.throwIfAborted();
  if (options.signal || options.allowUrl) {
    return readStylesheet(url, options);
  }
  const cache = options.cache ?? stylesheetCache(options.fetch ?? globalThis.fetch);

  const cached = cache.get(url);
  if (cached) {
    return cached.then((css) => {
      const maxBytes = options.maxBytes ?? maxCssBytes;
      if (new TextEncoder().encode(css).byteLength > maxBytes) {
        throw new Error(`Response exceeds ${maxBytes} bytes`);
      }
      return css;
    });
  }

  const pending = readStylesheet(url, options);
  cache.set(url, pending);
  if (options.cache) {
    pending.catch(() => {
      if (cache.get(url) === pending) {
        cache.delete(url);
      }
    });
  }

  return pending;
}
