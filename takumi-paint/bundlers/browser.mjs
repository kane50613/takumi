import * as wasm from "../dist/export.mjs";
import url from "./wasm-url.mjs";

wasm.initSync({ module: await fetch(url).then((response) => response.arrayBuffer()) });

export * from "../dist/export.mjs";
export default wasm.default;
