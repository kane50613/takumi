import type { CssInput, Node } from "@takumi-rs/helpers";
import type { Properties } from "csstype";

export type {
  ContainerNode,
  ImageNode,
  NodeMetadata,
  RgbaImage,
  TextNode,
} from "@takumi-rs/helpers";

export type { CssInput, Node };

export type ByteBuf = Uint8Array | ArrayBuffer | Buffer;

export interface FontDetails {
  /**
   * The name of the font. If not provided, the name in the font file will be used.
   */
  name?: string;
  /**
   * The font data.
   */
  data: ByteBuf;
  /**
   * The weight of the font. If not provided, the weight in the font file will be used.
   */
  weight?: number;
  /**
   * The style of the font. If not provided, the style in the font file will be used.
   */
  style?: "normal" | "italic" | "oblique" | `oblique ${number}deg` | (string & {});
  /**
   * Inclusive codepoint ranges the font serves, as `@font-face` `unicode-range`. Register each
   * subset of a family under the same `name` with its own ranges.
   */
  unicodeRange?: [number, number][];
  /** @deprecated Register under `name` with `unicodeRange` instead. */
  subsetOf?: string;
  /** @deprecated Ignored; `unicodeRange` decides which subset serves a codepoint. */
  subsetRank?: number;
  /**
   * CSS generic family keyword this font resolves for, so stacks ending in e.g.
   * `monospace` reach it without naming the family.
   */
  generic?:
    | "serif"
    | "sans-serif"
    | "monospace"
    | "cursive"
    | "fantasy"
    | "system-ui"
    | "ui-serif"
    | "ui-sans-serif"
    | "ui-monospace"
    | "ui-rounded"
    | "emoji"
    | "math"
    | "fangsong";
}

export type Font = FontDetails | ByteBuf;

/** A font of a render's `fonts` list, described before its bytes arrive. */
type ListedFont = Pick<FontDetails, "name" | "weight" | "style" | "unicodeRange"> & { key: string };

/** @deprecated Use a `{ keyframes, steps }` entry in `css` instead. Will be removed in v3. */
export type KeyframesMap = Record<string, Record<string, Properties>>;
/** @deprecated Use a `{ keyframes, steps }` entry in `css` instead. Will be removed in v3. */
export type KeyframesRuleList = {
  name: string;
  keyframes: {
    offsets: number[];
    declarations: Record<string, Properties>;
  }[];
}[];
/** @deprecated Use a `{ keyframes, steps }` entry in `css` instead. Will be removed in v3. */
export type Keyframes = KeyframesMap | KeyframesRuleList;

/** Output format for animated images. */
export type AnimationOutputFormat = "webp" | "apng" | "gif";

export type DitheringAlgorithm = "none" | "ordered-bayer" | "floyd-steinberg";

/** Cache policy for a decoded image. Defaults to `"auto"`. */
export type ImageCacheMode = "auto" | "none";

/** An image source with its URL and raw data. */
export interface ImageSource {
  /** The source URL of the image. */
  src: string;
  /** The raw image data (Uint8Array or ArrayBuffer). */
  data: Uint8Array | ArrayBuffer;
  /** Cache policy for the decoded image. Defaults to `"auto"`. */
  cache?: ImageCacheMode;
}

/** Output format for static images. */
export type OutputFormat = "webp" | "png" | "jpeg" | "ico" | "raw";

/** Options for rendering a sequential scene animation. */
export interface RenderAnimationOptions {
  /** The scenes to render sequentially. */
  scenes: Array<AnimationScene>;
  /** Whether to draw debug borders around layout elements. */
  drawDebugBorder?: boolean;
  /** The width of each frame in pixels. */
  width: number;
  /** The height of each frame in pixels. */
  height: number;
  /** The output animation format (WebP, APNG, or GIF). */
  format?: AnimationOutputFormat;
  /**
   * The quality of lossy WebP (0-100). Ignored for APNG and GIF, and when
   * `lossless` is set.
   */
  quality?: number;
  /**
   * Encode WebP losslessly. Defaults to lossless when neither `quality` nor
   * `lossless` is given. Ignored for APNG and GIF.
   */
  lossless?: boolean;
  /** Frames per second for timeline sampling. */
  fps: number;
  /** Images keyed by `src`, each carrying raw bytes. */
  images?: Array<ImageSource>;
  /**
   * CSS to apply before rendering: stylesheet text, or a rule written as an
   * object.
   */
  css?: CssInput[];
  /** @deprecated Use `css` instead. Will be removed in v3. */
  stylesheets?: Array<string>;
  /** @deprecated Use a `{ keyframes, steps }` entry in `css` instead. Will be removed in v3. */
  keyframes?: Keyframes;
  /**
   * The device pixel ratio.
   * @default 1.0
   */
  devicePixelRatio?: number;
  /**
   * Per-render font stack: ordered family names used as the fallback chain.
   * Defaults to all registered families in registration order.
   */
  fontFamilies?: Array<string>;
  /** Default BCP-47 language applied to the root, inherited by nodes without their own lang. */
  lang?: string;
}

