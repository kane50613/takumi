import {
  type CodepointSource,
  fontFromUrl,
  type GenericFontFamily,
  LIST_MARKER_CHARACTERS,
  subsetFonts,
} from "./fonts";
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

/** Font byte payloads accepted across environments. */
export type ByteBuf = Uint8Array | ArrayBuffer | Buffer;

/** A font descriptor, matching each backend's generated `FontDetails`. */
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
      /** @deprecated Use `unicodeRange`. */
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
  const bySrc = new Map(sources.map((image) => [image.src, image]));

  return Promise.all(
    [...bySrc.values()].map(async ({ src, data, cache: own }) => ({
      src,
      data: asView(typeof data === "function" ? await data() : data),
      cache: own ?? cache,
    })),
  );
}

/** The renderer option that bounds the fonts renders' `fonts` lists keep registered. */
export type FontCacheRendererOptions = {
  /**
   * Font file bytes that renders' `fonts` lists may keep registered; past it, the least recently
   * used families are released. Fonts from `registerFont` and fonts a running render lists stay.
   * @default Infinity
   */
  fontCacheMaxBytes?: number;
};

/** How a {@link FontRegistry} releases fonts that renders' `fonts` lists registered. */
export type FontCacheOptions = {
  /**
   * Bytes of font files from renders' `fonts` lists to keep; see
   * {@link FontCacheRendererOptions.fontCacheMaxBytes}. A family a render in progress lists in
   * `fonts` or `fontFamilies` is not released.
   * @default Infinity
   */
  maxBytes?: number;
  /** The backend's `unregisterFont`, which releasing a family calls. */
  unregister?: (family: string) => unknown;
};

/** What a registry knows of a family: its name as the backend gave it, its bytes, and who uses it. */
type FamilyUse = {
  name: string;
  bytes: number;
  /** Registered through `registerFont`, so never released for space. */
  pinned: boolean;
  /** The registry's clock when a render last used it. */
  lastUsed: number;
  /** Renders in progress that use it. */
  users: number;
};

/** A family name as the backend matches it: each character lowercased on its own, as fontique does. */
const foldFamily = (family: string) =>
  Array.from(family, (character) => character.toLowerCase()).join("");

/**
 * Font-registration state machine shared by both backends: dedupes registrations by a stable key
 * (or by buffer identity), resolves lazy `data()` loaders once, and turns a set of {@link FontLoader}s
 * plus an {@link ImagesInput} into the `fontFamilies`/`images` the binding expects. With a
 * {@link FontCacheOptions.maxBytes} budget it also releases the fonts renders used least recently.
 *
 * `registerInner` is the backend's raw `registerFont`; its precise `RegisteredFamily` type flows
 * back out through {@link TFamily}.
 */
export class FontRegistry<TFamily extends RegisteredFamilyLike> {
  private readonly byKey = new Map<string, Promise<TFamily[]>>();
  private byData = new WeakMap<ByteBuf, Promise<TFamily[]>>();
  /** Each family, folded as the backend matches names, to the registrations that produced it. */
  private readonly byFamily = new Map<string, Set<string | WeakRef<ByteBuf>>>();
  /** Each family, lowercased, to its bytes and use. A font in more than one family counts in each. */
  private readonly uses = new Map<string, FamilyUse>();
  private clock = 0;
  /** Counts releases, so a registration can tell whether one ran while it was pending. */
  private epoch = 0;
  /** Each family released while a registration was pending, to the epoch of its release. */
  private readonly released = new Map<string, number>();
  /** The epoch of the last release of every family. */
  private releasedAll = 0;
  private pending = 0;

  constructor(
    private readonly registerInner: (font: Font) => Awaitable<TFamily[]>,
    private readonly cache: FontCacheOptions = {},
  ) {
    if (cache.maxBytes !== undefined && !(cache.maxBytes >= 0)) {
      throw new RangeError(
        `fontCacheMaxBytes must be a non-negative number, got ${cache.maxBytes}`,
      );
    }
  }

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

