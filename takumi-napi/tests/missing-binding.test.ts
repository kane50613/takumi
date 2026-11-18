import { expect, test } from "bun:test";
import { join } from "node:path";

// The shipped loader in `dist`, whose `TAKUMI_CORE_TARGET` names the only binary it tries.
test.each(["export.mjs", "export.cjs"])(
  "%s names the platform package it could not load",
  (output) => {
    const { stderr, exitCode } = Bun.spawnSync({
      cmd: [
        process.execPath,
        "-e",
        `require(${JSON.stringify(join(import.meta.dir, "../dist", output))})`,
      ],
      env: { ...process.env, TAKUMI_CORE_TARGET: join(import.meta.dir, "missing.node") },
    });
    const message = stderr.toString();

    expect(exitCode).not.toBe(0);
    expect(message).toContain(`Cannot find @takumi-rs/core-${process.platform}-${process.arch}`);
    expect(message).toContain('serverExternalPackages: ["@takumi-rs/core"]');
    expect(message).toContain(
      "https://takumi.kane.tw/docs/troubleshooting#cannot-find-native-binding",
    );
    expect(message).not.toContain("npm has a bug");
  },
);
