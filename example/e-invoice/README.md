# e-invoice

Builds the container an EU e-invoice needs, from JSX, using [takumi-pdf](../../takumi-pdf-js).

An e-invoice is one file that has to satisfy two readers. A human opens it and sees an invoice. A machine opens it and reads `factur-x.xml`, the structured payload attached inside the PDF. The container has to be PDF/A-3, the only PDF/A level that allows arbitrary attachments.

The output is a valid PDF/A-3B and PDF/UA-1 document carrying a valid Factur-X MINIMUM payload. It is not yet a conforming Factur-X file: that also needs an `fx:` XMP block, which takumi-pdf cannot write today. See [Validating](#validating).

Build the wasm package first (needs [Rust](https://www.rust-lang.org/tools/install) and [wasm-pack](https://rustwasm.github.io/wasm-pack/installer/)):

```bash
cd takumi-pdf-js
bun run build
```

Then run the example:

```bash
cd ../example/e-invoice
bun index.tsx
```

Open `output/invoice.pdf`. The attachment panel holds `factur-x.xml`.

## What the example uses

`pdfa: "3b"` emits the archival container: an sRGB output intent, XMP metadata, and embedded subset fonts. A document that cannot conform rejects the render instead of writing a broken file.

`attachments` embeds the XML with the `AFRelationship` of `Data`, the filename `factur-x.xml`, and a media type, all of which Factur-X readers key on. `metadata.creationDate` supplies the attachment's modification date, so two runs produce identical bytes.

`measure` lays out the footer band on its own and returns its height. The bottom margin is that height plus a gap, so the footer never collides with the body no matter what the band contains.

`tagged: "ua1"` claims PDF/UA-1 on top of that. Headings, paragraphs, the table rows, and the logo's alt text land in the structure tree. The footer band, the backgrounds, and the borders are artifacts, so a screen reader skips them.

## Validating

Two validators cover the two halves of the file. Both need Java.

[veraPDF](https://verapdf.org/) is the reference implementation for PDF/A and PDF/UA. Download the installer, then run it against the output:

```bash
verapdf --flavour 3b --format text output/invoice.pdf
verapdf --flavour ua1 --format text output/invoice.pdf
```

```text
PASS output/invoice.pdf 3b
PASS output/invoice.pdf ua1
```

Swap `--format text` for `--format mrr` to see which rule failed and where.

The [Mustang](https://www.mustangproject.org/) CLI validates the Factur-X half, both the XML against the profile's schematron and the PDF against the Factur-X packaging rules:

```bash
java -jar Mustang-CLI.jar --action validate --source output/invoice.pdf
```

The XML section reports `valid`. The PDF section still reports errors, because Factur-X also wants an `fx:` XMP extension schema naming the profile and the attachment. takumi-pdf writes the PDF/A extension schemas but has no hook for a payload-specific one yet, so a full Factur-X container needs a post-processing step for that block today.
