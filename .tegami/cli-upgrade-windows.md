---
packages:
  takumi-cli:
    type: patch
---

# `takumi upgrade` finds a Windows install and keeps a crates.io install on crates.io

`takumi upgrade` now works for a takumi installed on Windows, and a takumi installed from crates.io reinstalls from crates.io even when Cargo has another default registry.

```sh
$ takumi upgrade --dry-run
# Before
cargo install --locked takumi-cli --root /opt/takumi
# After
cargo install --locked --registry crates-io takumi-cli --root /opt/takumi
```

- On Windows, `takumi upgrade` no longer says takumi was not installed with `cargo install`.
- On Windows, it prints the command to run in PowerShell once takumi exits, since Windows cannot replace a running program. Every command it prints there is quoted for PowerShell.
- A printed command leaves out the password in a Git URL, and the user name too unless the URL uses SSH.
