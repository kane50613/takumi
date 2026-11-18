import * as helpers from "@takumi-rs/helpers";

/** @deprecated Import `AnimationRule` from `takumi-js` instead. Will be removed in v3. */
export type AnimationRule = helpers.AnimationRule;

/** @deprecated Pass the fonts to a render, which picks the subsets its text uses. Will be removed in v3. */
export type CodepointSource = helpers.CodepointSource;

/** @deprecated Renderers pick font subsets from the text they lay out. Will be removed in v3. */
export const collectCodepoints: typeof helpers.collectCodepoints = helpers.collectCodepoints;

/** @deprecated Import `container` from `takumi-js` instead. Will be removed in v3. */
export const container: typeof helpers.container = helpers.container;

/** @deprecated Import `ContainerNode` from `takumi-js` instead. Will be removed in v3. */
export type ContainerNode = helpers.ContainerNode;

/** @deprecated Import `CssInput` from `takumi-js` instead. Will be removed in v3. */
export type CssInput = helpers.CssInput;

/** @deprecated Import `Declarations` from `takumi-js` instead. Will be removed in v3. */
export type Declarations = helpers.Declarations;

/** @deprecated Set the `maxBytes` fetch option explicitly instead. Will be removed in v3. */
export const defaultMaxFetchBytes: typeof helpers.defaultMaxFetchBytes =
  helpers.defaultMaxFetchBytes;

/** @deprecated Write the CSS string, such as `"1.5em"`, instead. Will be removed in v3. */
export const em: typeof helpers.em = helpers.em;

/** @deprecated Use a renderer's `prepare`, which reads the images after the cascade. Will be removed in v3. */
export type FetchedImage = helpers.FetchedImage;

/** @deprecated Import `FetchLike` from `@takumi-rs/core` or `@takumi-rs/wasm` instead. Will be removed in v3. */
export type FetchLike = helpers.FetchLike;

/** @deprecated Call `fetch` instead. Will be removed in v3. */
export const fetchOk: typeof helpers.fetchOk = helpers.fetchOk;

/** @deprecated Import `FetchOptions` from `takumi-js` instead. Will be removed in v3. */
export type FetchOptions = helpers.FetchOptions;

/** @deprecated Pass the URL string in the `fonts` option instead. Will be removed in v3. */
export const fontFromUrl: typeof helpers.fontFromUrl = helpers.fontFromUrl;

/** @deprecated Import `FontSubset` from `@takumi-rs/core` or `@takumi-rs/wasm` instead. Will be removed in v3. */
export type FontSubset = helpers.FontSubset;

/** @deprecated Write the CSS string, such as `"1fr"`, instead. Will be removed in v3. */
export const fr: typeof helpers.fr = helpers.fr;

/** @deprecated Import `GenericFontFamily` from `@takumi-rs/core` or `@takumi-rs/wasm` instead. Will be removed in v3. */
export type GenericFontFamily = helpers.GenericFontFamily;

/** @deprecated Import `GoogleFontFamily` from `takumi-js` instead. Will be removed in v3. */
export type GoogleFontFamily = helpers.GoogleFontFamily;

/** @deprecated Import `googleFonts` from `takumi-js` instead. Will be removed in v3. */
export const googleFonts: typeof helpers.googleFonts = helpers.googleFonts;

/** @deprecated Import `GoogleFontsOptions` from `takumi-js` instead. Will be removed in v3. */
export type GoogleFontsOptions = helpers.GoogleFontsOptions;

/** @deprecated Import `image` from `takumi-js` instead. Will be removed in v3. */
export const image: typeof helpers.image = helpers.image;

/** @deprecated Import `ImageFetchCache` from `@takumi-rs/core` or `@takumi-rs/wasm` instead. Will be removed in v3. */
export type ImageFetchCache = helpers.ImageFetchCache;

/** @deprecated Import `ImageNode` from `takumi-js` instead. Will be removed in v3. */
export type ImageNode = helpers.ImageNode;

