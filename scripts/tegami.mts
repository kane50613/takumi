import { $ } from "bun";
import { tegami, type PackageOptions, type TegamiPlugin } from "tegami";
import { createCli } from "tegami/cli";
import { github } from "tegami/plugins/github";

const refreshCargoLock: TegamiPlugin = {
  name: "refresh-cargo-lock",
  enforce: "pre",
  cli: {
    async publishPlanApplied() {
      await $`cargo update --workspace`;
    },
  },
};

// v2 beta line; clear this for the stable 2.0.0 release.
const prerelease = "beta";

const groupedNpmPackages = [
  "takumi-js",
  "@takumi-rs/core",
  "@takumi-rs/helpers",
  "@takumi-rs/wasm",
  "@takumi-rs/image-response",
];

const independentCrates = ["takumi-core", "takumi-css", "takumi-raster", "takumi-svg"];

const packages: Record<string, PackageOptions<"core">> = {
  "cargo:takumi": { group: "core" },
};

for (const name of groupedNpmPackages) {
  packages[`npm:${name}`] = {
    group: "core",
    npm: prerelease ? { distTag: prerelease } : undefined,
  };
}

for (const name of independentCrates) {
  packages[`cargo:${name}`] = { prerelease };
}

const paper = tegami({
  plugins: [
    refreshCargoLock,
    github({ repo: "kane50613/takumi", cli: { versionPr: { base: "master" } } }),
  ],
  groups: {
    core: { syncBump: true, syncGitTag: true, prerelease },
  },
  packages,
  ignore: [
    /^(npm:)?(docs|takumi-template|example-.*|ffmpeg-keyframe-animation|ffplay|svelte|waku-ssr)$/,
  ],
  npm: {
    client: "bun",
    updateLockFile: true,
    bumpDep: ({ kind }) => (kind === "dependencies" ? "patch" : false),
  },
  cargo: {
    bumpDep: ({ kind }) => (kind === "dependencies" ? "patch" : false),
  },
});

void createCli(paper).parseAsync();
