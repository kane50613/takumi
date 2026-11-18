# takumi-paint

## `takumi-paint`, `takumi-paint/next`, `takumi-paint/no-init`

```ts
import { CssInput } from '@takumi-rs/helpers';
import { FetchOptions } from '@takumi-rs/helpers';
import { GenericFontFamily } from '@takumi-rs/helpers';
import { ImageFetchCache } from '@takumi-rs/helpers';
import { Node as Node_2 } from '@takumi-rs/helpers';
import { ReactElementLike } from '@takumi-rs/helpers';
import { ReactNode } from 'react';

type Awaitable<T> = T | Promise<T>;

interface BeginClipStep {
    readonly clip: Shape;
    readonly node: BoxNode;
    readonly type: "begin-clip";
}

interface BeginGroupStep {
    readonly effects: Effects;
    readonly node: BoxNode;
    readonly type: "begin-group";
}

export type BlendMode = "normal" | "multiply" | "screen" | "overlay" | "darken" | "lighten" | "color-dodge" | "color-burn" | "hard-light" | "soft-light" | "difference" | "exclusion" | "hue" | "saturation" | "color" | "luminosity" | "plus-lighter";

// Warning: (ae-forgotten-export) The symbol "NodeBase" needs to be exported
export interface BoxNode extends NodeBase {
    readonly children: readonly PaintNode[];
    readonly contentBox: Rect;
    readonly effects: Effects | undefined;
    readonly outline: readonly Drawable[];
    readonly overflowClip: Shape | undefined;
    readonly type: "box";
}

// @internal
type ByteBuf$1 = Uint8Array | ArrayBuffer | Buffer;

type ByteBuf = Uint8Array | ArrayBuffer;

export type ColorStop = {
    readonly offset: number;
    readonly color: Rgba;
};

export type CornerRadii = {
    readonly topLeft: Point;
    readonly topRight: Point;
    readonly bottomRight: Point;
    readonly bottomLeft: Point;
};

// @internal
type DocumentInput = Node_2 | ReactNode | ReactElementLike | string;

// Warning: (ae-forgotten-export) The symbol "RawDrawable" needs to be exported
export type Drawable = RawDrawable<TextRun>;

interface DrawStep {
    readonly drawables: readonly Drawable[];
    readonly node: PaintNode;
    readonly type: "draw";
}

// Warning: (ae-forgotten-export) The symbol "RawEffects" needs to be exported
export type Effects = RawEffects<Drawable>;

export type ElementInfo = {
    readonly id?: string;
    readonly tagName?: string;
    readonly className?: string;
    readonly path: readonly number[];
};

export type Filter =
/** Gaussian blur; `radius` is the standard deviation in px. */
    {
    readonly type: "blur";
    readonly radius: number;
} |
/** A 4×5 row-major matrix, as SVG `feColorMatrix type="matrix"`. Brightness, contrast, grayscale, hue-rotate, invert, opacity, saturate, and sepia all become one. */
    {
    readonly type: "color-matrix";
    readonly matrix: readonly number[];
} | {
    readonly type: "drop-shadow";
    readonly offset: Point;
    readonly blur: number;
    readonly color: Rgba;
} |
/** A filter takumi cannot resolve, such as `url(#svg-filter)`. */
    {
    readonly type: "unsupported";
    readonly css: string;
};

// Warning: (ae-forgotten-export) The symbol "FontDetails$1" needs to be exported
// Warning: (ae-forgotten-export) The symbol "ByteBuf$1" needs to be exported
// @internal
type Font$1 = FontDetails$1 | ByteBuf$1;

// Warning: (ae-forgotten-export) The symbol "RawFont" needs to be exported
export interface Font extends RawFont {
    data(): Uint8Array;
}

// Warning: (ae-forgotten-export) The symbol "RegisteredFamilyLike" needs to be exported
// @internal
type FontBackend<TFamily extends RegisteredFamilyLike> = {
    registerFont(font: Font$1, key: string): Awaitable<TFamily[]>;
    cacheFont(font: Font$1, key: string): Awaitable<TFamily[]>;
    cachedFonts(key: string, pin: boolean): TFamily[] | null | undefined;
    cacheFontFile(url: string, data: ArrayBuffer | Uint8Array): Awaitable<void>;
};

export type FontCacheOptions = {
    fontCacheMaxBytes?: number;
};

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

type FontDetails = {
    name?: string;
    data: ByteBuf;
    weight?: number;
    style?: "normal" | "italic" | "oblique" | `oblique ${number}deg` | (string & {});
    unicodeRange?: [number, number][];
    generic?: string;
};

// Warning: (ae-forgotten-export) The symbol "FontDetails" needs to be exported
type FontInput = FontDetails | ByteBuf;

// @internal
export type FontLoader = string | Font$1 | (Omit<FontDetails$1, "data"> & {
    data: () => Promise<FontDetails$1["data"]> | FontDetails$1["data"];
    ranges?: [number, number][];
} & ({
    key: string;
} | {
    name: string;
}));

// @internal
class FontRegistry<TFamily extends RegisteredFamilyLike> {
    // Warning: (ae-forgotten-export) The symbol "FontBackend" needs to be exported
    constructor(backend: FontBackend<TFamily>);
    cache(fonts: FontLoader[]): Promise<boolean>;
    cacheFontFile(url: string, data: ArrayBuffer | Uint8Array): Promise<void>;
    // Warning: (ae-forgotten-export) The symbol "ListedFont$1" needs to be exported
    listed(fonts: FontLoader[]): ListedFont$1[] | undefined;
    register(font: FontLoader, pinned?: boolean): Promise<TFamily[]>;
    resolveResources<T>(fonts: FontLoader[] | undefined, images: ImagesInput | undefined, fontFamilies: string[] | undefined, use: (resources: {
        images?: ImageSource$1[];
        fontFamilies?: string[];
    }) => Awaitable<T>): Promise<T>;
}

export type Glyph = {
    readonly id: number;
    readonly x: number;
    readonly y: number;
    readonly cluster?: number;
};

export type Gradient = {
    readonly type: "linear-gradient";
    readonly start: Point;
    readonly end: Point;
    readonly stops: readonly ColorStop[];
    readonly spread: Spread;
} |
/**
* Elliptical when `radiusX !== radiusY`: scale the y axis by `radiusY / radiusX` about `center`.
* Offset 0 sits at `start` times the radii, as `createRadialGradient`'s inner circle.
*/
    {
    readonly type: "radial-gradient";
    readonly center: Point;
    readonly radiusX: number;
    readonly radiusY: number;
    readonly start: number;
    readonly stops: readonly ColorStop[];
    readonly spread: Spread;
} |
/**
* Angles in radians, clockwise from the positive x axis, as `createConicGradient` takes its
* start. The stops span `startAngle` to `endAngle`, a full turn unless the gradient repeats.
*/
    {
    readonly type: "conic-gradient";
    readonly center: Point;
    readonly startAngle: number;
    readonly endAngle: number;
    readonly stops: readonly ColorStop[];
    readonly spread: Spread;
};

type ImageCacheMode$1 = "auto" | "none";

type ImageCacheMode = "auto" | "none";

type ImageInput = {
    src: string;
    data: ByteBuf;
    cache?: ImageCacheMode;
};

// @internal
type ImageLoader = Omit<ImageSource$1, "data"> & {
    data: ImageSource$1["data"] | (() => ImageSource$1["data"] | Promise<ImageSource$1["data"]>);
};

export interface ImageNode extends NodeBase {
    readonly image: ImageSource;
    readonly type: "image";
}

export type ImagePaint = {
    readonly type: "image";
    readonly image: ImageSource;
    readonly sampling: Sampling;
};

// Warning: (ae-forgotten-export) The symbol "ImageLoader" needs to be exported
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
    readonly src: string;
    readonly width: number;
    readonly height: number;
};

