# Contributing to Takumi

First of all, thanks for contributing to Takumi.

This guide covers local setup, development flow, testing/build commands, fixtures, and changelogs.

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
5. Add a changelog file for user-facing package/crate changes.
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
(cd takumi-napi && bun test --silent)
(cd takumi-wasm && bun test --silent)
(cd takumi-js && bun test --silent)
(cd docs && bun test tests --silent)
```

Or run every package's suite at once:

```bash
bun run test
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
bun --filter ./takumi-napi run build:debug
bun --filter ./takumi-wasm run build:debug
bun --filter ./takumi-image-response run build
```

Notes:

- `takumi-napi` release build needs target-specific setup; for local validation, `build:debug` is usually enough.
- `takumi-wasm` build requires `wasm-pack`.

## Fixture Workflow (Rust Rendering)

Fixture sources are HTML files in `takumi/tests/fixtures-html`. The `html_fixtures` test parses each file and writes its goldens (`.webp`, `.svg`) to `takumi/tests/fixtures-generated`. The same file opens in a browser for a reference render.

When you change rendering/layout behavior:

1. Add or edit an HTML file in `takumi/tests/fixtures-html`. Put CSS in a `<style>` block. The `<body>` inline width/height set the viewport.
2. Run:

```bash
CARGO_PROFILE_TEST_STRIP=debuginfo cargo test -q
```

3. Review updated files in `takumi/tests/fixtures-generated`. Compare against the HTML opened in a browser.
4. Include intentional fixture updates in your PR.

CI will fail if generated files change unexpectedly.

Do not format files in `fixtures-html`: whitespace is part of the fixture. `.oxfmtrc.json` ignores the directory.

A few fixtures stay as Rust tests in `takumi/tests/fixtures/*.rs`. They animate, assert on rendered pixels, or use node features HTML cannot express (intrinsic image size hints, inline SVG sources).

## Changelogs

For any user-facing change in published packages/crates, add a changelog file:

```bash
bun run tegami
```

Select affected packages and choose `patch` / `minor` / `major`.

Changelog files are stored in `.tegami/*.md`. See the [changelog format docs](https://tegami.fuma-nama.dev/changelog) for the frontmatter and headings.

## README Sync for Rust Crate

`takumi/README.md` is checked in CI with `cargo rdme --check`.

If Rust doc comments or crate-facing examples changed, regenerate:

```bash
cd takumi
cargo rdme
```

Then commit the updated `takumi/README.md`.

## Release Notes

Release/version commands are handled by maintainers/CI via Tegami:

- `bun tegami ci`

You usually do not need to run these in feature PRs.

## PR Checklist

- Code is formatted (`cargo fmt --all`, oxlint passes)
- Relevant tests pass locally
- Scope is focused (one logical change per PR when possible)
- Fixture updates are intentional and reviewed
- Changelog file added (if user-facing)
- Generated files that CI checks are committed
- Docs updated where needed

## Code of Conduct

By participating, you agree to the [Code of Conduct](./CODE_OF_CONDUCT.md).