  /**
   * Registers one font, deduped against earlier registrations. A `pinned` font, as `registerFont`
   * registers, is never released for space.
   */
  async register(font: FontLoader, pinned = true): Promise<TFamily[]> {
    const loader = typeof font === "string" ? fontFromUrl(font) : font;
    const key = createFontKey(loader);

    const cached = this.getFont(key);
    if (cached) {
      return pinned ? cached.then((families) => this.pin(families)) : cached;
    }

    const started = this.epoch;

    this.pending++;

    const promise: Promise<TFamily[]> = Promise.resolve(extractFontBuffer(loader))
      .then(async (data) => {
        const view = asView(data);
        const families = await this.registerInner(
          isBuffer(loader) ? view : { ...loader, unicodeRange: unicodeRangeOf(loader), data: view },
        );

        // A family released while this was pending may be gone from the backend, so the next
        // registration of the font has to reach it again.
        if (families.some(({ name }) => this.releasedSince(name, started))) {
          if (this.getFont(key) === promise) {
            this.deleteFont(key);
          }
        } else {
          this.remember(key, families, view.byteLength, pinned);
        }

        return families;
      })
      .catch((error) => {
        this.deleteFont(key);
        throw error;
      })
      .finally(() => {
        if (--this.pending === 0) {
          this.released.clear();
        }
      });

    this.setFont(key, promise);

    return promise;
  }

  /**
   * Forgets every registration that produced `family`, so registering one of them again reaches
   * the backend. Call it when the backend unregisters the family.
   */
  forget(family: string) {
    const folded = foldFamily(family);

    this.epoch++;

    if (this.pending > 0) {
      this.released.set(folded, this.epoch);
    }

    for (const entry of this.byFamily.get(folded) ?? []) {
      const key = typeof entry === "string" ? entry : entry.deref();

      if (key !== undefined) {
        this.deleteFont(key);
      }
    }

    this.byFamily.delete(folded);
    this.uses.delete(folded);
  }

  /**
   * Forgets every registration. Call it when the backend clears its fonts. A registration still
   * loading finishes afterwards, but is not cached, so a later render registers it again.
   */
  forgetAll() {
    this.releasedAll = ++this.epoch;
    this.byKey.clear();
    this.byData = new WeakMap();
    this.byFamily.clear();
    this.uses.clear();
  }

  /**
   * Marks `families` used by a render until the returned function is called, which then releases
   * the least recently used fonts past the budget.
   */
  use(families: readonly string[]): () => void {
    const used = families.map((family) => this.uses.get(foldFamily(family)));
    const now = ++this.clock;

    for (const use of used) {
      if (use) {
        use.users++;
        use.lastUsed = now;
      }
    }

    return () => {
      for (const use of used) {
        if (use) {
          use.users--;
        }
      }

      this.evict();
    };
  }

  /** Releases the least recently used unpinned families until the rest fit the budget. */
  private evict() {
    const { maxBytes = Infinity, unregister } = this.cache;

    if (!unregister || maxBytes === Infinity) {
      return;
    }

    const evictable = [...this.uses].filter(([, use]) => !use.pinned);
    let total = evictable.reduce((sum, [, use]) => sum + use.bytes, 0);

    for (const [, use] of evictable.sort(([, a], [, b]) => a.lastUsed - b.lastUsed)) {
      if (total <= maxBytes) {
        return;
      }

      if (use.users > 0) {
        continue;
      }

      unregister(use.name);
      this.forget(use.name);
      total -= use.bytes;
    }
  }

  /** Whether any of `families` is no longer registered here. */
  private lost(families: string[] | undefined) {
    return families?.some((family) => !this.uses.has(foldFamily(family))) ?? false;
  }

  private releasedSince(family: string, epoch: number) {
    return Math.max(this.released.get(foldFamily(family)) ?? 0, this.releasedAll) > epoch;
  }

