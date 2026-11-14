import { defaultMaxFetchBytes, fetchOk, type FetchOptions, readBodyLimited } from "../fetch";
import { type DownloadStore, fetchStored } from "./store";

/**
 * Fetches `url`'s body, rejecting past `options.maxBytes`, else past `fallbackMaxBytes`. With a
 * `store`, it reads and keeps the body there.
 */
export function fetchBytes(
  url: string,
  options: FetchOptions & { init?: RequestInit; store?: DownloadStore | null },
  fallbackMaxBytes: number = defaultMaxFetchBytes,
): Promise<ArrayBuffer> {
  const maxBytes = options.maxBytes ?? fallbackMaxBytes;

  if (options.store) {
    return fetchStored(url, options, options.store, maxBytes);
  }

  return fetchOk(url, options).then((response) => readBodyLimited(response, maxBytes));
}
