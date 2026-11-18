// Fresh declarations, not `export { … } from` specifiers, so the deprecation on the
// `@takumi-rs/helpers` exports does not follow these names into the packages that re-export them.
import * as helpers from "@takumi-rs/helpers";
import * as emoji from "@takumi-rs/helpers/emoji";
import * as jsx from "@takumi-rs/helpers/jsx";

export const container: typeof helpers.container = helpers.container;
export const text: typeof helpers.text = helpers.text;
export const image: typeof helpers.image = helpers.image;
export const fromJsx: typeof jsx.fromJsx = jsx.fromJsx;
export const Bitmap: typeof jsx.Bitmap = jsx.Bitmap;
export const googleFonts: typeof helpers.googleFonts = helpers.googleFonts;
export const fontFromUrl: typeof helpers.fontFromUrl = helpers.fontFromUrl;
export const extractEmojis: typeof emoji.extractEmojis = emoji.extractEmojis;

export type Node = helpers.Node;
export type ContainerNode = helpers.ContainerNode;
export type TextNode = helpers.TextNode;
export type ImageNode = helpers.ImageNode;
export type NodeMetadata = helpers.NodeMetadata;
export type NodeAttributes = helpers.NodeAttributes;
export type RgbaImage = helpers.RgbaImage;
export type Declarations = helpers.Declarations;
export type TextFit = helpers.TextFit;
export type ReactElementLike = helpers.ReactElementLike;

export type CssInput = helpers.CssInput;
export type StyleRule = helpers.StyleRule;
export type AnimationRule = helpers.AnimationRule;
export type MediaRule = helpers.MediaRule;
export type SupportsRule = helpers.SupportsRule;
export type LayerRule = helpers.LayerRule;

export type FromJsxOptions = jsx.FromJsxOptions;
export type FromJsxResult = jsx.FromJsxResult;
export type BitmapProps = jsx.BitmapProps;

export type GoogleFontsOptions = helpers.GoogleFontsOptions;
export type GoogleFontFamily = helpers.GoogleFontFamily;
export type GenericFontFamily = helpers.GenericFontFamily;
export type FontSubset = helpers.FontSubset;
export type EmojiType = emoji.EmojiType;

export type FetchOptions = helpers.FetchOptions;
export type FetchLike = helpers.FetchLike;
export type ImageFetchCache = helpers.ImageFetchCache;
