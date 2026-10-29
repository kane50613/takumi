import { fontFromUrl, type GenericFontFamily } from "@takumi-rs/helpers";
import type { JsonSource } from "./node-source";
import type { ListedFont, PrepareExtras } from "./prepared";
import { unicodeRangeOf } from "./unicode-range";

/**
 * Shared wrapper primitives for the `@takumi-rs/core` (napi) and `@takumi-rs/wasm` renderer
 * bindings. Both backends compose their public `Renderer` from these so the two surfaces cannot
 * drift: the font/image loader shapes, the output-format options, and the font-registration state
 * machine live here once. The concrete `Font`/`ImageSource`/`ByteBuf`/`RegisteredFamily` shapes
 * mirror each backend's generated types field-for-field; the register callback wiring in each
 * backend fails to compile if they diverge.
 */

type Awaitable<T> = T | Promise<T>;

/**
 * Font byte payloads accepted across environments.
 *
 * @internal
 */
export type ByteBuf = Uint8Array | ArrayBuffer | Buffer;

/**
 * A font descriptor, matching each backend's generated `FontDetails`.
 *
 * @internal
 */
export type FontDetails = {
  name?: string;
  data: ByteBuf;
  weight?: number;
  style?: "normal" | "italic" | "oblique" | `oblique ${number}deg` | (string & {});
  /**
   * Inclusive codepoint ranges the font serves, as `@font-face` `unicode-range`. Register each
   * subset of a family under the same `name` with its own ranges.
   */
  unicodeRange?: [number, number][];
  /** @deprecated Register under `name` with `unicodeRange` instead. */
  subsetOf?: string;
  /** @deprecated Ignored; `unicodeRange` decides which subset serves a codepoint. */
  subsetRank?: number;
  /**
   * CSS generic family keyword this font resolves for, so stacks ending in e.g.
   * `monospace` reach it without naming the family.
   */
  generic?: GenericFontFamily;
};

/**
 * A registered font, either detailed or raw bytes.
 *
 * @internal
 */
export type Font = FontDetails | ByteBuf;

/** Cache policy for a decoded image. Defaults to `"auto"`. */
type ImageCacheMode = "auto" | "none";

/**
 * An image source with its URL and raw bytes, matching each backend's generated `ImageSource`.
 *
 * @internal
 */
export type ImageSource = {
  src: string;
  data: ByteBuf;
  cache?: ImageCacheMode;
};

/**
 * A font family produced by `registerFont`. Only `name` is read here; backends keep the full type.
 *
 * @internal
 */
export type RegisteredFamilyLike = { name: string };

/**
 * A font to register. Either a URL string (fetched on demand, with name/weight/style read from the
 * file), raw bytes, or a descriptor with a lazy `data()` loader.
 * @internal
 */
export type FontLoader =
  | string
  | Font
  | (Omit<FontDetails, "data"> & {
      data: () => Promise<FontDetails["data"]> | FontDetails["data"];
      /** @deprecated Use `unicodeRange`. */
      ranges?: [number, number][];
    } & ({ key: string } | { name: string }));

/** @internal */
export type ImageLoader = Omit<ImageSource, "data"> & {
  data: ImageSource["data"] | (() => ImageSource["data"] | Promise<ImageSource["data"]>);
};

/**
 * Images for a render: pre-fetched entries, or a group with a decode-cache default.
 *
 * @internal
 */
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
 * @internal
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
 * @internal
 */
export type AnimationOutputFormatOptions =
  | { format?: "webp"; quality?: number; lossless?: boolean }
  | { format: "apng" }
  | { format: "gif" };

/** The wrapper-managed extras every render entry point accepts on top of the binding options. */
type RenderExtras = {
  fonts?: FontLoader[] | Promise<FontLoader[]>;
  signal?: AbortSignal;
  images?: ImagesInput;
};

/**
 * Public `render`/`measure` options for a backend whose binding options are {@link TInternal}.
 *
 * @internal
 */
export type BackendRenderOptions<TInternal> = Omit<
  TInternal,
  "images" | "format" | "quality" | "lossless"
> &
  OutputFormatOptions &
  RenderExtras;

/** An animation's scenes and options, besides what a render loads, for a backend whose binding options are {@link TInternal}. */
type AnimationLayout<TInternal extends { scenes: { node: unknown }[] }> = Omit<
  TInternal,
  "images" | "format" | "quality" | "lossless" | "scenes"
> & {
  /** The scenes to render in order, each a node tree or HTML markup. */
  scenes: (Omit<TInternal["scenes"][number], "node"> & {
    node: Exclude<TInternal["scenes"][number]["node"], JsonSource>;
  })[];
} & AnimationOutputFormatOptions;

/**
 * Public `renderAnimation` options for a backend whose binding options are {@link TInternal}.
 *
 * @internal
 */
export type BackendAnimationOptions<TInternal extends { scenes: { node: unknown }[] }> =
  AnimationLayout<TInternal> & RenderExtras;

/**
 * Public `prepareAnimation` options for a backend whose binding options are {@link TInternal}.
 *
 * @internal
 */
export type BackendPrepareAnimationOptions<TInternal extends { scenes: { node: unknown }[] }> =
  AnimationLayout<TInternal> & PrepareExtras;

/**
 * Public `renderSvg` options for a backend whose binding options are {@link TInternal}.
 *
 * @internal
 */
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

