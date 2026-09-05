import { fetchOk, type FetchLike, type FetchOptions, readBodyLimited } from "../fetch";

const chromeUserAgent =
  "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

const maxCssBytes = 2 * 1024 * 1024;

const maxCachedCssBytes = 1024 * 1024;
const maxCachedCssEntries = 64;

export class FontStylesheetCache {
  private static readonly byFetch = new WeakMap<FetchLike, FontStylesheetCache>();
  private readonly entries = new Map<string, { pending: Promise<string>; bytes: number }>();
  private bytes = 0;

  private static forFetch(fetch: FetchLike) {
    let cache = this.byFetch.get(fetch);
    if (!cache) {
      cache = new FontStylesheetCache();
      this.byFetch.set(fetch, cache);
    }
    return cache;
  }

  get(url: string) {
    const entry = this.entries.get(url);
    if (entry) {
      this.entries.delete(url);
      this.entries.set(url, entry);
    }
    return entry?.pending;
  }

  set(url: string, pending: Promise<string>) {
    this.delete(url);
    const entry = { pending, bytes: url.length * 2 };
    this.entries.set(url, entry);
    this.bytes += entry.bytes;
    this.trim();
    pending.then(
      (css) => {
        if (this.entries.get(url) !== entry) {
          return;
        }
        entry.bytes += css.length * 2;
        this.bytes += css.length * 2;
        if (entry.bytes > maxCachedCssBytes) {
          this.delete(url);
        }
        this.trim();
      },
      () => {
        if (this.entries.get(url) === entry) {
          this.delete(url);
        }
      },
    );
  }

  delete(url: string) {
    const entry = this.entries.get(url);
    if (entry) {
      this.bytes -= entry.bytes;
      this.entries.delete(url);
    }
  }

  private trim() {
    for (const url of this.entries.keys()) {
      if (this.entries.size <= maxCachedCssEntries && this.bytes <= maxCachedCssBytes) {
        break;
      }
      this.delete(url);
    }
  }

  private static read(url: string, options: FetchOptions) {
    return fetchOk(url, {
      ...options,
      init: { headers: { "User-Agent": chromeUserAgent } },
    })
      .then((r) => readBodyLimited(r, options.maxBytes ?? maxCssBytes))
      .then((buffer) => new TextDecoder().decode(buffer));
  }

  static load(
    url: string,
    options: FetchOptions & {
      cache?: Pick<Map<string, Promise<string>>, "get" | "set" | "delete">;
    },
  ) {
    options.signal?.throwIfAborted();
    if (options.signal || options.allowUrl) {
      return this.read(url, options);
    }
    const cache = options.cache ?? this.forFetch(options.fetch ?? globalThis.fetch);

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

    const pending = this.read(url, options);
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
}
