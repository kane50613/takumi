---
packages:
  takumi-cli:
    type: patch
---

# `takumi upgrade` finds a Windows install and keeps a crates.io install on crates.io

On Windows, `takumi upgrade` now recognizes a takumi that `cargo install` put there, and gives you the command to finish the upgrade yourself. A takumi installed from crates.io reinstalls from crates.io, even when Cargo has another default registry.

```sh
$ takumi upgrade --dry-run
# Before
cargo install --locked takumi-cli --root /opt/takumi
# After
cargo install --locked --registry crates-io takumi-cli --root /opt/takumi
```

- On Windows, `takumi upgrade` no longer says takumi was not installed with `cargo install`.
- On Windows, it does not upgrade takumi itself, since Windows cannot replace a running program. It fails with "Windows cannot replace takumi while it runs. Run `cargo install …` in PowerShell once this command exits." Every command it prints there is quoted for PowerShell.
- A printed command leaves out the password in a Git URL, and the user name too unless the URL uses SSH.
