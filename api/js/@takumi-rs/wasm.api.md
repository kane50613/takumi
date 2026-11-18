# @takumi-rs/wasm

## `@takumi-rs/wasm`, `@takumi-rs/wasm/no-bundler`

```ts
import { ContainerNode } from '@takumi-rs/helpers';
import { CssInput } from '@takumi-rs/helpers';
import { FetchOptions } from '@takumi-rs/helpers';
import { GenericFontFamily } from '@takumi-rs/helpers';
import { ImageFetchCache } from '@takumi-rs/helpers';
import { ImageNode } from '@takumi-rs/helpers';
import { Node as Node_2 } from '@takumi-rs/helpers';
import { NodeMetadata } from '@takumi-rs/helpers';
import { Properties } from 'csstype';
import { RgbaImage } from '@takumi-rs/helpers';
import { TextNode } from '@takumi-rs/helpers';

function __wbg_init(module_or_path?: {
    module_or_path: InitInput | Promise<InitInput>;
} | InitInput | Promise<InitInput>): Promise<InitOutput>;
export default __wbg_init;

type AnimationLayout<TInternal extends {
    scenes: {
        node: unknown;
    }[];
}> = Omit<TInternal, "images" | "format" | "quality" | "lossless" | "scenes"> & {
    scenes: (Omit<TInternal["scenes"][number], "node"> & {
        node: Exclude<TInternal["scenes"][number]["node"], JsonSource>;
    })[];
} & AnimationOutputFormatOptions;

export type AnimationOutputFormat = "webp" | "apng" | "gif";

// @internal
export type AnimationOutputFormatOptions = {
    format?: "webp";
    quality?: number;
    lossless?: boolean;
} | {
    format: "apng";
} | {
    format: "gif";
};

export type AnimationScene = {
    node: Node_2 | string | {
        json: string;
    };
    durationMs: number;
};

// Warning: (ae-forgotten-export) The symbol "AnimationLayout" needs to be exported
// Warning: (ae-forgotten-export) The symbol "RenderExtras" needs to be exported
// @internal
type BackendAnimationOptions<TInternal extends {
    scenes: {
        node: unknown;
    }[];
}> = AnimationLayout<TInternal> & RenderExtras;

// Warning: (ae-forgotten-export) The symbol "PrepareExtras" needs to be exported
// @internal
type BackendPrepareAnimationOptions<TInternal extends {
    scenes: {
        node: unknown;
    }[];
}> = AnimationLayout<TInternal> & PrepareExtras;

// @internal
type BackendPreparedRenderOptions<TInternal> = Pick<TInternal, Extract<keyof TInternal, "dithering" | "drawDebugBorder">> & OutputFormatOptions & {
    signal?: AbortSignal;
};

// Warning: (ae-forgotten-export) The symbol "OutputOnly" needs to be exported
// @internal
type BackendPrepareOptions<TInternal> = Omit<TInternal, "images" | "failedFontFiles" | OutputOnly> & PrepareExtras;

// @internal
type BackendRenderOptions<TInternal> = Omit<TInternal, "images" | "format" | "quality" | "lossless"> & OutputFormatOptions & RenderExtras;

// @internal
type BackendSvgOptions<TInternal> = Omit<TInternal, "images"> & RenderExtras;

// @internal
type ByteBuf$1 = Uint8Array | ArrayBuffer | Buffer;

export type ByteBuf = Uint8Array | ArrayBuffer | Buffer;

export { ContainerNode }

export { CssInput }

export type DitheringAlgorithm = "none" | "ordered-bayer" | "floyd-steinberg";

// Warning: (ae-forgotten-export) The symbol "FontDetails$1" needs to be exported
// Warning: (ae-forgotten-export) The symbol "ByteBuf$1" needs to be exported
// @internal
type Font$1 = FontDetails$1 | ByteBuf$1;

export type Font = FontDetails | ByteBuf;

// @internal
type FontDetails$1 = {
    name?: string;
    data: ByteBuf$1;
    weight?: number;
    style?: "normal" | "italic" | "oblique" | `oblique ${number}deg` | (string & {});
    unicodeRange?: [number, number][];
    subsetOf?: string;
    subsetRank?: number;
    generic?: GenericFontFamily;
};

export type FontDetails = {
    name?: string;
    data: ByteBuf;
    weight?: number;
    style?: "normal" | "italic" | "oblique" | `oblique ${number}deg` | (string & {});
    unicodeRange?: [number, number][];
    subsetOf?: string;
    subsetRank?: number;
    generic?: "serif" | "sans-serif" | "monospace" | "cursive" | "fantasy" | "system-ui" | "ui-serif" | "ui-sans-serif" | "ui-monospace" | "ui-rounded" | "emoji" | "math" | "fangsong";
};

// Warning: (ae-forgotten-export) The symbol "Font$1" needs to be exported
// @internal
export type FontLoader = string | Font$1 | (Omit<FontDetails$1, "data"> & {
    data: () => Promise<FontDetails$1["data"]> | FontDetails$1["data"];
    ranges?: [number, number][];
} & ({
    key: string;
} | {
    name: string;
}));

type ImageCacheMode$1 = "auto" | "none";

export type ImageCacheMode = "auto" | "none";

// Warning: (ae-forgotten-export) The symbol "ImageSource$1" needs to be exported
// @internal
export type ImageLoader = Omit<ImageSource$1, "data"> & {
    data: ImageSource$1["data"] | (() => ImageSource$1["data"] | Promise<ImageSource$1["data"]>);
};

export { ImageNode }

// @internal
export type ImagesInput = ImageLoader[] | {
    sources?: ImageLoader[];
    cache?: NonNullable<ImageLoader["cache"]>;
};

// @internal
type ImageSource$1 = {
    src: string;
    data: ByteBuf$1;
    cache?: ImageCacheMode$1;
};

export type ImageSource = {
    src: string;
    data: ByteBuf;
    cache?: ImageCacheMode;
};

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly __externref_table_alloc: () => number;
    readonly __externref_table_dealloc: (a: number) => void;
    readonly __wbg_renderer_free: (a: number, b: number) => void;
    readonly __wbindgen_exn_store: (a: number) => void;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_start: () => void;
    readonly memory: WebAssembly.Memory;
    readonly renderer_animationNeeds: (a: number, b: any, c: number) => [number, number, number];
    readonly renderer_cachedFonts: (a: number, b: number, c: number, d: number) => [number, number, number];
    readonly renderer_cacheFont: (a: number, b: any, c: number, d: number) => [number, number, number];
    readonly renderer_cacheFontFile: (a: number, b: number, c: number, d: number, e: number) => [number, number];
    readonly renderer_clearFonts: (a: number) => [number, number];
    readonly renderer_measure: (a: number, b: any, c: number) => [number, number, number];
    readonly renderer_needs: (a: number, b: any, c: number, d: number) => [number, number, number];
    readonly renderer_new: (a: number) => [number, number, number];
    readonly renderer_registerFont: (a: number, b: any, c: number, d: number) => [number, number, number];
    readonly renderer_render: (a: number, b: any, c: number) => [number, number, number, number];
    readonly renderer_renderAnimation: (a: number, b: any) => [number, number, number, number];
    readonly renderer_renderAsDataUrl: (a: number, b: any, c: any) => [number, number, number, number];
    readonly renderer_renderSvg: (a: number, b: any, c: number) => [number, number, number, number];
    readonly renderer_unregisterFont: (a: number, b: number, c: number) => number;
    readonly setGlyphCacheMaxBytes: (a: number) => void;
}

export function initSync(module: {
    module: SyncInitInput;
} | SyncInitInput): InitOutput;

// @internal
type JsonSource = {
    json: string;
};

export type KeyframeRule = {
    offsets: number[];
    declarations: Record<string, Properties>;
};

// @deprecated
export type Keyframes = KeyframesMap | KeyframesRuleList;

// @deprecated
export type KeyframesMap = Record<string, Record<string, Properties>>;

export type KeyframesRule = {
    name: string;
    keyframes: KeyframeRule[];
};

// @deprecated
export type KeyframesRuleList = {
    name: string;
    keyframes: {
        offsets: number[];
        declarations: Record<string, Properties>;
    }[];
}[];

export type MeasuredNode = {
    width: number;
    height: number;
    transform: [number, number, number, number, number, number];
    children: MeasuredNode[];
    runs: MeasuredTextRun[];
};

export type MeasuredTextRun = {
    text: string;
    x: number;
    y: number;
    width: number;
    height: number;
};

export { Node_2 as Node }

export { NodeMetadata }

export type OutputFormat = "png" | "jpeg" | "webp" | "ico" | "raw";

// @internal
export type OutputFormatOptions = {
    format?: "png";
} | {
    format: "jpeg";
    quality?: number;
} | {
    format: "webp";
    quality?: number;
    lossless?: boolean;
} | {
    format: "ico";
} | {
    format: "raw";
};

type OutputOnly = "format" | "quality" | "lossless" | "dithering" | "drawDebugBorder";

// Warning: (ae-forgotten-export) The symbol "BackendPrepareAnimationOptions" needs to be exported
// Warning: (ae-forgotten-export) The symbol "RenderAnimationOptions$1" needs to be exported
export type PrepareAnimationOptions = BackendPrepareAnimationOptions<RenderAnimationOptions$1>;

export class Prepared<TSource = unknown, TLayout = unknown> {
}

export type PreparedAnimation = Prepared<RenderAnimationOptions$1["scenes"], Omit<RenderAnimationOptions$1, "images" | "fontFamilies" | "scenes">>;

// Warning: (ae-forgotten-export) The symbol "BackendPreparedRenderOptions" needs to be exported
// Warning: (ae-forgotten-export) The symbol "RenderOptions$1" needs to be exported
export type PreparedRenderOptions = BackendPreparedRenderOptions<RenderOptions$1>;

export type PreparedSource = Prepared<Node_2 | string | JsonSource, Omit<RenderOptions$1, "images" | "fontFamilies">>;

// @internal
type PrepareExtras = {
    fonts?: FontLoader[] | Promise<FontLoader[]>;
    images?: PrepareImagesInput;
    signal?: AbortSignal;
};

// @internal
export type PrepareImagesInput = ImageLoader[] | (Omit<FetchOptions, "signal"> & {
    sources?: ImageLoader[];
    fetchCache?: ImageFetchCache;
    cache?: NonNullable<ImageLoader["cache"]>;
    throwOnError?: boolean;
});

// Warning: (ae-forgotten-export) The symbol "BackendPrepareOptions" needs to be exported
export type PrepareOptions = BackendPrepareOptions<RenderOptions$1>;

export type RegisteredFace = {
    weight: number;
    style: string;
    width: number;
    index: number;
};

export type RegisteredFamily = {
    name: string;
    faces: RegisteredFace[];
};

// @internal
type RegisteredFamilyLike = {
    name: string;
};

type RenderAnimationOptions$1 = {
    scenes: AnimationScene[];
    width: number;
    height: number;
    format?: AnimationOutputFormat;
    quality?: number;
    lossless?: boolean;
    images?: ImageSource[];
    drawDebugBorder?: boolean;
    css?: CssInput[];
    stylesheets?: string[];
    keyframes?: Keyframes;
    devicePixelRatio?: number;
    fps: number;
    fontFamilies?: string[];
    failedFontFiles?: string[];
    lang?: string;
};

// Warning: (ae-forgotten-export) The symbol "BackendAnimationOptions" needs to be exported
export type RenderAnimationOptions = BackendAnimationOptions<RenderAnimationOptions$1>;

export class Renderer {
    [Symbol.dispose](): void;
    constructor(options?: RendererOptions);
    clearFonts(): void;
    free(): void;
    measure(source: PreparedSource, options?: {
        signal?: AbortSignal;
    }): Promise<MeasuredNode>;
    measure(source: Node_2 | string, options?: RenderOptions): Promise<MeasuredNode>;
    prepare(source: Node_2 | string, options?: PrepareOptions): Promise<PreparedSource>;
    prepareAnimation(options: PrepareAnimationOptions): Promise<PreparedAnimation>;
    registerFont(font: FontLoader): Promise<RegisteredFamily[]>;
    render(source: PreparedSource, options?: PreparedRenderOptions): Promise<Uint8Array<ArrayBuffer>>;
    render(source: Node_2 | string, options?: RenderOptions): Promise<Uint8Array<ArrayBuffer>>;
    renderAnimation(source: PreparedAnimation, options?: {
        signal?: AbortSignal;
    }): Promise<Uint8Array<ArrayBuffer>>;
    renderAnimation(options: RenderAnimationOptions): Promise<Uint8Array<ArrayBuffer>>;
    renderAsDataUrl(source: PreparedSource, options?: PreparedRenderOptions): Promise<string>;
    renderAsDataUrl(source: Node_2 | string, options?: RenderOptions): Promise<string>;
    renderSvg(source: PreparedSource, options?: {
        signal?: AbortSignal;
    }): Promise<string>;
    renderSvg(source: Node_2 | string, options?: SvgRenderOptions): Promise<string>;
    // Warning: (ae-forgotten-export) The symbol "RegisteredFamilyLike" needs to be exported
    unregisterFont(family: RegisteredFamilyLike): boolean;
}

export type RendererOptions = {
    cacheMaxBytes?: number;
    fontCacheMaxBytes?: number;
};

type RenderExtras = {
    fonts?: FontLoader[] | Promise<FontLoader[]>;
    signal?: AbortSignal;
    images?: ImagesInput;
};

export type RenderNeeds = {
    images: string[];
    fonts: number[];
};

type RenderOptions$1 = {
    width?: number;
    height?: number;
    format?: OutputFormat;
    quality?: number;
    lossless?: boolean;
    images?: ImageSource[];
    css?: CssInput[];
    stylesheets?: string[];
    keyframes?: Keyframes;
    drawDebugBorder?: boolean;
    devicePixelRatio?: number;
    timeMs?: number;
    dithering?: DitheringAlgorithm;
    fontFamilies?: string[];
    failedFontFiles?: string[];
    lang?: string;
};

// Warning: (ae-forgotten-export) The symbol "BackendRenderOptions" needs to be exported
export type RenderOptions = BackendRenderOptions<RenderOptions$1>;

export { RgbaImage }

export function setGlyphCacheMaxBytes(bytes: number): void;

type SvgRenderOptions$1 = Omit<RenderOptions$1, "format" | "quality" | "lossless" | "drawDebugBorder" | "devicePixelRatio" | "dithering">;

// Warning: (ae-forgotten-export) The symbol "BackendSvgOptions" needs to be exported
// Warning: (ae-forgotten-export) The symbol "SvgRenderOptions$1" needs to be exported
export type SvgRenderOptions = BackendSvgOptions<SvgRenderOptions$1>;

export type SyncInitInput = BufferSource | WebAssembly.Module;

export { TextNode }

// Warnings were encountered during analysis:
// (ae-forgotten-export) The symbol "ImageCacheMode$1" needs to be exported
// (ae-forgotten-export) The symbol "JsonSource" needs to be exported
```

