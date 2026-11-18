# takumi-js

## `takumi-js`

```ts
import { AnimationOutputFormat } from '@takumi-rs/core';
import { AnimationRule } from '@takumi-rs/helpers';
import { ContainerNode } from '@takumi-rs/helpers';
import { CssInput } from '@takumi-rs/helpers';
import { Declarations } from '@takumi-rs/helpers';
import { DitheringAlgorithm } from '@takumi-rs/core';
import { EmojiType } from '@takumi-rs/helpers/emoji';
import { FetchOptions } from '@takumi-rs/helpers';
import { Font } from '@takumi-rs/core';
import { FontDetails } from '@takumi-rs/core';
import { FontLoader } from '@takumi-rs/core';
import { FromJsxOptions } from '@takumi-rs/helpers/jsx';
import { ImageFetchCache } from '@takumi-rs/helpers';
import { ImageNode } from '@takumi-rs/helpers';
import { ImageSource } from '@takumi-rs/core';
import { Keyframes } from '@takumi-rs/core';
import { KeyframesMap } from '@takumi-rs/core';
import { KeyframesRuleList } from '@takumi-rs/core';
import { LayerRule } from '@takumi-rs/helpers';
import { MeasuredNode } from '@takumi-rs/core';
import { MeasuredTextRun } from '@takumi-rs/core';
import { MediaRule } from '@takumi-rs/helpers';
import * as napi from '@takumi-rs/core';
import { Node as Node_2 } from '@takumi-rs/helpers';
import { NodeAttributes } from '@takumi-rs/helpers';
import { NodeMetadata } from '@takumi-rs/helpers';
import { OutputFormat } from '@takumi-rs/core';
import { ReactElementLike } from '@takumi-rs/helpers';
import { ReactNode } from 'react';
import { RgbaImage } from '@takumi-rs/helpers';
import { StyleRule } from '@takumi-rs/helpers';
import { SupportsRule } from '@takumi-rs/helpers';
import { TextNode } from '@takumi-rs/helpers';
import * as wasm from '@takumi-rs/wasm';

export { AnimationOutputFormat }

export { AnimationRule }

export type AnimationScene = Omit<napi.AnimationScene, "node"> & {
    node: RenderInput;
};

// Warning: (ae-forgotten-export) The symbol "RendererBindings" needs to be exported
type Backend = Pick<typeof napi, RendererBindings> | Pick<typeof wasm, RendererBindings>;

export { ContainerNode }

export { CssInput }

type CssOptions = {
    css?: CssInput | readonly CssInput[];
    stylesheets?: string[];
};

export { Declarations }

type DistributiveOmit<T, K extends PropertyKey> = T extends unknown ? Omit<T, K> : never;

export { DitheringAlgorithm }

export { FetchOptions }

export { Font }

export { FontDetails }

export { FontLoader }

type ImageLoader = napi.ImageLoader | wasm.ImageLoader;

export { ImageNode }

// Warning: (ae-forgotten-export) The symbol "ImageLoader" needs to be exported
export type ImagesInput = ImageLoader[] | (FetchOptions & {
    sources?: ImageLoader[];
    fetchCache?: ImageFetchCache;
    cache?: NonNullable<ImageLoader["cache"]>;
    throwOnError?: boolean;
});

export { ImageSource }

// Warning: (ae-forgotten-export) The symbol "DistributiveOmit" needs to be exported
// Warning: (ae-forgotten-export) The symbol "CssOptions" needs to be exported
type InnerRenderAnimationOptions = DistributiveOmit<napi.RenderAnimationOptions | wasm.RenderAnimationOptions, "images" | "css"> & CssOptions;

type InnerRenderOptions = DistributiveOmit<napi.RenderOptions | wasm.RenderOptions, "images" | "css"> & CssOptions;

type InnerSvgRenderOptions = DistributiveOmit<napi.SvgRenderOptions | wasm.SvgRenderOptions, "images" | "css"> & CssOptions;

export { Keyframes }

export { KeyframesMap }

export { KeyframesRuleList }

export { LayerRule }

// Warning: (ae-forgotten-export) The symbol "SharedRenderExtras" needs to be exported
// Warning: (ae-forgotten-export) The symbol "ManagedRendererOptions" needs to be exported
type Managed<TInner> = (TInner & SharedRenderExtras) | (TInner & Omit<SharedRenderExtras, "renderer"> & ManagedRendererOptions);

type ManagedRendererOptions = {
    module?: wasm.InitInput | Promise<wasm.InitInput> | {
        default: wasm.InitInput;
    };
};

export function measure(element: RenderInput, options?: RenderOptions): Promise<napi.MeasuredNode>;

export { MeasuredNode }

export { MeasuredTextRun }

export { MediaRule }

export { Node_2 as Node }

export { NodeAttributes }

export { NodeMetadata }

export { OutputFormat }

export { ReactElementLike }

export function render(element: RenderInput, options?: RenderOptions): Promise<Buffer<ArrayBuffer> | Uint8Array<ArrayBuffer>>;

export function renderAnimation(options: RenderAnimationOptions): Promise<Buffer<ArrayBuffer> | Uint8Array<ArrayBuffer>>;

// Warning: (ae-forgotten-export) The symbol "Managed" needs to be exported
// Warning: (ae-forgotten-export) The symbol "InnerRenderAnimationOptions" needs to be exported
export type RenderAnimationOptions = Managed<DistributiveOmit<InnerRenderAnimationOptions, "scenes"> & {
    scenes: AnimationScene[];
}>;

// Warning: (ae-forgotten-export) The symbol "Backend" needs to be exported
type Renderer$1 = InstanceType<Backend["Renderer"]>;

type RendererBindings = "Renderer" | "setGlyphCacheMaxBytes";

export type RenderInput = ReactNode | ReactElementLike | Node_2 | string;

// Warning: (ae-forgotten-export) The symbol "InnerRenderOptions" needs to be exported
export type RenderOptions = Managed<InnerRenderOptions>;

export function renderSvg(element: RenderInput, options?: RenderSvgOptions): Promise<string>;

// Warning: (ae-forgotten-export) The symbol "InnerSvgRenderOptions" needs to be exported
export type RenderSvgOptions = Managed<InnerSvgRenderOptions>;

export { RgbaImage }

export function setGlyphCacheMaxBytes(bytes: number): void;

type SharedRenderExtras = {
    renderer: Renderer$1;
    signal?: AbortSignal;
    jsx?: FromJsxOptions;
    images?: ImagesInput;
    emoji?: EmojiType | "from-font";
};

export { StyleRule }

export { SupportsRule }

export { TextNode }

// Warnings were encountered during analysis:
// dist/render-oPv4-TXB.d.mts:26:3 - (ae-forgotten-export) The symbol "Renderer$1" needs to be exported
```

