import url from "../pkg/takumi_wasm_bg.wasm?url";

export default fetch(url).then((response) => response.arrayBuffer());
