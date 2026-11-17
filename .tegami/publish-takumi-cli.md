---
packages:
  takumi-cli: minor
---

# The `takumi` command line is on crates.io

`takumi` renders an HTML file to a PNG, JPEG, WebP, SVG, or PDF from the command line. A document reads only the directories you grant, and never the network.

```sh
cargo install --locked takumi-cli
takumi image card.html -o card.png
takumi pdf report.html -o report.pdf
```
