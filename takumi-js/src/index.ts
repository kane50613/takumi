export type * from "@takumi-rs/helpers";
export * from "./render";
export type { OutputFormat } from "@takumi-rs/core";

declare module "react" {
  interface DOMAttributes<T> {
    tw?: string;
  }
}
