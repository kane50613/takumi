# From `@react-pdf/renderer`

[pdf/migration/react-pdf.md](https://takumi.kane.tw/docs/pdf/migration/react-pdf.md) maps each component and option to `takumi-pdf`.

## Swap

1. **Replace the components.** `<Document>` goes away, because `render(template, options)` from `takumi-pdf` takes the tree, and `<Page size>` becomes the `size` option. Each `<Page>` after the first becomes a section with `break-before: page`, so short pages do not flow onto one. `<View>`, `<Text>`, and `<Image>` become HTML elements, as the component map lists. The swap is done when no `@react-pdf/renderer` component remains in the template.

2. **Keep the layout.** react-pdf makes every `<View>` a column flex container and measures lengths in pt. Convert the styles as the "Preserve the layout" section describes. The layout is done when every converted container states its `display` and `flex-direction`, and every length is in CSS px.

3. **Move the page bands.** A `fixed` `<View>` that repeats on each page becomes the `header` or `footer` option, and a `render` callback that prints page numbers becomes `<PageNumber />` and `<TotalPages />` from `takumi-pdf/primitives`. The bands are done when no element in the template body repeats per page.

4. **Move the assets.** `Font.register` calls become `fonts` entries, and `Font.registerEmojiSource` changes as the component map shows. The assets are done when no `Font` call remains.

## Workarounds to delete

- code that splits content into pages by hand, replaced by `break-before: page`, `break-inside: avoid`, and the pagination in [pdf/pagination.md](https://takumi.kane.tw/docs/pdf/pagination.md)