function init(source: {
    module_or_path: InitInput | Promise<InitInput>;
}): Promise<void>;
export default init;

type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export function initSync(source: {
    module: SyncInitInput;
}): void;

// @internal
type JsonSource = {
    json: string;
};

// @internal
type ListedFont$1 = Pick<FontDetails$1, "name" | "weight" | "style" | "unicodeRange"> & {
    key: string;
};

type ListedFont = Pick<FontDetails, "name" | "weight" | "style" | "unicodeRange"> & {
    key: string;
};

export type Matrix = [number, number, number, number, number, number];

interface NodeBase {
    readonly bounds: Rect;
    readonly drawables: readonly Drawable[];
    readonly element: ElementInfo | undefined;
    readonly height: number;
    readonly parent: BoxNode | undefined;
    readonly transform: Matrix;
    readonly width: number;
}

// Warning: (ae-forgotten-export) The symbol "DocumentInput" needs to be exported
export type NodeInput = DocumentInput;

export type Paint = {
    readonly type: "color";
    readonly color: Rgba;
} | Gradient | ImagePaint |
/** A tile repeated at every `x` and `y` pair, showing only inside `area`: CSS background and mask layers. */
    {
    readonly type: "pattern";
    readonly tile: Gradient | ImagePaint;
    readonly tileWidth: number;
    readonly tileHeight: number;
    readonly x: readonly number[];
    readonly y: readonly number[];
    readonly area: Rect;
};

