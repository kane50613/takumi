# takumi-cli

`takumi` renders an HTML file to an image or a PDF.

```sh
cargo install --locked takumi-cli
takumi upgrade   # later, to install the newest release
takumi completions zsh > ~/.zfunc/_takumi   # zsh Tab completion. See the install guide for bash, fish, and PowerShell

takumi image card.html -o card.png --width 1200 --height 630
takumi pdf invoice.html -o invoice.pdf --page "letter landscape" --footer footer.html
takumi paint card.html -o -   # what the render draws, as text
```

The [command line guide](https://takumi.kane.tw/docs/cli) covers installation, every product, and scripting. Run `takumi image --help` or `takumi pdf --help` for each flag, and see [DESIGN.md](DESIGN.md) for the conventions the commands follow.

## What a document can read

A document reads files only from the directories it is granted, and it never reaches the network. This keeps an untrusted HTML file from pulling files outside those directories into its output. Since the input file's directory is granted by default, keep an untrusted HTML file in a directory of its own, not beside files you want kept private.

- By default, the document may read files under the input file's directory. Reading from standard input grants no directory.
- `--root <DIR>` replaces that directory, and `--allow-read <DIR>` adds another. Repeat `--allow-read` for more.
- Each granted directory is opened once, and every read resolves inside it, so neither `..`, a symlink, nor a file swapped for a symlink mid-read can step outside, and nothing outside a granted directory is opened or probed.
- `data:` URLs load wherever the document is, within the same size limits. `http` and `https` URLs never do, and neither does a `file:` URL that names another machine, such as a Windows network share. takumi skips them with a warning, as a browser skips a resource that fails to load.
- A read outside the granted directories stops the render with exit code 3, and the error names the `--allow-read` flag that would allow it.
- One resource may hold at most 32 MiB, all of them together 256 MiB, and a render may load at most 1024 of them.
- A symlink inside a granted directory must be relative and stay inside it. An absolute one is refused even when it points back inside. A granted directory may itself be reached through a symlink, which takumi resolves when it opens the directory.

`--list-resources` prints the stylesheets, images, and fonts a document would load and whether each is allowed, without rendering. It needs no `-o`, and a stylesheet that would not load is listed as `blocked`, or `missing` when the file is not there, while the listing goes on. It checks each image and font on its own, so a document past the total limits can list as allowed and still stop when it renders. Stylesheets load to be listed, so they count toward the totals.

The input and the files named with `--font`, `--header`, and `--footer` are trusted, since you name them on the command line. What they reference goes through the same checks. When the document loads no font, text falls back to Geist.
