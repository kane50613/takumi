# Generate a Factur-X PDF invoice

Render a Factur-X MINIMUM invoice from JSX with [`takumi-pdf`](../../takumi-pdf-js).

A Factur-X invoice is one file with two readers. A person opens the PDF and sees an invoice. Software reads `factur-x.xml`, the structured invoice data attached inside the PDF. Factur-X requires a PDF/A-3 container.

This example writes a PDF/A-3B file that is also PDF/UA-1. It carries the XML attachment and the `fx:` XMP block that identifies it.

## Run

Build the [PDF example packages](../README.md#pdf-examples) first. Then render the invoice:

```bash
cd example/e-invoice
bun index.tsx
```

Open `output/invoice.pdf`. The viewer's attachment panel lists `factur-x.xml`. The script loads Inter from Google Fonts, so it needs network access.

## What the example uses

- **`pdfa: "3b"`** writes the archival container. It has an sRGB output intent, XMP metadata, and embedded subset fonts. A document that cannot conform fails the render instead of writing a broken file.
- **`attachments`** embeds the XML with the file name `factur-x.xml`, a media type, and the `data` relationship. Factur-X fixes all three. `metadata.creationDate` also sets the attachment's modification date, so that date stays the same between runs.
- **`measure`** lays out the footer and returns its height. The bottom margin is that height plus a 32px gap, which keeps body content clear of the footer.
- **`tagged: "ua1"`** adds PDF/UA-1. Headings, paragraphs, table rows, and the logo's alt text go into the structure tree. The footer, backgrounds, and borders are artifacts, so screen readers skip them.
- **`metadata.xmp`** takes the `fx:` namespace and its properties as data. The renderer writes the values and the schema description PDF/A requires, so the two always match. It does not check the values. A wrong profile name shows up in validation, not at render time.

## Validate the output

Two validators cover the two halves of the file. Both need Java.

### PDF/A and PDF/UA

[veraPDF](https://verapdf.org/) checks PDF/A and PDF/UA. Install it with `brew install verapdf` or the installer from its site:

```bash
verapdf --flavour 3b --format text output/invoice.pdf
verapdf --flavour ua1 --format text output/invoice.pdf
```

```text
PASS output/invoice.pdf 3b
PASS output/invoice.pdf ua1
```

Use `--format mrr` instead of `--format text` to see which rule failed and where.

### Factur-X

The [Mustang](https://www.mustangproject.org/) CLI checks the XML against the profile's schematron and the PDF against the Factur-X packaging rules. It reads the `fx:` XMP block to find the profile and the attachment. Download the CLI jar from a [mustangproject release](https://github.com/ZUGFeRD/mustangproject/releases), such as 2.25.0:

```bash
curl -sLO https://github.com/ZUGFeRD/mustangproject/releases/download/core-2.25.0/Mustang-CLI-2.25.0.jar
java -jar Mustang-CLI-2.25.0.jar --action validate --source output/invoice.pdf
```

```text
<summary status="valid"/>
```
