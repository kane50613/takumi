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
export { render, renderAnimation, renderSvg } from "./render";

export type {
  AnimationScene,
  ManagedImagesInput,
  RenderAnimationOptions,
  RenderInput,
  RenderOptions,
  RenderSvgOptions,
} from "./render";
export type { ImagesInput } from "@takumi-rs/helpers/renderer";

declare module "react" {
  interface DOMAttributes<T> {
    tw?: string;
  }
}
