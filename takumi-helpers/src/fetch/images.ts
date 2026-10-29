import { defaultMaxFetchBytes, type FetchOptions } from "../fetch";
import { isEmojiImageUrl } from "../emoji";
import type { FetchedImage, ImageFetchCache, PrepareImagesOptions } from "../images";
import { fetchBytes } from "./bytes";
import { BoundedCache, perFetch } from "./cache";
import { FetchDeadline } from "./deadline";

/** Whether `value` is an `http:` or `https:` URL, the only kind fetched without a custom `fetch`. */
export function isRemoteUrl(value: string): boolean {
  return /^https?:\/\//i.test(value);
}

/** The most a shared emoji fetch downloads, whatever the caller allows; emoji SVGs run a few KB. */
const maxSharedImageBytes = 1024 * 1024;

/** Emoji SVGs fetched without a `fetchCache`, which providers serve at stable URLs. */
const emojiImageCache = perFetch(
  () =>
    new BoundedCache<ArrayBuffer>({
      maxBytes: 8 * 1024 * 1024,
      maxEntries: 1024,
      sizeOf: (data) => data.byteLength,
    }),
);

function withinMaxBytes(data: ArrayBuffer, maxBytes: number): ArrayBuffer {
  if (data.byteLength > maxBytes) {
    throw new Error(`Response exceeds ${maxBytes} bytes`);
  }

  return data;
}

/**
 * Fetches `url` through a cache other callers share. The shared request carries no caller's
 * `signal` or `timeout`, so a caller that gives up ends only its own wait; it stops at the default
 * timeout and {@link maxSharedImageBytes}, and leaves the cache when it fails.
 */
function fetchSharedImageData(
  url: string,
  options: FetchOptions,
  cache: ImageFetchCache,
): Promise<ArrayBuffer> {
  let pending = cache.get(url);

  if (!pending) {
    options.signal?.throwIfAborted();
    pending = fetchBytes(url, { fetch: options.fetch, maxBytes: maxSharedImageBytes });
    cache.set(url, pending);
  }

  const maxBytes = options.maxBytes ?? defaultMaxFetchBytes;

  return new FetchDeadline(options).waitFor(pending).then((data) => withinMaxBytes(data, maxBytes));
}

function fetchImageData(
  url: string,
  options: FetchOptions,
  fetchCache?: ImageFetchCache,
): Promise<ArrayBuffer> {
  const maxBytes = options.maxBytes ?? defaultMaxFetchBytes;

  const cached = fetchCache?.get(url);
  if (cached) {
    return new FetchDeadline(options).waitFor(cached).then((data) => {
      if (options.allowUrl && !options.allowUrl(url)) {
        throw new Error(`URL blocked by allowUrl policy: ${url}`);
      }

      return withinMaxBytes(data, maxBytes);
    });
  }

  const promise = fetchBytes(url, options).catch((error) => {
    fetchCache?.delete(url);
    throw error;
  });

  fetchCache?.set(url, promise);
  return promise;
}

/**
 * Fetches each of `urls` not already in `sources`, and returns the entries ready to hand to a
 * renderer, `sources` first.
 */
export async function fetchImages<T extends { src: string } = FetchedImage>({
  urls = [],
  sources = [],
  fetchCache,
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
    data:
      !fetchCache && !allowUrl && isEmojiImageUrl(src)
        ? await fetchSharedImageData(src, fetchOptions, emojiImageCache(fetch ?? globalThis.fetch))
        : await fetchImageData(src, fetchOptions, fetchCache),
  }));
  const fetched = throwOnError
    ? await Promise.all(tasks)
    : (await Promise.allSettled(tasks))
        .filter((result) => result.status === "fulfilled")
        .map((result) => result.value);

  return [...provided.values(), ...fetched];
}
