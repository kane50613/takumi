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
export { render, renderAnimation, renderSvg } from "./render";

export type {
  AnimationScene,
  ImagesInput,
  RenderAnimationOptions,
  RenderInput,
  RenderOptions,
  RenderSvgOptions,
} from "./render";
export type { AnimationRule, CssInput, Declarations, StyleRule } from "@takumi-rs/helpers";

declare module "react" {
  interface DOMAttributes<T> {
    tw?: string;
  }
}