export function paint(document: PreparedSource): Promise<PaintTree>;

export function paint(node: NodeInput, options?: PaintOptions): Promise<PaintTree>;

class Painter$1 {
    [Symbol.dispose](): void;
    constructor(options?: FontCacheOptions | null);
    cachedFonts(key: string, pin: boolean): RegisteredFamily[] | undefined;
    // Warning: (ae-forgotten-export) The symbol "FontInput" needs to be exported
    cacheFont(font: FontInput, key: string): RegisteredFamily[];
    cacheFontFile(url: string, data: Uint8Array): void;
    clearFonts(): void;
    free(): void;
    // Warning: (ae-forgotten-export) The symbol "PaintOptions$1" needs to be exported
    // Warning: (ae-forgotten-export) The symbol "ListedFont" needs to be exported
    // Warning: (ae-forgotten-export) The symbol "RenderNeeds" needs to be exported
    needs(node: Node_2 | string | {
        json: string;
    }, options?: PaintOptions$1 | null, fonts?: ListedFont[] | null): RenderNeeds;
    // Warning: (ae-forgotten-export) The symbol "RenderedPaint" needs to be exported
    paint(node: Node_2 | string | {
        json: string;
    }, options?: PaintOptions$1 | null): RenderedPaint;
    registerFont(font: FontInput, key?: string | null): RegisteredFamily[];
    unregisterFont(family: string): boolean;
}

// Warning: (ae-forgotten-export) The symbol "WasmRenderer" needs to be exported
// Warning: (ae-forgotten-export) The symbol "Painter$1" needs to be exported
export class Painter extends WasmRenderer<RegisteredFamily, Painter$1> {
    constructor(options?: FontCacheOptions);
    paint(document: PreparedSource): Promise<PaintTree>;
    paint(node: NodeInput, options?: PaintOptions): Promise<PaintTree>;
    prepare(node: NodeInput, options?: PrepareOptions): Promise<PreparedSource>;
}

export type PaintNode = BoxNode | TextNode | ImageNode;

type PaintOptions$1 = {
    width?: number;
    height?: number;
    devicePixelRatio?: number;
    images?: ImageInput[];
    css?: CssInput[];
    fontFamilies?: string[];
    failedFontFiles?: string[];
    lang?: string;
};

export type PaintOptions = {
    width?: number;
    height?: number;
    devicePixelRatio?: number;
    fonts?: FontLoader[];
    images?: ImagesInput;
    css?: CssInput | readonly CssInput[];
    fontFamilies?: string[];
    lang?: string;
};

// Warning: (ae-forgotten-export) The symbol "DrawStep" needs to be exported
// Warning: (ae-forgotten-export) The symbol "BeginGroupStep" needs to be exported
// Warning: (ae-forgotten-export) The symbol "BeginClipStep" needs to be exported
export type PaintStep = DrawStep | BeginGroupStep | {
    readonly type: "end-group";
    readonly node: BoxNode;
} | BeginClipStep | {
    readonly type: "end-clip";
    readonly node: BoxNode;
};

