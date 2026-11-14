import type { FetchOptions } from "../fetch";
import { isEmojiImageUrl } from "../emoji";
import type { FetchedImage, ImageFetchCache, PrepareImagesOptions } from "../images";
import { fetchBytes } from "./bytes";
import { defaultStore, type DownloadStore, perObject, type StoredDownload } from "./store";

/** Whether `value` is an `http:` or `https:` URL, the only kind fetched without a custom `fetch`. */
export function isRemoteUrl(value: string): boolean {
  return /^https?:\/\//i.test(value);
}

/** The URL each download a {@link FetchCacheStore} kept came from after redirects. */
const finalUrls = new WeakMap<Promise<ArrayBuffer>, string>();

/** A deprecated `fetchCache` read and written as a {@link DownloadStore}. */
class FetchCacheStore implements DownloadStore {
  constructor(private readonly cache: ImageFetchCache) {}

  get(url: string): Promise<StoredDownload | undefined> | undefined {
    const pending = this.cache.get(url);

    // A promise the caller put there came from its own fetch, at the URL it is keyed by.
    return pending?.then(
      (bytes) => ({ bytes, url: finalUrls.get(pending) ?? url }),
      () => undefined,
    );
  }

  set(url: string, download: StoredDownload): void {
    const kept = Promise.resolve(download.bytes);

    finalUrls.set(kept, download.url);
    this.cache.set(url, kept);
  }
}

/** The store a `fetchCache` stands for, one for each cache so requests sharing it share downloads. */
const fetchCacheStore = perObject((cache: ImageFetchCache) => new FetchCacheStore(cache));

/**
 * The store that keeps `url`, which is the `fetchCache` when there is one and `store` otherwise.
 * Without either, emoji SVGs go to the default store, since providers serve them at stable URLs.
 */
function storeFor(
  url: string,
  { fetchCache, store, fetch }: Pick<PrepareImagesOptions, "fetchCache" | "store" | "fetch">,
): DownloadStore | null {
  if (fetchCache) {
    return fetchCacheStore(fetchCache);
  }

  if (store !== undefined) {
    return store;
  }

  return isEmojiImageUrl(url) ? defaultStore(fetch ?? globalThis.fetch) : null;
}

/** Fetches each of `urls` not already in `sources`, returning the entries with `sources` first. */
export async function fetchImages<T extends { src: string } = FetchedImage>({
  urls = [],
  sources = [],
  fetchCache,
  store,
  fetch,
  timeout,
  signal,
  maxBytes,
  allowUrl,
  throwOnError = true,
}: Omit<PrepareImagesOptions<T>, "node">): Promise<(T | FetchedImage)[]> {
  const provided = new Map(sources.map((image) => [image.src, image]));
  const missing = [...new Set(urls)].filter(
    (url) => !provided.has(url) && (fetch !== undefined || isRemoteUrl(url)),
  );
  const fetchOptions: FetchOptions = { fetch, timeout, signal, maxBytes, allowUrl };

  const tasks = missing.map(async (src): Promise<FetchedImage> => ({
    src,
    data: await fetchBytes(src, {
      ...fetchOptions,
      store: storeFor(src, { fetchCache, store, fetch }),
    }),
  }));
  const fetched = throwOnError
    ? await Promise.all(tasks)
    : (await Promise.allSettled(tasks))
        .filter((result) => result.status === "fulfilled")
        .map((result) => result.value);

  return [...provided.values(), ...fetched];
}
