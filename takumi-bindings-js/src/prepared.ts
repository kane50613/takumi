import type { FetchOptions, ImageFetchCache } from "@takumi-rs/helpers";
import { type FailedImage, fetchImages, fetchImagesSettled } from "@takumi-rs/helpers/renderer";
import {
  downloadStore,
  type FontDetails,
  type FontFetch,
  type FontLoad,
  type FontLoader,
  type FontRegistry,
  type FontScope,
  fontScope,
  type HeldFontFiles,
  type ImageLoader,
  type ImageSource,
  type OutputFormatOptions,
  type RegisteredFamilyLike,
  resolveImageLoaders,
  type WrapperOptions,
} from "./renderer";
import { warnUndrawn } from "./warning";

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
      /** @deprecated A render without its own `fetch` keeps its downloads in a store the renderers share. When set, it keeps the images and `@font-face` files this render downloads instead. */
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
  onMissingGlyphs?: MissingGlyphsHandler;
};

/**
 * What a render reads its source with: {@link PrepareExtras} and the fallback `fontFamilies`.
 *
 * @internal
 */
export type SourceExtras = PrepareExtras & { fontFamilies?: string[] };

/** {@link SourceExtras} apart from the layout options of `T`. */
type SplitExtras<T> = { extras: SourceExtras; layout: Omit<T, keyof SourceExtras> };

/**
 * `options` split into its {@link SourceExtras} and the layout options the binding takes, member by
 * member when `T` is a union, so the layout keeps its discriminants.
 *
 * @internal
 */
export function splitExtras<T extends SourceExtras>(
  options: T,
): T extends unknown ? SplitExtras<T> : never;
export function splitExtras<T extends SourceExtras>({
  fonts,
  images,
  signal,
  fontFamilies,
  onMissingGlyphs,
  ...layout
}: T): SplitExtras<T> {
  return {
    extras: { fonts, images, signal, fontFamilies, onMissingGlyphs },
    layout,
  };
}

/** Returns fonts for the characters no font of a render draws, which then load as part of its `fonts`. */
export type MissingGlyphsHandler = (characters: string[]) => Awaitable<FontLoader[] | undefined>;

/**
 * Public `prepare` options for a backend whose binding options are {@link TInternal}.
 *
 * @internal
 */
export type BackendPrepareOptions<TInternal> = Omit<TInternal, WrapperOptions | OutputOnly> &
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
  /** The fonts the render lists, and the `@font-face` files and optional fonts that failed, so their rules move on to their next `src`. */
  fontScope: FontScope;
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
  { images, fonts, fontFamilies, fontScope }: NeedsRequest,
): [
  TLayout & { images?: ImageSource[]; fontFamilies?: string[]; fontScope: FontScope },
  ListedFont[] | undefined,
] {
  return [{ ...layout, images, fontFamilies, fontScope }, fonts];
}

