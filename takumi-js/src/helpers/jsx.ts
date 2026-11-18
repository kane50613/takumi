import * as jsx from "@takumi-rs/helpers/jsx";

/** @deprecated Import `Bitmap` from `takumi-js` instead. Will be removed in v3. */
export const Bitmap: typeof jsx.Bitmap = jsx.Bitmap;

/** @deprecated Import `BitmapProps` from `takumi-js` instead. Will be removed in v3. */
export type BitmapProps = jsx.BitmapProps;

/** @deprecated Will be removed in v3. */
export const defaultStylePresets: typeof jsx.defaultStylePresets = jsx.defaultStylePresets;

/** @deprecated `render` and `measure` take JSX. Import `fromJsx` from `@takumi-rs/core` or `@takumi-rs/wasm` for a renderer binding. Will be removed in v3. */
export const fromJsx: typeof jsx.fromJsx = jsx.fromJsx;

/** @deprecated Import `FromJsxOptions` from `@takumi-rs/core` or `@takumi-rs/wasm` instead. Will be removed in v3. */
export type FromJsxOptions = jsx.FromJsxOptions;

/** @deprecated Import `FromJsxResult` from `@takumi-rs/core` or `@takumi-rs/wasm` instead. Will be removed in v3. */
export type FromJsxResult = jsx.FromJsxResult;

/** @deprecated Will be removed in v3. */
export type HtmlProps = jsx.HtmlProps;
