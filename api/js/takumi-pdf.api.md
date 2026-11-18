# takumi-pdf

## `takumi-pdf`, `takumi-pdf/next`, `takumi-pdf/no-init`

```ts
import { CssInput } from '@takumi-rs/helpers';
import { CSSProperties } from 'react';
import { FetchOptions } from '@takumi-rs/helpers';
import { GenericFontFamily } from '@takumi-rs/helpers';
import { ImageFetchCache } from '@takumi-rs/helpers';
import { Node as Node_2 } from '@takumi-rs/helpers';
import { ReactElement } from 'react';
import { ReactElementLike } from '@takumi-rs/helpers';
import { ReactNode } from 'react';

// Warning: (ae-forgotten-export) The symbol "InitInput" needs to be exported
// Warning: (ae-forgotten-export) The symbol "InitOutput" needs to be exported
function __wbg_init(module_or_path?: {
    module_or_path: InitInput | Promise<InitInput>;
} | InitInput | Promise<InitInput>): Promise<InitOutput>;
export default __wbg_init;

export type ArchivalAttachment = Attachment & {
    mimeType: string;
    description: string;
};

type Attachment$1 = {
    name: string;
    data: Uint8Array | string;
    mimeType?: string;
    description?: string;
    relationship?: "source" | "data" | "alternative" | "supplement" | "unspecified";
    modificationDate?: string;
};

export type Attachment = {
    name: string;
    data: Uint8Array | string;
    mimeType?: string;
    description?: string;
    relationship?: "source" | "data" | "alternative" | "supplement" | "unspecified";
    modificationDate?: string;
};

type Awaitable<T> = T | Promise<T>;

type Band = Node_2 | string | false;

export type BandInput = NodeInput | false;

// @internal
type ByteBuf$1 = Uint8Array | ArrayBuffer | Buffer;

type ByteBuf = Uint8Array | ArrayBuffer | Buffer;

type ConformanceOptions = {
    pdfa?: never;
    tagged?: boolean | "ua1" | "ua2";
    attachments?: Attachment[];
} | {
    pdfa: "2b" | "2u";
    tagged?: boolean | "ua1";
    attachments?: never;
} | {
    pdfa: "2a";
    tagged?: true | "ua1";
    attachments?: never;
} | {
    pdfa: "3b" | "3u";
    tagged?: boolean | "ua1";
    attachments?: ArchivalAttachment[];
} | {
    pdfa: "3a";
    tagged?: true | "ua1";
    attachments?: ArchivalAttachment[];
} | {
    pdfa: "4";
    tagged?: boolean | "ua2";
    attachments?: never;
} | {
    pdfa: "4f";
    tagged?: boolean | "ua2";
    attachments?: ArchivalAttachment[];
};

export type CounterProps = {
    format?: CounterStyle;
    className?: string;
    style?: CSSProperties;
    tw?: string;
};

export type CounterStyle = "decimal" | "decimal-leading-zero" | "lower-roman" | "upper-roman" | "lower-alpha" | "lower-latin" | "upper-alpha" | "upper-latin" | "lower-greek" | "hiragana" | "katakana" | "trad-chinese-informal" | "cjk-ideographic" | "cjk-decimal" | "arabic-indic" | "bengali" | "cambodian" | "khmer" | "devanagari" | "gujarati" | "gurmukhi" | "kannada" | "lao" | "malayalam" | "mongolian" | "myanmar" | "oriya" | "persian" | "urdu" | "tamil" | "telugu" | "thai" | "tibetan";

type Dimensions$1 = {
    width: number;
    height: number;
};

export type Dimensions = {
    width: number;
    height: number;
};

type DistributiveOmit<T, K extends PropertyKey> = T extends unknown ? Omit<T, K> : never;

// @internal
type DocumentInput = Node_2 | ReactNode | ReactElementLike | string;

// Warning: (ae-forgotten-export) The symbol "DistributiveOmit" needs to be exported
type DocumentLayout = DistributiveOmit<PrepareOptions, "fonts" | "images" | "signal" | "fontFamilies" | "css" | "header" | "footer" | "pages"> & {
    header?: Node_2 | string;
    footer?: Node_2 | string;
    pages?: PageRules;
    css?: CssInput[];
};

// Warning: (ae-forgotten-export) The symbol "FontDetails$1" needs to be exported
// Warning: (ae-forgotten-export) The symbol "ByteBuf$1" needs to be exported
// @internal
type Font$1 = FontDetails$1 | ByteBuf$1;

// Warning: (ae-forgotten-export) The symbol "FontDetails" needs to be exported
// Warning: (ae-forgotten-export) The symbol "ByteBuf" needs to be exported
type Font = FontDetails | ByteBuf;

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

type ImageCacheMode$1 = "auto" | "none";

type ImageCacheMode = "auto" | "none";

// @internal
type ImageLoader = Omit<ImageSource$1, "data"> & {
    data: ImageSource$1["data"] | (() => ImageSource$1["data"] | Promise<ImageSource$1["data"]>);
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

type ImageSource = {
    src: string;
    data: ByteBuf;
    cache?: ImageCacheMode;
};

type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

interface InitOutput {
    readonly __externref_table_alloc: () => number;
    readonly __externref_table_dealloc: (a: number) => void;
    readonly __wbg_pdfrenderer_free: (a: number, b: number) => void;
    readonly __wbindgen_exn_store: (a: number) => void;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_start: () => void;
    readonly memory: WebAssembly.Memory;
    readonly pdfrenderer_cachedFonts: (a: number, b: number, c: number, d: number) => [number, number, number];
    readonly pdfrenderer_cacheFont: (a: number, b: any, c: number, d: number) => [number, number, number];
    readonly pdfrenderer_cacheFontFile: (a: number, b: number, c: number, d: number, e: number) => [number, number];
    readonly pdfrenderer_clearFonts: (a: number) => [number, number];
    readonly pdfrenderer_measure: (a: number, b: any, c: number) => [number, number, number];
    readonly pdfrenderer_needs: (a: number, b: any, c: number, d: number) => [number, number, number];
    readonly pdfrenderer_new: (a: number) => [number, number, number];
    readonly pdfrenderer_registerFont: (a: number, b: any, c: number, d: number) => [number, number, number];
    readonly pdfrenderer_render: (a: number, b: any, c: number) => [number, number, number, number];
    readonly pdfrenderer_unregisterFont: (a: number, b: number, c: number) => number;
}

// Warning: (ae-forgotten-export) The symbol "SyncInitInput" needs to be exported
export function initSync(module: {
    module: SyncInitInput;
} | SyncInitInput): InitOutput;

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

export function measure(document: PreparedSource): Promise<MeasuredSize>;

export function measure(node: NodeInput, options?: MeasureOptions): Promise<MeasuredSize>;

type MeasuredSize$1 = {
    width: number;
    height: number;
};

export type MeasuredSize = {
    width: number;
    height: number;
};

// Warning: (ae-forgotten-export) The symbol "PdfRenderOptions" needs to be exported
type MeasureOptions$1 = ({
    size?: PageSize$1;
    landscape?: boolean;
    viewport?: never;
} | {
    viewport: ViewportInput$1;
    size?: never;
    landscape?: never;
}) & Pick<PdfRenderOptions, "images" | "css" | "cssVariables" | "fontFamilies" | "lang">;

export type MeasureOptions = ({
    size?: PageSize;
    landscape?: boolean;
    viewport?: never;
} | {
    viewport: ViewportInput;
    size?: never;
    landscape?: never;
}) & {
    fonts?: FontLoader[];
    images?: ImagesInput;
    css?: CssInput | readonly CssInput[];
    fontFamilies?: string[];
    lang?: string;
};

// Warning: (ae-forgotten-export) The symbol "DocumentInput" needs to be exported
export type NodeInput = DocumentInput;

type OutputOnly = "metadata" | "outline" | "backgroundColor" | "uncoveredText" | "pdfa" | "tagged" | "attachments";

type PagedOptions = {
    viewport?: never;
    size?: PageSize;
    landscape?: boolean;
    margin?: PageMargin;
    header?: BandInput;
    footer?: BandInput;
    pages?: PageRulesInput;
    pageRanges?: PageRange[];
};

// Warning: (ae-forgotten-export) The symbol "PageMarginSide$1" needs to be exported
type PageMargin$1 = PageMarginSide$1 | {
    top?: PageMarginSide$1;
    right?: PageMarginSide$1;
    bottom?: PageMarginSide$1;
    left?: PageMarginSide$1;
};

export type PageMargin = PageMarginSide | {
    top?: PageMarginSide;
    right?: PageMarginSide;
    bottom?: PageMarginSide;
    left?: PageMarginSide;
};

type PageMarginSide$1 = number | "auto";

export type PageMarginSide = number | "auto";

export function PageNumber(props?: CounterProps): ReactElement;

type PageOverride = {
    header?: Band;
    footer?: Band;
};

export type PageOverrideInput = {
    header?: BandInput;
    footer?: BandInput;
};

type PageRange$1 = number | {
    from?: number;
    to?: number;
};

export type PageRange = number | {
    from?: number;
    to?: number;
};

type PageRules = {
    first?: PageOverride;
    last?: PageOverride;
    odd?: PageOverride;
    even?: PageOverride;
};

export type PageRulesInput = {
    first?: PageOverrideInput;
    last?: PageOverrideInput;
    odd?: PageOverrideInput;
    even?: PageOverrideInput;
};

// Warning: (ae-forgotten-export) The symbol "PageSizeName$1" needs to be exported
// Warning: (ae-forgotten-export) The symbol "Dimensions$1" needs to be exported
type PageSize$1 = PageSizeName$1 | Dimensions$1;

export type PageSize = PageSizeName | Dimensions;

type PageSizeName$1 = "a3" | "a4" | "a5" | "b4" | "b5" | "jis-b4" | "jis-b5" | "ledger" | "legal" | "letter";

export type PageSizeName = "a3" | "a4" | "a5" | "b4" | "b5" | "jis-b4" | "jis-b5" | "ledger" | "legal" | "letter";

type Pdfa = "2a" | "2b" | "2u" | "3a" | "3b" | "3u" | "4" | "4f";

type PdfMetadata$1 = {
    title?: string;
    description?: string;
    authors?: string[];
    keywords?: string[];
    creator?: string;
    creationDate?: string;
};

export type PdfMetadata = {
    title?: string;
    description?: string;
    authors?: string[];
    keywords?: string[];
    creator?: string;
    creationDate?: string;
    xmp?: XmpSchema[];
};

class PdfRenderer$1 {
    [Symbol.dispose](): void;
    constructor(options?: FontCacheOptions | null);
    cachedFonts(key: string, pin: boolean): RegisteredFamily[] | undefined;
    // Warning: (ae-forgotten-export) The symbol "Font" needs to be exported
    // Warning: (ae-forgotten-export) The symbol "RegisteredFamily" needs to be exported
    cacheFont(font: Font, key: string): RegisteredFamily[];
    cacheFontFile(url: string, data: Uint8Array): void;
    clearFonts(): void;
    free(): void;
    // Warning: (ae-forgotten-export) The symbol "MeasureOptions$1" needs to be exported
    // Warning: (ae-forgotten-export) The symbol "MeasuredSize$1" needs to be exported
    measure(node: Node_2 | string | {
        json: string;
    }, options?: MeasureOptions$1 | null): MeasuredSize$1;
    // Warning: (ae-forgotten-export) The symbol "ListedFont" needs to be exported
    // Warning: (ae-forgotten-export) The symbol "RenderNeeds" needs to be exported
    needs(node: Node_2 | string | {
        json: string;
    }, options?: PdfRenderOptions | null, fonts?: ListedFont[] | null): RenderNeeds;
    registerFont(font: Font, key?: string | null): RegisteredFamily[];
    render(node: Node_2 | string | {
        json: string;
    }, options?: PdfRenderOptions | null): Uint8Array<ArrayBuffer>;
    unregisterFont(family: string): boolean;
}

// Warning: (ae-forgotten-export) The symbol "WasmRenderer" needs to be exported
// Warning: (ae-forgotten-export) The symbol "PdfRenderer$1" needs to be exported
export class PdfRenderer extends WasmRenderer<RegisteredFamily, PdfRenderer$1> {
    constructor(options?: FontCacheOptions);
    measure(document: PreparedSource): Promise<MeasuredSize>;
    measure(node: NodeInput, options?: MeasureOptions): Promise<MeasuredSize>;
    prepare(node: NodeInput, options?: PrepareOptions): Promise<PreparedSource>;
    render(document: PreparedSource, options?: PreparedRenderOptions): Promise<Uint8Array>;
    render(node: NodeInput, options?: RenderOptions): Promise<Uint8Array>;
}

type PdfRenderOptions = {
    viewport?: ViewportInput$1;
    size?: PageSize$1;
    landscape?: boolean;
    margin?: PageMargin$1;
    header?: Band;
    footer?: Band;
    pages?: PageRules;
    pageRanges?: PageRange$1[];
    images?: ImageSource[];
    css?: CssInput[];
    cssVariables?: Record<string, string>;
    fontFamilies?: string[];
    failedFontFiles?: string[];
    lang?: string;
    metadata?: PdfMetadata$1;
    outline?: boolean;
    pdfa?: Pdfa;
    tagged?: Tagged;
    attachments?: Attachment$1[];
    uncoveredText?: UncoveredText$1;
};

export function prepare(node: NodeInput, options?: PrepareOptions): Promise<PreparedSource>;

export class Prepared<TSource = unknown, TLayout = unknown> {
}

// Warning: (ae-forgotten-export) The symbol "ConformanceOptions" needs to be exported
// Warning: (ae-forgotten-export) The symbol "PagedOptions" needs to be exported
export type PreparedRenderOptions = ConformanceOptions & Pick<PagedOptions & RenderOptions, "metadata" | "outline" | "backgroundColor" | "uncoveredText"> & {
    signal?: AbortSignal;
};

// Warning: (ae-forgotten-export) The symbol "JsonSource" needs to be exported
// Warning: (ae-forgotten-export) The symbol "DocumentLayout" needs to be exported
export type PreparedSource = Prepared<Node_2 | string | JsonSource, DocumentLayout>;

// @internal
export type PrepareImagesInput = ImageLoader[] | (Omit<FetchOptions, "signal"> & {
    sources?: ImageLoader[];
    fetchCache?: ImageFetchCache;
    cache?: NonNullable<ImageLoader["cache"]>;
    throwOnError?: boolean;
});

// Warning: (ae-forgotten-export) The symbol "OutputOnly" needs to be exported
export type PrepareOptions = DistributiveOmit<RenderOptions, OutputOnly | "images"> & {
    images?: PrepareImagesInput;
    signal?: AbortSignal;
};

type RegisteredFace = {
    weight: number;
    style: string;
    width: number;
    index: number;
};

type RegisteredFamily = {
    name: string;
    faces: RegisteredFace[];
};

// @internal
type RegisteredFamilyLike = {
    name: string;
};

export function render(document: PreparedSource, options?: PreparedRenderOptions): Promise<Uint8Array>;

export function render(node: NodeInput, options?: RenderOptions): Promise<Uint8Array>;

type RenderNeeds = {
    images: string[];
    fonts: number[];
};

// Warning: (ae-forgotten-export) The symbol "ViewportOptions" needs to be exported
export type RenderOptions = (PagedOptions | ViewportOptions) & ConformanceOptions & {
    fonts?: FontLoader[];
    images?: ImagesInput;
    css?: CssInput | readonly CssInput[];
    fontFamilies?: string[];
    lang?: string;
    metadata?: PdfMetadata;
    outline?: boolean;
    backgroundColor?: string;
    uncoveredText?: UncoveredText;
    signal?: AbortSignal;
};

type SyncInitInput = BufferSource | WebAssembly.Module;

type Tagged = boolean | "ua1" | "ua2";

export function TargetPageNumber(input: CounterProps & {
    href?: string;
}): ReactElement;

export function TotalPages(props?: CounterProps): ReactElement;

type UncoveredText$1 = "error" | "placeholder" | "blank";

export type UncoveredText = "error" | "placeholder" | "blank";

type ViewportInput$1 = {
    width: number;
    height?: number;
};

export type ViewportInput = {
    width: number;
    height?: number;
};

type ViewportOptions = {
    viewport: ViewportInput;
    size?: never;
    landscape?: never;
    margin?: never;
    header?: never;
    footer?: never;
    pages?: never;
    pageRanges?: never;
};

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

export type XmpProperty = {
    name: string;
    value: string;
    description: string;
};

export type XmpSchema = {
    name: string;
    prefix: string;
    namespace: string;
    properties: XmpProperty[];
};

// Warnings were encountered during analysis:
// dist/export.d.mts:79:3 - (ae-forgotten-export) The symbol "ImageCacheMode$1" needs to be exported
// dist/export.d.mts:125:3 - (ae-forgotten-export) The symbol "Font$1" needs to be exported
// dist/export.d.mts:125:3 - (ae-forgotten-export) The symbol "Awaitable" needs to be exported
// dist/export.d.mts:166:5 - (ae-forgotten-export) The symbol "ImageSource$1" needs to be exported
// dist/export.d.mts:263:3 - (ae-forgotten-export) The symbol "RegisteredFace" needs to be exported
// dist/export.d.mts:269:3 - (ae-forgotten-export) The symbol "ImageCacheMode" needs to be exported
// dist/export.d.mts:295:3 - (ae-forgotten-export) The symbol "Band" needs to be exported
// dist/export.d.mts:300:3 - (ae-forgotten-export) The symbol "PageOverride" needs to be exported
// dist/export.d.mts:365:3 - (ae-forgotten-export) The symbol "PageMargin$1" needs to be exported
// dist/export.d.mts:379:3 - (ae-forgotten-export) The symbol "PageRange$1" needs to be exported
// dist/export.d.mts:381:3 - (ae-forgotten-export) The symbol "ImageSource" needs to be exported
// dist/export.d.mts:396:3 - (ae-forgotten-export) The symbol "PdfMetadata$1" needs to be exported
// dist/export.d.mts:400:3 - (ae-forgotten-export) The symbol "Pdfa" needs to be exported
// dist/export.d.mts:402:3 - (ae-forgotten-export) The symbol "Tagged" needs to be exported
// dist/export.d.mts:404:3 - (ae-forgotten-export) The symbol "Attachment$1" needs to be exported
// dist/export.d.mts:413:3 - (ae-forgotten-export) The symbol "UncoveredText$1" needs to be exported
// dist/export.d.mts:419:3 - (ae-forgotten-export) The symbol "PageSize$1" needs to be exported
// dist/export.d.mts:423:3 - (ae-forgotten-export) The symbol "ViewportInput$1" needs to be exported
// dist/export.d.mts:862:3 - (ae-forgotten-export) The symbol "PageRules" needs to be exported
```

