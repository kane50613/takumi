export type {
  ContainerNode,
  FetchResourcesOptions,
  ImageNode,
  Node,
  NodeAttributes,
  NodeMetadata,
  ReactElementLike,
  TextNode,
} from "@takumi-rs/helpers";
export type {
  AnimationFrameSource,
  AnimationOutputFormat,
  AnimationSceneSource,
  DitheringAlgorithm,
  EncodeFramesOptions,
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
export { render, renderAnimation, renderSvg } from "./render";

export type {
  RenderAnimationOptions,
  RenderAnimationScene,
  RenderInput,
  RenderOptions,
  RenderSvgOptions,
} from "./render";

declare module "react" {
  interface DOMAttributes<T> {
    tw?: string;
  }
}
