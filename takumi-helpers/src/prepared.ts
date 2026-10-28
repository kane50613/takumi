import type { FetchOptions } from "./fetch";
import { type ImageFetchCache, prepareImages, type PrepareImagesOptions } from "./images";
import {
  type ByteBuf,
  type FontDetails,
  type FontLoader,
  type FontRegistry,
  fontsToLoad,
  type ImageFont,
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

/**
 * An {@link ImageFont} as `prepare` loads it: the images of the clusters the source draws from it
 * that `glyphs` lacks come from `load`.
 */
export type ImageFontLoader = ImageFont & {
  /**
   * The image of `cluster`: a URL, which `prepare` fetches with the fetch options of `images`, or
   * its bytes. A cluster without one, or whose fetch fails, draws from the next font.
   */
  load?: (cluster: string) => Awaitable<string | ByteBuf | undefined>;
};

/** What `prepare` takes on top of the layout options. */
export type PrepareExtras = {
  fonts?: FontLoader[] | Promise<FontLoader[]>;
  images?: PrepareImagesInput;
  /** Faces whose glyphs are images, one per grapheme cluster. */
  imageFonts?: ImageFontLoader[];
  signal?: AbortSignal;
};

/** Public `prepare` options for a backend whose binding options are {@link TInternal}. */
export type BackendPrepareOptions<TInternal> = Omit<
  TInternal,
  "images" | "imageFonts" | OutputOnly
> &
  PrepareExtras;

/** Public options for rendering a {@link Prepared} on a backend whose binding options are {@link TInternal}. */
export type BackendPreparedRenderOptions<TInternal> = Pick<
  TInternal,
  Extract<keyof TInternal, "dithering" | "drawDebugBorder">
> &
  OutputFormatOptions & { signal?: AbortSignal };

/** The grapheme clusters a render would draw from an image font that has no image for them. */
export type ImageFontNeeds = { name: string; clusters: string[] };

/** What a render reads from a source before it lays it out. */
export type SourceNeeds = {
  images: string[];
  characters: string;
  faces: number[];
  glyphs: ImageFontNeeds[];
};

/**
 * A font a render may load, described before its bytes arrive. Font matching takes it as a face
 * still loading, and `needs` reports its `id` when the render would load it.
 */
export type PendingFont = Pick<FontDetails, "weight" | "style" | "unicodeRange"> & {
  id: number;
  name: string;
};

/** What a needs pass reads a source with besides its layout. */
export type NeedsRequest = {
  /** The SVG images a render will draw, whose `text` elements count too. */
  images?: ImageSource[];
  /** Fonts still loading, whose ids it reports when font matching would load them. */
  pending?: PendingFont[];
  /** The font stack the render falls back through. */
  fontFamilies?: string[];
  /** The image fonts the render registers, whose missing clusters it reports. */
  imageFonts?: ImageFont[];
};

/** Reads what a source needs after the cascade. */
export type SourceReader = (request: NeedsRequest) => Awaitable<SourceNeeds>;

/** The options and pending fonts a binding's `needs` takes for `request` on top of `layout`. */
export function needsArgs<TLayout>(
  layout: TLayout,
  { images, pending, fontFamilies, imageFonts }: NeedsRequest,
): [
  TLayout & { images?: ImageSource[]; fontFamilies?: string[]; imageFonts?: ImageFont[] },
  PendingFont[] | undefined,
] {
  return [{ ...layout, images, fontFamilies, ...(imageFonts && { imageFonts }) }, pending];
}

/** What a render of several sources, such as a PDF's bands or an animation's scenes, needs. */
export function joinNeeds(reads: SourceNeeds[]): SourceNeeds {
  return {
    images: reads.flatMap((read) => read.images),
    characters: reads.map((read) => read.characters).join(""),
    faces: reads.flatMap((read) => read.faces),
    glyphs: reads.flatMap((read) => read.glyphs),
  };
}

type PreparedState<TSource, TLayout> = {
  source: TSource;
  layout: TLayout;
  fonts: FontLoader[] | undefined;
  fontFamilies: string[] | undefined;
  images: ImageSource[];
  imageFonts: ImageFont[] | undefined;
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
 * Reads what `source` needs, then loads the font subsets its text uses and fetches the images it
 * draws that `extras.images` does not supply. `needs` reads the source after the cascade under
 * `layout`, the binding options it will render with.
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
 * {@link prepareSource} without fetching: the images are the ones `extras.images` supplies, and
 * `needs` runs only when a font subset depends on the characters.
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

async function collect<TFamily extends RegisteredFamilyLike, TSource, TLayout>(
  registry: FontRegistry<TFamily>,
  source: TSource,
  layout: TLayout,
  extras: PrepareExtras & { fontFamilies?: string[] },
  needs: SourceReader,
  fetchMissing: boolean,
): Promise<PreparedState<TSource, TLayout>> {
  const { fonts, fontFamilies, images = [], imageFonts, signal } = extras;

  signal?.throwIfAborted();

  const resolvedFonts = await fonts;
  const pending = resolvedFonts && registry.pendingFonts(resolvedFonts);
  const request: NeedsRequest = {
    pending,
    fontFamilies: fontFamilies ?? (pending && resolvedFonts && registry.familiesOf(resolvedFonts)),
    imageFonts: imageFonts?.map(({ name }) => ({ name })),
  };
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
  const loadedImageFonts =
    imageFonts &&
    (fetchMissing && imageFonts.some((imageFont) => imageFont.load)
      ? await loadImageFonts(imageFonts, (await read()).glyphs, {
          ...fetchOptions,
          fetchCache,
          signal,
        })
      : imageFonts.map(({ name, glyphs }) => ({ name, glyphs })));
  const svgs = entries.filter((image) => isSvg(image.data));
  const subset =
    resolvedFonts &&
    (pending
      ? fontsToLoad(
          resolvedFonts,
          pending,
          (await (svgs.length > 0 ? needs({ ...request, images: svgs }) : read())).faces,
        )
      : resolvedFonts);

  return {
    source,
    layout,
    fonts: subset,
    fontFamilies,
    images: entries,
    imageFonts: loadedImageFonts,
  };
}

/**
 * Each of `imageFonts` with the images of the clusters `needs` reports it lacks, from its `load`.
 * A URL fetches with `fetchOptions`, and a cluster whose fetch fails is left out, so it draws from
 * the next font.
 */
export async function loadImageFonts(
  imageFonts: ImageFontLoader[],
  needs: ImageFontNeeds[],
  fetchOptions: Omit<PrepareImagesOptions, "urls" | "node" | "sources" | "throwOnError">,
): Promise<ImageFont[]> {
  return Promise.all(
    imageFonts.map(async ({ name, glyphs = [], load }) => {
      const held = new Set(glyphs.map((glyph) => glyph.cluster));
      const clusters = new Set(
        needs
          .filter((needed) => needed.name === name)
          .flatMap((needed) => needed.clusters)
          .filter((cluster) => !held.has(cluster)),
      );

      if (!load || clusters.size === 0) {
        return { name, glyphs };
      }

      const sources = await Promise.all(
        [...clusters].map(async (cluster) => ({ cluster, source: await load(cluster) })),
      );
      const fetched = new Map(
        (
          await prepareImages({
            ...fetchOptions,
            urls: sources.flatMap(({ source }) => (typeof source === "string" ? [source] : [])),
            throwOnError: false,
          })
        ).map((image) => [image.src, image.data]),
      );
      const loaded = sources.flatMap(({ cluster, source }) => {
        const data = typeof source === "string" ? fetched.get(source) : source;

        return data ? [{ cluster, data }] : [];
      });

      return { name, glyphs: [...glyphs, ...loaded] };
    }),
  );
}

/**
 * Whether `data` holds SVG markup, whose `text` elements draw with the render's fonts. Approximate:
 * it looks for `<svg` in the first 4 KiB only, so a document with a longer prolog reads as raster.
 */
function isSvg(data: ImageSource["data"]): boolean {
  const view = data instanceof ArrayBuffer ? new Uint8Array(data) : data;

  return new TextDecoder().decode(view.subarray(0, 4096)).includes("<svg");
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
      Omit<TOutput, "signal"> & {
        images?: ImageSource[];
        fontFamilies?: string[];
        imageFonts?: ImageFont[];
      },
    signal: AbortSignal | undefined,
  ) => Awaitable<TResult>,
): Promise<TResult> {
  const state = stateOf(prepared);
  const { signal, ...encoding } = output;

  signal?.throwIfAborted();

  return registry.resolveResources(state.fonts, state.images, state.fontFamilies, (resolved) => {
    signal?.throwIfAborted();

    return call(
      state.source,
      {
        ...encoding,
        ...state.layout,
        ...resolved,
        ...(state.imageFonts && { imageFonts: state.imageFonts }),
      },
      signal,
    );
  });
}
