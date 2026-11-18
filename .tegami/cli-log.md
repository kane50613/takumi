---
packages:
  takumi-cli: minor
---

# `--log` prints what a render did

`--log info` prints a line to standard error for each stage of a render, with the time since the command began.

```sh
$ takumi image card.html -o card.png --log info
[     0.2ms] input: read file:///site/card.html, 198 bytes
[     1.4ms] stylesheet: style sheets loaded: 0
[     3.6ms] resource: images loaded: 0
[     8.2ms] render: rendered 1464 bytes
[     8.2ms] output: writing card.png
```

`--log debug` adds a line for:

- each resource loaded
- each page, and why it ended
- the CSS the parser dropped
- each font family or image the render found missing
