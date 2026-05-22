import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, readdirSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { gunzipSync } from "node:zlib";
import { Package, checkPackage } from "@arethetypeswrong/core";
import { publint } from "publint";
import { formatMessage } from "publint/utils";

// @arethetypeswrong/core's built-in tarball parser drops chunks for some
// gzip outputs (see https://github.com/101arrowz/fflate/issues/207); decompress
// and untar manually, then feed the file map to the Package constructor.
const TAR_HEADER_SIZE = 512;
const TAR_NAME_END = 100;
const TAR_SIZE_START = 124;
const TAR_SIZE_END = 136;

// Problem kinds that are not regressions in the published .d.ts itself.
// CJSResolvesToESM is a separate, pre-existing concern about the dual exports
// map; tracking it is fine but it should not block CI on this guard.
const IGNORED_PROBLEM_KINDS = new Set(["CJSResolvesToESM"]);

const pkgDir = dirname(dirname(fileURLToPath(import.meta.url)));
const pkgJson = JSON.parse(readFileSync(join(pkgDir, "package.json"), "utf8"));

let failed = false;

const { messages } = await publint({ pkgDir });
if (messages.length > 0) {
  console.log("publint:");
  for (const msg of messages) {
    const formatted = formatMessage(msg, pkgJson) ?? msg.code;
    console.log(`  [${msg.type}] ${formatted}`);
    if (msg.type === "error") failed = true;
  }
}

const tmp = mkdtempSync(join(tmpdir(), "verify-napi-pkg-"));
try {
  execFileSync("bun", ["pm", "pack", "--ignore-scripts", "--destination", tmp, "--quiet"], {
    cwd: pkgDir,
    stdio: ["ignore", "inherit", "inherit"],
  });
  const tarballName = readdirSync(tmp).find((f) => f.endsWith(".tgz"));
  if (!tarballName) throw new Error("bun pm pack did not produce a tarball");
  const tarBuf = gunzipSync(readFileSync(join(tmp, tarballName)));

  const files: Record<string, Uint8Array> = {};
  let prefix = "";
  let offset = 0;
  while (offset < tarBuf.length) {
    const header = tarBuf.subarray(offset, offset + TAR_HEADER_SIZE);
    const name = header.subarray(0, TAR_NAME_END).toString("utf8").replace(/\0.*$/, "");
    if (!name) break;
    const sizeOctal = header
      .subarray(TAR_SIZE_START, TAR_SIZE_END)
      .toString("utf8")
      .replace(/\0.*$/, "")
      .trim();
    const size = parseInt(sizeOctal || "0", 8);
    if (!prefix) prefix = name.slice(0, name.indexOf("/") + 1);
    const innerPath = name.slice(prefix.length);
    files[`/node_modules/${pkgJson.name}/${innerPath}`] = new Uint8Array(
      tarBuf.subarray(offset + TAR_HEADER_SIZE, offset + TAR_HEADER_SIZE + size),
    );
    offset += TAR_HEADER_SIZE + Math.ceil(size / TAR_HEADER_SIZE) * TAR_HEADER_SIZE;
  }

  const pkg = new Package(files, pkgJson.name, pkgJson.version);
  const result = await checkPackage(pkg);

  if ("problems" in result && result.problems.length > 0) {
    const fatal = result.problems.filter((p) => !IGNORED_PROBLEM_KINDS.has(p.kind));
    const ignored = result.problems.filter((p) => IGNORED_PROBLEM_KINDS.has(p.kind));
    if (ignored.length > 0) {
      console.log("attw (ignored):");
      for (const p of ignored) console.log(`  [${p.kind}]`);
    }
    if (fatal.length > 0) {
      console.log("attw:");
      for (const p of fatal) console.log(`  [${p.kind}] ${JSON.stringify(p)}`);
      failed = true;
    }
  }
} finally {
  rmSync(tmp, { recursive: true, force: true });
}

if (failed) {
  console.error("\nPackage verification failed.");
  process.exit(1);
}

console.log("Package verification passed.");
