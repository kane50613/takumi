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

const refreshLockfile: TegamiPlugin = {
  name: "refresh-lockfile",
  async applyCliDraft() {
    await $`bun install`.quiet();
    await $`cargo update --workspace`.quiet();
  },
};

const groupedPackages = [
  "npm:takumi-js",
  "npm:@takumi-rs/core",
  "npm:@takumi-rs/helpers",
  "npm:@takumi-rs/wasm",
  "npm:@takumi-rs/image-response",
  "cargo:takumi",
];

// The crate is never published; it carries a version so the `/Producer` it
// writes matches the npm package a reader installed.
const pdfPackages = ["npm:takumi-pdf", "cargo:takumi-pdf"];

const packages: Record<string, PackageOptions<"takumi" | "takumi-pdf">> = {};

for (const name of groupedPackages) {
  packages[name] = { group: "takumi" };
}

for (const name of pdfPackages) {
  packages[name] = { group: "takumi-pdf" };
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
    takumi: { syncBump: true, syncGitTag: true },
    "takumi-pdf": { syncBump: true },
  },
  packages,
  npm: { client: "bun", updateLockFile: true, bumpDep },
});

void createCli(paper).parseAsync();
