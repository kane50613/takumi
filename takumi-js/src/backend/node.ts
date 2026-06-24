import * as core from "@takumi-rs/core";
import type { LoadBackend } from "./types";

// Selected by the `node`/`bun` import condition: the native napi addon. It is
// never reachable from a worker/edge bundle, so bundlers can't drag the `.node`
// binary into runtimes that can't load it.
export const loadBackend: LoadBackend = async () => core;
