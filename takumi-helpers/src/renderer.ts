import { type CodepointSource, fontFromUrl, type GenericFontFamily, subsetFonts } from "./fonts";

/**
 * Shared wrapper primitives for the `@takumi-rs/core` (napi) and `@takumi-rs/wasm` renderer
 * bindings. Both backends compose their public `Renderer` from these so the two surfaces cannot
 * drift: the font/image loader shapes, the output-format options, and the font-registration state
 * machine live here once. The concrete `Font`/`ImageSource`/`ByteBuf`/`RegisteredFamily` shapes
 * mirror each backend's generated types field-for-field; the register callback wiring in each
 * backend fails to compile if they diverge.
 */

type Awaitable<T> = T | Promise<T>;

/** Font byte payloads accepted across environments. */
export type ByteBuf = Uint8Array | ArrayBuffer | Buffer;

/** A font descriptor, matching each backend's generated `FontDetails`. */
export type FontDetails = {
  name?: string;
  data: ByteBuf;
  weight?: number;
  style?: "normal" | "italic" | "oblique" | `oblique ${number}deg` | (string & {});
  /**
   * Logical family this font is a coverage subset of. Subsets sharing a `subsetOf` are
   * kept as distinct families and `font-family: {subsetOf}` expands to all of them, so each
   * script routes to the subset that covers it.
   */
  subsetOf?: string;
  /**
   * Where this subset sits in its group's fallback order; lowest is tried first, and equal
   * ranks order by family name. A subset's `cmap` reaches past the range it was cut for, so
   * the rank is what settles which subset serves a codepoint several of them encode.
   */
  subsetRank?: number;
  /**
   * CSS generic family keyword this font resolves for, so stacks ending in e.g.
   * `monospace` reach it without naming the family.
   */
  generic?: GenericFontFamily;
};

/** A registered font, either detailed or raw bytes. */
export type Font = FontDetails | ByteBuf;

/** Cache policy for a decoded image. Defaults to `"auto"`. */
export type ImageCacheMode = "auto" | "none";

/** An image source with its URL and raw bytes, matching each backend's generated `ImageSource`. */
export type ImageSource = {
  src: string;
  data: ByteBuf;
  cache?: ImageCacheMode;
};

/** A font family produced by `registerFont`. Only `name` is read here; backends keep the full type. */
export type RegisteredFamilyLike = { name: string };

/**
 * A font to register. Either a URL string (fetched on demand, with name/weight/style read from the
 * file), raw bytes, or a descriptor with a lazy `data()` loader.
 */
export type FontLoader =
  | string
  | Font
  | (Omit<FontDetails, "data"> & {
      data: () => Promise<FontDetails["data"]> | FontDetails["data"];
      /** Inclusive codepoint ranges this face covers; lets `render` skip it when unused. */
      ranges?: [number, number][];
    } & ({ key: string } | { name: string }));

export type ImageLoader = Omit<ImageSource, "data"> & {
  data: ImageSource["data"] | (() => ImageSource["data"] | Promise<ImageSource["data"]>);
};

/** Images for a render: pre-fetched entries, or a group with a decode-cache default. */
export type ImagesInput =
  | ImageLoader[]
  | {
      /** Pre-fetched entries, same as the array form. */
      sources?: ImageLoader[];
      /** Decode-cache default for every image this render; a source's own `cache` wins. */
      cache?: NonNullable<ImageLoader["cache"]>;
    };

/**
 * Output format. Format-specific options live on the variant that supports them, so `quality`
 * cannot be paired with PNG/ICO/raw, and `lossless` is WebP-only. On native (`@takumi-rs/core`),
 * WebP honors both — `lossless` takes precedence over `quality`, and omitting both encodes
 * losslessly. On wasm (`@takumi-rs/wasm`) WebP is always lossless: `lossless` is effectively always
 * on and `quality` is ignored for WebP (lossy WebP is native-only).
 */
export type OutputFormatOptions =
  | { format?: "png" }
  | { format: "jpeg"; quality?: number }
  | { format: "webp"; quality?: number; lossless?: boolean }
  | { format: "ico" }
  | { format: "raw" };

/**
 * Animation output format. `quality` and `lossless` are WebP-only. On native, `lossless` takes
 * precedence over `quality` and omitting both encodes losslessly; on wasm animated WebP is always
 * lossless and `quality` is ignored.
 */
export type AnimationOutputFormatOptions =
  | { format?: "webp"; quality?: number; lossless?: boolean }
  | { format: "apng" }
  | { format: "gif" };

/** The wrapper-managed extras every render entry point accepts on top of the binding options. */
export type RenderExtras = {
  fonts?: FontLoader[] | Promise<FontLoader[]>;
  signal?: AbortSignal;
  images?: ImagesInput;
};

/** Public `render`/`measure` options for a backend whose binding options are {@link TInternal}. */
export type BackendRenderOptions<TInternal> = Omit<
  TInternal,
  "images" | "format" | "quality" | "lossless"
> &
  OutputFormatOptions &
  RenderExtras;

/** Public `renderAnimation` options for a backend whose binding options are {@link TInternal}. */
export type BackendAnimationOptions<TInternal> = Omit<
  TInternal,
  "images" | "format" | "quality" | "lossless"
> &
  AnimationOutputFormatOptions &
  RenderExtras;

/** Public `renderSvg` options for a backend whose binding options are {@link TInternal}. */
export type BackendSvgOptions<TInternal> = Omit<TInternal, "images"> & RenderExtras;

function isBuffer(data: unknown): data is ByteBuf {
  return (
    data instanceof Uint8Array ||
    data instanceof ArrayBuffer ||
    (typeof Buffer !== "undefined" && Buffer.isBuffer(data))
  );
}

