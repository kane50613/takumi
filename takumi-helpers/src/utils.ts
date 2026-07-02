export type FetchLike = (input: string, init?: RequestInit) => Promise<Response>;

export type FetchOptions = {
  /** Custom fetch implementation. @default globalThis.fetch */
  fetch?: FetchLike;
  /** Abort the request after this many milliseconds. */
  timeout?: number;
  /** Caller abort signal, combined with the timeout. */
  signal?: AbortSignal;
};

/** Fetches a URL, applying a timeout signal and throwing on a non-OK status. */
export async function fetchOk(url: string, options: FetchOptions & { init?: RequestInit } = {}) {
  const fetchImpl = options.fetch ?? globalThis.fetch;
  const timeoutSignal =
    options.timeout === undefined ? undefined : AbortSignal.timeout(options.timeout);
  const signals = [options.signal, options.init?.signal, timeoutSignal].filter(
    (s): s is AbortSignal => s !== undefined,
  );
  const signal = signals.length ? AbortSignal.any(signals) : undefined;
  const response = await fetchImpl(url, { ...options.init, signal });
  if (!response.ok) {
    throw new Error(`HTTP ${response.status} ${response.statusText} fetching ${url}`);
  }
  return response;
}
