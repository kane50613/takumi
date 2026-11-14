import { type FetchOptions, type ImageFetchCache, prepareImages } from "@takumi-rs/helpers";
import {
  type FontDetails,
  type FontLoader,
  type FontRegistry,
  type HeldFontFiles,
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
      /** @deprecated Use the renderer's `cache.store`. When set, it keeps this render's downloads instead. */
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
export type BackendPrepareOptions<TInternal> = Omit<
  TInternal,
  "images" | "failedFontFiles" | OutputOnly
> &
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

/** What a render reads from a source before it lays it out: the images it draws, the ids of the listed fonts it loads, the `@font-face` files it loads, and the loaded ones it draws with. */
type SourceNeeds = {
  images: string[];
  fonts: number[];
  fontFiles?: string[];
  loadedFontFiles?: string[];
};

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
  /** The `@font-face` files this document failed to download, so their rules move on to their next `src`. */
  failedFontFiles?: string[];
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
  { images, fonts, fontFamilies, failedFontFiles }: NeedsRequest,
): [
  TLayout & { images?: ImageSource[]; fontFamilies?: string[]; failedFontFiles?: string[] },
  ListedFont[] | undefined,
] {
  return [{ ...layout, images, fontFamilies, failedFontFiles }, fonts];
}

/** How {@link loadFontFiles} fetches `@font-face` files. */
type FontFileFetch = Omit<FetchOptions, "signal"> & { fetchCache?: ImageFetchCache };

