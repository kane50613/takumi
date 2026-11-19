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

  return isEmojiImageUrl(url) ? defaultStore(fetch) : null;
}

/**
 * An image URL a fetch left out, and why.
 *
 * @internal
 */
export type FailedImage = { src: string; error: unknown };

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
  throwOnError = true,
  ...options
}: FetchImagesOptions<T>): Promise<(T | FetchedImage)[]> {
  if (!throwOnError) {
    return (await fetchImagesSettled(options)).images;
  }

  const { provided, tasks } = startFetches(options);
  // An emoji image never rejects the render. One that fails to fetch is left out.
  const emoji = fulfilled(tasks.filter(({ src }) => isEmojiImageUrl(src)));
  const fetched = await Promise.all(
    tasks.filter(({ src }) => !isEmojiImageUrl(src)).map(({ fetched }) => fetched),
  );
  const emojiFetched = await emoji;

  options.signal?.throwIfAborted();

  return [...provided, ...fetched, ...emojiFetched];
}

/** The images of the `tasks` that fetch, leaving out the ones that fail. */
async function fulfilled(tasks: ImageFetch[]): Promise<FetchedImage[]> {
  return (await Promise.allSettled(tasks.map(({ fetched }) => fetched))).flatMap((result) =>
    result.status === "fulfilled" ? [result.value] : [],
  );
}

/**
 * Fetches as {@link fetchImages} does with `throwOnError: false`, also returning the URLs it left out
 * other than emoji images.
 *
 * @internal
 */
export async function fetchImagesSettled<T extends { src: string } = FetchedImage>(
  options: Omit<FetchImagesOptions<T>, "throwOnError">,
): Promise<{ images: (T | FetchedImage)[]; failed: FailedImage[] }> {
  const { provided, tasks } = startFetches(options);
  const settled = await Promise.all(
    tasks.map(({ src, fetched }) =>
      fetched.then(
        (image) => ({ image }),
        (error: unknown) => ({ failure: { src, error } }),
      ),
    ),
  );

  options.signal?.throwIfAborted();

  return {
    images: [...provided, ...settled.flatMap((result) => ("image" in result ? result.image : []))],
    failed: settled.flatMap((result) =>
      "failure" in result && !isEmojiImageUrl(result.failure.src) ? result.failure : [],
    ),
  };
}

/** A started fetch of the image at `src`. */
type ImageFetch = { src: string; fetched: Promise<FetchedImage> };

/** Starts fetching each of `urls` not already in `sources`, with the `sources` to put first. */
function startFetches<T extends { src: string }>({
  urls = [],
  sources = [],
  fetchCache,
  store,
  fetch,
  timeout,
  signal,
  maxBytes,
  allowUrl,
}: Omit<FetchImagesOptions<T>, "throwOnError">): {
  provided: T[];
  tasks: ImageFetch[];
} {
  const provided = new Map(sources.map((image) => [image.src, image]));
  const missing = [...new Set(urls)].filter(
    (url) => !provided.has(url) && (fetch !== undefined || isRemoteUrl(url)),
  );
  const fetchOptions: FetchOptions = { fetch, timeout, signal, maxBytes, allowUrl };

  return {
    provided: [...provided.values()],
    tasks: missing.map((src) => ({
      src,
      fetched: (fetchCache
        ? fetchCached(src, fetchOptions, fetchCache)
        : fetchBytes(src, { ...fetchOptions, store: storeFor(src, { store, fetch }) })
      ).then((data) => ({ src, data })),
    })),
  };
}
