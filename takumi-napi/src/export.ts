import type {
  MeasuredNode,
  Node,
  RenderNeeds,
  RegisteredFamily,
  RenderAnimationOptions as RenderAnimationOptionsInternal,
  RenderOptions as RenderOptionsInternal,
  RendererOptions,
  SvgRenderOptions as SvgRenderOptionsInternal,
} from "../index";
export type * from "../index";
import { Renderer as RendererInternal } from "../index";

export { setGlyphCacheMaxBytes } from "../index";

import {
  FontRegistry,
  Prepared,
  prepareSource,
  renderPrepared,
  toNativeSource,
  withRenderInput,
} from "@takumi-rs/helpers/renderer";
import type {
  BackendAnimationOptions,
  BackendPreparedRenderOptions,
  BackendPrepareOptions,
  BackendRenderOptions,
  BackendSvgOptions,
  FontLoader,
  JsonSource,
  RegisteredFamilyLike,
} from "@takumi-rs/helpers/renderer";

export type {
  AnimationOutputFormatOptions,
  FontLoader,
  ImageLoader,
  ImagesInput,
  OutputFormatOptions,
  PrepareImagesInput,
} from "@takumi-rs/helpers/renderer";
export { Prepared } from "@takumi-rs/helpers/renderer";

export type RenderOptions = BackendRenderOptions<RenderOptionsInternal>;
export type RenderAnimationOptions = BackendAnimationOptions<RenderAnimationOptionsInternal>;
export type SvgRenderOptions = BackendSvgOptions<SvgRenderOptionsInternal>;
export type PrepareOptions = BackendPrepareOptions<RenderOptionsInternal>;
export type PreparedRenderOptions = BackendPreparedRenderOptions<RenderOptionsInternal>;

/** A source {@link Renderer.prepare} readied, with its fonts and images loaded. */
export type PreparedSource = Prepared<
  Node | string | JsonSource,
  Omit<RenderOptionsInternal, "images" | "fontFamilies">
>;

export class Renderer {
  private inner: RendererInternal;
  private fonts: FontRegistry<RegisteredFamily>;

  constructor(options?: RendererOptions) {
    this.inner = new RendererInternal(options);
    this.fonts = new FontRegistry(this.inner);
  }

  /**
   * Reads what `source` needs under `options`, then loads the font subsets its text uses and fetches
   * the images it draws, so {@link render}, {@link renderSvg}, and {@link measure} only lay it out
   * and encode it.
   */
  prepare(source: Node | string, options?: PrepareOptions): Promise<PreparedSource> {
    const { fonts, images, signal, fontFamilies, ...layout } = options ?? {};
    const native = toNativeSource(source);

    return prepareSource(this.fonts, native, layout, { fonts, images, signal, fontFamilies }, () =>
      this.inner.needs(native, layout, signal),
    );
  }

  render(source: PreparedSource, options?: PreparedRenderOptions): Promise<Buffer<ArrayBuffer>>;
  render(source: Node | string, options?: RenderOptions): Promise<Buffer<ArrayBuffer>>;
  render(
    source: PreparedSource | Node | string,
    options?: RenderOptions | PreparedRenderOptions,
  ): Promise<Buffer<ArrayBuffer>> {
    if (source instanceof Prepared) {
      return renderPrepared(this.fonts, source, options ?? {}, (native, opts, signal) =>
        this.inner.render(native, opts, signal),
      );
    }

    return withRenderInput(
      this.fonts,
      options ?? {},
      source,
      (opts, signal) => this.inner.render(toNativeSource(source), opts, signal),
      this.characters(source),
    );
  }

  renderSvg(source: PreparedSource, options?: { signal?: AbortSignal }): Promise<string>;
  renderSvg(source: Node | string, options?: SvgRenderOptions): Promise<string>;
  renderSvg(
    source: PreparedSource | Node | string,
    options?: SvgRenderOptions | { signal?: AbortSignal },
  ): Promise<string> {
    if (source instanceof Prepared) {
      return renderPrepared(this.fonts, source, options ?? {}, (native, opts, signal) =>
        this.inner.renderSvg(native, opts, signal),
      );
    }

    return withRenderInput(
      this.fonts,
      options ?? {},
      source,
      (opts, signal) => this.inner.renderSvg(toNativeSource(source), opts, signal),
      this.characters(source),
    );
  }

  measure(source: PreparedSource, options?: { signal?: AbortSignal }): Promise<MeasuredNode>;
  measure(source: Node | string, options?: RenderOptions): Promise<MeasuredNode>;
  measure(
    source: PreparedSource | Node | string,
    options?: RenderOptions | { signal?: AbortSignal },
  ): Promise<MeasuredNode> {
    if (source instanceof Prepared) {
      return renderPrepared(this.fonts, source, options ?? {}, (native, opts, signal) =>
        this.inner.measure(native, opts, signal),
      );
    }

    return withRenderInput(
      this.fonts,
      options ?? {},
      source,
      (opts, signal) => this.inner.measure(toNativeSource(source), opts, signal),
      this.characters(source),
    );
  }

  /**
   * The images `source` looks up by URL and the characters its text shapes, read after the cascade.
   *
   * @internal For `takumi-js`, which fetches the images before rendering. Not a stable API.
   */
  needs(source: Node | string, options?: RenderOptions): Promise<RenderNeeds> {
    const { fonts: _fonts, images: _images, signal, ...rest } = options ?? {};

    return this.inner.needs(toNativeSource(source), rest, signal);
  }

  /** Reads the characters `source` shapes, for `withRenderInput` to pick font subsets by. */
  private characters(source: Node | string) {
    return async (options: RenderOptionsInternal) =>
      (await this.inner.needs(toNativeSource(source), options)).characters;
  }

  renderAnimation(options: RenderAnimationOptions): Promise<Buffer<ArrayBuffer>> {
    const nodes = options.scenes.map((scene) => scene.node);

    return withRenderInput(this.fonts, options, nodes, (opts, signal) =>
      this.inner.renderAnimation(opts, signal),
    );
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
}
