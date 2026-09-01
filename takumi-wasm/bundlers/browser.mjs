import url from "./wasm-url.mjs";

export default fetch(url).then((response) => response.arrayBuffer());
