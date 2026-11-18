// Checks the declarations the packages publish in `dist`, so it needs them built first. A
// declaration build that cannot resolve a type writes `any`, which plain assignability accepts.
import type * as core from "@takumi-rs/core";
import type * as coreRequire from "@takumi-rs/core" with { "resolution-mode": "require" };
import type * as wasm from "@takumi-rs/wasm";
import type * as wasmRequire from "@takumi-rs/wasm" with { "resolution-mode": "require" };
import type * as js from "takumi-js";
import type * as jsRequire from "takumi-js" with { "resolution-mode": "require" };
import type * as response from "takumi-js/response";
import type * as responseRequire from "takumi-js/response" with { "resolution-mode": "require" };

type Expect<T extends true> = T;

type IsAny<T> = 0 extends 1 & T ? true : false;

/** Whether `T` is a type of its own, neither `any` nor the `never` a missing member gives. */
type Known<T> = [T] extends [never] ? false : IsAny<T> extends true ? false : true;

/** The type of `T[K]`, or `never` when `T` has no such member. */
type MemberOf<T, K extends PropertyKey> = K extends keyof T ? T[K] : never;

/** The return type of method `K` of `T`, or `never` when `T` has no such method. */
type ReturnOf<T, K extends PropertyKey> =
  IsAny<MemberOf<T, K>> extends true
    ? any
    : MemberOf<T, K> extends (...args: never[]) => infer Result
      ? Result
      : never;

/** The instance type of class `K` of `M`, or `never` when `M` has no such class. */
type InstanceOf<M, K extends PropertyKey> =
  IsAny<MemberOf<M, K>> extends true
    ? any
    : MemberOf<M, K> extends abstract new (...args: never[]) => infer Instance
      ? Instance
      : never;

/** Whether `Actual` is a promise of `Expected`, rather than `any` or a promise of `any`. */
type ResolvesTo<Actual, Expected> =
  Known<Actual> extends false
    ? false
    : [Actual] extends [Promise<infer Value>]
      ? Known<Value> extends false
        ? false
        : [Value] extends [Expected]
          ? true
          : false
      : false;

/** The members of `T` typed `any`, and the methods that return `any` or a promise of `any`. */
type AnyMethods<T> = {
  [K in keyof T]: IsAny<T[K]> extends true
    ? K
    : T[K] extends (...args: never[]) => infer Result
      ? IsAny<Result> extends true
        ? K
        : IsAny<Awaited<Result>> extends true
          ? K
          : never
      : never;
}[keyof T];

/**
 * What `render` of `R` returns for a prepared source and for a node, one per overload, since
 * inferring from an overloaded method sees only its last signature.
 */
type RenderOverloads<R> = R extends {
  render(source: infer _Prepared, options?: infer _PreparedOptions): infer FromPrepared;
  render(source: infer _Node, options?: infer _NodeOptions): infer FromNode;
}
  ? [FromPrepared, FromNode]
  : [never, never];

/** `true` when every check in `Checks` holds, else the names of the ones that fail. */
type Passes<Checks> = {
  [K in keyof Checks]: Checks[K] extends true ? never : K;
}[keyof Checks] extends infer Failing
  ? [Failing] extends [never]
    ? true
    : Failing
  : never;

type Measured = { width: number; height: number };

type RenderFunctions<M> = {
  render: ResolvesTo<ReturnOf<M, "render">, Uint8Array>;
  renderSvg: ResolvesTo<ReturnOf<M, "renderSvg">, string>;
  measure: ResolvesTo<ReturnOf<M, "measure">, Measured>;
  renderAnimation: ResolvesTo<ReturnOf<M, "renderAnimation">, Uint8Array>;
};

type ImageResponseClass<R> = {
  known: Known<R>;
  response: [R] extends [Response] ? true : false;
  ready: ResolvesTo<MemberOf<R, "ready">, void>;
};

type RendererClass<R> = RenderFunctions<R> & {
  known: Known<R>;
  noMethodReturnsAny: [AnyMethods<R>] extends [never] ? true : false;
  prepare: ResolvesTo<ReturnOf<R, "prepare">, object>;
  renderPrepared: ResolvesTo<RenderOverloads<R>[0], Uint8Array>;
  renderNode: ResolvesTo<RenderOverloads<R>[1], Uint8Array>;
};

type WasmRendererClass<R> = RendererClass<R> & {
  renderAsDataUrl: ResolvesTo<ReturnOf<R, "renderAsDataUrl">, string>;
};

export type Checks = [
  Expect<Passes<RenderFunctions<typeof js>>>,
  Expect<Passes<RenderFunctions<typeof jsRequire>>>,
  Expect<Passes<ImageResponseClass<InstanceOf<typeof response, "ImageResponse">>>>,
  Expect<Passes<ImageResponseClass<InstanceOf<typeof responseRequire, "ImageResponse">>>>,
  Expect<Passes<RendererClass<InstanceOf<typeof core, "Renderer">>>>,
  Expect<Passes<RendererClass<InstanceOf<typeof coreRequire, "Renderer">>>>,
  Expect<Passes<WasmRendererClass<InstanceOf<typeof wasm, "Renderer">>>>,
  Expect<Passes<WasmRendererClass<InstanceOf<typeof wasmRequire, "Renderer">>>>,
];
