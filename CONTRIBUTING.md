# Contributing to Takumi

## Contents

- [Ways to contribute](#ways-to-contribute)
- [Local setup](#local-setup)
- [Code style](#code-style)
- [Local validation](#local-validation)
- [Goldens and fixtures](#goldens-and-fixtures)
- [Performance changes](#performance-changes)
- [Documentation and changelogs](#documentation-and-changelogs)
- [Pull requests](#pull-requests)
- [Release commands](#release-commands)
- [Code of Conduct](#code-of-conduct)

## Ways to contribute

- Report bugs with the [Bug Report template](https://github.com/kane50613/takumi/issues/new/choose).
- Propose enhancements with the [Feature Request template](https://github.com/kane50613/takumi/issues/new/choose).
- Ask usage questions via the [Question template](https://github.com/kane50613/takumi/issues/new/choose).
- Improve docs, examples, tests, and fixture coverage.

## Local setup

Install these prerequisites:

- Rust 1.91 or newer
- Bun 1.4
- The `wasm32-unknown-unknown` target, for the WebAssembly binding: `rustup target add wasm32-unknown-unknown`
- [`wasm-pack`](https://rustwasm.github.io/wasm-pack/installer/), for building that binding. CI installs the latest release, so no version is pinned.

Install the workspace dependencies:

```bash
bun install
```

The install also sets up `lefthook` through the root `postinstall` script.

Create a feature branch before making changes. Keep the branch focused on one logical change.

## Code style

### Rust comments and documentation

Code has no comment by default. Add a comment only for one of these reasons:

- Link to an external reference.
- Mark incomplete work with a TODO.
- State an invariant that the code cannot show.

Put the reason for a change in the commit message. Do not put it in a code comment.

A doc comment is one line that says what the item is. Delete a doc comment that only repeats the name or signature. Use more than one line only to cite an external specification or another implementation.

### Rust paths and expressions

- Import a path with `use` at the top of the file. Do not use a fully qualified path in an expression, even for one call.
- Give `format!` and related macros inline named arguments. Do not use positional `{}` placeholders followed by a trailing argument list.
- Use `if` and `else` for a boolean condition. Do not `match` on a boolean.
- Replace a repeatable value with a named constant. Do not hide it in a helper function or repeat the expression.

### Rust functions and modules

- Make a free function a method when its first parameter is a type the project owns.
- Make a builder or parser an associated function on the type it creates.
- Keep stateless recursive descent parser functions free.
- Name a module file after a domain noun. Its contents should be clear from its name, such as `page.rs`.
- Extract an inline helper closure into a named top-level function.
- Reuse an existing helper. Do not add a second copy.

### Cargo dependencies

Put dependency versions in `[workspace.dependencies]` in the root `Cargo.toml`.

A member inherits the dependency and adds features when needed:

```toml
[dependencies.serde]
workspace = true
features = ["rc"]
```

Do not repeat the version in a member. Use a `[dependencies.<name>]` block when an entry needs more than a bare version string. Do not use an inline table.

### Rust public APIs

- Lower an internal item to `pub(crate)` instead of hiding it with `#[doc(hidden)]`.
- Put a cross-crate internal API in its own named module.
- Add `#[non_exhaustive]` to public value types.

### WebAssembly binding APIs

A TypeScript-facing WebAssembly signature does not expose `JsValue` or `js_sys::Object`.

Follow the pattern in `takumi-wasm`:

- Put the TypeScript declarations in `takumi-wasm/src/dts-header.d.ts`.
- Include that file from a `#[wasm_bindgen(typescript_custom_section)]` item.
- Declare extern types with `#[wasm_bindgen(typescript_type = "...")]`.
- Add `unchecked_return_type` to bindings that return bytes.

Keep the `RwLock` used with `&self` in `takumi-wasm` and `takumi-pdf-wasm`. Keep the `Mutex` in `takumi-napi`. A shared-memory build would make `&mut self` unsound.

### TypeScript type safety

Fix the cause of every type error. Do not silence the checker with any of these changes:

- A non-null assertion
- An `as` assertion used to dodge an error
- `as any`
- `@ts-ignore`
- `@ts-expect-error`
- A looser `tsconfig`

## Local validation

### Formatting and linting

Format Rust and check JavaScript and TypeScript:

```bash
cargo fmt --all
bun run lint
```

Apply JavaScript and TypeScript fixes when needed:

```bash
bun run lint:fix
```

### Focused test commands

Run commands for the affected crate or package. A whole-workspace build is slow and is rarely useful twice.

| Change                                | Command                                               |
| ------------------------------------- | ----------------------------------------------------- |
| Umbrella Rust crate or render goldens | `cargo test -p takumi`                                |
| Rust engine crate                     | `cargo test -p takumi-core`                           |
| Helpers package                       | `(cd takumi-helpers && bun test --silent)`            |
| Native binding package                | `(cd takumi-napi && bun test --silent)`               |
| WebAssembly binding package           | `(cd takumi-wasm && bun test --silent)`               |
| JavaScript package                    | `(cd takumi-js && bun test --silent)`                 |
| Docs                                  | `(cd docs && bun test app/playground tests --silent)` |

Run all package tests only when a change crosses package boundaries:

```bash
bun run test
```

Run all Rust tests only when a change crosses crate boundaries:

```bash
CARGO_PROFILE_TEST_STRIP=debuginfo cargo test -q
```

Do not accept a loose output pattern as proof that a test failed. The pattern `failed. [1-9]` misses output such as `test result: FAILED. 0 passed; 1 failed`. Match `FAILED|panicked`, or read the end of the output.

### Rust CI gates

CI runs these Rust gates:

```bash
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo clippy --locked --target wasm32-unknown-unknown -p takumi-wasm -p takumi-pdf-wasm -- -D warnings
cargo test --locked
cargo machete
```

A fresh cache can make Clippy skip an unchanged crate. Touch that crate's existing `src/lib.rs` before the Clippy command. The full flags cover feature-gated code and every target.

### Binding checks

Check changed bindings locally:

```bash
cargo check -p takumi-napi
cargo check -p takumi-wasm --target wasm32-unknown-unknown
```

### Focused build commands

Build only the packages affected by the change:

| Package             | Command                                          |
| ------------------- | ------------------------------------------------ |
| Helpers             | `bun run --filter ./takumi-helpers build`        |
| WebAssembly binding | `bun run --filter ./takumi-wasm build:debug`     |
| Image response      | `bun run --filter ./takumi-image-response build` |

The WebAssembly binding build requires `wasm-pack`.

### Docs playground builds

The docs playground reads build output instead of package source.

| Source change                                         | Build command                                |
| ----------------------------------------------------- | -------------------------------------------- |
| `takumi-helpers/src`                                  | `bun run --filter ./takumi-helpers build`    |
| Rust rendering engine used by the WebAssembly binding | `bun run --filter ./takumi-wasm build:debug` |

## Goldens and fixtures

### Golden change policy

A change to rendered or serialized output includes its regenerated golden in the same pull request. Inspect every generated golden before committing it.

A pure refactor keeps every golden byte for byte. Put a behavior change in its own pull request. Do not fold one into a refactor.

Prove a behavior change with a differential run:

- Simulate the old behavior.
- Confirm that the old-behavior build compiled and ran.
- Confirm that it regenerated the output.
- Compare the old and new output.

A result with no changed output counts only after all four checks.

### Render fixture sources

Render fixture sources are HTML files in `takumi/tests/fixtures-html`. The `html_fixtures` test writes `.webp` and `.svg` goldens to `takumi/tests/fixtures-generated`.

The harness reads only these parts of each HTML file:

- The inline `<style>` block
- The contents of `<body>`
- The inline `width` and `height` on `<body>`

The linked `takumi/tests/shared.css` supports browser comparison. It loads the fixture fonts and matches renderer defaults such as `box-sizing: border-box` and a body margin of zero.

Add or change a render fixture like this:

- Add or edit an HTML file in `takumi/tests/fixtures-html`.
- Put fixture CSS in the inline `<style>` block.
- Set the viewport with inline `width` and `height` values on `<body>`.
- Run `cargo test -p takumi`.
- Review the changed files in `takumi/tests/fixtures-generated`.
- Open the HTML in a browser and compare the reference render.
- Include every intentional golden change in the pull request.

The render goldens belong to the umbrella `takumi` crate. Tests for lower crates do not regenerate them.

Do not make unrelated formatting changes in `takumi/tests/fixtures-html`. Whitespace is fixture input. `.oxfmtrc.json` applies strict HTML whitespace handling to this directory.

Fixtures that animate or assert on pixels or measured layout stay as Rust tests in `takumi/tests/fixtures`.

CI fails when test execution changes a generated file that is not included in the pull request.

### Platform-specific goldens

- WebP goldens are canonical on Linux.
- Some WebP fixtures differ on macOS because the platform math library produces different floating-point results. Let CI regenerate those fixtures.
- PDF fixtures in `takumi-pdf/tests/fixtures-generated` are byte identical on Linux and macOS.

## Performance changes

The full JavaScript benchmark is the gate for a performance change:

```bash
bun run --filter ./takumi-napi bench
```

Do not use one native micro-benchmark as the gate. Measure a WebAssembly change in WebAssembly. Native results do not carry over.

Measure binary size through the shipped pipeline:

- Build the release WebAssembly package. Its `wasm-pack` profile runs `wasm-opt`.
- Compress the resulting `.wasm` file with `gzip -9`.
- Report the size difference. Do not report only the absolute size.

For `takumi-wasm`, use:

```bash
bun run --filter ./takumi-wasm build
gzip -9 -c takumi-wasm/pkg/takumi_wasm_bg.wasm | wc -c
```

## Documentation and changelogs

### External behavior and public APIs

A change to external behavior or a public API includes both of these updates:

- The relevant file in `docs/content/docs/*.mdx`
- A changelog file in `.tegami/*.md`

Create the changelog file with:

```bash
bun run tegami
```

Select the affected packages and the `patch`, `minor`, or `major` release type.

Write the changelog entry as one imperative sentence. Do not add a second sentence or a rationale. Put reasoning in the commit body or pull request.

See the [changelog format docs](https://tegami.fuma-nama.dev/changelog) for the frontmatter and headings.

### Rust crate READMEs

CI checks published Rust crate READMEs with `cargo rdme --check`.

Regenerate a changed crate README from that crate's directory. For the umbrella crate:

```bash
cd takumi
cargo rdme
```

Include the regenerated `takumi/README.md` when Rust doc comments or crate-facing examples change.

## Pull requests

### Pull request scope

- Name the user-visible outcome in the title. Prefer `Support display: table` to `Lay out tables on the grid algorithm`.
- Keep one logical change in each pull request.
- Split multi-part work into a stack. Give reviewers one layer per pull request.
- Keep behavior changes separate from refactors.

### Push safety

Check the current branch before pushing:

```bash
git branch --show-current
```

Push with an explicit remote and branch:

```bash
git push origin "$(git branch --show-current)"
```

Do not push another commit to a pull request after auto-merge is armed. The squash can land first and leave the new commit behind on the branch.

### Pull request checklist

- Rust code is formatted with `cargo fmt --all`.
- `bun run lint` passes.
- Tests for affected crates and packages pass.
- Clippy covers all targets and all features.
- Fixture changes are intentional and reviewed.
- Pure refactors leave goldens unchanged.
- User-visible changes include docs and a changelog entry.
- Generated files checked by CI are included.
- The pull request contains one logical change or one layer of a stack.

## Release commands

Maintainers and CI handle release and version commands through Tegami:

```bash
bun tegami ci
```

Feature pull requests do not need this command.

## Code of Conduct

By participating, you agree to the [Code of Conduct](./CODE_OF_CONDUCT.md).
