#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import { createRequire } from "node:module";
import { constants } from "node:os";

const require = createRequire(import.meta.url);
const platformPackage = `@takumi-rs/cli-${process.platform}-${process.arch}`;
const executable = process.platform === "win32" ? "takumi.exe" : "takumi";

let binary;

try {
  binary = require.resolve(`${platformPackage}/${executable}`);
} catch {
  console.error(
    `error: ${platformPackage} is not installed, so takumi has no binary to run. Reinstall without skipping optional dependencies, or build takumi with \`cargo install --locked takumi-cli\` on a platform it does not ship for.`,
  );
  process.exit(1);
}

const result = spawnSync(binary, process.argv.slice(2), { stdio: "inherit" });

if (result.error) {
  throw result.error;
}

process.exitCode = result.signal ? 128 + constants.signals[result.signal] : result.status;
