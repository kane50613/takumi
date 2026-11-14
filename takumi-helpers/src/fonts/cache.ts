import type { FetchOptions } from "../fetch";
import { fetchBytes } from "../fetch/bytes";
import type { DownloadStore } from "../fetch/store";

const chromeUserAgent =
  "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

const maxCssBytes = 2 * 1024 * 1024;

/** css2 answers a current browser with `woff2` files, so every stylesheet request claims to be one. */
const stylesheetInit: RequestInit = { headers: { "User-Agent": chromeUserAgent } };

/** A cache of stylesheet text by URL. */
export type StylesheetCache = Pick<Map<string, Promise<string>>, "get" | "set" | "delete">;

function readStylesheet(url: string, options: FetchOptions & { store: DownloadStore | null }) {
  return fetchBytes(url, { ...options, init: stylesheetInit }, maxCssBytes).then((buffer) =>
    new TextDecoder().decode(buffer),
  );
}

/**
 * Fetches a font stylesheet through `store`. A `cache` of stylesheet text takes its place, unless
 * a `signal` or `allowUrl` is set.
 */
export function loadFontStylesheet(
  url: string,
  options: FetchOptions & { cache?: StylesheetCache; store: DownloadStore | null },
): Promise<string> {
  options.signal?.throwIfAborted();

  const cache = options.cache;

  if (!cache) {
    return readStylesheet(url, options);
  }

  if (options.signal || options.allowUrl) {
    return readStylesheet(url, { ...options, store: null });
  }

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

  const pending = readStylesheet(url, { ...options, store: null });

  cache.set(url, pending);
  pending.catch(() => {
    if (cache.get(url) === pending) {
      cache.delete(url);
    }
  });

  return pending;
}
