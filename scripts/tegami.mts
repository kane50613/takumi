import { $ } from "bun";
import {
  tegami,
  type BumpType,
  type PackageOptions,
  type TegamiPlugin,
  type WorkspacePackage,
} from "tegami";
import { createCli } from "tegami/cli";
import { cargo } from "tegami/plugins/cargo";
import { github } from "tegami/plugins/github";

// tegami serializes changelogs/manifests in a style oxfmt rejects; reformat
// before the github plugin stages and commits the version branch.
const oxfmt: TegamiPlugin = {
  name: "oxfmt",
  enforce: "pre",
  async applyCliDraft() {
    await $`oxfmt --write .`.quiet();
  },
};

// tegami updates the lockfile with `bun install --frozen-lockfile`, which never
// rewrites the workspace version fields bun.lock caches. bun then resolves
// `workspace:*` deps from those stale versions at publish, pinning internal
// deps to an old release. Re-run a writable install so the version PR's
// bun.lock records the bumped versions. Runs after the frozen install (npm
// plugin, `enforce: "pre"`) and before the github plugin commits the branch.
const refreshLockfile: TegamiPlugin = {
  name: "refresh-lockfile",
  async applyCliDraft() {
    await $`bun install`.quiet();
  },
};

// v2 beta line; clear this for the stable 2.0.0 release.
const prerelease = "rc";

if (prerelease) process.env.npm_config_tag = prerelease;

const groupedNpmPackages = [
  "takumi-js",
  "@takumi-rs/core",
  "@takumi-rs/helpers",
  "@takumi-rs/wasm",
  "@takumi-rs/image-response",
];

const independentCrates = ["takumi-core", "takumi-raster", "takumi-svg", "takumi-html"];

const packages: Record<string, PackageOptions<"takumi">> = {
  "cargo:takumi": { group: "takumi" },
};

for (const name of groupedNpmPackages) {
  packages[`npm:${name}`] = { group: "takumi" };
}

for (const name of independentCrates) {
  packages[`cargo:${name}`] = { prerelease };
}

// Skip versionless dependents (private examples, docs, templates); only real
// `dependencies` bumps propagate.
const bumpDep = ({
  dependent,
  kind,
}: {
  dependent: WorkspacePackage;
  kind: string;
}): BumpType | false => (dependent.version && kind === "dependencies" ? "patch" : false);

const paper = tegami({
  plugins: [
    oxfmt,
    refreshLockfile,
    github({ repo: "kane50613/takumi", versionPr: { base: "master" } }),
    cargo({ updateLockFile: true, bumpDep }),
  ],
  groups: {
    takumi: { syncBump: true, syncGitTag: true, prerelease },
  },
  packages,
  npm: { client: "bun", updateLockFile: true, bumpDep },
});

void createCli(paper).parseAsync();
