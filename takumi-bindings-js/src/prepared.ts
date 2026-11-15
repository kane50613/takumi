import {
  type FailedImage,
  type FetchOptions,
  fetchImagesSettled,
  type ImageFetchCache,
  prepareImages,
} from "@takumi-rs/helpers";
import {
  type FailedFont,
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
      /** @deprecated Use the renderer's `cache.store`. When set, it keeps the images and `@font-face` files this render downloads instead, and fonts listed as URLs still use the store. */
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
  fonts?: readonly FontLoader[] | Promise<readonly FontLoader[]>;
  images?: PrepareImagesInput;
  signal?: AbortSignal;
  onMissingGlyphs?: MissingGlyphsHandler;
};

/** Returns fonts for the characters no font of a render draws, which then load as part of its `fonts`. */
export type MissingGlyphsHandler = (
  characters: string[],
) => Awaitable<readonly FontLoader[] | undefined>;

/** What a render could not satisfy. */
export type RenderReport = {
  /** The characters no font in their fallback chain draws, the built-in Geist included, each once in document order. */
  missingCharacters: string[];
  /** The families `font-family` or `fontFamilies` names that no registered or listed font provides. */
  unresolvedFamilies: string[];
  /** The optional fonts and `@font-face` files that failed to load. */
  failedFonts: FailedFont[];
  /** The images left out under `images.throwOnError: false`. */
  failedImages: FailedImage[];
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

/** What a render reads from a source before it lays it out: the images it draws, the ids of the listed fonts it loads, the `@font-face` files it loads, the loaded ones it draws with, and what no font provides. */
type SourceNeeds = {
  images: string[];
  fonts: number[];
  fontFiles?: string[];
  loadedFontFiles?: string[];
  missingCharacters?: string[];
  unresolvedFamilies?: string[];
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

/** How a render fetches `@font-face` files and the fonts its `fonts` list names by URL. */
type FontFetch = Omit<FetchOptions, "signal"> & { fetchCache?: ImageFetchCache };

type PreparedState<TSource, TLayout> = {
  source: TSource;
  layout: TLayout;
  fonts: readonly FontLoader[] | undefined;
  /** `fonts` and every listed font a read loaded, which the render forgets once it settles if they use `cache: "none"`. */
  loadedFonts: readonly FontLoader[] | undefined;
  fontFamilies: string[] | undefined;
  images: ImageSource[];
  /** The `@font-face` files the source draws with, which a render loads again once the renderer evicted them. */
  fontFiles: string[];
  failedFontFiles: string[];
  fontFetch: FontFetch;
  report: RenderReport;
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
  /** What the render could not satisfy when it was prepared. */
  readonly report: RenderReport;
  readonly #state: PreparedState<TSource, TLayout>;

  private constructor(state: PreparedState<TSource, TLayout>) {
    this.#state = state;
    this.report = state.report;
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

  const failedFonts = await registry.dropOnFailure(state.fonts, async () => {
    const failed = await registry.resolveResources(
      state.fonts && registry.loaders(state.fonts, { ...state.fontFetch, signal: extras.signal }),
      undefined,
      state.fontFamilies,
      (resolved) => resolved.failedFonts ?? [],
      undefined,
      extras.signal,
    );

    extras.signal?.throwIfAborted();

    return failed;
  });

  return createPrepared({
    ...state,
    fonts: state.fonts && registry.without(state.fonts, failedFonts),
    report: { ...state.report, failedFonts: [...state.report.failedFonts, ...failedFonts] },
  });
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

/** Fetches each of `urls` and hands it to the renderer leased to `lease`, returning the ones that failed and why. */
async function loadFontFiles<TFamily extends RegisteredFamilyLike>(
  registry: FontRegistry<TFamily>,
  urls: string[],
  options: FontFetch & { signal?: AbortSignal },
  lease: number,
): Promise<FailedFont[]> {
  const { images, failed } = await fetchImagesSettled({
    ...options,
    urls,
    store: registry.store.for(options.fetch),
  });
  const fetched = new Map(images.map(({ src, data }) => [src, data]));
  const errors = new Map(failed.map(({ src, error }) => [src, error]));
  const loaded = await Promise.all(
    urls.map(async (url): Promise<FailedFont[]> => {
      const data = fetched.get(url);

      if (data === undefined) {
        const error = errors.get(url) ?? new Error(`Fetching ${url} needs your own fetch`);

        return [{ source: url, error }];
      }

      return registry.cacheFontFile(url, data, lease).then(
        () => [],
        (error: unknown) => [{ source: url, error }],
      );
    }),
  );

  return loaded.flat();
}

/** How {@link loadReached} loads what each read reaches. */
type Reach = {
  /** The `@font-face` files handled, each at most once. */
  tried: Set<string>;
  /** The `@font-face` files that failed, which each read moves their rules on from. */
  failedFontFiles: string[];
  /** How to fetch `@font-face` files, which are left alone without it. */
  fetch?: FontFetch & { signal?: AbortSignal };
  /** The listed fonts a read reaches that are not cached yet. */
  uncachedOf?: (needs: SourceNeeds | undefined) => FontLoader[];
  /** The optional listed fonts and the `@font-face` files that failed to load. */
  failedFonts?: FailedFont[];
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
  { tried, failedFontFiles, fetch, uncachedOf, failedFonts = [], signal }: Reach,
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
      const failed = await loadFontFiles(registry, evicted, fetch, lease);

      failedFontFiles.push(...failed.map((font) => font.source));
      failedFonts.push(...failed);
    }

    const loadedListed = await registry.cache(
      uncachedOf?.(latest) ?? [],
      lease,
      failedFonts,
      signal,
    );

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
  const { fonts, fontFamilies, images = [], signal, onMissingGlyphs } = extras;

  signal?.throwIfAborted();

  const resolvedFonts = await fonts;
  const listed = resolvedFonts && registry.listed(resolvedFonts);
  const request: NeedsRequest = { fonts: listed, fontFamilies };
  let first: Promise<SourceNeeds> | undefined;
  const read = () => (first ??= Promise.resolve(needs(request)));
  const {
    sources = [],
    cache,
    throwOnError = true,
    fetchCache,
    ...fetchOptions
  } = Array.isArray(images) ? { sources: images } : images;
  const provided = await resolveImageLoaders({ sources, cache });
  const failedImages: FailedImage[] = [];
  const fetchImages = async (urls: string[]) => {
    const options = {
      ...fetchOptions,
      urls,
      sources: provided,
      store: registry.store.for(fetchOptions.fetch),
      fetchCache,
      signal,
    };

    if (throwOnError) {
      return prepareImages(options);
    }

    const settled = await fetchImagesSettled(options);

    failedImages.push(...settled.failed);

    return settled.images;
  };
  const loaded = fetchMissing ? await fetchImages((await read()).images) : provided;
  const entries: ImageSource[] = loaded.map((image) => ({ cache, ...image }));
  const firstReached = listed
    ? await (entries.length > 0 ? needs({ ...request, images: entries }) : read())
    : fetchMissing
      ? await read()
      : undefined;

  const tried = new Set<string>();
  const cached = new Set<number>();
  const failedFontFiles: string[] = [];
  const failedFonts: FailedFont[] = [];
  const fontFetch = { ...fetchOptions, fetchCache };
  const fetch = { ...fontFetch, signal };
  const reread = (failed: string[]) =>
    needs({
      ...request,
      fonts: listedWithout(listed, failedFonts),
      images: entries,
      failedFontFiles: failed,
    });
  /** Reads the source with every font of `fonts` loaded, falling back through `families` as the render does. */
  const readLoaded = (fonts: readonly FontLoader[] | undefined, families: string[] | undefined) =>
    registry.resolveResources(
      fonts && registry.loaders(fonts, fetch),
      undefined,
      families,
      ({ fontFamilies: resolved, failedFonts: failed = [] }) => {
        failedFonts.push(...failed);

        return needs({ images: entries, fontFamilies: resolved, failedFontFiles });
      },
    );

  // The fonts each read loads stay leased, so the next read sees them however small the cache.
  const settled = await registry.dropOnFailure(resolvedFonts, () =>
    registry.leased(async (lease) => {
      const reached = await loadReached(
        registry,
        firstReached,
        reread,
        {
          tried,
          failedFontFiles,
          fetch: fetchMissing ? fetch : undefined,
          uncachedOf: (needs) =>
            listed && resolvedFonts
              ? registry.loaders(
                  unseen(needs?.fonts ?? [], cached).flatMap((id) => resolvedFonts[id] ?? []),
                  fetch,
                )
              : [],
          failedFonts,
          signal,
        },
        lease,
      );
      const loads = listed && reached?.fonts;
      const subset = loads ? resolvedFonts?.filter((_, id) => loads.includes(id)) : resolvedFonts;
      const kept = subset && registry.without(subset, failedFonts);

      if (!fetchMissing && !onMissingGlyphs) {
        return { fonts: kept, fontFamilies, coverage: undefined, added: [] };
      }

      // A read saw each listed font as loading, which a listed font that failed makes wrong, and
      // saw none of the fonts when none is listed.
      const coverage =
        listed && kept?.length === subset?.length
          ? reached
          : resolvedFonts
            ? await readLoaded(kept, fontFamilies)
            : (reached ?? (await read()));
      const characters = coverage?.missingCharacters ?? [];
      const added = characters.length > 0 ? ((await onMissingGlyphs?.(characters)) ?? []) : [];

      signal?.throwIfAborted();

      if (added.length === 0) {
        return { fonts: kept, fontFamilies, coverage, added };
      }

      return registry.dropOnFailure(added, async () => {
        const families = fontFamilies && [
          ...new Set([...fontFamilies, ...(await familiesOf(registry, added, fetch, failedFonts))]),
        ];
        const all = registry.without([...(kept ?? []), ...added], failedFonts);

        return {
          fonts: all,
          fontFamilies: families,
          coverage: await readLoaded(all, families),
          added,
        };
      });
    }),
  );

  return {
    source,
    layout,
    fonts: settled.fonts && registry.without(settled.fonts, failedFonts),
    loadedFonts: [
      ...((listed ? resolvedFonts?.filter((_, id) => cached.has(id)) : resolvedFonts) ?? []),
      ...settled.added,
    ],
    fontFamilies: settled.fontFamilies,
    images: entries,
    fontFiles: [...tried].filter((url) => !failedFontFiles.includes(url)),
    failedFontFiles,
    fontFetch,
    reread,
    report: {
      missingCharacters: settled.coverage?.missingCharacters ?? [],
      unresolvedFamilies: settled.coverage?.unresolvedFamilies ?? [],
      failedFonts,
      failedImages,
    },
  };
}

/** `listed` with each font in `failed` left as a bare key, so a read reaches the family after it. */
function listedWithout(
  listed: ListedFont[] | undefined,
  failed: FailedFont[],
): ListedFont[] | undefined {
  if (!listed || failed.length === 0) {
    return listed;
  }

  const sources = new Set(failed.map((font) => font.source));

  return listed.map((font) => (sources.has(font.key) ? { key: font.key } : font));
}

/** The families `fonts` register, loading them and adding the optional ones that fail to `failed`. */
function familiesOf<TFamily extends RegisteredFamilyLike>(
  registry: FontRegistry<TFamily>,
  fonts: readonly FontLoader[],
  fetch: FetchOptions,
  failed: FailedFont[],
): Promise<string[]> {
  return registry.resolveResources(
    registry.loaders(fonts, fetch),
    undefined,
    undefined,
    ({ fontFamilies = [], failedFonts = [] }) => {
      failed.push(...failedFonts);

      return fontFamilies;
    },
  );
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

  const fetch = { ...state.fontFetch, signal };
  const files: HeldFontFiles | undefined =
    state.fontFiles.length > 0
      ? {
          urls: state.fontFiles,
          load: async (urls, lease) => {
            const failed = (await loadFontFiles(registry, urls, fetch, lease)).map(
              (font) => font.source,
            );

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
      state.fonts && registry.loaders(state.fonts, fetch),
      state.images,
      state.fontFamilies,
      ({ images, fontFamilies, failedFontFiles: failedAgain = [], failedFonts = [] }) => {
        signal?.throwIfAborted();

        // An optional font that fails now, as after the renderer evicted it, stays out of later renders.
        if (failedFonts.length > 0) {
          state.fonts = state.fonts && registry.without(state.fonts, failedFonts);
          state.failedFonts.push(...failedFonts);
        }

        const failedFontFiles = [...state.failedFontFiles, ...failedAgain];
        const failed = failedFontFiles.length > 0 ? { failedFontFiles } : {};

        return call(
          state.source,
          { ...encoding, ...state.layout, images, fontFamilies, ...failed },
          signal,
        );
      },
      files,
      signal,
    )
    .finally(() => registry.dropUnkept(state.loadedFonts));
}
