import type { CssInput, Node } from "@takumi-rs/helpers";
import type { EmojiType } from "@takumi-rs/helpers/emoji";
import type { PendingFont } from "@takumi-rs/helpers/renderer";
import type { Properties } from "csstype";

export {
  ContainerNode,
  ImageNode,
  Node,
  NodeMetadata,
  RgbaImage,
  TextNode,
} from "@takumi-rs/helpers";

export type ByteBuf = Uint8Array | ArrayBuffer | Buffer;

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
export type { CssInput };

/** Output format for static images. */
export type OutputFormat = "png" | "jpeg" | "webp" | "ico" | "raw";

/** Output format for animated images. */
export type AnimationOutputFormat = "webp" | "apng" | "gif";

/**
 * Gradient dithering algorithm.
 *
 * `"floyd-steinberg"` is a deprecated alias of `"ordered-bayer"`; it will be
 * removed in v3.
 */
export type DitheringAlgorithm = "none" | "ordered-bayer" | "floyd-steinberg";

/** Cache policy for a decoded image. Defaults to `"auto"`. */
export type ImageCacheMode = "auto" | "none";

export type RendererOptions = {
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
};

export type RenderOptions = {
  /**
   * The width of the image. If not provided, the width will be automatically calculated based on the content.
   */
  width?: number;
  /**
   * The height of the image. If not provided, the height will be automatically calculated based on the content.
   */
  height?: number;
  /**
   * The format of the image.
   * @default "png"
   */
  format?: OutputFormat;
  /**
   * The quality of lossy formats (0-100). For JPEG; on wasm, WebP is always
   * lossless so this is ignored for WebP.
   */
  quality?: number;
  /**
   * Encode WebP losslessly. On wasm, WebP is always lossless, so this is
   * accepted for parity with the native backend but has no effect.
   */
  lossless?: boolean;
  /**
   * Images keyed by `src`, each carrying raw bytes. Provided up front and used
   * in place of fetching external `src` URLs during rendering.
   */
  images?: ImageSource[];
  /**
   * CSS to apply before rendering: stylesheet text, or a rule written as an
   * object.
   */
  css?: CssInput[];
  /**
   * @deprecated Use `css` instead. Will be removed in v3.
   */
  stylesheets?: string[];
  /**
   * @deprecated Use a `{ keyframes, steps }` entry in `css` instead. Will be removed in v3.
   */
  keyframes?: Keyframes;
  /**
   * Whether to draw debug borders.
   */
  drawDebugBorder?: boolean;
  /**
   * Defines the ratio resolution of the image to the physical pixels.
   * @default 1.0
   */
  devicePixelRatio?: number;
  /**
   * The animation timeline time in milliseconds.
   */
  timeMs?: number;
  /**
   * Dithers gradient fills before they quantize to 8-bit.
   * @default "none"
   */
  dithering?: DitheringAlgorithm;
  /**
   * Per-render font stack: ordered family names used as the fallback chain.
   * Defaults to all registered families in registration order.
   */
  fontFamilies?: string[];
  /** Default BCP-47 language applied to the root, inherited by nodes without their own lang. */
  lang?: string;
  /**
   * Draws each emoji as an SVG image from this CDN, fetched as one of `images`. `from-font`, or no
   * value, draws emoji from the fonts.
   */
  emoji?: EmojiType | "from-font";
};

/**
 * SVG is a vector format, so the raster-only knobs do not apply.
 */
export type SvgRenderOptions = Omit<
  RenderOptions,
  "format" | "quality" | "lossless" | "drawDebugBorder" | "devicePixelRatio" | "dithering"
>;

export type RenderAnimationOptions = {
  scenes: AnimationScene[];
  width: number;
  height: number;
  format?: AnimationOutputFormat;
  /**
   * The quality of lossy WebP (0-100). Ignored for APNG and GIF; on wasm, WebP
   * is always lossless so this is ignored for WebP too.
   */
  quality?: number;
  /**
   * Encode WebP losslessly. On wasm, animated WebP is always lossless, so this
   * is accepted for parity with the native backend but has no effect.
   */
  lossless?: boolean;
  /**
   * Images keyed by `src`, each carrying raw bytes. Provided up front and used
   * in place of fetching external `src` URLs during rendering.
   */
  images?: ImageSource[];
  drawDebugBorder?: boolean;
  /**
   * CSS to apply before rendering: stylesheet text, or a rule written as an
   * object.
   */
  css?: CssInput[];
  /**
   * @deprecated Use `css` instead. Will be removed in v3.
   */
  stylesheets?: string[];
  /**
   * @deprecated Use a `{ keyframes, steps }` entry in `css` instead. Will be removed in v3.
   */
  keyframes?: Keyframes;
  /**
   * Defines the ratio resolution of the image to the physical pixels.
   * @default 1.0
   */
  devicePixelRatio?: number;
  /**
   * Frames per second for timeline sampling.
   */
  fps: number;
  /**
   * Per-render font stack: ordered family names used as the fallback chain.
   * Defaults to all registered families in registration order.
   */
  fontFamilies?: string[];
  /** Default BCP-47 language applied to the root, inherited by nodes without their own lang. */
  lang?: string;
  /**
   * Draws each emoji as an SVG image from this CDN, fetched as one of `images`. `from-font`, or no
   * value, draws emoji from the fonts.
   */
  emoji?: EmojiType | "from-font";
};

export type FontDetails = {
  name?: string;
  data: ByteBuf;
  weight?: number;
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
};

export type ImageSource = {
  src: string;
  data: ByteBuf;
  /** Cache policy for the decoded image. Defaults to `"auto"`. */
  cache?: ImageCacheMode;
};

export type KeyframeRule = {
  offsets: number[];
  declarations: Record<string, Properties>;
};

export type KeyframesRule = {
  name: string;
  keyframes: KeyframeRule[];
};

export type Font = FontDetails | ByteBuf;

export type RegisteredFace = {
  weight: number;
  style: string;
  width: number;
  index: number;
};

export type RegisteredFamily = {
  name: string;
  faces: RegisteredFace[];
};

export type MeasuredTextRun = {
  text: string;
  x: number;
  y: number;
  width: number;
  height: number;
};

export type MeasuredNode = {
  width: number;
  height: number;
  transform: [number, number, number, number, number, number];
  children: MeasuredNode[];
  runs: MeasuredTextRun[];
};

/** The images a node tree looks up by URL and the characters its text shapes, read after the cascade. */
export type RenderNeeds = {
  /** The URLs of image elements and computed background, mask, and list marker images, except `data:` URIs and inline SVG, sorted. */
  images: string[];
  /** Every character the text shapes once, sorted, generated content and `text-transform` included. */
  characters: string;
  /** The ids of the pending fonts the render would load, sorted. */
  faces: number[];
};

export type AnimationScene = {
  node: Node | string | { json: string };
  durationMs: number;
};