type PreparedState<TSource, TLayout> = {
  source: TSource;
  layout: TLayout;
  fonts: FontLoader[] | undefined;
  /** `fonts` came from `onMissingGlyphs` alone, so they join the registered fallback chain. */
  implicitFamilies: boolean;
  /** `fonts` and every listed font a read loaded, which the render forgets once it settles if they use `cache: "none"`. */
  loadedFonts: FontLoader[] | undefined;
  fontFamilies: string[] | undefined;
  images: ImageSource[];
  /** The `@font-face` files the source draws with, which a render loads again once the renderer evicted them. */
  fontFiles: string[];
  /** The `@font-face` files and the optional fonts that failed to load, which `fonts` leaves out. */
  failedFontFiles: string[];
  fontFetch: Omit<FontFetch, "signal">;
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
  extras: SourceExtras,
  needs: SourceReader,
): Promise<Prepared<TSource, TLayout>> {
  const state = await collect(registry, source, layout, extras, needs, true);

  const failed = await registry.dropOnFailure(state.fonts, async () => {
    const failedFonts = await registry.resolveResources(
      state.fonts && registry.loaders(state.fonts, { ...state.fontFetch, signal: extras.signal }),
      undefined,
      state.fontFamilies,
      (resolved) => resolved.fontScope.failed ?? [],
      { implicitFamilies: state.implicitFamilies },
    );

    extras.signal?.throwIfAborted();

    return failedFonts;
  });

  return createPrepared({
    ...state,
    fonts: state.fonts && registry.without(state.fonts, failed),
    failedFontFiles: [...state.failedFontFiles, ...failed],
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
  extras: SourceExtras,
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

/** How {@link loadReached} loads what each read reaches. */
type Reach = {
  /** The `@font-face` files handled, each at most once. */
  tried: Set<string>;
  /** How to fetch `@font-face` files, which are left alone without it. */
  fetch?: FontFetch;
  /** The listed fonts a read reaches that are not cached yet. */
  uncachedOf?: (needs: SourceNeeds | undefined) => FontLoader[];
  signal?: AbortSignal;
};

/**
 * Leases or loads the fonts `first` reaches under `load`, then reads again with `read` and loads
 * what that reaches, until a read reaches nothing new. Returns the last read. The `@font-face`
 * files and optional fonts that fail join `load.failed`, which each read moves on from.
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
  { tried, fetch, uncachedOf, signal }: Reach,
  load: FontLoad,
): Promise<SourceNeeds | undefined> {
  const reached = (urls: string[] = []) => (fetch ? unseen(urls, tried) : []);
  let latest = first;

  for (;;) {
    const pending = reached(latest?.fontFiles);
    const evicted = registry.leaseFontFiles(
      [...pending, ...reached(latest?.loadedFontFiles)],
      load.lease,
    );

    if (fetch && evicted.length > 0) {
      await registry.loadFontFiles(evicted, fetch, load);
    }

    signal?.throwIfAborted();

    const loadedListed = await registry.cache(uncachedOf?.(latest) ?? [], load);

    signal?.throwIfAborted();

    if (pending.length === 0 && evicted.length === 0 && !loadedListed) {
      return latest;
    }

    latest = await read(load.failed);
  }
}

async function collect<TFamily extends RegisteredFamilyLike, TSource, TLayout>(
  registry: FontRegistry<TFamily>,
  source: TSource,
  layout: TLayout,
  extras: SourceExtras,
  needs: SourceReader,
  fetchMissing: boolean,
): Promise<PreparedState<TSource, TLayout>> {
  const { fonts, fontFamilies, images = [], signal, onMissingGlyphs } = extras;

  signal?.throwIfAborted();

  const resolvedFonts = await fonts;

  signal?.throwIfAborted();

  const listed = resolvedFonts && registry.listed(resolvedFonts);
  const listedKeys = resolvedFonts && registry.keys(resolvedFonts);
  const request: NeedsRequest = {
    fonts: listed,
    fontFamilies,
    fontScope: fontScope(listedKeys, []),
  };
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
  const loadImages = async (urls: string[]) => {
    const options = {
      ...fetchOptions,
      urls,
      sources: provided,
      store: downloadStore(fetchOptions.fetch),
      fetchCache,
      signal,
    };

    if (throwOnError) {
      return fetchImages(options);
    }

    const settled = await fetchImagesSettled(options);

    failedImages.push(...settled.failed);

    return settled.images;
  };
  const loaded = fetchMissing ? await loadImages((await read()).images) : provided;
  const entries: ImageSource[] = loaded.map((image) => ({ cache, ...image }));
  // The images go into the read, so text and `@font-face` files inside a downloaded SVG count.
  const readWithImages = () =>
    entries.length > 0 ? needs({ ...request, images: entries }) : read();
  const firstReached = listed || fetchMissing ? await readWithImages() : undefined;

  const tried = new Set<string>();
  const cached = new Set<number>();
  const failedFontFiles: string[] = [];
  const fontFetch = { ...fetchOptions, fetchCache };
  const fetch = { ...fontFetch, signal };
  const reread = (failed: string[]) =>
    needs({
      ...request,
      fonts: listedWithout(listed, failed),
      images: entries,
      fontScope: fontScope(listedKeys, failed),
    });
  /** Loads every font of `fonts` as the render does, then calls `use` with the families it falls back through. */
  const resolveFonts = <T>(
    fonts: FontLoader[] | undefined,
    families: string[] | undefined,
    use: (fontFamilies: string[] | undefined, keys: string[] | undefined) => Awaitable<T>,
    implicitFamilies = false,
  ) =>
    registry.resolveResources(
      fonts && registry.loaders(fonts, fetch),
      undefined,
      families,
      ({ fontFamilies: resolved, fontScope: { listed: keys, failed = [] } }) => {
        failedFontFiles.push(...failed);

        return use(resolved, keys);
      },
      { implicitFamilies },
    );
  /** Reads the source with every font of `fonts` loaded, falling back through `families` as the render does. */
  const readLoaded = (
    fonts: FontLoader[] | undefined,
    families: string[] | undefined,
    implicitFamilies = false,
  ) =>
    resolveFonts(
      fonts,
      families,
      (resolved, keys) =>
        needs({
          images: entries,
          fontFamilies: resolved,
          fontScope: fontScope(keys, failedFontFiles),
        }),
      implicitFamilies,
    );

  const fontFileFetch = fetchMissing ? fetch : undefined;
  const load = (lease: number): FontLoad => ({ lease, failed: failedFontFiles });

  // The fonts each read loads stay leased, so the next read sees them however small the cache.
  const settled = await registry.dropOnFailure(resolvedFonts, () =>
    registry.leased(async (lease) => {
      const reached = await loadReached(
        registry,
        firstReached,
        reread,
        {
          tried,
          fetch: fontFileFetch,
          uncachedOf: (needs) =>
            listed && resolvedFonts
              ? registry.loaders(
                  unseen(needs?.fonts ?? [], cached).flatMap((id) => resolvedFonts[id] ?? []),
                  fetch,
                )
              : [],
          signal,
        },
        load(lease),
      );
      const loads = listed && reached?.fonts;
      const subset = loads ? resolvedFonts?.filter((_, id) => loads.includes(id)) : resolvedFonts;
      const kept = subset && registry.without(subset, failedFontFiles);

      // Without a fetch, a read checks what no font draws only for `onMissingGlyphs`.
      if (!fetchMissing && !onMissingGlyphs) {
        return {
          fonts: kept,
          fontFamilies,
          coverage: undefined,
          added: [],
          implicitFamilies: false,
        };
      }

      // A read saw each listed font as loading, which a listed font that failed makes wrong, and
      // saw none of the fonts when none is listed.
      const coverage =
        listed && kept?.length === subset?.length
          ? reached
          : resolvedFonts
            ? await readLoaded(kept, fontFamilies)
            : (reached ?? (await readWithImages()));
      const characters = coverage?.missingCharacters ?? [];
      const added = characters.length > 0 ? ((await onMissingGlyphs?.([...characters])) ?? []) : [];

      signal?.throwIfAborted();

      if (added.length === 0) {
        return { fonts: kept, fontFamilies, coverage, added, implicitFamilies: false };
      }

      // The fonts that failed stay out, though the hook's fonts may reuse their keys, and an earlier
      // failure under a key the hook returns again does not keep the hook's font out.
      const loadable = kept && registry.without(kept, failedFontFiles);

      registry.clearFailures(added, failedFontFiles);

      return registry.dropOnFailure(added, async () => {
        // Without `fonts` or `fontFamilies`, the hook's fonts join the registered fallback chain.
        const implicitFamilies = !resolvedFonts && !fontFamilies;
        const families = fontFamilies && [
          ...new Set([
            ...fontFamilies,
            ...(await resolveFonts(added, undefined, (registered = []) => registered)),
          ]),
        ];
        const all = [...(loadable ?? []), ...added];
        const read = () => readLoaded(all, families, implicitFamilies);
        // The added families can reach `@font-face` files the earlier reads did not.
        const coverage = await loadReached(
          registry,
          await read(),
          read,
          { tried, fetch: fontFileFetch, signal },
          load(lease),
        );

        return {
          fonts: all,
          fontFamilies: families,
          coverage,
          added,
          implicitFamilies,
        };
      });
    }),
  );

  warnUndrawn(
    settled.coverage?.missingCharacters ?? [],
    settled.coverage?.unresolvedFamilies ?? [],
    failedImages,
  );

  return {
    source,
    layout,
    fonts: settled.fonts && registry.without(settled.fonts, failedFontFiles),
    loadedFonts: [
      ...((listed ? resolvedFonts?.filter((_, id) => cached.has(id)) : resolvedFonts) ?? []),
      ...settled.added,
    ],
    fontFamilies: settled.fontFamilies,
    implicitFamilies: settled.implicitFamilies,
    images: entries,
    fontFiles: [...tried].filter((url) => !failedFontFiles.includes(url)),
    failedFontFiles,
    fontFetch,
    reread,
  };
}

/** `listed` with each font whose key `failed` lists left as a bare key, so a read reaches the family after it. */
function listedWithout(
  listed: ListedFont[] | undefined,
  failed: string[],
): ListedFont[] | undefined {
  if (!listed || failed.length === 0) {
    return listed;
  }

  return listed.map((font) => (failed.includes(font.key) ? { key: font.key } : font));
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
        fontScope: FontScope;
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
            const load: FontLoad = { lease, failed: [...state.failedFontFiles] };

            await registry.loadFontFiles(urls, fetch, load);

            if (load.failed.length === state.failedFontFiles.length) {
              return [];
            }

            // Only another read reaches the next `src` of a rule whose file failed again.
            const tried = new Set([...state.fontFiles, ...load.failed]);
            const first = await state.reread(load.failed);

            await loadReached(registry, first, state.reread, { tried, fetch, signal }, load);

            return load.failed.slice(state.failedFontFiles.length);
          },
        }
      : undefined;

  return registry
    .resolveResources(
      state.fonts && registry.loaders(state.fonts, fetch),
      state.images,
      state.fontFamilies,
      ({ images, fontFamilies, fontScope: { listed, failed = [] } }) => {
        signal?.throwIfAborted();

        return call(
          state.source,
          {
            ...encoding,
            ...state.layout,
            images,
            fontFamilies,
            fontScope: fontScope(listed, [...state.failedFontFiles, ...failed]),
          },
          signal,
        );
      },
      { files, implicitFamilies: state.implicitFamilies },
    )
    .finally(() => registry.dropUnkept(state.loadedFonts));
}
