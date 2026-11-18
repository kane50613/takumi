export type {
  AnimationRule,
  BitmapProps,
  ContainerNode,
  CssInput,
  Declarations,
  FetchOptions,
  GoogleFontFamily,
  GoogleFontsOptions,
  ImageNode,
  LayerRule,
  MediaRule,
  Node,
  NodeAttributes,
  NodeMetadata,
  RgbaImage,
  ReactElementLike,
  StyleRule,
  SupportsRule,
  TextNode,
} from "@takumi-rs/bindings-js/helpers";
export { Bitmap, container, googleFonts, image, text } from "@takumi-rs/bindings-js/helpers";
export type {
  AnimationOutputFormat,
  DitheringAlgorithm,
  Font,
  FontDetails,
  FontLoader,
  ImageSource,
  Keyframes,
  KeyframesMap,
  KeyframesRuleList,
  MeasuredNode,
  MeasuredTextRun,
  OutputFormat,
} from "@takumi-rs/core";
export { setGlyphCacheMaxBytes } from "./glyph-cache";
export { measure, render, renderAnimation, renderSvg } from "./render";

export type {
  AnimationScene,
  DocumentInput,
  ImagesInput,
  RenderAnimationOptions,
  RenderInput,
  RenderOptions,
  RenderSvgOptions,
} from "./render";

declare module "react" {
  interface DOMAttributes<T> {
    tw?: string;
  }
}
