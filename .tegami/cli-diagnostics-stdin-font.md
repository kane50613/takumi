---
packages:
  takumi-cli: patch
---

# `takumi` names standard input, `--font`, and a missing directory in what it prints

`--log info` names standard input for `-`, where it printed the URL of the working directory. A `--font` file that is missing or not a font fails with an error that starts with the flag. An output in a directory that does not exist names that directory.

```diff
  $ cat card.html | takumi image - -o card.png --log info
- [     0.0ms] input: read file:///home/me/site/, 198 bytes
+ [     0.0ms] input: read standard input, 198 bytes

  $ takumi image card.html -o card.png --font nope.ttf
- error: nope.ttf: No such file or directory (os error 2)
+ error: `--font nope.ttf`: No such file or directory (os error 2)

  $ takumi image card.html -o out/card.png
- error: out/card.png: No such file or directory (os error 2)
+ error: out/card.png: the directory out does not exist. Create it before writing there.
```

`takumi image --help` and `takumi pdf --help` also name the tab-separated columns `--list-resources` prints.