// The native binding copies bare ArrayBuffers but passes typed-array views zero-copy.
function asView(data: ByteBuf): Uint8Array {
  return data instanceof ArrayBuffer ? new Uint8Array(data) : data;
}

function extractFontBuffer(font: Exclude<FontLoader, string>) {
  if (isBuffer(font)) {
    return font;
  }

  if (typeof font.data !== "function") {
    return font.data;
  }

  return font.data();
}

function createFontKey(font: Exclude<FontLoader, string>): string | ByteBuf {
  if (isBuffer(font)) {
    return font;
  }

  if ("key" in font && font.key) {
    return font.key;
  }

  if (typeof font.data === "function") {
    return `${font.name ?? ""}:${font.weight ?? ""}:${font.style ?? ""}`;
  }

  return font.data;
}

/** Normalize an {@link ImagesInput} into concrete, deduped {@link ImageSource} entries. */
export async function resolveImageLoaders(images: ImagesInput): Promise<ImageSource[]> {
  const { sources = [], cache } = Array.isArray(images) ? { sources: images } : images;
  const bySrc = new Map<string, ImageLoader>();

  for (const image of sources) {
    bySrc.set(image.src, image);
  }

  return Promise.all(
    [...bySrc.values()].map(async ({ src, data, cache: own }) => ({
      src,
      data: asView(typeof data === "function" ? await data() : data),
      cache: own ?? cache,
    })),
  );
}

/**
 * Font-registration state machine shared by both backends: dedupes registrations by a stable key
 * (or by buffer identity), resolves lazy `data()` loaders once, and turns a set of {@link FontLoader}s
 * plus an {@link ImagesInput} into the `fontFamilies`/`images` the binding expects.
 *
 * `registerInner` is the backend's raw `registerFont`; its precise `RegisteredFamily` type flows
 * back out through {@link TFamily}.
 */
export class FontRegistry<TFamily extends RegisteredFamilyLike> {
  private readonly byKey = new Map<string, Promise<TFamily[]>>();
  private readonly byData = new WeakMap<ByteBuf, Promise<TFamily[]>>();

  constructor(private readonly registerInner: (font: Font) => Awaitable<TFamily[]>) {}

  private getFont(key: string | ByteBuf) {
    return typeof key === "string" ? this.byKey.get(key) : this.byData.get(key);
  }

  private setFont(key: string | ByteBuf, family: Promise<TFamily[]>) {
    if (typeof key === "string") {
      this.byKey.set(key, family);
    } else {
      this.byData.set(key, family);
    }
  }

  private deleteFont(key: string | ByteBuf) {
    if (typeof key === "string") {
      this.byKey.delete(key);
    } else {
      this.byData.delete(key);
    }
  }

  /** Registers one font, deduped against earlier registrations. */
  async register(font: FontLoader): Promise<TFamily[]> {
    const loader = typeof font === "string" ? fontFromUrl(font) : font;
    const key = createFontKey(loader);

    const cached = this.getFont(key);
    if (cached) {
      return cached;
    }

    const extracted = extractFontBuffer(loader);

    const promise = Promise.resolve(extracted)
      .then((data) => {
        const view = asView(data);

        return this.registerInner(isBuffer(loader) ? view : { ...loader, data: view });
      })
      .catch((error) => {
        this.deleteFont(key);
        throw error;
      });

    this.setFont(key, promise);

    return promise;
  }

  /** Registers every font and returns the distinct family names produced. */
  async prepareFonts(fonts: FontLoader[] | undefined): Promise<string[] | undefined> {
    if (!fonts) {
      return;
    }

    const families = await Promise.all(fonts.map((font) => this.register(font)));

    return [...new Set(families.flat().map((family) => family.name))];
  }

  /**
   * Registers `fonts` and resolves lazy `images`, yielding the `images`/`fontFamilies` the binding
   * expects. Explicit `fontFamilies` wins over the registered set.
   */
  async resolveResources(
    fonts: FontLoader[] | undefined,
    images: ImagesInput | undefined,
    fontFamilies: string[] | undefined,
  ): Promise<{ images?: ImageSource[]; fontFamilies?: string[] }> {
    const registeredFamilies = await this.prepareFonts(fonts);

    return {
      images: images ? await resolveImageLoaders(images) : undefined,
      fontFamilies: fontFamilies ?? registeredFamilies,
    };
  }
}

/** The binding options {@link prepareRenderInput} yields: extras stripped, resolved resources merged in. */
export type ResolvedRenderOptions<TOptions> = Omit<
  TOptions,
  keyof RenderExtras | "fontFamilies"
> & {
  images?: ImageSource[];
  fontFamilies?: string[];
};

/**
 * Shared body for every backend render entry point: subsets and registers `fonts` against `source`,
 * resolves `images`/`fontFamilies`, and enforces one abort policy: `signal` is checked before and
 * after the (network-bound) resource resolution. Returns the binding options plus the `signal` for
 * the backend to forward into a native call when it supports cancellation.
 */
export async function prepareRenderInput<
  TOptions extends RenderExtras & { fontFamilies?: string[] },
  TFamily extends RegisteredFamilyLike,
>(
  registry: FontRegistry<TFamily>,
  options: TOptions,
  source: CodepointSource,
): Promise<{ options: ResolvedRenderOptions<TOptions>; signal: AbortSignal | undefined }> {
  const { fonts, fontFamilies, signal, images, ...rest } = options;
  signal?.throwIfAborted();

  const resolvedFonts = await fonts;
  const resolved = await registry.resolveResources(
    resolvedFonts && subsetFonts({ fonts: resolvedFonts, source }),
    images,
    fontFamilies,
  );
  signal?.throwIfAborted();

  return { options: { ...rest, ...resolved }, signal };
}
