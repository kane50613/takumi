import module from "../pkg/takumi_pdf_wasm_bg.wasm";
import * as wasm from "../dist/export.mjs";

wasm.initSync({ module });

export * from "../dist/export.mjs";
export default wasm.default;
