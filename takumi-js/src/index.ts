export type {
  ContainerNode,
  FailedImage,
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
  CacheBudgets,
  CacheOptions,
  DitheringAlgorithm,
  FailedFont,
  Font,
  FontDetails,
  FontLoader,
  ImageSource,
  Keyframes,
  KeyframesMap,
  KeyframesRuleList,
  MeasuredNode,
  MeasuredTextRun,
  MissingGlyphsHandler,
  OutputFormat,
  RenderReport,
} from "@takumi-rs/core";
export { cacheStats, setCacheOptions, setGlyphCacheMaxBytes } from "./cache";
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