  private remember(key: string | ByteBuf, families: TFamily[], bytes: number, pinned: boolean) {
    const entry = typeof key === "string" ? key : new WeakRef(key);

    for (const { name } of families) {
      const folded = foldFamily(name);
      const use = this.uses.get(folded) ?? {
        name,
        bytes: 0,
        pinned,
        lastUsed: ++this.clock,
        users: 0,
      };

      use.bytes += bytes;
      use.pinned ||= pinned;
      this.uses.set(folded, use);
      this.byFamily.set(folded, (this.byFamily.get(folded) ?? new Set()).add(entry));
    }
  }

  private pin(families: TFamily[]) {
    for (const { name } of families) {
      const use = this.uses.get(foldFamily(name));

      if (use) {
        use.pinned = true;
      }
    }

    return families;
  }

  /** Registers every font and returns the distinct family names produced. */
  async prepareFonts(fonts: FontLoader[] | undefined): Promise<string[] | undefined> {
    if (!fonts) {
      return;
    }

    const families = await Promise.all(fonts.map((font) => this.register(font, false)));

    return [...new Set(families.flat().map((family) => family.name))];
  }

  /**
   * Registers `fonts` and resolves lazy `images`, yielding the `images`/`fontFamilies` the binding
   * expects. Explicit `fontFamilies` wins over the registered set. Call `release` once the render
   * is done, so its fonts can be released for space again.
   */
  async resolveResources(
    fonts: FontLoader[] | undefined,
    images: ImagesInput | undefined,
    fontFamilies: string[] | undefined,
  ): Promise<{ images?: ImageSource[]; fontFamilies?: string[]; release: () => void }> {
    let registeredFamilies = await this.prepareFonts(fonts);
    const listed = () => [...(registeredFamilies ?? []), ...(fontFamilies ?? [])];
    let release = this.use(listed());

    // A release elsewhere can evict a family before `use` protects it, so register those again,
    // a bounded number of times so a budget smaller than one render's fonts cannot loop forever.
    for (let attempt = 0; attempt < 3 && this.lost(registeredFamilies); attempt++) {
      const previous = release;

      registeredFamilies = await this.prepareFonts(fonts);
      release = this.use(listed());
      previous();
    }

    try {
      return {
        images: images ? await resolveImageLoaders(images) : undefined,
        fontFamilies: fontFamilies ?? registeredFamilies,
        release,
      };
    } catch (error) {
      release();
      throw error;
    }
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
 * the backend to forward into a native call when it supports cancellation, and `release` to call
 * once that call is done.
 */
export async function prepareRenderInput<
  TOptions extends RenderExtras & { fontFamilies?: string[] },
  TFamily extends RegisteredFamilyLike,
>(
  registry: FontRegistry<TFamily>,
  options: TOptions,
  source: CodepointSource,
): Promise<{
  options: ResolvedRenderOptions<TOptions>;
  signal: AbortSignal | undefined;
  release: () => void;
}> {
  const { fonts, fontFamilies, signal, images, ...rest } = options;
  signal?.throwIfAborted();

  const resolvedFonts = await fonts;
  const { release, ...resolved } = await registry.resolveResources(
    resolvedFonts &&
      subsetFonts({
        fonts: resolvedFonts,
        source: [...(Array.isArray(source) ? source : [source]), LIST_MARKER_CHARACTERS],
      }),
    images,
    fontFamilies,
  );

  if (signal?.aborted) {
    release();
    signal.throwIfAborted();
  }

  return { options: { ...rest, ...resolved }, signal, release };
}

/** Runs `call` with the binding options {@link prepareRenderInput} yields, then releases the render's fonts. */
export async function withRenderInput<
  TOptions extends RenderExtras & { fontFamilies?: string[] },
  TFamily extends RegisteredFamilyLike,
  TResult,
>(
  registry: FontRegistry<TFamily>,
  options: TOptions,
  source: CodepointSource,
  call: (
    options: ResolvedRenderOptions<TOptions>,
    signal: AbortSignal | undefined,
  ) => Awaitable<TResult>,
): Promise<TResult> {
  const {
    options: resolved,
    signal,
    release,
  } = await prepareRenderInput(registry, options, source);

  try {
    return await call(resolved, signal);
  } finally {
    release();
  }
}