type PreparedState<TSource, TLayout> = {
  source: TSource;
  layout: TLayout;
  fonts: FontLoader[] | undefined;
  fontFamilies: string[] | undefined;
  images: ImageSource[];
  /** The `@font-face` files the source draws with, which a render loads again once the renderer evicted them. */
  fontFiles: string[];
  failedFontFiles: string[];
  fontFileFetch: FontFileFetch;
  /** Reads the source again with `failedFontFiles`, so their rules move on to their next `src`. */
  reread: (failedFontFiles: string[]) => Awaitable<SourceNeeds>;
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

  await registry.dropOnFailure(state.fonts, async () => {
    await registry.resolveResources(state.fonts, undefined, state.fontFamilies, () => undefined);
    extras.signal?.throwIfAborted();
  });

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

/** The `items` not `seen` yet, which join them. */
function unseen<T>(items: T[], seen: Set<T>): T[] {
  const fresh = items.filter((item) => !seen.has(item));

  for (const item of fresh) {
    seen.add(item);
  }

  return fresh;
}

/** Fetches each of `urls` and hands it to the renderer leased to `lease`, returning the ones that failed. */
async function loadFontFiles<TFamily extends RegisteredFamilyLike>(
  registry: FontRegistry<TFamily>,
  urls: string[],
  options: FontFileFetch & { signal?: AbortSignal },
  lease: number,
): Promise<string[]> {
  const fetched = new Map(
    (
      await prepareImages({
        ...options,
        urls,
        store: registry.store.for(options.fetch),
        throwOnError: false,
      })
    ).map(({ src, data }) => [src, data]),
  );

  const loaded = await Promise.all(
    urls.map(async (url) => {
      const data = fetched.get(url);

      return (
        data !== undefined &&
        registry.cacheFontFile(url, data, lease).then(
          () => true,
          () => false,
        )
      );
    }),
  );

  return urls.filter((_, index) => !loaded[index]);
}

/** How {@link loadReached} loads what each read reaches. */
type Reach = {
  /** The `@font-face` files handled, each at most once. */
  tried: Set<string>;
  /** The `@font-face` files that failed, which each read moves their rules on from. */
  failedFontFiles: string[];
  /** How to fetch `@font-face` files, which are left alone without it. */
  fetch?: FontFileFetch & { signal?: AbortSignal };
  /** The listed fonts a read reaches that are not cached yet. */
  uncachedOf?: (needs: SourceNeeds | undefined) => FontLoader[];
  signal?: AbortSignal;
};

/**
 * Leases or loads the fonts `first` reaches, leased to `lease`, then reads again with `read` and
 * loads what that reaches, until a read reaches nothing new. Returns the last read.
 *
 * A file that fails moves its rule on to its next `src`, and a listed subset without a glyph its
 * `unicodeRange` claims moves the character on to the next family, as a browser does. The next read
 * reports either. Each URL and listed font loads at most once, so a font the renderer evicts to fit
 * another cannot loop the reads.
 */
async function loadReached<TFamily extends RegisteredFamilyLike>(
  registry: FontRegistry<TFamily>,
  first: SourceNeeds | undefined,
  read: (failedFontFiles: string[]) => Awaitable<SourceNeeds>,
  { tried, failedFontFiles, fetch, uncachedOf, signal }: Reach,
  lease: number,
): Promise<SourceNeeds | undefined> {
  const reached = (urls: string[] = []) => (fetch ? unseen(urls, tried) : []);
  let latest = first;

  for (;;) {
    const pending = reached(latest?.fontFiles);
    const evicted = registry.leaseFontFiles(
      [...pending, ...reached(latest?.loadedFontFiles)],
      lease,
    );

    if (fetch && evicted.length > 0) {
      failedFontFiles.push(...(await loadFontFiles(registry, evicted, fetch, lease)));
    }

    const loadedListed = await registry.cache(uncachedOf?.(latest) ?? [], lease);

    signal?.throwIfAborted();

    if (pending.length === 0 && evicted.length === 0 && !loadedListed) {
      return latest;
    }

    latest = await read(failedFontFiles);
  }
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
        store: registry.store.for(fetchOptions.fetch),
        fetchCache,
        throwOnError,
        signal,
      })
    : provided;
  const entries: ImageSource[] = loaded.map((image) => ({ cache, ...image }));
  const firstReached = listed
    ? await (entries.length > 0 ? needs({ ...request, images: entries }) : read())
    : fetchMissing
      ? await read()
      : undefined;

  const tried = new Set<string>();
  const cached = new Set<number>();
  const failedFontFiles: string[] = [];
  const fontFileFetch = { ...fetchOptions, fetchCache };
  const reread = (failed: string[]) =>
    needs({ ...request, images: entries, failedFontFiles: failed });
  // The fonts each read loads stay leased, so the next read sees them however small the cache.
  const reached = await registry.dropOnFailure(resolvedFonts, () =>
    registry.leased((lease) =>
      loadReached(
        registry,
        firstReached,
        reread,
        {
          tried,
          failedFontFiles,
          fetch: fetchMissing ? { ...fontFileFetch, signal } : undefined,
          uncachedOf: (needs) =>
            listed && resolvedFonts
              ? unseen(needs?.fonts ?? [], cached).flatMap((id) => resolvedFonts[id] ?? [])
              : [],
          signal,
        },
        lease,
      ),
    ),
  );

  const loads = listed && reached?.fonts;
  const subset = loads ? resolvedFonts?.filter((_, id) => loads.includes(id)) : resolvedFonts;

  return {
    source,
    layout,
    fonts: subset,
    fontFamilies,
    images: entries,
    fontFiles: [...tried].filter((url) => !failedFontFiles.includes(url)),
    failedFontFiles,
    fontFileFetch,
    reread,
  };
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
      Omit<TOutput, "signal"> & {
        images?: ImageSource[];
        fontFamilies?: string[];
        failedFontFiles?: string[];
      },
    signal: AbortSignal | undefined,
  ) => Awaitable<TResult>,
): Promise<TResult> {
  const state = stateOf(prepared);
  const { signal, ...encoding } = output;

  signal?.throwIfAborted();

  const fetch = { ...state.fontFileFetch, signal };
  const files: HeldFontFiles | undefined =
    state.fontFiles.length > 0
      ? {
          urls: state.fontFiles,
          load: async (urls, lease) => {
            const failed = await loadFontFiles(registry, urls, fetch, lease);

            if (failed.length === 0) {
              return failed;
            }

            // Only another read reaches the next `src` of a rule whose file failed again.
            const failedFontFiles = [...state.failedFontFiles, ...failed];
            const tried = new Set([...state.fontFiles, ...failedFontFiles]);
            const first = await state.reread(failedFontFiles);

            await loadReached(
              registry,
              first,
              state.reread,
              { tried, failedFontFiles, fetch, signal },
              lease,
            );

            return failedFontFiles.slice(state.failedFontFiles.length);
          },
        }
      : undefined;

  return registry
    .resolveResources(
      state.fonts,
      state.images,
      state.fontFamilies,
      ({ failedFontFiles: failedAgain = [], ...resolved }) => {
        signal?.throwIfAborted();

        const failedFontFiles = [...state.failedFontFiles, ...failedAgain];
        const failed = failedFontFiles.length > 0 ? { failedFontFiles } : {};

        return call(state.source, { ...encoding, ...state.layout, ...resolved, ...failed }, signal);
      },
      files,
    )
    .finally(() => registry.dropUnkept(state.fonts));
}
