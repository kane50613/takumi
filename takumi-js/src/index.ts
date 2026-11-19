export type {
  ContainerNode,
  FetchOptions,
  ImageNode,
  Node,
  NodeAttributes,
  NodeMetadata,
  RgbaImage,
  ReactElementLike,
  TextNode,
} from "@takumi-rs/helpers";
export type {
  AnimationOutputFormat,
  CacheMode,
  CacheOptions,
  CacheStats,
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
  FallbackFontsLoader,
  OutputFormat,
} from "@takumi-rs/core";
export { cacheStats } from "./cache";
export { setGlyphCacheMaxBytes } from "./glyph-cache";
export { measure, render, renderAnimation, renderSvg } from "./render";

export type {
  AnimationScene,
  ImagesInput,
  RenderAnimationOptions,
  RenderInput,
  RenderOptions,
  RenderSvgOptions,
} from "./render";
export type {
  AnimationRule,
  CssInput,
  Declarations,
  LayerRule,
  MediaRule,
  StyleRule,
  SupportsRule,
} from "@takumi-rs/helpers";

declare module "react" {
  interface DOMAttributes<T> {
    tw?: string;
  }
}
