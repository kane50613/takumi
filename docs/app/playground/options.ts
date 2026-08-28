import type { Keyframes } from "takumi-js";
import type { CssEntry } from "./schema";
import type { RenderOptions } from "takumi-pdf";

// `Omit` collapses a union, which would lose the paged/viewport split that makes
// the PDF options self-documenting in the editor.
type DistributiveOmit<T, K extends PropertyKey> = T extends unknown ? Omit<T, K> : never;

/** PDF options the playground forwards; it supplies fonts, images and CSS itself. */
export type PlaygroundPdfOptions = DistributiveOmit<
  RenderOptions,
  "fonts" | "images" | "css" | "fontFamilies"
>;

declare global {
  type PlaygroundOptions = {
    /**
     * @description width of the render viewport.
     */
    width?: number;
    /**
     * @description height of the render viewport.
     */
    height?: number;
    /**
     * @description format to render.
     * @default png
     */
    format?: "png" | "jpeg" | "webp" | "ico";
    /**
     * @description quality of jpeg format (0-100).
     * @default 75
     */
    quality?: number;
    /**
     * @description device pixel ratio.
     * @default 1.0
     */
    devicePixelRatio?: number;
    /**
     * @description CSS applied before rendering: stylesheet text, or a rule written as an object. A `{ selector: ":root", style: { "--color-brand": "#7c3aed" } }` entry makes `bg-brand` resolve.
     */
    css?: CssEntry | CssEntry[];
    /**
     * @deprecated use a `{ keyframes, steps }` entry in `css`. Removed in v3.
     * @description structured keyframes registered alongside the CSS.
     */
    keyframes?: Keyframes;
    /**
     * @description timeline animation output. When present, the playground renders an animated image instead of a single frame.
     */
    animation?: {
      /**
       * @description total timeline duration in milliseconds.
       */
      durationMs: number;
      /**
       * @description frames per second used to sample keyframes.
       * @default 30
       */
      fps?: number;
      /**
       * @description animation output format.
       * @default webp
       */
      format?: "webp" | "apng" | "gif";
    };
    /**
     * @description PDF output. When present, the playground renders a paged PDF instead of an image, and `width`/`height` are ignored. Pass `{}` for A4 defaults.
     */
    pdf?: PlaygroundPdfOptions;
    /**
     * @description emoji style to use.
     * @default twemoji
     */
    emoji?: "twemoji" | "blobmoji" | "noto" | "openmoji";
  };
}
