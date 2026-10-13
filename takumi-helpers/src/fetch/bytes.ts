import { defaultMaxFetchBytes, fetchOk, type FetchOptions, readBodyLimited } from "../fetch";

/** Fetches `url`'s body, rejecting past `options.maxBytes`, else past `fallbackMaxBytes`. */
export function fetchBytes(
  url: string,
  options: FetchOptions & { init?: RequestInit },
  fallbackMaxBytes = defaultMaxFetchBytes,
): Promise<ArrayBuffer> {
  return fetchOk(url, options).then((response) =>
    readBodyLimited(response, options.maxBytes ?? fallbackMaxBytes),
  );
}
