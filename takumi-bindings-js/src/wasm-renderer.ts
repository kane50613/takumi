import {
  type FontBackend,
  type FontLoader,
  FontRegistry,
  type RegisteredFamilyLike,
} from "./renderer";
import type { CssInput } from "@takumi-rs/helpers";

/**
 * A wasm binding renderer whose fonts a {@link FontRegistry} loads.
 *
 * @internal
 */
export type WasmBinding<TFamily extends RegisteredFamilyLike> = FontBackend<TFamily> & {
  unregisterFont(family: string): boolean;
  clearFonts(): void;
  free(): void;
};

/**
 * The fonts and memory of a wasm document renderer, such as a PDF renderer or a painter.
 *
 * @internal
 */
export class WasmRenderer<
  TFamily extends RegisteredFamilyLike,
  TInner extends WasmBinding<TFamily>,
> {
  protected readonly inner: TInner;
  protected readonly fonts: FontRegistry<TFamily>;

  constructor(inner: TInner) {
    this.inner = inner;
    this.fonts = new FontRegistry(inner);
  }

  /** Registers a font ahead of time, deduped against earlier registrations. */
  registerFont(font: FontLoader): Promise<TFamily[]> {
    return this.fonts.register(font);
  }

  /** Removes every face registered under the family `name`, returning whether it had any. */
  unregisterFont(family: RegisteredFamilyLike): boolean {
    return this.inner.unregisterFont(family.name);
  }

  /** Removes every registered font, leaving the bundled fallback font. */
  clearFonts(): void {
    this.inner.clearFonts();
  }

  /** Releases the underlying wasm memory. */
  free(): void {
    this.inner.free();
  }
}

/**
 * The `css` option followed by the sheets a document brings, or `undefined` for none.
 *
 * @internal
 */
export function sheetsOf(
  css: CssInput | readonly CssInput[] | undefined,
  documentCss: string[],
): CssInput[] | undefined {
  const own: readonly CssInput[] = css === undefined ? [] : isCssList(css) ? css : [css];
  const sheets = [...own, ...documentCss];

  return sheets.length > 0 ? sheets : undefined;
}

/** Narrows the `css` option, which takes one entry or a list of them. */
function isCssList(css: CssInput | readonly CssInput[]): css is readonly CssInput[] {
  return Array.isArray(css);
}

/**
 * A function returning the one value `create` makes on the first call.
 *
 * @internal
 */
export function lazy<T>(create: () => T): () => T {
  let value: T | undefined;

  return () => (value ??= create());
}