export interface PaintTree {
    [Symbol.dispose](): void;
    readonly fonts: readonly Font[];
    free(): void;
    readonly height: number;
    readonly nodes: readonly PaintNode[];
    readonly root: BoxNode;
    readonly steps: readonly PaintStep[];
    toJSON(): SerializedPaintTree;
    readonly width: number;
}

export type Point = {
    readonly x: number;
    readonly y: number;
};

export function prepare(node: NodeInput, options?: PrepareOptions): Promise<PreparedSource>;

export class Prepared<TSource = unknown, TLayout = unknown> {
}

// Warning: (ae-forgotten-export) The symbol "JsonSource" needs to be exported
export type PreparedSource = Prepared<Node_2 | string | JsonSource, Omit<PaintOptions, "fonts" | "images" | "fontFamilies" | "css"> & {
    css?: CssInput[];
}>;

// @internal
export type PrepareImagesInput = ImageLoader[] | (Omit<FetchOptions, "signal"> & {
    sources?: ImageLoader[];
    fetchCache?: ImageFetchCache;
    cache?: NonNullable<ImageLoader["cache"]>;
    throwOnError?: boolean;
});

export type PrepareOptions = Omit<PaintOptions, "images"> & {
    images?: PrepareImagesInput;
    signal?: AbortSignal;
};

type RawDrawable<Run = number> =
/** With `blendMode`, blends with what the enclosing group already holds, as `background-blend-mode` does. */
    {
    readonly type: "fill";
    readonly role: Role;
    readonly shape: Shape;
    readonly paint: Paint;
    readonly blendMode?: BlendMode;
    readonly clips?: readonly Shape[];
} | {
    readonly type: "stroke";
    readonly role: Role;
    readonly shape: Shape;
    readonly stroke: Stroke;
    readonly paint: Paint;
    readonly clips?: readonly Shape[];
} |
/**
* A blurred copy of `shape`, moved by `offset`, visible only on one side of `box`: outside for
* an outer box shadow, inside for an inset one. `blur` is the Gaussian's standard deviation.
* With `stroke`, the shape is stroked instead of filled, as a dashed or wavy text decoration
* casts.
*/
    {
    readonly type: "shadow";
    readonly role: Role;
    readonly shape: Shape;
    readonly stroke?: Stroke;
    readonly offset: Point;
    readonly blur: number;
    readonly color: Rgba;
    readonly visible: "outside" | "inside";
    readonly box: Shape;
} |
/**
* A run's glyphs filled with `paint`, which sits in the node's space. With `blur`, a text
* shadow. With `stroke`, the outlines are stroked instead of filled.
*/
    {
    readonly type: "glyphs";
    readonly role: Role;
    readonly run: Run;
    readonly paint: Paint;
    readonly offset: Point;
    readonly blur: number;
    readonly stroke?: Stroke;
} | {
    readonly type: "image";
    readonly role: Role;
    readonly image: ImageSource;
    readonly rect: Rect;
    readonly clip: Shape;
    readonly sampling: Sampling;
} |
/** `content` kept only where `mask` covers, as `destination-in` compositing keeps it. */
    {
    readonly type: "masked";
    readonly role: Role;
    readonly mask: readonly RawDrawable<Run>[];
    readonly content: readonly RawDrawable<Run>[];
} |
/** Draw `drawables` into a layer, then composite it at `opacity`. */
    {
    readonly type: "group";
    readonly opacity: number;
    readonly drawables: readonly RawDrawable<Run>[];
};

type RawEffects<Drawable = RawDrawable> = {
    readonly opacity: number;
    readonly blendMode: BlendMode;
    readonly isolation: boolean;
    readonly filters: readonly Filter[];
    readonly backdropFilters: readonly Filter[];
    readonly backdropClip?: Shape;
    readonly clip?: Shape;
    readonly mask?: readonly Drawable[];
};

type RawFont = {
    readonly family?: string;
    readonly weight: number;
    readonly style: "normal" | "italic" | "oblique";
    readonly stretch: number;
    readonly variationSettings: readonly {
        readonly tag: string;
        readonly value: number;
    }[];
    readonly faceIndex: number;
};