const toLoader = (font: FontLoader) => (typeof font === "string" ? fontFromUrl(font) : font);

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

/**
 * Normalize an {@link ImagesInput} into concrete, deduped {@link ImageSource} entries.
 *
 * @internal
 */
export async function resolveImageLoaders(images: ImagesInput): Promise<ImageSource[]> {
  const { sources = [], cache } = Array.isArray(images) ? { sources: images } : images;
  const bySrc = new Map(sources.map((image) => [image.src, image]));

  return Promise.all(
    [...bySrc.values()].map(async ({ src, data, cache: own }) => ({
      src,
      data: asView(typeof data === "function" ? await data() : data),
      cache: own ?? cache,
    })),
  );
}

/** How many times a render registers its fonts before it renders with whichever the backend still holds. */
const REGISTER_ATTEMPTS = 3;

/**
 * What a {@link FontRegistry} needs from a backend: its registration, and the cache behind it.
 *
 * @internal
 */
export type FontBackend<TFamily extends RegisteredFamilyLike> = {
  /** Registers a font under `key`; it is never released for space. */
  registerFont(font: Font, key: string): Awaitable<TFamily[]>;
  /** Registers a font a render's `fonts` list loaded under `key`, released for space past the budget. */
  cacheFont(font: Font, key: string): Awaitable<TFamily[]>;
  /** The families a registration under `key` added while the backend holds them; `pin` keeps them. */
  cachedFonts(key: string, pin: boolean): TFamily[] | null | undefined;
};

/**
 * Font registration shared by every backend: asks the backend whether it still holds a font before
 * loading it, loads a font once however many renders ask for it at the same time, and turns a set
 * of {@link FontLoader}s plus an {@link ImagesInput} into the `fontFamilies`/`images` the binding
 * expects. The backend keeps the fonts and releases them; this keeps no record of its own.
 * @internal
 */
export class FontRegistry<TFamily extends RegisteredFamilyLike> {
  private readonly loading = new Map<string, Promise<TFamily[]>>();
  /** A key for each buffer passed as a font, since the backend only takes string keys. */
  private readonly bufferKeys = new WeakMap<ByteBuf, string>();
  private buffers = 0;

  constructor(private readonly backend: FontBackend<TFamily>) {}

  /**
   * Registers one font unless the backend still holds it. A `pinned` font, as `registerFont`
   * registers, is never released for space.
   */
  async register(font: FontLoader, pinned = true): Promise<TFamily[]> {
    const loader = toLoader(font);
    const key = this.keyOf(loader);
    const held = this.backend.cachedFonts(key, pinned);

    if (held) {
      return held;
    }

    const pending = this.loading.get(key);

    if (pending) {
      return pinned
        ? pending.then(() => this.backend.cachedFonts(key, true) ?? this.register(loader))
        : pending;
    }

    const load = Promise.resolve(extractFontBuffer(loader))
      .then((data) => {
        const view = asView(data);
        const input = isBuffer(loader) ? view : { ...loader, data: view };

        return pinned ? this.backend.registerFont(input, key) : this.backend.cacheFont(input, key);
      })
      .finally(() => this.loading.delete(key));

    this.loading.set(key, load);

    return load;
  }

  /** `fonts` described for a backend's needs pass. None when no font declares a `unicodeRange`. */
  listed(fonts: FontLoader[]): ListedFont[] | undefined {
    const loaders = fonts.map(toLoader);

    if (!loaders.some((loader) => unicodeRangeOf(loader).length > 0)) {
      return undefined;
    }

    return loaders.map((loader) => {
      const key = this.keyOf(loader);

      if (isBuffer(loader) || !("name" in loader)) {
        return { key };
      }

      const { name, weight, style } = loader;

      return { key, name, weight, style, unicodeRange: unicodeRangeOf(loader) };
    });
  }

  private keyOf(loader: Exclude<FontLoader, string>) {
    const key = createFontKey(loader);

    if (typeof key === "string") {
      return key;
    }

    const known = this.bufferKeys.get(key);

    if (known) {
      return known;
    }

    const assigned = `buffer:${++this.buffers}`;

    this.bufferKeys.set(key, assigned);

    return assigned;
  }

  /**
   * Registers `fonts` and resolves lazy `images`, then calls `use` with the `images` and
   * `fontFamilies` the binding expects while the backend holds every font. Explicit `fontFamilies`
   * wins over the registered set.
   */
  async resolveResources<T>(
    fonts: FontLoader[] | undefined,
    images: ImagesInput | undefined,
    fontFamilies: string[] | undefined,
    use: (resources: { images?: ImageSource[]; fontFamilies?: string[] }) => Awaitable<T>,
  ): Promise<T> {
    const resolvedImages = images ? await resolveImageLoaders(images) : undefined;
    const loaders = fonts?.map(toLoader) ?? [];

    for (let attempt = 1; ; attempt++) {
      const families = await Promise.all(loaders.map((loader) => this.register(loader, false)));

      // Nothing yields from this check into `use`, and backends only release fonts on this
      // thread, so no other render can release these fonts before the render takes them.
      if (
        attempt === REGISTER_ATTEMPTS ||
        loaders.every((loader) => this.backend.cachedFonts(this.keyOf(loader), false))
      ) {
        const registered = fonts && [...new Set(families.flat().map((family) => family.name))];

        return use({ images: resolvedImages, fontFamilies: fontFamilies ?? registered });
      }
    }
  }
}
