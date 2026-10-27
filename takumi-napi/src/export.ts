import type {
  HtmlSource,
  MeasuredNode,
  Node,
  RegisteredFamily,
  RenderAnimationOptions as RenderAnimationOptionsInternal,
  RenderOptions as RenderOptionsInternal,
  RendererOptions,
  SvgRenderOptions as SvgRenderOptionsInternal,
} from "../index";
export type * from "../index";
import { Renderer as RendererInternal } from "../index";

export { setGlyphCacheMaxBytes } from "../index";

import { FontRegistry, withRenderInput } from "@takumi-rs/helpers/renderer";
import { toNativeSource } from "./node-source";
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

  render(source: Node | HtmlSource, options?: RenderOptions): Promise<Buffer<ArrayBuffer>> {
    return withRenderInput(this.fonts, options ?? {}, source, (opts, signal) =>
      this.inner.render(toNativeSource(source), opts, signal),
    );
  }

  renderSvg(source: Node | HtmlSource, options?: SvgRenderOptions): Promise<string> {
    return withRenderInput(this.fonts, options ?? {}, source, (opts, signal) =>
      this.inner.renderSvg(toNativeSource(source), opts, signal),
    );
  }

  measure(source: Node | HtmlSource, options?: RenderOptions): Promise<MeasuredNode> {
    return withRenderInput(this.fonts, options ?? {}, source, (opts, signal) =>
      this.inner.measure(toNativeSource(source), opts, signal),
    );
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
