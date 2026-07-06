import type { Node } from "@takumi-rs/helpers";

export type { ContainerNode, ImageNode, NodeMetadata, TextNode } from "@takumi-rs/helpers";

export type { Node };

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
}

export type Font = FontDetails | ByteBuf;

export type KeyframesMap = Record<string, Record<string, CSSStyleDeclaration>>;
export type KeyframesRuleList = {
  name: string;
  keyframes: {
    offsets: number[];
    declarations: Record<string, CSSStyleDeclaration>;
  }[];
}[];
export type Keyframes = KeyframesMap | KeyframesRuleList;
