const { join } = require("node:path");
const { pathToFileURL } = require("node:url");

module.exports = pathToFileURL(join(__dirname, "../pkg/takumi_pdf_wasm_bg.wasm"));
