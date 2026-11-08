# takumi-cli

`takumi` renders an HTML file to PNG, JPEG, WebP, SVG, or PDF.

```sh
takumi render index.html -o card.png --width 1200 --height 630
takumi render invoice.html -o invoice.pdf --page letter
```

The output file's extension picks the format. The document's `<link rel="stylesheet">`, `@import`, `@font-face`, and image URLs load as they would in a browser, with each relative URL resolved against the file that holds it.

## What a document can read

A document reads files only from the directories it is granted, and it never reaches the network. This keeps an untrusted HTML file from pulling private files into its output.

- By default, the document may read files under the input file's directory. Reading from standard input grants no directory.
- `--root <DIR>` replaces that directory, and `--allow-read <DIR>` adds another. Repeat `--allow-read` for more.
- A path is checked as written before anything on disk is touched, so nothing outside a granted directory is probed. It is checked again after symlinks resolve, so a symlink cannot step outside either.
- `data:` URLs always load. `http` and `https` URLs never do, and neither does a `file:` URL that names another machine, such as a Windows network share. takumi skips them with a warning, as a browser skips a resource that fails to load.
- A read outside the granted directories stops the render, and the error names the `--allow-read` flag that would allow it.
- One resource may hold at most 32 MiB, all of them together 256 MiB, and a render may load at most 1024 of them.

`--list-resources` prints the images and fonts a document would load and whether each is allowed, without rendering.

Font files named with `--font` and the input file itself are trusted, since you name them on the command line. When the document loads no font, text falls back to Geist.
