import { fetchFollowed, type FetchLike, type FetchOptions, readBodyLimited } from "../fetch";

/** Bytes a store keeps for the URL they were requested at. */
type StoredDownload = {
  bytes: ArrayBuffer;
  /** The URL the bytes came from after redirects, which a later request's `allowUrl` checks. */
  url: string;
};

/** The bytes a {@link MemoryStore} holds unless it is built with another size. */
const defaultMaxStoreBytes = 32 * 1024 * 1024;

/**
 * Downloads in memory by the URL they were requested at, dropping the ones used least recently
 * once it holds more than `maxBytes`. Sizes are estimates that add two bytes for each character of
 * the URLs to the bytes.
 *
 * @internal
 */
export class MemoryStore {
  constructor(private readonly maxBytes: number = defaultMaxStoreBytes) {}

  private readonly entries = new Map<string, { download: StoredDownload; bytes: number }>();
  private current = 0;

  get(url: string): StoredDownload | undefined {
    const entry = this.entries.get(url);

    if (!entry) {
      return undefined;
    }

    this.entries.delete(url);
    this.entries.set(url, entry);

    return entry.download;
  }

  set(url: string, download: StoredDownload): void {
    this.delete(url);

    const bytes = download.bytes.byteLength + (url.length + download.url.length) * 2;

    if (bytes > this.maxBytes) {
      return;
    }

    this.entries.set(url, { download, bytes });
    this.current += bytes;

    for (const oldest of this.entries.keys()) {
      if (this.current <= this.maxBytes) {
        break;
      }

      this.delete(oldest);
    }
  }

  private delete(url: string) {
    const entry = this.entries.get(url);

    if (entry) {
      this.current -= entry.bytes;
      this.entries.delete(url);
    }
  }
}

const defaultStores = new WeakMap<FetchLike, MemoryStore>();

/**
 * The store downloads made with `fetch` share, one for each `fetch` function, since a custom
 * `fetch` can answer the same URL differently.
 *
 * @internal
 */
export function defaultStore(fetch: FetchLike): MemoryStore {
  let store = defaultStores.get(fetch);

  if (!store) {
    store = new MemoryStore();
    defaultStores.set(fetch, store);
  }

  return store;
}

/**
 * Fetches `url` through `store`. A kept download serves the request when its `allowUrl` accepts
 * the URL the bytes came from, and a request it refuses fetches again without dropping it.
 */
export async function fetchStored(
  url: string,
  options: FetchOptions & { init?: RequestInit },
  store: MemoryStore,
  maxBytes: number,
): Promise<ArrayBuffer> {
  if (options.allowUrl && !options.allowUrl(url)) {
    throw new Error(`URL blocked by allowUrl policy: ${url}`);
  }

  options.signal?.throwIfAborted();

  const stored = store.get(url);

  if (stored && (!options.allowUrl || options.allowUrl(stored.url))) {
    if (stored.bytes.byteLength > maxBytes) {
      throw new Error(`Response exceeds ${maxBytes} bytes`);
    }

    return stored.bytes;
  }

  const followed = await fetchFollowed(url, options);
  const bytes = await readBodyLimited(followed.response, maxBytes);

  store.set(url, { bytes, url: followed.url });

  return bytes;
}
