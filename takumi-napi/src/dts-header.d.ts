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