## `@takumi-rs/wasm/node`, `@takumi-rs/wasm/auto` (bun), `@takumi-rs/wasm/auto` (webpack.node.default, node.default)

```ts
import { ContainerNode } from '@takumi-rs/helpers';
import { CssInput } from '@takumi-rs/helpers';
import { FetchOptions } from '@takumi-rs/helpers';
import { GenericFontFamily } from '@takumi-rs/helpers';
import { ImageFetchCache } from '@takumi-rs/helpers';
import { ImageNode } from '@takumi-rs/helpers';
import { Node as Node_2 } from '@takumi-rs/helpers';
import { NodeMetadata } from '@takumi-rs/helpers';
import { Properties } from 'csstype';
import { RgbaImage } from '@takumi-rs/helpers';
import { TextNode } from '@takumi-rs/helpers';

type AnimationLayout<TInternal extends {
    scenes: {
        node: unknown;
    }[];
}> = Omit<TInternal, "images" | "format" | "quality" | "lossless" | "scenes"> & {
    scenes: (Omit<TInternal["scenes"][number], "node"> & {
        node: Exclude<TInternal["scenes"][number]["node"], JsonSource>;
    })[];
} & AnimationOutputFormatOptions;

export type AnimationOutputFormat = "webp" | "apng" | "gif";

// @internal
export type AnimationOutputFormatOptions = {
    format?: "webp";
    quality?: number;
    lossless?: boolean;
} | {
    format: "apng";
} | {
    format: "gif";
};

export type AnimationScene = {
    node: Node_2 | string | {
        json: string;
    };
    durationMs: number;
};

// Warning: (ae-forgotten-export) The symbol "AnimationLayout" needs to be exported
// Warning: (ae-forgotten-export) The symbol "RenderExtras" needs to be exported
// @internal
type BackendAnimationOptions<TInternal extends {
    scenes: {
        node: unknown;
    }[];
}> = AnimationLayout<TInternal> & RenderExtras;

// Warning: (ae-forgotten-export) The symbol "PrepareExtras" needs to be exported
// @internal
type BackendPrepareAnimationOptions<TInternal extends {
    scenes: {
        node: unknown;
    }[];
}> = AnimationLayout<TInternal> & PrepareExtras;

// @internal
type BackendPreparedRenderOptions<TInternal> = Pick<TInternal, Extract<keyof TInternal, "dithering" | "drawDebugBorder">> & OutputFormatOptions & {
    signal?: AbortSignal;
};

// Warning: (ae-forgotten-export) The symbol "OutputOnly" needs to be exported
// @internal
type BackendPrepareOptions<TInternal> = Omit<TInternal, "images" | "failedFontFiles" | OutputOnly> & PrepareExtras;

// @internal
type BackendRenderOptions<TInternal> = Omit<TInternal, "images" | "format" | "quality" | "lossless"> & OutputFormatOptions & RenderExtras;

// @internal
type BackendSvgOptions<TInternal> = Omit<TInternal, "images"> & RenderExtras;

// @internal
type ByteBuf$1 = Uint8Array | ArrayBuffer | Buffer;

export type ByteBuf = Uint8Array | ArrayBuffer | Buffer;

export { ContainerNode }

export { CssInput }

export type DitheringAlgorithm = "none" | "ordered-bayer" | "floyd-steinberg";

// Warning: (ae-forgotten-export) The symbol "FontDetails$1" needs to be exported
// Warning: (ae-forgotten-export) The symbol "ByteBuf$1" needs to be exported
// @internal
type Font$1 = FontDetails$1 | ByteBuf$1;

export type Font = FontDetails | ByteBuf;

// @internal
type FontDetails$1 = {
    name?: string;
    data: ByteBuf$1;
    weight?: number;
    style?: "normal" | "italic" | "oblique" | `oblique ${number}deg` | (string & {});
    unicodeRange?: [number, number][];
    subsetOf?: string;
    subsetRank?: number;
    generic?: GenericFontFamily;
};

export type FontDetails = {
    name?: string;
    data: ByteBuf;
    weight?: number;
    style?: "normal" | "italic" | "oblique" | `oblique ${number}deg` | (string & {});
    unicodeRange?: [number, number][];
    subsetOf?: string;
    subsetRank?: number;
    generic?: "serif" | "sans-serif" | "monospace" | "cursive" | "fantasy" | "system-ui" | "ui-serif" | "ui-sans-serif" | "ui-monospace" | "ui-rounded" | "emoji" | "math" | "fangsong";
};

// Warning: (ae-forgotten-export) The symbol "Font$1" needs to be exported
// @internal
export type FontLoader = string | Font$1 | (Omit<FontDetails$1, "data"> & {
    data: () => Promise<FontDetails$1["data"]> | FontDetails$1["data"];
    ranges?: [number, number][];
} & ({
    key: string;
} | {
    name: string;
}));

type ImageCacheMode$1 = "auto" | "none";

export type ImageCacheMode = "auto" | "none";

// Warning: (ae-forgotten-export) The symbol "ImageSource$1" needs to be exported
// @internal
export type ImageLoader = Omit<ImageSource$1, "data"> & {
    data: ImageSource$1["data"] | (() => ImageSource$1["data"] | Promise<ImageSource$1["data"]>);
};

export { ImageNode }

// @internal
export type ImagesInput = ImageLoader[] | {
    sources?: ImageLoader[];
    cache?: NonNullable<ImageLoader["cache"]>;
};

// @internal
type ImageSource$1 = {
    src: string;
    data: ByteBuf$1;
    cache?: ImageCacheMode$1;
};

export type ImageSource = {
    src: string;
    data: ByteBuf;
    cache?: ImageCacheMode;
};

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly __externref_table_alloc: () => number;
    readonly __externref_table_dealloc: (a: number) => void;
    readonly __wbg_renderer_free: (a: number, b: number) => void;
    readonly __wbindgen_exn_store: (a: number) => void;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_start: () => void;
    readonly memory: WebAssembly.Memory;
    readonly renderer_animationNeeds: (a: number, b: any, c: number) => [number, number, number];
    readonly renderer_cachedFonts: (a: number, b: number, c: number, d: number) => [number, number, number];
    readonly renderer_cacheFont: (a: number, b: any, c: number, d: number) => [number, number, number];
    readonly renderer_cacheFontFile: (a: number, b: number, c: number, d: number, e: number) => [number, number];
    readonly renderer_clearFonts: (a: number) => [number, number];
    readonly renderer_measure: (a: number, b: any, c: number) => [number, number, number];
    readonly renderer_needs: (a: number, b: any, c: number, d: number) => [number, number, number];
    readonly renderer_new: (a: number) => [number, number, number];
    readonly renderer_registerFont: (a: number, b: any, c: number, d: number) => [number, number, number];
    readonly renderer_render: (a: number, b: any, c: number) => [number, number, number, number];
    readonly renderer_renderAnimation: (a: number, b: any) => [number, number, number, number];
    readonly renderer_renderAsDataUrl: (a: number, b: any, c: any) => [number, number, number, number];
    readonly renderer_renderSvg: (a: number, b: any, c: number) => [number, number, number, number];
    readonly renderer_unregisterFont: (a: number, b: number, c: number) => number;
    readonly setGlyphCacheMaxBytes: (a: number) => void;
}

export function initSync(module: {
    module: SyncInitInput;
} | SyncInitInput): InitOutput;

// @internal
type JsonSource = {
    json: string;
};

export type KeyframeRule = {
    offsets: number[];
    declarations: Record<string, Properties>;
};

// @deprecated
export type Keyframes = KeyframesMap | KeyframesRuleList;

// @deprecated
export type KeyframesMap = Record<string, Record<string, Properties>>;

export type KeyframesRule = {
    name: string;
    keyframes: KeyframeRule[];
};

// @deprecated
export type KeyframesRuleList = {
    name: string;
    keyframes: {
        offsets: number[];
        declarations: Record<string, Properties>;
    }[];
}[];

export type MeasuredNode = {
    width: number;
    height: number;
    transform: [number, number, number, number, number, number];
    children: MeasuredNode[];
    runs: MeasuredTextRun[];
};

export type MeasuredTextRun = {
    text: string;
    x: number;
    y: number;
    width: number;
    height: number;
};

export { Node_2 as Node }

export { NodeMetadata }

export type OutputFormat = "png" | "jpeg" | "webp" | "ico" | "raw";

// @internal
export type OutputFormatOptions = {
    format?: "png";
} | {
    format: "jpeg";
    quality?: number;
} | {
    format: "webp";
    quality?: number;
    lossless?: boolean;
} | {
    format: "ico";
} | {
    format: "raw";
};

type OutputOnly = "format" | "quality" | "lossless" | "dithering" | "drawDebugBorder";

// Warning: (ae-forgotten-export) The symbol "BackendPrepareAnimationOptions" needs to be exported
// Warning: (ae-forgotten-export) The symbol "RenderAnimationOptions$1" needs to be exported
export type PrepareAnimationOptions = BackendPrepareAnimationOptions<RenderAnimationOptions$1>;

export class Prepared<TSource = unknown, TLayout = unknown> {
}

export type PreparedAnimation = Prepared<RenderAnimationOptions$1["scenes"], Omit<RenderAnimationOptions$1, "images" | "fontFamilies" | "scenes">>;

// Warning: (ae-forgotten-export) The symbol "BackendPreparedRenderOptions" needs to be exported
// Warning: (ae-forgotten-export) The symbol "RenderOptions$1" needs to be exported
export type PreparedRenderOptions = BackendPreparedRenderOptions<RenderOptions$1>;

export type PreparedSource = Prepared<Node_2 | string | JsonSource, Omit<RenderOptions$1, "images" | "fontFamilies">>;

// @internal
type PrepareExtras = {
    fonts?: FontLoader[] | Promise<FontLoader[]>;
    images?: PrepareImagesInput;
    signal?: AbortSignal;
};

// @internal
export type PrepareImagesInput = ImageLoader[] | (Omit<FetchOptions, "signal"> & {
    sources?: ImageLoader[];
    fetchCache?: ImageFetchCache;
    cache?: NonNullable<ImageLoader["cache"]>;
    throwOnError?: boolean;
});

// Warning: (ae-forgotten-export) The symbol "BackendPrepareOptions" needs to be exported
export type PrepareOptions = BackendPrepareOptions<RenderOptions$1>;

export type RegisteredFace = {
    weight: number;
    style: string;
    width: number;
    index: number;
};

export type RegisteredFamily = {
    name: string;
    faces: RegisteredFace[];
};

// @internal
type RegisteredFamilyLike = {
    name: string;
};

type RenderAnimationOptions$1 = {
    scenes: AnimationScene[];
    width: number;
    height: number;
    format?: AnimationOutputFormat;
    quality?: number;
    lossless?: boolean;
    images?: ImageSource[];
    drawDebugBorder?: boolean;
    css?: CssInput[];
    stylesheets?: string[];
    keyframes?: Keyframes;
    devicePixelRatio?: number;
    fps: number;
    fontFamilies?: string[];
    failedFontFiles?: string[];
    lang?: string;
};

// Warning: (ae-forgotten-export) The symbol "BackendAnimationOptions" needs to be exported
export type RenderAnimationOptions = BackendAnimationOptions<RenderAnimationOptions$1>;

export class Renderer {
    [Symbol.dispose](): void;
    constructor(options?: RendererOptions);
    clearFonts(): void;
    free(): void;
    measure(source: PreparedSource, options?: {
        signal?: AbortSignal;
    }): Promise<MeasuredNode>;
    measure(source: Node_2 | string, options?: RenderOptions): Promise<MeasuredNode>;
    prepare(source: Node_2 | string, options?: PrepareOptions): Promise<PreparedSource>;
    prepareAnimation(options: PrepareAnimationOptions): Promise<PreparedAnimation>;
    registerFont(font: FontLoader): Promise<RegisteredFamily[]>;
    render(source: PreparedSource, options?: PreparedRenderOptions): Promise<Uint8Array<ArrayBuffer>>;
    render(source: Node_2 | string, options?: RenderOptions): Promise<Uint8Array<ArrayBuffer>>;
    renderAnimation(source: PreparedAnimation, options?: {
        signal?: AbortSignal;
    }): Promise<Uint8Array<ArrayBuffer>>;
    renderAnimation(options: RenderAnimationOptions): Promise<Uint8Array<ArrayBuffer>>;
    renderAsDataUrl(source: PreparedSource, options?: PreparedRenderOptions): Promise<string>;
    renderAsDataUrl(source: Node_2 | string, options?: RenderOptions): Promise<string>;
    renderSvg(source: PreparedSource, options?: {
        signal?: AbortSignal;
    }): Promise<string>;
    renderSvg(source: Node_2 | string, options?: SvgRenderOptions): Promise<string>;
    // Warning: (ae-forgotten-export) The symbol "RegisteredFamilyLike" needs to be exported
    unregisterFont(family: RegisteredFamilyLike): boolean;
}

export type RendererOptions = {
    cacheMaxBytes?: number;
    fontCacheMaxBytes?: number;
};

type RenderExtras = {
    fonts?: FontLoader[] | Promise<FontLoader[]>;
    signal?: AbortSignal;
    images?: ImagesInput;
};

export type RenderNeeds = {
    images: string[];
    fonts: number[];
};

type RenderOptions$1 = {
    width?: number;
    height?: number;
    format?: OutputFormat;
    quality?: number;
    lossless?: boolean;
    images?: ImageSource[];
    css?: CssInput[];
    stylesheets?: string[];
    keyframes?: Keyframes;
    drawDebugBorder?: boolean;
    devicePixelRatio?: number;
    timeMs?: number;
    dithering?: DitheringAlgorithm;
    fontFamilies?: string[];
    failedFontFiles?: string[];
    lang?: string;
};

// Warning: (ae-forgotten-export) The symbol "BackendRenderOptions" needs to be exported
export type RenderOptions = BackendRenderOptions<RenderOptions$1>;

export { RgbaImage }

export function setGlyphCacheMaxBytes(bytes: number): void;

type SvgRenderOptions$1 = Omit<RenderOptions$1, "format" | "quality" | "lossless" | "drawDebugBorder" | "devicePixelRatio" | "dithering">;

// Warning: (ae-forgotten-export) The symbol "BackendSvgOptions" needs to be exported
// Warning: (ae-forgotten-export) The symbol "SvgRenderOptions$1" needs to be exported
export type SvgRenderOptions = BackendSvgOptions<SvgRenderOptions$1>;

export type SyncInitInput = BufferSource | WebAssembly.Module;

export { TextNode }

// Warnings were encountered during analysis:
// (ae-forgotten-export) The symbol "ImageCacheMode$1" needs to be exported
// (ae-forgotten-export) The symbol "JsonSource" needs to be exported
```

