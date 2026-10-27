import {
  Renderer as RendererInternal,
  type MeasuredNode,
  type Node,
  type RegisteredFamily,
  type RenderNeeds,
  type RendererOptions,
  type RenderAnimationOptions as RenderAnimationOptionsInternal,
  type RenderOptions as RenderOptionsInternal,
  type SvgRenderOptions as SvgRenderOptionsInternal,
} from "../pkg/takumi_wasm";

export * from "../pkg/takumi_wasm";
export { default } from "../pkg/takumi_wasm";

import { FontRegistry, toNativeSource, withRenderInput } from "@takumi-rs/helpers/renderer";
import type {
  BackendAnimationOptions,
  BackendRenderOptions,
  BackendSvgOptions,
  FontLoader,
  RegisteredFamilyLike,
} from "@takumi-rs/helpers/renderer";

export type {
  AnimationOutputFormatOptions,
  FontLoader,
  ImageLoader,
  ImagesInput,
  OutputFormatOptions,
} from "@takumi-rs/helpers/renderer";

export type RenderOptions = BackendRenderOptions<RenderOptionsInternal>;
export type RenderAnimationOptions = BackendAnimationOptions<RenderAnimationOptionsInternal>;
export type SvgRenderOptions = BackendSvgOptions<SvgRenderOptionsInternal>;

export class Renderer {
  private inner: RendererInternal;
  private fonts: FontRegistry<RegisteredFamily>;

  constructor(options?: RendererOptions) {
    this.inner = new RendererInternal(options);
    this.fonts = new FontRegistry(this.inner);
  }

  render(source: Node | string, options?: RenderOptions): Promise<Uint8Array<ArrayBuffer>> {
    return withRenderInput(
      this.fonts,
      options ?? {},
      source,
      (opts) => this.inner.render(toNativeSource(source), opts),
      this.characters(source),
    );
  }

  renderAsDataUrl(source: Node | string, options?: RenderOptions): Promise<string> {
    return withRenderInput(
      this.fonts,
      options ?? {},
      source,
      (opts) => this.inner.renderAsDataUrl(toNativeSource(source), opts),
      this.characters(source),
    );
  }

  renderSvg(source: Node | string, options?: SvgRenderOptions): Promise<string> {
    return withRenderInput(
      this.fonts,
      options ?? {},
      source,
      (opts) => this.inner.renderSvg(toNativeSource(source), opts),
      this.characters(source),
    );
  }

  measure(source: Node | string, options?: RenderOptions): Promise<MeasuredNode> {
    return withRenderInput(
      this.fonts,
      options ?? {},
      source,
      (opts) => this.inner.measure(toNativeSource(source), opts),
      this.characters(source),
    );
  }

  /** The remote images `source` draws and the characters its text shapes, read after the cascade. */
  async needs(source: Node | string, options?: RenderOptionsInternal): Promise<RenderNeeds> {
    return this.inner.needs(toNativeSource(source), options);
  }

  /** Reads the characters `source` shapes, for `withRenderInput` to pick font subsets by. */
  private characters(source: Node | string) {
    return async (options: RenderOptionsInternal) =>
      (await this.inner.needs(toNativeSource(source), options)).characters;
  }

  renderAnimation(options: RenderAnimationOptions): Promise<Uint8Array<ArrayBuffer>> {
    const nodes = options.scenes.map((scene) => scene.node);

    return withRenderInput(this.fonts, options, nodes, (opts) => this.inner.renderAnimation(opts));
  }

  registerFont(font: FontLoader): Promise<RegisteredFamily[]> {
    return this.fonts.register(font);
  }

  /**
   * Removes every face registered under the family `name`, from any `registerFont` call and each
   * `unicodeRange` subset alike; returns whether it had any. Takes a family `registerFont` returned
   * as well. A later render that lists one of its fonts registers it again.
   */
  unregisterFont(family: RegisteredFamilyLike): boolean {
    return this.inner.unregisterFont(family.name);
  }

  /** Removes every registered font, leaving the bundled fallback font. */
  clearFonts(): void {
    this.inner.clearFonts();
  }

  /** Releases the underlying wasm renderer's memory. */
  free(): void {
    this.inner.free();
  }
}