type RawNodeBase = {
    readonly id: number;
    readonly parent?: number;
    readonly element?: ElementInfo;
    readonly transform: Matrix;
    readonly width: number;
    readonly height: number;
    readonly bounds: Rect;
    readonly drawables: readonly RawDrawable[];
    readonly children: readonly number[];
};

// Warning: (ae-forgotten-export) The symbol "RawNodeBase" needs to be exported
type RawPaintNode = (RawNodeBase & {
    readonly type: "box";
    readonly contentBox: Rect;
    readonly outline: readonly RawDrawable[];
    readonly effects?: RawEffects;
    readonly overflowClip?: Shape;
}) | (RawNodeBase & {
    readonly type: "text";
    readonly textAlign: "left" | "right" | "center" | "justify";
    readonly runs: readonly RawTextRun[];
}) | (RawNodeBase & {
    readonly type: "image";
    readonly image: ImageSource;
});

type RawPaintStep = {
    readonly type: "draw";
    readonly node: number;
    readonly part: "drawables" | "outline";
} | {
    readonly type: "begin-group" | "end-group" | "begin-clip" | "end-clip";
    readonly node: number;
};

type RawPaintTree = {
    readonly width: number;
    readonly height: number;
    readonly nodes: readonly RawPaintNode[];
    readonly fonts: readonly RawFont[];
    readonly steps: readonly RawPaintStep[];
};

type RawTextRun = {
    readonly text: string;
    readonly element?: ElementInfo;
    readonly x: number;
    readonly y: number;
    readonly width: number;
    readonly line: number;
    readonly ascent: number;
    readonly descent: number;
    readonly font: number;
    readonly fontSize: number;
    readonly lineHeight: number;
    readonly letterSpacing: number;
    readonly glyphs: readonly Glyph[];
    readonly transform?: Matrix;
};

export type Rect = {
    readonly x: number;
    readonly y: number;
    readonly width: number;
    readonly height: number;
};

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

class RenderedPaint {
    [Symbol.dispose](): void;
    fontData(index: number): Uint8Array | undefined;
    free(): void;
    runOutlines(node: number, run: number): (string | null)[] | undefined;
    // Warning: (ae-forgotten-export) The symbol "RawPaintTree" needs to be exported
    tree(): RawPaintTree;
}

type RenderNeeds = {
    images: string[];
    fonts: number[];
};

export type Rgba = [number, number, number, number];

export type Role = "background" | "border" | "box-shadow" | "outline" | "image" | "text" | "text-shadow" | "text-stroke" | "text-decoration" | "inline-background";

export type Sampling = "smooth" | "pixelated";

const SERIALIZED_VERSION = 1;

export type SerializedPaintTree = {
    readonly version: typeof SERIALIZED_VERSION;
    readonly tree: RawPaintTree;
    readonly outlines: readonly (readonly (readonly (string | null)[])[] | null)[];
};

export type Shape = {
    readonly type: "rect";
    readonly rect: Rect;
} | {
    readonly type: "rounded-rect";
    readonly rect: Rect;
    readonly radii: CornerRadii;
} |
/** SVG path data, the string `new Path2D()` takes. */
    {
    readonly type: "path";
    readonly d: string;
    readonly fillRule: "nonzero" | "evenodd";
};

export type Spread = "pad" | "repeat";

export type Stroke = {
    readonly width: number;
    readonly dash?: readonly number[];
    readonly cap: "butt" | "round";
    readonly join: "miter" | "round" | "bevel";
};

type SyncInitInput = BufferSource | WebAssembly.Module;

export interface TextNode extends NodeBase {
    readonly runs: readonly TextRun[];
    readonly textAlign: "left" | "right" | "center" | "justify";
    readonly type: "text";
}

export interface TextRun {
    readonly ascent: number;
    readonly descent: number;
    readonly element?: ElementInfo;
    readonly font: Font;
    readonly fontSize: number;
    readonly glyphs: readonly Glyph[];
    readonly letterSpacing: number;
    readonly line: number;
    readonly lineHeight: number;
    outlines(): readonly (string | null)[];
    readonly text: string;
    readonly transform?: Matrix;
    readonly width: number;
    readonly x: number;
    readonly y: number;
}

