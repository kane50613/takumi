# Agent instructions

Follow [CONTRIBUTING.md](CONTRIBUTING.md) for setup, code style, validation, and pull requests.

## Public API

The files under `api/` list the public API of every published package and crate. CI regenerates them and fails when they differ from the committed copies. To update them, run `bun run api` after building the packages, or download the `api-snapshots` artifact of the failed run into `api/`.

New public API is a decision, not a side effect of a change:

- A pull request that grows a snapshot says in its body what the new API replaces, or why an existing entry point, option, or default cannot do the job.
- Prefer a default that makes a new option unnecessary.
- When an addition supersedes an existing export, mark at least one superseded export `@deprecated … Will be removed in v3`.
- Experimental API goes behind an `unstable_` prefix or an `/experimental` subpath.
