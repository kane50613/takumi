---
packages:
  takumi-cli:
    type: patch
---

# `takumi upgrade` finds a Windows install and keeps crates.io installs on crates.io

`takumi upgrade` now recognizes `takumi.exe` in Cargo's install record, so a Windows install no longer reads as one Cargo did not make. On Windows it prints the command to run in PowerShell once takumi exits, since Windows cannot replace a running program, and every command it prints on Windows is quoted for PowerShell. A crates.io install reinstalls with `--registry crates-io`, so a configured default registry cannot take its place. A Git URL's password, and its user name unless the URL uses SSH, no longer appear in the printed command.
