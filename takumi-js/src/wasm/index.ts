import autoModule from "@takumi-rs/wasm/auto";
import {
  render as renderManaged,
  renderAnimation as renderAnimationManaged,
  renderSvg as renderSvgManaged,
  type RenderAnimationOptions,
  type RenderInput,
  type RenderOptions,
  type RenderSvgOptions,
} from "../render";

export { default } from "@takumi-rs/wasm/auto";
export { default as init } from "@takumi-rs/wasm";
export * from "@takumi-rs/wasm";

export type { RenderAnimationOptions, RenderInput, RenderOptions, RenderSvgOptions };

type ManagedOptions = RenderOptions | RenderSvgOptions | RenderAnimationOptions;

/** Whether the caller already picked a backend, via `renderer` or `module`. */
function hasBackend(options?: ManagedOptions): boolean {
  return (
    !!options &&
    (("renderer" in options && !!options.renderer) || ("module" in options && !!options.module))
  );
}

/**
 * {@link renderManaged | render} pinned to the WASM backend: never resolves the
 * native addon, and loads the binary `@takumi-rs/wasm/auto` picks for the host
 * bundler. Same managed pipeline as the main entry: shared renderer, `fonts`
 * dedupe, image fetching, emoji extraction.
 *
 * @example
 * ```tsx
 * import { render } from "takumi-js/wasm";
 *
 * const png = await render(<div tw="p-4">Hello</div>, { width: 1200, height: 630 });
 * ```
 */
export function render(element: RenderInput, options?: RenderOptions) {
  return renderManaged(element, hasBackend(options) ? options : { ...options, module: autoModule });
}

/** {@link renderSvgManaged | renderSvg} pinned to the WASM backend. */
export function renderSvg(element: RenderInput, options?: RenderSvgOptions) {
  return renderSvgManaged(
    element,
    hasBackend(options) ? options : { ...options, module: autoModule },
  );
}

/** {@link renderAnimationManaged | renderAnimation} pinned to the WASM backend. */
export function renderAnimation(options: RenderAnimationOptions) {
  return renderAnimationManaged(hasBackend(options) ? options : { ...options, module: autoModule });
}
