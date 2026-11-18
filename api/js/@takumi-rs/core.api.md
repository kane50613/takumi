# @takumi-rs/core

## `@takumi-rs/core`

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

export const __napiBindingTarget: 'native' | 'wasm32-wasi' | 'wasm32-wasip1';

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

export interface AnimationScene {
    durationMs: number;
    node: Node_2 | string | {
        json: string;
    };
}

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

export interface FontDetails {
    data: ByteBuf;
    generic?: "serif" | "sans-serif" | "monospace" | "cursive" | "fantasy" | "system-ui" | "ui-serif" | "ui-sans-serif" | "ui-monospace" | "ui-rounded" | "emoji" | "math" | "fangsong";
    name?: string;
    style?: "normal" | "italic" | "oblique" | `oblique ${number}deg` | (string & {});
    // @deprecated
    subsetOf?: string;
    // @deprecated
    subsetRank?: number;
    unicodeRange?: [number, number][];
    weight?: number;
}

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

export interface ImageSource {
    cache?: ImageCacheMode;
    data: Uint8Array | ArrayBuffer;
    src: string;
}

// @internal
type JsonSource = {
    json: string;
};

// @deprecated
export type Keyframes = KeyframesMap | KeyframesRuleList;

// @deprecated
export type KeyframesMap = Record<string, Record<string, Properties>>;

// @deprecated
export type KeyframesRuleList = {
    name: string;
    keyframes: {
        offsets: number[];
        declarations: Record<string, Properties>;
    }[];
}[];

export interface MeasuredNode {
    children: Array<MeasuredNode>;
    height: number;
    runs: Array<MeasuredTextRun>;
    transform: [number, number, number, number, number, number];
    width: number;
}

export interface MeasuredTextRun {
    height: number;
    text: string;
    width: number;
    x: number;
    y: number;
}

export { Node_2 as Node }

export { NodeMetadata }

export type OutputFormat = "webp" | "png" | "jpeg" | "ico" | "raw";

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

export interface RegisteredFace {
    index: number;
    style: string;
    weight: number;
    width: number;
}

export interface RegisteredFamily {
    faces: Array<RegisteredFace>;
    name: string;
}

// @internal
type RegisteredFamilyLike = {
    name: string;
};

interface RenderAnimationOptions$1 {
    css?: CssInput[];
    devicePixelRatio?: number;
    drawDebugBorder?: boolean;
    // @internal
    failedFontFiles?: Array<string>;
    fontFamilies?: Array<string>;
    format?: AnimationOutputFormat;
    fps: number;
    height: number;
    images?: Array<ImageSource>;
    // @deprecated
    keyframes?: Keyframes;
    lang?: string;
    lossless?: boolean;
    quality?: number;
    scenes: Array<AnimationScene>;
    // @deprecated
    stylesheets?: Array<string>;
    width: number;
}

// Warning: (ae-forgotten-export) The symbol "BackendAnimationOptions" needs to be exported
export type RenderAnimationOptions = BackendAnimationOptions<RenderAnimationOptions$1>;

export class Renderer {
    constructor(options?: RendererOptions);
    clearFonts(): void;
    measure(source: PreparedSource, options?: {
        signal?: AbortSignal;
    }): Promise<MeasuredNode>;
    measure(source: Node_2 | string, options?: RenderOptions): Promise<MeasuredNode>;
    prepare(source: Node_2 | string, options?: PrepareOptions): Promise<PreparedSource>;
    prepareAnimation(options: PrepareAnimationOptions): Promise<PreparedAnimation>;
    registerFont(font: FontLoader): Promise<RegisteredFamily[]>;
    render(source: PreparedSource, options?: PreparedRenderOptions): Promise<Buffer<ArrayBuffer>>;
    render(source: Node_2 | string, options?: RenderOptions): Promise<Buffer<ArrayBuffer>>;
    renderAnimation(source: PreparedAnimation, options?: {
        signal?: AbortSignal;
    }): Promise<Buffer<ArrayBuffer>>;
    renderAnimation(options: RenderAnimationOptions): Promise<Buffer<ArrayBuffer>>;
    renderSvg(source: PreparedSource, options?: {
        signal?: AbortSignal;
    }): Promise<string>;
    renderSvg(source: Node_2 | string, options?: SvgRenderOptions): Promise<string>;
    // Warning: (ae-forgotten-export) The symbol "RegisteredFamilyLike" needs to be exported
    unregisterFont(family: RegisteredFamilyLike): boolean;
}

export interface RendererOptions {
    cacheMaxBytes?: number;
    fontCacheMaxBytes?: number;
}

type RenderExtras = {
    fonts?: FontLoader[] | Promise<FontLoader[]>;
    signal?: AbortSignal;
    images?: ImagesInput;
};

export interface RenderNeeds {
    fontFiles: Array<string>;
    fonts: Array<number>;
    images: Array<string>;
}

interface RenderOptions$1 {
    css?: CssInput[];
    devicePixelRatio?: number;
    dithering?: DitheringAlgorithm;
    drawDebugBorder?: boolean;
    // @internal
    failedFontFiles?: Array<string>;
    fontFamilies?: Array<string>;
    format?: OutputFormat;
    height?: number;
    images?: Array<ImageSource>;
    // @deprecated
    keyframes?: Keyframes;
    lang?: string;
    lossless?: boolean;
    quality?: number;
    // @deprecated
    stylesheets?: Array<string>;
    timeMs?: number;
    width?: number;
}

// Warning: (ae-forgotten-export) The symbol "BackendRenderOptions" needs to be exported
export type RenderOptions = BackendRenderOptions<RenderOptions$1>;

export { RgbaImage }

export function setGlyphCacheMaxBytes(bytes: number): void;

interface SvgRenderOptions$1 {
    css?: CssInput[];
    // @internal
    failedFontFiles?: Array<string>;
    fontFamilies?: Array<string>;
    height?: number;
    images?: Array<ImageSource>;
    // @deprecated
    keyframes?: Keyframes;
    lang?: string;
    // @deprecated
    stylesheets?: Array<string>;
    timeMs?: number;
    width?: number;
}

// Warning: (ae-forgotten-export) The symbol "BackendSvgOptions" needs to be exported
// Warning: (ae-forgotten-export) The symbol "SvgRenderOptions$1" needs to be exported
export type SvgRenderOptions = BackendSvgOptions<SvgRenderOptions$1>;

export { TextNode }

// Warnings were encountered during analysis:
// dist/export.d.mts:357:3 - (ae-forgotten-export) The symbol "ImageCacheMode$1" needs to be exported
// dist/export.d.mts:453:5 - (ae-forgotten-export) The symbol "JsonSource" needs to be exported
```
