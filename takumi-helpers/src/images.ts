import type { CSSProperties } from "react";
import type { Node } from "./types";
import type { FetchOptions } from "./fetch";
import { fetchImages, isRemoteUrl } from "./fetch/images";

const cssUrlPattern = /url\(\s*(['"]?)(.*?)\1\s*\)/g;

function collectCssUrls(value: unknown, urls: Set<string>) {
  if (typeof value === "string") {
    for (const match of value.matchAll(cssUrlPattern)) {
      const url = match[2]?.trim();
      if (url && isRemoteUrl(url)) {
        urls.add(url);
      }
    }
  } else if (Array.isArray(value)) {
    for (const item of value) {
      collectCssUrls(item, urls);
    }
  }
}

function collectStyleUrls(style: CSSProperties | undefined, urls: Set<string>) {
  if (!style) {
    return;
  }

  collectCssUrls(style.backgroundImage, urls);
  collectCssUrls(style.maskImage, urls);
  collectCssUrls(style.listStyleImage, urls);
}

/** Adds every remote image URL the tree references through `src` or image-bearing styles. */
function collectImageUrls(node: Node, urls: Set<string>) {
  collectStyleUrls(node.style, urls);
  collectStyleUrls(node.preset, urls);
  collectCssUrls(node.tw, urls);

  if (node.type === "image") {
    if (typeof node.src === "string" && isRemoteUrl(node.src)) {
      urls.add(node.src);
    }
    return;
  }

  if (node.type === "container") {
    for (const child of node.children ?? []) {
      collectImageUrls(child, urls);
    }
  }
}

/**
 * A cache of image fetches keyed by URL. Sharing one across renders deduplicates concurrent
 * requests for the same URL (single-flight) and reuses their bytes. Any object with `Map`-like
 * `get`/`set`/`delete` works, so LRU/TTL policies can be plugged in.
 */
export interface ImageFetchCache {
  get(url: string): Promise<ArrayBuffer> | undefined;
  set(url: string, data: Promise<ArrayBuffer>): unknown;
  delete(url: string): unknown;
}

/** A fetched image entry: its source URL and raw bytes. */
export interface FetchedImage {
  src: string;
  data: ArrayBuffer;
}

export type PrepareImagesOptions<T extends { src: string } = FetchedImage> = FetchOptions & {
  /** Image URLs to fetch. A relative one is fetched only through a custom `fetch`, which can resolve it. */
  urls?: Iterable<string>;
  /**
   * The node tree(s) whose remote images to fetch, read from `src`, inline styles, and `tw`
   * classes. Images a stylesheet or a `<style>` element names are not collected.
   */
  node?: Node | Node[];
  /** Pre-fetched entries; their URLs are not re-fetched. */
  sources?: T[];
  /** Single-flight byte cache shared across renders. */
  fetchCache?: ImageFetchCache;
  /** Throw on any fetch failure; if `false`, failed URLs are dropped. @default true */
  throwOnError?: boolean;
};

/**
 * Collects every remote image a node tree references, fetches the ones not already in `sources`,
 * and returns them as entries ready to hand to a renderer.
 *
 * @deprecated Use a renderer's `prepare`, which reads the images after the cascade and fetches
 * them with the same options.
 */
export async function prepareImages<T extends { src: string } = FetchedImage>({
  urls,
  node,
  ...options
}: PrepareImagesOptions<T>): Promise<(T | FetchedImage)[]> {
  const referenced = new Set(urls);

  for (const root of node === undefined ? [] : Array.isArray(node) ? node : [node]) {
    collectImageUrls(root, referenced);
  }

  return fetchImages({ ...options, urls: referenced });
}