/** @deprecated Import `LayerRule` from `takumi-js` instead. Will be removed in v3. */
export type LayerRule = helpers.LayerRule;

/** @deprecated Renderers read the characters markers generate themselves. Will be removed in v3. */
export const LIST_MARKER_CHARACTERS: typeof helpers.LIST_MARKER_CHARACTERS =
  helpers.LIST_MARKER_CHARACTERS;

/** @deprecated Import `MediaRule` from `takumi-js` instead. Will be removed in v3. */
export type MediaRule = helpers.MediaRule;

/** @deprecated Import `Node` from `takumi-js` instead. Will be removed in v3. */
export type Node = helpers.Node;

/** @deprecated Import `NodeAttributes` from `takumi-js` instead. Will be removed in v3. */
export type NodeAttributes = helpers.NodeAttributes;

/** @deprecated Import `NodeMetadata` from `takumi-js` instead. Will be removed in v3. */
export type NodeMetadata = helpers.NodeMetadata;

/** @deprecated Write the CSS string, such as `"50%"`, instead. Will be removed in v3. */
export const percentage: typeof helpers.percentage = helpers.percentage;

/** @deprecated Use a renderer's `prepare`, which reads the images after the cascade. Will be removed in v3. */
export const prepareImages: typeof helpers.prepareImages = helpers.prepareImages;

/** @deprecated Use a renderer's `prepare`, which reads the images after the cascade. Will be removed in v3. */
export type PrepareImagesOptions<T extends { src: string } = helpers.FetchedImage> =
  helpers.PrepareImagesOptions<T>;

/** @deprecated Import `ReactElementLike` from `takumi-js` instead. Will be removed in v3. */
export type ReactElementLike = helpers.ReactElementLike;

/** @deprecated Read the body with `response.arrayBuffer()` instead. Will be removed in v3. */
export const readBodyLimited: typeof helpers.readBodyLimited = helpers.readBodyLimited;

/** @deprecated Write the CSS string, such as `"1.5rem"`, instead. Will be removed in v3. */
export const rem: typeof helpers.rem = helpers.rem;

/** @deprecated Write the CSS string, such as `"rgb(0 0 0 / 0.5)"`, instead. Will be removed in v3. */
export const rgba: typeof helpers.rgba = helpers.rgba;

/** @deprecated Import `RgbaImage` from `takumi-js` instead. Will be removed in v3. */
export type RgbaImage = helpers.RgbaImage;

/** @deprecated Type the object as `CSSProperties` from React instead. Will be removed in v3. */
export const style: typeof helpers.style = helpers.style;

/** @deprecated Import `StyleRule` from `takumi-js` instead. Will be removed in v3. */
export type StyleRule = helpers.StyleRule;

/** @deprecated Pass the fonts to a render, which picks the subsets its text uses. Will be removed in v3. */
export const subsetFonts: typeof helpers.subsetFonts = helpers.subsetFonts;

/** @deprecated Import `SupportsRule` from `takumi-js` instead. Will be removed in v3. */
export type SupportsRule = helpers.SupportsRule;

/** @deprecated Import `text` from `takumi-js` instead. Will be removed in v3. */
export const text: typeof helpers.text = helpers.text;

/** @deprecated Import `TextFit` from `@takumi-rs/core` or `@takumi-rs/wasm` instead. Will be removed in v3. */
export type TextFit = helpers.TextFit;

/** @deprecated Import `TextNode` from `takumi-js` instead. Will be removed in v3. */
export type TextNode = helpers.TextNode;

/** @deprecated Write the CSS string, such as `"100vh"`, instead. Will be removed in v3. */
export const vh: typeof helpers.vh = helpers.vh;

/** @deprecated Write the CSS string, such as `"100vw"`, instead. Will be removed in v3. */
export const vw: typeof helpers.vw = helpers.vw;

export * from "./emoji";
export * from "./html";
export * from "./jsx";