// @internal
type WasmBinding<TFamily extends RegisteredFamilyLike> = FontBackend<TFamily> & {
    unregisterFont(family: string): boolean;
    clearFonts(): void;
    free(): void;
};

// Warning: (ae-forgotten-export) The symbol "WasmBinding" needs to be exported
// @internal
class WasmRenderer<TFamily extends RegisteredFamilyLike, TInner extends WasmBinding<TFamily>> {
    [Symbol.dispose](): void;
    constructor(inner: TInner);
    clearFonts(): void;
    // Warning: (ae-forgotten-export) The symbol "FontRegistry" needs to be exported
    protected readonly fonts: FontRegistry<TFamily>;
    free(): void;
    protected readonly inner: TInner;
    registerFont(font: FontLoader): Promise<TFamily[]>;
    unregisterFont(family: RegisteredFamilyLike): boolean;
}

// Warnings were encountered during analysis:
// dist/export.d.mts:78:3 - (ae-forgotten-export) The symbol "ImageCacheMode$1" needs to be exported
// dist/export.d.mts:124:3 - (ae-forgotten-export) The symbol "Font$1" needs to be exported
// dist/export.d.mts:124:3 - (ae-forgotten-export) The symbol "Awaitable" needs to be exported
// dist/export.d.mts:165:5 - (ae-forgotten-export) The symbol "ImageSource$1" needs to be exported
// dist/export.d.mts:238:3 - (ae-forgotten-export) The symbol "ByteBuf" needs to be exported
// dist/export.d.mts:268:3 - (ae-forgotten-export) The symbol "ImageCacheMode" needs to be exported
// dist/export.d.mts:278:3 - (ae-forgotten-export) The symbol "ImageInput" needs to be exported
// dist/export.d.mts:577:3 - (ae-forgotten-export) The symbol "RawTextRun" needs to be exported
// dist/export.d.mts:600:3 - (ae-forgotten-export) The symbol "RawPaintNode" needs to be exported
// dist/export.d.mts:602:3 - (ae-forgotten-export) The symbol "RawPaintStep" needs to be exported
// dist/export.d.mts:847:3 - (ae-forgotten-export) The symbol "SERIALIZED_VERSION" needs to be exported
// dist/export.d.mts:891:3 - (ae-forgotten-export) The symbol "InitInput" needs to be exported
// dist/export.d.mts:895:3 - (ae-forgotten-export) The symbol "SyncInitInput" needs to be exported
```

## `takumi-paint/takumi_paint_wasm_bg.wasm`

```ts
export const __externref_drop_slice: (a: number, b: number) => void;

export const __externref_table_alloc: () => number;

export const __externref_table_dealloc: (a: number) => void;

export const __wbg_painter_free: (a: number, b: number) => void;

export const __wbg_renderedpaint_free: (a: number, b: number) => void;

export const __wbindgen_exn_store: (a: number) => void;

export const __wbindgen_externrefs: WebAssembly.Table;

export const __wbindgen_free: (a: number, b: number, c: number) => void;

export const __wbindgen_malloc: (a: number, b: number) => number;

export const __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;

export const __wbindgen_start: () => void;

export const memory: WebAssembly.Memory;

export const painter_cachedFonts: (a: number, b: number, c: number, d: number) => [number, number, number];

export const painter_cacheFont: (a: number, b: any, c: number, d: number) => [number, number, number];

export const painter_cacheFontFile: (a: number, b: number, c: number, d: number, e: number) => [number, number];

export const painter_clearFonts: (a: number) => [number, number];

export const painter_needs: (a: number, b: any, c: number, d: number) => [number, number, number];

export const painter_new: (a: number) => [number, number, number];

export const painter_paint: (a: number, b: any, c: number) => [number, number, number];

export const painter_registerFont: (a: number, b: any, c: number, d: number) => [number, number, number];

export const painter_unregisterFont: (a: number, b: number, c: number) => number;

export const renderedpaint_fontData: (a: number, b: number) => any;

export const renderedpaint_runOutlines: (a: number, b: number, c: number) => [number, number];

export const renderedpaint_tree: (a: number) => any;
```

## `takumi-paint/wasm-url`

```ts
const url: URL;
export default url;
```
