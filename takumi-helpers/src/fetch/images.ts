import { defaultMaxFetchBytes, fetchFollowed, type FetchOptions, readBodyLimited } from "../fetch";
import { isEmojiImageUrl } from "../emoji";
import type { FetchedImage, ImageFetchCache, PrepareImagesOptions } from "../images";
import { fetchBytes } from "./bytes";
import { FetchDeadline } from "./deadline";
import { defaultStore, type MemoryStore } from "./store";

/** Whether `value` is an `http:` or `https:` URL, the only kind fetched without a custom `fetch`. */
export function isRemoteUrl(value: string): boolean {
  return /^https?:\/\//i.test(value);
}

/** The URL each download a `fetchCache` keeps came from after redirects. */
const finalUrls = new WeakMap<Promise<ArrayBuffer>, string>();

/**
 * Fetches `url` through a deprecated `fetchCache`, which keeps the pending download by URL. A kept
 * download serves the request when its `allowUrl` accepts the URL the bytes came from, and a request
 * it refuses fetches again without dropping it.
 */
async function fetchCached(
  url: string,
  options: FetchOptions,
  fetchCache: ImageFetchCache,
): Promise<ArrayBuffer> {
  if (options.allowUrl && !options.allowUrl(url)) {
    throw new Error(`URL blocked by allowUrl policy: ${url}`);
  }

  const maxBytes = options.maxBytes ?? defaultMaxFetchBytes;
  const cached = fetchCache.get(url);

  if (cached) {
    const data = await new FetchDeadline(options).waitFor(cached);

    if (options.allowUrl && !options.allowUrl(finalUrls.get(cached) ?? url)) {
      return fetchBytes(url, options);
    }

    if (data.byteLength > maxBytes) {
      throw new Error(`Response exceeds ${maxBytes} bytes`);
    }

    return data;
  }

  const download: Promise<ArrayBuffer> = fetchFollowed(url, options).then((followed) => {
    finalUrls.set(download, followed.url);

    return readBodyLimited(followed.response, maxBytes);
  });

  fetchCache.set(url, download);

  return download.catch((error) => {
    fetchCache.delete(url);
    throw error;
  });
}

/**
 * The store that keeps `url`: `store` when the caller gives one, or else the default store for an
 * emoji SVG, which providers serve at stable URLs.
 */
function storeFor(
  url: string,
  { store, fetch }: Pick<FetchImagesOptions, "store" | "fetch">,
): MemoryStore | null {
  if (store !== undefined) {
    return store;
  }

  return isEmojiImageUrl(url) ? defaultStore(fetch ?? globalThis.fetch) : null;
}

/**
 * How {@link fetchImages} fetches.
 *
 * @internal
 */
export type FetchImagesOptions<T extends { src: string } = FetchedImage> = Omit<
  PrepareImagesOptions<T>,
  "node"
> & {
  /**
   * The store that keeps the downloaded bytes. `null` keeps none. Left out, emoji SVGs go to a
   * store for each `fetch` function, 32 MiB at most, and other images to none.
   */
  store?: MemoryStore | null;
};

/**
 * Fetches each of `urls` not already in `sources`, returning the entries with `sources` first.
 *
 * @internal
 */
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
}: FetchImagesOptions<T>): Promise<(T | FetchedImage)[]> {
  const provided = new Map(sources.map((image) => [image.src, image]));
  const missing = [...new Set(urls)].filter(
    (url) => !provided.has(url) && (fetch !== undefined || isRemoteUrl(url)),
  );
  const fetchOptions: FetchOptions = { fetch, timeout, signal, maxBytes, allowUrl };

  const fetchImage = async (src: string): Promise<FetchedImage> => ({
    src,
    data: fetchCache
      ? await fetchCached(src, fetchOptions, fetchCache)
      : await fetchBytes(src, { ...fetchOptions, store: storeFor(src, { store, fetch }) }),
  });
  // An emoji image never rejects the render. One that fails to fetch is left out.
  const emoji = fulfilled(missing.filter(isEmojiImageUrl).map(fetchImage));
  const markup = missing.filter((url) => !isEmojiImageUrl(url)).map(fetchImage);
  const fetched = throwOnError ? await Promise.all(markup) : await fulfilled(markup);
  const emojiFetched = await emoji;

  signal?.throwIfAborted();

  return [...provided.values(), ...fetched, ...emojiFetched];
}

/** The values of the `tasks` that fulfil, leaving out the ones that reject. */
async function fulfilled<T>(tasks: Promise<T>[]): Promise<T[]> {
  return (await Promise.allSettled(tasks)).flatMap((result) =>
    result.status === "fulfilled" ? [result.value] : [],
  );
}
