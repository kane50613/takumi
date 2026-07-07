# AGENTS.md

Agent-facing facts for working in this repo. Human-oriented setup, PR
checklist, and rationale live in `CONTRIBUTING.md`; read that too.

## Build/test commands

Rust:

```bash
CARGO_PROFILE_TEST_STRIP=debuginfo cargo test -q       # full suite (also test -p takumi for goldens)
cargo clippy --all-targets --all-features -- -D warnings
cargo machete                                            # unused deps
```

JS/TS, per package you touched:

```bash
(cd <package> && bun test --silent)
bun run lint         # oxlint + oxfmt --check
bun knip              # unused exports/deps
bun publish-lint      # attw + publint (needs a built dist)
```

`takumi-image-response` has no test script; covered by
`takumi-js/tests/response.test.tsx`.

## Fixture rules

`cargo test` REWRITES `takumi/tests/fixtures-generated/`. Diffs limited to
`.html` are regen noise; discard them unless you intended the change. Diffs
touching `.webp`/`.svg` are real render changes; review and commit them
intentionally. CI fails the build if generated files change unexpectedly on
a clean run.

## Golden platform rule

Goldens are Linux-canonical. Some fixtures render ±1-2px differently on
macOS. Never re-baseline goldens from a macOS run. Pull the updated
`.webp`/`.svg` files from the CI `changed-files` artifact instead.

## Toolchain gotchas

- Pre-commit hook (lefthook) needs `cargo-rdme` and `jq` on PATH; runs
  `cargo fmt`, `cargo rdme --force`, `bun run lint:fix`. Rust steps are
  glob-scoped to `*.rs`/`Cargo.toml`, so JS-only commits skip them.
- In a linked git worktree, `LEFTHOOK=0 git commit ...` skips the hook if it
  misbehaves. History: lefthook exported `GIT_DIR` without `GIT_WORK_TREE`
  into the `cargo rdme` job, so `cd`-ing into a crate dir made git treat
  that crate as repo root and `git add README.md` clobbered the root
  README's index entry. The job now unsets both before running.
- Docs playground (`docs/`) consumes `takumi-helpers/dist` (built), not
  source. Rebuild helpers and clear `docs/.vite` for helper changes to show.
- `takumi-napi` release build needs target-specific setup; no debug build
  script exists. Use `cargo check -p takumi-napi` for fast local iteration.
- `takumi-wasm` builds need `wasm-pack`; CI additionally pins a nightly
  toolchain for `build-std`.

## Changelogs

```bash
bun run tegami
```

Writes `.tegami/*.md` (frontmatter `packages: "npm:<pkg>"|"cargo:<crate>":
<bump>` + `### Title` + body). All non-`takumi` crates are pre-1.0: a
breaking change bumps `minor`, not `major`. Only `takumi` itself uses
`major` for breaking changes.