## `takumi-pdf/primitives`

```ts
import { CSSProperties } from 'react';
import { ReactElement } from 'react';

export type CounterProps = {
    format?: CounterStyle;
    className?: string;
    style?: CSSProperties;
    tw?: string;
};

export type CounterStyle = "decimal" | "decimal-leading-zero" | "lower-roman" | "upper-roman" | "lower-alpha" | "lower-latin" | "upper-alpha" | "upper-latin" | "lower-greek" | "hiragana" | "katakana" | "trad-chinese-informal" | "cjk-ideographic" | "cjk-decimal" | "arabic-indic" | "bengali" | "cambodian" | "khmer" | "devanagari" | "gujarati" | "gurmukhi" | "kannada" | "lao" | "malayalam" | "mongolian" | "myanmar" | "oriya" | "persian" | "urdu" | "tamil" | "telugu" | "thai" | "tibetan";

export function PageNumber(props?: CounterProps): ReactElement;

export function TargetPageNumber(input: CounterProps & {
    href?: string;
}): ReactElement;

export function TotalPages(props?: CounterProps): ReactElement;
```

## `takumi-pdf/takumi_pdf_wasm_bg.wasm`

```ts
export const __externref_table_alloc: () => number;

export const __externref_table_dealloc: (a: number) => void;

export const __wbg_pdfrenderer_free: (a: number, b: number) => void;

export const __wbindgen_exn_store: (a: number) => void;

export const __wbindgen_externrefs: WebAssembly.Table;

export const __wbindgen_free: (a: number, b: number, c: number) => void;

export const __wbindgen_malloc: (a: number, b: number) => number;

export const __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;

export const __wbindgen_start: () => void;

export const memory: WebAssembly.Memory;

export const pdfrenderer_cachedFonts: (a: number, b: number, c: number, d: number) => [number, number, number];

export const pdfrenderer_cacheFont: (a: number, b: any, c: number, d: number) => [number, number, number];

export const pdfrenderer_cacheFontFile: (a: number, b: number, c: number, d: number, e: number) => [number, number];

export const pdfrenderer_clearFonts: (a: number) => [number, number];

export const pdfrenderer_measure: (a: number, b: any, c: number) => [number, number, number];

export const pdfrenderer_needs: (a: number, b: any, c: number, d: number) => [number, number, number];

export const pdfrenderer_new: (a: number) => [number, number, number];

export const pdfrenderer_registerFont: (a: number, b: any, c: number, d: number) => [number, number, number];

export const pdfrenderer_render: (a: number, b: any, c: number) => [number, number, number, number];

export const pdfrenderer_unregisterFont: (a: number, b: number, c: number) => number;
```

## `takumi-pdf/wasm-url`

```ts
const url: URL;
export default url;
```
