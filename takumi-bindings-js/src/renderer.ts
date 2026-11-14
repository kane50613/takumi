import { type FontCacheMode, fontFromUrl, type GenericFontFamily } from "@takumi-rs/helpers";
import type { JsonSource } from "./node-source";
import type { ListedFont, PrepareExtras } from "./prepared";
import { RendererStore } from "./store";
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

/** Whether a renderer keeps a font a render's `fonts` list loaded once the render settles. */
type FontCachePolicy = {
  /**
   * `"none"` lets the font go once the render that loaded it settles, though a render still uses
   * one the renderer already holds. `registerFont` keeps a font whatever this says.
   * @default "auto"
   */
  cache?: FontCacheMode;
};

/**
 * A font to register. Either a URL string (fetched on demand, with name/weight/style read from the
 * file), raw bytes, or a descriptor with a lazy `data()` loader.
 * @internal
 */
export type FontLoader =
  | string
  | ByteBuf
  | (FontDetails & FontCachePolicy)
  | (Omit<FontDetails, "data"> &
      FontCachePolicy & {
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

function createFontKey(font: Exclude<FontLoader, string>): string | ByteBuf {
  if (isBuffer(font)) {
    return font;
  }

  if ("key" in font && font.key) {
    return font.key;
  }

  if (typeof font.data === "function") {
    return faceKey(font);
  }

  return font.data;
}

/** The face descriptors `font` registers under, which a key must include to keep each face. */
function faceKey(font: Exclude<FontLoader, string | ByteBuf>): string {
  const ranges = unicodeRangeOf(font)
    .map(([start, end]) => `${start}-${end}`)
    .join(",");

  return [
    font.subsetOf ?? font.name ?? "",
    font.weight ?? "",
    font.style ?? "",
    ranges,
    font.generic ?? "",
  ].join(":");
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

/**
 * What a {@link FontRegistry} needs from a backend: its registration, and the cache behind it. The
 * cache evicts one font file at a time, and a render leases the files it uses until it settles.
 *
 * @internal
 */
export type FontBackend<TFamily extends RegisteredFamilyLike> = {
  /** Registers a font under `key`; it is never evicted. */
  registerFont(font: Font, key: string): Awaitable<TFamily[]>;
  /** Registers a font a render's `fonts` list loaded under `key`, its file leased to `lease`. */
  cacheFont(font: Font, key: string, lease: number): Awaitable<TFamily[]>;
  /**
   * The families a registration under `key` added while the backend holds its file. `pin` keeps the
   * file for good, and `lease` leases it.
   */
  cachedFonts(key: string, pin: boolean, lease?: number): TFamily[] | null | undefined;
  /** Loads the `@font-face` file `url` names, leased to `lease`. */
  cacheFontFile(url: string, data: ArrayBuffer | Uint8Array, lease: number): Awaitable<void>;
  /** Whether the backend holds the `@font-face` file `url`, leasing it to `lease` when it does. */
  cachedFontFile(url: string, lease: number): boolean;
  /** Opens a set of leases, which keeps the files a render uses from eviction until `releaseFonts`. */
  leaseFonts(): number;
  /** Releases the leases `leaseFonts` opened, so the backend can evict their files. */
  releaseFonts(lease: number): void;
  /** Gives the faces registered under `keys` precedence in their families, a later key over an earlier one. */
  orderFonts(keys: string[]): void;
  /** Forgets the fonts registered under `keys` that no render leases, returning those keys and the ones nothing is under. */
  dropFonts(keys: string[]): string[];
};

/**
 * The `@font-face` files a prepared render loaded, and how to load the ones evicted since.
 *
 * @internal
 */
export type HeldFontFiles = {
  urls: string[];
  /** Loads `urls` leased to `lease`, and the next sources their rules reach when one fails, returning the ones that failed. */
  load(urls: string[], lease: number): Promise<string[]>;
};

/** The resources a render uses, as the binding expects them. */
type ResolvedResources = {
  images?: ImageSource[];
  fontFamilies?: string[];
  /** The held `@font-face` files that failed to load again. */
  failedFontFiles?: string[];
};

/**
 * A 53-bit hash of `bytes` with their length, after
 * [cyrb53](https://github.com/bryc/code/blob/master/jshash/experimental/cyrb53.js).
 */
function hashBytes(bytes: Uint8Array): string {
  let h1 = 0xdeadbeef;
  let h2 = 0x41c6ce57;

  for (let index = 0; index < bytes.length; index++) {
    const byte = bytes[index] ?? 0;

    h1 = Math.imul(h1 ^ byte, 2654435761);
    h2 = Math.imul(h2 ^ byte, 1597334677);
  }

  h1 = Math.imul(h1 ^ (h1 >>> 16), 2246822507);
  h1 ^= Math.imul(h2 ^ (h2 >>> 13), 3266489909);
  h2 = Math.imul(h2 ^ (h2 >>> 16), 2246822507);
  h2 ^= Math.imul(h1 ^ (h1 >>> 13), 3266489909);

  return `${bytes.length}:${(4294967296 * (2097151 & h2) + (h1 >>> 0)).toString(36)}`;
}

/**
 * Font registration shared by every backend: asks the backend whether it still holds a font before
 * loading it, loads a font once however many renders ask for it at the same time, and turns a set
 * of {@link FontLoader}s plus an {@link ImagesInput} into the `fontFamilies`/`images` the binding
 * expects. The backend keeps the fonts and evicts them; this keeps no record of its own.
 * @internal
 */
export class FontRegistry<TFamily extends RegisteredFamilyLike> {
  private readonly loading = new Map<string, Promise<TFamily[]>>();
  /** A key for each buffer passed as a font, by its bytes, since the backend only takes string keys. */
  private readonly bufferKeys = new WeakMap<ByteBuf, string>();
  /** The keys of fonts loaded with `cache: "none"` that no render has asked to keep since. */
  private readonly transient = new Set<string>();
  /** The keys of `transient` whose renders settled, forgotten once no render leases their files. */
  private readonly unkept = new Set<string>();
  /** The leases `leased` holds open. */
  private readonly open = new Set<number>();

  constructor(
    private readonly backend: FontBackend<TFamily>,
    /** The store the renderer keeps the bytes it downloads in. */
    readonly store: RendererStore = new RendererStore(),
  ) {}

  /**
   * Registers one font unless the backend still holds it, its file leased to the render that
   * `lease` names. Without a lease, as `registerFont` registers, it is never evicted.
   */
  async register(font: FontLoader, lease?: number): Promise<TFamily[]> {
    const loader = this.toLoader(font);
    const key = this.keyOf(loader);
    const pinned = lease === undefined;
    const keeps = pinned || isBuffer(loader) || loader.cache !== "none";
    const held = this.backend.cachedFonts(key, pinned, lease);

    if (held) {
      if (keeps) {
        this.keep(key);
      }

      return held;
    }

    const pending = this.loading.get(key);

    if (pending) {
      // The render loading it leases its file until that render settles, after this continues.
      return pending.then(() => this.register(loader, lease));
    }

    const load = Promise.resolve(extractFontBuffer(loader))
      .then((data) => {
        const view = asView(data);
        const input = isBuffer(loader) ? view : { ...loader, data: view };

        return lease === undefined
          ? this.backend.registerFont(input, key)
          : this.backend.cacheFont(input, key, lease);
      })
      .then((families) => {
        if (keeps) {
          this.keep(key);
        } else {
          this.transient.add(key);

          // The render that loaded it failed before the load finished, and dropped its fonts already.
          if (lease !== undefined && !this.open.has(lease)) {
            this.unkept.add(key);
            this.dropSettled();
          }
        }

        return families;
      })
      .finally(() => this.loading.delete(key));

    this.loading.set(key, load);

    return load;
  }

  /** Loads the `@font-face` file `url` names, leased to `lease`. A wasm backend reads only a `Uint8Array`. */
  async cacheFontFile(url: string, data: ArrayBuffer | Uint8Array, lease: number): Promise<void> {
    await this.backend.cacheFontFile(url, asView(data), lease);
  }

  /** Loads each of `fonts` the backend does not hold, leased to `lease`, and returns whether any was loaded. */
  async cache(fonts: FontLoader[], lease: number): Promise<boolean> {
    const loaders = fonts
      .map((font) => this.toLoader(font))
      .filter((loader) => !this.backend.cachedFonts(this.keyOf(loader), false, lease));

    await Promise.all(loaders.map((loader) => this.register(loader, lease)));

    return loaders.length > 0;
  }

  /** Calls `use` with a set of leases, released once what it returns settles. */
  async leased<T>(use: (lease: number) => Awaitable<T>): Promise<T> {
    const lease = this.backend.leaseFonts();

    this.open.add(lease);

    try {
      return await use(lease);
    } finally {
      this.open.delete(lease);
      this.backend.releaseFonts(lease);
    }
  }

  /** Calls `use`, and forgets the fonts of `fonts` loaded with `cache: "none"` when it fails. */
  async dropOnFailure<T>(fonts: FontLoader[] | undefined, use: () => Promise<T>): Promise<T> {
    try {
      return await use();
    } catch (error) {
      this.dropUnkept(fonts);
      throw error;
    }
  }

  /**
   * Forgets the fonts of `fonts` loaded with `cache: "none"` that no render asked to keep, once no
   * render leases their files. A render calls it after it settles, which also forgets the ones an
   * earlier render left because another render leased them then.
   */
  dropUnkept(fonts: FontLoader[] | undefined): void {
    for (const font of fonts ?? []) {
      const key = this.keyOf(this.toLoader(font));

      if (this.transient.has(key)) {
        this.unkept.add(key);
      }
    }

    this.dropSettled();
  }

  /** Forgets the fonts under `unkept` that no render leases, leaving the others for a later call. */
  private dropSettled() {
    if (this.unkept.size === 0) {
      return;
    }

    for (const key of this.backend.dropFonts([...this.unkept])) {
      this.transient.delete(key);
      this.unkept.delete(key);
    }
  }

  /** Marks the font under `key` as one a render asked to keep. */
  private keep(key: string) {
    this.transient.delete(key);
    this.unkept.delete(key);
  }

  /** `fonts` described for a backend's needs pass. None when no font declares a `unicodeRange`. */
  listed(fonts: FontLoader[]): ListedFont[] | undefined {
    const loaders = fonts.map((font) => this.toLoader(font));

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

  /** `font` with a URL string turned into a loader that downloads through the renderer's store. */
  private toLoader(font: FontLoader): Exclude<FontLoader, string> {
    return typeof font === "string" ? fontFromUrl(font, { store: this.store.current }) : font;
  }

  private keyOf(loader: Exclude<FontLoader, string>) {
    const key = createFontKey(loader);

    if (typeof key === "string") {
      return key;
    }

    const buffer = this.bufferKeyOf(key);

    return isBuffer(loader) ? buffer : `${buffer}:${faceKey(loader)}`;
  }

  private bufferKeyOf(buffer: ByteBuf) {
    const known = this.bufferKeys.get(buffer);

    if (known) {
      return known;
    }

    const assigned = `buffer:${hashBytes(asView(buffer))}`;

    this.bufferKeys.set(buffer, assigned);

    return assigned;
  }

  /** Leases each of the `@font-face` files `urls` the backend still holds to `lease`, returning the others. */
  leaseFontFiles(urls: string[], lease: number): string[] {
    return urls.filter((url) => !this.backend.cachedFontFile(url, lease));
  }

  /** Leases each held file the backend still holds, and loads the others again, returning the ones that failed. */
  private async holdFiles({ urls, load }: HeldFontFiles, lease: number): Promise<string[]> {
    const evicted = this.leaseFontFiles(urls, lease);

    return evicted.length > 0 ? load(evicted, lease) : [];
  }

  /**
   * Registers `fonts`, resolves lazy `images`, and leases the `@font-face` `files` a prepared render
   * loaded, then calls `use` with the `images` and `fontFamilies` the binding expects. Every font
   * stays leased until what `use` returns settles. Explicit `fontFamilies` wins over the registered
   * set.
   */
  async resolveResources<T>(
    fonts: FontLoader[] | undefined,
    images: ImagesInput | undefined,
    fontFamilies: string[] | undefined,
    use: (resources: ResolvedResources) => Awaitable<T>,
    files?: HeldFontFiles,
  ): Promise<T> {
    const resolvedImages = images ? await resolveImageLoaders(images) : undefined;
    const loaders = fonts?.map((font) => this.toLoader(font)) ?? [];

    return this.leased(async (lease) => {
      const families = await Promise.all(loaders.map((loader) => this.register(loader, lease)));
      const failedFontFiles = files ? await this.holdFiles(files, lease) : [];
      const registered = fonts && [...new Set(families.flat().map((family) => family.name))];

      // Nothing yields from here into `use`, so the order stands when the render takes its fonts.
      if (loaders.length > 0) {
        this.backend.orderFonts(loaders.map((loader) => this.keyOf(loader)));
      }

      return use({
        images: resolvedImages,
        fontFamilies: fontFamilies ?? registered,
        ...(failedFontFiles.length > 0 ? { failedFontFiles } : {}),
      });
    });
  }
}
