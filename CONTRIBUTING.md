# Contributing to Takumi

First of all, thanks for contributing to Takumi.

This guide covers local setup, development flow, testing/build commands, fixtures, and changesets.

## Ways to Contribute

- Report bugs with the [Bug Report template](https://github.com/kane50613/takumi/issues/new/choose).
- Propose enhancements with the [Feature Request template](https://github.com/kane50613/takumi/issues/new/choose).
- Ask usage questions via the [Question template](https://github.com/kane50613/takumi/issues/new/choose).
- Improve docs, examples, tests, and fixture coverage.

## Prerequisites

- Rust `1.91+`
- Bun (latest)

## Local Setup

```bash
bun install
```

This installs all workspace dependencies and sets up `lefthook`.

## Development Flow

1. Create a feature branch.
2. Make your changes.
3. Run formatting and tests for affected packages.
4. Update generated fixtures if rendering output changed.
5. Add a changeset for user-facing package/crate changes.
6. Open a PR.

## Formatting and Lint

```bash
cargo fmt --all
bun run lint
```

Use auto-fix when needed:

```bash
bun run lint:fix
```

## Test Commands

Run all Rust tests:

```bash
CARGO_PROFILE_TEST_STRIP=debuginfo cargo test -q
```

Run workspace package tests (pick what you changed):

```bash
(cd takumi-helpers && bun test --silent)
(cd takumi-napi-core && bun test --silent)
(cd takumi-wasm && bun test --silent)
(cd takumi-image-response && bun test --silent)
(cd takumi-template && bun test --silent)
```

To match CI quality gates for Rust changes, also run:

```bash
cargo clippy --all-targets --all-features -- -D warnings
cargo machete
```

## Build Commands

Run build only for packages you touched:

```bash
bun --filter ./takumi-helpers run build
bun --filter ./takumi-napi-core run build:debug
bun --filter ./takumi-wasm run build:debug
bun --filter ./takumi-image-response run build
```

Notes:

- `takumi-napi-core` release build needs target-specific setup; for local validation, `build:debug` is usually enough.
- `takumi-wasm` build requires `wasm-pack`.

## Fixture Workflow (Rust Rendering)

Takumi fixture tests write snapshots under `takumi/tests/fixtures-generated`.

When you change rendering/layout behavior:

1. Update or add fixture tests in `takumi/tests/fixtures/*.rs`.
2. Register new fixture modules in `takumi/tests/fixtures.rs` if you added a new file.
3. Run:

```bash
CARGO_PROFILE_TEST_STRIP=debuginfo cargo test -q
```

4. Review updated files in `takumi/tests/fixtures-generated`.
5. Include intentional fixture updates in your PR.

CI will fail if generated files change unexpectedly.

## Changesets

For any user-facing change in published packages/crates, add a changeset:

```bash
bunx changeset
```

Select affected packages and choose `patch` / `minor` / `major`.

Changesets are stored in `.changeset/*.md`.

## README Sync for Rust Crate

`takumi/README.md` is checked in CI with `cargo rdme --check`.

If Rust doc comments or crate-facing examples changed, regenerate:

```bash
cd takumi
cargo rdme
```

Then commit the updated `takumi/README.md`.

## Release Notes

Release/version commands are handled by maintainers/CI via Changesets:

- `bun run version`
- `bun run release`

You usually do not need to run these in feature PRs.

## PR Checklist

- Code is formatted (`cargo fmt --all`, Biome lint passes)
- Relevant tests pass locally
- Scope is focused (one logical change per PR when possible)
- Fixture updates are intentional and reviewed
- Changeset added (if user-facing)
- Generated files that CI checks are committed
- Docs updated where needed

## Code of Conduct

By participating, you agree to the [Code of Conduct](./CODE_OF_CONDUCT.md).