## `@takumi-rs/wasm/auto` (unwasm, workerd, edge-light, default), `@takumi-rs/wasm/next`

```ts
const wasmModule: WebAssembly.Module;
export default wasmModule;
```

## `@takumi-rs/wasm/auto` (webpack.default, browser, module)

```ts
const module_2: string;
export default module_2;
```

## `@takumi-rs/wasm/takumi_wasm_bg.wasm`

```ts
export const __externref_table_alloc: () => number;

export const __externref_table_dealloc: (a: number) => void;

export const __wbg_renderer_free: (a: number, b: number) => void;

export const __wbindgen_exn_store: (a: number) => void;

export const __wbindgen_externrefs: WebAssembly.Table;

export const __wbindgen_free: (a: number, b: number, c: number) => void;

export const __wbindgen_malloc: (a: number, b: number) => number;

export const __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;

export const __wbindgen_start: () => void;

export const memory: WebAssembly.Memory;

export const renderer_animationNeeds: (a: number, b: any, c: number) => [number, number, number];

export const renderer_cachedFonts: (a: number, b: number, c: number, d: number) => [number, number, number];

export const renderer_cacheFont: (a: number, b: any, c: number, d: number) => [number, number, number];

export const renderer_cacheFontFile: (a: number, b: number, c: number, d: number, e: number) => [number, number];

export const renderer_clearFonts: (a: number) => [number, number];

export const renderer_measure: (a: number, b: any, c: number) => [number, number, number];

export const renderer_needs: (a: number, b: any, c: number, d: number) => [number, number, number];

export const renderer_new: (a: number) => [number, number, number];

export const renderer_registerFont: (a: number, b: any, c: number, d: number) => [number, number, number];

export const renderer_render: (a: number, b: any, c: number) => [number, number, number, number];

export const renderer_renderAnimation: (a: number, b: any) => [number, number, number, number];

export const renderer_renderAsDataUrl: (a: number, b: any, c: any) => [number, number, number, number];

export const renderer_renderSvg: (a: number, b: any, c: number) => [number, number, number, number];

export const renderer_unregisterFont: (a: number, b: number, c: number) => number;

export const setGlyphCacheMaxBytes: (a: number) => void;
```

## `@takumi-rs/wasm/wasm-url`

```ts
const url: URL;
export default url;
```

## `@takumi-rs/wasm/vite`

```ts
// @deprecated
const module_2: string;
export default module_2;
```
