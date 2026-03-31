export type * from "@takumi-rs/helpers";
export type * from "@takumi-rs/core";
export * from "./render";

export type { RenderOptions } from "./render";

declare module "react" {
  interface DOMAttributes<T> {
    tw?: string;
  }
}
