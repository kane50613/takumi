export type {
  ContainerNode,
  FetchOptions,
  ImageNode,
  Node,
  NodeAttributes,
  NodeMetadata,
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
export { prepareImages, render, renderAnimation, renderSvg } from "./render";

export type {
  AnimationScene,
  ImageFetchCache,
  ImagesInput,
  PrepareImagesOptions,
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
