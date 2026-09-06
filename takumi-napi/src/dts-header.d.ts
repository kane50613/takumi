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
   * Logical family this font is a coverage subset of. Subsets sharing a `subsetOf` are
   * kept as distinct families and `font-family: {subsetOf}` expands to all of them, so each
   * script routes to the subset that covers it. Set by {@link loadGoogleFonts}.
   */
  subsetOf?: string;
  /**
   * Where this subset sits in its group's fallback order; lowest is tried first, and equal
   * ranks order by family name. A subset's `cmap` reaches past the range it was cut for, so
   * the rank is what settles which subset serves a codepoint several of them encode.
   */
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
