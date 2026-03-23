import module from "../pkg/takumi_wasm_bg.wasm?module";

// typeof module says its Module { } but instanceof WebAssembly.Module is false
// Have to force override the prototype to be WebAssembly.Module
Object.setPrototypeOf(module, WebAssembly.Module.prototype);

export default module;
