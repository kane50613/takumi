# CLI design

This file sets the conventions every `takumi` command follows, so a new command reads like the old ones. They draw on the [Command Line Interface Guidelines](https://clig.dev/). When a change breaks one, change this file in the same PR.

## Commands

Each product is one subcommand named by the noun it writes, and rendering is implied:

```sh
takumi image <input> -o card.png
takumi pdf <input> -o invoice.pdf
```

A product gets its own subcommand when its options differ from every other product's, so no flag is accepted and then ignored. `takumi animation` will follow the same shape. An action other than rendering becomes a top-level verb, never a third word under a product: `takumi upgrade` reinstalls takumi, and measuring would be `takumi measure`. There is one word per action, so no `update` beside `upgrade`. An action that reaches the network or changes the install runs only when asked, and takes `-n`/`--dry-run` to print what it would do. Subcommands are never matched by prefix, so adding one cannot change what an existing abbreviation means.

## Arguments and flags

- The input is the one positional argument. Everything else is a flag, and every flag has a long form. Only `-o`, `-q`, `-h`, and `-V` have short ones.
- `-` means standard input for the input and standard output for `-o`. Writing to standard output needs `--format`, since there is no extension to read. A binary format is never written to a terminal.
- Flags shared by products come from one clap group each, flattened into every product that takes them: the input and its permissions (`--font`, `--root`, `--allow-read`), the output (`-o`, `--list-resources`), and the viewport (`--width`, `--height`, `--dpr`) for the raster products.
- A value that CSS can express is written in CSS and parsed by takumi's own parser: `--page "A4 landscape"`, `--margin "20mm 15mm"`, `--background "#fafafa"`. A flag that sets what the document can also set is a default, and the document wins, as the document's `@page` rule wins over Chrome's print dialog.
- A flag name says what it sets, in the words CSS or the output format uses: `--page` for the page size, `--pages` for the pages kept.

## Output and errors

- The rendered file goes to `-o`. Nothing else is written to standard output, except the list `--list-resources` prints, one resource per line with tab-separated fields. A flag that only reads, such as `--list-resources`, needs no `-o`.
- Warnings and errors go to standard error, as `warning: …` and `error: …`. `-q` hides warnings. A warning names something the document asked for that the render went on without, prints once, and leaves the exit code alone unless `--deny-warnings` is set, which fails the command before it writes. An error says what failed and the flag or change that fixes it.
- `--log info` and `--log debug` print what a render does to standard error, as `[<time>] <stage>: …`. A stage is the last part of a `log` target such as `takumi::font`, and `info` holds one line per stage. A new step logs under an existing stage where one fits. Each `debug` line prints once.
- Exit codes: `0` when the file is written, `1` when rendering fails, `2` for a usage error, including an output format takumi does not write, and `3` when the sandbox denies a read, so a script can tell a policy failure from a broken document.

## Resources and trust

- Every file a document reaches goes through one `Sandbox`. See the README for the rules.
- A file named on the command line is trusted: the input, `--font`, `--header`, `--footer`, and later `--attach`. What those files reference is not, and goes through the sandbox like the document's own references.
- A new way to load resources, such as the network, comes as an explicit permission flag that is off by default, in the shape of `--allow-read`.

## Changes

New behavior arrives as a new flag or subcommand. Changing what an existing flag does needs a deprecation warning for at least one release first, and that warning names the replacement.

## Planned

- `takumi pdf`: `--standard` for PDF/A, `--attach`, and `--no-tagged`.
- `takumi animation`: animated WebP, PNG, and GIF, with `--duration` and `--fps`.
- `--json` on `--list-resources`, once a consumer needs more than the tab-separated fields.
- `--allow-net`, with the address checks the README's network section will describe.