/** Options for constructing a [`Renderer`]. */
export interface RendererOptions {
  /**
   * Byte budget shared by every cached resource — decoded images, SVG
   * rasters, parsed stylesheets. `0` disables caching.
   * @default 64 MiB
   */
  cacheMaxBytes?: number;
  /**
   * Bytes of decoded fonts that renders' `fonts` lists may keep registered. Past it, the least
   * recently used families are released and load again when a render lists them. Fonts from
   * `registerFont` stay, and so does the font loaded last, even past the budget alone. A running
   * render keeps the fonts it started with.
   * @default unbounded
   */
  fontCacheMaxBytes?: number;
}

/** Options for rendering an image. */
export interface RenderOptions {
  /** The width of the image. If not provided, the width will be automatically calculated based on the content. */
  width?: number;
  /** The height of the image. If not provided, the height will be automatically calculated based on the content. */
  height?: number;
  /** The format of the image. */
  format?: OutputFormat;
  /**
   * The quality of lossy formats (0-100). For JPEG; for WebP it selects lossy
   * encoding unless `lossless` is set.
   */
  quality?: number;
  /**
   * Encode WebP losslessly. Defaults to lossless when neither `quality` nor
   * `lossless` is given.
   */
  lossless?: boolean;
  /** Whether to draw debug borders. */
  drawDebugBorder?: boolean;
  /** Images keyed by `src`, each carrying raw bytes. */
  images?: Array<ImageSource>;
  /**
   * CSS to apply before rendering: stylesheet text, or a rule written as an
   * object.
   */
  css?: CssInput[];
  /** @deprecated Use `css` instead. Will be removed in v3. */
  stylesheets?: Array<string>;
  /** @deprecated Use a `{ keyframes, steps }` entry in `css` instead. Will be removed in v3. */
  keyframes?: Keyframes;
  /**
   * The device pixel ratio.
   * @default 1.0
   */
  devicePixelRatio?: number;
  /** The animation timeline time in milliseconds. */
  timeMs?: number;
  /** Dithers gradient fills before they quantize to 8-bit. */
  dithering?: DitheringAlgorithm;
  /**
   * Per-render font stack: ordered family names used as the fallback chain.
   * Defaults to all registered families in registration order.
   */
  fontFamilies?: Array<string>;
  /** Default BCP-47 language applied to the root, inherited by nodes without their own lang. */
  lang?: string;
}

/**
 * Options for rendering a node tree to an SVG document. SVG is a vector
 * format, so the raster-only knobs (`format`, `quality`, `lossless`,
 * `dithering`, `drawDebugBorder`, `devicePixelRatio`) do not apply.
 */
export interface SvgRenderOptions {
  /** The width of the viewport. If not provided, it is derived from content. */
  width?: number;
  /** The height of the viewport. If not provided, it is derived from content. */
  height?: number;
  /** Images keyed by `src`, each carrying raw bytes. */
  images?: Array<ImageSource>;
  /**
   * CSS to apply before rendering: stylesheet text, or a rule written as an
   * object.
   */
  css?: CssInput[];
  /** @deprecated Use `css` instead. Will be removed in v3. */
  stylesheets?: Array<string>;
  /** @deprecated Use a `{ keyframes, steps }` entry in `css` instead. Will be removed in v3. */
  keyframes?: Keyframes;
  /** The animation timeline time in milliseconds. */
  timeMs?: number;
  /**
   * Per-render font stack: ordered family names used as the fallback chain.
   * Defaults to all registered families in registration order.
   */
  fontFamilies?: Array<string>;
  /** Default BCP-47 language applied to the root, inherited by nodes without their own lang. */
  lang?: string;
}
