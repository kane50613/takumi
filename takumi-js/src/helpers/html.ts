import * as html from "@takumi-rs/helpers/html";

/** @deprecated `render` and `measure` take HTML. Import `fromHtml` from `@takumi-rs/helpers/html` for a renderer binding. Will be removed in v3. */
export const fromHtml: typeof html.fromHtml = html.fromHtml;

/** @deprecated Import `FromHtmlResult` from `@takumi-rs/helpers/html` instead. Will be removed in v3. */
export type FromHtmlResult = html.FromHtmlResult;
