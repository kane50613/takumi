import {
  defaultMaxFetchBytes as maxFetchBytes,
  fetchOk as fetchOkInternal,
  readBodyLimited as readBodyLimitedInternal,
} from "./fetch";

export * from "./fonts";
export * from "./helpers";
export * from "./types";
export type { FetchLike, FetchOptions } from "./fetch";
export * from "./images";

/** @deprecated Set the `maxBytes` fetch option explicitly instead. Will be removed in v3. */
export const defaultMaxFetchBytes: number = maxFetchBytes;

/** @deprecated Call `fetch` instead. Will be removed in v3. */
export const fetchOk: typeof fetchOkInternal = fetchOkInternal;

/** @deprecated Read the body with `response.arrayBuffer()` instead. Will be removed in v3. */
export const readBodyLimited: typeof readBodyLimitedInternal = readBodyLimitedInternal;
