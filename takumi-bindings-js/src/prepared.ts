import { type FetchOptions, type ImageFetchCache, prepareImages } from "@takumi-rs/helpers";
import {
  type FontDetails,
  type FontLoader,
  type FontRegistry,
  type ImageLoader,
  type ImageSource,
  type OutputFormatOptions,
  type RegisteredFamilyLike,
  resolveImageLoaders,
} from "./renderer";

type Awaitable<T> = T | Promise<T>;

/** The binding options that only change how a laid-out page is encoded. */
type OutputOnly = "format" | "quality" | "lossless" | "dithering" | "drawDebugBorder";

/**
 * Images for {@link Prepared}: entries to use as they are, and how to fetch the rest.
 *
 * @internal
 */
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

/**
 * What `prepare` takes on top of the layout options.
 *
 * @internal
 */
export type PrepareExtras = {
  fonts?: FontLoader[] | Promise<FontLoader[]>;
  images?: PrepareImagesInput;
  signal?: AbortSignal;
};

/**
 * Public `prepare` options for a backend whose binding options are {@link TInternal}.
 *
 * @internal
 */
export type BackendPrepareOptions<TInternal> = Omit<TInternal, "images" | OutputOnly> &
  PrepareExtras;

/**
 * Public options for rendering a {@link Prepared} on a backend whose binding options are {@link TInternal}.
 *
 * @internal
 */
export type BackendPreparedRenderOptions<TInternal> = Pick<
  TInternal,
  Extract<keyof TInternal, "dithering" | "drawDebugBorder">
> &
  OutputFormatOptions & { signal?: AbortSignal };

/** What a render reads from a source before it lays it out: the images it draws, the ids of the listed fonts it loads, and the `@font-face` files it loads. */
type SourceNeeds = { images: string[]; fonts: number[]; fontFiles?: string[] };

/**
 * A font of a render's `fonts` list, described before its bytes arrive.
 *
 * @internal
 */
export type ListedFont = Pick<FontDetails, "name" | "weight" | "style" | "unicodeRange"> & {
  key: string;
};

/**
 * What a needs pass reads a source with besides its layout.
 *
 * @internal
 */
export type NeedsRequest = {
  /** The images a render will draw, whose SVG `text` elements count too. */
  images?: ImageSource[];
  /** The render's `fonts` list, whose ids it reports when the render loads them. */
  fonts?: ListedFont[];
  /** The font stack the render falls back through, or by default the one `fonts` describes. */
  fontFamilies?: string[];
};

/**
 * Reads what a source needs after the cascade.
 *
 * @internal
 */
export type SourceReader = (request: NeedsRequest) => Awaitable<SourceNeeds>;

/**
 * The options and listed fonts a binding's `needs` takes for `request` on top of `layout`.
 *
 * @internal
 */
export function needsArgs<TLayout>(
  layout: TLayout,
  { images, fonts, fontFamilies }: NeedsRequest,
): [TLayout & { images?: ImageSource[]; fontFamilies?: string[] }, ListedFont[] | undefined] {
  return [{ ...layout, images, fontFamilies }, fonts];
}

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

/** A source with its layout options, the font subsets its text uses, and the images it draws, fetched. */
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
 * Reads what `source` needs under `layout`, then loads its font subsets and fetches the images `extras.images` lacks.
 *
 * @internal
 */
export async function prepareSource<TFamily extends RegisteredFamilyLike, TSource, TLayout>(
  registry: FontRegistry<TFamily>,
  source: TSource,
  layout: TLayout,
  extras: PrepareExtras & { fontFamilies?: string[] },
  needs: SourceReader,
): Promise<Prepared<TSource, TLayout>> {
  const state = await collect(registry, source, layout, extras, needs, true);

  await registry.resolveResources(state.fonts, undefined, state.fontFamilies, () => undefined);
  extras.signal?.throwIfAborted();

  return createPrepared(state);
}

