export type FetchLike = (input: string, init?: RequestInit) => Promise<Response>;

export type FetchOptions = {
  /** Custom fetch implementation. @default globalThis.fetch */
  fetch?: FetchLike;
  /** Abort the request after this many milliseconds. */
  timeout?: number;
};

/** Fetches a URL, applying a timeout signal and throwing on a non-OK status. */
export async function fetchOk(url: string, options: FetchOptions & { init?: RequestInit } = {}) {
  const fetchImpl = options.fetch ?? globalThis.fetch;
  const signal =
    options.timeout === undefined ? options.init?.signal : AbortSignal.timeout(options.timeout);
  const response = await fetchImpl(url, { ...options.init, signal });
  if (!response.ok) {
    throw new Error(`HTTP ${response.status} ${response.statusText} fetching ${url}`);
  }
  return response;
}
