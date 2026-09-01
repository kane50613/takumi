import url from "../pkg/takumi_pdf_wasm_bg.wasm?url";
import * as wasm from "../dist/export.mjs";

wasm.initSync({ module: await fetch(url).then((response) => response.arrayBuffer()) });

export * from "../dist/export.mjs";
export default wasm.default;
