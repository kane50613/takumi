import type { CSSProperties } from "react";
import type { DownloadStore } from "./fetch/store";

/**
 * Value of the `text-fit` longhand: `[ none | grow | shrink ] [ consistent | per-line | per-line-all ]? <percentage>?`.
 *
 * @see https://drafts.csswg.org/css-text-4/#text-fit-property
 */
export type TextFit =
  | "none"
  | "grow"
  | "shrink"
  | "consistent"
  | "per-line"
  | "per-line-all"
  | `${number}%`
  | (string & {});

declare module "react" {
  interface CSSProperties extends Record<`--${string}`, string | number | undefined> {
    textFit?: TextFit;
  }
}

export type NodeAttributes = Record<string, string>;

/** A declaration block, with custom properties allowed. */
export type Declarations = CSSProperties;

/** A style rule written as an object. */
export type StyleRule = {
  selector: string;
  style?: Declarations;
  rules?: StyleRule[];
};

/** An animation written as an object. */
export type AnimationRule = {
  keyframes: string;
  steps: { offset: string; style?: Declarations }[];
};

/** A group of entries gated by a media query. */
export type MediaRule = {
  media: string;
  rules: CssInput[];
};

/** A group of entries gated by a support condition. */
export type SupportsRule = {
  supports: string;
  rules: CssInput[];
};

/** A cascade layer. Without `rules` it declares the layer's order alone. */
export type LayerRule = {
  layer: string;
  rules?: CssInput[];
};

/** One entry of the `css` render option: stylesheet text, or a rule object. */
export type CssInput = string | StyleRule | AnimationRule | MediaRule | SupportsRule | LayerRule;

/** A JSX element from any React-shaped runtime, Preact included. */
export type ReactElementLike = {
  type:
    | string
    | symbol
    | ((props: never) => unknown)
    | (new (props: never) => unknown)
    | ReactElementLike;
  props: unknown;
  $$typeof?: symbol | string;
};

export type NodeMetadata = {
  tagName?: string;
  className?: string;
  id?: string;
  dir?: "ltr" | "rtl";
  lang?: string;
  attributes?: NodeAttributes;
  tw?: string;
  style?: Declarations;
  preset?: Declarations;
};

export type Node = ContainerNode | TextNode | ImageNode;

export type ContainerNode = NodeMetadata & {
  type: "container";
  children?: Node[];
};

export type TextNode = NodeMetadata & {
  type: "text";
  text: string;
};

/** Raw row-major RGBA pixels, rendered without decoding. */
export type RgbaImage = {
  /** The image width in pixels. */
  width: number;
  /** The image height in pixels. */
  height: number;
  /** RGBA bytes, `width * height * 4` long. */
  data: Uint8Array | ArrayBuffer;
  /** The bytes are already alpha-premultiplied, so the premultiply pass is skipped. */
  premultiplied?: boolean;
};

export type ImageNode = NodeMetadata & {
  type: "image";
  src: string | Uint8Array | ArrayBuffer | RgbaImage;
  width?: number;
  height?: number;
};

/** The memory a renderer keeps across renders, one pool of bytes every kind of data shares. */
export interface CacheOptions {
  /**
   * The bytes the renderer keeps for later renders, shared by every kind of data. When it is full,
   * the entries used least recently go first, whatever their kind. `0` keeps nothing but the fonts
   * from `registerFont`. Sizes are estimates, and the process uses memory this does not count.
   * @default 128 MiB
   */
  maxBytes?: number;
  /** The most bytes one kind of data may hold, for a kind that needs a bound of its own. */
  budgets?: CacheBudgets;
  /**
   * Where the renderer keeps the bytes it downloads, by URL: images, `@font-face` files, and fonts
   * listed as URLs. The store decides what to keep and when to drop it. `null` keeps none.
   * @default a `MemoryStore` of 32 MiB for each renderer
   */
  store?: DownloadStore | null;
}

/**
 * The most bytes each kind of data may hold. A kind with a budget keeps its entries apart, and the
 * bytes it holds come out of `maxBytes`. `0` keeps none of that kind between renders, except as
 * `fonts` says.
 */
export interface CacheBudgets {
  /** Decoded images, and the SVG rasters and downscaled bitmaps drawn from them. */
  images?: number;
  /** SVG images laid out to drawing ops, which rasters and PDF and SVG output draw from. */
  pictures?: number;
  /** Parsed stylesheets. */
  stylesheets?: number;
  /** The rules `tw` classes compile to. */
  tailwind?: number;
  /**
   * Resolved glyphs, and the masks rasterized from glyphs and shapes.
   * @default none, or the budget the last `setGlyphCacheMaxBytes` call set where the package
   * exports it
   */
  glyphs?: number;
  /** Where glyph outlines cross a decoration line, for `text-decoration-skip-ink`. */
  intercepts?: number;
  /** The faces `@font-face` rules add, weighed by the font files they hold. */
  fontFaces?: number;
  /**
   * Decoded font files, each counted once. Past it, the files renders used least recently are
   * evicted one at a time and load again when a render lists them. A render keeps every file it
   * uses until it finishes, even past the budget. Fonts from `registerFont` always stay and count
   * toward it.
   */
  fonts?: number;
}

/**
 * What a renderer's caches hold and how their lookups went since the renderer was created, under
 * the keys `CacheOptions` uses. Sizes are the same estimates the budgets count, not process memory.
 */
export interface CacheStats {
  /** The pool that every kind of data shares. */
  memory: CacheMemoryStats;
  /** Each kind of data, whether it has a budget or shares the pool. */
  budgets: Record<keyof CacheBudgets, CacheKindStats>;
  /** The store that keeps downloaded bytes, or `null` when `store` is `null`. */
  store: CacheStoreStats | null;
}

/** The bytes the pool holds, every kind of data together. */
export interface CacheMemoryStats {
  /** The bytes held now, budgets included. */
  current: number;
  /** The most bytes held at once since the renderer was created. */
  high: number;
  /** The pool's `maxBytes`. */
  max: number;
}

/** What one kind of data holds and how lookups for it went. */
export interface CacheKindStats {
  /** The bytes held now. For `fonts`, fonts from `registerFont` included. */
  current: number;
  /** The kind's budget, or `null` when it has none. */
  max: number | null;
  /** Lookups that found what they looked for. */
  hits: number;
  /** Lookups that found nothing and did the work again. */
  misses: number;
  /** Entries released to stay within `maxBytes` or the budget. */
  evictions: number;
}

/**
 * What the download store holds and how the lookups the renderer made in it went. A store that
 * does not report its size, as a custom one may not, has `null` for `current`, `max`, and
 * `evictions`.
 */
export interface CacheStoreStats {
  /** The bytes held now. */
  current: number | null;
  /** The most bytes the store holds. */
  max: number | null;
  /** Lookups that found a download the request could use. */
  hits: number;
  /** Lookups that found none, or one the request's `allowUrl` refused, and downloaded it. */
  misses: number;
  /** Downloads the store dropped to stay within `max`. */
  evictions: number | null;
}

/** Options for constructing a renderer. */
export interface RendererOptions {
  /** The memory the renderer keeps across renders. */
  cache?: CacheOptions;
  /** @deprecated Use `cache.maxBytes`. */
  cacheMaxBytes?: number;
  /** @deprecated Use `cache.budgets.fonts`. */
  fontCacheMaxBytes?: number;
}
