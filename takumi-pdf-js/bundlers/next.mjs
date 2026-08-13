import module from "../pkg/takumi_pdf_wasm_bg.wasm?module";
import * as wasm from "../dist/export.mjs";

// typeof module says its Module { } but instanceof WebAssembly.Module is false
// Have to force override the prototype to be WebAssembly.Module
Object.setPrototypeOf(module, WebAssembly.Module.prototype);

wasm.initSync({ module });

export * from "../dist/export.mjs";
export default wasm.default;