/**
 * {@link prepareSource} without fetching, running `needs` only when `fonts` holds `unicodeRange` subsets.
 *
 * @internal
 */
export async function readSource<TFamily extends RegisteredFamilyLike, TSource, TLayout>(
  registry: FontRegistry<TFamily>,
  source: TSource,
  layout: TLayout,
  extras: PrepareExtras & { fontFamilies?: string[] },
  needs: SourceReader,
): Promise<Prepared<TSource, TLayout>> {
  return createPrepared(await collect(registry, source, layout, extras, needs, false));
}

/** The `@font-face` files `reached` reports that the renderer holds neither loaded nor failed. */
function missingFontFiles<TFamily extends RegisteredFamilyLike>(
  registry: FontRegistry<TFamily>,
  reached: SourceNeeds | undefined,
): string[] {
  return (reached?.fontFiles ?? []).filter((url) => !registry.hasFontFile(url));
}

/** Fetches each of `urls` and hands it to the renderer, recording the ones that fail. */
async function loadFontFiles<TFamily extends RegisteredFamilyLike>(
  registry: FontRegistry<TFamily>,
  urls: string[],
  options: Omit<FetchOptions, "signal"> & { fetchCache?: ImageFetchCache; signal?: AbortSignal },
): Promise<void> {
  const fetched = new Map(
    (await prepareImages({ ...options, urls, throwOnError: false })).map(({ src, data }) => [
      src,
      data,
    ]),
  );

  await Promise.all(
    urls.map(async (url) => {
      const data = fetched.get(url);
      const loaded =
        data !== undefined &&
        (await registry.cacheFontFile(url, data).then(
          () => true,
          () => false,
        ));

      if (!loaded) {
        registry.failFontFile(url);
      }
    }),
  );
}

async function collect<TFamily extends RegisteredFamilyLike, TSource, TLayout>(
  registry: FontRegistry<TFamily>,
  source: TSource,
  layout: TLayout,
  extras: PrepareExtras & { fontFamilies?: string[] },
  needs: SourceReader,
  fetchMissing: boolean,
): Promise<PreparedState<TSource, TLayout>> {
  const { fonts, fontFamilies, images = [], signal } = extras;

  signal?.throwIfAborted();

  const resolvedFonts = await fonts;
  const listed = resolvedFonts && registry.listed(resolvedFonts);
  const request: NeedsRequest = { fonts: listed, fontFamilies };
  let first: Promise<SourceNeeds> | undefined;
  const read = () => (first ??= Promise.resolve(needs(request)));
  const {
    sources = [],
    cache,
    throwOnError,
    fetchCache,
    ...fetchOptions
  } = Array.isArray(images) ? { sources: images } : images;
  const provided = await resolveImageLoaders({ sources, cache });
  const loaded = fetchMissing
    ? await prepareImages({
        ...fetchOptions,
        urls: (await read()).images,
        sources: provided,
        fetchCache,
        throwOnError,
        signal,
      })
    : provided;
  const entries: ImageSource[] = loaded.map((image) => ({ cache, ...image }));
  let reached = listed
    ? await (entries.length > 0 ? needs({ ...request, images: entries }) : read())
    : fetchMissing
      ? await read()
      : undefined;

  // A file that fails moves its rule on to its next `src`, which the next read reports.
  let missing = fetchMissing ? missingFontFiles(registry, reached) : [];

  while (missing.length > 0) {
    await loadFontFiles(registry, missing, { ...fetchOptions, fetchCache, signal });
    reached = await needs({ ...request, images: entries });
    missing = missingFontFiles(registry, reached);
  }

  const loads = listed && reached?.fonts;
  const subset = loads ? resolvedFonts?.filter((_, id) => loads.includes(id)) : resolvedFonts;

  return { source, layout, fonts: subset, fontFamilies, images: entries };
}

/**
 * Calls `call` with the binding options for rendering `prepared` with `output`. The layout `prepared` holds wins.
 *
 * @internal
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
