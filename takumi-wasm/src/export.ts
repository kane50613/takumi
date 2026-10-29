import {
  Renderer as RendererInternal,
  type MeasuredNode,
  type Node,
  type RegisteredFamily,
  type RendererOptions,
  type RenderAnimationOptions as RenderAnimationOptionsInternal,
  type RenderOptions as RenderOptionsInternal,
  type SvgRenderOptions as SvgRenderOptionsInternal,
} from "../pkg/takumi_wasm";

export * from "../pkg/takumi_wasm";
export { default } from "../pkg/takumi_wasm";

import {
  FontRegistry,
  Prepared,
  prepareSource,
  needsArgs,
  readSource,
  renderPrepared,
  toNativeSource,
} from "@takumi-rs/helpers/renderer";
import type {
  BackendAnimationOptions,
  BackendPrepareAnimationOptions,
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
export type PrepareAnimationOptions =
  BackendPrepareAnimationOptions<RenderAnimationOptionsInternal>;

/** A source {@link Renderer.prepare} readied, with its fonts and images loaded. */
export type PreparedSource = Prepared<
  Node | string | JsonSource,
  Omit<RenderOptionsInternal, "images" | "fontFamilies">
>;

/** An animation {@link Renderer.prepareAnimation} readied, with its fonts and images loaded. */
export type PreparedAnimation = Prepared<
  RenderAnimationOptionsInternal["scenes"],
  Omit<RenderAnimationOptionsInternal, "images" | "fontFamilies" | "scenes">
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

    return prepareSource(
      this.fonts,
      native,
      layout,
      { fonts, images, signal, fontFamilies },
      (request) => this.inner.needs(native, ...needsArgs(layout, request)),
    );
  }

  render(source: PreparedSource, options?: PreparedRenderOptions): Promise<Uint8Array<ArrayBuffer>>;
  render(source: Node | string, options?: RenderOptions): Promise<Uint8Array<ArrayBuffer>>;
  async render(
    source: PreparedSource | Node | string,
    options: RenderOptions = {},
  ): Promise<Uint8Array<ArrayBuffer>> {
    return renderPrepared(
      this.fonts,
      source instanceof Prepared ? source : await this.read(source, options),
      source instanceof Prepared ? options : { signal: options.signal },
      (native, opts) => this.inner.render(native, opts),
    );
  }

  renderAsDataUrl(source: PreparedSource, options?: PreparedRenderOptions): Promise<string>;
  renderAsDataUrl(source: Node | string, options?: RenderOptions): Promise<string>;
  async renderAsDataUrl(
    source: PreparedSource | Node | string,
    options: RenderOptions = {},
  ): Promise<string> {
    return renderPrepared(
      this.fonts,
      source instanceof Prepared ? source : await this.read(source, options),
      source instanceof Prepared ? options : { signal: options.signal },
      (native, opts) => this.inner.renderAsDataUrl(native, opts),
    );
  }

  renderSvg(source: PreparedSource, options?: { signal?: AbortSignal }): Promise<string>;
  renderSvg(source: Node | string, options?: SvgRenderOptions): Promise<string>;
  async renderSvg(
    source: PreparedSource | Node | string,
    options: SvgRenderOptions = {},
  ): Promise<string> {
    return renderPrepared(
      this.fonts,
      source instanceof Prepared ? source : await this.read(source, options),
      source instanceof Prepared ? options : { signal: options.signal },
      (native, opts) => this.inner.renderSvg(native, opts),
    );
  }

  measure(source: PreparedSource, options?: { signal?: AbortSignal }): Promise<MeasuredNode>;
  measure(source: Node | string, options?: RenderOptions): Promise<MeasuredNode>;
  async measure(
    source: PreparedSource | Node | string,
    options: RenderOptions = {},
  ): Promise<MeasuredNode> {
    return renderPrepared(
      this.fonts,
      source instanceof Prepared ? source : await this.read(source, options),
      source instanceof Prepared ? options : { signal: options.signal },
      (native, opts) => this.inner.measure(native, opts),
    );
  }

  /** `source` read under `options` without fetching, for {@link render} and its siblings. */
  private read(
    source: Node | string,
    options: RenderOptions | SvgRenderOptions,
  ): Promise<PreparedSource> {
    const { fonts, images, signal, fontFamilies, ...layout } = options;
    const native = toNativeSource(source);

    return readSource(
      this.fonts,
      native,
      layout,
      { fonts, images, signal, fontFamilies },
      (request) => this.inner.needs(native, ...needsArgs(layout, request)),
    );
  }

  /**
   * Reads what every scene of an animation needs, then loads the font subsets their text uses and
   * fetches the images they draw, so {@link renderAnimation} only lays the scenes out and encodes
   * them.
   */
  prepareAnimation(options: PrepareAnimationOptions): Promise<PreparedAnimation> {
    return this.readAnimation(options, prepareSource);
  }

  renderAnimation(
    source: PreparedAnimation,
    options?: { signal?: AbortSignal },
  ): Promise<Uint8Array<ArrayBuffer>>;
  renderAnimation(options: RenderAnimationOptions): Promise<Uint8Array<ArrayBuffer>>;
  async renderAnimation(
    source: PreparedAnimation | RenderAnimationOptions,
    options: { signal?: AbortSignal } = {},
  ): Promise<Uint8Array<ArrayBuffer>> {
    const prepared =
      source instanceof Prepared ? source : await this.readAnimation(source, readSource);

    return renderPrepared(
      this.fonts,
      prepared,
      { signal: source instanceof Prepared ? options.signal : source.signal },
      (scenes, opts) => this.inner.renderAnimation({ ...opts, scenes }),
    );
  }

  /** The scenes of `options` read with `collect`, which fetches their images when it prepares them. */
  private readAnimation(
    options: PrepareAnimationOptions | RenderAnimationOptions,
    collect: typeof prepareSource,
  ): Promise<PreparedAnimation> {
    const { fonts, images, signal, fontFamilies, scenes, ...layout } = options;
    const sources = scenes.map((scene) => ({ ...scene, node: toNativeSource(scene.node) }));

    return collect(
      this.fonts,
      sources,
      layout,
      { fonts, images, signal, fontFamilies },
      (request) => this.inner.animationNeeds(...needsArgs({ ...layout, scenes: sources }, request)),
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

  /** Releases the underlying wasm renderer's memory. */
  free(): void {
    this.inner.free();
  }
}