## `takumi-js/response`

```ts
import { CssInput } from '@takumi-rs/helpers';
import { EmojiType } from '@takumi-rs/helpers/emoji';
import { FetchOptions } from '@takumi-rs/helpers';
import { FromJsxOptions } from '@takumi-rs/helpers/jsx';
import { ImageFetchCache } from '@takumi-rs/helpers';
import * as napi from '@takumi-rs/core';
import { Node as Node_2 } from '@takumi-rs/helpers';
import { ReactElementLike } from '@takumi-rs/helpers';
import { ReactNode } from 'react';
import * as wasm from '@takumi-rs/wasm';

// Warning: (ae-forgotten-export) The symbol "RendererBindings" needs to be exported
type Backend = Pick<typeof napi, RendererBindings> | Pick<typeof wasm, RendererBindings>;

type CssOptions = {
    css?: CssInput | readonly CssInput[];
    stylesheets?: string[];
};

type DistributiveOmit<T, K extends PropertyKey> = T extends unknown ? Omit<T, K> : never;

type ImageLoader = napi.ImageLoader | wasm.ImageLoader;

class ImageResponse extends Response {
    // Warning: (ae-forgotten-export) The symbol "RenderInput" needs to be exported
    constructor(component: RenderInput, options?: ImageResponseOptions);
    readonly ready: Promise<void>;
}
export { ImageResponse }
export default ImageResponse;

// Warning: (ae-forgotten-export) The symbol "RenderOptions" needs to be exported
export type ImageResponseOptions = RenderOptions & ResponseInit & {
    onError?: (error: unknown) => void | Promise<void>;
};

export type ImageResponseResult = Response & {
    readonly ready: Promise<void>;
};

// Warning: (ae-forgotten-export) The symbol "ImageLoader" needs to be exported
type ImagesInput = ImageLoader[] | (FetchOptions & {
    sources?: ImageLoader[];
    fetchCache?: ImageFetchCache;
    cache?: NonNullable<ImageLoader["cache"]>;
    throwOnError?: boolean;
});

// Warning: (ae-forgotten-export) The symbol "DistributiveOmit" needs to be exported
// Warning: (ae-forgotten-export) The symbol "CssOptions" needs to be exported
type InnerRenderOptions = DistributiveOmit<napi.RenderOptions | wasm.RenderOptions, "images" | "css"> & CssOptions;

// Warning: (ae-forgotten-export) The symbol "SharedRenderExtras" needs to be exported
// Warning: (ae-forgotten-export) The symbol "ManagedRendererOptions" needs to be exported
type Managed<TInner> = (TInner & SharedRenderExtras) | (TInner & Omit<SharedRenderExtras, "renderer"> & ManagedRendererOptions);

type ManagedRendererOptions = {
    module?: wasm.InitInput | Promise<wasm.InitInput> | {
        default: wasm.InitInput;
    };
};

// Warning: (ae-forgotten-export) The symbol "Backend" needs to be exported
type Renderer$1 = InstanceType<Backend["Renderer"]>;

type RendererBindings = "Renderer" | "setGlyphCacheMaxBytes";

type RenderInput = ReactNode | ReactElementLike | Node_2 | string;

// Warning: (ae-forgotten-export) The symbol "Managed" needs to be exported
// Warning: (ae-forgotten-export) The symbol "InnerRenderOptions" needs to be exported
type RenderOptions = Managed<InnerRenderOptions>;

type SharedRenderExtras = {
    renderer: Renderer$1;
    signal?: AbortSignal;
    jsx?: FromJsxOptions;
    images?: ImagesInput;
    emoji?: EmojiType | "from-font";
};

// Warnings were encountered during analysis:
// dist/render-oPv4-TXB.d.mts:26:3 - (ae-forgotten-export) The symbol "Renderer$1" needs to be exported
// dist/render-oPv4-TXB.d.mts:29:3 - (ae-forgotten-export) The symbol "ImagesInput" needs to be exported
```

## `takumi-js/node`

```ts
export * from "@takumi-rs/core";
```

## `takumi-js/wasm`

```ts
import auto_default from '@takumi-rs/wasm/auto';
import init from '@takumi-rs/wasm';

export default auto_default;

export { init }


export * from "@takumi-rs/wasm";
```

## `takumi-js/wasm/no-init`

```ts
import init from '@takumi-rs/wasm';

export { init }


export * from "@takumi-rs/wasm";
```

## `takumi-js/wasm-url`

```ts
const url: URL;
export default url;
```

## `takumi-js/helpers`

```ts
export * from "@takumi-rs/helpers";
```

## `takumi-js/helpers/emoji`

```ts
export * from "@takumi-rs/helpers/emoji";
```

## `takumi-js/helpers/jsx`

```ts
export * from "@takumi-rs/helpers/jsx";
```

## `takumi-js/helpers/html`

```ts
export * from "@takumi-rs/helpers/html";
```
