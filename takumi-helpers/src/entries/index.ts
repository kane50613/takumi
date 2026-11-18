import * as fetching from "../fetch";
import * as fonts from "../fonts";
import * as nodes from "../helpers";
import type * as images from "../images";
import type * as types from "../types";

export { em, fr, percentage, rem, rgba, style, vh, vw } from "../helpers";
export {
  collectCodepoints,
  type CodepointSource,
  LIST_MARKER_CHARACTERS,
  subsetFonts,
} from "../fonts";
export { prepareImages } from "../images";

/** @deprecated Set the `maxBytes` fetch option explicitly instead. Will be removed in v3. */
export const defaultMaxFetchBytes: number = fetching.defaultMaxFetchBytes;

/** @deprecated Call `fetch` instead. Will be removed in v3. */
export const fetchOk: typeof fetching.fetchOk = fetching.fetchOk;

/** @deprecated Read the body with `response.arrayBuffer()` instead. Will be removed in v3. */
export const readBodyLimited: typeof fetching.readBodyLimited = fetching.readBodyLimited;

/** @deprecated Import from takumi-js, @takumi-rs/core, or @takumi-rs/wasm instead. @takumi-rs/helpers becomes internal in v3. */
export type AnimationRule = types.AnimationRule;

/** @deprecated Import from takumi-js, @takumi-rs/core, or @takumi-rs/wasm instead. @takumi-rs/helpers becomes internal in v3. */
export const container: typeof nodes.container = nodes.container;

/** @deprecated Import from takumi-js, @takumi-rs/core, or @takumi-rs/wasm instead. @takumi-rs/helpers becomes internal in v3. */
export type ContainerNode = types.ContainerNode;

/** @deprecated Import from takumi-js, @takumi-rs/core, or @takumi-rs/wasm instead. @takumi-rs/helpers becomes internal in v3. */
export type CssInput = types.CssInput;

/** @deprecated Import from takumi-js, @takumi-rs/core, or @takumi-rs/wasm instead. @takumi-rs/helpers becomes internal in v3. */
export type Declarations = types.Declarations;

/** @deprecated Takumi uses this internally. @takumi-rs/helpers becomes internal in v3. */
export type FetchedImage = images.FetchedImage;

/** @deprecated Import from takumi-js, @takumi-rs/core, or @takumi-rs/wasm instead. @takumi-rs/helpers becomes internal in v3. */
export type FetchLike = fetching.FetchLike;

/** @deprecated Import from takumi-js, @takumi-rs/core, or @takumi-rs/wasm instead. @takumi-rs/helpers becomes internal in v3. */
export type FetchOptions = fetching.FetchOptions;

/** @deprecated Import from takumi-js, @takumi-rs/core, or @takumi-rs/wasm instead. @takumi-rs/helpers becomes internal in v3. */
export const fontFromUrl: typeof fonts.fontFromUrl = fonts.fontFromUrl;

/** @deprecated Import from takumi-js, @takumi-rs/core, or @takumi-rs/wasm instead. @takumi-rs/helpers becomes internal in v3. */
export type FontSubset = fonts.FontSubset;

/** @deprecated Import from takumi-js, @takumi-rs/core, or @takumi-rs/wasm instead. @takumi-rs/helpers becomes internal in v3. */
export type GenericFontFamily = fonts.GenericFontFamily;

/** @deprecated Import from takumi-js, @takumi-rs/core, or @takumi-rs/wasm instead. @takumi-rs/helpers becomes internal in v3. */
export type GoogleFontFamily = fonts.GoogleFontFamily;

/** @deprecated Import from takumi-js, @takumi-rs/core, or @takumi-rs/wasm instead. @takumi-rs/helpers becomes internal in v3. */
export const googleFonts: typeof fonts.googleFonts = fonts.googleFonts;

/** @deprecated Import from takumi-js, @takumi-rs/core, or @takumi-rs/wasm instead. @takumi-rs/helpers becomes internal in v3. */
export type GoogleFontsOptions = fonts.GoogleFontsOptions;

/** @deprecated Import from takumi-js, @takumi-rs/core, or @takumi-rs/wasm instead. @takumi-rs/helpers becomes internal in v3. */
export const image: typeof nodes.image = nodes.image;

/** @deprecated Import from takumi-js, @takumi-rs/core, or @takumi-rs/wasm instead. @takumi-rs/helpers becomes internal in v3. */
export type ImageFetchCache = images.ImageFetchCache;

/** @deprecated Import from takumi-js, @takumi-rs/core, or @takumi-rs/wasm instead. @takumi-rs/helpers becomes internal in v3. */
export type ImageNode = types.ImageNode;

/** @deprecated Import from takumi-js, @takumi-rs/core, or @takumi-rs/wasm instead. @takumi-rs/helpers becomes internal in v3. */
export type LayerRule = types.LayerRule;

/** @deprecated Import from takumi-js, @takumi-rs/core, or @takumi-rs/wasm instead. @takumi-rs/helpers becomes internal in v3. */
export type MediaRule = types.MediaRule;

/** @deprecated Import from takumi-js, @takumi-rs/core, or @takumi-rs/wasm instead. @takumi-rs/helpers becomes internal in v3. */
export type Node = types.Node;

/** @deprecated Import from takumi-js, @takumi-rs/core, or @takumi-rs/wasm instead. @takumi-rs/helpers becomes internal in v3. */
export type NodeAttributes = types.NodeAttributes;

/** @deprecated Import from takumi-js, @takumi-rs/core, or @takumi-rs/wasm instead. @takumi-rs/helpers becomes internal in v3. */
export type NodeMetadata = types.NodeMetadata;

/** @deprecated Takumi uses this internally. @takumi-rs/helpers becomes internal in v3. */
export type PrepareImagesOptions<T extends { src: string } = images.FetchedImage> =
  images.PrepareImagesOptions<T>;

/** @deprecated Import from takumi-js, @takumi-rs/core, or @takumi-rs/wasm instead. @takumi-rs/helpers becomes internal in v3. */
export type ReactElementLike = types.ReactElementLike;

/** @deprecated Import from takumi-js, @takumi-rs/core, or @takumi-rs/wasm instead. @takumi-rs/helpers becomes internal in v3. */
export type RgbaImage = types.RgbaImage;

/** @deprecated Import from takumi-js, @takumi-rs/core, or @takumi-rs/wasm instead. @takumi-rs/helpers becomes internal in v3. */
export type StyleRule = types.StyleRule;

/** @deprecated Import from takumi-js, @takumi-rs/core, or @takumi-rs/wasm instead. @takumi-rs/helpers becomes internal in v3. */
export type SupportsRule = types.SupportsRule;

/** @deprecated Import from takumi-js, @takumi-rs/core, or @takumi-rs/wasm instead. @takumi-rs/helpers becomes internal in v3. */
export const text: typeof nodes.text = nodes.text;

/** @deprecated Import from takumi-js, @takumi-rs/core, or @takumi-rs/wasm instead. @takumi-rs/helpers becomes internal in v3. */
export type TextFit = types.TextFit;

/** @deprecated Import from takumi-js, @takumi-rs/core, or @takumi-rs/wasm instead. @takumi-rs/helpers becomes internal in v3. */
export type TextNode = types.TextNode;
