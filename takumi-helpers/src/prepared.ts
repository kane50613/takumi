import type { FetchOptions } from "./fetch";
import { type ImageFetchCache, prepareImages } from "./images";
import {
  type FontLoader,
  type FontRegistry,
  fontsFor,
  type ImageLoader,
  type ImageSource,
  type OutputFormatOptions,
  type RegisteredFamilyLike,
  resolveImageLoaders,
} from "./renderer";

type Awaitable<T> = T | Promise<T>;

/** The binding options that only change how a laid-out page is encoded. */
type OutputOnly = "format" | "quality" | "lossless" | "dithering" | "drawDebugBorder";

/** Images for {@link Prepared}: entries to use as they are, and how to fetch the rest. */
export type PrepareImagesInput =
  | ImageLoader[]
  | (Omit<FetchOptions, "signal"> & {
      /** Entries to use as they are; their URLs are not fetched. */
      sources?: ImageLoader[];
      /** Single-flight byte cache shared across renders. */
      fetchCache?: ImageFetchCache;
      /** Decode-cache default for every image; a source's own `cache` wins. */
      cache?: NonNullable<ImageLoader["cache"]>;
      /** Throw on any fetch failure; if `false`, failed images are left out. @default true */
      throwOnError?: boolean;
    });

/** What `prepare` takes on top of the layout options. */
export type PrepareExtras = {
  fonts?: FontLoader[] | Promise<FontLoader[]>;
  images?: PrepareImagesInput;
  signal?: AbortSignal;
};

/** Public `prepare` options for a backend whose binding options are {@link TInternal}. */
export type BackendPrepareOptions<TInternal> = Omit<TInternal, "images" | OutputOnly> &
  PrepareExtras;

/** Public options for rendering a {@link Prepared} on a backend whose binding options are {@link TInternal}. */
export type BackendPreparedRenderOptions<TInternal> = Pick<
  TInternal,
  Extract<keyof TInternal, "dithering" | "drawDebugBorder">
> &
  OutputFormatOptions & { signal?: AbortSignal };

/** What a render reads from a source before it lays it out. */
export type SourceNeeds = { images: string[]; characters: string };

type PreparedState<TSource, TLayout> = {
  source: TSource;
  layout: TLayout;
  fonts: FontLoader[] | undefined;
  fontFamilies: string[] | undefined;
  images: ImageSource[];
};

let createPrepared: <TSource, TLayout>(
  state: PreparedState<TSource, TLayout>,
) => Prepared<TSource, TLayout>;
let stateOf: <TSource, TLayout>(
  prepared: Prepared<TSource, TLayout>,
) => PreparedState<TSource, TLayout>;

/**
 * A source with its layout options, the font subsets its text uses, and the images it draws,
 * fetched. Render it with the `render`, `renderSvg`, or `measure` of a renderer of the backend that
 * prepared it.
 */
export class Prepared<TSource = unknown, TLayout = unknown> {
  readonly #state: PreparedState<TSource, TLayout>;

  private constructor(state: PreparedState<TSource, TLayout>) {
    this.#state = state;
  }

  // Lets this module read and create a `Prepared` while callers see no state or constructor.
  static {
    createPrepared = (state) => new Prepared(state);
    stateOf = (prepared) => prepared.#state;
  }
}

/**
 * Reads what `source` needs under `options`, then loads the font subsets its text uses and fetches
 * the images it draws that `options.images` does not supply. `needs` reads the source after the
 * cascade.
 */
export async function prepareSource<
  TFamily extends RegisteredFamilyLike,
  TSource,
  TOptions extends PrepareExtras & { fontFamilies?: string[] },
>(
  registry: FontRegistry<TFamily>,
  source: TSource,
  options: TOptions,
  needs: (layout: Omit<TOptions, keyof PrepareExtras | "fontFamilies">) => Awaitable<SourceNeeds>,
): Promise<Prepared<TSource, Omit<TOptions, keyof PrepareExtras | "fontFamilies">>> {
  const { fonts, fontFamilies, images = [], signal, ...layout } = options;

  signal?.throwIfAborted();

  const read = await needs(layout);
  const resolvedFonts = await fonts;
  const subset = resolvedFonts && (await fontsFor(resolvedFonts, () => read.characters));
  const {
    sources = [],
    cache,
    throwOnError,
    fetchCache,
    ...fetchOptions
  } = Array.isArray(images) ? { sources: images } : images;
  const provided = await resolveImageLoaders({ sources, cache });
  const fetched = await prepareImages({
    ...fetchOptions,
    urls: read.images,
    sources: provided,
    fetchCache,
    throwOnError,
    signal,
  });

  await registry.resolveResources(subset, undefined, fontFamilies, () => undefined);
  signal?.throwIfAborted();

  return createPrepared({
    source,
    layout,
    fonts: subset,
    fontFamilies,
    images: fetched.map((image) => ({ cache, ...image })),
  });
}

/**
 * Calls `call` with the binding options for rendering `prepared` with `output`, while the backend
 * holds every font it uses. The layout `prepared` holds wins over any layout option in `output`.
 */
export function renderPrepared<
  TFamily extends RegisteredFamilyLike,
  TSource,
  TLayout,
  TOutput extends { signal?: AbortSignal },
  TResult,
>(
  registry: FontRegistry<TFamily>,
  prepared: Prepared<TSource, TLayout>,
  output: TOutput,
  call: (
    source: TSource,
    options: TLayout &
      Omit<TOutput, "signal"> & { images?: ImageSource[]; fontFamilies?: string[] },
    signal: AbortSignal | undefined,
  ) => Awaitable<TResult>,
): Promise<TResult> {
  const state = stateOf(prepared);
  const { signal, ...encoding } = output;

  signal?.throwIfAborted();

  return registry.resolveResources(state.fonts, state.images, state.fontFamilies, (resolved) => {
    signal?.throwIfAborted();

    return call(state.source, { ...encoding, ...state.layout, ...resolved }, signal);
  });
}
